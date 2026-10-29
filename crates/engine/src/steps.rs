//! The real step runner on the tools (design parts 5.2, 5.4 to 5.8, 6.4, 6.5): the disc phase (the job profile and its
//! 1009 fallback, the drive listing before every `makemkvcon` call, the scan with its one retry past its limit, the
//! plan, the rip check, the rip with the index guard, the rip's verification and the eject) and the file phase (the
//! probe, the decision rendered through `lazymkv-dev`'s `decide_doc`, the remux check, the remux into the library
//! root's partial folder, the partial file's verification and the rename that never replaces). Every run is recorded
//! capture-style in the job folder, with a `run` and an `ended` line in job.log (design part 7.2). A step never posts
//! to the Outbox once its CancelToken is raised (R22.11).

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, TryLockError};
use std::thread;
use std::time::UNIX_EPOCH;

use lazymkv_clients::{run_round, HttpTransport, LookupBook, LookupState, PendingReason};
use lazymkv_core::audio::rule::RULE_VERSION;
use lazymkv_core::decision::{same_path_reviews, FileDecision};
use lazymkv_core::disc::{disc, same_scan_title, Disc, Title};
use lazymkv_core::ipc::{OutputStream, CANCELLED};
use lazymkv_core::job::{classify_title_failure, decision_done, Action, Event, Failure, FailureClass, State, Tool};
use lazymkv_core::naming::NamedPath;
use lazymkv_core::plan::movie::LibraryFacts;
use lazymkv_core::plan::{plan, Kind, Mode, Plan, PlanInputs, PlannedTitle};
use lazymkv_core::profile::{
    make_job_profile, ProfileError, ALL_TRACKS_SELECTION, FALLBACK_PROFILE_FILE, FALLBACK_SELECTION, JOB_PROFILE_FILE,
};
use lazymkv_core::record::{FiledFile, JobRecord, TitleRecord};
use lazymkv_core::recording::files::{FfprobeStreams, MkvmergeIdent, Selection};
use lazymkv_core::review::{Note, ReviewReason};
use lazymkv_core::robot::lines::RunSummary;
use lazymkv_core::robot::{parse, Scan};
use lazymkv_core::settings::version_triple;
use lazymkv_core::tracks::subtitle::step10_runs;
use lazymkv_core::ustar::{ustar_entry, DEFAULT_PROFILE_NAME};
use lazymkv_core::verify::{
    failure_reviews, removed_scan_streams, rip_title, selected_streams, verify_final, verify_rip, VerifyReport,
};
use lazymkv_dev::cmd::decide::{decide_doc, DecideDoc, DecisionEntry, EngineDecideDoc};
use lazymkv_dev::evidence::Ctx;
use lazymkv_dev::pyjson::{dumps, Style};
use lazymkv_platform::{
    rename_no_replace, write_atomic, Ejector, Host, Platform, ProcessGuard, Reach, SecretStore, ShareConnector,
};
use lazymkv_tools::makemkv::{EXIT_WAIT_PASSED, SCAN_PAST_LIMIT};
use lazymkv_tools::{
    drive_list_args, find_tool, ident, ident_args, list_drives, probe, probe_args, remux, remux_args, rip, rip_args,
    save_debug_log, scan, scan_args, snapshot, with_suffix, CancelToken, Limits, MakemkvFlags, RipOutcome, RunResult,
    ScanOutcome, StagingHealth, ToolCtx,
};
use serde_json::{json, Value};

use crate::arrival::chain_inputs;
use crate::file_phase::{
    engine_decide_inputs, library_facts, lookup_word, partial_path, read_back_inputs, refreshed_plan,
};
use crate::owner::{action_tag, panic_text, JobContext, Outbox, StepResult, StepRunner};
use crate::restart::READ_BACK_LINE;
use crate::selftest::MakeMkv;
use crate::space::{
    make_root, planned_remux_promise, refine_remux_promise, rip_check, rip_promise, staged_bytes, staged_remux_check,
    Promises,
};
use crate::store::Store;
use crate::EngineConfig;

/// Ruling R23.6: a failed rip's staging is out of space below 1 MiB free, because design part 5.6 gives no number.
const OUT_OF_SPACE: u64 = 1_048_576;

/// The message of a Copy plan, which phase 2 does not rip (design part 5.5, P1).
const COPY_PLAN: &str = "the plan needs whole-disc copy mode, which arrives in phase 3";

/// The class of every failure the engine finds itself (design part 5.2).
const UNKNOWN: Failure = Failure { class: FailureClass::Unknown, output: None };

/// The sentence of a Decide whose title needs step 10's subtitle evidence, which this version does not build (S32).
const STEP10_NOT_BUILT: &str = "subtitle evidence (step 10) is not built in this version";

/// ExtractSubtitleEvidence's job.log line (S33): step 10a's packet MD5 pass is not built either.
const STEP10A_NOT_RUN: &str = "step 10a did not run in this version";

/// Locks `mutex`, ignoring a poison a panicked thread left.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A step's result: Err is an early end, carrying the result as Ok does.
type Outcome = Result<StepResult, StepResult>;

/// A result feeding `event`, with no message.
fn event(event: Event) -> StepResult {
    StepResult { event: Some(event), message: None }
}

/// A result feeding `event` with `message`.
fn with(event: Event, message: impl Into<String>) -> StepResult {
    StepResult { event: Some(event), message: Some(message.into()) }
}

/// A result feeding `failed(Unknown)` with `message`.
fn unknown(failed: fn(Failure) -> Event, message: impl Into<String>) -> StepResult {
    with(failed(UNKNOWN), message)
}

/// The progress callback of a run that reports none to the owner.
fn no_progress(_: f64) {}

/// A result with no event and `message`.
fn none(message: impl Into<String>) -> StepResult {
    StepResult { event: None, message: Some(message.into()) }
}

/// The result of a step whose CancelToken was raised: no event, and nothing posted (R22.11).
fn cancelled() -> StepResult {
    none(CANCELLED)
}

/// Whether a title is done with the disc phase: verified, or kept after its second failed verification (design part
/// 5.6 step 7).
fn is_done(title: &TitleRecord) -> bool {
    title.verified || title.rip_reviews.contains(&ReviewReason::VerifyFailed)
}

/// The position in `record.titles` of the first title not done, the one RipTitle and VerifyFile work on.
fn current_title(record: &JobRecord) -> Option<usize> {
    record.titles.iter().position(|title| !is_done(title))
}

/// `<stem>-<n>.txt`'s highest n in `dir`, 0 when there is none.
fn highest_numbered(dir: &Path, stem: &str) -> u32 {
    let prefix = format!("{stem}-");
    fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|entry| entry.ok())
                .filter_map(|entry| {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    name.strip_prefix(&prefix)?.strip_suffix(".txt")?.parse::<u32>().ok()
                })
                .max()
                .unwrap_or(0)
        })
        .unwrap_or(0)
}

/// The JSON file at `path`, or None when it is absent or not JSON.
fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}

/// The text of the file at `path` with invalid UTF-8 replaced, or None when it cannot be read.
fn read_lossy(path: &Path) -> Option<String> {
    fs::read(path).ok().map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
}

/// Adds `path` to the record's collisions unless it is there, keeping them sorted [ENG-22].
fn add_collision(record: &mut JobRecord, path: String) {
    if !record.collisions.contains(&path) {
        record.collisions.push(path);
        record.collisions.sort();
    }
}

/// Whether `a` and `b` name one file, as both names of a rename stopped between its hard link and its remove do: both
/// are files of equal size and modification time (design part 6.5).
#[cfg(not(windows))]
fn one_file(a: &Path, b: &Path) -> bool {
    match (fs::metadata(a), fs::metadata(b)) {
        (Ok(a), Ok(b)) => {
            let same_time = a.modified().ok().is_some_and(|at| b.modified().ok() == Some(at));
            a.is_file() && b.is_file() && a.len() == b.len() && same_time
        }
        _ => false,
    }
}

/// The message of a rename whose hard link was made and whose remove of the partial name failed (design part 6.5).
#[cfg(not(windows))]
fn half_done(partial: &Path, target: &str, error: &io::Error) -> String {
    format!("{target} holds the filed file, but the partial file {} could not be removed: {error}", partial.display())
}

/// `dir` emptied, or made when absent.
fn empty_dir(dir: &Path) -> io::Result<()> {
    match fs::remove_dir_all(dir) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    fs::create_dir_all(dir)
}

/// The regular `.mkv` files directly in `dir`, in any ASCII case, sorted (design part 5.6 step 6: a title's staging
/// folder holds one). Every caller runs after RipTitle made the folder, so a folder that cannot be listed, a missing
/// one included, and an entry that cannot be read are an Err naming the folder, `<dir>: <error>` (review round 1, V3;
/// RR1-6(a)). The tools crate's copy, which reads a missing folder as empty for the rip's own count, stays separate.
/// Def: T23, its Err PR #6 review (V3). Use: T23, T24, T25.
pub(crate) fn mkv_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let shown = |error: io::Error| format!("{}: {error}", dir.display());
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).map_err(shown)? {
        let path = entry.map_err(shown)?.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("mkv")) {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

/// The class of a title staging folder that cannot be emptied or made before a rip (spec 7, 10.1, 10.2's last
/// paragraph; review round 1, V5 and RR1-8): a write error on staging, classed by the share check as a failed rip is,
/// so Output, which leads to AwaitingShare, or to AwaitingSpace when staging is out of space; never Unknown.
fn staging_write_failure(health: StagingHealth) -> Failure {
    classify_title_failure(health.share_healthy, health.out_of_space, true, false)
}

/// The class of a title folder VerifyFile cannot list after a rip (review round 1, V3 with V9, RR1-7): classed by the
/// share check with no write error and no read errors, so Output while staging does not answer (AwaitingShare) or is
/// out of space (AwaitingSpace), and Unknown otherwise, never a write error a healthy staging would wait on.
fn unlisted_title_failure(health: StagingHealth) -> Failure {
    classify_title_failure(health.share_healthy, health.out_of_space, false, false)
}

/// A plain sentence for a profile the default profile could not become.
fn profile_error(error: ProfileError) -> &'static str {
    match error {
        ProfileError::NoNameElement => "has no <name> element",
        ProfileError::NoDefaultSelection => "has no defaultSelection attribute",
    }
}

/// `<makemkvcon's folder>/appdata.tar`'s default profile through make_job_profile, named `lazymkv-<job id>` (S50),
/// written as `<job_dir>/<file>` with write_atomic (design part 5.4 step 4, K6, L4). Every Err names the file at
/// fault. Def: T23. Use: T23.
pub(crate) fn write_job_profile(
    makemkvcon: &Path,
    job_dir: &Path,
    job_id: &str,
    selection: &str,
    file: &str,
) -> Result<PathBuf, String> {
    let tar = makemkvcon.parent().unwrap_or_else(|| Path::new("")).join("appdata.tar");
    let shown = tar.display();
    let archive = fs::read(&tar).map_err(|error| format!("{shown}: {error}"))?;
    let default = ustar_entry(&archive, DEFAULT_PROFILE_NAME)
        .map_err(|error| format!("{shown}: {error}"))?
        .ok_or_else(|| format!("{shown}: holds no {DEFAULT_PROFILE_NAME}"))?;
    let profile = make_job_profile(&String::from_utf8_lossy(default), &format!("lazymkv-{job_id}"), selection)
        .map_err(|error| format!("{shown}: {DEFAULT_PROFILE_NAME} {}", profile_error(error)))?;
    let path = job_dir.join(file);
    fs::create_dir_all(job_dir)
        .and_then(|()| write_atomic(&path, profile.as_bytes()))
        .map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(path)
}

/// The N.N.N of a MSG:1005 version: the word after "MakeMKV v", else the whole text, through version_triple (design
/// part 3.7, spec 9.1). Def: T23. Use: T23, T26.
pub(crate) fn version_of(version: &str) -> Option<(u32, u32, u32)> {
    const BANNER: &str = "MakeMKV v";
    let text = match version.find(BANNER) {
        Some(at) => {
            let rest = &version[at + BANNER.len()..];
            &rest[..rest.find(char::is_whitespace).unwrap_or(rest.len())]
        }
        None => version,
    };
    version_triple(text)
}

/// `<title dir>/attempt-<n>` with n one more than the highest existing attempt (1 for the first; design part 7.1).
/// Def: T23. Use: T23.
pub(crate) fn next_attempt_dir(title_dir: &Path) -> PathBuf {
    let highest = fs::read_dir(title_dir)
        .map(|entries| {
            entries
                .filter_map(|entry| entry.ok())
                .filter_map(|entry| entry.file_name().to_string_lossy().strip_prefix("attempt-")?.parse::<u32>().ok())
                .max()
                .unwrap_or(0)
        })
        .unwrap_or(0);
    title_dir.join(format!("attempt-{}", highest + 1))
}

/// The folder of the job folder the Fingerprint step snapshots into before it compares fingerprints (engine.md:587,
/// review round 1's V10). Def: T3-18. Use: T3-18.
pub(crate) const FRESH_SNAPSHOT: &str = "fingerprint.lazymkv-new";

/// `path` removed, a folder with everything in it and anything else as a file; a missing one is not an error.
fn remove_entry(path: &Path) -> io::Result<()> {
    let removed = match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(error) => Err(error),
    };
    match removed {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        removed => removed,
    }
}

/// The Fingerprint step's recording (design part 5.4 step 2; engine.md:587): `tools::snapshot` of the disc at `root`
/// into `<job_dir>/FRESH_SNAPSHOT`, which a crash may have left and is removed first; on the job's `fingerprint`,
/// `files.json`, `nav` and `meta.json` replace the job folder's own, and `FRESH_SNAPSHOT` is removed. On any other
/// outcome the job folder's snapshot is untouched and `FRESH_SNAPSHOT` is removed. Err is the step's message:
/// `the disc's recording could not be written: <error>` (the snapshot, or a leftover that cannot be removed),
/// `the disc's fingerprint <found or "none"> differs from the job's <fingerprint>`, or `the disc's recording could not
/// be put in place: <error>` (the swap). Def: T3-18. Use: T3-18.
pub(crate) fn snapshot_aside(
    root: &Path,
    label: &str,
    job_dir: &Path,
    captured_unix: u64,
    fingerprint: &str,
) -> Result<(), String> {
    let fresh = job_dir.join(FRESH_SNAPSHOT);
    let unwritten = |error: io::Error| format!("the disc's recording could not be written: {error}");
    remove_entry(&fresh).map_err(unwritten)?;
    // A removal that fails after a failed or differing snapshot leaves the folder to the next run's first removal, so
    // its error never replaces the step's message (Ruling PF3-S18-20-3).
    let found = match snapshot(root, label, &fresh, captured_unix) {
        Ok(meta) => meta.fingerprint,
        Err(error) => {
            let _ = remove_entry(&fresh);
            return Err(unwritten(error));
        }
    };
    if found.as_deref() != Some(fingerprint) {
        let _ = remove_entry(&fresh);
        let found = found.as_deref().unwrap_or("none");
        return Err(format!("the disc's fingerprint {found} differs from the job's {fingerprint}"));
    }
    let unplaced = |error: io::Error| format!("the disc's recording could not be put in place: {error}");
    for name in ["files.json", "nav", "meta.json"] {
        let (from, to) = (fresh.join(name), job_dir.join(name));
        remove_entry(&to).map_err(unplaced)?;
        match fs::symlink_metadata(&from) {
            Ok(_) => fs::rename(&from, &to).map_err(unplaced)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(unplaced(error)),
        }
    }
    fs::remove_dir(&fresh).map_err(unplaced)
}

/// rip-summary.json's `started`: `host.local_time` of the title record's `rip_started_ms`, None when the record has
/// none (R23.7; Ruling W16). Def: T3-9. Use: T3-9 (`write_rip_summary`).
pub(crate) fn rip_started_text(title: &TitleRecord, host: &dyn Host) -> Option<String> {
    title.rip_started_ms.map(|unix_ms| host.local_time(unix_ms))
}

/// One lookup round on its own thread (design part 8.2): run_round over the locked book, each SavedFile written with
/// write_atomic into the job folder (a write that fails is logged in engine.log); `done` gets `book.pending()` or
/// chain_inputs' Err. The round holds the book for the whole round (R23.1). Each lookup the round leaves
/// `Pending { Unparsed(sentence) }` writes the job.log line `lookup <word>: <sentence>` (design part 8.3, R24.6). A
/// panic of the round is caught and given to `done` as an Err. Def: T23. Use: T23, T24.
pub(crate) fn spawn_round(
    job_dir: PathBuf,
    lookups: Arc<Mutex<LookupBook>>,
    http: Arc<dyn HttpTransport>,
    token: Option<String>,
    today: String,
    store: Arc<Store>,
    done: Box<dyn FnOnce(Result<(bool, bool), String>) + Send>,
) {
    thread::spawn(move || {
        let round = panic::catch_unwind(AssertUnwindSafe(|| {
            let inputs = chain_inputs(&job_dir)?;
            let mut book = lock(&lookups);
            run_round(http.as_ref(), token.as_deref(), &mut book, &inputs, &today, &mut |file| {
                let path = job_dir.join(&file.name);
                if let Err(error) = write_atomic(&path, &file.bytes) {
                    store.log(&format!("{}: the lookup answer could not be written: {error}", path.display()));
                }
            });
            let unparsed: Vec<String> = book
                .states
                .iter()
                .filter_map(|(lookup, state)| match state {
                    LookupState::Pending { reason: PendingReason::Unparsed(sentence) } => {
                        Some(format!("lookup {}: {sentence}", lookup_word(*lookup)))
                    }
                    _ => None,
                })
                .collect();
            let pending = book.pending();
            drop(book);
            if let Some(id) = job_dir.file_name().and_then(|name| name.to_str()) {
                for line in &unparsed {
                    store.job_log(id, line);
                }
            }
            Ok(pending)
        }));
        done(round.unwrap_or_else(|payload| Err(format!("the lookup round panicked: {}", panic_text(&*payload)))));
    });
}

/// The real StepRunner (design part 5.2): Fingerprint, RunScan, Plan, CheckSpace in Planned, RipTitle, VerifyFile in
/// Ripping, Eject and EmptyTitleDirectory for the disc phase (Task 23); Probe, ExtractSubtitleEvidence, Decide,
/// CheckSpace in Decided, Remux, VerifyFile in Remuxing, File and DeletePartialFile for the file phase (Task 24).
/// Def: T23. Use: T23 to T29.
pub(crate) struct RealSteps {
    /// The resources folder the tool lookup reads second (design part 3.6).
    resources: Option<PathBuf>,
    /// Limits that replace `Limits::from_settings`, for tests only.
    limits_override: Option<Limits>,
    /// The `PATH` the tool lookup reads last (design part 3.6).
    path_var: Option<OsString>,
    processes: Arc<dyn ProcessGuard>,
    host: Arc<dyn Host>,
    shares: Arc<dyn ShareConnector>,
    secrets: Arc<dyn SecretStore>,
    ejector: Arc<dyn Ejector>,
    http: Arc<dyn HttpTransport>,
    store: Arc<Store>,
    outbox: Outbox,
    promises: Arc<Mutex<Promises>>,
    /// The last scan's MSG:3344 verdict (design part 5.4 step 4).
    java_in_use: Arc<Mutex<Option<bool>>>,
    /// The MakeMKV state each drive listing is observed into (design part 3.7, S79).
    makemkv: Arc<MakeMkv>,
    /// The probe of each (job, title key) Probe ran in this engine run, which Decide reads (design part 5.7 step 11.3).
    probes: Mutex<BTreeMap<(String, String), FfprobeStreams>>,
}

impl RealSteps {
    /// The runner over the engine's configuration and platform parts (design part 5.2). `java_in_use` holds the last
    /// scan's MSG:3344 verdict (design part 5.4 step 4); the owner keeps the same Arc for Task 26's self-test, which
    /// reads Java use from the last scan (design part 9.4). `makemkv` is the engine's MakeMKV state, which every drive
    /// listing feeds (S79). Def: T23. Use: T23, T26.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        config: &EngineConfig,
        platform: &Platform,
        http: Arc<dyn HttpTransport>,
        store: Arc<Store>,
        outbox: Outbox,
        promises: Arc<Mutex<Promises>>,
        java_in_use: Arc<Mutex<Option<bool>>>,
        makemkv: Arc<MakeMkv>,
    ) -> RealSteps {
        RealSteps {
            resources: config.resources.clone(),
            limits_override: config.limits_override,
            path_var: config.path_var.clone(),
            processes: Arc::clone(&platform.processes),
            host: Arc::clone(&platform.host),
            shares: Arc::clone(&platform.shares),
            secrets: Arc::clone(&platform.secrets),
            ejector: Arc::clone(&platform.ejector),
            http,
            store,
            outbox,
            promises,
            java_in_use,
            makemkv,
            probes: Mutex::default(),
        }
    }
}

impl StepRunner for RealSteps {
    fn run(
        &self,
        job: &JobContext,
        action: &Action,
        cancel: CancelToken,
        progress: &(dyn Fn(Option<&str>, f64) + Sync),
    ) -> StepResult {
        let limits = self.limits_override.unwrap_or_else(|| Limits::from_settings(&job.settings));
        let step = Step { steps: self, job, id: &job.record.id, cancel, limits, progress };
        let outcome = match (action, &job.record.state) {
            (Action::Fingerprint, _) => step.fingerprint(),
            (Action::RunScan, _) => step.run_scan(),
            (Action::Plan, _) => step.plan(),
            (Action::CheckSpace, State::Planned { .. }) => step.check_space(),
            (Action::CheckSpace, State::Decided { .. }) => step.remux_space(),
            (Action::RipTitle, _) => step.rip_title(),
            (Action::VerifyFile, State::Ripping) => step.verify_rip(),
            (Action::VerifyFile, State::Remuxing) => step.verify_partial(),
            (Action::Eject, _) => step.eject(),
            (Action::EmptyTitleDirectory, _) => step.empty_title_directory(),
            (Action::Probe, _) => step.probe(),
            (Action::ExtractSubtitleEvidence, _) => step.extract_subtitle_evidence(),
            (Action::Decide, _) => step.decide(),
            (Action::Remux, _) => step.remux(),
            (Action::File, _) => step.file(),
            (Action::DeletePartialFile, _) => step.delete_partial_file(),
            (other, state) => {
                Ok(none(format!("{} in {} is not built in this version", action_tag(other), state.as_str())))
            }
        };
        outcome.unwrap_or_else(|early| early)
    }
}

/// One step of one job: the runner, the job, its cancel token, its limits and the progress callback.
struct Step<'a> {
    steps: &'a RealSteps,
    job: &'a JobContext,
    id: &'a str,
    cancel: CancelToken,
    limits: Limits,
    progress: &'a (dyn Fn(Option<&str>, f64) + Sync),
}

impl Step<'_> {
    /// Appends `line` to the job's job.log.
    fn log(&self, line: &str) {
        self.steps.store.job_log(self.id, line);
    }

    /// `find_tool`'s path of `tool` (design part 3.6), when it is a file: a tool the settings name but that is not
    /// there is missing, so its job waits in AwaitingTool (design part 3.7).
    fn tool(&self, tool: Tool) -> Option<PathBuf> {
        let (steps, settings) = (self.steps, &self.job.settings);
        let resources = steps.resources.as_deref();
        let found = find_tool(tool, settings, resources, steps.host.as_ref(), steps.path_var.as_deref());
        found.map(|found| found.path).filter(|path| path.is_file())
    }

    /// Whether every makemkvcon call gets `--debug` (S45).
    fn debug(&self) -> bool {
        self.job.settings.keep_tuning_evidence
    }

    /// The job's makemkvcon flags with `profile`, a file of the job folder (design part 5.4 step 4).
    fn flags(&self, profile: &str) -> MakemkvFlags {
        let settings = &self.job.settings;
        MakemkvFlags {
            minlength_s: settings.minlength_s,
            cache_mb: settings.cache_mb,
            profile: self.job.job_dir.join(profile),
            debug: self.debug(),
        }
    }

    /// Runs `call` with a ToolCtx over the runner's guard and host, the step's limits and cancel token, no output
    /// callback and `on_progress`.
    fn ctx<R>(&self, on_progress: &(dyn Fn(f64) + Sync), call: impl FnOnce(&ToolCtx) -> R) -> R {
        let on_output = |_: OutputStream, _: &str| {};
        let ctx = ToolCtx {
            guard: self.steps.processes.as_ref(),
            host: self.steps.host.as_ref(),
            limits: &self.limits,
            cancel: self.cancel.clone(),
            on_output: &on_output,
            on_progress,
        };
        call(&ctx)
    }

    /// The job.log line before a run: `run <name>: <program> <args joined by " ">` (design part 7.1).
    fn started(&self, name: &str, program: &Path, args: &[String]) {
        self.log(&format!("run {name}: {} {}", program.display(), args.join(" ")));
    }

    /// The job.log line after a run: `ended <name>: exit <code or none> after <elapsed_ms> ms`, from the run.
    fn ended(&self, name: &str, run: &RunResult) {
        let code = run.exit_code.map_or_else(|| "none".to_string(), |code| code.to_string());
        self.log(&format!("ended {name}: exit {code} after {} ms", run.elapsed_ms));
    }

    /// `ended` for a driver that gives no RunResult, from the run's `<out>.exit.json` (removed before the run, so it
    /// is this run's); a missing file says so.
    fn ended_from_file(&self, name: &str, out: &Path) {
        match read_json(&with_suffix(out, ".exit.json")) {
            Some(exit) => {
                let code = exit["exit_code"].as_i64().filter(|code| *code != -1);
                let code = code.map_or_else(|| "none".to_string(), |code| code.to_string());
                self.log(&format!("ended {name}: exit {code} after {} ms", exit["elapsed_ms"].as_u64().unwrap_or(0)));
            }
            None => self.log(&format!("ended {name}: no exit file was written")),
        }
    }

    /// The job.log line of a run that could not be recorded.
    fn not_run(&self, name: &str, error: &io::Error) {
        self.log(&format!("ended {name}: {error}"));
    }

    /// With `keep_tuning_evidence` on, MakeMKV's debug log saved beside `out` (design part 7.2, S45, S61); a log not
    /// saved only writes `<name>: the MakeMKV debug log was not saved: <reason>`.
    fn keep_debug_log(&self, name: &str, started_unix: u64, out: &Path) {
        if !self.debug() {
            return;
        }
        if let Err(error) = save_debug_log(self.steps.host.as_ref(), started_unix, out) {
            self.log(&format!("{name}: the MakeMKV debug log was not saved: {error}"));
        }
    }

    /// Posts an edit of the job's record, unless the step was cancelled (R22.11). False when cancelled.
    fn post_edit(&self, edit: impl FnOnce(&mut JobRecord) + Send + 'static) -> bool {
        if self.cancel.is_cancelled() {
            return false;
        }
        self.steps.outbox.edit(self.id, edit);
        true
    }

    /// Posts an edit of the title `key`, unless the step was cancelled (R22.11). False when cancelled.
    fn post_title(&self, key: &str, edit: impl FnOnce(&mut TitleRecord) + Send + 'static) -> bool {
        let key = key.to_string();
        self.post_edit(move |record| {
            if let Some(title) = record.titles.iter_mut().find(|title| title.key == key) {
                edit(title);
            }
        })
    }

    /// Err(cancelled) once the step's CancelToken is raised.
    fn check_cancel(&self) -> Result<(), StepResult> {
        if self.cancel.is_cancelled() {
            Err(cancelled())
        } else {
            Ok(())
        }
    }

    /// The scan the plan was made from, `<job_dir>/scan.txt`, read with replacement and typed.
    fn planned_scan(&self) -> Result<(Scan, Disc), String> {
        let path = self.job.job_dir.join("scan.txt");
        let bytes = fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        let scan = parse(&String::from_utf8_lossy(&bytes));
        let typed = disc(&scan).map_err(|error| format!("{}: {error}", path.display()))?;
        Ok((scan, typed))
    }

    /// The drive listing before a makemkvcon call (design part 5.4 step 1): recorded as `drive-list.txt` at the job's
    /// first listing and as `drive-list-<n>.txt` after it (RP-1), with the job.log line `drive listing <n>: <drive> is
    /// index <index>`. Its (version, licence code) is observed into the engine's MakeMKV state (S79), unless the step
    /// was cancelled. A listing the exit wait killed after its final message gives `failed(Unknown)` with
    /// `EXIT_WAIT_PASSED`, the process not having exited (design part 5.3, spec 10.4). A listing with no DRV line for
    /// the job's drive gives `failed(Unknown)` with `MakeMKV's drive listing has no drive <drive>` (RP-2). A job's
    /// listing has no time limit (S57; Ruling W20 bounds only the checks).
    fn listing(&self, program: &Path, failed: fn(Failure) -> Event) -> Result<(u32, RunSummary), StepResult> {
        let drive = self.job.record.drive.as_str();
        if drive.is_empty() {
            // `drive_index` with an empty id would match an empty DRV slot (Task 13's carried note).
            return Err(unknown(failed, "the job names no drive"));
        }
        let dir = &self.job.job_dir;
        let n = if dir.join("drive-list.txt").is_file() { highest_numbered(dir, "drive-list").max(1) + 1 } else { 1 };
        let name = if n == 1 { "drive-list.txt".to_string() } else { format!("drive-list-{n}.txt") };
        let out = dir.join(&name);
        self.started(&name, program, &drive_list_args(self.debug()));
        let listed = self.ctx(&no_progress, |ctx| list_drives(ctx, program, self.debug(), drive, &out, None));
        let listing = match listed {
            Ok(listing) => listing,
            Err(error) => {
                self.not_run(&name, &error);
                return Err(unknown(failed, format!("the drive listing could not run: {error}")));
            }
        };
        self.ended(&name, &listing.run);
        self.keep_debug_log(&name, listing.run.started_unix, &out);
        self.check_cancel()?;
        let summary = &listing.summary;
        self.steps.makemkv.observe(summary.version.clone(), summary.licence.as_ref().map(|message| message.code));
        if listing.run.timed_out {
            return Err(unknown(failed, EXIT_WAIT_PASSED));
        }
        match listing.index {
            Some(index) => {
                self.log(&format!("drive listing {n}: {drive} is index {index}"));
                Ok((index, listing.summary))
            }
            None => Err(unknown(failed, format!("MakeMKV's drive listing has no drive {drive}"))),
        }
    }

    /// A scan into `<job_dir>/<name>` (design part 5.4 step 4), recorded with its job.log lines.
    fn scan_into(&self, name: &str, program: &Path, flags: &MakemkvFlags, index: u32) -> io::Result<ScanOutcome> {
        let out = self.job.job_dir.join(name);
        let _ = fs::remove_file(with_suffix(&out, ".exit.json"));
        self.started(name, program, &scan_args(flags, index));
        let started_unix = self.steps.host.now_unix();
        let scanned = self.ctx(&no_progress, |ctx| scan(ctx, program, flags, index, &out));
        match &scanned {
            Ok(_) => self.ended_from_file(name, &out),
            Err(error) => self.not_run(name, error),
        }
        self.keep_debug_log(name, started_unix, &out);
        scanned
    }

    /// Fingerprint, for a job a restart left in Detected (design parts 5.2, 5.4 step 2): the recording written again
    /// with `snapshot_aside`, into a fresh folder that replaces the job folder's snapshot only when the fingerprint is
    /// the job's (engine.md:587), so a different disc leaves the job's recording as it was; the same fingerprint gives
    /// FingerprintDone, anything else FingerprintFailed { Unknown } with `snapshot_aside`'s message.
    fn fingerprint(&self) -> Outcome {
        let failed = |failure| Event::FingerprintFailed { failure };
        let Some(root) = &self.job.drive_root else {
            return Ok(unknown(failed, "the disc is in no drive"));
        };
        let host = self.steps.host.as_ref();
        let label = host.volume_label(root).unwrap_or_default();
        match snapshot_aside(root, &label, &self.job.job_dir, host.now_unix(), &self.job.record.fingerprint) {
            Ok(()) => Ok(event(Event::FingerprintDone)),
            Err(message) => Ok(unknown(failed, message)),
        }
    }

    /// When the job's book has no lookup yet, the first round starts beside the scan (design parts 5.4 step 3, 8.2);
    /// a book a running round holds already has its round.
    fn start_lookups(&self) {
        let empty = match self.job.lookups.try_lock() {
            Ok(book) => book.states.is_empty(),
            Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner().states.is_empty(),
            Err(TryLockError::WouldBlock) => false,
        };
        if empty {
            let steps = self.steps;
            spawn_round(
                self.job.job_dir.clone(),
                Arc::clone(&self.job.lookups),
                Arc::clone(&steps.http),
                steps.secrets.get("tmdb_token"),
                steps.host.local_date(),
                Arc::clone(&steps.store),
                Box::new(|_| {}),
            );
        }
    }

    /// RunScan (design parts 5.2, 5.4 steps 1, 3 and 4). No makemkvcon, then no mkvmerge, gives ToolMissing for that
    /// tool before the drives are listed, the lookups start or the job profile is written: without mkvmerge no disc is
    /// started (spec 10.2; review round 1, V8), since the disc phase's first mkvmerge run is VerifyFile's, after a rip.
    fn run_scan(&self) -> Outcome {
        let failed = |failure| Event::ScanFailed { failure };
        let record = &self.job.record;
        let job_dir = &self.job.job_dir;
        let Some(makemkvcon) = self.tool(Tool::Makemkvcon) else {
            return Ok(event(Event::ToolMissing { tool: Tool::Makemkvcon }));
        };
        if self.tool(Tool::Mkvmerge).is_none() {
            return Ok(event(Event::ToolMissing { tool: Tool::Mkvmerge }));
        }
        let noted = record.notes.iter().any(|noted| noted.note == Note::LabelRuleUnavailable);
        if self.tool(Tool::Java).is_none() && !noted {
            self.check_cancel()?;
            self.steps.outbox.feed_first(self.id, Event::ToolMissing { tool: Tool::Java });
        }
        self.start_lookups();
        if !job_dir.join(JOB_PROFILE_FILE).is_file() {
            // RP-3: a makemkvcon without its appdata.tar is an incomplete install.
            let written = write_job_profile(&makemkvcon, job_dir, self.id, ALL_TRACKS_SELECTION, JOB_PROFILE_FILE);
            if let Err(error) = written {
                return Ok(with(Event::ToolMissing { tool: Tool::Makemkvcon }, error));
            }
        }
        let mut profile = record.profile.clone();
        let mut retried = false;
        loop {
            let (index, summary) = self.listing(&makemkvcon, failed)?;
            self.check_version(&makemkvcon, &summary)?;
            let scanned = self.scan_into("scan.txt", &makemkvcon, &self.flags(&profile), index);
            self.check_cancel()?;
            match scanned {
                Err(error) => return Ok(unknown(failed, format!("the scan could not run: {error}"))),
                Ok(ScanOutcome::Done { summary, .. }) => {
                    let version = summary.version.as_deref().unwrap_or("version unknown");
                    self.log(&format!("scan: MakeMKV {version}; java in use: {}", summary.java));
                    *lock(&self.steps.java_in_use) = Some(summary.java);
                    return Ok(event(Event::ScanDone));
                }
                Ok(ScanOutcome::ProfileRejected { text }) if profile == JOB_PROFILE_FILE => {
                    // L4, L5: one fallback only, named by every later call of the job.
                    let written =
                        write_job_profile(&makemkvcon, job_dir, self.id, FALLBACK_SELECTION, FALLBACK_PROFILE_FILE);
                    if let Err(error) = written {
                        return Ok(unknown(failed, format!("MakeMKV rejected {JOB_PROFILE_FILE}: {text}; {error}")));
                    }
                    if !self.post_edit(|record| record.profile = FALLBACK_PROFILE_FILE.to_string()) {
                        return Err(cancelled());
                    }
                    self.log(&format!("MakeMKV rejected {JOB_PROFILE_FILE}: {text}"));
                    profile = FALLBACK_PROFILE_FILE.to_string();
                }
                Ok(ScanOutcome::ProfileRejected { text }) => {
                    return Ok(unknown(failed, format!("MakeMKV rejected the fallback profile: {text}")));
                }
                Ok(ScanOutcome::Failed { message, .. }) if message == SCAN_PAST_LIMIT && !retried => {
                    // RP-5, S58: the engine lists the drives again and scans once more into the same scan.txt.
                    retried = true;
                    self.log(&format!("scan.txt: {message}; the drives are listed and the disc scanned once more"));
                }
                Ok(ScanOutcome::Failed { message, .. }) if message == SCAN_PAST_LIMIT => {
                    return Ok(unknown(failed, format!("{SCAN_PAST_LIMIT} twice")));
                }
                Ok(ScanOutcome::Failed { failure, message }) => return Ok(with(failed(failure), message)),
            }
        }
    }

    /// The listing's MakeMKV version against `makemkv_min_version` (design part 3.7, spec 9.1): below it gives
    /// ToolMissing { Makemkvcon } with `<tool file stem> is version <found>, below makemkv_min_version <setting>`
    /// (PF-24); no version only writes a job.log line.
    fn check_version(&self, program: &Path, summary: &RunSummary) -> Result<(), StepResult> {
        let Some(found) = summary.version.as_deref() else {
            self.log("the drive listing printed no MakeMKV version");
            return Ok(());
        };
        let minimum = &self.job.settings.makemkv_min_version;
        match (version_of(found), version_triple(minimum)) {
            (Some(have), Some(need)) if have < need => {
                let stem = program.file_stem().map(|stem| stem.to_string_lossy().into_owned()).unwrap_or_default();
                let message = format!("{stem} is version {found}, below makemkv_min_version {minimum}");
                Err(with(Event::ToolMissing { tool: Tool::Makemkvcon }, message))
            }
            _ => Ok(()),
        }
    }

    /// Plan (design part 5.4 step 5, S30): `core::plan::plan` over the scan, the book as it stands (R23.1: a book a
    /// running round holds counts as empty, so every lookup is pending) and the remembered decision, persisted with
    /// one TitleRecord per planned title. A Copy plan gives PlanFailed { Unknown } (P1, design part 5.5). The data
    /// folder's remembered decision is copied to `<job>/remembered.json`; when there is none, when it is refused, or
    /// when the copy cannot be written (`<path>: <error>` in job.log), a copy an earlier Plan of the job left there is
    /// removed, so the job folder never holds a remembered file the plan did not use, unless that copy can be neither
    /// replaced nor removed, both errors then in job.log (review round 1, the prerequisite of V14; the final review's
    /// C7).
    fn plan(&self) -> Outcome {
        let failed = |failure| Event::PlanFailed { failure };
        let record = &self.job.record;
        let (_, typed) = self.planned_scan().map_err(|error| unknown(failed, error))?;
        let inputs = chain_inputs(&self.job.job_dir).map_err(|error| unknown(failed, error))?;
        let book = match self.job.lookups.try_lock() {
            Ok(book) => book.clone(),
            Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner().clone(),
            Err(TryLockError::WouldBlock) => LookupBook::default(),
        };
        let remembered = match self.steps.store.remembered(&record.fingerprint) {
            Ok(Some((remembered, bytes))) => {
                let path = self.job.job_dir.join("remembered.json");
                if let Err(error) = write_atomic(&path, &bytes) {
                    self.log(&format!("{}: {error}", path.display()));
                    // An older copy left in place would hand Decide overrides this plan did not use (C7).
                    self.drop_remembered_copy();
                }
                Some(remembered)
            }
            Ok(None) => {
                self.drop_remembered_copy();
                None
            }
            Err(error) => {
                self.steps.store.log(&format!("{}: the remembered decision is refused: {error}", self.id));
                self.drop_remembered_copy();
                None
            }
        };
        let planned = plan(&PlanInputs {
            disc: &typed,
            fingerprint: Some(&record.fingerprint),
            disc_id: record.disc_id.as_deref(),
            lookups: book.lookup_inputs(inputs.name.as_ref()),
            remembered: remembered.as_ref(),
            overrides: None,
            sessions: &[],
            library: &LibraryFacts::default(),
            settings: &self.job.settings,
        })
        .map_err(|error| unknown(failed, error))?;
        let mode = planned.mode;
        let titles: Vec<TitleRecord> = planned
            .titles
            .iter()
            .map(|title| TitleRecord {
                key: title.key.clone(),
                scan_title: title.title,
                rip_index: None,
                attempts: 0,
                verified: false,
                verify_failures: 0,
                read_errors: Vec::new(),
                rip_reviews: Vec::new(),
                decision: None,
                filed: None,
                rip_started_ms: None,
            })
            .collect();
        if !self.post_edit(move |record| {
            record.plan = Some(planned);
            record.titles = titles;
        }) {
            return Err(cancelled());
        }
        Ok(match mode {
            Mode::Direct => event(Event::PlanDone { mode: Mode::Direct }),
            Mode::Copy => unknown(failed, COPY_PLAN),
        })
    }

    /// Removes `<job>/remembered.json`, the copy an earlier Plan of the job made, when it is there; a removal that
    /// fails writes `<path>: <error>` in job.log (review round 1, the prerequisite of V14).
    fn drop_remembered_copy(&self) {
        let path = self.job.job_dir.join("remembered.json");
        match fs::remove_file(&path) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => self.log(&format!("{}: {error}", path.display())),
            _ => {}
        }
    }

    /// CheckSpace's result from a check's `(event, sentence)` (design part 6.1, "the error says which"; review round 1,
    /// V15): with a sentence, the job.log line `check space: <sentence>` and the event carrying it, so the record, the
    /// waiting notification and job.log name the root and why; otherwise the event alone. A SpaceShort never carries
    /// its sentence, which only `staged_remux_check` gives it (staged files that could not be read): the sentence stays
    /// a job.log line, and AwaitingSpace's message and notification say what any shortage says, `waiting: space` (the
    /// final review of review round 1, N1). Called once the `Promises` guard is dropped.
    fn space_result(&self, (found, why): (Event, Option<String>)) -> StepResult {
        match why {
            Some(why) => {
                self.log(&format!("check space: {why}"));
                if found == Event::SpaceShort {
                    event(found)
                } else {
                    with(found, why)
                }
            }
            None => event(found),
        }
    }

    /// CheckSpace in Planned (design part 5.8): `rip_check` of the plan's rip and remux promises, staging against the
    /// library root of the plan's kind, its result through `space_result`. An unset `staging_root` gives ShareLost
    /// saying so. A staging root that answers `reach` is made first when only its last folder is missing (`make_root`,
    /// design part 6.2, Ruling W15), and one that cannot be made gives ShareLost with `make_root`'s sentence; one that
    /// does not answer is named by `rip_check`'s own `reach`. Every share read runs here, on the step's thread, before
    /// `rip_check` locks the promises (engine.md:584). Then job.log gets `share access for <root>: <access>` for
    /// staging and the library root whose `access` reports one (design parts 6.1, 7.1).
    fn check_space(&self) -> Outcome {
        let record = &self.job.record;
        let Some(plan) = &record.plan else {
            return Ok(none("the job has no plan"));
        };
        let settings = &self.job.settings;
        let Some(staging) = settings.staging_root.as_deref() else {
            return Ok(self.space_result((Event::ShareLost, Some("staging_root is unset".to_string()))));
        };
        let library = match plan.kind {
            Kind::Movie => settings.movies_root.as_deref(),
            Kind::Tv => settings.tv_root.as_deref(),
        };
        let meta = read_json(&self.job.job_dir.join("meta.json"));
        let disc_size = meta.and_then(|meta| meta["total_bytes"].as_u64()).unwrap_or(0);
        self.check_cancel()?;
        let (rip_bytes, remux_bytes) = (rip_promise(plan, disc_size), planned_remux_promise(plan));
        let shares = self.steps.shares.as_ref();
        if shares.reach(staging) == Reach::Reachable {
            if let Err(sentence) = make_root(staging) {
                return Ok(self.space_result((Event::ShareLost, Some(sentence))));
            }
        }
        let checked = rip_check(rip_bytes, remux_bytes, staging, library, shares, &self.steps.promises, self.id);
        for root in std::iter::once(staging).chain(library) {
            if let Some(access) = shares.access(root) {
                self.log(&format!("share access for {root}: {access}"));
            }
        }
        Ok(self.space_result(checked))
    }

    /// The staging folder of the title `key`: `<staging>/<job id>/rip/<key>` (design part 5.6 step 6).
    fn rip_dir(&self, key: &str) -> Option<PathBuf> {
        let staging = self.job.settings.staging_root.as_deref()?;
        Some(Path::new(staging).join(self.id).join("rip").join(key))
    }

    /// The staging share's health as `classify_title_failure` reads it (design part 5.6 step 6): whether `reach` finds
    /// the staging root reachable, and whether its free space reads as below `OUT_OF_SPACE` (R23.6).
    fn staging_health(&self) -> StagingHealth {
        let staging = self.job.settings.staging_root.as_deref().unwrap_or_default();
        let shares = self.steps.shares.as_ref();
        StagingHealth {
            share_healthy: shares.reach(staging) == Reach::Reachable,
            out_of_space: shares.free_bytes(staging).is_ok_and(|free| free < OUT_OF_SPACE),
        }
    }

    /// RipTitle (design part 5.6 step 6): the first title not done, its earlier attempt moved aside, its staging folder
    /// emptied, then the listing and the rip, with the index guard's rescan and remap up to the third mismatch. No
    /// makemkvcon, then no mkvmerge, gives ToolMissing for that tool before any file is touched, so no title is ripped
    /// that VerifyFile could not check (spec 10.2; review round 1, V8). A staging folder that cannot be emptied or made
    /// gives TitleRipFailed classed by the share check, `staging_write_failure` (review round 1, V5 and RR1-8), with
    /// `<folder>: <error>`.
    fn rip_title(&self) -> Outcome {
        let failed = |failure| Event::TitleRipFailed { failure };
        let record = &self.job.record;
        let Some(at) = current_title(record) else {
            return Ok(none("every planned title is ripped"));
        };
        let title = &record.titles[at];
        let key = title.key.as_str();
        let Some(makemkvcon) = self.tool(Tool::Makemkvcon) else {
            return Ok(event(Event::ToolMissing { tool: Tool::Makemkvcon }));
        };
        if self.tool(Tool::Mkvmerge).is_none() {
            return Ok(event(Event::ToolMissing { tool: Tool::Mkvmerge }));
        }
        let rip_dir = self.rip_dir(key).ok_or_else(|| unknown(failed, "staging_root is unset"))?;
        let (_, typed) = self.planned_scan().map_err(|error| unknown(failed, error))?;
        let planned = typed.title(title.scan_title).map_err(|error| unknown(failed, error))?;
        let key_dir = self.job.job_dir.join(key);
        if key_dir.join("rip.txt").is_file() {
            self.move_aside(&key_dir).map_err(|error| unknown(failed, error))?;
        }
        let flags = self.flags(&record.profile);
        let health = || self.staging_health();
        let on_progress = |fraction: f64| (self.progress)(Some(key), fraction);
        let out = key_dir.join("rip.txt");
        let name = format!("{key}/rip.txt");
        let mut index = title.rip_index.unwrap_or(title.scan_title);
        let mut moved = 0;
        loop {
            if let Err(error) = empty_dir(&rip_dir) {
                // The share is probed once, after the failure, on this worker thread.
                let failure = staging_write_failure(self.staging_health());
                return Ok(with(failed(failure), format!("{}: {error}", rip_dir.display())));
            }
            let (drive, _) = self.listing(&makemkvcon, failed)?;
            let started_ms = self.steps.host.now_ms();
            if !self.post_title(key, move |title| title.rip_started_ms = Some(started_ms)) {
                return Err(cancelled());
            }
            self.started(&name, &makemkvcon, &rip_args(&flags, drive, index, &rip_dir));
            let ripped = self.ctx(&on_progress, |ctx| {
                rip(ctx, &makemkvcon, &flags, drive, index, planned.source(), &rip_dir, &out, &health)
            });
            let ripped = match ripped {
                Ok(ripped) => ripped,
                Err(error) => {
                    self.not_run(&name, &error);
                    self.check_cancel()?;
                    return Ok(unknown(failed, format!("the rip could not run: {error}")));
                }
            };
            self.ended(&name, &ripped.run);
            self.keep_debug_log(&name, ripped.run.started_unix, &out);
            self.check_cancel()?;
            match ripped.outcome {
                RipOutcome::Ripped { .. } => {
                    let used = index;
                    if !self.post_title(key, move |title| {
                        title.attempts += 1;
                        title.rip_index = Some(used);
                    }) {
                        return Err(cancelled());
                    }
                    return Ok(event(Event::TitleRipped));
                }
                RipOutcome::Licence { message } => return Ok(with(Event::LicenceMessage, message)),
                RipOutcome::Failed { failure, message } => return Ok(with(failed(failure), message)),
                RipOutcome::IndexMismatch => {
                    moved += 1;
                    if moved == 3 {
                        return Ok(unknown(failed, format!("the index guard found {key} moved three times")));
                    }
                    index = self.remap(&makemkvcon, &flags, key, planned)?;
                }
            }
        }
    }

    /// After an index mismatch (design part 5.6 step 6): the listing, a rescan saved as `rescan-<n>.txt` (n from 2),
    /// and the index of the rescan's first title that `same_scan_title` matches with the planned title.
    fn remap(&self, program: &Path, flags: &MakemkvFlags, key: &str, planned: &Title) -> Result<u32, StepResult> {
        let failed = |failure| Event::TitleRipFailed { failure };
        let (drive, _) = self.listing(program, failed)?;
        let name = format!("rescan-{}.txt", highest_numbered(&self.job.job_dir, "rescan").max(1) + 1);
        let scanned = self.scan_into(&name, program, flags, drive);
        self.check_cancel()?;
        let text = match scanned {
            Ok(ScanOutcome::Done { text, .. }) => text,
            Ok(ScanOutcome::ProfileRejected { text }) => {
                return Err(unknown(failed, format!("MakeMKV rejected the profile on the rescan: {text}")));
            }
            Ok(ScanOutcome::Failed { failure, message }) => return Err(with(failed(failure), message)),
            Err(error) => return Err(unknown(failed, format!("the rescan could not run: {error}"))),
        };
        let rescanned = disc(&parse(&text)).map_err(|error| unknown(failed, format!("{name}: {error}")))?;
        let found = rescanned.titles.values().find(|title| same_scan_title(title, planned));
        let index = found.map(|title| title.number).ok_or_else(|| {
            unknown(failed, format!("the rescan lists no title matching {key}"))
        })?;
        self.log(&format!("{key}: the index guard found the title moved; {name} lists it as title {index}"));
        Ok(index)
    }

    /// Moves every file directly under `key_dir` into `next_attempt_dir`, so the files under it always describe the
    /// attempt the job kept (design part 7.1).
    fn move_aside(&self, key_dir: &Path) -> Result<(), String> {
        let attempt = next_attempt_dir(key_dir);
        let shown = |path: &Path, error: io::Error| format!("{}: {error}", path.display());
        fs::create_dir_all(&attempt).map_err(|error| shown(&attempt, error))?;
        let entries = fs::read_dir(key_dir).map_err(|error| shown(key_dir, error))?;
        for entry in entries {
            let path = entry.map_err(|error| shown(key_dir, error))?.path();
            if path.is_file() {
                let to = attempt.join(path.file_name().unwrap_or_default());
                fs::rename(&path, &to).map_err(|error| shown(&path, error))?;
            }
        }
        self.log(&format!("the earlier attempt was moved to {}", attempt.display()));
        Ok(())
    }

    /// `<key>/rip-summary.json` (design part 7.1, R23.4, R23.7): written before the verification, from the run's
    /// exit file and the title record RipTitle edited. `started` is `rip_started_text`, after `bytes`; a record without
    /// a start writes no `started` (R23.7, Ruling W16).
    fn write_rip_summary(&self, title: &TitleRecord, rip_dir: &Path, files: &[PathBuf]) {
        let key_dir = self.job.job_dir.join(&title.key);
        let exit = read_json(&key_dir.join("rip.txt.exit.json")).unwrap_or(Value::Null);
        let bytes: u64 = files.iter().filter_map(|file| fs::metadata(file).ok()).map(|meta| meta.len()).sum();
        let names: Vec<String> =
            files.iter().filter_map(|file| file.file_name()).map(|name| name.to_string_lossy().into_owned()).collect();
        let mut summary = json!({
            "ok": true,
            "result": "ok",
            "slug": self.id,
            "label": title.key,
            "title": title.scan_title,
            "profile": self.job.record.profile,
            "bytes": bytes,
        });
        if let Some(started) = rip_started_text(title, self.steps.host.as_ref()) {
            summary["started"] = json!(started);
        }
        summary["elapsed_s"] = json!(exit["elapsed_ms"].as_u64().map(|ms| ms as f64 / 1000.0));
        summary["exit_code"] = exit["exit_code"].clone();
        summary["files"] = json!(names);
        summary["dest"] = json!(rip_dir.display().to_string());
        summary["cache_mb"] = json!(self.job.settings.cache_mb);
        if let Some(index) = title.rip_index.filter(|index| *index != title.scan_title) {
            summary["rip_index"] = json!(index);
        }
        let path = key_dir.join("rip-summary.json");
        let written = serde_json::to_vec_pretty(&summary)
            .map_err(io::Error::from)
            .and_then(|mut text| {
                text.push(b'\n');
                write_atomic(&path, &text)
            });
        if let Err(error) = written {
            self.log(&format!("{}: {error}", path.display()));
        }
    }

    /// VerifyFile in Ripping (design part 5.6 step 7, spec 6.2 step 7): the title folder listed once, rip-summary.json
    /// written from it, `mkvmerge -J` of the title's one file, then `verify_rip` with `Selection::All`.
    /// - A pass gives TitleVerified { last }.
    /// - A mismatch, or a folder without exactly one `.mkv`, gives TitleVerifyFailed { second, last }, counted in the
    ///   title's `verify_failures`; the title's earlier count makes it the second (review round 1, V12 and RR1-13). The
    ///   record handed to the step already holds the earlier failure's posted edit (R22.3).
    /// - A header read that fails (its time limit, an exit code other than 0 or 1, JSON that does not parse, a spawn
    ///   error), a planned scan that cannot be read and a scan title whose streams cannot be selected give
    ///   TitleRipFailed { Unknown } with their sentence, as `cancel` counts the step (design parts 5.3, 9.4; review
    ///   round 1, V9): no comparison was made, so nothing is counted and no re-rip is silent.
    /// - A folder that cannot be listed writes no rip-summary.json and gives TitleRipFailed classed by the share check
    ///   (`unlisted_title_failure`) with `<folder>: <error>`: Output while staging does not answer or is full, else
    ///   Unknown (RR1-7).
    fn verify_rip(&self) -> Outcome {
        let failed = |failure| Event::TitleRipFailed { failure };
        let record = &self.job.record;
        let Some(at) = current_title(record) else {
            return Ok(none("every planned title is verified"));
        };
        let title = &record.titles[at];
        let key = title.key.as_str();
        let last = record.titles[at + 1..].iter().all(is_done);
        let second = title.verify_failures > 0;
        let rip_dir = self.rip_dir(key).ok_or_else(|| none("staging_root is unset"))?;
        let files = match mkv_files(&rip_dir) {
            Ok(files) => files,
            Err(message) => {
                self.log(&format!("{key}/rip-summary.json was not written: {message}"));
                let failure = unlisted_title_failure(self.staging_health());
                return Ok(with(failed(failure), message));
            }
        };
        self.write_rip_summary(title, &rip_dir, &files);
        let Some(mkvmerge) = self.tool(Tool::Mkvmerge) else {
            return Ok(event(Event::ToolMissing { tool: Tool::Mkvmerge }));
        };
        let [file] = files.as_slice() else {
            let message = format!("the title folder holds {} .mkv files, not one", files.len());
            return self.verify_failed(key, second, last, None, message);
        };
        let out = self.job.job_dir.join(key).join("mkvmerge.json");
        let name = format!("{key}/mkvmerge.json");
        let _ = fs::remove_file(with_suffix(&out, ".exit.json"));
        self.started(&name, &mkvmerge, &ident_args(file));
        let read = self.ctx(&no_progress, |ctx| ident(ctx, &mkvmerge, file, &out));
        match &read {
            Ok(_) => self.ended_from_file(&name, &out),
            Err(error) => self.not_run(&name, error),
        }
        self.check_cancel()?;
        let header = match read {
            Ok(Ok(header)) => header,
            Ok(Err(message)) => return Ok(self.unverified(key, "the rip's header read failed", message)),
            Err(error) => {
                let message = format!("mkvmerge -J could not run: {error}");
                return Ok(self.unverified(key, "the rip's header read failed", message));
            }
        };
        let (scan, typed) = match self.planned_scan() {
            Ok(read) => read,
            Err(message) => return Ok(self.unverified(key, "the rip could not be verified", message)),
        };
        let checked = typed.title(title.scan_title).and_then(|planned| {
            let selection = selected_streams(&scan, title.scan_title, Selection::All)?;
            let rip_log = fs::read(self.job.job_dir.join(key).join("rip.txt"))
                .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
                .ok();
            Ok(verify_rip(planned, &header, &selection, rip_log.as_deref(), typed.is_dvd))
        });
        let report = match checked {
            Ok(report) => report,
            Err(message) => return Ok(self.unverified(key, "the rip could not be verified", message)),
        };
        if !report.ok {
            let message = report.mismatches.join("; ");
            return self.verify_failed(key, second, last, Some(report), message);
        }
        let reviews = failure_reviews(&report, false);
        let read_errors = report.read_errors;
        if !self.post_title(key, move |title| {
            title.verified = true;
            title.read_errors = read_errors;
            title.rip_reviews = reviews;
        }) {
            return Err(cancelled());
        }
        Ok(event(Event::TitleVerified { last }))
    }

    /// A rip VerifyFile could not compare (design part 5.3, spec 6.2 step 7; review round 1, V9): the job.log line
    /// `<key>: <what>: <message>` and TitleRipFailed { Unknown } with the message, never counted as a failed
    /// verification.
    fn unverified(&self, key: &str, what: &str, message: String) -> StepResult {
        self.log(&format!("{key}: {what}: {message}"));
        unknown(|failure| Event::TitleRipFailed { failure }, message)
    }

    /// A failed verification of the title `key` (design part 5.6 step 7, spec 6.2 step 7): the job.log line `<key>: the
    /// rip's verification failed: <message>`, and one posted edit of the title that counts it in `verify_failures`
    /// (review round 1, V12 and RR1-13) and, on the second failure, sets the reasons of `failure_reviews(report, true)`
    /// (verify_failed alone when no report was made) and the report's read errors. A refused post is the cancelled
    /// result.
    fn verify_failed(
        &self,
        key: &str,
        second: bool,
        last: bool,
        report: Option<VerifyReport>,
        message: String,
    ) -> Outcome {
        self.check_cancel()?;
        self.log(&format!("{key}: the rip's verification failed: {message}"));
        let kept = second.then(|| {
            let reviews = match &report {
                Some(report) => failure_reviews(report, true),
                None => vec![ReviewReason::VerifyFailed],
            };
            (reviews, report.map(|report| report.read_errors).unwrap_or_default())
        });
        if !self.post_title(key, move |title| {
            title.verify_failures = title.verify_failures.saturating_add(1);
            if let Some((reviews, read_errors)) = kept {
                title.read_errors = read_errors;
                title.rip_reviews = reviews;
            }
        }) {
            return Err(cancelled());
        }
        Ok(with(Event::TitleVerifyFailed { second, last }, message))
    }

    /// Eject (design part 5.6 step 8): Ok gives Ejected; Err gives EjectFailed with its message, also in job.log.
    fn eject(&self) -> Outcome {
        match self.steps.ejector.eject(&self.job.record.drive) {
            Ok(()) => Ok(event(Event::Ejected)),
            Err(message) => {
                self.log(&format!("the eject failed: {message}"));
                Ok(with(Event::EjectFailed, message))
            }
        }
    }

    /// EmptyTitleDirectory (design part 5.2): the staging folder of the first title not done, emptied; no event.
    fn empty_title_directory(&self) -> Outcome {
        let record = &self.job.record;
        let key = current_title(record).map(|at| record.titles[at].key.clone());
        if let Some(dir) = key.and_then(|key| self.rip_dir(&key)) {
            if let Err(error) = empty_dir(&dir) {
                self.log(&format!("{}: {error}", dir.display()));
            }
        }
        Ok(StepResult { event: None, message: None })
    }

    /// The job's plan, or `failed(Unknown)` with `the job has no plan`.
    fn job_plan(&self, failed: fn(Failure) -> Event) -> Result<&Plan, StepResult> {
        self.job.record.plan.as_ref().ok_or_else(|| unknown(failed, "the job has no plan"))
    }

    /// The staged file of the title `key`: the one `.mkv` of its staging folder (design part 5.6 step 6). A folder that
    /// cannot be listed is an Err naming it and the error, which Probe and Remux feed as class Unknown (design part
    /// 5.2, K9; review round 1, V3).
    fn staged_file(&self, key: &str) -> Result<PathBuf, String> {
        let dir = self.rip_dir(key).ok_or_else(|| "staging_root is unset".to_string())?;
        match mkv_files(&dir)?.as_slice() {
            [file] => Ok(file.clone()),
            files => Err(format!("{} holds {} .mkv files, not one", dir.display(), files.len())),
        }
    }

    /// The title `key`'s decision and its final path (design part 5.7 step 11).
    fn decided(&self, key: &str) -> Result<(&FileDecision, &NamedPath), String> {
        let title = self.job.record.titles.iter().find(|title| title.key == key);
        let decision = title.and_then(|title| title.decision.as_ref()).ok_or_else(|| format!("{key} has no decision"))?;
        let path = decision.path.as_ref().ok_or_else(|| format!("{key}'s decision has no final path"))?;
        Ok((decision, path))
    }

    /// The staged file's verification report, the call VerifyFile in Ripping made (design part 5.6 step 7):
    /// `verify_rip` of the scan title against `<key>/mkvmerge.json` with `Selection::All` and the rip log, read from
    /// the job folder so that it survives a restart. Its pairs map each scan stream to the staged file's track ID
    /// (S60).
    fn staged_report(&self, scan: &Scan, typed: &Disc, planned: &PlannedTitle) -> Result<VerifyReport, String> {
        let key_dir = self.job.job_dir.join(&planned.key);
        let path = key_dir.join("mkvmerge.json");
        let bytes = fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        let header: MkvmergeIdent =
            serde_json::from_slice(&bytes).map_err(|error| format!("{}: {error}", path.display()))?;
        let title = typed.title(planned.title)?;
        let selection = selected_streams(scan, planned.title, Selection::All)?;
        let rip_log = read_lossy(&key_dir.join("rip.txt"));
        Ok(verify_rip(title, &header, &selection, rip_log.as_deref(), typed.is_dvd))
    }

    /// Probe (design part 5.7 step 9): `ffprobe` of each planned title's staged file, in plan order, recorded as
    /// `<key>/ffprobe-streams.json` and kept in memory for Decide. No ffprobe gives ToolMissing { Ffprobe }, so the
    /// file phase waits; a probe that fails gives ProbeFailed { Unknown } with its sentence.
    fn probe(&self) -> Outcome {
        let failed = |failure| Event::ProbeFailed { failure };
        let plan = self.job_plan(failed)?;
        let Some(ffprobe) = self.tool(Tool::Ffprobe) else {
            return Ok(event(Event::ToolMissing { tool: Tool::Ffprobe }));
        };
        let mut probed = Vec::new();
        for planned in &plan.titles {
            let key = planned.key.as_str();
            let raw = self.staged_file(key).map_err(|error| unknown(failed, error))?;
            let out = self.job.job_dir.join(key).join("ffprobe-streams.json");
            let name = format!("{key}/ffprobe-streams.json");
            let _ = fs::remove_file(with_suffix(&out, ".exit.json"));
            self.started(&name, &ffprobe, &probe_args(&raw));
            let read = self.ctx(&no_progress, |ctx| probe(ctx, &ffprobe, &raw, &out));
            match &read {
                Ok(_) => self.ended_from_file(&name, &out),
                Err(error) => self.not_run(&name, error),
            }
            self.check_cancel()?;
            match read {
                Ok(Ok(streams)) => probed.push((key.to_string(), streams)),
                Ok(Err(message)) => return Ok(unknown(failed, message)),
                Err(error) => return Ok(unknown(failed, format!("ffprobe could not run: {error}"))),
            }
        }
        let mut probes = lock(&self.steps.probes);
        for (key, streams) in probed {
            probes.insert((self.id.to_string(), key), streams);
        }
        Ok(event(Event::ProbeDone))
    }

    /// ExtractSubtitleEvidence (design part 5.7 steps 10 and 10a, PF-6): neither is built in this version, so it writes
    /// the job.log line `step 10a did not run in this version` (S33) and returns no event; Decide fails a title that
    /// needs step 10 (S32).
    fn extract_subtitle_evidence(&self) -> Outcome {
        self.log(STEP10A_NOT_RUN);
        Ok(StepResult { event: None, message: None })
    }

    /// Whether step 10 runs for the planned title (design part 5.7 step 10): `step10_runs` over the scan title narrowed
    /// to `Selection::All`'s streams, less the streams the rip log's MSG:4047 lines removed.
    fn needs_step10(&self, scan: &Scan, typed: &Disc, planned: &PlannedTitle) -> Result<bool, String> {
        let title = typed.title(planned.title)?;
        let selection = selected_streams(scan, planned.title, Selection::All)?;
        let rip_log = read_lossy(&self.job.job_dir.join(&planned.key).join("rip.txt"));
        Ok(step10_runs(&rip_title(title, &selection), &removed_scan_streams(rip_log.as_deref(), &selection)))
    }

    /// One title's decision document (design parts 5.7 step 11, 6.4): `decide_doc` over `engine_decide_inputs` with the
    /// probe Probe kept, or over `read_back_inputs` when this engine run holds none (PF-27, Ruling S74), which writes
    /// the job.log line `<key>: ` and `READ_BACK_LINE` (design part 5.10), first with no library
    /// facts, then rendered again with the `library_facts` of the path the last render chose while they differ from
    /// the facts it was rendered with (Ruling R24.c2: the facts can lead to an edition-named path, whose own facts may
    /// hold a collision). The film folder does not change with the edition, so this settles at once; facts seen before
    /// end it too. The decision fed is the document's (Ruling R24.3). Err is the reason the inputs, the library facts
    /// (`library_facts`' error, Ruling W22) or the decision could not be made.
    fn title_doc(&self, ctx: &Ctx, plan: &Plan, key: &str) -> Result<DecideDoc, String> {
        let probes = lock(&self.steps.probes).get(&(self.id.to_string(), key.to_string())).cloned();
        let mut inputs = match &probes {
            Some(probes) => engine_decide_inputs(ctx, self.job, plan, key, probes, LibraryFacts::default())?,
            None => {
                let inputs = read_back_inputs(ctx, &self.job.job_dir, plan, key)?;
                self.log(&format!("{key}: {READ_BACK_LINE}"));
                inputs
            }
        };
        let mut seen = vec![inputs.library.clone()];
        loop {
            let doc = decide_doc(&inputs);
            let path = match &doc.decision {
                DecisionEntry::Results(found) => found.decision.path.clone(),
                DecisionEntry::Skipped { skipped } => return Err(skipped.clone()),
            };
            let style = self.job.settings.edition_style;
            let facts = path.map(|path| library_facts(&path, style)).transpose()?.unwrap_or_default();
            if seen.contains(&facts) {
                return Ok(doc);
            }
            seen.push(facts.clone());
            inputs.library = facts;
        }
    }

    /// `<key>/decide.json` (design part 7.1): the document with the audio rule's and lazymkv's versions as its last two
    /// keys, through `dumps` with `INDENT1_ASCII` and a newline, written with write_atomic.
    fn write_decide_json(&self, doc: DecideDoc) -> Result<(), String> {
        let path = self.job.job_dir.join(&doc.label).join("decide.json");
        let shown = |error: &dyn std::fmt::Display| format!("{}: {error}", path.display());
        let version = env!("CARGO_PKG_VERSION").to_string();
        let engine = EngineDecideDoc { doc, audio_rule_version: RULE_VERSION, lazymkv_version: version };
        let value = serde_json::to_value(&engine).map_err(|error| shown(&error))?;
        let text = format!("{}\n", dumps(&value, Style::INDENT1_ASCII));
        write_atomic(&path, text.as_bytes()).map_err(|error| shown(&error))
    }

    /// Decide (design parts 5.7 step 11, 8.4): the record's plan, refreshed from the saved answers while it is
    /// lookup-pending and persisted so (R24.2); each planned title failing with S32's sentence when it needs step 10,
    /// else its `title_doc`; `same_path_reviews` over the titles' decisions, written back into their documents; each
    /// `<key>/decide.json`; the decisions and the colliding paths [ENG-22] recorded; then `decision_done`. A failure
    /// gives DecisionFailed { Unknown } with its sentence.
    ///
    /// Before anything else it asks `reach` of the plan kind's library root, when that root is set, as File does
    /// (Ruling W22, review round 1's V7): a root that does not answer gives DecisionFailed { Unknown } with `the
    /// library share holding <root> does not answer (<why>); the library was not read, and a retry once the share
    /// answers decides again`, so a share that does not answer never reads as an empty library. An unset root asks
    /// nothing, as the namer then gives no path. A library read that fails is DecisionFailed { Unknown } with
    /// `library_facts`' error.
    fn decide(&self) -> Outcome {
        let failed = |failure| Event::DecisionFailed { failure };
        let mut plan = self.job_plan(failed)?.clone();
        let settings = &self.job.settings;
        let library = match plan.kind {
            Kind::Movie => settings.movies_root.as_deref(),
            Kind::Tv => settings.tv_root.as_deref(),
        };
        if let Some(root) = library {
            if let Reach::Unreachable(why) = self.steps.shares.reach(root) {
                return Ok(unknown(
                    failed,
                    format!(
                        "the library share holding {root} does not answer ({why}); the library was not read, and a \
                         retry once the share answers decides again"
                    ),
                ));
            }
        }
        if plan.identity.lookup_pending {
            let job = self.job;
            let refreshed = refreshed_plan(&plan, &job.job_dir, &job.lookups, &job.settings);
            plan = refreshed.map_err(|error| unknown(failed, error))?;
            let refreshed = plan.clone();
            if !self.post_edit(move |record| record.plan = Some(refreshed)) {
                return Err(cancelled());
            }
        }
        let (scan, typed) = self.planned_scan().map_err(|error| unknown(failed, error))?;
        let ctx = Ctx::from_process();
        let mut docs = Vec::new();
        for planned in &plan.titles {
            if self.needs_step10(&scan, &typed, planned).map_err(|error| unknown(failed, error))? {
                return Ok(unknown(failed, STEP10_NOT_BUILT));
            }
            docs.push(self.title_doc(&ctx, &plan, &planned.key).map_err(|error| unknown(failed, error))?);
        }
        let mut decisions = Vec::new();
        for doc in &docs {
            match &doc.decision {
                DecisionEntry::Results(found) => decisions.push(found.decision.clone()),
                DecisionEntry::Skipped { skipped } => return Ok(unknown(failed, skipped.clone())),
            }
        }
        same_path_reviews(&mut decisions);
        for (doc, decision) in docs.iter_mut().zip(&decisions) {
            if let DecisionEntry::Results(found) = &mut doc.decision {
                found.decision = decision.clone();
            }
        }
        self.check_cancel()?;
        for doc in docs {
            self.write_decide_json(doc).map_err(|error| unknown(failed, error))?;
        }
        let collisions: BTreeSet<String> = decisions
            .iter()
            .filter(|decision| decision.reviews.contains(&ReviewReason::PathCollision))
            .filter_map(|decision| decision.path.as_ref().map(|path| path.path.clone()))
            .collect();
        let decided = decisions.clone();
        if !self.post_edit(move |record| {
            for decision in decided {
                if let Some(title) = record.titles.iter_mut().find(|title| title.key == decision.title_key) {
                    title.decision = Some(decision);
                }
            }
            for path in collisions {
                add_collision(record, path);
            }
        }) {
            return Err(cancelled());
        }
        Ok(event(decision_done(&decisions)))
    }

    /// CheckSpace in Decided (design part 5.8, S54): `staged_remux_check` of the staged files' sizes, read before the
    /// `Promises` lock, against the library root of the plan's kind, its result through `space_result`. Sizes that
    /// cannot be read give ShareLost only when staging does not answer, and otherwise the plan's estimate (review round
    /// 1, V3 and RR1-6(b)). An unset library root or `staging_root` gives ShareLost saying so. Its share reads all come
    /// before the check locks the promises (engine.md:584).
    fn remux_space(&self) -> Outcome {
        let Some(plan) = &self.job.record.plan else {
            return Ok(none("the job has no plan"));
        };
        let settings = &self.job.settings;
        let (name, library) = match plan.kind {
            Kind::Movie => ("movies_root", settings.movies_root.as_deref()),
            Kind::Tv => ("tv_root", settings.tv_root.as_deref()),
        };
        let Some(library) = library else {
            return Ok(self.space_result((Event::ShareLost, Some(format!("{name} is unset")))));
        };
        let Some(staging) = settings.staging_root.as_deref() else {
            return Ok(self.space_result((Event::ShareLost, Some("staging_root is unset".to_string()))));
        };
        let staged = staged_bytes(staging, self.id, plan);
        self.check_cancel()?;
        let shares = self.steps.shares.as_ref();
        let checked = staged_remux_check(staged, plan, staging, library, shares, &self.steps.promises, self.id);
        Ok(self.space_result(checked))
    }

    /// Remux (design part 5.7 step 13): for each planned title in plan order, the partial folder of its library root
    /// made when missing (Review Focus 3), then `mkvmerge --gui-mode -o <partial>` of its staged file with the
    /// decision's layout, recorded as `<key>/remux-mkvmerge.txt`, its progress forwarded with the title key. No
    /// mkvmerge gives ToolMissing { Mkvmerge }; a remux that cannot be built or fails gives RemuxFailed { Unknown }
    /// with its sentence, and the staged file stays.
    ///
    /// Once mkvmerge is found, the job's Remux promise becomes `remux_promise` of the staged sizes, read here on the
    /// step's thread (`refine_remux_promise`; engine.md:585, Ruling T3-17-a), when `staging_root` and the plan kind's
    /// library root are set. Each title asks `reach` of its library root before it touches the share, the call that
    /// connects with the stored credential (design part 6.1); a root that does not answer gives RemuxFailed
    /// { Unknown } with `<key>: the library share holding <root> does not answer (<why>); the remux was not started,
    /// and a retry once the share answers runs it`, so a restart into Remuxing writes nothing to a share it has not
    /// reached (engine.md:590, as File does since review round 1's V7).
    fn remux(&self) -> Outcome {
        let failed = |failure| Event::RemuxFailed { failure };
        let plan = self.job_plan(failed)?;
        let Some(mkvmerge) = self.tool(Tool::Mkvmerge) else {
            return Ok(event(Event::ToolMissing { tool: Tool::Mkvmerge }));
        };
        let settings = &self.job.settings;
        let library = match plan.kind {
            Kind::Movie => settings.movies_root.as_deref(),
            Kind::Tv => settings.tv_root.as_deref(),
        };
        if let (Some(staging), Some(library)) = (settings.staging_root.as_deref(), library) {
            let staged = staged_bytes(staging, self.id, plan);
            let (shares, promises) = (self.steps.shares.as_ref(), &self.steps.promises);
            refine_remux_promise(staged, library, shares, promises, self.id, &self.cancel);
        }
        let (scan, typed) = self.planned_scan().map_err(|error| unknown(failed, error))?;
        for planned in &plan.titles {
            let key = planned.key.as_str();
            let (decision, path) = self.decided(key).map_err(|error| unknown(failed, error))?;
            if let Reach::Unreachable(why) = self.steps.shares.reach(&path.root) {
                let root = &path.root;
                return Ok(unknown(
                    failed,
                    format!(
                        "{key}: the library share holding {root} does not answer ({why}); the remux was not started, \
                         and a retry once the share answers runs it"
                    ),
                ));
            }
            let partial = partial_path(&path.root, self.id, key);
            if let Some(folder) = partial.parent() {
                let made = fs::create_dir_all(folder);
                made.map_err(|error| unknown(failed, format!("{}: {error}", folder.display())))?;
            }
            let staged = self.staged_report(&scan, &typed, planned).map_err(|error| unknown(failed, error))?;
            let raw = self.staged_file(key).map_err(|error| unknown(failed, error))?;
            let args = remux_args(&partial, &decision.layout, &staged, &raw).map_err(|error| unknown(failed, error))?;
            let out = self.job.job_dir.join(key).join("remux-mkvmerge.txt");
            let name = format!("{key}/remux-mkvmerge.txt");
            let _ = fs::remove_file(with_suffix(&out, ".exit.json"));
            self.started(&name, &mkvmerge, &args);
            let on_progress = |fraction: f64| (self.progress)(Some(key), fraction);
            let ran = self.ctx(&on_progress, |ctx| remux(ctx, &mkvmerge, &args, &out));
            match &ran {
                Ok(_) => self.ended_from_file(&name, &out),
                Err(error) => self.not_run(&name, error),
            }
            self.check_cancel()?;
            match ran {
                Ok(Ok(())) => {}
                Ok(Err(message)) => return Ok(unknown(failed, message)),
                Err(error) => return Ok(unknown(failed, format!("the remux could not run: {error}"))),
            }
        }
        Ok(event(Event::RemuxDone))
    }

    /// VerifyFile in Remuxing (design part 5.7 step 14): for each planned title, `mkvmerge -J` of its partial file,
    /// recorded as `<key>/partial-mkvmerge.json`, then `verify_final` against the decision's layout, the scan title and
    /// the staged file's report. The first title with a difference gives FileVerifyFailed { Unknown } with its
    /// mismatches joined by "; ", also in job.log; an ident error gives the same with its sentence. When every title
    /// passes, each title's `filed` records its final path and its partial file's size with `mtime_unix` 0, the
    /// verified size that File reads after a restart (RP-7, design part 6.5); File then records the filed file's own.
    fn verify_partial(&self) -> Outcome {
        let failed = |failure| Event::FileVerifyFailed { failure };
        let plan = self.job_plan(failed)?;
        let Some(mkvmerge) = self.tool(Tool::Mkvmerge) else {
            return Ok(event(Event::ToolMissing { tool: Tool::Mkvmerge }));
        };
        let (scan, typed) = self.planned_scan().map_err(|error| unknown(failed, error))?;
        let mut verified = Vec::new();
        for planned in &plan.titles {
            let key = planned.key.as_str();
            let (decision, path) = self.decided(key).map_err(|error| unknown(failed, error))?;
            let partial = partial_path(&path.root, self.id, key);
            let out = self.job.job_dir.join(key).join("partial-mkvmerge.json");
            let name = format!("{key}/partial-mkvmerge.json");
            let _ = fs::remove_file(with_suffix(&out, ".exit.json"));
            self.started(&name, &mkvmerge, &ident_args(&partial));
            let read = self.ctx(&no_progress, |ctx| ident(ctx, &mkvmerge, &partial, &out));
            match &read {
                Ok(_) => self.ended_from_file(&name, &out),
                Err(error) => self.not_run(&name, error),
            }
            self.check_cancel()?;
            let header = match read {
                Ok(Ok(header)) => header,
                Ok(Err(message)) => return Ok(unknown(failed, message)),
                Err(error) => return Ok(unknown(failed, format!("mkvmerge -J could not run: {error}"))),
            };
            let staged = self.staged_report(&scan, &typed, planned).map_err(|error| unknown(failed, error))?;
            let title = typed.title(planned.title).map_err(|error| unknown(failed, error))?;
            let report = verify_final(&decision.layout, title, &staged, &header);
            if !report.ok {
                let message = report.mismatches.join("; ");
                self.log(&format!("{key}: the partial file's verification failed: {message}"));
                return Ok(unknown(failed, message));
            }
            let size = fs::metadata(&partial).map(|meta| meta.len());
            let size = size.map_err(|error| unknown(failed, format!("{}: {error}", partial.display())))?;
            verified.push((key.to_string(), FiledFile { path: path.path.clone(), size, mtime_unix: 0 }));
        }
        if !self.post_edit(move |record| {
            for (key, filed) in verified {
                if let Some(title) = record.titles.iter_mut().find(|title| title.key == key) {
                    title.filed = Some(filed);
                }
            }
        }) {
            return Err(cancelled());
        }
        Ok(event(Event::FileVerified))
    }

    /// File (design parts 5.7 step 14, 6.5): for each planned title in plan order, the final path's missing folders
    /// made, then the partial file renamed into it with `rename_no_replace`, and the filed file's path, size and
    /// modification time recorded. A file already at the final path gives FileFailed { Unknown } with `<final> already
    /// exists; the partial file stays at <partial>`, and the record keeps the colliding path [ENG-22]. An earlier
    /// rename is taken as done, as design part 6.5 says after a restart in Verified, when the partial file is gone and
    /// the final path holds a file of the verified size (the title's `filed` size, RP-7); a partial file that is gone
    /// otherwise gives FileFailed { Unknown }. Off Windows, where `rename_no_replace` is a hard link then a remove
    /// (S13), a rename stopped between the two leaves both names on one file (`one_file`): its partial name is then
    /// removed and the rename taken as done, and a remove that fails after the link gives FileFailed { Unknown } with a
    /// message saying the final path holds the file. On Windows `MoveFileExW` renames in one step, so that half state
    /// cannot arise and those branches are not compiled (Ruling T3-4-b).
    ///
    /// L7 (design part 6.5, Ruling W28): once `create_dir_all` of the final path's folder returns Ok or
    /// `AlreadyExists`, that folder is opened with `fs::read_dir` before the rename. A folder that is present but
    /// cannot be opened, as phase 0's experiment 9 met on the share, gives FileFailed { Unknown } with `<folder> is
    /// present but cannot be opened (<error>); the partial file stays at <partial>, and a retry tries again`, records
    /// no collision, and renames nothing. The folder opened is the title's own, never the library root, so LIB-7
    /// stands. Any other `create_dir_all` error gives FileFailed { Unknown } with `<folder>: <error>`.
    ///
    /// Each title first asks `reach` of its library root, the call that connects with the stored credential (design
    /// part 6.1), and asks again before it reports a partial file gone, because std reads Windows' ERROR_BAD_NETPATH
    /// and ERROR_BAD_NET_NAME as NotFound: only a NotFound on a share that answers counts as a partial file gone or a
    /// final path holding no file. A root that does not answer, or any other metadata error, gives FileFailed
    /// { Unknown } naming the title, the root or path, and the error (design part 5.2 keeps the class; review round
    /// 1, V7 and RR1-9).
    fn file(&self) -> Outcome {
        let failed = |failure| Event::FileFailed { failure };
        let plan = self.job_plan(failed)?;
        for planned in &plan.titles {
            let key = planned.key.as_str();
            let (_, path) = self.decided(key).map_err(|error| unknown(failed, error))?;
            let partial = partial_path(&path.root, self.id, key);
            let target = Path::new(&path.path);
            let unanswered = |why: &str| {
                let (root, stays) = (&path.root, partial.display());
                format!(
                    "{key}: the library share holding {root} does not answer ({why}); the partial file {stays} was not \
                     checked or renamed, and a retry once the share answers files it"
                )
            };
            if let Reach::Unreachable(why) = self.steps.shares.reach(&path.root) {
                return Ok(unknown(failed, unanswered(&why)));
            }
            let present = match fs::metadata(&partial) {
                Ok(_) => true,
                Err(error) if error.kind() == io::ErrorKind::NotFound => false,
                Err(error) => {
                    let shown = partial.display();
                    let message =
                        format!("{key}: {shown}: {error}; the partial file was not checked, so it was not renamed");
                    return Ok(unknown(failed, message));
                }
            };
            if !present {
                let title = self.job.record.titles.iter().find(|title| title.key == key);
                let verified = title.and_then(|title| title.filed.as_ref()).map(|filed| filed.size);
                let found = match fs::metadata(target) {
                    Ok(meta) => Some(meta).filter(|meta| meta.is_file()),
                    Err(error) if error.kind() == io::ErrorKind::NotFound => None,
                    Err(error) => {
                        let (shown, gone) = (&path.path, partial.display());
                        let message = format!(
                            "{key}: {shown}: {error}; the final path was not checked, and the partial file {gone} was \
                             not found"
                        );
                        return Ok(unknown(failed, message));
                    }
                };
                match (found, verified) {
                    (Some(meta), Some(size)) if meta.len() == size => {
                        let shown = &path.path;
                        self.log(&format!("{key}: the partial file is gone and {shown} holds the verified file"));
                        self.record_filed(key, &path.path)?;
                        continue;
                    }
                    (Some(_), _) => {
                        let collision = path.path.clone();
                        if !self.post_edit(move |record| add_collision(record, collision)) {
                            return Err(cancelled());
                        }
                        let message = format!(
                            "{} already exists and is not the verified file; the partial file {} is gone",
                            path.path,
                            partial.display()
                        );
                        return Ok(unknown(failed, message));
                    }
                    (None, _) => {
                        // A share that stopped answering since the first `reach` reports a missing path as NotFound.
                        if let Reach::Unreachable(why) = self.steps.shares.reach(&path.root) {
                            return Ok(unknown(failed, unanswered(&why)));
                        }
                        let (gone, shown) = (partial.display(), &path.path);
                        let message = format!("the partial file {gone} is gone and {shown} holds no file");
                        return Ok(unknown(failed, message));
                    }
                }
            }
            if let Some(folder) = target.parent() {
                match fs::create_dir_all(folder) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(error) => return Ok(unknown(failed, format!("{}: {error}", folder.display()))),
                }
                // L7 (Ruling W28): a folder that is present but cannot be opened stops the rename.
                if let Err(error) = fs::read_dir(folder) {
                    let (shown, stays) = (folder.display(), partial.display());
                    return Ok(unknown(
                        failed,
                        format!(
                            "{shown} is present but cannot be opened ({error}); the partial file stays at {stays}, \
                             and a retry tries again"
                        ),
                    ));
                }
            }
            self.check_cancel()?;
            match rename_no_replace(&partial, target) {
                Ok(()) => {}
                #[cfg(not(windows))]
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists && one_file(&partial, target) => {
                    // Design part 6.5: an earlier rename stopped between its hard link and its remove.
                    if let Err(error) = fs::remove_file(&partial) {
                        return Ok(unknown(failed, half_done(&partial, &path.path, &error)));
                    }
                    let shown = &path.path;
                    self.log(&format!("{key}: {shown} already held the partial file, so the rename was finished"));
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    let collision = path.path.clone();
                    if !self.post_edit(move |record| add_collision(record, collision)) {
                        return Err(cancelled());
                    }
                    let (shown, stays) = (&path.path, partial.display());
                    return Ok(unknown(failed, format!("{shown} already exists; the partial file stays at {stays}")));
                }
                #[cfg(not(windows))]
                Err(error) if one_file(&partial, target) => {
                    // The hard link was made and only the remove of the partial name failed.
                    return Ok(unknown(failed, half_done(&partial, &path.path, &error)));
                }
                Err(error) => {
                    let message = format!("{} could not be renamed to {}: {error}", partial.display(), path.path);
                    return Ok(unknown(failed, message));
                }
            }
            self.record_filed(key, &path.path)?;
        }
        Ok(event(Event::FileDone))
    }

    /// Records the file at `path` as the title `key`'s filed file: its path, size and modification time (design part
    /// 6.5). Err is FileFailed { Unknown } when it cannot be read, or the cancelled result.
    fn record_filed(&self, key: &str, path: &str) -> Result<(), StepResult> {
        let failed = |failure| Event::FileFailed { failure };
        let meta = fs::metadata(path).map_err(|error| unknown(failed, format!("{path}: {error}")))?;
        let modified = meta.modified().ok().and_then(|time| time.duration_since(UNIX_EPOCH).ok());
        let mtime_unix = modified.map_or(0, |at| at.as_secs());
        let filed = FiledFile { path: path.to_string(), size: meta.len(), mtime_unix };
        if !self.post_title(key, move |title| title.filed = Some(filed)) {
            return Err(cancelled());
        }
        Ok(())
    }

    /// DeletePartialFile (design part 5.2; 10.1 "Resuming after a restart"): each decided title's partial file removed
    /// before the remux runs again, a missing one ignored and any other error logged; no event. Each title first asks
    /// `reach` of its library root (engine.md:590, design part 6.1); one that does not answer is skipped with the
    /// job.log line `<key>: the library share holding <root> does not answer (<why>); the partial file <partial> was
    /// not deleted`, and the Remux after it fails naming the share.
    fn delete_partial_file(&self) -> Outcome {
        let keys = self.job.record.plan.iter().flat_map(|plan| plan.titles.iter().map(|planned| planned.key.as_str()));
        for key in keys {
            let Ok((_, path)) = self.decided(key) else {
                continue;
            };
            let partial = partial_path(&path.root, self.id, key);
            if let Reach::Unreachable(why) = self.steps.shares.reach(&path.root) {
                let (root, kept) = (&path.root, partial.display());
                self.log(&format!(
                    "{key}: the library share holding {root} does not answer ({why}); the partial file {kept} was not \
                     deleted"
                ));
                continue;
            }
            match fs::remove_file(&partial) {
                Err(error) if error.kind() != io::ErrorKind::NotFound => {
                    self.log(&format!("{}: {error}", partial.display()));
                }
                _ => {}
            }
        }
        Ok(StepResult { event: None, message: None })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lazymkv_core::testsupport::{default_profile_xml, ustar_build, Scratch, ScratchExt};

    #[test]
    fn version_of_reads_the_banner_or_a_bare_version() {
        assert_eq!(version_of("MakeMKV v2.0.0 win(x64-release)"), Some((2, 0, 0)));
        assert_eq!(version_of("1.17.0"), Some((1, 17, 0)));
        assert_eq!(version_of("MakeMKV v2.0 beta"), None);
    }

    #[test]
    fn the_job_profile_is_written_from_appdata_tar_with_a_job_unique_name() {
        let s = Scratch::new("profile");
        s.write("tools/appdata.tar", ustar_build(&[("data/default.mmcp.xml", default_profile_xml().as_bytes())]));
        let written = write_job_profile(&s.0.join("tools/makemkvcon64"), &s.0.join("job"), "abc-1", ALL_TRACKS_SELECTION, JOB_PROFILE_FILE).unwrap();
        assert_eq!(written, s.0.join("job/job.mmcp.xml"));
        let xml = std::fs::read_to_string(written).unwrap();
        assert!(xml.contains("<name lang=\"eng\">lazymkv-abc-1</name>"));
        assert_eq!(xml.matches("defaultSelection=\"+sel:all,-sel:mvcvideo,=100:all\"").count(), 2);
    }

    #[test]
    fn a_missing_appdata_tar_is_an_error_naming_it() {
        let s = Scratch::new("no-tar");
        let error = write_job_profile(&s.0.join("makemkvcon64"), &s.0.join("job"), "abc-1", ALL_TRACKS_SELECTION, JOB_PROFILE_FILE).unwrap_err();
        assert!(error.contains("appdata.tar"), "{error}");
    }

    #[test]
    fn the_attempt_folder_is_numbered_after_the_last() {
        let s = Scratch::new("attempts");
        assert_eq!(next_attempt_dir(&s.0), s.0.join("attempt-1"));
        s.write("attempt-1/rip.txt", "");
        assert_eq!(next_attempt_dir(&s.0), s.0.join("attempt-2"));
    }

    /// Review round 1, V3: only the regular files whose extension is `mkv` in any ASCII case, sorted.
    #[test]
    fn mkv_files_lists_the_regular_mkv_files_sorted_in_any_case() {
        let s = Scratch::new("mkv-files");
        s.write("b.mkv", "b");
        s.write("A.MKV", "a");
        s.write("c.txt", "c");
        fs::create_dir_all(s.0.join("d.mkv")).unwrap();
        assert_eq!(mkv_files(&s.0), Ok(vec![s.0.join("A.MKV"), s.0.join("b.mkv")]));
    }

    /// Review round 1, V3 (RR1-6(a)): every caller runs after RipTitle made the folder, so a missing one is an error
    /// naming it, not an empty list.
    #[test]
    fn a_missing_title_folder_is_an_error_naming_it() {
        let s = Scratch::new("mkv-files-missing");
        let dir = s.0.join("t00-00800");
        let error = mkv_files(&dir).unwrap_err();
        assert!(error.starts_with(&format!("{}: ", dir.display())), "{error}");
    }

    /// Review round 1, V3: a path that is a regular file cannot be listed, on every OS, and the error names it.
    #[test]
    fn a_title_folder_that_is_a_file_is_an_error_naming_it() {
        let s = Scratch::new("mkv-files-file");
        s.write("t00-00800", "");
        let dir = s.0.join("t00-00800");
        let error = mkv_files(&dir).unwrap_err();
        assert!(error.starts_with(&format!("{}: ", dir.display())), "{error}");
    }

    /// Review round 1, V5 (RR1-8): a staging folder that cannot be emptied is a write error on staging, so its class is
    /// Output whatever the share check says, never Unknown or MediaReadError.
    #[test]
    fn a_title_folder_that_cannot_be_emptied_is_classed_by_the_share_check() {
        use lazymkv_core::job::OutputCause;
        let output = |cause| Failure { class: FailureClass::Output, output: Some(cause) };
        let classed = |share_healthy, out_of_space| staging_write_failure(StagingHealth { share_healthy, out_of_space });
        assert_eq!(classed(false, false), output(OutputCause::ShareUnreachable));
        assert_eq!(classed(false, true), output(OutputCause::ShareUnreachable));
        assert_eq!(classed(true, true), output(OutputCause::OutOfSpace));
        assert_eq!(classed(true, false), output(OutputCause::WriteError));
    }

    /// The final review of review round 1 (C1, RR1-7): a title folder VerifyFile cannot list is classed by the share
    /// check with no write error, so a healthy staging fails Unknown rather than cycling through AwaitingShare.
    #[test]
    fn a_title_folder_verify_cannot_list_is_unknown_on_a_healthy_staging() {
        use lazymkv_core::job::OutputCause;
        let output = |cause| Failure { class: FailureClass::Output, output: Some(cause) };
        let classed = |share_healthy, out_of_space| unlisted_title_failure(StagingHealth { share_healthy, out_of_space });
        assert_eq!(classed(true, false), Failure { class: FailureClass::Unknown, output: None });
        assert_eq!(classed(false, false), output(OutputCause::ShareUnreachable));
        assert_eq!(classed(false, true), output(OutputCause::ShareUnreachable));
        assert_eq!(classed(true, true), output(OutputCause::OutOfSpace));
    }

    /// Ruling W16: `started` is the record's start, not the clock at VerifyFile.
    #[test]
    fn started_is_the_records_rip_start_not_the_clock() {
        let host = lazymkv_platform::fakes::FakeHost::new("2026-10-23", 1_792_800_000);
        let mut title = TitleRecord {
            key: "t00-00800".to_string(), scan_title: 0, rip_index: None, attempts: 1, verified: false,
            verify_failures: 0, read_errors: Vec::new(), rip_reviews: Vec::new(), decision: None, filed: None,
            rip_started_ms: Some(1_700_000_000_123),
        };
        assert_eq!(rip_started_text(&title, &host).as_deref(), Some("2023-11-14 22:13:20.123"));
        title.rip_started_ms = None;
        assert_eq!(rip_started_text(&title, &host), None);
    }

    /// A synthetic disc under `s` as the harness builds one: an index, a playlist and a stream of `stream` zero bytes.
    fn disc_volume(s: &Scratch, name: &str, stream: usize) -> PathBuf {
        s.write(&format!("{name}/BDMV/index.bdmv"), "INDX0200");
        s.write(&format!("{name}/BDMV/PLAYLIST/00800.mpls"), "MPLS0200");
        s.write(&format!("{name}/BDMV/STREAM/00001.m2ts"), vec![0u8; stream]);
        s.0.join(name)
    }

    /// The job folder's snapshot, read whole.
    fn snapshot_files(job: &Path) -> Vec<Vec<u8>> {
        ["files.json", "meta.json", "nav/BDMV/index.bdmv", "nav/BDMV/PLAYLIST/00800.mpls"]
            .iter()
            .map(|file| fs::read(job.join(file)).unwrap())
            .collect()
    }

    /// engine.md:587 (V10): a different disc in the job's drive leaves the job's nav/, files.json and meta.json as they
    /// were, and no fresh snapshot behind.
    #[test]
    fn a_different_disc_leaves_the_jobs_snapshot_unchanged() {
        let s = Scratch::new("aside-differs");
        let job = s.0.join("job");
        let fingerprint = snapshot(&disc_volume(&s, "a", 1000), "A", &job, 1).unwrap().fingerprint.unwrap();
        let before = snapshot_files(&job);
        let error = snapshot_aside(&disc_volume(&s, "b", 2000), "B", &job, 2, &fingerprint).unwrap_err();
        let tail = format!(" differs from the job's {fingerprint}");
        assert!(error.starts_with("the disc's fingerprint ") && error.ends_with(&tail), "{error}");
        assert_eq!(snapshot_files(&job), before);
        assert!(!job.join(FRESH_SNAPSHOT).exists());
    }

    #[test]
    fn a_matching_disc_swaps_in_the_new_snapshot() {
        let s = Scratch::new("aside-matches");
        let job = s.0.join("job");
        let disc = disc_volume(&s, "a", 1000);
        let fingerprint = snapshot(&disc, "A", &job, 1).unwrap().fingerprint.unwrap();
        fs::write(job.join("nav/BDMV/stale.bdmv"), b"an older snapshot's file").unwrap();
        snapshot_aside(&disc, "A", &job, 2, &fingerprint).unwrap();
        let meta: Value = serde_json::from_slice(&fs::read(job.join("meta.json")).unwrap()).unwrap();
        assert_eq!(meta["captured_unix"], json!(2));
        assert!(job.join("nav/BDMV/index.bdmv").is_file());
        assert!(!job.join("nav/BDMV/stale.bdmv").exists());
        assert!(!job.join(FRESH_SNAPSHOT).exists());
    }

    #[test]
    fn a_fresh_snapshot_a_crash_left_is_replaced() {
        let s = Scratch::new("aside-leftover");
        let job = s.0.join("job");
        let disc = disc_volume(&s, "a", 1000);
        let fingerprint = snapshot(&disc, "A", &job, 1).unwrap().fingerprint.unwrap();
        s.write(&format!("job/{FRESH_SNAPSHOT}/nav/BDMV/left.bdmv"), "left by a crash");
        snapshot_aside(&disc, "A", &job, 2, &fingerprint).unwrap();
        assert!(!job.join(FRESH_SNAPSHOT).exists());
        assert!(!job.join("nav/BDMV/left.bdmv").exists());
    }

    #[test]
    fn a_disc_that_cannot_be_walked_leaves_the_jobs_snapshot_unchanged() {
        let s = Scratch::new("aside-unwalked");
        let job = s.0.join("job");
        let fingerprint = snapshot(&disc_volume(&s, "a", 1000), "A", &job, 1).unwrap().fingerprint.unwrap();
        let before = snapshot_files(&job);
        let error = snapshot_aside(&s.0.join("absent"), "", &job, 2, &fingerprint).unwrap_err();
        assert!(error.starts_with("the disc's recording could not be written: "), "{error}");
        assert_eq!(snapshot_files(&job), before);
        assert!(!job.join(FRESH_SNAPSHOT).exists());
    }
}
