//! The OCR worker's driver (design parts 5.3, 10.7, 10.8; K2): lazymkv-ocr started below normal with RTEN_NUM_THREADS=3, recorded capture-style and killed at its time cap; the engine and lazymkv-dev ocr-trial both start it here, so the trial measures what ships. Frozen from the trial's build on (T5-R54).

use std::io;
use std::path::Path;
use std::time::Duration;

use lazymkv_core::ipc::OutputStream;
use lazymkv_platform::Spawn;

use crate::record::{run_recorded_in, with_suffix, LineAction, RunResult, ToolCtx};

/// The worker's one environment variable: rten's thread count, the J4125's four cores less one, a fixed value of
/// spec 14 (design part 10.7, K2). Def: T5-9. Use: T5-9.
pub const RTEN_NUM_THREADS: (&str, &str) = ("RTEN_NUM_THREADS", "3");

/// The worker's command line `<evidence.mks> --stream N --clear-max BYTES --out DIR` (call 5): `evidence` and `out`
/// as `to_string_lossy` gives them, `stream` the subtitle-relative index of T5-R3, `clear_max` the job's
/// `sub_clear_max_bytes` (T5-R59). Def: T5-9. Use: T5-10, T5-15.
pub fn ocr_args(evidence: &Path, stream: i64, clear_max: i64, out: &Path) -> Vec<String> {
    vec![
        evidence.to_string_lossy().into_owned(),
        "--stream".to_string(),
        stream.to_string(),
        "--clear-max".to_string(),
        clear_max.to_string(),
        "--out".to_string(),
        out.to_string_lossy().into_owned(),
    ]
}

/// How one worker run ended (call 5, T5-R8). Def: T5-9. Use: T5-10, T5-15.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkerEnd {
    /// Exit 0: the stream was read.
    Read,
    /// Exit 3 with a first standard error line: the stream was refused; the line is the reason.
    Refused(String),
    /// Exit 2: the worker refused its arguments, an engine bug; the first standard error line, or empty.
    ArgumentError(String),
    /// Any other exit code (101 is a panic, 3 with no reason line included), or None when the run ended without one.
    Exited(Option<i32>),
    /// Killed once its cap passed (design part 10.8).
    TimedOut,
    /// Killed because the job's cancel flag was raised (design part 10.7, ENG-13).
    Cancelled,
}

/// T5-R8's reason for an end: `Refused(line)` gives the line; `ArgumentError(line)` gives
/// `lazymkv-ocr refused its arguments: <line>`, or `lazymkv-ocr refused its arguments` when the line is empty;
/// `Exited(Some(c))` gives `lazymkv-ocr exited with code <c>` and quotes no standard error (a panic's can hold
/// subtitle text, design part 10.12); `Exited(None)` gives `lazymkv-ocr ended without an exit code`; None for
/// `Read`, `TimedOut` (the caller writes `cap_reason`) and `Cancelled`. Def: T5-9. Use: T5-15.
pub fn end_reason(end: &WorkerEnd) -> Option<String> {
    match end {
        WorkerEnd::Read | WorkerEnd::TimedOut | WorkerEnd::Cancelled => None,
        WorkerEnd::Refused(line) => Some(line.clone()),
        WorkerEnd::ArgumentError(line) if line.is_empty() => Some("lazymkv-ocr refused its arguments".to_string()),
        WorkerEnd::ArgumentError(line) => Some(format!("lazymkv-ocr refused its arguments: {line}")),
        WorkerEnd::Exited(Some(code)) => Some(format!("lazymkv-ocr exited with code {code}")),
        WorkerEnd::Exited(None) => Some("lazymkv-ocr ended without an exit code".to_string()),
    }
}

/// Reads how a recorded run ended; a kill outranks the exit code (T5-R8).
fn worker_end(run: &RunResult, stderr_first: &str) -> WorkerEnd {
    if run.timed_out {
        return WorkerEnd::TimedOut;
    }
    if run.cancelled {
        return WorkerEnd::Cancelled;
    }
    match run.exit_code {
        Some(0) => WorkerEnd::Read,
        Some(3) if !stderr_first.is_empty() => WorkerEnd::Refused(stderr_first.to_string()),
        Some(2) => WorkerEnd::ArgumentError(stderr_first.to_string()),
        Some(code) => WorkerEnd::Exited(Some(code)),
        None => WorkerEnd::Exited(None),
    }
}

/// The first line of the file at `path`, without its `\n` and the `\r` before it; empty when the file is empty or
/// cannot be read.
fn first_line(path: &Path) -> String {
    let bytes = std::fs::read(path).unwrap_or_default();
    let line = bytes.split(|byte| *byte == b'\n').next().unwrap_or(&[]);
    let line = line.strip_suffix(b"\r").unwrap_or(line);
    String::from_utf8_lossy(line).into_owned()
}

/// Runs the OCR worker once (design parts 5.3, 10.7, 10.8; K2; T5-R8, T5-R10, T5-R57): `program` with `args` under
/// `ctx.guard`, at below-normal priority with `RTEN_NUM_THREADS=3` as its only extra variable, recorded capture-style
/// at `capture` with `<capture>.stderr`, `.timing.jsonl` and `.exit.json` beside it (the run's wall time is
/// `.exit.json`'s `elapsed_ms`), and killed once `cap` has passed since its start, the model load included. No line
/// of the worker reaches `ctx.on_output` (design part 10.12). `cap` is the caller's `time_cap_s(events)` seconds,
/// a parameter and never a setting (T5-R10). Err only when the worker cannot start or its run cannot be recorded,
/// as `run_recorded`. Def: T5-9. Use: T5-10, T5-15.
pub fn run_worker(ctx: &ToolCtx, program: &Path, args: &[String], capture: &Path, cap: Duration) -> io::Result<WorkerEnd> {
    let quiet = ToolCtx {
        guard: ctx.guard,
        host: ctx.host,
        limits: ctx.limits,
        cancel: ctx.cancel.clone(),
        on_output: &|_: OutputStream, _: &str| {},
        on_progress: ctx.on_progress,
    };
    let spawn = Spawn {
        program: program.to_path_buf(),
        args: args.to_vec(),
        env: vec![(RTEN_NUM_THREADS.0.to_string(), RTEN_NUM_THREADS.1.to_string())],
        below_normal: true,
        ..Spawn::default()
    };
    let run = run_recorded_in(&quiet, &spawn, capture, Some(cap), &mut |_| LineAction::Continue)?;
    Ok(worker_end(&run, &first_line(&with_suffix(capture, ".stderr"))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn ocr_args_are_evidence_stream_clear_max_and_out() {
        let work = Path::new("work").join("ocr-job-title");
        let (evidence, out) = (work.join("evidence.mks"), work.join("s1"));
        let want = [
            evidence.to_string_lossy().into_owned(),
            "--stream".to_string(),
            "1".to_string(),
            "--clear-max".to_string(),
            "64".to_string(),
            "--out".to_string(),
            out.to_string_lossy().into_owned(),
        ];
        assert_eq!(ocr_args(&evidence, 1, 64, &out), want);
    }

    #[test]
    fn the_worker_thread_count_is_spec_14s_fixed_3() {
        assert_eq!(RTEN_NUM_THREADS, ("RTEN_NUM_THREADS", "3"));
    }

    #[test]
    fn end_reason_gives_t5_r8s_sentences() {
        let refused = WorkerEnd::Refused("evidence.mks has no subtitle stream 4".to_string());
        assert_eq!(end_reason(&refused).as_deref(), Some("evidence.mks has no subtitle stream 4"));
        let usage = "usage: lazymkv-ocr <evidence.mks> --stream N --clear-max BYTES --out DIR";
        assert_eq!(
            end_reason(&WorkerEnd::ArgumentError(usage.to_string())),
            Some(format!("lazymkv-ocr refused its arguments: {usage}"))
        );
        assert_eq!(
            end_reason(&WorkerEnd::ArgumentError(String::new())).as_deref(),
            Some("lazymkv-ocr refused its arguments")
        );
        assert_eq!(end_reason(&WorkerEnd::Exited(Some(101))).as_deref(), Some("lazymkv-ocr exited with code 101"));
        assert_eq!(
            end_reason(&WorkerEnd::Exited(Some(-1073741819))).as_deref(),
            Some("lazymkv-ocr exited with code -1073741819")
        );
        for none in [WorkerEnd::Read, WorkerEnd::TimedOut, WorkerEnd::Cancelled] {
            assert_eq!(end_reason(&none), None, "{none:?}");
        }
    }

    #[test]
    fn a_worker_ended_without_an_exit_code_is_named_so() {
        assert_eq!(end_reason(&WorkerEnd::Exited(None)).as_deref(), Some("lazymkv-ocr ended without an exit code"));
    }

    fn result(exit_code: Option<i32>, timed_out: bool, cancelled: bool) -> RunResult {
        RunResult { exit_code, lines: 0, elapsed_ms: 5, started_unix: 1_790_000_000, timed_out, cancelled, stdout: Vec::new() }
    }

    /// A kill outranks the exit code: a worker that exits 0 as the cap's kill lands is past its cap all the same.
    #[test]
    fn a_runs_end_maps_to_the_workers_end() {
        let first = "content encoding 3 of track 2 is not zlib compression or header stripping";
        assert_eq!(worker_end(&result(Some(0), false, false), first), WorkerEnd::Read);
        assert_eq!(worker_end(&result(Some(3), false, false), first), WorkerEnd::Refused(first.to_string()));
        assert_eq!(worker_end(&result(Some(3), false, false), ""), WorkerEnd::Exited(Some(3)));
        assert_eq!(worker_end(&result(Some(2), false, false), "usage: x"), WorkerEnd::ArgumentError("usage: x".to_string()));
        assert_eq!(worker_end(&result(Some(2), false, false), ""), WorkerEnd::ArgumentError(String::new()));
        assert_eq!(worker_end(&result(Some(101), false, false), first), WorkerEnd::Exited(Some(101)));
        assert_eq!(worker_end(&result(None, false, false), ""), WorkerEnd::Exited(None));
        assert_eq!(worker_end(&result(None, true, false), first), WorkerEnd::TimedOut);
        assert_eq!(worker_end(&result(Some(0), true, false), ""), WorkerEnd::TimedOut);
        assert_eq!(worker_end(&result(None, false, true), ""), WorkerEnd::Cancelled);
    }
}
