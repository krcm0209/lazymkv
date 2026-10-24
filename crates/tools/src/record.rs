//! Runs a tool, tees its standard output to a file, and records when each line arrived (design parts 5.3, 7.2): the
//! one recorder every driver uses, with the limits, the cancel token and the line actions of design part 5.3.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use lazymkv_core::ipc::OutputStream;
use lazymkv_core::settings::Settings;
use lazymkv_platform::{Host, ProcessGuard, Spawn};

/// Tells how many milliseconds have passed since a run began (design part 7.2). It is a trait so that the tests of
/// `tee_lines` can step a clock by hand. Moved from capture. Def: T11. Use: T13, T14, T23.
pub trait Clock {
    /// Milliseconds since the clock started.
    fn elapsed_ms(&self) -> u64;
}

/// The clock that reads the operating system's monotonic time (design part 7.2). Moved from capture.
/// Def: T11. Use: T13, T14, T23.
pub struct RealClock(Instant);

impl RealClock {
    /// Starts a clock at zero now (design part 7.2). Def: T11. Use: T13, T14, T23.
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
/// record per line to `timing`. Returns the number of lines. Moved from capture so that there is one recorder
/// (design part 7.2, S17). Def: T11. Use: T13, T14, T23.
pub fn tee_lines(
    reader: &mut dyn BufRead,
    out: &mut dyn Write,
    timing: &mut dyn Write,
    echo: &mut dyn Write,
    clock: &dyn Clock,
) -> io::Result<u64> {
    tee_each(reader, out, timing, clock, &mut |line| {
        // The echo is a convenience: a reader that closes the pipe must not stop the recording.
        let _ = echo.write_all(line);
        let _ = echo.flush();
    })
}

/// The loop of `tee_lines`: copies each line of `reader` to `out`, calls `each` with the line's bytes, and writes its
/// timing record. Returns the number of lines.
fn tee_each(
    reader: &mut dyn BufRead,
    out: &mut dyn Write,
    timing: &mut dyn Write,
    clock: &dyn Clock,
    each: &mut dyn FnMut(&[u8]),
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
        each(&line);
        writeln!(timing, "{{\"n\":{},\"t_ms\":{}}}", count, arrived_ms)?;
        count += 1;
    }
    out.flush()?;
    timing.flush()?;
    Ok(count)
}

/// `path` with `suffix` added to its file name, such as `scan.txt` and `.stderr` (design part 7.2). Moved from
/// capture. Def: T11. Use: T13, T14, T23.
pub fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

/// How long a tool may run before the engine kills it (design part 5.3). Def: T11. Use: T13, T14, T23.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// The longest a header call, such as `mkvmerge -J` or `ffprobe`, may take.
    pub header: Duration,
    /// The longest a MakeMKV scan may take (`scan_timeout_min`).
    pub scan: Duration,
    /// The longest to wait for MakeMKV to exit after its last line (`exit_wait_min`).
    pub exit_wait: Duration,
    /// The longest to wait for a killed tool to be gone.
    pub after_kill: Duration,
}

impl Limits {
    /// The limits of design part 5.3: 120 s for a header call, the settings' scan and exit-wait minutes, and 10 s
    /// after a kill. Def: T11. Use: T13, T14, T23.
    pub fn from_settings(settings: &Settings) -> Limits {
        Limits {
            header: Duration::from_secs(120),
            scan: Duration::from_secs(u64::from(settings.scan_timeout_min) * 60),
            exit_wait: Duration::from_secs(u64::from(settings.exit_wait_min) * 60),
            after_kill: Duration::from_secs(10),
        }
    }
}

/// A flag the engine raises to stop a running tool (design part 5.3). Every clone shares the flag.
/// Def: T11. Use: T13, T14, T23.
#[derive(Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    /// Raises the flag (design part 5.3). Def: T11. Use: T13, T14, T23.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Whether the flag is raised (design part 5.3). Def: T11. Use: T13, T14, T23.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// What a driver needs to run a tool (design part 5.3): the process runner, the host, the limits, the cancel flag and
/// the two callbacks. Def: T11. Use: T13, T14, T23.
pub struct ToolCtx<'a> {
    /// Starts the tool.
    pub guard: &'a dyn ProcessGuard,
    /// The clock behind `RunResult::started_unix`.
    pub host: &'a dyn Host,
    /// How long each kind of call may take.
    pub limits: &'a Limits,
    /// Raised to stop the run.
    pub cancel: CancelToken,
    /// Called with each line of the tool's standard output and error, as it arrives.
    pub on_output: &'a (dyn Fn(OutputStream, &str) + Sync),
    /// Called with the fraction done, 0.0 to 1.0, when the tool reports one.
    pub on_progress: &'a (dyn Fn(f64) + Sync),
}

/// How a recorded run ended (design parts 5.3, 7.2). Def: T11. Use: T13, T14, T23.
#[derive(Clone, Debug)]
pub struct RunResult {
    /// The tool's exit code, or None when it ended without one.
    pub exit_code: Option<i32>,
    /// The number of lines of standard output.
    pub lines: u64,
    /// Milliseconds from the start of the run to its end.
    pub elapsed_ms: u64,
    /// When the run began, in seconds since the Unix epoch, from `Host::now_unix`.
    pub started_unix: u64,
    /// Whether the run was killed for passing its limit.
    pub timed_out: bool,
    /// Whether the run was killed because the cancel flag was raised.
    pub cancelled: bool,
    /// Every byte the tool wrote to standard output.
    pub stdout: Vec<u8>,
}

/// What `run_recorded`'s line callback tells it to do next (design part 5.3). Def: T11. Use: T13, T14, T23.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineAction {
    /// Keep running.
    Continue,
    /// Kill the tool. Reading goes on to the end of its output.
    Kill,
}

/// A line without its `\n` and the `\r` before it, with any byte that is not UTF-8 replaced (PIPE-14).
fn line_text(line: &[u8]) -> String {
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    let line = line.strip_suffix(b"\r").unwrap_or(line);
    String::from_utf8_lossy(line).into_owned()
}

/// Runs `program` with `args` under `ctx.guard`. Standard output goes to `out`, line timing to `<out>.timing.jsonl`,
/// standard error to `<out>.stderr`, and the exit code to `<out>.exit.json` (design part 7.2). The output and error
/// files keep every byte the tool wrote. Each line also goes to `ctx.on_output` and, for standard output, to
/// `on_line`; both get the line without its line ending, decoded lossily (PIPE-14). The run is killed when `limit`
/// passes since the start (`timed_out`), when `ctx.cancel` is raised (`cancelled`), or when `on_line` returns
/// `LineAction::Kill`; of the first two, only the one that came first is recorded. A program that cannot start gives `Err` before any run file is
/// written. The exit file holds -1 when the tool had no exit code. Design parts 5.3, 7.2.
/// Def: T11. Use: T13, T14, T23.
pub fn run_recorded(
    ctx: &ToolCtx,
    program: &Path,
    args: &[String],
    out: &Path,
    limit: Option<Duration>,
    below_normal: bool,
    on_line: &mut dyn FnMut(&str) -> LineAction,
) -> io::Result<RunResult> {
    if let Some(parent) = out.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let started_unix = ctx.host.now_unix();
    let clock = RealClock::start();
    let spawn = Spawn { program: program.to_path_buf(), args: args.to_vec(), env: Vec::new(), below_normal };
    let mut child = ctx.guard.spawn(&spawn)?;

    let mut out_file = File::create(out)?;
    let mut timing = File::create(with_suffix(out, ".timing.jsonl"))?;
    let mut stderr_file = File::create(with_suffix(out, ".stderr"))?;
    let mut stdout_reader = BufReader::new(child.take_stdout());
    let stderr_reader = BufReader::new(child.take_stderr());
    let killer = child.killer();

    let done = AtomicBool::new(false);
    let timed_out = AtomicBool::new(false);
    let cancelled = AtomicBool::new(false);
    let mut stdout = Vec::new();
    let (tee, waited, stderr_outcome) = std::thread::scope(|scope| {
        // The tool's standard error is drained on its own thread: a tool that writes a lot to it would otherwise fill
        // the pipe and stop before it ever closes its standard output.
        let stderr_thread = scope.spawn(move || -> io::Result<()> {
            let mut reader = stderr_reader;
            let mut failure = None;
            let mut line = Vec::new();
            loop {
                line.clear();
                if reader.read_until(b'\n', &mut line)? == 0 {
                    break;
                }
                // A file that cannot be written must not stop the draining.
                if let Err(error) = stderr_file.write_all(&line) {
                    failure.get_or_insert(error);
                }
                (ctx.on_output)(OutputStream::Stderr, &line_text(&line));
            }
            match failure {
                Some(error) => Err(error),
                None => stderr_file.flush(),
            }
        });
        let watchdog = scope.spawn(|| {
            let mut reason: Option<&AtomicBool> = None;
            let mut killed = false;
            while !done.load(Ordering::SeqCst) {
                if reason.is_none() {
                    if limit.is_some_and(|limit| clock.elapsed_ms() >= limit.as_millis() as u64) {
                        reason = Some(&timed_out);
                    } else if ctx.cancel.is_cancelled() {
                        reason = Some(&cancelled);
                    }
                    if let Some(flag) = reason {
                        flag.store(true, Ordering::SeqCst);
                    }
                }
                if reason.is_some() && !killed {
                    killed = killer.kill().is_ok();
                }
                std::thread::park_timeout(Duration::from_millis(20));
            }
        });

        let tee = tee_each(&mut stdout_reader, &mut out_file, &mut timing, &clock, &mut |line| {
            stdout.extend_from_slice(line);
            let text = line_text(line);
            (ctx.on_output)(OutputStream::Stdout, &text);
            if on_line(&text) == LineAction::Kill {
                let _ = killer.kill();
            }
        });
        if tee.is_err() {
            // The recording cannot go on, so the tool must not either; its own end closes the pipes.
            let _ = killer.kill();
        }
        let waited = child.wait();
        done.store(true, Ordering::SeqCst);
        watchdog.thread().unpark();
        let stderr_outcome = stderr_thread.join().expect("the stderr thread does not panic");
        (tee, waited, stderr_outcome)
    });
    let lines = tee?;
    let exit_code = waited?;
    stderr_outcome?;

    let elapsed_ms = clock.elapsed_ms();
    std::fs::write(
        with_suffix(out, ".exit.json"),
        format!(
            "{{\"exit_code\":{},\"elapsed_ms\":{},\"lines\":{}}}\n",
            exit_code.unwrap_or(-1),
            elapsed_ms,
            lines
        ),
    )?;
    Ok(RunResult {
        exit_code,
        lines,
        elapsed_ms,
        started_unix,
        timed_out: timed_out.load(Ordering::SeqCst),
        cancelled: cancelled.load(Ordering::SeqCst),
        stdout,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_from_settings_are_120s_30min_30min_10s() {
        assert_eq!(
            Limits::from_settings(&Settings::default()),
            Limits {
                header: Duration::from_secs(120),
                scan: Duration::from_secs(30 * 60),
                exit_wait: Duration::from_secs(30 * 60),
                after_kill: Duration::from_secs(10),
            }
        );
        let short = Settings { scan_timeout_min: 2, exit_wait_min: 3, ..Settings::default() };
        assert_eq!(Limits::from_settings(&short).scan, Duration::from_secs(120));
        assert_eq!(Limits::from_settings(&short).exit_wait, Duration::from_secs(180));
    }

    #[test]
    fn a_cancel_token_is_shared_by_its_clones() {
        let token = CancelToken::default();
        let clone = token.clone();
        assert!(!clone.is_cancelled());
        token.cancel();
        assert!(clone.is_cancelled());
    }
}
