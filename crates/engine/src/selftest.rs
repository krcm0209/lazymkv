//! The self-test (design parts 3.7, 9.4; Rulings S53, S79 to S81, RP-9): the MakeMKV check with the (version,
//! licence code) it records across restarts, each tool's path, version and where it was found, the write, rename and
//! delete probe of the three roots, the TMDb token's check, and what a `retry` re-checks for a waiting job. Every run
//! is recorded capture-style under `<data>/selftest/` (design part 7.2). The owner thread runs none of this itself: it
//! hands each self-test and re-check to a thread of its own (design part 4.4).

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use lazymkv_clients::{authentication_url, user_agent, HttpTransport, TransportError};
use lazymkv_core::ipc::{OutputStream, SelfTest, SelfTestCheck};
use lazymkv_core::job::{Event, State, Tool};
use lazymkv_core::plan::Kind;
use lazymkv_core::record::JobRecord;
use lazymkv_core::settings::{version_triple, Settings};
use lazymkv_platform::{rename_no_replace, write_atomic, Host, ProcessGuard, Reach, SecretStore, ShareConnector};
use lazymkv_tools::{
    find_tool, list_drives, run_recorded, save_debug_log, with_suffix, CancelToken, Found, LineAction, Limits, ToolCtx,
    ToolLocation,
};
use serde_json::{json, Value};

use crate::file_phase::PARTIAL_FOLDER;
use crate::space::{planned_remux_promise, rip_check, rip_promise, staged_bytes, staged_remux_check, Promises};
use crate::steps::version_of;

/// The folder of the data folder the self-test records into (design part 7.2, S79).
const SELFTEST_DIR: &str = "selftest";
/// The file the recorded (version, licence code) lives in (RP-9).
const RECORDED_FILE: &str = "makemkv-check.json";
/// The MakeMKV check's run file outside a job (S79).
const DRIVE_LIST_FILE: &str = "drive-list.txt";
/// The root probe's file, and its name after the rename (spec 7).
const PROBE_FILE: &str = "lazymkv-selftest.probe";
const PROBE_RENAMED: &str = "lazymkv-selftest.probe.renamed";
/// The detail of a check that has not run since the engine started (S80).
const NOT_CHECKED: &str = "not checked since the engine started";

/// Locks `mutex`, ignoring a poison a panicked thread left.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// MakeMKV's last check and the (version, licence code) it recorded (design part 3.7), kept in `file` across restarts
/// (RP-9). Def: T26. Use: T26.
pub(crate) struct MakeMkvState {
    /// The last MakeMKV check outside a job, None until one ran.
    pub last: Option<SelfTestCheck>,
    /// The (version, licence code) the last listing recorded, None before the first.
    pub recorded: Option<(Option<String>, Option<u32>)>,
    /// `<data>/selftest/makemkv-check.json`, or None to keep `recorded` in memory only.
    pub file: Option<PathBuf>,
}

impl MakeMkvState {
    /// `file` is `<data>/selftest/makemkv-check.json`; `recorded` is read from it, `{"version":..,"licence":..}`,
    /// and stays None when it is absent or unreadable. Def: T26. Use: T26.
    pub(crate) fn open(data: &Path) -> MakeMkvState {
        let file = data.join(SELFTEST_DIR).join(RECORDED_FILE);
        let recorded = fs::read(&file).ok().and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok()).and_then(
            |value| {
                let version = match &value["version"] {
                    Value::String(version) => Some(version.clone()),
                    Value::Null => None,
                    _ => return None,
                };
                let licence = match &value["licence"] {
                    Value::Null => None,
                    other => Some(u32::try_from(other.as_u64()?).ok()?),
                };
                Some((version, licence))
            },
        );
        MakeMkvState { last: None, recorded, file: Some(file) }
    }

    /// True when (version, licence) differs from a recorded one; records it, and rewrites `file` with write_atomic
    /// whenever `recorded` changes. A file that cannot be written keeps the new state in memory. Def: T26. Use: T26.
    pub(crate) fn observe(&mut self, version: Option<String>, licence: Option<u32>) -> bool {
        let seen = (version, licence);
        if self.recorded.as_ref() == Some(&seen) {
            return false;
        }
        let changed = self.recorded.is_some();
        if let Some(file) = &self.file {
            let bytes = serde_json::to_vec(&json!({"version": seen.0, "licence": seen.1}));
            let parent = file.parent().unwrap_or_else(|| Path::new(""));
            let _ = bytes.map_err(io::Error::from).and_then(|bytes| {
                fs::create_dir_all(parent)?;
                write_atomic(file, &bytes)
            });
        }
        self.recorded = Some(seen);
        changed
    }
}

/// The engine's one `MakeMkvState` (S79), shared by the owner, the checks and every job's drive listing, with the flag
/// a changed (version, licence code) raises until the owner takes it and delivers MakeMkvChanged (design part 3.7).
/// Def: T26. Use: T26.
pub(crate) struct MakeMkv {
    state: Mutex<MakeMkvState>,
    changed: AtomicBool,
}

impl MakeMkv {
    /// The shared state over `state` (S79). Def: T26. Use: T26.
    pub(crate) fn new(state: MakeMkvState) -> MakeMkv {
        MakeMkv { state: Mutex::new(state), changed: AtomicBool::new(false) }
    }

    /// `MakeMkvState::observe` of a listing that printed a version; a change raises the flag (design part 3.7, S79). A
    /// listing with no version (a `makemkvcon` that started and died) says nothing about MakeMKV and is not observed,
    /// so it raises no change and leaves the recorded check alone (ruling at the final review). Def: T26. Use: T26
    /// (with every job's drive listing in steps.rs).
    pub(crate) fn observe(&self, version: Option<String>, licence: Option<u32>) {
        if version.is_some() && lock(&self.state).observe(version, licence) {
            self.changed.store(true, Ordering::SeqCst);
        }
    }

    /// Whether a change was observed since the last call, lowering the flag (design part 3.7). Def: T26. Use: T26.
    pub(crate) fn take_changed(&self) -> bool {
        self.changed.swap(false, Ordering::SeqCst)
    }

    /// The last check marked busy, for a check that comes while a `makemkvcon` runs (design part 3.7, S80); without
    /// one, a busy check that is not ok, with `why`.
    fn busy(&self, why: &str) -> SelfTestCheck {
        let last = lock(&self.state).last.clone();
        let mut check = last.unwrap_or_else(|| check(Tool::Makemkvcon.as_str(), false, why.to_string()));
        check.busy = true;
        check
    }

    /// Keeps `check` as the last MakeMKV check.
    fn set_last(&self, check: &SelfTestCheck) {
        lock(&self.state).last = Some(check.clone());
    }
}

/// A check with no path, version or place, not busy.
fn check(name: &str, ok: bool, detail: String) -> SelfTestCheck {
    SelfTestCheck { name: name.to_string(), ok, detail, path: None, version: None, found: None, busy: false }
}

/// The check of `name` before it ran (S80): not ok, not busy, every optional field None. Def: T26. Use: T26.
pub(crate) fn not_checked(name: &str) -> SelfTestCheck {
    check(name, false, NOT_CHECKED.to_string())
}

/// The self-test a start shows until the first `run_selftest` (S80): `makemkv`, the check at start, then the other
/// eight checks `not_checked`, in S80's order. Def: T26. Use: T26.
pub(crate) fn first_selftest(makemkv: SelfTestCheck) -> SelfTest {
    let others = ["mkvmerge", "ffprobe", "ffmpeg", "java", "staging_root", "tuning_root", "movies_root", "tmdb_token"];
    SelfTest { checks: std::iter::once(makemkv).chain(others.into_iter().map(not_checked)).collect() }
}

/// The serde spelling of `found`, as the check's `found` shows it (design part 9.2).
fn found_text(found: Found) -> Option<String> {
    serde_json::to_value(found).ok().and_then(|value| value.as_str().map(str::to_string))
}

/// Whether a MSG:1005 version is below `minimum` (design part 3.7, spec 9.1); an unreadable one is not. Def: T26.
/// Use: T26.
pub(crate) fn below_minimum(version: &str, minimum: &str) -> bool {
    matches!((version_of(version), version_triple(minimum)), (Some(have), Some(need)) if have < need)
}

/// The version a tool prints (design part 9.2): the word after "mkvmerge v", "ffprobe version " or "ffmpeg version "
/// on stdout's first line, or for java the text between the first two `"` of stderr's first line; None otherwise.
/// Def: T26. Use: T26.
pub(crate) fn version_in(tool: Tool, stdout: &str, stderr: &str) -> Option<String> {
    let first = |text: &str| text.lines().next().unwrap_or_default().to_string();
    let after = |line: String, marker: &str| {
        let rest = &line[line.find(marker)? + marker.len()..];
        let word = rest.split_whitespace().next()?;
        Some(word.to_string())
    };
    match tool {
        Tool::Mkvmerge => after(first(stdout), "mkvmerge v"),
        Tool::Ffprobe => after(first(stdout), "ffprobe version "),
        Tool::Ffmpeg => after(first(stdout), "ffmpeg version "),
        Tool::Java => {
            let line = first(stderr);
            let mut parts = line.split('"').skip(1);
            let inside = parts.next()?;
            parts.next().map(|_| inside.to_string())
        }
        Tool::Makemkvcon => None,
    }
}

/// Runs `<program> --version` (mkvmerge) or `-version` (ffprobe, ffmpeg, java) under `limits.header`, recorded at
/// `out` (`<data>/selftest/<tool>-version.txt`, design part 7.2), and reads it with `version_in`. Def: T26. Use: T26.
pub(crate) fn tool_version(ctx: &ToolCtx, tool: Tool, program: &Path, out: &Path) -> Option<String> {
    let flag = if tool == Tool::Mkvmerge { "--version" } else { "-version" };
    let run = run_recorded(ctx, program, &[flag.to_string()], out, Some(ctx.limits.header), false, &mut |_| {
        LineAction::Continue
    })
    .ok()?;
    let stdout = String::from_utf8_lossy(&run.stdout).into_owned();
    let stderr = fs::read(with_suffix(out, ".stderr")).map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
    version_in(tool, &stdout, &stderr.unwrap_or_default())
}

/// The write, rename and delete probe of spec 7 in `dir`: `lazymkv-selftest.probe` written, renamed with
/// `rename_no_replace` and deleted; Err is the sentence of the step that failed.
fn probe(dir: &Path) -> Result<(), String> {
    let at = |path: &Path, error: io::Error| format!("{}: {error}", path.display());
    let (probe, renamed) = (dir.join(PROBE_FILE), dir.join(PROBE_RENAMED));
    let _ = fs::remove_file(&renamed);
    fs::write(&probe, b"lazymkv self-test\n").map_err(|error| at(&probe, error))?;
    if let Err(error) = rename_no_replace(&probe, &renamed) {
        let _ = fs::remove_file(&probe);
        return Err(at(&renamed, error));
    }
    fs::remove_file(&renamed).map_err(|error| at(&renamed, error))
}

/// The write, rename and delete probe of spec 7 in `root` (in `<root>/.lazymkv-partial` when `library`). A library
/// root that is not a folder fails with `<root> is not a folder`, and only the partial folder inside an existing root
/// is made, so a mistyped or unmounted root never passes or leaves a folder behind (ruling at the final review).
/// Def: T26. Use: T26.
pub(crate) fn root_check(name: &str, root: Option<&str>, library: bool) -> SelfTestCheck {
    let Some(root) = root else {
        return check(name, false, format!("{name} is unset"));
    };
    let dir = if library { Path::new(root).join(PARTIAL_FOLDER) } else { PathBuf::from(root) };
    let made = if !library {
        Ok(())
    } else if !Path::new(root).is_dir() {
        Err(format!("{root} is not a folder"))
    } else {
        match fs::create_dir(&dir) {
            Err(error) if error.kind() != io::ErrorKind::AlreadyExists => Err(format!("{}: {error}", dir.display())),
            _ => Ok(()),
        }
    };
    let (ok, detail) = match made.and_then(|()| probe(&dir)) {
        Ok(()) => (true, format!("{} takes a write, a rename and a delete", dir.display())),
        Err(error) => (false, error),
    };
    SelfTestCheck { path: Some(root.to_string()), ..check(name, ok, detail) }
}

/// The words of a transport error, for a check's detail.
fn transport_text(error: &TransportError) -> String {
    match error {
        TransportError::Timeout => "no answer came in time".to_string(),
        TransportError::Unreachable(why) => why.clone(),
        TransportError::Unrecorded(url) => format!("no answer is recorded for {url}"),
    }
}

/// GET authentication_url() with the bearer token; ok on 200 (design part 9.4, S53). Def: T26. Use: T26.
pub(crate) fn token_check(http: &dyn HttpTransport, token: Option<&str>) -> SelfTestCheck {
    const NAME: &str = "tmdb_token";
    let Some(token) = token else {
        return check(NAME, false, "no tmdb_token is set".to_string());
    };
    let (bearer, agent) = (format!("Bearer {token}"), user_agent());
    match http.get(&authentication_url(), &[("Authorization", &bearer), ("User-Agent", &agent)]) {
        Ok(response) if response.status == 200 => check(NAME, true, "TMDb answered 200 to the token".to_string()),
        Ok(response) => check(NAME, false, format!("TMDb answered {} to the token", response.status)),
        Err(error) => check(NAME, false, format!("TMDb could not be asked: {}", transport_text(&error))),
    }
}

/// A waiting job as a `retry` re-checks it (Ruling S81): its record, its own settings (ENG-19) and its folder.
/// Def: T26. Use: T26.
pub(crate) struct RecheckJob {
    /// The job's record when the retry came.
    pub(crate) record: JobRecord,
    /// The settings the job started with.
    pub(crate) settings: Settings,
    /// `<data>/jobs/<id>`.
    pub(crate) job_dir: PathBuf,
}

/// What the self-test and the re-checks run with (design parts 3.6, 3.7, 9.4): the tool lookup's places, the limits,
/// the platform parts, the transport, the secrets, the last scan's Java use and the shared MakeMKV state. Def: T26.
/// Use: T26.
#[derive(Clone)]
pub(crate) struct CheckEnv {
    /// The data folder; the self-test records into `<data>/selftest/`.
    pub(crate) data: PathBuf,
    /// The resources folder the tool lookup reads second (design part 3.6).
    pub(crate) resources: Option<PathBuf>,
    /// The `PATH` the tool lookup reads last (design part 3.6).
    pub(crate) path_var: Option<OsString>,
    /// Limits that replace `Limits::from_settings`, for tests only.
    pub(crate) limits_override: Option<Limits>,
    /// Starts the tools the checks run.
    pub(crate) processes: Arc<dyn ProcessGuard>,
    /// The machine, for the tool lookup and the runs' clock.
    pub(crate) host: Arc<dyn Host>,
    /// The transport the token check asks TMDb over (S53).
    pub(crate) http: Arc<dyn HttpTransport>,
    /// The secrets, for the `tmdb_token` the token check sends.
    pub(crate) secrets: Arc<dyn SecretStore>,
    /// The last scan's MSG:3344 verdict, None before the first scan (design part 9.4).
    pub(crate) java_in_use: Arc<Mutex<Option<bool>>>,
    /// The shared MakeMKV state (S79).
    pub(crate) makemkv: Arc<MakeMkv>,
    /// Stops the check's tool runs: the owner gives each check a token of its own and raises it at a stop or a forced
    /// shutdown, because a drive listing has no time limit (design parts 3.8, 5.3; S57).
    pub(crate) cancel: CancelToken,
}

impl CheckEnv {
    /// `find_tool` of `tool` (design part 3.6), when its path is a file: a tool named in the settings but not there
    /// counts as not found (design part 3.7). Def: T26. Use: T26.
    pub(crate) fn locate(&self, tool: Tool, settings: &Settings) -> Option<ToolLocation> {
        let (resources, path_var) = (self.resources.as_deref(), self.path_var.as_deref());
        find_tool(tool, settings, resources, self.host.as_ref(), path_var).filter(|found| found.path.is_file())
    }

    /// Runs `call` with a ToolCtx over the guard and host, the limits, the check's cancel token and no callbacks.
    fn with_ctx<R>(&self, settings: &Settings, call: impl FnOnce(&ToolCtx) -> R) -> R {
        let limits = self.limits_override.unwrap_or_else(|| Limits::from_settings(settings));
        let on_output = |_: OutputStream, _: &str| {};
        let on_progress = |_: f64| {};
        let ctx = ToolCtx {
            guard: self.processes.as_ref(),
            host: self.host.as_ref(),
            limits: &limits,
            cancel: self.cancel.clone(),
            on_output: &on_output,
            on_progress: &on_progress,
        };
        call(&ctx)
    }

    /// `<data>/selftest/<name>`, its folder made.
    fn record_path(&self, name: &str) -> io::Result<PathBuf> {
        let dir = self.data.join(SELFTEST_DIR);
        fs::create_dir_all(&dir)?;
        Ok(dir.join(name))
    }

    /// The MakeMKV check (design part 3.7, S79, S80): while a `makemkvcon` runs (`busy`, or another MakeMKV the
    /// listing finds running) the last check marked busy, with no run; otherwise the drive listing recorded at
    /// `<data>/selftest/drive-list.txt`, its (version, licence code) observed. It is ok when `makemkvcon` is found,
    /// prints a version of at least `makemkv_min_version` and no licence message. The check becomes the last one. A
    /// listing the check's cancel token ended is not observed and does not become the last check. Def: T26. Use: T26.
    pub(crate) fn makemkv_check(&self, settings: &Settings, busy: bool) -> SelfTestCheck {
        let name = Tool::Makemkvcon.as_str();
        if busy {
            return self.makemkv.busy("a makemkvcon runs for a job and no check ran before it");
        }
        let Some(location) = self.locate(Tool::Makemkvcon, settings) else {
            let missing = check(name, false, format!("{name} was not found"));
            self.makemkv.set_last(&missing);
            return missing;
        };
        let debug = settings.keep_tuning_evidence;
        let listed = self.record_path(DRIVE_LIST_FILE).and_then(|out| {
            let listing = self.with_ctx(settings, |ctx| list_drives(ctx, &location.path, debug, "", &out))?;
            if debug {
                let _ = save_debug_log(self.host.as_ref(), listing.run.started_unix, &out);
            }
            Ok(listing)
        });
        let (ok, detail, version) = match listed {
            Err(error) if error.kind() == io::ErrorKind::ResourceBusy => return self.makemkv.busy(&error.to_string()),
            Err(error) => (false, format!("the drive listing could not run: {error}"), None),
            Ok(listing) if listing.run.cancelled => {
                let cancelled = check(name, false, "the drive listing was cancelled".to_string());
                return SelfTestCheck { path: Some(location.path.display().to_string()), ..cancelled };
            }
            Ok(listing) => {
                let summary = listing.summary;
                self.makemkv.observe(summary.version.clone(), summary.licence.as_ref().map(|message| message.code));
                let minimum = &settings.makemkv_min_version;
                let (ok, detail) = match (&summary.version, &summary.licence) {
                    (_, Some(message)) => {
                        (false, format!("MakeMKV printed licence message {}: {}", message.code, message.text))
                    }
                    (None, None) => (false, format!("{name} printed no version")),
                    (Some(version), None) if below_minimum(version, minimum) => {
                        (false, format!("{name} is version {version}, below makemkv_min_version {minimum}"))
                    }
                    (Some(version), None) => (true, format!("MakeMKV {version}")),
                };
                (ok, detail, summary.version)
            }
        };
        let found = SelfTestCheck {
            path: Some(location.path.display().to_string()),
            version,
            found: found_text(location.found),
            ..check(name, ok, detail)
        };
        self.makemkv.set_last(&found);
        found
    }

    /// One of the other four tools' checks (S80): its path, the version `tool_version` reads, recorded at
    /// `<data>/selftest/<tool>-version.txt`, and where it was found; ok when found. The java check's detail also says
    /// Java use at the last scan, or that no scan ran yet. Once the check's cancel token is raised, no tool runs.
    fn tool_check(&self, settings: &Settings, tool: Tool) -> SelfTestCheck {
        let name = tool.as_str();
        if self.cancel.is_cancelled() {
            return check(name, false, "the self-test was cancelled before this check ran".to_string());
        }
        let mut found = match self.locate(tool, settings) {
            None => check(name, false, format!("{name} was not found")),
            Some(location) => {
                let out = self.record_path(&format!("{name}-version.txt"));
                let version = out.ok().and_then(|out| {
                    self.with_ctx(settings, |ctx| tool_version(ctx, tool, &location.path, &out))
                });
                let detail = match &version {
                    Some(version) => format!("{name} {version}"),
                    None => format!("{name} printed no version"),
                };
                SelfTestCheck {
                    path: Some(location.path.display().to_string()),
                    version,
                    found: found_text(location.found),
                    ..check(name, true, detail)
                }
            }
        };
        if tool == Tool::Java {
            match *lock(&self.java_in_use) {
                Some(java) => found.detail.push_str(&format!("; java in use at the last scan: {java}")),
                None => found.detail.push_str("; no scan yet"),
            }
        }
        found
    }

    /// Every check of spec 13's self-test that needs no disc, in S80's order: the MakeMKV check (`busy` while a
    /// `makemkvcon` runs), mkvmerge, ffprobe, ffmpeg, java, the three roots and the TMDb token (design part 9.4, S53).
    /// `listed` runs once the MakeMKV check has ended, so the owner can free the disc-phase token the check holds
    /// (Ruling R26.c1). Def: T26. Use: T26.
    pub(crate) fn selftest(&self, settings: &Settings, busy: bool, listed: &dyn Fn()) -> SelfTest {
        let mut checks = vec![self.makemkv_check(settings, busy)];
        listed();
        let tools = [Tool::Mkvmerge, Tool::Ffprobe, Tool::Ffmpeg, Tool::Java];
        checks.extend(tools.map(|tool| self.tool_check(settings, tool)));
        checks.push(root_check("staging_root", settings.staging_root.as_deref(), false));
        checks.push(root_check("tuning_root", settings.tuning_root.as_deref(), false));
        checks.push(root_check("movies_root", settings.movies_root.as_deref(), true));
        checks.push(token_check(self.http.as_ref(), self.secrets.get("tmdb_token").as_deref()));
        SelfTest { checks }
    }

    /// AwaitingTool's re-check (Ruling S81): for a disc-phase return (or an extraction), `makemkvcon` and `mkvmerge`
    /// found and `makemkv`'s check showing a usable `makemkvcon` (found, and a version of at least
    /// `makemkv_min_version`, or running for a job) give ToolFound { Makemkvcon }; for a file-phase return, `mkvmerge`
    /// and `ffprobe` found give ToolFound { Mkvmerge }. `makemkv` runs only once both tools are found. Def: T26.
    /// Use: T26.
    pub(crate) fn tool_found(
        &self,
        settings: &Settings,
        return_to: &State,
        makemkv: impl FnOnce() -> SelfTestCheck,
    ) -> Option<Event> {
        let disc = return_to.is_disc_phase() || matches!(return_to, State::Extracting);
        let needed = if disc { [Tool::Makemkvcon, Tool::Mkvmerge] } else { [Tool::Mkvmerge, Tool::Ffprobe] };
        if needed.iter().any(|tool| self.locate(*tool, settings).is_none()) {
            return None;
        }
        if !disc {
            return Some(Event::ToolFound { tool: Tool::Mkvmerge });
        }
        let check = makemkv();
        let minimum = &settings.makemkv_min_version;
        let version_ok = check.version.as_deref().is_some_and(|version| !below_minimum(version, minimum));
        (check.found.is_some() && (check.busy || version_ok)).then_some(Event::ToolFound { tool: Tool::Makemkvcon })
    }

    /// What a `retry` in a waiting state re-checks (Ruling S81), as the event its cleared condition gives, or None:
    /// AwaitingShare reaches the roots the return uses (ShareBack); AwaitingSpace runs the CheckSpace computation
    /// (SpaceFreed); AwaitingMakeMkv runs a fresh MakeMKV check, whose observed change the owner delivers (None here);
    /// AwaitingTool runs `tool_found` with a fresh MakeMKV check. `busy` says a `makemkvcon` runs for a job. Def: T26.
    /// Use: T26.
    pub(crate) fn recheck(
        &self,
        job: &RecheckJob,
        busy: bool,
        shares: &dyn ShareConnector,
        promises: &Mutex<Promises>,
    ) -> Option<Event> {
        let settings = &job.settings;
        match &job.record.state {
            State::AwaitingShare { .. } => share_back(&job.record, settings, shares),
            State::AwaitingSpace { return_to } => space_freed(job, return_to, shares, promises),
            State::AwaitingMakeMkv { .. } => {
                self.makemkv_check(settings, busy);
                None
            }
            State::AwaitingTool { return_to } => {
                self.tool_found(settings, return_to, || self.makemkv_check(settings, busy))
            }
            _ => None,
        }
    }
}

/// The library root of a job's plan kind, `movies_root` without a plan.
fn library_root<'a>(record: &JobRecord, settings: &'a Settings) -> Option<&'a str> {
    match record.plan.as_ref().map(|plan| plan.kind) {
        Some(Kind::Tv) => settings.tv_root.as_deref(),
        _ => settings.movies_root.as_deref(),
    }
}

/// AwaitingShare's re-check (Ruling S81): the roots the return state uses all reachable give ShareBack. A Planned
/// return's CheckSpace reads staging and the plan kind's library root, so it waits on both (review round 1, V15: a
/// library that does not answer no longer sends the job back to a CheckSpace that feeds ShareLost again); any other
/// disc-phase return, a rip, uses staging alone (the final review's Minor 5); a file-phase return uses staging and the
/// plan kind's library root.
fn share_back(record: &JobRecord, settings: &Settings, shares: &dyn ShareConnector) -> Option<Event> {
    let staging = settings.staging_root.as_deref()?;
    let staging_only = matches!(
        &record.state,
        State::AwaitingShare { return_to } if return_to.is_disc_phase() && !matches!(**return_to, State::Planned { .. })
    );
    let library = if staging_only { None } else { library_root(record, settings) };
    let mut roots = std::iter::once(staging).chain(library);
    roots.all(|root| shares.reach(root) == Reach::Reachable).then_some(Event::ShareBack)
}

/// AwaitingSpace's re-check (Ruling S81): the CheckSpace computation of the return, `rip_check` before a rip and
/// `staged_remux_check` of the staged files' sizes before a remux (the plan's estimate when they cannot be read; review
/// round 1, V3), giving SpaceFreed where it gives SpaceOk. Its event is not fed, so a condition not cleared feeds
/// nothing, ShareLost included. The job folder and staging are read before the `Promises` lock is taken.
fn space_freed(
    job: &RecheckJob,
    return_to: &State,
    shares: &dyn ShareConnector,
    promises: &Mutex<Promises>,
) -> Option<Event> {
    let (record, settings) = (&job.record, &job.settings);
    let plan = record.plan.as_ref()?;
    let staging = settings.staging_root.as_deref()?;
    let library = library_root(record, settings);
    let (event, _) = if return_to.is_disc_phase() {
        let meta = fs::read(job.job_dir.join("meta.json")).ok();
        let meta = meta.and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
        let disc_size = meta.and_then(|meta| meta["total_bytes"].as_u64()).unwrap_or(0);
        let (rip, remux) = (rip_promise(plan, disc_size), planned_remux_promise(plan));
        let mut promises = lock(promises);
        rip_check(rip, remux, staging, library, shares, &mut promises, &record.id)
    } else {
        let library = library?;
        let staged = staged_bytes(staging, &record.id, plan);
        let mut promises = lock(promises);
        staged_remux_check(staged, plan, staging, library, shares, &mut promises, &record.id)
    };
    (event == Event::SpaceOk).then_some(Event::SpaceFreed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lazymkv_clients::RecordedTransport;
    use lazymkv_core::testsupport::Scratch;

    #[test]
    fn a_library_root_is_probed_inside_lazymkv_partial_and_left_clean() {
        let s = Scratch::new("probe");
        let check = root_check("movies_root", Some(s.0.to_str().unwrap()), true);
        assert!(check.ok, "{}", check.detail);
        assert_eq!(check.name, "movies_root");
        let partial = s.0.join(".lazymkv-partial");
        assert!(partial.is_dir());
        assert_eq!(std::fs::read_dir(&partial).unwrap().count(), 0);
        assert_eq!(std::fs::read_dir(&s.0).unwrap().count(), 1);
    }

    /// A library root that is not a folder (mistyped, or not mounted) fails its check, and the check creates nothing
    /// (ruling at the final review).
    #[test]
    fn a_missing_library_root_fails_its_check_and_is_not_created() {
        let s = Scratch::new("probe-missing");
        let missing = s.0.join("Moives");
        let check = root_check("movies_root", Some(missing.to_str().unwrap()), true);
        assert!(!check.ok);
        assert_eq!(check.detail, format!("{} is not a folder", missing.display()));
        assert_eq!(check.path.as_deref(), missing.to_str());
        assert!(!missing.exists());
    }

    #[test]
    fn an_unset_root_fails_its_check() {
        assert!(!root_check("tuning_root", None, false).ok);
    }

    #[test]
    fn the_token_check_is_get_3_authentication() {
        let http = RecordedTransport::new();
        http.answer(&lazymkv_clients::authentication_url(), 200, b"{\"success\":true}");
        let check = token_check(&http, Some("test-token"));
        assert!(check.ok);
        let (url, headers) = http.requests().pop().unwrap();
        assert_eq!(url, "https://api.themoviedb.org/3/authentication");
        assert!(headers.contains(&("Authorization".to_string(), "Bearer test-token".to_string())));
        let refused = RecordedTransport::new();
        refused.answer(&lazymkv_clients::authentication_url(), 401, b"");
        let check = token_check(&refused, Some("test-token"));
        assert!(!check.ok && check.detail.contains("401"));
        assert!(!token_check(&RecordedTransport::new(), None).ok);
    }

    #[test]
    fn version_lines_are_read_per_tool() {
        use lazymkv_core::job::Tool;
        assert_eq!(version_in(Tool::Mkvmerge, "mkvmerge v88.0 ('Synthetic') 64-bit\n", "").as_deref(), Some("88.0"));
        assert_eq!(version_in(Tool::Ffprobe, "ffprobe version 7.1 Copyright\n", "").as_deref(), Some("7.1"));
        assert_eq!(version_in(Tool::Ffmpeg, "ffmpeg version 7.1 Copyright\n", "").as_deref(), Some("7.1"));
        assert_eq!(version_in(Tool::Java, "", "openjdk version \"21.0.2\" 2024-01-16\n").as_deref(), Some("21.0.2"));
        assert_eq!(version_in(Tool::Mkvmerge, "something else\n", ""), None);
    }

    #[test]
    fn the_recorded_check_survives_a_reopen() {
        let s = Scratch::new("makemkv-check");
        let mut state = MakeMkvState::open(&s.0);
        assert!(!state.observe(Some("2.0.0".to_string()), None));
        let reopened = MakeMkvState::open(&s.0);
        assert_eq!(reopened.recorded, Some((Some("2.0.0".to_string()), None)));
        assert!(s.0.join("selftest/makemkv-check.json").is_file());
    }

    #[test]
    fn a_share_recheck_reaches_the_roots_its_return_uses() {
        use lazymkv_core::job::State;
        use lazymkv_platform::fakes::FakeShares;
        let settings = Settings {
            staging_root: Some("/staging".to_string()),
            movies_root: Some("/movies".to_string()),
            ..Settings::default()
        };
        let shares = FakeShares::new();
        shares.set_reach("/movies", Reach::Unreachable("cable out".to_string()));
        let waiting = |return_to: State| JobRecord {
            format: lazymkv_core::record::JOB_FORMAT.to_string(), id: "abc-1".to_string(),
            fingerprint: "ABC".to_string(), disc_id: None, drive: "tok0".to_string(), created_unix: 1, updated_unix: 1,
            state: State::AwaitingShare { return_to: Box::new(return_to) }, message: None, rip_again: false,
            profile: "job.mmcp.xml".to_string(), plan: None, titles: vec![], filed_unix: None, collisions: vec![],
            notes: vec![],
        };
        assert_eq!(share_back(&waiting(State::Ripping), &settings, &shares), Some(Event::ShareBack));
        assert_eq!(share_back(&waiting(State::Remuxing), &settings, &shares), None);
        // Review round 1, V15: a Planned return's CheckSpace reads the library root too.
        let planned = || waiting(State::Planned { mode: lazymkv_core::plan::Mode::Direct });
        assert_eq!(share_back(&planned(), &settings, &shares), None);
        shares.set_reach("/movies", Reach::Reachable);
        assert_eq!(share_back(&waiting(State::Remuxing), &settings, &shares), Some(Event::ShareBack));
        assert_eq!(share_back(&planned(), &settings, &shares), Some(Event::ShareBack));
    }

    /// A listing that printed no version says nothing about MakeMKV's version (a makemkvcon that started and died),
    /// so it is not observed: no change is raised and the recorded check stays (ruling at the final review).
    #[test]
    fn a_listing_with_no_version_is_not_observed() {
        let s = Scratch::new("makemkv-no-version");
        let makemkv = MakeMkv::new(MakeMkvState::open(&s.0));
        makemkv.observe(Some("2.0.0".to_string()), None);
        assert!(!makemkv.take_changed());
        makemkv.observe(None, None);
        assert!(!makemkv.take_changed());
        assert_eq!(MakeMkvState::open(&s.0).recorded, Some((Some("2.0.0".to_string()), None)));
        makemkv.observe(Some("2.0.0".to_string()), None);
        assert!(!makemkv.take_changed());
    }

    #[test]
    fn a_changed_version_or_licence_state_is_reported_once() {
        let mut state = MakeMkvState { last: None, recorded: None, file: None };
        assert!(!state.observe(Some("2.0.0".to_string()), None));
        assert!(!state.observe(Some("2.0.0".to_string()), None));
        assert!(state.observe(Some("2.0.1".to_string()), None));
        assert!(state.observe(Some("2.0.1".to_string()), Some(5050)));
        assert!(!state.observe(Some("2.0.1".to_string()), Some(5050)));
    }
}
