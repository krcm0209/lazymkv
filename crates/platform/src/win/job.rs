//! The Job Object runner (design part 4.2; Rulings W7, T3-5-a, T3-5-b): every child of the engine in a Job Object of
//! its own, created suspended and assigned to the job before its first instruction, so a kill, or the engine's end,
//! takes the child's whole tree with it, the grandchildren it starts at once included.

use std::io::{self, Read};
use std::mem::size_of;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::os::windows::process::CommandExt;
use std::process::{Child, ChildStderr, ChildStdout, Command, Stdio};
use std::ptr::null;
use std::sync::Arc;

use windows_sys::Win32::Foundation::{ERROR_NO_MORE_FILES, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
    TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows_sys::Win32::System::Threading::{
    OpenThread, ResumeThread, BELOW_NORMAL_PRIORITY_CLASS, CREATE_SUSPENDED, THREAD_SUSPEND_RESUME,
};

use crate::process::handed_over;
use crate::{GuardedChild, KillHandle, ProcessGuard, Spawn};

/// The Windows `ProcessGuard` (spec 4.3, design part 4.2; Ruling W7): each child in a Job Object of its own with
/// `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, started suspended and assigned before its first instruction, at
/// `BELOW_NORMAL_PRIORITY_CLASS` when `Spawn::below_normal`. A kill is `TerminateJobObject(job, 1)` and ends the
/// child's whole tree; so does the engine's end, since closing a job's last handle kills what it holds (design part
/// 3.8). Standard input null, output and error piped, `Spawn::env` added (Rulings T9-a to T9-e hold).
/// Def: T3-5. Use: T3-23.
pub struct JobProcessGuard;

impl ProcessGuard for JobProcessGuard {
    fn spawn(&self, spawn: &Spawn) -> io::Result<Box<dyn GuardedChild>> {
        // Nothing has started if the job cannot be made.
        let job = Job::new()?;
        let flags = if spawn.below_normal { CREATE_SUSPENDED | BELOW_NORMAL_PRIORITY_CLASS } else { CREATE_SUSPENDED };
        // std's own spawn keeps its handle discipline, so a concurrent spawn never inherits this child's pipe ends. Its
        // error is returned unchanged (a missing program is NotFound) while the empty job closes.
        let mut child = Command::new(&spawn.program)
            .args(&spawn.args)
            .envs(spawn.env.iter().map(|(name, value)| (name, value)))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .creation_flags(flags)
            .spawn()?;
        // The child has run no instruction yet, so whatever it starts is in the job too (Review Focus 3).
        if let Err(error) = job.assign(&child) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        if let Err(error) = resume(&child) {
            // A wait on a child still suspended would never return, so when the job cannot be ended the child is
            // killed on its own.
            if job.terminate().is_err() {
                let _ = child.kill();
            }
            let _ = child.wait();
            return Err(error);
        }
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        Ok(Box::new(JobChild { child, job: Arc::new(job), stdout, stderr }))
    }
}

/// One Job Object; dropping the last `Arc` of it closes the handle and so kills what the job still holds.
struct Job(OwnedHandle);

impl Job {
    /// A new, unnamed job whose handle no child inherits, with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` and no other limit.
    fn new() -> io::Result<Job> {
        // SAFETY: both pointers may be null: default security, so the handle is not inherited, and no name.
        let raw = unsafe { CreateJobObjectW(null(), null()) };
        if raw.is_null() {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: raw is an open job handle that nothing else owns.
        let job = Job(unsafe { OwnedHandle::from_raw_handle(raw) });
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let length = size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32;
        // SAFETY: the job handle is open; `info` is the structure JobObjectExtendedLimitInformation reads, it outlives
        // the call, and `length` is its size.
        let set = unsafe {
            SetInformationJobObject(
                job.0.as_raw_handle(),
                JobObjectExtendedLimitInformation,
                (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                length,
            )
        };
        if set == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(job)
    }

    /// Puts `child`, still suspended, in this job.
    fn assign(&self, child: &Child) -> io::Result<()> {
        // SAFETY: the job handle is open, and `child`'s process handle is open while `child` lives.
        if unsafe { AssignProcessToJobObject(self.0.as_raw_handle(), child.as_raw_handle()) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    /// Ends every process the job holds, with exit code 1; a job whose processes have all ended takes it as no error.
    fn terminate(&self) -> io::Result<()> {
        // SAFETY: the job handle is open for the whole call.
        if unsafe { TerminateJobObject(self.0.as_raw_handle(), 1) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

/// The suspended child's main thread resumed (Ruling T3-5-b). A process created suspended has exactly one thread,
/// found in a Toolhelp snapshot of the system's threads by its owner, `child.id()`, opened with
/// `THREAD_SUSPEND_RESUME` and resumed. A `ResumeThread` return of `u32::MAX` is a failure.
fn resume(child: &Child) -> io::Result<()> {
    let pid = child.id();
    // SAFETY: CreateToolhelp32Snapshot takes no pointer; an INVALID_HANDLE_VALUE result is checked below.
    let raw = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    if raw == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: raw is an open snapshot handle that nothing else owns.
    let snapshot = unsafe { OwnedHandle::from_raw_handle(raw) };
    let mut entry = THREADENTRY32 { dwSize: size_of::<THREADENTRY32>() as u32, ..THREADENTRY32::default() };
    let mut thread_id = None;
    // SAFETY: the snapshot is open, and `entry` is a THREADENTRY32 whose dwSize says its size; it is read only after a
    // call that returned non-zero filled it.
    let mut filled = unsafe { Thread32First(snapshot.as_raw_handle(), &mut entry) };
    while filled != 0 {
        if entry.th32OwnerProcessID == pid {
            thread_id = Some(entry.th32ThreadID);
            break;
        }
        // SAFETY: as for Thread32First.
        filled = unsafe { Thread32Next(snapshot.as_raw_handle(), &mut entry) };
    }
    let Some(thread_id) = thread_id else {
        // The last call returned zero: the end of the list is ERROR_NO_MORE_FILES, anything else is its error.
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(ERROR_NO_MORE_FILES as i32) {
            return Err(error);
        }
        return Err(io::Error::new(io::ErrorKind::NotFound, format!("process {pid} has no thread in the snapshot")));
    };
    drop(snapshot);
    // SAFETY: OpenThread takes no pointer; a null result is checked below.
    let raw = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, thread_id) };
    if raw.is_null() {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: raw is an open thread handle that nothing else owns.
    let thread = unsafe { OwnedHandle::from_raw_handle(raw) };
    // SAFETY: the thread handle is open with THREAD_SUSPEND_RESUME for the whole call.
    if unsafe { ResumeThread(thread.as_raw_handle()) } == u32::MAX {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// One child of `JobProcessGuard`; its `KillHandle`s hold the same `Arc<Job>` (Ruling T3-5-a).
struct JobChild {
    child: Child,
    job: Arc<Job>,
    stdout: Option<ChildStdout>,
    stderr: Option<ChildStderr>,
}

impl GuardedChild for JobChild {
    fn take_stdout(&mut self) -> Box<dyn Read + Send> {
        handed_over(self.stdout.take())
    }

    fn take_stderr(&mut self) -> Box<dyn Read + Send> {
        handed_over(self.stderr.take())
    }

    fn killer(&self) -> KillHandle {
        let job = Arc::clone(&self.job);
        KillHandle(Arc::new(move || job.terminate()))
    }

    /// A blocking wait with no lock: a kill goes through the job and never touches the `Child` (Ruling T9-a).
    fn wait(&mut self) -> io::Result<Option<i32>> {
        Ok(self.child.wait()?.code())
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self, BufRead, BufReader, Read};
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcess, GetPriorityClass, OpenProcess, WaitForSingleObject, BELOW_NORMAL_PRIORITY_CLASS,
        IDLE_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
    };

    use super::JobProcessGuard;
    use crate::process::tests::{check_kills_its_child, check_pipes_both_streams_and_adds_env, helper};
    use crate::testutil::{ci_factor, temp_dir};
    use crate::{GuardedChild, ProcessGuard, Spawn};

    /// How long a killed tree may take to end. A hang is the failure it catches, so it does not scale (Global
    /// Constraints).
    const ENDS_WITHIN: Duration = Duration::from_secs(10);

    /// A handle to the running process `pid`, opened while it runs, so a reused id cannot answer for it.
    fn open_process(pid: u32) -> OwnedHandle {
        // SAFETY: OpenProcess takes no pointer; a null result is checked before the handle is owned.
        let raw = unsafe { OpenProcess(PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        assert!(!raw.is_null(), "process {pid} could not be opened: {}", io::Error::last_os_error());
        // SAFETY: raw is an open handle that nothing else owns.
        unsafe { OwnedHandle::from_raw_handle(raw) }
    }

    /// Whether `process` ended within `bound`.
    fn ended_within(process: &OwnedHandle, bound: Duration) -> bool {
        // SAFETY: the handle is open for the whole call.
        unsafe { WaitForSingleObject(process.as_raw_handle(), bound.as_millis() as u32) == WAIT_OBJECT_0 }
    }

    /// `process`'s priority class.
    fn priority(process: &OwnedHandle) -> u32 {
        // SAFETY: the handle is open and has PROCESS_QUERY_LIMITED_INFORMATION.
        unsafe { GetPriorityClass(process.as_raw_handle()) }
    }

    /// `child`'s standard output read on a thread: each line, then None at the end of file.
    fn lines_of(child: &mut dyn GuardedChild) -> mpsc::Receiver<Option<String>> {
        let reader = BufReader::new(child.take_stdout());
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for line in reader.lines().map_while(Result::ok) {
                if tx.send(Some(line)).is_err() {
                    return;
                }
            }
            let _ = tx.send(None);
        });
        rx
    }

    /// The process ids the `spawn-grandchild` helper writes, as (child, grandchild), within 10 s (scaled).
    fn pids(lines: &mpsc::Receiver<Option<String>>) -> (u32, u32) {
        let deadline = Instant::now() + Duration::from_secs(10) * ci_factor();
        let (mut child, mut grandchild) = (None, None);
        while child.is_none() || grandchild.is_none() {
            match lines.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(Some(line)) => {
                    if let Some(pid) = line.strip_prefix("child ") {
                        child = pid.trim().parse().ok();
                    }
                    if let Some(pid) = line.strip_prefix("grandchild ") {
                        grandchild = pid.trim().parse().ok();
                    }
                }
                other => panic!("the helper's process ids did not come: {other:?}"),
            }
        }
        (child.unwrap(), grandchild.unwrap())
    }

    /// Whether the reader of `lines_of` reached the end of file within `bound`.
    fn end_of_file_within(lines: &mpsc::Receiver<Option<String>>, bound: Duration) -> bool {
        let deadline = Instant::now() + bound;
        loop {
            match lines.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(None) | Err(mpsc::RecvTimeoutError::Disconnected) => return true,
                Ok(Some(_)) => {}
                Err(mpsc::RecvTimeoutError::Timeout) => return false,
            }
        }
    }

    /// `spawn` under the job runner, its output read on a thread, and handles to the child and the grandchild the
    /// `spawn-grandchild` helper names, opened while both run.
    fn start_tree(spawn: &Spawn) -> (Box<dyn GuardedChild>, mpsc::Receiver<Option<String>>, [OwnedHandle; 2]) {
        let mut child = JobProcessGuard.spawn(spawn).unwrap();
        let lines = lines_of(&mut *child);
        let (parent, grandchild) = pids(&lines);
        (child, lines, [open_process(parent), open_process(grandchild)])
    }

    #[test]
    fn the_job_runner_kills_its_child() {
        check_kills_its_child(&JobProcessGuard);
    }

    #[test]
    fn the_job_runner_pipes_both_streams_and_adds_env() {
        check_pipes_both_streams_and_adds_env(&JobProcessGuard);
    }

    /// Review Focus 3: the helper's first act starts the grandchild, so a job assigned after the child ran would miss it.
    #[test]
    fn a_grandchild_started_at_once_is_killed_with_its_parent() {
        let (mut child, lines, [parent, grandchild]) = start_tree(&helper("spawn-grandchild"));
        child.killer().kill().unwrap();
        assert!(ended_within(&parent, ENDS_WITHIN), "the child outlived the kill");
        assert!(ended_within(&grandchild, ENDS_WITHIN), "the grandchild outlived the kill");
        assert!(end_of_file_within(&lines, ENDS_WITHIN), "standard output stayed open after the kill");
        assert_ne!(child.wait().unwrap(), Some(0));
    }

    /// Ruling T9-b, and Ruling T3-5-a's kill handle that outlives its child.
    #[test]
    fn kill_after_exit_is_ok() {
        let mut child = JobProcessGuard.spawn(&helper("echo")).unwrap();
        let killer = child.killer();
        io::copy(&mut child.take_stdout(), &mut io::sink()).unwrap();
        assert_eq!(child.wait().unwrap(), Some(0));
        assert!(killer.kill().is_ok() && killer.kill().is_ok(), "a kill after the exit is no error");
        drop(child);
        assert!(killer.kill().is_ok(), "nor is a kill from a handle that outlived its child");
    }

    /// Ruling T9-a: `wait` holds no lock a kill needs.
    #[test]
    fn a_kill_from_another_thread_gets_through_while_wait_runs() {
        let mut child = JobProcessGuard.spawn(&helper("sleep")).unwrap();
        let killer = child.killer();
        let (tx, rx) = mpsc::channel();
        let waiter = std::thread::spawn(move || tx.send(child.wait().map_err(|error| error.kind())).unwrap());
        std::thread::sleep(Duration::from_millis(300));
        killer.kill().unwrap();
        let status = rx.recv_timeout(ENDS_WITHIN).expect("wait did not return after the kill");
        assert_ne!(status, Ok(Some(0)));
        waiter.join().unwrap();
    }

    /// Ruling T9-d, for both streams.
    #[test]
    fn the_streams_are_given_once_then_empty() {
        let mut child = JobProcessGuard.spawn(&helper("echo")).unwrap();
        let (mut out, mut err) = (String::new(), String::new());
        child.take_stdout().read_to_string(&mut out).unwrap();
        child.take_stderr().read_to_string(&mut err).unwrap();
        assert_eq!(child.wait().unwrap(), Some(0));
        assert!(out.contains("helper-out\n") && err.contains("helper-err\n"), "{out:?} {err:?}");
        let (mut out, mut err) = (String::new(), String::new());
        child.take_stdout().read_to_string(&mut out).unwrap();
        child.take_stderr().read_to_string(&mut err).unwrap();
        assert_eq!((out.as_str(), err.as_str()), ("", ""));
    }

    /// Spec 4.3 and design part 5.3: `mkvmerge`, `ffprobe`, `ffmpeg` and the optical character recognition (OCR) worker
    /// run below normal, and what they start inherits it. Without the flag the child gets the class Windows gives any
    /// child of this process: normal, unless this process itself runs idle or below normal.
    #[test]
    fn below_normal_is_the_childs_priority_class() {
        // SAFETY: GetCurrentProcess returns a pseudo handle that needs no closing; GetPriorityClass only reads it.
        let own = unsafe { GetPriorityClass(GetCurrentProcess()) };
        let inherited =
            if own == IDLE_PRIORITY_CLASS || own == BELOW_NORMAL_PRIORITY_CLASS { own } else { NORMAL_PRIORITY_CLASS };
        for (below_normal, expected) in [(true, BELOW_NORMAL_PRIORITY_CLASS), (false, inherited)] {
            let (mut child, _lines, [parent, grandchild]) =
                start_tree(&Spawn { below_normal, ..helper("spawn-grandchild") });
            assert_eq!(priority(&parent), expected, "the child, below_normal {below_normal}");
            if below_normal {
                assert_eq!(priority(&grandchild), BELOW_NORMAL_PRIORITY_CLASS, "a below-normal child's grandchild");
            }
            child.killer().kill().unwrap();
            child.wait().unwrap();
        }
    }

    /// The engine runs several tools at once and the harness tests run in parallel in one process.
    #[test]
    fn killing_one_childs_tree_leaves_another_guarded_child_running() {
        let (mut first, _first_lines, first_tree) = start_tree(&helper("spawn-grandchild"));
        let (mut second, _second_lines, second_tree) = start_tree(&helper("spawn-grandchild"));
        first.killer().kill().unwrap();
        assert!(first_tree.iter().all(|process| ended_within(process, ENDS_WITHIN)), "the first tree outlived its kill");
        assert!(
            second_tree.iter().all(|process| !ended_within(process, Duration::ZERO)),
            "the second tree ended with the first"
        );
        second.killer().kill().unwrap();
        assert!(second_tree.iter().all(|process| ended_within(process, ENDS_WITHIN)));
        first.wait().unwrap();
        second.wait().unwrap();
    }

    /// Kill-on-close: the guarded child dropped, with no kill handle taken, closes the job, as the engine's end does.
    #[test]
    fn dropping_the_guarded_child_ends_its_tree() {
        let (child, lines, tree) = start_tree(&helper("spawn-grandchild"));
        drop(child);
        assert!(tree.iter().all(|process| ended_within(process, ENDS_WITHIN)), "kill-on-close did not end the tree");
        assert!(end_of_file_within(&lines, ENDS_WITHIN));
    }

    #[test]
    fn a_program_that_cannot_start_is_the_os_error() {
        let dir = temp_dir("job-missing");
        let spawn = Spawn { program: dir.join("absent.exe"), args: Vec::new(), env: Vec::new(), below_normal: false };
        let error = JobProcessGuard.spawn(&spawn).err().expect("an absent program does not start");
        std::fs::remove_dir(&dir).unwrap();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }
}
