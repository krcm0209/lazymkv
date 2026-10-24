//! tools::run_recorded against the stand-in (design parts 5.3, 7.2). The scenario sits beside the copied stand-in,
//! so no environment variable is passed (S15).

use lazymkv_core::ipc::OutputStream;
use lazymkv_core::settings::Settings;
use lazymkv_core::testsupport::{Scratch, ScratchExt};
use lazymkv_platform::fakes::FakeHost;
use lazymkv_platform::{GuardedChild, KillHandle, ProcessGuard, Spawn, StdProcessGuard};
use lazymkv_tools::{run_recorded, with_suffix, CancelToken, LineAction, Limits, RunResult, ToolCtx};
use std::fs;
use std::io::{self, Read};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

const NOW: u64 = 1_790_000_000;

fn stand_in(dir: &Scratch, scenario: &str) -> PathBuf {
    let tool = dir.0.join(format!("makemkvcon64{}", std::env::consts::EXE_SUFFIX));
    fs::copy(env!("CARGO_BIN_EXE_fake-tool"), &tool).unwrap();
    dir.write("makemkvcon64.json", scenario);
    tool
}

type Seen = Vec<(&'static str, String)>;

fn run(
    tool: &Path,
    out: &Path,
    limit: Option<Duration>,
    cancel: CancelToken,
    on_line: &mut dyn FnMut(&str) -> LineAction,
) -> io::Result<(RunResult, Seen)> {
    let guard = StdProcessGuard;
    let host = FakeHost::new("2026-10-23", NOW);
    let limits = Limits::from_settings(&Settings::default());
    let seen = Mutex::new(Vec::new());
    let on_output = |stream: OutputStream, line: &str| {
        let name = if matches!(stream, OutputStream::Stdout) { "stdout" } else { "stderr" };
        seen.lock().unwrap().push((name, line.to_string()));
    };
    let ctx = ToolCtx { guard: &guard, host: &host, limits: &limits, cancel, on_output: &on_output, on_progress: &|_: f64| {} };
    let result = run_recorded(&ctx, tool, &["info".to_string(), "disc:0".to_string()], out, limit, false, on_line)?;
    Ok((result, seen.into_inner().unwrap()))
}

#[test]
fn a_run_writes_stdout_stderr_timing_and_exit_files() {
    let dir = Scratch::new("rec-files");
    let tool = stand_in(&dir, r#"{"responses":[{"args_contain":["info"],"stdout":"out.txt","stderr":"err.txt","exit_code":3}]}"#);
    dir.write("out.txt", "MSG:1005,0,1,\"MakeMKV v2.0.0\"\r\nTCOUNT:0\n");
    dir.write("err.txt", "warn\n");
    let out = dir.0.join("job/scan.txt");
    let mut lines = Vec::new();

    let (result, seen) =
        run(&tool, &out, None, CancelToken::default(), &mut |line| { lines.push(line.to_string()); LineAction::Continue }).unwrap();

    assert_eq!(fs::read(&out).unwrap(), b"MSG:1005,0,1,\"MakeMKV v2.0.0\"\r\nTCOUNT:0\n");
    assert_eq!(result.stdout, fs::read(&out).unwrap());
    assert_eq!(fs::read(with_suffix(&out, ".stderr")).unwrap(), b"warn\n");
    let timing: Vec<serde_json::Value> = fs::read_to_string(with_suffix(&out, ".timing.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(timing.len(), 2);
    assert_eq!(timing[1]["n"], 1);
    assert!(timing[1]["t_ms"].is_u64());
    let exit: serde_json::Value = serde_json::from_slice(&fs::read(with_suffix(&out, ".exit.json")).unwrap()).unwrap();
    assert_eq!(exit.as_object().unwrap().keys().collect::<Vec<_>>(), ["exit_code", "elapsed_ms", "lines"]);
    assert_eq!((exit["exit_code"].clone(), exit["lines"].clone()), (serde_json::json!(3), serde_json::json!(2)));
    assert_eq!(
        (result.exit_code, result.lines, result.started_unix, result.timed_out, result.cancelled),
        (Some(3), 2, NOW, false, false)
    );
    assert_eq!(lines, ["MSG:1005,0,1,\"MakeMKV v2.0.0\"", "TCOUNT:0"]);
    assert!(seen.contains(&("stdout", "TCOUNT:0".to_string())));
    assert!(seen.contains(&("stderr", "warn".to_string())));
}

#[test]
fn invalid_utf8_reaches_the_file_unchanged_and_the_callback_replaced() {
    let dir = Scratch::new("rec-utf8");
    let tool = stand_in(&dir, r#"{"responses":[{"args_contain":["info"],"stdout":"out.txt"}]}"#);
    dir.write("out.txt", b"caf\xe9\n");
    let out = dir.0.join("scan.txt");
    let mut lines = Vec::new();

    let (result, seen) =
        run(&tool, &out, None, CancelToken::default(), &mut |line| { lines.push(line.to_string()); LineAction::Continue }).unwrap();

    assert_eq!(fs::read(&out).unwrap(), b"caf\xe9\n");
    assert_eq!(result.stdout, b"caf\xe9\n");
    assert_eq!(lines, ["caf\u{fffd}"]);
    assert_eq!(seen, [("stdout", "caf\u{fffd}".to_string())]);
}

#[test]
fn a_header_call_past_its_limit_is_killed_and_reported_timed_out() {
    let dir = Scratch::new("rec-limit");
    let tool = stand_in(&dir, r#"{"responses":[{"args_contain":["info"],"stdout":"out.txt","hang_after_line":0}]}"#);
    dir.write("out.txt", "never\n");
    let out = dir.0.join("mkvmerge.json");

    let (result, _) =
        run(&tool, &out, Some(Duration::from_millis(200)), CancelToken::default(), &mut |_| LineAction::Continue).unwrap();

    assert!(result.timed_out);
    assert!(!result.cancelled);
    assert_eq!(result.lines, 0);
    assert!(result.elapsed_ms >= 200 && result.elapsed_ms < 10_000, "{}", result.elapsed_ms);
    assert_ne!(result.exit_code, Some(0));
    assert!(with_suffix(&out, ".exit.json").exists());
}

#[test]
fn a_cancelled_run_is_killed() {
    let dir = Scratch::new("rec-cancel");
    let tool = stand_in(&dir, r#"{"responses":[{"args_contain":["info"],"stdout":"out.txt","hang_after_line":1}]}"#);
    dir.write("out.txt", "MSG:5015,0,0,\"Saving 1 titles\"\nTCOUNT:0\n");
    let token = CancelToken::default();
    let trigger = token.clone();

    let (result, _) =
        run(&tool, &dir.0.join("rip.txt"), None, token, &mut |_| { trigger.cancel(); LineAction::Continue }).unwrap();

    assert!(result.cancelled);
    assert!(!result.timed_out);
    assert_eq!(result.lines, 1);
    assert!(result.elapsed_ms < 10_000, "{}", result.elapsed_ms);
}

#[test]
fn a_line_action_kill_stops_the_run() {
    let dir = Scratch::new("rec-kill");
    let tool = stand_in(&dir, r#"{"responses":[{"args_contain":["info"],"stdout":"out.txt","hang_after_line":1}]}"#);
    dir.write("out.txt", "MSG:3307,0,0,\"x\",\"00800.mpls\",\"1\"\nMSG:5015,0,0,\"Saving 1 titles\"\n");
    let mut calls = 0;

    let (result, _) =
        run(&tool, &dir.0.join("rip.txt"), None, CancelToken::default(), &mut |_| { calls += 1; LineAction::Kill }).unwrap();

    assert_eq!(calls, 1);
    assert_eq!(result.lines, 1);
    assert!(!result.cancelled && !result.timed_out);
    assert_ne!(result.exit_code, Some(0));
    assert!(result.elapsed_ms < 10_000, "{}", result.elapsed_ms);
}

#[test]
fn a_program_that_cannot_start_is_an_error_and_writes_no_files() {
    let dir = Scratch::new("rec-missing");
    let out = dir.0.join("scan.txt");

    let outcome = run(&dir.0.join("no-such-tool"), &out, None, CancelToken::default(), &mut |_| LineAction::Continue);

    assert!(outcome.is_err());
    assert!(!out.exists());
    assert!(!with_suffix(&out, ".exit.json").exists());
}

/// A process runner that notes when the child it started has been waited for, so a test can tell that it is gone.
struct Reaping(Arc<AtomicBool>);

impl ProcessGuard for Reaping {
    fn spawn(&self, spawn: &Spawn) -> io::Result<Box<dyn GuardedChild>> {
        Ok(Box::new(ReapingChild { inner: StdProcessGuard.spawn(spawn)?, reaped: Arc::clone(&self.0) }))
    }
}

struct ReapingChild {
    inner: Box<dyn GuardedChild>,
    reaped: Arc<AtomicBool>,
}

impl GuardedChild for ReapingChild {
    fn take_stdout(&mut self) -> Box<dyn Read + Send> {
        self.inner.take_stdout()
    }

    fn take_stderr(&mut self) -> Box<dyn Read + Send> {
        self.inner.take_stderr()
    }

    fn killer(&self) -> KillHandle {
        self.inner.killer()
    }

    fn wait(&mut self) -> io::Result<Option<i32>> {
        let status = self.inner.wait();
        if status.is_ok() {
            self.reaped.store(true, Ordering::SeqCst);
        }
        status
    }
}

#[test]
fn a_panicking_line_callback_kills_the_child_and_run_recorded_panics() {
    let dir = Scratch::new("rec-panic");
    let tool = stand_in(&dir, r#"{"responses":[{"args_contain":["info"],"stdout":"out.txt","hang_after_line":1}]}"#);
    dir.write("out.txt", "MSG:5015,0,0,\"Saving 1 titles\"\nTCOUNT:0\n");
    let out = dir.0.join("rip.txt");
    let reaped = Arc::new(AtomicBool::new(false));
    let (sender, receiver) = mpsc::channel();
    let guard = Reaping(Arc::clone(&reaped));

    // The call runs on its own thread so that a run that never returns fails the test instead of hanging it.
    std::thread::spawn(move || {
        let host = FakeHost::new("2026-10-23", NOW);
        let limits = Limits::from_settings(&Settings::default());
        let ctx = ToolCtx {
            guard: &guard,
            host: &host,
            limits: &limits,
            cancel: CancelToken::default(),
            on_output: &|_: OutputStream, _: &str| {},
            on_progress: &|_: f64| {},
        };
        let args = ["info".to_string(), "disc:0".to_string()];
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            run_recorded(&ctx, &tool, &args, &out, None, false, &mut |_| panic!("the line callback failed"))
        }));
        let _ = sender.send(outcome.is_err());
    });

    let panicked = receiver.recv_timeout(Duration::from_secs(10)).expect("run_recorded did not return within 10 s");
    assert!(panicked, "run_recorded swallowed the panic");
    assert!(reaped.load(Ordering::SeqCst), "the child was not killed and waited for");
}
