//! The process runner on `std::process` (design part 4.2): the Linux and macOS stand-in for the Windows Job Object
//! runner, and what pull request 2 uses on every operating system (S13). It kills only the child it started, not the
//! child's own children, and ignores `Spawn::below_normal` (S43). Pull request 3's Job Object runner replaces it on
//! Windows, where closing the engine also ends the child.

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
fn handed_over<R: Read + Send + 'static>(pipe: Option<R>) -> Box<dyn Read + Send> {
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
mod tests {
    use std::io::{Read, Write};
    use std::sync::mpsc;
    use std::time::Duration;

    use super::*;
    use crate::{ProcessGuard, Spawn};

    fn helper(mode: &str) -> Spawn {
        Spawn {
            program: std::env::current_exe().unwrap(),
            args: ["--exact", "process::tests::helper_child", "--ignored", "--nocapture"].map(String::from).to_vec(),
            env: vec![("LAZYMKV_TEST_HELPER".to_string(), mode.to_string())],
            below_normal: false,
        }
    }

    /// The child the runner tests start: this test binary running only this test. It sleeps until killed when
    /// LAZYMKV_TEST_HELPER is `sleep`, prints one line to each stream when it is `echo`, and returns at once otherwise.
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
            _ => {}
        }
    }

    #[test]
    fn the_runner_kills_its_child() {
        let mut child = StdProcessGuard.spawn(&helper("sleep")).unwrap();
        std::thread::sleep(Duration::from_millis(300));
        let killer = child.killer();
        killer.kill().unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || tx.send(child.wait().map_err(|e| e.kind())).unwrap());
        let status = rx.recv_timeout(Duration::from_secs(10)).expect("the killed child did not end within 10 s");
        assert_ne!(status, Ok(Some(0)));
        assert!(killer.kill().is_ok(), "a kill after the exit is not an error");
    }

    #[test]
    fn the_runner_pipes_the_childs_output_and_passes_its_environment() {
        let mut child = StdProcessGuard.spawn(&helper("echo")).unwrap();
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
}
