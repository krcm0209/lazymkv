//! The test helpers of the plan's Global Constraints, compiled under `cfg(test)` only: the continuous integration (CI)
//! factor for waits, the `skipped:` and `CUE:` lines written past the test harness's capture, the pipe tests' bounded
//! wait (`within`) and line read (`line`), and, on Windows, a registry value read through `reg query` (`reg_query`).
//! A fresh temporary folder is core's `testsupport::Scratch` and an in-memory secret store is `fakes::MemorySecrets`.

use std::io::{BufRead, Write};
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::time::Duration;

/// What a waiting bound is multiplied by: 4 when the `CI` environment variable is set, as GitHub Actions sets it, else
/// 1 (Ruling R29.c2). A copy of the engine's `testkit::ci_factor`, since this crate sits below the engine.
pub(crate) fn ci_factor() -> u32 {
    if std::env::var_os("CI").is_some() {
        4
    } else {
        1
    }
}

/// The line `skip` writes: "skipped: <reason>" and a newline.
fn skip_line(reason: &str) -> String {
    format!("skipped: {reason}\n")
}

/// A test whose input is absent: writes `skip_line(reason)` straight to standard error (not through `eprintln!`,
/// which the test harness captures), so `cargo test` shows it; the test then returns and passes.
pub(crate) fn skip(reason: &str) {
    let _ = std::io::stderr().write_all(skip_line(reason).as_bytes());
}

/// The line `cue` writes: "CUE: <text>" and a newline.
fn cue_line(text: &str) -> String {
    format!("CUE: {text}\n")
}

/// A test that waits for the owner: writes `cue_line(text)` straight to standard error and flushes it, so the test
/// harness's capture never holds the cue back.
pub(crate) fn cue(text: &str) {
    let mut stderr = std::io::stderr();
    let _ = stderr.write_all(cue_line(text).as_bytes());
    let _ = stderr.flush();
}

/// `work` on a thread of its own; the test fails when it has not ended within `bound`.
pub(crate) fn within<T: Send + 'static>(bound: Duration, work: impl FnOnce() -> T + Send + 'static) -> T {
    let (done, result) = channel();
    let worker = std::thread::spawn(move || {
        let _ = done.send(work());
    });
    match result.recv_timeout(bound) {
        Ok(value) => value,
        Err(RecvTimeoutError::Timeout) => panic!("the work had not ended within {bound:?}"),
        // The worker dropped its sender without sending: it panicked, and its panic is the test's.
        Err(RecvTimeoutError::Disconnected) => match worker.join() {
            Err(panic) => std::panic::resume_unwind(panic),
            Ok(()) => unreachable!("the worker ended without sending its result"),
        },
    }
}

/// One line from `reader` with its newline, "" at the end of the stream.
pub(crate) fn line(reader: &mut dyn BufRead) -> String {
    let mut line = String::new();
    reader.read_line(&mut line).expect("a line");
    line
}

/// The string value `value` of the registry key `key` as `reg query` prints it, unexpanded, read in a child process so
/// that it does not share the code under test; None when `reg query` fails or prints no `REG_SZ` or `REG_EXPAND_SZ`
/// line for it.
#[cfg(windows)]
pub(crate) fn reg_query(key: &str, value: &str) -> Option<String> {
    let output = std::process::Command::new("reg").args(["query", key, "/v", value]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    // A value line reads `    <value>    <type>    <data>`.
    String::from_utf8_lossy(&output.stdout).lines().find_map(|line| {
        let typed = line.trim_start().strip_prefix(value)?.trim_start();
        let data = typed.strip_prefix("REG_SZ").or_else(|| typed.strip_prefix("REG_EXPAND_SZ"))?;
        Some(data.trim().to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_skip_and_cue_lines_end_in_a_newline_after_their_prefix() {
        assert_eq!(skip_line("no drive in LAZYMKV_TEST_DRIVE"), "skipped: no drive in LAZYMKV_TEST_DRIVE\n");
        assert_eq!(cue_line("insert a disc in D:"), "CUE: insert a disc in D:\n");
    }
}
