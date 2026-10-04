//! Runs a command, tees its standard output to a file, and records when each line arrived.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

pub trait Clock {
    fn elapsed_ms(&self) -> u64;
}

pub struct RealClock(Instant);

impl RealClock {
    pub fn start() -> Self {
        RealClock(Instant::now())
    }
}

impl Clock for RealClock {
    fn elapsed_ms(&self) -> u64 {
        self.0.elapsed().as_millis() as u64
    }
}

/// Copies `reader` line by line to `out` and `echo`, and writes one timing
/// record per line to `timing`. Returns the number of lines.
pub fn tee_lines(
    reader: &mut dyn BufRead,
    out: &mut dyn Write,
    timing: &mut dyn Write,
    echo: &mut dyn Write,
    clock: &dyn Clock,
) -> io::Result<u64> {
    let mut line = Vec::new();
    let mut count = 0u64;
    loop {
        line.clear();
        if reader.read_until(b'\n', &mut line)? == 0 {
            break;
        }
        let arrived_ms = clock.elapsed_ms();
        out.write_all(&line)?;
        // The echo is a convenience: a reader that closes the pipe must not stop the recording.
        let _ = echo.write_all(&line);
        let _ = echo.flush();
        writeln!(timing, "{{\"n\":{},\"t_ms\":{}}}", count, arrived_ms)?;
        count += 1;
    }
    out.flush()?;
    timing.flush()?;
    Ok(count)
}

pub fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

/// Runs `command`. Standard output goes to `out_path`, line timing to
/// `<out_path>.timing.jsonl`, standard error to `<out_path>.stderr`, and the
/// exit code to `<out_path>.exit.json`. Returns the exit code.
pub fn run_recorded(command: &[String], out_path: &Path) -> io::Result<i32> {
    let (program, args) = command
        .split_first()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no command given"))?;
    if let Some(parent) = out_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let stderr_file = File::create(with_suffix(out_path, ".stderr"))?;
    let clock = RealClock::start();
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::from(stderr_file))
        .spawn()?;
    let mut reader = BufReader::new(child.stdout.take().expect("stdout was piped"));
    let mut out = File::create(out_path)?;
    let mut timing = File::create(with_suffix(out_path, ".timing.jsonl"))?;
    let mut echo = io::stdout();
    let lines = tee_lines(&mut reader, &mut out, &mut timing, &mut echo, &clock)?;
    let code = child.wait()?.code().unwrap_or(-1);
    std::fs::write(
        with_suffix(out_path, ".exit.json"),
        format!(
            "{{\"exit_code\":{},\"elapsed_ms\":{},\"lines\":{}}}\n",
            code,
            clock.elapsed_ms(),
            lines
        ),
    )?;
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::temp_dir;
    use std::cell::Cell;
    use std::io::Cursor;

    struct StepClock(Cell<u64>);

    impl Clock for StepClock {
        fn elapsed_ms(&self) -> u64 {
            let now = self.0.get();
            self.0.set(now + 10);
            now
        }
    }

    struct BrokenPipe;

    impl std::io::Write for BrokenPipe {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "closed"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "closed"))
        }
    }

    #[test]
    fn tee_copies_lines_and_records_their_timing() {
        let input = b"MSG:1005,0,1,\"a\"\nTCOUNT:2\nlast line without newline".to_vec();
        let mut reader = Cursor::new(input.clone());
        let (mut out, mut timing, mut echo) = (Vec::new(), Vec::new(), Vec::new());

        let lines = tee_lines(&mut reader, &mut out, &mut timing, &mut echo, &StepClock(Cell::new(5))).unwrap();

        assert_eq!(lines, 3);
        assert_eq!(out, input);
        assert_eq!(echo, input);
        assert_eq!(
            String::from_utf8(timing).unwrap(),
            "{\"n\":0,\"t_ms\":5}\n{\"n\":1,\"t_ms\":15}\n{\"n\":2,\"t_ms\":25}\n"
        );
    }

    #[test]
    fn run_recorded_captures_output_and_the_exit_code() {
        let out = temp_dir("rec").join("scan.txt");
        let command: Vec<String> = if cfg!(windows) {
            vec!["cmd".into(), "/c".into(), "echo hello& exit /b 3".into()]
        } else {
            vec!["sh".into(), "-c".into(), "echo hello; exit 3".into()]
        };

        let code = run_recorded(&command, &out).unwrap();

        assert_eq!(code, 3);
        assert_eq!(std::fs::read_to_string(&out).unwrap().trim(), "hello");
        let exit = std::fs::read_to_string(with_suffix(&out, ".exit.json")).unwrap();
        assert!(exit.contains("\"exit_code\":3"), "{exit}");
        assert!(exit.contains("\"lines\":1"), "{exit}");
        assert!(with_suffix(&out, ".timing.jsonl").exists());
        assert!(with_suffix(&out, ".stderr").exists());
    }

    #[test]
    fn tee_keeps_recording_when_the_echo_reader_is_gone() {
        let input = b"one\ntwo\n".to_vec();
        let mut reader = Cursor::new(input.clone());
        let (mut out, mut timing) = (Vec::new(), Vec::new());

        let lines = tee_lines(&mut reader, &mut out, &mut timing, &mut BrokenPipe, &StepClock(Cell::new(0))).unwrap();

        assert_eq!(lines, 2);
        assert_eq!(out, input);
        assert_eq!(String::from_utf8(timing).unwrap().lines().count(), 2);
    }
}
