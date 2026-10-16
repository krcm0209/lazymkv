//! Running ffprobe and ffmpeg (port spec 3.1): the trait the commands call, the real programs, and a fake for tests,
//! so that the Python's mocked subprocess tests port as they are.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::io;
use std::path::{Path, PathBuf};

/// What a finished program gave: exit status (negative signal number when killed, as subprocess), and its output
/// decoded as UTF-8 (lossily) with universal newlines, as subprocess.run(..., text=True) reads it.
/// Py: subprocess.CompletedProcess.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Output {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Runs ffprobe and ffmpeg. Py: subprocess.run(args, cwd=..., capture_output=True, text=True).
pub trait Runner {
    /// Runs `program` with `args` in `cwd` (None: the current directory); Err when it cannot be started
    /// (io::ErrorKind::NotFound when it is not on PATH).
    fn run(&self, program: &str, args: &[String], cwd: Option<&Path>) -> io::Result<Output>;
}

/// std::process::Command.
pub struct RealRunner;

impl Runner for RealRunner {
    fn run(&self, program: &str, args: &[String], cwd: Option<&Path>) -> io::Result<Output> {
        let mut command = std::process::Command::new(program);
        command.args(args);
        if let Some(dir) = cwd {
            command.current_dir(dir);
        }
        let output = command.output()?;
        Ok(Output { status: exit_status(&output.status), stdout: text_of(&output.stdout), stderr: text_of(&output.stderr) })
    }
}

/// The exit code, or minus the signal number when a signal ended the program (subprocess's returncode).
#[cfg(unix)]
fn exit_status(status: &std::process::ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;
    status.code().or_else(|| status.signal().map(|signal| -signal)).unwrap_or(-1)
}

/// The exit code (Windows has no signals).
#[cfg(not(unix))]
fn exit_status(status: &std::process::ExitStatus) -> i32 {
    status.code().unwrap_or(-1)
}

/// text=True: UTF-8 (here lossily, where Python would raise) with "\r\n" and "\r" read as "\n".
fn text_of(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).replace("\r\n", "\n").replace('\r', "\n")
}

/// One call a FakeRunner received.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Call {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
}

/// The handler a FakeRunner answers with: it may write files into `cwd` (as the Python test's fake ffmpeg writes
/// raw-<series>.txt).
pub type Handler = Box<dyn Fn(&str, &[String], Option<&Path>) -> io::Result<Output>>;

/// A Runner for tests (the mocked `subprocess.run` and `proxy_tracks` of ProxyStreams port through it).
pub struct FakeRunner {
    handler: Handler,
    calls: RefCell<Vec<Call>>,
}

impl FakeRunner {
    pub fn new(handler: Handler) -> FakeRunner {
        FakeRunner { handler, calls: RefCell::new(Vec::new()) }
    }

    /// Every program is missing (NotFound): the "ffmpeg absent" skip path.
    pub fn absent() -> FakeRunner {
        FakeRunner::new(Box::new(|program: &str, _args: &[String], _cwd: Option<&Path>| -> io::Result<Output> {
            Err(io::Error::new(io::ErrorKind::NotFound, format!("{}: not found (FakeRunner::absent)", program)))
        }))
    }

    /// Answers the k-th call with the k-th output; a call past the end panics.
    pub fn scripted(outputs: Vec<Output>) -> FakeRunner {
        let queue = RefCell::new(VecDeque::from(outputs));
        FakeRunner::new(Box::new(move |program: &str, _args: &[String], _cwd: Option<&Path>| -> io::Result<Output> {
            match queue.borrow_mut().pop_front() {
                Some(output) => Ok(output),
                None => panic!("FakeRunner::scripted has no output left for {}", program),
            }
        }))
    }

    /// The calls so far, in order.
    pub fn calls(&self) -> Vec<Call> {
        self.calls.borrow().clone()
    }
}

impl Runner for FakeRunner {
    fn run(&self, program: &str, args: &[String], cwd: Option<&Path>) -> io::Result<Output> {
        self.calls.borrow_mut().push(Call {
            program: program.to_string(),
            args: args.to_vec(),
            cwd: cwd.map(Path::to_path_buf),
        });
        (self.handler)(program, args, cwd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_runner_records_each_call_and_answers_from_its_handler() {
        let runner = FakeRunner::new(Box::new(
            |program: &str, args: &[String], cwd: Option<&Path>| -> io::Result<Output> {
                Ok(Output {
                    status: 0,
                    stdout: format!("{} {} {}", program, args.join(" "), cwd.is_some()),
                    stderr: String::new(),
                })
            },
        ));
        let args = vec!["-v".to_string(), "error".to_string()];
        let output = runner.run("ffprobe", &args, Some(Path::new("/work"))).unwrap();
        assert_eq!(output.stdout, "ffprobe -v error true");
        assert_eq!(
            runner.calls(),
            vec![Call { program: "ffprobe".to_string(), args, cwd: Some(PathBuf::from("/work")) }]
        );
    }

    #[test]
    fn an_absent_fake_runner_says_not_found() {
        let error = FakeRunner::absent().run("ffmpeg", &[], None).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn a_scripted_fake_runner_answers_in_order() {
        let first = Output { status: 0, stdout: "one".to_string(), stderr: String::new() };
        let second = Output { status: 1, stdout: String::new(), stderr: "two".to_string() };
        let runner = FakeRunner::scripted(vec![first.clone(), second.clone()]);
        assert_eq!(runner.run("ffprobe", &[], None).unwrap(), first);
        assert_eq!(runner.run("ffmpeg", &[], None).unwrap(), second);
        assert_eq!(runner.calls().len(), 2);
    }

    #[test]
    fn the_real_runner_reports_a_missing_program_as_not_found() {
        let error = RealRunner.run("lazymkv-no-such-program", &[], None).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn output_text_is_read_with_universal_newlines() {
        assert_eq!(text_of(b"a\r\nb\rc\n\xff"), "a\nb\nc\n\u{fffd}");
    }
}
