//! The OCR worker's driver `tools::ocr::run_worker` against the stand-in (design parts 5.3, 10.7, 10.8; T5-R8). The
//! scenario sits beside the copied stand-in, so no environment variable is passed (S15). The records are made-up
//! sentinel words, never subtitle text.

use lazymkv_core::ipc::OutputStream;
use lazymkv_core::settings::Settings;
use lazymkv_core::testsupport::{Scratch, ScratchExt};
use lazymkv_engine::testkit::{ci_factor, BusyRetry};
use lazymkv_platform::fakes::FakeHost;
use lazymkv_platform::{GuardedChild, ProcessGuard, Spawn, StdProcessGuard};
use lazymkv_tools::ocr::{end_reason, ocr_args, run_worker, WorkerEnd};
use lazymkv_tools::{run_recorded, with_suffix, CancelToken, LineAction, Limits, ToolCtx};
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

const NOW: u64 = 1_790_000_000;
const SENTINEL: &str = "text-sentinel-3e8f";
const USAGE: &str = "usage: lazymkv-ocr <evidence.mks> --stream N --clear-max BYTES --out DIR";

/// The stand-in copied as `lazymkv-ocr`, answering from `lazymkv-ocr.json` beside it.
fn stand_in(dir: &Scratch, scenario: &str) -> PathBuf {
    let tool = dir.0.join(format!("lazymkv-ocr{}", std::env::consts::EXE_SUFFIX));
    fs::copy(env!("CARGO_BIN_EXE_fake-tool"), &tool).unwrap();
    dir.write("lazymkv-ocr.json", scenario);
    tool
}

/// A process runner that keeps a copy of every spawn and starts it through `BusyRetry(StdProcessGuard)`.
#[derive(Default)]
struct Recording(Mutex<Vec<Spawn>>);

impl ProcessGuard for Recording {
    fn spawn(&self, spawn: &Spawn) -> io::Result<Box<dyn GuardedChild>> {
        self.0.lock().unwrap().push(spawn.clone());
        BusyRetry(StdProcessGuard).spawn(spawn)
    }
}

/// B1 (the worker's folder)'s layout: `<work>/evidence.mks`, the output `<work>/s0/`, the capture `<work>/s0.capture.txt`.
struct Work {
    dir: PathBuf,
    args: Vec<String>,
    out: PathBuf,
    capture: PathBuf,
}

fn work(scratch: &Scratch) -> Work {
    let dir = scratch.0.join("work").join("ocr-job-title");
    let out = dir.join("s0");
    fs::create_dir_all(&out).unwrap();
    let args = ocr_args(&dir.join("evidence.mks"), 0, 64, &out);
    Work { capture: dir.join("s0.capture.txt"), dir, args, out }
}

/// Runs the worker on its own thread and waits up to 30 s, so a kill that regressed fails the test instead of hanging
/// it; the bound proves a kill and is not scaled. Gives the end and every line `on_output` got.
fn run(guard: Arc<Recording>, tool: PathBuf, work: &Work, cap: Duration, cancel: CancelToken) -> (WorkerEnd, Vec<String>) {
    let (args, capture) = (work.args.clone(), work.capture.clone());
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let host = FakeHost::new("2026-11-06", NOW);
        let limits = Limits::from_settings(&Settings::default());
        let seen = Mutex::new(Vec::new());
        let on_output = |_: OutputStream, line: &str| seen.lock().unwrap().push(line.to_string());
        let ctx = ToolCtx { guard: &*guard, host: &host, limits: &limits, cancel, on_output: &on_output, on_progress: &|_: f64| {} };
        let end = run_worker(&ctx, &tool, &args, &capture, cap).map_err(|error| error.to_string());
        let _ = sender.send((end, seen.into_inner().unwrap()));
    });
    let (end, seen) = receiver.recv_timeout(Duration::from_secs(30)).expect("run_worker did not return within 30 s");
    (end.unwrap(), seen)
}

/// The cap of a run that must end before it: a wait that only waits, so it scales.
fn long() -> Duration {
    Duration::from_secs(20) * ci_factor()
}

#[test]
fn the_worker_gets_rten_num_threads_3_and_below_normal_priority() {
    let dir = Scratch::new("ocr-env");
    let tool = stand_in(&dir, r#"{"responses":[{"args_contain":["--stream"]}]}"#);
    let work = work(&dir);
    let guard = Arc::new(Recording::default());
    let (end, _) = run(Arc::clone(&guard), tool.clone(), &work, long(), CancelToken::default());
    assert_eq!(end, WorkerEnd::Read);
    let spawns = guard.0.lock().unwrap();
    assert_eq!(spawns.len(), 1);
    assert_eq!(spawns[0].program, tool);
    assert_eq!(spawns[0].args, work.args);
    assert_eq!(spawns[0].env, [("RTEN_NUM_THREADS".to_string(), "3".to_string())]);
    assert!(spawns[0].below_normal);
}

/// S43 and R3-worker-10: only the worker gets a variable; every driver of design part 5.3 runs through `run_recorded`.
#[test]
fn no_other_tool_gets_an_environment_variable() {
    let dir = Scratch::new("ocr-env-other");
    let tool = dir.0.join(format!("mkvmerge{}", std::env::consts::EXE_SUFFIX));
    fs::copy(env!("CARGO_BIN_EXE_fake-tool"), &tool).unwrap();
    dir.write("mkvmerge.json", r#"{"responses":[{"args_contain":["-J"],"stdout":"out.txt"}]}"#);
    dir.write("out.txt", "{}\n");
    let guard = Recording::default();
    let host = FakeHost::new("2026-11-06", NOW);
    let limits = Limits::from_settings(&Settings::default());
    let ctx = ToolCtx {
        guard: &guard,
        host: &host,
        limits: &limits,
        cancel: CancelToken::default(),
        on_output: &|_: OutputStream, _: &str| {},
        on_progress: &|_: f64| {},
    };
    let args = ["-J".to_string(), "title.mkv".to_string()];
    for below_normal in [false, true] {
        let out = dir.0.join(format!("ident-{below_normal}.json"));
        run_recorded(&ctx, &tool, &args, &out, None, below_normal, &mut |_| LineAction::Continue).unwrap();
    }
    let spawns = guard.0.lock().unwrap();
    let seen: Vec<(bool, bool)> = spawns.iter().map(|spawn| (spawn.env.is_empty(), spawn.below_normal)).collect();
    assert_eq!(seen, [(true, false), (true, true)]);
}

#[test]
fn a_worker_exit_3_is_refused_with_its_first_stderr_line() {
    let dir = Scratch::new("ocr-exit-3");
    let tool = stand_in(&dir, r#"{"responses":[{"args_contain":["--stream"],"stderr":"err.txt","exit_code":3}]}"#);
    dir.write("err.txt", "track 2 has a laced block at 12.345 s\r\nsecond line\n");
    let (end, _) = run(Arc::default(), tool, &work(&dir), long(), CancelToken::default());
    assert_eq!(end, WorkerEnd::Refused("track 2 has a laced block at 12.345 s".to_string()));
    assert_eq!(end_reason(&end).as_deref(), Some("track 2 has a laced block at 12.345 s"));
}

#[test]
fn a_worker_exit_3_without_a_reason_is_exited_with_code_3() {
    let dir = Scratch::new("ocr-exit-3-bare");
    let tool = stand_in(&dir, r#"{"responses":[{"args_contain":["--stream"],"exit_code":3}]}"#);
    let (end, _) = run(Arc::default(), tool, &work(&dir), long(), CancelToken::default());
    assert_eq!(end, WorkerEnd::Exited(Some(3)));
    assert_eq!(end_reason(&end).as_deref(), Some("lazymkv-ocr exited with code 3"));
}

#[test]
fn a_worker_exit_2_is_an_argument_error_with_its_first_stderr_line() {
    let dir = Scratch::new("ocr-exit-2");
    let tool = stand_in(&dir, r#"{"responses":[{"args_contain":["--stream"],"stderr":"err.txt","exit_code":2}]}"#);
    dir.write("err.txt", &format!("{USAGE}\n"));
    let (end, _) = run(Arc::default(), tool, &work(&dir), long(), CancelToken::default());
    assert_eq!(end, WorkerEnd::ArgumentError(USAGE.to_string()));
    assert_eq!(end_reason(&end), Some(format!("lazymkv-ocr refused its arguments: {USAGE}")));
}

/// Review Focus 2's driver half: a panic's standard error can hold text, so neither the reason nor a callback quotes it.
#[test]
fn a_worker_exit_101_is_exited_and_quotes_no_stderr() {
    let dir = Scratch::new("ocr-exit-101");
    let tool = stand_in(&dir, r#"{"responses":[{"args_contain":["--stream"],"stderr":"err.txt","exit_code":101}]}"#);
    dir.write("err.txt", &format!("thread 'main' panicked at {SENTINEL}\n"));
    let (end, seen) = run(Arc::default(), tool, &work(&dir), long(), CancelToken::default());
    assert_eq!(end, WorkerEnd::Exited(Some(101)));
    let reason = end_reason(&end).unwrap();
    assert_eq!(reason, "lazymkv-ocr exited with code 101");
    assert!(!reason.contains(SENTINEL));
    assert!(seen.is_empty(), "{seen:?}");
}

/// The cap is unscaled: it is the kill under test, not a wait (Global Constraints, Tests).
#[test]
fn a_worker_past_its_cap_is_timed_out() {
    let dir = Scratch::new("ocr-cap");
    let tool = stand_in(&dir, r#"{"responses":[{"args_contain":["--stream"],"stdout":"out.txt","hang_after_line":0}]}"#);
    dir.write("out.txt", "never\n");
    let work = work(&dir);
    let (end, _) = run(Arc::default(), tool, &work, Duration::from_millis(200), CancelToken::default());
    assert_eq!(end, WorkerEnd::TimedOut);
    assert_eq!(end_reason(&end), None);
    let exit: serde_json::Value = serde_json::from_slice(&fs::read(with_suffix(&work.capture, ".exit.json")).unwrap()).unwrap();
    assert!(exit["elapsed_ms"].as_u64().unwrap() >= 200, "{exit}");
}

#[test]
fn a_cancelled_worker_is_cancelled() {
    let dir = Scratch::new("ocr-cancel");
    let tool = stand_in(&dir, r#"{"responses":[{"args_contain":["--stream"],"stdout":"out.txt","hang_after_line":0}]}"#);
    dir.write("out.txt", "never\n");
    let token = CancelToken::default();
    token.cancel();
    let (end, _) = run(Arc::default(), tool, &work(&dir), long(), token);
    assert_eq!(end, WorkerEnd::Cancelled);
}

/// T5-R57: the capture's four files sit at the capture's path, outside `--out`, which holds only what the worker wrote.
#[test]
fn the_capture_is_written_beside_its_path_and_not_in_out() {
    let dir = Scratch::new("ocr-capture");
    let tool = stand_in(
        &dir,
        r#"{"responses":[{"args_contain":["--stream"],"stdout":"out.txt","stderr":"err.txt",
            "creates":[{"dir_arg_from_end":1,"name":"sub-0.ocr.jsonl","copy_from":"records-s0.txt"}]}]}"#,
    );
    dir.write("out.txt", "event 1 of 1\n");
    dir.write("err.txt", "loaded\n");
    dir.write("records-s0.txt", &format!("{{\"pts\":1.0,\"size\":900,\"text\":\"{SENTINEL}\"}}\n"));
    let work = work(&dir);
    let (end, _) = run(Arc::default(), tool, &work, long(), CancelToken::default());
    assert_eq!(end, WorkerEnd::Read);
    let names = |folder: &PathBuf| {
        let mut names: Vec<String> =
            fs::read_dir(folder).unwrap().map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    };
    assert_eq!(names(&work.out), ["sub-0.ocr.jsonl"]);
    assert_eq!(
        names(&work.dir),
        ["s0", "s0.capture.txt", "s0.capture.txt.exit.json", "s0.capture.txt.stderr", "s0.capture.txt.timing.jsonl"]
    );
    assert_eq!(fs::read_to_string(&work.capture).unwrap(), "event 1 of 1\n");
    assert_eq!(fs::read_to_string(with_suffix(&work.capture, ".stderr")).unwrap(), "loaded\n");
}

/// Design part 10.12: the worker's lines are kept in its capture only, never handed to `ToolCtx::on_output`, which
/// the engine forwards to the IPC (inter-process communication) snapshot.
#[test]
fn the_workers_output_reaches_no_output_callback() {
    let dir = Scratch::new("ocr-quiet");
    let tool = stand_in(&dir, r#"{"responses":[{"args_contain":["--stream"],"stdout":"out.txt","stderr":"err.txt"}]}"#);
    dir.write("out.txt", &format!("{SENTINEL}\n"));
    dir.write("err.txt", &format!("{SENTINEL}\n"));
    let work = work(&dir);
    let (end, seen) = run(Arc::default(), tool, &work, long(), CancelToken::default());
    assert_eq!(end, WorkerEnd::Read);
    assert!(seen.is_empty(), "{seen:?}");
    assert_eq!(fs::read_to_string(&work.capture).unwrap(), format!("{SENTINEL}\n"));
}
