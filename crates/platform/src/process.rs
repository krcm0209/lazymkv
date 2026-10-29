//! The process runner on `std::process` (design part 4.2): the runner on Linux and macOS, and on Windows the one the
//! driver tests in `crates/fake-tools/tests` keep (Ruling W7). It kills only the child it started, not the child's own
//! children, and ignores `Spawn::below_normal` (S43). The engine on Windows takes `win::JobProcessGuard` through
//! `native_guard`, which kills the tree.

use std::io::{self, Read};
use std::process::{Child, ChildStderr, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use crate::{GuardedChild, KillHandle, ProcessGuard, Spawn};

/// Starts children with `std::process::Command`: standard input null, standard output and error piped, `Spawn::env`
/// added to the inherited environment (Ruling T9-e, design part 4.2, S13). `below_normal` does nothing here.
/// Def: T9. Use: T11, T13, T23.
pub struct StdProcessGuard;

impl ProcessGuard for StdProcessGuard {
    fn spawn(&self, spawn: &Spawn) -> io::Result<Box<dyn GuardedChild>> {
        let mut child = Command::new(&spawn.program)
            .args(&spawn.args)
            .envs(spawn.env.iter().map(|(name, value)| (name, value)))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        Ok(Box::new(StdChild { child: Arc::new(Mutex::new(child)), stdout, stderr }))
    }
}

/// One child started by `StdProcessGuard`. The child sits behind a lock that `wait` takes only for each poll
/// (Ruling T9-a), so a `KillHandle` on another thread always gets it.
struct StdChild {
    child: Arc<Mutex<Child>>,
    stdout: Option<ChildStdout>,
    stderr: Option<ChildStderr>,
}

/// The pipe the first time, an empty reader after that (Ruling T9-d).
pub(crate) fn handed_over<R: Read + Send + 'static>(pipe: Option<R>) -> Box<dyn Read + Send> {
    match pipe {
        Some(pipe) => Box::new(pipe),
        None => Box::new(io::empty()),
    }
}

impl GuardedChild for StdChild {
    fn take_stdout(&mut self) -> Box<dyn Read + Send> {
        handed_over(self.stdout.take())
    }

    fn take_stderr(&mut self) -> Box<dyn Read + Send> {
        handed_over(self.stderr.take())
    }

    fn killer(&self) -> KillHandle {
        let child = Arc::clone(&self.child);
        KillHandle(Arc::new(move || {
            let mut child = child.lock().unwrap_or_else(PoisonError::into_inner);
            match child.kill() {
                // std reports a child that already ended as InvalidInput; a cancel or timeout racing the exit is no
                // error (Ruling T9-b).
                Err(error) if error.kind() == io::ErrorKind::InvalidInput => Ok(()),
                other => other,
            }
        }))
    }

    fn wait(&mut self) -> io::Result<Option<i32>> {
        loop {
            {
                let mut child = self.child.lock().unwrap_or_else(PoisonError::into_inner);
                if let Some(status) = child.try_wait()? {
                    return Ok(status.code());
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::io::{Read, Write};
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    use std::time::Duration;

    use super::*;
    use crate::{ProcessGuard, Spawn};

    /// The arguments that make this test binary run only `helper_child`.
    pub(crate) const HELPER_ARGS: [&str; 4] = ["--exact", "process::tests::helper_child", "--ignored", "--nocapture"];

    /// This test binary as the child `helper_child` runs in, in `mode`.
    pub(crate) fn helper(mode: &str) -> Spawn {
        Spawn {
            program: std::env::current_exe().unwrap(),
            args: HELPER_ARGS.map(String::from).to_vec(),
            env: vec![("LAZYMKV_TEST_HELPER".to_string(), mode.to_string())],
            below_normal: false,
        }
    }

    /// Writes `line` on a line of its own to standard output and flushes it (Ruling T3-5-c).
    fn say(line: &str) {
        let mut out = std::io::stdout();
        out.write_all(format!("\n{line}\n").as_bytes()).unwrap();
        out.flush().unwrap();
    }

    /// The child the runner tests start: this test binary running only this test, in the mode LAZYMKV_TEST_HELPER
    /// names. `sleep` sleeps until killed; `echo` prints one line to each stream; `spawn-grandchild` starts this test
    /// binary again as `print-pid-sleep`, with this child's standard output and error, then writes `child <pid>` and
    /// sleeps 60 s; `print-pid-sleep` writes `grandchild <pid>` and sleeps 60 s. Any other mode returns at once.
    #[test]
    #[ignore]
    fn helper_child() {
        match std::env::var("LAZYMKV_TEST_HELPER").as_deref() {
            Ok("sleep") => loop {
                std::thread::sleep(Duration::from_secs(60));
            },
            Ok("echo") => {
                std::io::stdout().write_all(b"helper-out\n").unwrap();
                std::io::stderr().write_all(b"helper-err\n").unwrap();
            }
            Ok("spawn-grandchild") => {
                // The first act: the grandchild starts before this child writes anything (Review Focus 3).
                let _grandchild = Command::new(std::env::current_exe().unwrap())
                    .args(HELPER_ARGS)
                    .env("LAZYMKV_TEST_HELPER", "print-pid-sleep")
                    .stdin(Stdio::null())
                    .spawn()
                    .unwrap();
                say(&format!("child {}", std::process::id()));
                std::thread::sleep(Duration::from_secs(60));
            }
            Ok("print-pid-sleep") => {
                say(&format!("grandchild {}", std::process::id()));
                std::thread::sleep(Duration::from_secs(60));
            }
            _ => {}
        }
    }

    /// The body of pull request 2's `the_runner_kills_its_child`, for any runner: a sleeping child is killed, its wait
    /// returns within 10 s with no success, and a kill after the exit is no error (Ruling T9-b).
    pub(crate) fn check_kills_its_child(guard: &dyn ProcessGuard) {
        let mut child = guard.spawn(&helper("sleep")).unwrap();
        std::thread::sleep(Duration::from_millis(300));
        let killer = child.killer();
        killer.kill().unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || tx.send(child.wait().map_err(|e| e.kind())).unwrap());
        let status = rx.recv_timeout(Duration::from_secs(10)).expect("the killed child did not end within 10 s");
        assert_ne!(status, Ok(Some(0)));
        assert!(killer.kill().is_ok(), "a kill after the exit is not an error");
    }

    /// The body of pull request 2's `the_runner_pipes_the_childs_output_and_passes_its_environment`, for any runner:
    /// both streams arrive, `Spawn::env` reaches the child, and a stream taken twice is empty the second time (Rulings
    /// T9-d, T9-e).
    pub(crate) fn check_pipes_both_streams_and_adds_env(guard: &dyn ProcessGuard) {
        let mut child = guard.spawn(&helper("echo")).unwrap();
        let mut out = String::new();
        child.take_stdout().read_to_string(&mut out).unwrap();
        let mut err = String::new();
        child.take_stderr().read_to_string(&mut err).unwrap();
        assert_eq!(child.wait().unwrap(), Some(0));
        assert!(out.contains("helper-out\n"), "{out}");
        assert!(err.contains("helper-err\n"), "{err}");
        let mut again = String::new();
        child.take_stdout().read_to_string(&mut again).unwrap();
        assert_eq!(again, "");
    }

    #[test]
    fn the_runner_kills_its_child() {
        check_kills_its_child(&StdProcessGuard);
    }

    #[test]
    fn the_runner_pipes_the_childs_output_and_passes_its_environment() {
        check_pipes_both_streams_and_adds_env(&StdProcessGuard);
    }
}
