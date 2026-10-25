//! The engine test harness (design part 12.1): a sink that keeps notifications, timers a test fires by hand, a step
//! runner that plays a script, and `Harness`, which runs a real engine over a scratch data folder with the platform
//! fakes. It is compiled under the engine's own `cfg(test)` and under the `testkit` feature (Task 1), so a plain
//! `cargo test -p lazymkv-engine` builds it.
//!
//! The scratch root is `<temp>/engine-harness-<pid>-<n>`, removed on drop. Its name holds neither `mkv` nor `lazymkv`
//! (PF-2): the stand-in tools match their needles by substring, so a root holding `mkv` would make the rip needle match
//! every scan's `--profile=<root>/.../job.mmcp.xml` argument, and a second scan in one tools folder would replay the
//! rip's response.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use lazymkv_clients::RecordedTransport;
use lazymkv_core::fingerprint::content_hash;
use lazymkv_core::ipc::Notification;
use lazymkv_core::job::Action;
use lazymkv_core::record::{JobRecord, Transition};
use lazymkv_platform::fakes::{
    CountingKeepAwake, DriveScript, FakeHost, FakeShares, MemoryConnector, MemoryIpc, MemorySecrets, RecordingEjector,
    ScriptedDrives,
};
use lazymkv_platform::{DriveEvent, DriveWatcher, Platform, ProcessGuard, StdProcessGuard};
use lazymkv_tools::{list_files, CancelToken, Limits};
use serde_json::{json, Value};

use crate::owner::{action_tag, start_until, EngineHandle, StopAfter, NO_EVENT_ACTIONS};
use crate::{EngineConfig, JobContext, NotifySink, Services, StepResult, StepRunner, Timers};

/// How often the harness looks again while it waits.
const POLL: Duration = Duration::from_millis(20);
/// How often a hanging scripted step looks at its cancel token.
const HANG_POLL: Duration = Duration::from_millis(5);
/// `wait`'s cap.
const WAIT_CAP: Duration = Duration::from_secs(10);
/// `run_until`'s cap.
const RUN_UNTIL_CAP: Duration = Duration::from_secs(60);
/// How long `settle` needs everything unchanged.
const QUIET: Duration = Duration::from_millis(300);
/// `settle`'s cap.
const SETTLE_CAP: Duration = Duration::from_secs(10);

/// Locks `mutex`, ignoring a poison a panicked thread left, so one failed assertion does not cascade.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A notification sink that keeps every notification in a shared list (design part 8.6). Def: T22. Use: T22 to T29.
pub struct MemorySink {
    list: Arc<Mutex<Vec<Notification>>>,
}

impl MemorySink {
    /// A sink and the list it pushes a clone of each notification to (design part 8.6). Def: T22. Use: T22 to T29.
    pub fn new() -> (MemorySink, Arc<Mutex<Vec<Notification>>>) {
        let list = Arc::new(Mutex::new(Vec::new()));
        (MemorySink { list: Arc::clone(&list) }, list)
    }
}

impl NotifySink for MemorySink {
    fn deliver(&mut self, notification: &Notification) {
        lock(&self.list).push(notification.clone());
    }
}

/// StartTimer's timers held until a test fires them (design part 5.2). Def: T22. Use: T22 to T29.
#[derive(Default)]
pub struct ManualTimers {
    pending: Mutex<Vec<(u32, Box<dyn FnOnce() + Send>)>>,
}

impl ManualTimers {
    /// No timer pending (design part 5.2). Def: T22. Use: T22 to T29.
    pub fn new() -> ManualTimers {
        ManualTimers::default()
    }

    /// Runs and drops every pending timer, oldest first (design part 5.2). Def: T22. Use: T22 to T29.
    pub fn fire_all(&self) {
        let pending = std::mem::take(&mut *lock(&self.pending));
        for (_, fire) in pending {
            fire();
        }
    }

    /// The minutes of each pending timer, oldest first (design part 5.2). Def: T22. Use: T22 to T29.
    pub fn pending(&self) -> Vec<u32> {
        lock(&self.pending).iter().map(|(minutes, _)| *minutes).collect()
    }

    /// Drops every pending timer without running it, as a real restart loses its sleeping threads.
    fn drop_pending(&self) {
        lock(&self.pending).clear();
    }
}

impl Timers for ManualTimers {
    fn start(&self, minutes: u32, fire: Box<dyn FnOnce() + Send>) {
        lock(&self.pending).push((minutes, fire));
    }
}

/// What a scripted step does (design part 5.2). Def: T22. Use: T22 to T29.
pub enum StepScript {
    /// Returns this result at once.
    Result(StepResult),
    /// Returns `StepResult { event: None, message: Some("cancelled") }` once its cancel token is raised.
    Hang,
    /// Panics with this text, as a step with a bug would.
    Panic(&'static str),
}

/// One call of a scripted step (R22.2): the action's tag, state.json's state at the call and the number of
/// transitions.jsonl lines then. Def: T22. Use: T22.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepSeen {
    /// The action's serde tag.
    pub action: String,
    /// state.json's state, as `State::as_str`, or "" when unreadable.
    pub state: String,
    /// The lines of transitions.jsonl.
    pub lines: usize,
}

/// A step runner that plays a script (design part 5.2): each call takes the first unused entry of its action's tag.
/// With no entry left, EmptyTitleDirectory, DeletePartialFile and ExtractSubtitleEvidence return no event and no
/// message, and any other action returns no event with the message `no scripted result for <tag>`. Def: T22. Use: T22
/// to T29.
pub struct ScriptedSteps {
    script: Mutex<Vec<(&'static str, Option<StepScript>)>>,
    calls: Mutex<Vec<String>>,
    seen: Mutex<Vec<StepSeen>>,
}

impl ScriptedSteps {
    /// A runner over `script`, a list of (Action tag, what that call does) (design part 5.2). Def: T22. Use: T22 to
    /// T29.
    pub fn new(script: Vec<(&'static str, StepScript)>) -> ScriptedSteps {
        let script = script.into_iter().map(|(tag, step)| (tag, Some(step))).collect();
        ScriptedSteps { script: Mutex::new(script), calls: Mutex::default(), seen: Mutex::default() }
    }

    /// The tag of every call so far, in order (design part 5.2). Def: T22. Use: T22 to T29.
    pub fn calls(&self) -> Vec<String> {
        lock(&self.calls).clone()
    }

    /// What every call so far saw of its job's files (R22.2). Def: T22. Use: T22.
    pub fn seen(&self) -> Vec<StepSeen> {
        lock(&self.seen).clone()
    }
}

impl StepRunner for ScriptedSteps {
    fn run(
        &self,
        job: &JobContext,
        action: &Action,
        cancel: CancelToken,
        _progress: &(dyn Fn(Option<&str>, f64) + Sync),
    ) -> StepResult {
        let tag = action_tag(action);
        let state = read_record(&job.job_dir).map(|record| record.state.as_str().to_string()).unwrap_or_default();
        let lines = fs::read(job.job_dir.join("transitions.jsonl"))
            .map(|bytes| bytes.iter().filter(|&&byte| byte == b'\n').count())
            .unwrap_or(0);
        lock(&self.seen).push(StepSeen { action: tag.clone(), state, lines });
        lock(&self.calls).push(tag.clone());
        let entry = lock(&self.script)
            .iter_mut()
            .find(|(entry, step)| *entry == tag && step.is_some())
            .and_then(|(_, step)| step.take());
        match entry {
            Some(StepScript::Result(result)) => result,
            Some(StepScript::Hang) => {
                while !cancel.is_cancelled() {
                    thread::sleep(HANG_POLL);
                }
                StepResult { event: None, message: Some("cancelled".to_string()) }
            }
            Some(StepScript::Panic(text)) => panic!("{text}"),
            None if NO_EVENT_ACTIONS.contains(&tag.as_str()) => StepResult { event: None, message: None },
            None => StepResult { event: None, message: Some(format!("no scripted result for {tag}")) },
        }
    }
}

/// `<job_dir>/state.json`, when it reads as a job record.
fn read_record(job_dir: &Path) -> Option<JobRecord> {
    serde_json::from_slice(&fs::read(job_dir.join("state.json")).ok()?).ok()
}

/// The engine's drive watcher over the harness's `ScriptedDrives`, so every engine the harness starts reads the same
/// drive queue; `present` is the `DriveScript`'s.
struct SharedDrives {
    drives: Arc<Mutex<ScriptedDrives>>,
    script: DriveScript,
}

impl DriveWatcher for SharedDrives {
    fn next(&mut self, timeout: Duration) -> Option<DriveEvent> {
        lock(&self.drives).next(timeout)
    }

    fn present(&self) -> Vec<(String, PathBuf)> {
        self.script.present()
    }
}

/// A drive event a test asked for, held until the next Harness call (R22.1).
enum Queued {
    Arrive(String, PathBuf),
    Remove(String),
}

/// A real engine over a scratch folder `<root>/{data, staging, library/Movies, tuning, volumes}`, with the platform
/// fakes, scripted steps, a memory sink and manual timers (design part 12.1, the interface sheet's helper list). Every
/// method takes `&self`. `insert`, `insert_root` and `remove` only queue their drive event; every other method first
/// hands the queued events to the `DriveScript`, in order (R22.1), except `restart`, which hands them over once the
/// old engine has stopped, so the new engine reads them (R22.9). The data-folder readers work while the engine is
/// stopped. Dropping the harness stops a running engine and removes the scratch folder. Def: T22. Use: T22 to T29.
pub struct Harness {
    root: PathBuf,
    data: PathBuf,
    staging: PathBuf,
    library: PathBuf,
    volumes: PathBuf,
    script: DriveScript,
    drives: Arc<Mutex<ScriptedDrives>>,
    queued: Mutex<Vec<Queued>>,
    ejector: Arc<RecordingEjector>,
    processes: Arc<dyn ProcessGuard>,
    awake: Arc<CountingKeepAwake>,
    shares: Arc<FakeShares>,
    secrets: Arc<MemorySecrets>,
    ipc: Arc<MemoryIpc>,
    /// The client end of `ipc`, kept so the engine's end never sees every connector gone (Task 26 hands out clones).
    _connector: MemoryConnector,
    host: Arc<FakeHost>,
    http: Arc<RecordedTransport>,
    timers: Arc<ManualTimers>,
    steps: Arc<ScriptedSteps>,
    sent: Arc<Mutex<Vec<Notification>>>,
    /// The stop hook's target state and the states it saw since the last `restart` or `run_until`.
    hook: Arc<Mutex<(Option<String>, Vec<String>)>>,
    engine: Mutex<Option<EngineHandle>>,
}

impl Harness {
    /// A harness whose steps play `script` (design part 12.1). `data/settings.json` holds the harness defaults
    /// (`staging_root`, `movies_root` and `tuning_root` as the scratch folders, `keep_tuning_evidence` false,
    /// `drive_settle_s` 0, `drive_ready_timeout_s` 1), each key of `settings` replacing its default, and each key
    /// inside `tool_paths` replacing that one (R22.5). The host is `FakeHost::new("2026-10-23", 1_792_800_000)`, the
    /// ejector pushes the removal its eject causes (R22.6), and the secrets hold `tmdb_token`. The engine starts at
    /// once. Def: T22. Use: T22 to T29.
    pub fn new_scripted(script: Vec<(&'static str, StepScript)>, settings: Value) -> Harness {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!("engine-harness-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let (data, staging, library, tuning, volumes) = (
            root.join("data"),
            root.join("staging"),
            root.join("library").join("Movies"),
            root.join("tuning"),
            root.join("volumes"),
        );
        for dir in [&data, &staging, &library, &tuning, &volumes] {
            fs::create_dir_all(dir).expect("the harness makes its scratch folders");
        }
        let text = |path: &Path| Value::String(path.display().to_string());
        let mut merged = json!({
            "staging_root": text(&staging), "movies_root": text(&library), "tuning_root": text(&tuning),
            "keep_tuning_evidence": false, "drive_settle_s": 0, "drive_ready_timeout_s": 1,
        });
        merge_settings(&mut merged, settings);
        let written = serde_json::to_vec_pretty(&merged).expect("the harness settings serialise");
        fs::write(data.join("settings.json"), written).expect("the harness writes data/settings.json");
        let (drives, script_end) = ScriptedDrives::new();
        let (ipc, connector) = MemoryIpc::new();
        let harness = Harness {
            root,
            data,
            staging,
            library,
            volumes,
            ejector: Arc::new(RecordingEjector::new().with_removals(script_end.clone())),
            script: script_end,
            drives: Arc::new(Mutex::new(drives)),
            queued: Mutex::default(),
            processes: Arc::new(StdProcessGuard),
            awake: Arc::new(CountingKeepAwake::new()),
            shares: Arc::new(FakeShares::new()),
            secrets: Arc::new(MemorySecrets::new().with("tmdb_token", "test-token")),
            ipc: Arc::new(ipc),
            _connector: connector,
            host: Arc::new(FakeHost::new("2026-10-23", 1_792_800_000)),
            http: Arc::new(RecordedTransport::new()),
            timers: Arc::new(ManualTimers::new()),
            steps: Arc::new(ScriptedSteps::new(script)),
            sent: Arc::default(),
            hook: Arc::default(),
            engine: Mutex::default(),
        };
        harness.start();
        harness
    }

    /// Starts an engine with `start_until` and the stop hook over the harness's data folder and fakes.
    fn start(&self) {
        let platform = Platform {
            drives: Box::new(SharedDrives { drives: Arc::clone(&self.drives), script: self.script.clone() }),
            ejector: Arc::clone(&self.ejector) as _,
            processes: Arc::clone(&self.processes),
            awake: Arc::clone(&self.awake) as _,
            shares: Arc::clone(&self.shares) as _,
            secrets: Arc::clone(&self.secrets) as _,
            ipc: Arc::clone(&self.ipc) as _,
            host: Arc::clone(&self.host) as _,
        };
        let services = Services {
            platform,
            http: Arc::clone(&self.http) as _,
            notify: Box::new(MemorySink { list: Arc::clone(&self.sent) }),
            timers: Arc::clone(&self.timers) as _,
            steps: Some(Arc::clone(&self.steps) as _),
        };
        let config = EngineConfig {
            data: self.data.clone(),
            resources: None,
            limits_override: Some(Limits {
                header: Duration::from_secs(10),
                scan: Duration::from_secs(10),
                exit_wait: Duration::from_secs(10),
                after_kill: Duration::from_millis(50),
            }),
            path_var: Some(OsString::new()),
        };
        let hook = Arc::clone(&self.hook);
        let stop_after: StopAfter = Arc::new(move |transition: &Transition| {
            let mut hook = lock(&hook);
            let state = transition.new.as_str();
            hook.1.push(state.to_string());
            let stop = hook.0.as_deref() == Some(state);
            if stop {
                hook.0 = None;
            }
            stop
        });
        let engine = start_until(config, services, Some(stop_after)).expect("the harness's engine starts");
        *lock(&self.engine) = Some(engine);
    }

    /// Stops the running engine, if any, and joins its threads.
    fn stop(&self) {
        let engine = lock(&self.engine).take();
        if let Some(engine) = engine {
            engine.stop().expect("the harness's engine stops");
        }
    }

    /// Hands the queued drive events to the `DriveScript`, in order (R22.1).
    fn deliver(&self) {
        let queued = std::mem::take(&mut *lock(&self.queued));
        for event in queued {
            match event {
                Queued::Arrive(drive, root) => self.script.arrive(&drive, &root),
                Queued::Remove(drive) => self.script.remove(&drive),
            }
        }
    }

    /// `<root>/volumes/<disc>`, built on first use: `BDMV/index.bdmv` "INDX0200", `BDMV/PLAYLIST/00800.mpls`
    /// "MPLS0200", `BDMV/STREAM/00001.m2ts` of 1000 plus the sum of the name's bytes zero bytes and
    /// `AACS/Unit_Key_RO.inf` the name's bytes, labelled with the name in upper case.
    fn volume(&self, disc: &str) -> PathBuf {
        let root = self.volumes.join(disc);
        if !root.join("BDMV").is_dir() {
            let stream = vec![0u8; 1000 + disc.bytes().map(usize::from).sum::<usize>()];
            let files: [(&str, &[u8]); 4] = [
                ("BDMV/index.bdmv", &b"INDX0200"[..]),
                ("BDMV/PLAYLIST/00800.mpls", &b"MPLS0200"[..]),
                ("BDMV/STREAM/00001.m2ts", &stream[..]),
                ("AACS/Unit_Key_RO.inf", disc.as_bytes()),
            ];
            for (path, bytes) in files {
                let path = root.join(path);
                fs::create_dir_all(path.parent().expect("a volume file has a folder")).expect("the volume's folder");
                fs::write(&path, bytes).expect("the volume's file");
            }
        }
        self.host.set_label(&root, &disc.to_ascii_uppercase());
        root
    }

    /// Queues the arrival of the synthetic disc `disc` in `drive`, building `<root>/volumes/<disc>/` and its label on
    /// first use (R22.1). Def: T22. Use: T22 to T29.
    pub fn insert(&self, drive: &str, disc: &str) {
        let root = self.volume(disc);
        lock(&self.queued).push(Queued::Arrive(drive.to_string(), root));
    }

    /// Queues an arrival in `drive` with `root` as given (R22.1). Def: T22. Use: T22 to T29.
    pub fn insert_root(&self, drive: &str, root: &Path) {
        lock(&self.queued).push(Queued::Arrive(drive.to_string(), root.to_path_buf()));
    }

    /// Queues the removal of `drive`'s disc (R22.1). Def: T22. Use: T22 to T29.
    pub fn remove(&self, drive: &str) {
        lock(&self.queued).push(Queued::Remove(drive.to_string()));
    }

    /// Arms the stop hook for the first transition into `state`, hands the queued drive events to the engine, waits
    /// (60 s cap, then panics naming `state` and the states persisted so far) until the owner stopped after persisting
    /// that transition and before running any of its actions, and joins the engine's threads. The hook also logs every
    /// state it sees; when that log, kept since the last `restart` or `run_until`, already holds `state` (possible only
    /// right after `restart`, whose resumed actions start at once), it stops the engine at once instead. Def: T22. Use:
    /// T22 to T29.
    pub fn run_until(&self, state: &str) {
        let passed = {
            let mut hook = lock(&self.hook);
            let passed = hook.1.iter().any(|seen| seen == state);
            if !passed {
                hook.0 = Some(state.to_string());
            }
            hook.1.clear();
            passed
        };
        if !passed {
            self.deliver();
            let deadline = Instant::now() + RUN_UNTIL_CAP;
            loop {
                let (armed, seen) = {
                    let hook = lock(&self.hook);
                    (hook.0.is_some(), hook.1.clone())
                };
                if !armed {
                    break;
                }
                if Instant::now() >= deadline {
                    lock(&self.hook).0 = None;
                    panic!("run_until({state}): no transition into {state} within 60 s; persisted: {seen:?}");
                }
                thread::sleep(POLL);
            }
        }
        self.stop();
    }

    /// Stops a running engine (`EngineHandle::stop`), drops every pending ManualTimers timer, as a real restart loses
    /// its sleeping threads, and starts a new engine (`start_until`) over the same data folder with the same drive
    /// script, ejector, keep-awake, shares, secrets, MemoryIpc, host, RecordedTransport, ScriptedSteps (used entries
    /// stay used) and MemorySink list. The queued drive events are handed over once the old engine has stopped, so
    /// the new engine reads them (R22.9). Def: T22. Use: T22 to T29.
    pub fn restart(&self) {
        self.stop();
        self.timers.drop_pending();
        {
            let mut hook = lock(&self.hook);
            hook.0 = None;
            hook.1.clear();
        }
        self.deliver();
        self.start();
    }

    /// The content hash of the built volume of `disc`, upper case (spec 5). Def: T22. Use: T22 to T29.
    pub fn fingerprint(&self, disc: &str) -> String {
        self.deliver();
        let files = list_files(&self.volume(disc)).expect("the harness lists its volume");
        content_hash(&files).expect("a synthetic volume has a fingerprint")
    }

    /// The job folders holding a state.json, sorted (design part 7.3). Def: T22. Use: T22 to T29.
    pub fn job_ids(&self) -> Vec<String> {
        self.deliver();
        let mut ids: Vec<String> = fs::read_dir(self.data.join("jobs"))
            .map(|entries| {
                entries
                    .filter_map(|entry| entry.ok())
                    .filter(|entry| entry.path().join("state.json").is_file())
                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        ids.sort();
        ids
    }

    /// state.json's state as `State::as_str`; "" when no such job (design part 7.3). Def: T22. Use: T22 to T29.
    pub fn state_of(&self, id: &str) -> String {
        self.deliver();
        read_record(&self.job_dir(id)).map(|record| record.state.as_str().to_string()).unwrap_or_default()
    }

    /// The job's state.json (design part 7.3). Def: T22. Use: T22 to T29.
    pub fn record(&self, id: &str) -> JobRecord {
        self.deliver();
        read_record(&self.job_dir(id)).unwrap_or_else(|| panic!("job {id} has no readable state.json"))
    }

    /// The job's transitions.jsonl, every whole line (design part 7.3). Def: T22. Use: T22 to T29.
    pub fn transitions(&self, id: &str) -> Vec<Transition> {
        self.deliver();
        let text = fs::read_to_string(self.job_dir(id).join("transitions.jsonl")).unwrap_or_default();
        let whole = &text[..text.rfind('\n').map_or(0, |end| end + 1)];
        whole
            .lines()
            .map(|line| serde_json::from_str(line).unwrap_or_else(|error| panic!("{line}: {error}")))
            .collect()
    }

    /// Every notification the sink received, in order (design part 8.6). Def: T22. Use: T22 to T29.
    pub fn notifications(&self) -> Vec<Notification> {
        self.deliver();
        lock(&self.sent).clone()
    }

    /// engine.log, or "" when absent (design part 3.5). Def: T22. Use: T22 to T29.
    pub fn engine_log(&self) -> String {
        self.deliver();
        fs::read_to_string(self.data.join("engine.log")).unwrap_or_default()
    }

    /// drives.json as JSON, or Null when absent or unreadable (design part 7.4). Def: T22. Use: T22 to T29.
    pub fn drives_json(&self) -> Value {
        self.deliver();
        fs::read(self.data.join("drives.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or(Value::Null)
    }

    /// The scripted step runner (design part 5.2). Def: T22. Use: T22 to T29.
    pub fn steps(&self) -> &ScriptedSteps {
        self.deliver();
        &self.steps
    }

    /// Polls every 20 ms until `done` holds; panics naming `what` after 10 s. Def: T22. Use: T22 to T29.
    pub fn wait(&self, what: &str, done: impl Fn(&Harness) -> bool) {
        self.deliver();
        let deadline = Instant::now() + WAIT_CAP;
        while !done(self) {
            if Instant::now() >= deadline {
                panic!("waited 10 s for {what}");
            }
            thread::sleep(POLL);
        }
    }

    /// Returns once the job files, notifications, engine.log, drives.json, ejects, step calls and keep-awake holders
    /// are unchanged for 300 ms (10 s cap). Def: T22. Use: T22 to T29.
    pub fn settle(&self) {
        self.deliver();
        let cap = Instant::now() + SETTLE_CAP;
        let mut last = self.quiet_marks();
        let mut since = Instant::now();
        while Instant::now() < cap {
            thread::sleep(POLL);
            let marks = self.quiet_marks();
            if marks != last {
                last = marks;
                since = Instant::now();
            } else if since.elapsed() >= QUIET {
                return;
            }
        }
    }

    /// What `settle` watches, as sizes and counts.
    fn quiet_marks(&self) -> Vec<String> {
        let size = |path: PathBuf| fs::metadata(path).map_or(0, |meta| meta.len());
        let mut marks: Vec<String> = fs::read_dir(self.data.join("jobs"))
            .map(|entries| {
                entries
                    .filter_map(|entry| entry.ok())
                    .map(|entry| {
                        let dir = entry.path();
                        let name = entry.file_name().to_string_lossy().into_owned();
                        let state = size(dir.join("state.json"));
                        let (lines, log) = (size(dir.join("transitions.jsonl")), size(dir.join("job.log")));
                        format!("{name} {state} {lines} {log}")
                    })
                    .collect()
            })
            .unwrap_or_default();
        marks.sort();
        marks.push(format!(
            "{} {} {} {} {} {}",
            lock(&self.sent).len(),
            size(self.data.join("engine.log")),
            size(self.data.join("drives.json")),
            self.ejector.calls().len(),
            self.steps.calls().len(),
            self.awake.holders()
        ));
        marks
    }

    /// The data folder (design part 3.5). Def: T22. Use: T22 to T29.
    pub fn data(&self) -> &Path {
        self.deliver();
        &self.data
    }

    /// The staging root (spec 7). Def: T22. Use: T22 to T29.
    pub fn staging(&self) -> &Path {
        self.deliver();
        &self.staging
    }

    /// The movies root, `<root>/library/Movies` (spec 7). Def: T22. Use: T22 to T29.
    pub fn library(&self) -> &Path {
        self.deliver();
        &self.library
    }

    /// `<data>/jobs/<id>` (design part 7.1). Def: T22. Use: T22 to T29.
    pub fn job_dir(&self, id: &str) -> PathBuf {
        self.deliver();
        self.data.join("jobs").join(id)
    }

    /// The ejector, shared by every engine the harness starts (design part 4.2). Def: T22. Use: T22 to T29.
    pub fn ejector(&self) -> &RecordingEjector {
        self.deliver();
        &self.ejector
    }

    /// The keep-awake (design part 4.2). Def: T22. Use: T22 to T29.
    pub fn awake(&self) -> &CountingKeepAwake {
        self.deliver();
        &self.awake
    }

    /// The shares (design part 4.2). Def: T22. Use: T22 to T29.
    pub fn shares(&self) -> &FakeShares {
        self.deliver();
        &self.shares
    }

    /// The host (design part 4.2). Def: T22. Use: T22 to T29.
    pub fn host(&self) -> &FakeHost {
        self.deliver();
        &self.host
    }

    /// The lookups' recorded transport (design part 8.1). Def: T22. Use: T22 to T29.
    pub fn http(&self) -> &RecordedTransport {
        self.deliver();
        &self.http
    }

    /// The manual timers (design part 5.2). Def: T22. Use: T22 to T29.
    pub fn timers(&self) -> &ManualTimers {
        self.deliver();
        &self.timers
    }

    /// Runs and drops every pending timer (design part 5.2). Def: T22. Use: T22 to T29.
    pub fn fire_timers(&self) {
        self.deliver();
        self.timers.fire_all();
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let engine = lock(&self.engine).take();
        if let Some(engine) = engine {
            let _ = engine.stop();
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Puts each key of `given` into `defaults`, and each key inside `tool_paths` into the default `tool_paths` (R22.5:
/// one level deep, so a test can unset one tool).
fn merge_settings(defaults: &mut Value, given: Value) {
    let (Some(defaults), Value::Object(given)) = (defaults.as_object_mut(), given) else {
        return;
    };
    for (key, value) in given {
        if key == "tool_paths" {
            if let (Some(Value::Object(paths)), Value::Object(changes)) = (defaults.get_mut(&key), &value) {
                paths.extend(changes.clone());
                continue;
            }
        }
        defaults.insert(key, value);
    }
}
