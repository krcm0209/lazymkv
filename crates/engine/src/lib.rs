//! lazymkv-engine: the job-owner loop over core::job::reduce, the job store, the IPC (inter-process communication) server and the self-test (design parts 4.1, 4.4, 7.3).

mod arrival;
mod file_phase;
mod ipc_server;
mod notify;
mod owner;
mod relay;
mod restart;
mod selftest;
mod space;
mod steps;
mod store;
#[cfg(any(test, feature = "testkit"))]
pub mod testkit;
mod timers;

use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;

use lazymkv_dev::Args;
use lazymkv_tools::Limits;

pub use notify::{NoDelivery, NotifySink, NO_DELIVERY};
pub use owner::{start, EngineHandle, JobContext, Services, ShutdownHandle, StepResult, StepRunner};
pub use timers::{StdTimers, Timers};

/// The engine's start-up configuration (design parts 3.5, 3.6, 9.5; S44): the data folder, the resources folder the
/// tool lookup reads second, limits that replace `Limits::from_settings` (tests only), the `PATH` the tool lookup reads
/// last, the `lazymkvctl` the `command` relay runs, and the console writer. Def: T21. Use: T22 to T29.
pub struct EngineConfig {
    /// The data folder (`--data DIR`, design part 3.5).
    pub data: PathBuf,
    /// The resources folder the tool lookup reads second (design part 3.6).
    pub resources: Option<PathBuf>,
    /// Limits that replace `Limits::from_settings`, for tests only.
    pub limits_override: Option<Limits>,
    /// The `PATH` the tool lookup reads last (design part 3.6).
    pub path_var: Option<OsString>,
    /// The `lazymkvctl` the `command` relay runs as its child (design part 9.5); None refuses `command` with
    /// `COMMAND_NO_CTL`. Def: T3-10. Use: T3-22, T3-23.
    pub ctl: Option<PathBuf>,
    /// Where every engine.log line is also written: standard error in a console run (design part 3.5), the harness's
    /// buffer in tests; None writes engine.log only. Def: T3-10. Use: T3-10, T3-23.
    pub console: Option<Box<dyn Write + Send>>,
}

/// The binary's usage line.
const USAGE: &str = "usage: lazymkv-engine --data DIR";

/// The lazymkv-engine binary's run (design parts 3.1, 4.2, 4.6; S28). `--data DIR` is parsed with dev's `Args`. A
/// usage error prints "usage: lazymkv-engine --data DIR" and "lazymkv-engine: error: <message>" to `err` and returns
/// 2. Otherwise PR 2 prints its OS's sentence and returns 1: "lazymkv-engine runs only on Windows in this version"
/// outside Windows, and "lazymkv-engine has no Windows implementations in this build" on Windows. It creates nothing.
/// Def: T21. Use: T21.
pub fn main_with(args: &[String], err: &mut dyn Write) -> i32 {
    if let Err(message) = data_arg(args) {
        let _ = writeln!(err, "{USAGE}\nlazymkv-engine: error: {message}");
        return 2;
    }
    #[cfg(not(windows))]
    let sentence = "lazymkv-engine runs only on Windows in this version";
    #[cfg(windows)]
    let sentence = "lazymkv-engine has no Windows implementations in this build";
    let _ = writeln!(err, "{sentence}");
    1
}

/// The `--data` value, or the usage error's message in argparse's words: a missing value, a missing `--data` (checked
/// first, as argparse does), or words left over.
fn data_arg(args: &[String]) -> Result<String, String> {
    let mut parsed = Args::new(args, &["--data"]);
    let data = parsed.flag("--data")?;
    let rest = parsed.positionals()?;
    let data = data.ok_or_else(|| "the following arguments are required: --data".to_string())?;
    if !rest.is_empty() {
        return Err(format!("unrecognized arguments: {}", rest.join(" ")));
    }
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_data_value_is_a_usage_error_exit_2() {
        let words = |text: &[&str]| text.iter().map(|word| word.to_string()).collect::<Vec<String>>();
        let mut err = Vec::new();
        assert_eq!(main_with(&words(&["--data"]), &mut err), 2);
        assert_eq!(
            String::from_utf8(err).unwrap(),
            "usage: lazymkv-engine --data DIR\nlazymkv-engine: error: argument --data: expected one argument\n"
        );
        let mut err = Vec::new();
        assert_eq!(main_with(&words(&["--data", "x", "extra"]), &mut err), 2);
        assert_eq!(
            String::from_utf8(err).unwrap(),
            "usage: lazymkv-engine --data DIR\nlazymkv-engine: error: unrecognized arguments: extra\n"
        );
        let mut err = Vec::new();
        assert_eq!(main_with(&[], &mut err), 2);
        assert_eq!(
            String::from_utf8(err).unwrap(),
            "usage: lazymkv-engine --data DIR\nlazymkv-engine: error: the following arguments are required: --data\n"
        );
    }
}
