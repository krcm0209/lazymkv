//! The makemkvcon drivers (design parts 5.3, 5.4, 5.6): the drive listing, the scan and the rip. Each one checks that
//! no other MakeMKV process runs, records its run with `run_recorded`, reads each line with `LineReader`, kills the
//! run past its limits, and pauses `limits.after_kill` after any kill before it returns (S55 to S59).

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::thread::Thread;
use std::time::{Duration, Instant, UNIX_EPOCH};

use lazymkv_core::disc::TitleSource;
use lazymkv_core::ipc::CANCELLED;
use lazymkv_core::job::{classify_title_failure, Failure, FailureClass};
use lazymkv_core::robot::{self, lines::{Call, LineEvent, LineReader, RunSummary}};
use lazymkv_platform::Host;

use crate::record::{run_recorded, with_suffix, CancelToken, LineAction, RunResult, ToolCtx};

/// The flags a job fixes for every makemkvcon call (design parts 5.3, 5.4; spec 9.2). Def: T13. Use: T23.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MakemkvFlags {
    /// `--minlength`: the shortest title, in seconds, MakeMKV lists.
    pub minlength_s: u32,
    /// `--cache`: the read cache, in megabytes.
    pub cache_mb: u32,
    /// `--profile`: the job's profile file.
    pub profile: PathBuf,
    /// Whether `--debug` is passed (S45).
    pub debug: bool,
}

/// `["-r","--cache=1","info","disc:9999"]`, with `"--debug"` after `"-r"` when `debug` (design part 5.4 step 1, S45).
/// Def: T13. Use: T13, T23.
pub fn drive_list_args(debug: bool) -> Vec<String> {
    let mut args = vec!["-r".to_string()];
    if debug {
        args.push("--debug".to_string());
    }
    args.extend(["--cache=1", "info", "disc:9999"].map(String::from));
    args
}

/// The flags every scan and rip starts with: `-r`, the messages and progress to standard output, `--debug` when on,
/// then the length, cache and profile flags (design part 5.4 step 4).
fn job_flag_args(flags: &MakemkvFlags) -> Vec<String> {
    let mut args = ["-r", "--messages=-stdout", "--progress=-same"].map(String::from).to_vec();
    if flags.debug {
        args.push("--debug".to_string());
    }
    args.push(format!("--minlength={}", flags.minlength_s));
    args.push(format!("--cache={}", flags.cache_mb));
    args.push(format!("--profile={}", flags.profile.display()));
    args
}

/// `["-r","--messages=-stdout","--progress=-same",("--debug",)"--minlength=<m>","--cache=<c>","--profile=<p>","info",
/// "disc:<drive>"]` (design part 5.4 step 4). Def: T13. Use: T13, T23.
pub fn scan_args(flags: &MakemkvFlags, drive: u32) -> Vec<String> {
    let mut args = job_flag_args(flags);
    args.push("info".to_string());
    args.push(format!("disc:{drive}"));
    args
}

/// The scan's flags, then `"mkv","disc:<drive>","<title>","<dir>"` (design part 5.6 step 6). Def: T13. Use: T13, T23.
pub fn rip_args(flags: &MakemkvFlags, drive: u32, title: u32, dir: &Path) -> Vec<String> {
    let mut args = job_flag_args(flags);
    args.push("mkv".to_string());
    args.push(format!("disc:{drive}"));
    args.push(title.to_string());
    args.push(dir.display().to_string());
    args
}

/// Field 0, read as a number, of the first DRV line whose field 6 equals `drive` exactly (design part 5.4 step 1). A
/// line with fewer than 7 fields, or whose field 0 is not a number, is skipped. Def: T13. Use: T13, T23.
pub fn drive_index(drives: &[robot::Drive], drive: &str) -> Option<u32> {
    drives.iter().find_map(|fields| match fields.get(6) {
        Some(id) if id == drive => fields[0].parse().ok(),
        _ => None,
    })
}

/// One drive listing: the index for the drive id, the reducer's summary and the run (design parts 3.7, 5.4).
/// Def: T13. Use: T23, T26.
#[derive(Clone, Debug)]
pub struct DriveListing {
    /// The `disc:` index of the drive id, or None when no DRV line names it.
    pub index: Option<u32>,
    /// What the listing said.
    pub summary: RunSummary,
    /// How the run ended.
    pub run: RunResult,
}

/// Runs and records the drive listing into `out` (design part 5.4 step 1). Another MakeMKV process running gives `Err`
/// of kind `ResourceBusy` naming it (S55). The run has no time limit; it is killed only by the exit wait after its
/// `MSG:5010` (`run.timed_out`) or by the cancel (`run.cancelled`) (S57). Def: T13. Use: T23, T26.
pub fn list_drives(ctx: &ToolCtx, program: &Path, debug: bool, drive: &str, out: &Path) -> io::Result<DriveListing> {
    let (run, summary, _) = run_makemkv(ctx, program, &drive_list_args(debug), out, None, Call::DriveList)?;
    Ok(DriveListing { index: drive_index(&summary.drives, drive), summary, run })
}

/// How a scan ended (design part 5.4 step 4, L5). Def: T13. Use: T23.
#[derive(Clone, Debug, PartialEq)]
pub enum ScanOutcome {
    /// The scan finished: its standard output read with replacement, and what it said.
    Done { text: String, summary: RunSummary },
    /// MakeMKV rejected the profile (`MSG:1009`), with the message text.
    ProfileRejected { text: String },
    /// The scan failed, with its class and a message for the job's log.
    Failed { failure: Failure, message: String },
}

/// Runs and records the scan into `out`, killing it past `limits.scan` while it has not printed its final message, and
/// by the exit wait after it (S57); the engine lists the drives and retries once after the limit's kill (RP-5; design
/// parts 5.3, 5.4 step 4). Every call writes `out` from the start (S58). A licence message gives
/// `Failed { Licence }`, `MSG:1009` gives `ProfileRejected`, and a scan without `MSG:5011` and a `TCOUNT` line fails
/// whatever its exit code. Def: T13. Use: T23.
pub fn scan(ctx: &ToolCtx, program: &Path, flags: &MakemkvFlags, drive: u32, out: &Path) -> io::Result<ScanOutcome> {
    let failed = |failure, message: &str| ScanOutcome::Failed { failure, message: message.to_string() };
    let ran = run_makemkv(ctx, program, &scan_args(flags, drive), out, Some(ctx.limits.scan), Call::Scan);
    let (run, summary, kill) = match ran {
        Ok(ran) => ran,
        Err(error) => return busy_message(error).map(|message| failed(UNKNOWN, &message)),
    };
    Ok(match kill {
        Kill::Limit => failed(UNKNOWN, SCAN_PAST_LIMIT),
        Kill::ExitWait => failed(UNKNOWN, EXIT_WAIT_PASSED),
        Kill::Cancel => failed(UNKNOWN, CANCELLED),
        Kill::None | Kill::LineKill => {
            if let Some(licence) = &summary.licence {
                failed(Failure { class: FailureClass::Licence, output: None }, &licence.text)
            } else if let Some(text) = &summary.profile_rejected {
                ScanOutcome::ProfileRejected { text: text.clone() }
            } else if !summary.scan_complete {
                let code = exit_text(&run, kill);
                failed(UNKNOWN, &format!("the scan ended without MSG:5011 and a TCOUNT line (exit code {code})"))
            } else {
                ScanOutcome::Done { text: String::from_utf8_lossy(&run.stdout).into_owned(), summary }
            }
        }
    })
}

/// `classify_title_failure`'s first two arguments, read when a rip has failed (design part 5.6 step 6). Def: T13.
/// Use: T23.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StagingHealth {
    /// Whether the staging share can be reached.
    pub share_healthy: bool,
    /// Whether the staging share is out of space.
    pub out_of_space: bool,
}

/// How a rip ended (design part 5.6 step 6). Def: T13. Use: T23.
#[derive(Clone, Debug, PartialEq)]
pub enum RipOutcome {
    /// MakeMKV saved the title, and the title folder holds this one `.mkv` file.
    Ripped { file: PathBuf },
    /// The index guard found the planned title under another index, and the rip was killed (spec 6.2 step 6).
    IndexMismatch,
    /// A licence or key message, with its text.
    Licence { message: String },
    /// The rip failed, with its class and a message for the job's log.
    Failed { failure: Failure, message: String },
}

/// A rip's outcome, run and summary (design part 5.6 step 6). When another MakeMKV process stopped the rip before it
/// started (S55), `run` has no exit code, no lines and no output, and `summary` is empty. Def: T13. Use: T23.
#[derive(Clone, Debug)]
pub struct RipRun {
    /// How the rip ended.
    pub outcome: RipOutcome,
    /// How the run ended.
    pub run: RunResult,
    /// What the rip said.
    pub summary: RunSummary,
}

/// Runs and records one title's rip into `out`, with the index guard (design part 5.6 step 6). `title` is the title's
/// index in the scan and `source` how the guard recognises it; `dir` is the title's empty rip folder. A mismatch kills
/// the rip; a licence message does not (S59). `staging` is read only when the rip has failed. A title folder that
/// cannot be listed fails the title, classed like any failed title (review round 1, V6); `Err` comes only from a run
/// that could not be started or recorded, never from S55's refusal, which is `Failed`. Def: T13. Use: T23.
#[allow(clippy::too_many_arguments)]
pub fn rip(
    ctx: &ToolCtx,
    program: &Path,
    flags: &MakemkvFlags,
    drive: u32,
    title: u32,
    source: TitleSource,
    dir: &Path,
    out: &Path,
    staging: &dyn Fn() -> StagingHealth,
) -> io::Result<RipRun> {
    let call = Call::Rip { index: title, source };
    let (run, summary, kill) = match run_makemkv(ctx, program, &rip_args(flags, drive, title, dir), out, None, call) {
        Ok(ran) => ran,
        Err(error) => {
            let message = busy_message(error)?;
            let run = RunResult {
                exit_code: None,
                lines: 0,
                elapsed_ms: 0,
                started_unix: ctx.host.now_unix(),
                timed_out: false,
                cancelled: ctx.cancel.is_cancelled(),
                stdout: Vec::new(),
            };
            let outcome = RipOutcome::Failed { failure: UNKNOWN, message };
            return Ok(RipRun { outcome, run, summary: RunSummary::default() });
        }
    };
    let outcome = rip_outcome(&run, &summary, kill, dir, staging);
    Ok(RipRun { outcome, run, summary })
}

/// The verdict of a rip that ran, in design part 5.6 step 6's order: the kills, the index guard, a licence message, a
/// rejected profile, then the summary and the title folder. A title folder that cannot be listed is a failed title,
/// classed with `staging` like any other (spec 10.2: the share is checked first), and the listing error, naming the
/// folder, is appended to the message (review round 1, V6).
fn rip_outcome(
    run: &RunResult,
    summary: &RunSummary,
    kill: Kill,
    dir: &Path,
    staging: &dyn Fn() -> StagingHealth,
) -> RipOutcome {
    let failed = |message: String| RipOutcome::Failed { failure: UNKNOWN, message };
    match kill {
        Kill::Cancel => return failed(CANCELLED.to_string()),
        Kill::ExitWait => return failed(EXIT_WAIT_PASSED.to_string()),
        Kill::None | Kill::Limit | Kill::LineKill => {}
    }
    if summary.index_guard == Some(false) {
        return RipOutcome::IndexMismatch;
    }
    if let Some(licence) = &summary.licence {
        return RipOutcome::Licence { message: licence.text.clone() };
    }
    if let Some(text) = &summary.profile_rejected {
        return failed(format!("MakeMKV rejected the profile on the rip: {text}"));
    }
    let (mut files, unlisted) = match mkv_files(dir) {
        Ok(files) => (files, None),
        Err(error) => (Vec::new(), Some(error)),
    };
    if summary.title_saved && !summary.title_failed && files.len() == 1 {
        return RipOutcome::Ripped { file: files.remove(0) };
    }
    let code = exit_text(run, kill);
    let mut message = if summary.title_failed {
        format!("MakeMKV reported the title failed (exit code {code})")
    } else if !summary.title_saved {
        format!("the rip ended without a success summary (exit code {code})")
    } else if unlisted.is_some() {
        format!("the title folder could not be listed (exit code {code})")
    } else {
        format!("the title folder holds {} .mkv files, not one", files.len())
    };
    if let Some(error) = unlisted {
        message.push_str(&format!("; {}: {error}", dir.display()));
    }
    let health = staging();
    let failure =
        classify_title_failure(health.share_healthy, health.out_of_space, summary.write_error, summary.read_errors);
    RipOutcome::Failed { failure, message }
}

/// The regular files directly in `dir` whose extension is `mkv` in any ASCII case, sorted. A missing `dir` holds none.
fn mkv_files(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("mkv")) && entry.file_type()?.is_file() {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

/// What replaces MakeMKV's registration key in a saved debug log.
const KEY_REMOVED: &[u8] = b"[app_Key removed]";

/// Saves MakeMKV's debug log as `<out>.makemkv-debug.txt` with every occurrence of the registration key replaced by
/// `[app_Key removed]` (design part 7.2, PIPE-9). The log is copied as bytes, because it need not be UTF-8 (S61), and
/// it is copied unchanged when the host has no key. `started_unix` is the call's start: a log last modified before it
/// belongs to an earlier call and is not saved. The `Err` text says why nothing was saved and never holds the key.
/// Def: T14. Use: T23.
pub fn save_debug_log(host: &dyn Host, started_unix: u64, out: &Path) -> Result<(), String> {
    let log = host.makemkv_debug_log().ok_or_else(|| "MakeMKV's debug log path is not known".to_string())?;
    let failed = |path: &Path, error: io::Error| format!("{}: {error}", path.display());
    let metadata = match fs::metadata(&log) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(format!("{}: no debug log was written", log.display()));
        }
        Err(error) => return Err(failed(&log, error)),
    };
    let modified = metadata.modified().map_err(|error| failed(&log, error))?;
    if modified.duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs()) < started_unix {
        return Err(format!("{}: the debug log is older than the call", log.display()));
    }
    let mut bytes = fs::read(&log).map_err(|error| failed(&log, error))?;
    if let Some(key) = host.makemkv_app_key().filter(|key| !key.is_empty()) {
        bytes = replace_bytes(&bytes, key.as_bytes(), KEY_REMOVED);
    }
    let target = with_suffix(out, ".makemkv-debug.txt");
    fs::write(&target, bytes).map_err(|error| failed(&target, error))
}

/// `data` with each non-overlapping occurrence of the non-empty `key`, found from the left, replaced by `with`.
fn replace_bytes(data: &[u8], key: &[u8], with: &[u8]) -> Vec<u8> {
    let mut replaced = Vec::with_capacity(data.len());
    let mut at = 0;
    while at < data.len() {
        if data[at..].starts_with(key) {
            replaced.extend_from_slice(with);
            at += key.len();
        } else {
            replaced.push(data[at]);
            at += 1;
        }
    }
    replaced
}

/// The class every failure of the drivers has unless the summary says otherwise.
const UNKNOWN: Failure = Failure { class: FailureClass::Unknown, output: None };

/// The message of a scan killed past `limits.scan`, which the engine retries once after a fresh drive listing (RP-5,
/// design part 5.3). Def: T13. Use: T13, T23.
pub const SCAN_PAST_LIMIT: &str = "the scan did not finish within scan_timeout_min and was killed";

/// The message of a run killed by the exit wait (S57), also the engine's for a drive listing it killed (design part
/// 5.3). Def: T13. Use: T13, T23.
pub const EXIT_WAIT_PASSED: &str = "makemkvcon did not exit within exit_wait_min of its final message and was killed";

/// The process names, lower-cased and without `.exe`, that stop a call when running: the two command-line programs
/// and the graphical one (S55).
const MAKEMKV_PROCESSES: [&str; 3] = ["makemkvcon64", "makemkvcon", "makemkv"];

/// The prefixes of a rip's final messages (S57).
const RIP_ENDINGS: [&str; 4] = ["MSG:5004,", "MSG:5005,", "MSG:5036,", "MSG:5037,"];

/// How often the watcher of `run_makemkv` copies the cancel and checks the exit wait.
const WATCH_EVERY: Duration = Duration::from_millis(20);

/// Which kill ended a run (S56, S57).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kill {
    /// No kill: the tool ended by itself.
    None,
    /// The run passed its limit before its final message.
    Limit,
    /// The tool did not exit within `limits.exit_wait` of its final message.
    ExitWait,
    /// The line callback asked for the kill (the index guard).
    LineKill,
    /// The owner's cancel.
    Cancel,
}

/// S55's refusal, carried inside the `io::Error` so that `scan` and `rip` can tell it from any other error.
#[derive(Debug)]
struct Busy(String);

impl fmt::Display for Busy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "another MakeMKV process is running: {}", self.0)
    }
}

impl std::error::Error for Busy {}

/// S55's message when `error` is the busy refusal, else `error` itself.
fn busy_message(error: io::Error) -> io::Result<String> {
    match error.get_ref().and_then(|inner| inner.downcast_ref::<Busy>()) {
        Some(busy) => Ok(busy.to_string()),
        None => Err(error),
    }
}

/// The first running process that is MakeMKV, named as the host reported it (S55).
fn makemkv_running(host: &dyn Host) -> Option<String> {
    host.running_processes().into_iter().find(|name| {
        let lower = name.to_lowercase();
        let stem = lower.strip_suffix(".exe").unwrap_or(&lower);
        MAKEMKV_PROCESSES.contains(&stem)
    })
}

/// The exit code for a message, or `none` when the process was killed or ended without one.
fn exit_text(run: &RunResult, kill: Kill) -> String {
    match (kill, run.exit_code) {
        (Kill::None, Some(code)) => code.to_string(),
        _ => "none".to_string(),
    }
}

/// Recognises a call's final message by its line prefix, only to start the exit wait (S57).
enum Ending {
    /// The first `MSG:5010`.
    DriveList,
    /// The first `TCOUNT` after a `MSG:5011`.
    Scan { seen_5011: bool },
    /// The first of `RIP_ENDINGS`.
    Rip,
}

impl Ending {
    fn of(call: &Call) -> Ending {
        match call {
            Call::DriveList => Ending::DriveList,
            Call::Scan => Ending::Scan { seen_5011: false },
            Call::Rip { .. } => Ending::Rip,
        }
    }

    /// Whether `line` is a final message of the call.
    fn is_final(&mut self, line: &str) -> bool {
        match self {
            Ending::DriveList => line.starts_with("MSG:5010,"),
            Ending::Scan { seen_5011 } => {
                if line.starts_with("MSG:5011,") {
                    *seen_5011 = true;
                }
                *seen_5011 && line.starts_with("TCOUNT:")
            }
            Ending::Rip => RIP_ENDINGS.iter().any(|prefix| line.starts_with(prefix)),
        }
    }
}

/// Tells the watcher of `run_makemkv` to finish, however the run ends, a panic included.
struct StopWatcher<'a> {
    done: &'a AtomicBool,
    watcher: Thread,
}

impl Drop for StopWatcher<'_> {
    fn drop(&mut self) {
        self.done.store(true, Ordering::SeqCst);
        self.watcher.unpark();
    }
}

/// The run all three drivers share (design part 5.3): S55's check, then `run_recorded` with no limit of its own, a
/// `LineReader` fed each line, `Progress` forwarded to `ctx.on_progress`, an `IndexMismatch` answered with a kill, and
/// a watcher that copies `ctx.cancel`, kills once `limit` has passed since the call started while no final message has
/// come, and kills once `limits.exit_wait` has passed since the final message (S57). A final message ends the limit,
/// so a call that printed it and then hangs is the exit wait's, never reported as past its limit (ruling at the final
/// review). The result's `cancelled` reflects only `ctx.cancel`, and its `timed_out` is true after the limit's or the
/// exit wait's kill. After any kill it sleeps `limits.after_kill` (S56). It sleeps too when the recorder gives `Err`,
/// because the recorder kills the tool on its error paths without saying so.
fn run_makemkv(
    ctx: &ToolCtx,
    program: &Path,
    args: &[String],
    out: &Path,
    limit: Option<Duration>,
    call: Call,
) -> io::Result<(RunResult, RunSummary, Kill)> {
    if let Some(name) = makemkv_running(ctx.host) {
        return Err(io::Error::new(io::ErrorKind::ResourceBusy, Busy(name)));
    }
    let mut ending = Ending::of(&call);
    let mut reader = LineReader::new(call);
    let inner = CancelToken::default();
    let final_at: OnceLock<Instant> = OnceLock::new();
    // The watcher's reason for raising `inner`, the first one only.
    let reason: OnceLock<Kill> = OnceLock::new();
    let done = AtomicBool::new(false);
    let mut line_kill = false;
    // Taken before the spawn, as `run_recorded`'s own limit clock is (Ruling R29.c2b).
    let started = Instant::now();
    let ran = std::thread::scope(|scope| {
        let (cancel, exit_wait) = (ctx.cancel.clone(), ctx.limits.exit_wait);
        let (inner_ref, final_ref, reason_ref, done_ref) = (&inner, &final_at, &reason, &done);
        let watcher = scope.spawn(move || {
            while !done_ref.load(Ordering::SeqCst) {
                if cancel.is_cancelled() {
                    let _ = reason_ref.set(Kill::Cancel);
                    inner_ref.cancel();
                } else if final_ref.get().is_some_and(|at| at.elapsed() >= exit_wait) {
                    let _ = reason_ref.set(Kill::ExitWait);
                    inner_ref.cancel();
                } else if limit.is_some_and(|limit| final_ref.get().is_none() && started.elapsed() >= limit) {
                    let _ = reason_ref.set(Kill::Limit);
                    inner_ref.cancel();
                }
                std::thread::park_timeout(WATCH_EVERY);
            }
        });
        let _stop = StopWatcher { done: &done, watcher: watcher.thread().clone() };
        let inner_ctx = ToolCtx {
            guard: ctx.guard,
            host: ctx.host,
            limits: ctx.limits,
            cancel: inner.clone(),
            on_output: ctx.on_output,
            on_progress: ctx.on_progress,
        };
        run_recorded(&inner_ctx, program, args, out, None, false, &mut |line| {
            let line = line.strip_suffix('\r').unwrap_or(line);
            if ending.is_final(line) {
                let _ = final_at.set(Instant::now());
            }
            let mut action = LineAction::Continue;
            for event in reader.feed(line) {
                match event {
                    LineEvent::Progress { fraction } => (ctx.on_progress)(fraction),
                    LineEvent::IndexMismatch => {
                        line_kill = true;
                        action = LineAction::Kill;
                    }
                    // A licence message does not kill the run: MakeMKV ends it itself (S59).
                    LineEvent::Licence { .. } | LineEvent::ProfileRejected { .. } => {}
                }
            }
            action
        })
    });
    let mut run = match ran {
        Ok(run) => run,
        Err(error) => {
            std::thread::sleep(ctx.limits.after_kill);
            return Err(error);
        }
    };
    // `run_recorded` records a raised `inner`, whose reason the watcher set, and not a line kill.
    let kill = if run.cancelled {
        reason.get().copied().unwrap_or(Kill::Cancel)
    } else if line_kill {
        Kill::LineKill
    } else {
        Kill::None
    };
    run.cancelled = ctx.cancel.is_cancelled();
    run.timed_out = matches!(kill, Kill::Limit | Kill::ExitWait);
    if kill != Kill::None {
        std::thread::sleep(ctx.limits.after_kill);
    }
    Ok((run, reader.finish(), kill))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn flags(debug: bool) -> MakemkvFlags {
        MakemkvFlags { minlength_s: 120, cache_mb: 1024, profile: PathBuf::from("p"), debug }
    }

    fn drv(fields: &[&str]) -> Vec<String> {
        fields.iter().map(|field| field.to_string()).collect()
    }

    #[test]
    fn scan_args_match_the_design() {
        assert_eq!(
            scan_args(&flags(false), 0),
            ["-r", "--messages=-stdout", "--progress=-same", "--minlength=120", "--cache=1024", "--profile=p", "info", "disc:0"]
        );
        assert_eq!(
            scan_args(&flags(true), 2),
            ["-r", "--messages=-stdout", "--progress=-same", "--debug", "--minlength=120", "--cache=1024", "--profile=p", "info", "disc:2"]
        );
    }

    #[test]
    fn rip_args_end_with_the_title_index_and_folder() {
        assert_eq!(
            rip_args(&flags(false), 1, 3, Path::new("dir")),
            ["-r", "--messages=-stdout", "--progress=-same", "--minlength=120", "--cache=1024", "--profile=p", "mkv", "disc:1", "3", "dir"]
        );
    }

    #[test]
    fn the_drive_listing_gets_debug_after_r_when_debug_is_on() {
        assert_eq!(drive_list_args(false), ["-r", "--cache=1", "info", "disc:9999"]);
        assert_eq!(drive_list_args(true), ["-r", "--debug", "--cache=1", "info", "disc:9999"]);
    }

    #[test]
    fn the_drive_index_is_the_drv_line_whose_seventh_field_is_the_drive_id() {
        let drives = vec![
            drv(&["0", "2", "999", "12", "BD-RE DRIVE A 1.00 SERIAL", "DISC", "D:"]),
            drv(&["1", "2", "999", "12", "BD-RE DRIVE B 1.00 SERIAL", "OTHER", "E:"]),
            drv(&["2", "256", "999", "0", "", "", ""]),
            drv(&["x", "2", "999", "12", "BROKEN", "", "F:"]),
            drv(&["3", "2", "999"]),
        ];
        assert_eq!(drive_index(&drives, "D:"), Some(0));
        assert_eq!(drive_index(&drives, "E:"), Some(1));
        assert_eq!(drive_index(&drives, "F:"), None);
        assert_eq!(drive_index(&drives, "G:"), None);
    }

    #[test]
    fn the_drive_index_requires_an_exact_match_of_the_seventh_field() {
        let drives = vec![drv(&["0", "2", "999", "12", "BD-RE DRIVE A 1.00 SERIAL", "DISC", "D:"])];
        assert_eq!(drive_index(&drives, "D:"), Some(0));
        assert_eq!(drive_index(&drives, "D:\\"), None);
        assert_eq!(drive_index(&drives, "d:"), None);
    }

    use lazymkv_core::job::Tool;
    use lazymkv_platform::Host;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct KeyHost { log: Option<PathBuf>, key: Option<String> }

    impl Host for KeyHost {
        fn now_unix(&self) -> u64 { 0 }
        fn local_date(&self) -> String { String::new() }
        fn makemkv_app_key(&self) -> Option<String> { self.key.clone() }
        fn makemkv_debug_log(&self) -> Option<PathBuf> { self.log.clone() }
        fn running_processes(&self) -> Vec<String> { Vec::new() }
        fn volume_label(&self, _: &Path) -> Option<String> { None }
        fn default_install_path(&self, _: Tool) -> Option<PathBuf> { None }
    }

    fn now() -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
    }

    #[test]
    fn the_debug_log_is_saved_with_the_key_removed() {
        let dir = crate::testutil::temp_dir("debug-key");
        let log = dir.join("MakeMKV_log.txt");
        std::fs::write(&log, b"key ABC and ABC again, caf\xe9\n").unwrap();
        let host = KeyHost { log: Some(log), key: Some("ABC".to_string()) };
        save_debug_log(&host, now() - 60, &dir.join("scan.txt")).unwrap();
        assert_eq!(std::fs::read(dir.join("scan.txt.makemkv-debug.txt")).unwrap(),
            b"key [app_Key removed] and [app_Key removed] again, caf\xe9\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_debug_log_older_than_the_call_is_not_saved() {
        let dir = crate::testutil::temp_dir("debug-old");
        let log = dir.join("MakeMKV_log.txt");
        std::fs::write(&log, b"old").unwrap();
        let host = KeyHost { log: Some(log.clone()), key: None };
        assert_eq!(save_debug_log(&host, now() + 3600, &dir.join("scan.txt")),
            Err(format!("{}: the debug log is older than the call", log.display())));
        assert!(!dir.join("scan.txt.makemkv-debug.txt").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_debug_log_is_reported_not_saved() {
        let dir = crate::testutil::temp_dir("debug-missing");
        let log = dir.join("MakeMKV_log.txt");
        let host = KeyHost { log: Some(log.clone()), key: None };
        assert_eq!(save_debug_log(&host, now(), &dir.join("scan.txt")),
            Err(format!("{}: no debug log was written", log.display())));
        let none = KeyHost { log: None, key: None };
        assert_eq!(save_debug_log(&none, now(), &dir.join("scan.txt")),
            Err("MakeMKV's debug log path is not known".to_string()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Review round 1, V6: a title folder that cannot be listed still reads the staging health, once, and is classed
    /// with it like any failed title (spec 10.2: the share first, then a write error).
    #[test]
    fn a_title_folder_that_cannot_be_listed_reads_the_staging_health_once() {
        use lazymkv_core::job::OutputCause;
        use std::cell::Cell;

        let scratch = crate::testutil::temp_dir("rip-unlisted");
        let dir = scratch.join("t00-00800");
        std::fs::write(&dir, b"not a folder").unwrap();
        let unlisted = std::fs::read_dir(&dir).unwrap_err();
        assert_ne!(unlisted.kind(), std::io::ErrorKind::NotFound, "the guard: a file is not a missing folder");
        let run = RunResult {
            exit_code: Some(0),
            lines: 0,
            elapsed_ms: 0,
            started_unix: 0,
            timed_out: false,
            cancelled: false,
            stdout: Vec::new(),
        };
        let summary = RunSummary { title_failed: true, write_error: true, ..RunSummary::default() };
        for (share_healthy, cause) in [(true, OutputCause::WriteError), (false, OutputCause::ShareUnreachable)] {
            let calls = Cell::new(0);
            let staging = || {
                calls.set(calls.get() + 1);
                StagingHealth { share_healthy, out_of_space: false }
            };
            match rip_outcome(&run, &summary, Kill::None, &dir, &staging) {
                RipOutcome::Failed { failure, message } => {
                    assert_eq!(failure, Failure { class: FailureClass::Output, output: Some(cause) });
                    assert!(message.starts_with("MakeMKV reported the title failed (exit code 0); "), "{message}");
                    assert!(message.contains(&dir.display().to_string()), "{message}");
                }
                other => panic!("expected Failed, got {other:?}"),
            }
            assert_eq!(calls.get(), 1, "staging is read once");
        }
        let _ = std::fs::remove_dir_all(&scratch);
    }
}
