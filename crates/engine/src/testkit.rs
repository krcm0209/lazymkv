//! The engine test harness (design part 12.1): a sink that keeps notifications, timers a test fires by hand, a step
//! runner that plays a script, and `Harness`, which runs a real engine over a scratch data folder with the platform
//! fakes. It is compiled under the engine's own `cfg(test)` and under the `testkit` feature (Task 1), so a plain
//! `cargo test -p lazymkv-engine` builds it.
//!
//! The scratch root is `<temp>/engine-harness-<pid>-<n>`, removed on drop. Its name holds neither `mkv` nor `lazymkv`
//! (PF-2): the stand-in tools match their needles by substring, so a root holding `mkv` would make the rip needle match
//! every scan's `--profile=<root>/.../job.mmcp.xml` argument, and a second scan in one tools folder would replay the
//! rip's response.
//!
//! `Harness::new` runs the real step runner on stand-in tools (Task 23): it copies a scenario of `corpus/scenarios`
//! and the stand-in into `<root>/tools`, never running a stand-in inside `corpus/` (S12), builds the scenario's disc
//! from its `disc.json`, and replays its `http.json`. When the scenarios hold `lazymkvctl.json`, it also copies the
//! stand-in as `<root>/tools/lazymkvctl`, which the engine's `command` relay runs (design part 9.5, Task T3-22).
//!
//! On Windows, `Harness::new_native` runs the same stand-ins under the binary's own start, `native::run_with`, behind
//! a real mutex and pipe of the test's own names (Task T3-23).

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use lazymkv_clients::{search_url, HttpTransport, OfflineTransport, RecordedTransport};
use lazymkv_core::disc::bdmt_name;
use lazymkv_core::fingerprint::content_hash;
use lazymkv_core::ipc::Notification;
use lazymkv_core::job::{Action, Event, Tool};
use lazymkv_core::lookup::name::disc_name;
use lazymkv_core::record::{JobRecord, Transition};
use lazymkv_core::snapshot::{disc_meta, unit_key_file};
use lazymkv_core::testsupport::{default_profile_xml, ustar_build};
use lazymkv_platform::fakes::{
    CountingKeepAwake, DriveScript, FakeHost, FakeShares, MemoryConnector, MemoryIpc, MemorySecrets, RecordingEjector,
    ScriptedDrives,
};
use lazymkv_platform::{native_guard, DriveEvent, DriveWatcher, GuardedChild, Platform, ProcessGuard, Spawn};
use lazymkv_tools::{exe_name, list_files, CancelToken, Limits};
use serde_json::{json, Value};

#[cfg(windows)]
use std::io::BufRead;
#[cfg(windows)]
use std::panic;
#[cfg(windows)]
use std::sync::mpsc::channel;
#[cfg(windows)]
use std::sync::Condvar;
#[cfg(windows)]
use std::thread::JoinHandle;

#[cfg(windows)]
use lazymkv_core::ipc::PROTOCOL_VERSION;
#[cfg(windows)]
use lazymkv_platform::win::{own_token, protect_data_folder, PipeConnector};
#[cfg(windows)]
use lazymkv_platform::{ConnectError, IpcConnector};

#[cfg(windows)]
use crate::native::{run_with, NativeOptions};
#[cfg(windows)]
use crate::owner::panic_text;
use crate::owner::{action_tag, start_until, EngineHandle, StopAfter, TestHooks, NO_EVENT_ACTIONS};
use crate::store::{PersistFault, PersistFaults};
use crate::{
    EngineConfig, JobContext, NoDelivery, NotifySink, Services, ShutdownHandle, StepResult, StepRunner, Timers,
};

/// How often the harness looks again while it waits.
const POLL: Duration = Duration::from_millis(20);
/// How often a hanging scripted step looks at its cancel token.
const HANG_POLL: Duration = Duration::from_millis(5);
/// `wait`'s cap, times `ci_factor`.
const WAIT_CAP: Duration = Duration::from_secs(10);
/// `run_until`'s cap, times `ci_factor`.
const RUN_UNTIL_CAP: Duration = Duration::from_secs(60);
/// How long `settle` needs everything unchanged.
const QUIET: Duration = Duration::from_millis(300);
/// How long `settle` needs everything unchanged while a step or lookup round is outstanding (Ruling R24.c1).
const BUSY_QUIET: Duration = Duration::from_secs(2);
/// `settle`'s cap.
const SETTLE_CAP: Duration = Duration::from_secs(10);
/// How long a scan waits for the recorded lookups' files (R23.2). It is not scaled by `ci_factor` (Ruling R29.c2b):
/// the recorder starts its time-limit clock before the spawn, so this wait counts against the harness's 10 s scan
/// limit, and a scan whose lookups never answer, as a second synthetic disc's do, would be killed at that limit.
const LOOKUP_WAIT_CAP: Duration = Duration::from_secs(5);

/// What the harness's waiting bounds are multiplied by: 4 when the `CI` environment variable is set, as GitHub Actions
/// sets it, else 1 (Rulings R29.c2, R29.c2a, R29.c2b). A shared CI runner can be several times slower than the mini
/// PC, so `wait`'s, `run_until`'s and `fire_timers`' caps and the IPC tests' line wait stretch there; `settle`'s quiet
/// windows and cap and a scan's wait for the recorded lookups (`LOOKUP_WAIT_CAP`) do not. Def: T29. Use: T29.
pub fn ci_factor() -> u32 {
    if std::env::var_os("CI").is_some() {
        4
    } else {
        1
    }
}

/// How many times `retry_busy` tries in all, and how long it pauses between tries (Ruling R29.c4).
const BUSY_TRIES: u32 = 20;
const BUSY_PAUSE: Duration = Duration::from_millis(50);

/// Runs `attempt` again while it fails with `ExecutableFileBusy`, up to 20 tries 50 ms apart, and then returns its last
/// result (Rulings R29.c3, R29.c4). On Linux, executing a freshly copied stand-in can fail with ETXTBSY: while the
/// copy's write handle is open, another test thread may fork for its own process, and that child holds the handle
/// until its own exec closes it. Def: T29. Use: T29 tests.
pub fn retry_busy<T>(mut attempt: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    let mut tries = 1;
    loop {
        match attempt() {
            Err(error) if error.kind() == io::ErrorKind::ExecutableFileBusy && tries < BUSY_TRIES => {
                tries += 1;
                thread::sleep(BUSY_PAUSE);
            }
            result => return result,
        }
    }
}

/// A process guard that starts its inner guard's spawn again, through `retry_busy`, while the operating system refuses
/// it with `ExecutableFileBusy` (Linux ETXTBSY: a freshly copied stand-in executed while another test thread's fork
/// still holds the copy's write handle), and otherwise hands over the inner guard's own child, so its pipes, kill
/// handle and wait are unchanged (Ruling R29.c4). Def: T29. Use: T29 tests.
pub struct BusyRetry<G: ProcessGuard>(pub G);

impl<G: ProcessGuard> ProcessGuard for BusyRetry<G> {
    fn spawn(&self, spawn: &Spawn) -> io::Result<Box<dyn GuardedChild>> {
        retry_busy(|| self.0.spawn(spawn))
    }
}

/// `Harness::on_spawn`'s actions, each with its needles; an action is taken out when it runs.
type SpawnHooks = Arc<Mutex<Vec<(Vec<String>, Option<Box<dyn Fn() + Send + Sync>>)>>>;

/// The console handler a native run installed through `NativeOptions.console_stop` (`Harness::new_native`).
#[cfg(windows)]
type ConsoleHandler = Arc<Mutex<Option<Box<dyn Fn() + Send + Sync>>>>;

/// Whether a native harness of this test process is alive, and the wait for it to go (`NativeSlot`).
#[cfg(windows)]
static NATIVE_SLOT: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());

/// The one native harness alive in a test process at a time (`Harness::new_native`). A native engine runs on the real
/// `WinHost`, whose process list S55 reads before every `makemkvcon` call, so another native harness's stand-in
/// `makemkvcon64` would refuse this one's calls with `another MakeMKV process is running`. `take` waits until the slot
/// is free; the drop frees it.
#[cfg(windows)]
struct NativeSlot;

#[cfg(windows)]
impl NativeSlot {
    /// Waits until no other native harness of this process is alive, then holds the slot.
    fn take() -> NativeSlot {
        let (taken, freed) = &NATIVE_SLOT;
        let mut taken = lock(taken);
        while *taken {
            taken = freed.wait(taken).unwrap_or_else(PoisonError::into_inner);
        }
        *taken = true;
        NativeSlot
    }
}

#[cfg(windows)]
impl Drop for NativeSlot {
    fn drop(&mut self) {
        let (taken, freed) = &NATIVE_SLOT;
        *lock(taken) = false;
        freed.notify_one();
    }
}

/// Locks `mutex`, ignoring a poison a panicked thread left, so one failed assertion does not cascade.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A notification sink that keeps every notification in a shared list (design part 8.6). Def: T22; T3-21 returns Ok.
/// Use: T22 to T29, T3-21.
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
    fn deliver(&mut self, notification: &Notification) -> Result<(), String> {
        lock(&self.list).push(notification.clone());
        Ok(())
    }
}

/// The console writer every engine of a harness gets (`EngineConfig.console`, design part 3.5): it keeps what it is
/// given in the harness's buffer.
struct ConsoleBuffer(Arc<Mutex<Vec<u8>>>);

impl Write for ConsoleBuffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        lock(&self.0).extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
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

    /// Runs and drops every pending timer of `minutes`, oldest first, and leaves the others pending, as the minute that
    /// runs out a refeed timer leaves an hourly StartTimer timer running (design part 5.2; Ruling T3-20-a). Def: T3-20.
    /// Use: T3-20.
    pub fn fire_lasting(&self, minutes: u32) {
        let fired: Vec<_> = {
            let mut pending = lock(&self.pending);
            let (fired, kept) = std::mem::take(&mut *pending).into_iter().partition(|(lasting, _)| *lasting == minutes);
            *pending = kept;
            fired
        };
        for (_, fire) in fired {
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
    /// Sleeps for the duration, then returns the result whether or not the step was cancelled: a CheckSpace or Eject
    /// that `cancel` does not stop (design parts 3.8, 9.4; Task 26).
    After(Duration, StepResult),
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
            Some(StepScript::After(pause, result)) => {
                thread::sleep(pause);
                result
            }
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

/// What the stop hook stops after: the first transition into a state (`run_until`), or the first whose event a test's
/// predicate accepts (`run_until_event`).
enum StopAt {
    State(String),
    Event(Box<dyn Fn(&Event) -> bool + Send + Sync>),
}

/// A test's choice of the persists that meet a fault (`Harness::refuse_persists`, `Harness::lose_log_lines`).
type PersistChoice = Box<dyn Fn(&JobRecord, &Transition) -> bool + Send + Sync>;

/// The fault a test armed, with the persists it applies to; None while every persist works.
type ArmedFault = Option<(PersistFault, PersistChoice)>;

/// A drive event a test asked for, held until the next Harness call (R22.1).
enum Queued {
    Arrive(String, PathBuf),
    Remove(String),
}

/// A real engine over a scratch folder `<root>/{data, staging, library/Movies, tuning, volumes}`, with the platform
/// fakes, scripted steps (or, from `new`, the real step runner on stand-in tools in `<root>/tools`), a memory sink and
/// manual timers (design parts 12.1, 12.2, the interface sheet's helper list). Every
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
    /// `<root>/tools`: the merged scenario folder, holding the tool copies (`Harness::new`).
    tools: PathBuf,
    /// The first scenario's name and its `disc.json`, which `insert` of that name builds (`Harness::new`).
    disc: Option<(String, Value)>,
    /// Whether a scan waits for the recorded lookups' files (R23.2), shared with the `TestGuard`.
    lookups_wait: Arc<AtomicBool>,
    /// `on_spawn`'s actions, shared with the `TestGuard`.
    spawn_hooks: SpawnHooks,
    /// Every `Spawn` the `TestGuard` saw, in order; empty for `new_scripted`.
    spawns: Arc<Mutex<Vec<Spawn>>>,
    /// The stand-in `lazymkvctl` every engine gets as `EngineConfig.ctl`; None when no scenario holds
    /// `lazymkvctl.json`, and for `new_scripted`.
    ctl: Option<PathBuf>,
    /// True for `new_scripted`: the engine runs `steps`; false for `new`: the real runner.
    scripted: bool,
    script: DriveScript,
    drives: Arc<Mutex<ScriptedDrives>>,
    queued: Mutex<Vec<Queued>>,
    ejector: Arc<RecordingEjector>,
    processes: Arc<dyn ProcessGuard>,
    awake: Arc<CountingKeepAwake>,
    shares: Arc<FakeShares>,
    secrets: Arc<MemorySecrets>,
    /// The running engine's in-memory pipe and its client end, kept so the engine's end never sees every connector
    /// gone; `client` hands out clones (Task 26). `start` makes a fresh pair, because the engine's stop closes its
    /// transport (Ruling W10).
    ipc: Mutex<(Arc<MemoryIpc>, MemoryConnector)>,
    /// Everything every engine of this harness wrote to its console writer (`EngineConfig.console`).
    console: Arc<Mutex<Vec<u8>>>,
    host: Arc<FakeHost>,
    http: Arc<RecordedTransport>,
    timers: Arc<ManualTimers>,
    steps: Arc<ScriptedSteps>,
    sent: Arc<Mutex<Vec<Notification>>>,
    /// The stop hook's target and the states it saw since the last `restart`, `run_until` or `run_until_event`.
    hook: Arc<Mutex<(Option<StopAt>, Vec<String>)>>,
    /// The persist fault a test armed, shared with every engine the harness starts (review round 1, V1).
    faults: Arc<Mutex<ArmedFault>>,
    engine: Mutex<Option<EngineHandle>>,
    /// Whether `start` passes pull request 3's binary transport and sink, `use_binary_services`.
    binary_services: AtomicBool,
    /// The native run's thread, which returns `run_with`'s exit code (`new_native`); None for every other harness and
    /// once `native_exit`, `stop` or the drop took it.
    #[cfg(windows)]
    native: Mutex<Option<JoinHandle<i32>>>,
    /// The native run's pipe (`new_native`); None for every other harness.
    #[cfg(windows)]
    pipe: Option<String>,
    /// The console handler the native run installed (`new_native`).
    #[cfg(windows)]
    console_handler: ConsoleHandler,
    /// The process's one native slot, held by a native harness until its drop has stopped its run (`new_native`).
    #[cfg(windows)]
    native_slot: Option<NativeSlot>,
}

impl Harness {
    /// A harness whose steps play `script` (design part 12.1). `data/settings.json` holds the harness defaults
    /// (`staging_root`, `movies_root` and `tuning_root` as the scratch folders, `keep_tuning_evidence` false,
    /// `drive_settle_s` 0, `drive_ready_timeout_s` 1), each key of `settings` replacing its default, and each key
    /// inside `tool_paths` replacing that one (R22.5). The host is `FakeHost::new("2026-10-23", 1_792_800_000)`, the
    /// ejector pushes the removal its eject causes (R22.6), and the secrets hold `tmdb_token`. The engine starts at
    /// once. Def: T22. Use: T22 to T29.
    pub fn new_scripted(script: Vec<(&'static str, StepScript)>, settings: Value) -> Harness {
        let folders = Folders::make();
        let defaults = folders.defaults();
        let runner = Runner::scripted(ScriptedSteps::new(script), folders.root.join("tools"));
        let harness = folders.harness(defaults, settings, runner);
        harness.start();
        harness
    }

    /// A harness whose engine runs the real step runner on stand-in tools (design part 12.2, Task 23).
    /// - It copies `corpus/scenarios/<first>` into `<root>/tools`, then each later folder over it, and copies
    ///   `fake_tool` there as `exe_name(tool)` for every Tool, with an `appdata.tar` holding
    ///   `default_profile_xml()` as `data/default.mmcp.xml`. `tool_paths` names the copies; each key of
    ///   `settings.tool_paths` replaces one (R22.5).
    /// - `insert` of the first scenario's name builds `<root>/volumes/<name>/` from its `disc.json` (`text`,
    ///   hex-decoded `hex`, or `size` zero bytes) and labels it with the file's `label`; any other name gets
    ///   `new_scripted`'s synthetic volume, whose lookups are unrecorded and stay pending.
    /// - The transport is the merged `http.json` through `RecordedTransport::from_file`, with the vars `fingerprint`
    ///   and `disc_id` of the built volume and `search`, the search URL of its disc name.
    /// - Its process guard is `TestGuard` (R23.2): a scan waits up to 5 s for the recorded lookups' files,
    ///   `on_spawn`'s actions run before the spawns they name, and every spawn is kept for `spawns`.
    /// - When the merged scenarios hold `lazymkvctl.json`, it copies `fake_tool` as `lazymkvctl` (with the system's
    ///   executable suffix) into `<root>/tools`, and every engine gets that copy as `EngineConfig.ctl`, which the
    ///   `command` relay runs (design part 9.5, Task T3-22); otherwise `ctl` is None.
    ///
    /// Everything else is as `new_scripted`. Def: T23. Use: T23 to T29.
    pub fn new(fake_tool: &Path, scenarios: &[&str], settings: Value) -> Harness {
        let harness = Harness::prepared(fake_tool, scenarios, settings);
        harness.start();
        harness
    }

    /// `new`'s harness before its engine starts: the scratch folders, the scenario and stand-in copies, the recorded
    /// transport and the `TestGuard` (`new`, `new_native`).
    fn prepared(fake_tool: &Path, scenarios: &[&str], settings: Value) -> Harness {
        let folders = Folders::make();
        let tools = folders.root.join("tools");
        let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("corpus").join("scenarios");
        for name in scenarios {
            copy_folder(&corpus.join(name), &tools)
                .unwrap_or_else(|error| panic!("the harness copies the scenario {name}: {error}"));
        }
        let ctl = tools.join("lazymkvctl.json").is_file().then(|| {
            let copy = tools.join(format!("lazymkvctl{}", std::env::consts::EXE_SUFFIX));
            fs::copy(fake_tool, &copy).unwrap_or_else(|error| panic!("{}: {error}", copy.display()));
            copy
        });
        let mut tool_paths = serde_json::Map::new();
        for tool in Tool::ALL {
            let copy = tools.join(exe_name(tool));
            fs::copy(fake_tool, &copy).unwrap_or_else(|error| panic!("{}: {error}", copy.display()));
            tool_paths.insert(tool.as_str().to_string(), Value::String(copy.display().to_string()));
        }
        let appdata = ustar_build(&[("data/default.mmcp.xml", default_profile_xml().as_bytes())]);
        fs::write(tools.join("appdata.tar"), appdata).expect("the harness writes appdata.tar");
        let disc = scenarios.first().and_then(|first| {
            let text = fs::read(tools.join("disc.json")).ok()?;
            let spec: Value = serde_json::from_slice(&text).expect("the scenario's disc.json is JSON");
            Some((first.to_string(), spec))
        });
        let http_file = tools.join("http.json");
        let (http, urls) = match (&disc, http_file.is_file()) {
            (Some((name, spec)), true) => recorded_http(&http_file, &folders.volumes.join(name), spec),
            (None, true) => (RecordedTransport::from_file(&http_file, &[]).expect("the scenario's http.json"), 0),
            (_, false) => (RecordedTransport::new(), 0),
        };
        let mut defaults = folders.defaults();
        defaults["tool_paths"] = Value::Object(tool_paths);
        let lookups_wait = Arc::new(AtomicBool::new(true));
        let spawn_hooks = SpawnHooks::default();
        let spawns = Arc::new(Mutex::new(Vec::new()));
        let guard = TestGuard {
            wait: Arc::clone(&lookups_wait),
            urls,
            hooks: Arc::clone(&spawn_hooks),
            spawns: Arc::clone(&spawns),
            inner: native_guard(),
        };
        let runner = Runner {
            steps: ScriptedSteps::new(Vec::new()),
            scripted: false,
            processes: Arc::new(guard),
            http,
            tools,
            disc,
            lookups_wait,
            spawn_hooks,
            spawns,
            ctl,
        };
        folders.harness(defaults, settings, runner)
    }

    /// Starts an engine with `start_until` and the stop hook over the harness's data folder and fakes, on a fresh
    /// `MemoryIpc`, so a restarted engine never meets the transport its predecessor's stop closed.
    fn start(&self) {
        let (ipc, connector) = MemoryIpc::new();
        let ipc = Arc::new(ipc);
        *lock(&self.ipc) = (Arc::clone(&ipc), connector);
        let platform = Platform {
            drives: Box::new(SharedDrives { drives: Arc::clone(&self.drives), script: self.script.clone() }),
            ejector: Arc::clone(&self.ejector) as _,
            processes: Arc::clone(&self.processes),
            awake: Arc::clone(&self.awake) as _,
            shares: Arc::clone(&self.shares) as _,
            secrets: Arc::clone(&self.secrets) as _,
            ipc: ipc as _,
            host: Arc::clone(&self.host) as _,
        };
        let (http, notify): (Arc<dyn HttpTransport>, Box<dyn NotifySink>) =
            if self.binary_services.load(Ordering::SeqCst) {
                (Arc::new(OfflineTransport), Box::new(NoDelivery))
            } else {
                (Arc::clone(&self.http) as _, Box::new(MemorySink { list: Arc::clone(&self.sent) }))
            };
        let services = Services {
            platform,
            http,
            notify,
            timers: Arc::clone(&self.timers) as _,
            steps: self.scripted.then(|| Arc::clone(&self.steps) as _),
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
            ctl: self.ctl.clone(),
            console: Some(Box::new(ConsoleBuffer(Arc::clone(&self.console)))),
        };
        let hook = Arc::clone(&self.hook);
        let stop_after: StopAfter = Arc::new(move |transition: &Transition| {
            let mut hook = lock(&hook);
            let state = transition.new.as_str();
            hook.1.push(state.to_string());
            let stop = match &hook.0 {
                Some(StopAt::State(target)) => target == state,
                Some(StopAt::Event(accepts)) => transition.event.as_ref().is_some_and(accepts),
                None => false,
            };
            if stop {
                hook.0 = None;
            }
            stop
        });
        let armed = Arc::clone(&self.faults);
        let persist_faults: PersistFaults = Arc::new(move |record: &JobRecord, transition: &Transition| {
            let armed = lock(&armed);
            armed.as_ref().filter(|(_, applies)| applies(record, transition)).map(|(fault, _)| *fault)
        });
        let hooks = TestHooks { stop_after: Some(stop_after), persist_faults: Some(persist_faults) };
        let engine = start_until(config, services, hooks).expect("the harness's engine starts");
        *lock(&self.engine) = Some(engine);
    }

    /// Stops the running engine, if any, and joins its threads; a native run still going is stopped as the drop stops
    /// it (Ruling T3-23-c).
    fn stop(&self) {
        #[cfg(windows)]
        if let Some(outcome) = self.stop_native() {
            assert_eq!(outcome, Ok(0), "the harness's native engine stops");
        }
        let engine = lock(&self.engine).take();
        if let Some(engine) = engine {
            engine.stop().expect("the harness's engine stops");
        }
    }

    /// Hands the queued drive events to the `DriveScript`, in order (R22.1). Every other method does this first; a test
    /// that inserts or removes a disc and then waits only through an IPC client calls it itself (PF-1). Def: T22.
    /// Use: T22 to T29.
    pub fn deliver(&self) {
        let queued = std::mem::take(&mut *lock(&self.queued));
        for event in queued {
            match event {
                Queued::Arrive(drive, root) => self.script.arrive(&drive, &root),
                Queued::Remove(drive) => self.script.remove(&drive),
            }
        }
    }

    /// `<root>/volumes/<disc>`, built on first use. The first scenario's name is built from its `disc.json` and
    /// labelled with its `label` (`Harness::new`). Any other name gets `BDMV/index.bdmv` "INDX0200",
    /// `BDMV/PLAYLIST/00800.mpls` "MPLS0200", `BDMV/STREAM/00001.m2ts` of 1000 plus the sum of the name's bytes zero
    /// bytes and `AACS/Unit_Key_RO.inf` the name's bytes, labelled with the name in upper case.
    fn volume(&self, disc: &str) -> PathBuf {
        let root = self.volumes.join(disc);
        if let Some((_, spec)) = self.disc.as_ref().filter(|(name, _)| name == disc) {
            build_volume(&root, spec).unwrap_or_else(|error| panic!("{}: {error}", root.display()));
            self.host.set_label(&root, &label_of(spec));
            return root;
        }
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

    /// `<root>/volumes/<disc>`, built and labelled as `insert` builds it, so a test can change the disc's files before
    /// it is inserted (review round 1, V10: read-only files, as a disc's are). Def: PR #6 review (V10). Use:
    /// engine_restart.rs.
    pub fn volume_root(&self, disc: &str) -> PathBuf {
        self.volume(disc)
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

    /// Makes every persist that `refused` accepts, asked with the record and the transition about to be written, write
    /// nothing and fail with `PermissionDenied`, "refused by the test", until `allow_persists`, as a refused rename or
    /// a full data volume would (review round 1, V1). It replaces any fault armed before, and holds across `restart`,
    /// so it can be armed for a restart's EngineRestart lines. Def: PR #6 review (V1). Use: owner/tests.rs,
    /// engine_ipc.rs.
    pub fn refuse_persists(&self, refused: impl Fn(&JobRecord, &Transition) -> bool + Send + Sync + 'static) {
        self.deliver();
        *lock(&self.faults) = Some((PersistFault::Refuse, Box::new(refused)));
    }

    /// Makes every persist that `lost` accepts replace state.json and append no transitions line, as a failed append
    /// would, until `allow_persists` (review round 1, V1, RR1-3). It replaces any fault armed before. Def: PR #6 review
    /// (V1). Use: owner/tests.rs.
    pub fn lose_log_lines(&self, lost: impl Fn(&JobRecord, &Transition) -> bool + Send + Sync + 'static) {
        self.deliver();
        *lock(&self.faults) = Some((PersistFault::LoseLogLine, Box::new(lost)));
    }

    /// Lets every persist work again (review round 1, V1). Def: PR #6 review (V1). Use: owner/tests.rs, engine_ipc.rs.
    pub fn allow_persists(&self) {
        self.deliver();
        *lock(&self.faults) = None;
    }

    /// Arms the stop hook for the first transition into `state`, hands the queued drive events to the engine, waits
    /// (60 s cap, 240 s under CI as `ci_factor` says, then panics naming `state` and the states persisted so far) until
    /// the owner stopped after persisting that transition and before running any of its actions, and joins the
    /// engine's threads. The hook also logs every state it sees; when that log, kept since the last `restart` or
    /// `run_until`, already holds `state` (possible only right after `restart`, whose resumed actions start at once),
    /// it stops the engine at once instead. Def: T22. Use: T22 to T29.
    pub fn run_until(&self, state: &str) {
        let passed = {
            let mut hook = lock(&self.hook);
            let passed = hook.1.iter().any(|seen| seen == state);
            if !passed {
                hook.0 = Some(StopAt::State(state.to_string()));
            }
            hook.1.clear();
            passed
        };
        if !passed {
            self.deliver();
            self.wait_for_stop(&format!("run_until({state})"), &format!("into {state}"));
        }
        self.stop();
    }

    /// Arms the stop hook for the first persisted transition whose event `stop` accepts, hands the queued drive events
    /// to the engine, waits as `run_until` does (60 s cap, 240 s under CI, then panics naming `what` and the states
    /// persisted so far) until the owner stopped after persisting that transition and before running any of its
    /// actions, and joins the engine's threads. It has no shortcut for an event seen before it was armed, so a test arms
    /// it before the event can happen. A transition that keeps its state, as a TitleVerifyFailed in Ripping does, can be
    /// stopped at only this way (review round 1, V12). Def: PR #6 review (V12). Use: engine_restart.rs.
    pub fn run_until_event(&self, what: &str, stop: impl Fn(&Event) -> bool + Send + Sync + 'static) {
        {
            let mut hook = lock(&self.hook);
            hook.0 = Some(StopAt::Event(Box::new(stop)));
            hook.1.clear();
        }
        self.deliver();
        self.wait_for_stop(&format!("run_until_event({what})"), &format!("with {what}"));
        self.stop();
    }

    /// Waits until the armed stop hook has fired (60 s cap, 240 s under CI), then returns; past the cap it disarms the
    /// hook and panics with `<call>: no transition <which> within <cap>; persisted: <states>`.
    fn wait_for_stop(&self, call: &str, which: &str) {
        let cap = RUN_UNTIL_CAP * ci_factor();
        let deadline = Instant::now() + cap;
        loop {
            let (armed, seen) = {
                let hook = lock(&self.hook);
                (hook.0.is_some(), hook.1.clone())
            };
            if !armed {
                return;
            }
            if Instant::now() >= deadline {
                lock(&self.hook).0 = None;
                panic!("{call}: no transition {which} within {cap:?}; persisted: {seen:?}");
            }
            thread::sleep(POLL);
        }
    }

    /// Stops a running engine (`EngineHandle::stop`), drops every pending ManualTimers timer, as a real restart loses
    /// its sleeping threads, and starts a new engine (`start_until`) over the same data folder with the same drive
    /// script, ejector, keep-awake, shares, secrets, a new MemoryIpc, host, RecordedTransport, ScriptedSteps (used
    /// entries stay used) and MemorySink list. The queued drive events are handed over once the old engine has stopped,
    /// so the new engine reads them (R22.9). A native harness's restart panics (Ruling T3-23-c). Def: T22. Use: T22 to
    /// T29.
    pub fn restart(&self) {
        #[cfg(windows)]
        if self.pipe.is_some() {
            panic!("restart is not for a native harness");
        }
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

    /// Polls every 20 ms until `done` holds; panics naming `what` after 10 s, 40 s under CI (`ci_factor`). Def: T22.
    /// Use: T22 to T29.
    pub fn wait(&self, what: &str, done: impl Fn(&Harness) -> bool) {
        self.deliver();
        let cap = WAIT_CAP * ci_factor();
        let deadline = Instant::now() + cap;
        while !done(self) {
            if Instant::now() >= deadline {
                panic!("waited {cap:?} for {what}");
            }
            thread::sleep(POLL);
        }
    }

    /// Returns once the job files, notifications, engine.log, drives.json, ejects, step calls and keep-awake holders
    /// are unchanged for 300 ms, or for 2 s while the running engine has a step or lookup round outstanding (10 s
    /// cap). A step can pass 300 ms with no file changing, as a stand-in's first launch does, and a step that hangs on
    /// purpose ends the wait after 2 s (Ruling R24.c1). Def: T22. Use: T22 to T29.
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
                let busy = lock(&self.engine).as_ref().is_some_and(EngineHandle::busy);
                if !busy || since.elapsed() >= BUSY_QUIET {
                    return;
                }
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

    /// Waits until at least one timer is pending, then runs and drops every pending timer (design part 5.2). A
    /// transition is persisted before its actions run and StartTimer is one of them, so a test that fires as soon as
    /// it sees the state could otherwise find no timer and leave the job waiting for good (Ruling R29.c2a). Panics
    /// when no timer becomes pending within 10 s, 40 s under CI (`ci_factor`). Def: T22. Use: T22 to T29.
    pub fn fire_timers(&self) {
        self.deliver();
        let cap = WAIT_CAP * ci_factor();
        let deadline = Instant::now() + cap;
        while self.timers.pending().is_empty() {
            if Instant::now() >= deadline {
                panic!("fire_timers: no timer became pending within {} s", cap.as_secs());
            }
            thread::sleep(POLL);
        }
        self.timers.fire_all();
    }

    /// The merged scenario folder, `<root>/tools`, holding the tool copies (design part 12.2). Def: T23. Use: T23 to
    /// T29.
    pub fn tools(&self) -> &Path {
        self.deliver();
        &self.tools
    }

    /// Whether a scan waits for the recorded lookups' files, true by default (R23.2). Def: T23. Use: T23 to T29.
    pub fn scan_waits_for_lookups(&self, wait: bool) {
        self.deliver();
        self.lookups_wait.store(wait, Ordering::SeqCst);
    }

    /// From the next `start`, that is the next `restart`, the engine gets pull request 3's binary transport and sink
    /// (Ruling W26): `OfflineTransport` as `Services.http` and `NoDelivery` as `Services.notify`, so `http()` records
    /// no request and `notifications()` stays empty. Def: T3-21. Use: T3-21.
    pub fn use_binary_services(&self) {
        self.deliver();
        self.binary_services.store(true, Ordering::SeqCst);
    }

    /// A clone of the harness's `MemoryConnector`, which connects an IPC client to the running engine (design part
    /// 9.1). Def: T26. Use: T26, T28.
    pub fn client(&self) -> MemoryConnector {
        self.deliver();
        lock(&self.ipc).1.clone()
    }

    /// Everything the engines of this harness wrote to their console writer (`EngineConfig.console`), as text (design
    /// part 3.5). Def: T3-10. Use: T3-10, T3-23.
    pub fn console_text(&self) -> String {
        self.deliver();
        String::from_utf8_lossy(&lock(&self.console)).into_owned()
    }

    /// The running engine's `EngineHandle::shutdown_handle`. Def: T3-10. Use: T3-10.
    pub fn shutdown_handle(&self) -> ShutdownHandle {
        self.deliver();
        lock(&self.engine).as_ref().expect("an engine runs").shutdown_handle()
    }

    /// The running engine's `EngineHandle::wait_for_exit(Some(timeout))`; true when no engine runs. Def: T3-10.
    /// Use: T3-10.
    pub fn wait_for_exit(&self, timeout: Duration) -> bool {
        self.deliver();
        lock(&self.engine).as_ref().is_none_or(|engine| engine.wait_for_exit(Some(timeout)))
    }

    /// Registers `action` to run once, before the first spawn whose arguments hold every one of `needles`, each as a
    /// substring of some argument (as the stand-ins match theirs), and then lets that spawn go ahead (design part
    /// 12.2). Only `new`'s `TestGuard` runs the actions. Def: T24. Use: T24.
    pub fn on_spawn(&self, needles: &[&str], action: impl Fn() + Send + Sync + 'static) {
        self.deliver();
        let needles = needles.iter().map(|needle| needle.to_string()).collect();
        lock(&self.spawn_hooks).push((needles, Some(Box::new(action))));
    }

    /// Every `Spawn` the harness's `TestGuard` saw, in order (design part 12.2): the stand-in tools' and the relay's
    /// `lazymkvctl`. Empty for `new_scripted`, whose guard is not a `TestGuard`. Def: T3-22. Use: T3-22.
    pub fn spawns(&self) -> Vec<Spawn> {
        self.deliver();
        lock(&self.spawns).clone()
    }
}

#[cfg(windows)]
impl Harness {
    /// A harness whose engine is the Windows binary's own start, `native::run_with`, on a thread (design parts 3.1
    /// to 3.8; Ruling W27). The stand-ins, scenarios and settings are `new`'s; the data folder is made again through
    /// `protect_data_folder` with the same settings.json (Ruling T3-23-b). `run_with` gets `["--data", <data>]`, a
    /// buffer of its own for standard error, and `NativeOptions { mutex: Global\<the pipe's name after
    /// \\.\pipe\>, pipe, drives: the harness's SharedDrives, ejector: its RecordingEjector with removals, processes:
    /// its TestGuard, secrets: its MemorySecrets, http: its recorded transport, console: its console buffer,
    /// console_stop: a closure that keeps the handler, token: None }`. The real WinHost, WinKeepAwake and share
    /// connector run, so `host()`, `awake()` and `shares()` are not this engine's. It returns once a client through
    /// `pipe_client()` has read its snapshot line (60 s cap, 240 s under continuous integration, CI), and panics with
    /// `run_with`'s standard error when `run_with` returned first. `restart`, `run_until` and `run_until_event` are not
    /// for it. Native harnesses of one test process run one at a time (`NativeSlot`): the real WinHost shows S55 every
    /// running `makemkvcon64`, another native harness's stand-in included, so it first waits until no other is alive.
    /// Def: T3-23. Use: T3-23.
    pub fn new_native(fake_tool: &Path, scenarios: &[&str], settings: Value, pipe: &str) -> Harness {
        let slot = NativeSlot::take();
        let mut harness = Harness::prepared(fake_tool, scenarios, settings);
        harness.native_slot = Some(slot);
        let name = pipe.strip_prefix(r"\\.\pipe\").unwrap_or_else(|| panic!("{pipe} is not a pipe's name"));
        let mutex = format!(r"Global\{name}");
        harness.pipe = Some(pipe.to_string());
        // Ruling T3-23-b: a folder the test process made need not be owned by Administrators, which
        // `protect_data_folder` requires, so it makes the folder itself.
        let settings_file = harness.data.join("settings.json");
        let written = fs::read(&settings_file).expect("the harness reads data/settings.json");
        fs::remove_dir_all(&harness.data).expect("the harness removes the data folder it made");
        let sid = own_token().expect("the harness reads its own token").user_sid;
        protect_data_folder(&harness.data, &sid).unwrap_or_else(|text| panic!("the harness's data folder: {text}"));
        fs::write(&settings_file, written).expect("the harness writes data/settings.json again");
        let drives = SharedDrives { drives: Arc::clone(&harness.drives), script: harness.script.clone() };
        let kept = Arc::clone(&harness.console_handler);
        let console_stop = move |handler: Box<dyn Fn() + Send + Sync>| -> io::Result<()> {
            *lock(&kept) = Some(handler);
            Ok(())
        };
        let options = NativeOptions {
            mutex,
            pipe: pipe.to_string(),
            drives: Some(Box::new(drives)),
            ejector: Some(Arc::clone(&harness.ejector) as _),
            processes: Some(Arc::clone(&harness.processes)),
            secrets: Some(Arc::clone(&harness.secrets) as _),
            http: Some(Arc::clone(&harness.http) as _),
            console: Some(Box::new(ConsoleBuffer(Arc::clone(&harness.console)))),
            console_stop: Some(Box::new(console_stop)),
            token: None,
        };
        let args = vec!["--data".to_string(), harness.data.display().to_string()];
        let stderr = Arc::new(Mutex::new(Vec::new()));
        let mut err = ConsoleBuffer(Arc::clone(&stderr));
        let run = thread::spawn(move || run_with(&args, &mut err, options));
        *lock(&harness.native) = Some(run);
        harness.wait_for_snapshot(&stderr);
        harness
    }

    /// The real pipe client of this harness's pipe (design part 3.3). Def: T3-23. Use: T3-23.
    pub fn pipe_client(&self) -> PipeConnector {
        self.deliver();
        PipeConnector::new(self.pipe.as_deref().expect("pipe_client is for a native harness"))
    }

    /// Calls the console handler `run_with` installed, as Ctrl+C would (design part 3.8; Ruling W19); panics when
    /// none was installed. Def: T3-23. Use: T3-23.
    pub fn console_stop_event(&self) {
        self.deliver();
        let handler = lock(&self.console_handler);
        let handler = handler.as_ref().expect("run_with installed a console handler");
        handler();
    }

    /// Waits for `run_with` to return (60 s cap, 240 s under CI) and gives its exit code; the drop then has nothing
    /// to stop (Ruling T3-23-c). Def: T3-23. Use: T3-23.
    pub fn native_exit(&self) -> i32 {
        self.deliver();
        let run = lock(&self.native).take().expect("a native run is left to wait for");
        match join_native(run) {
            Ok(Ok(code)) => code,
            Ok(Err(payload)) => panic::resume_unwind(payload),
            Err(run) => {
                *lock(&self.native) = Some(run);
                panic!("native_exit: run_with did not return within {:?}", RUN_UNTIL_CAP * ci_factor());
            }
        }
    }

    /// `new_native`'s wait: a helper thread connects through `pipe_client()`, sleeping 20 ms after each `NotRunning`,
    /// and sends the first line it reads; a snapshot ends the wait (60 s cap, 240 s under CI). When `run_with` returns
    /// first it panics with `stderr`, what `run_with` wrote to its standard error.
    fn wait_for_snapshot(&self, stderr: &Mutex<Vec<u8>>) {
        let cap = RUN_UNTIL_CAP * ci_factor();
        let deadline = Instant::now() + cap;
        let connector = self.pipe_client();
        let (send, first) = channel();
        thread::spawn(move || {
            let line = loop {
                match connector.connect() {
                    Ok(mut stream) => {
                        let mut line = String::new();
                        break stream.reader.read_line(&mut line).map(|_| line).map_err(|error| error.to_string());
                    }
                    Err(ConnectError::NotRunning) if Instant::now() < deadline => thread::sleep(POLL),
                    Err(error) => break Err(format!("the client could not connect: {error:?}")),
                }
            };
            let _ = send.send(line);
        });
        loop {
            if let Ok(line) = first.try_recv() {
                match line {
                    Ok(line) if line.contains(r#""type":"snapshot""#) => return,
                    other => panic!("new_native: the pipe's first line is not a snapshot: {other:?}"),
                }
            }
            let returned = lock(&self.native).take_if(|run| run.is_finished());
            if let Some(run) = returned {
                let code = run.join().map_err(|payload| panic_text(payload.as_ref()));
                let text = String::from_utf8_lossy(&lock(stderr)).into_owned();
                panic!("new_native: run_with returned {code:?} before the pipe answered; its standard error: {text}");
            }
            assert!(Instant::now() < deadline, "new_native: no snapshot through the pipe within {cap:?}");
            thread::sleep(POLL);
        }
    }

    /// Ruling T3-23-c: a native run still going gets a forced `shutdown` through the pipe (a connect that finds no
    /// engine skips it), held open until the run ends, and its thread is joined (60 s cap, 240 s under CI). Its exit
    /// code, or why there is none; None when no native run is left.
    fn stop_native(&self) -> Option<Result<i32, String>> {
        let run = lock(&self.native).take()?;
        let mut client = None;
        if !run.is_finished() {
            if let Ok(mut stream) = self.pipe_client().connect() {
                let request = json!({"v": PROTOCOL_VERSION, "type": "shutdown", "id": 1, "payload": {"force": true}});
                let _ = writeln!(stream.writer, "{request}");
                let _ = stream.writer.flush();
                client = Some(stream);
            }
        }
        let outcome = match join_native(run) {
            Ok(Ok(code)) => Ok(code),
            Ok(Err(payload)) => Err(format!("run_with panicked: {}", panic_text(payload.as_ref()))),
            Err(_) => Err(format!("run_with did not return within {:?}", RUN_UNTIL_CAP * ci_factor())),
        };
        drop(client);
        Some(outcome)
    }
}

/// Polls `run` every 20 ms until it has finished (60 s cap, 240 s under CI) and joins it; past the cap, Err hands it
/// back unjoined.
#[cfg(windows)]
fn join_native(run: JoinHandle<i32>) -> Result<thread::Result<i32>, JoinHandle<i32>> {
    let deadline = Instant::now() + RUN_UNTIL_CAP * ci_factor();
    while !run.is_finished() {
        if Instant::now() >= deadline {
            return Err(run);
        }
        thread::sleep(POLL);
    }
    Ok(run.join())
}

impl Drop for Harness {
    /// Stops a running engine and removes the scratch folder. An engine thread that panicked fails the test here, as
    /// `run_until` and `restart` do, unless the test is already panicking (ruling at the final review). A native run
    /// still going is stopped first with a forced `shutdown` through its pipe, and an exit code other than 0 fails the
    /// test the same way (Ruling T3-23-c).
    fn drop(&mut self) {
        #[cfg(windows)]
        let native = self.stop_native();
        let engine = lock(&self.engine).take();
        let stopped = engine.map(EngineHandle::stop);
        let _ = fs::remove_dir_all(&self.root);
        if let Some(Err(error)) = stopped {
            if !thread::panicking() {
                panic!("the harness's engine stops: {error}");
            }
        }
        #[cfg(windows)]
        if let Some(outcome) = native.filter(|_| !thread::panicking()) {
            assert_eq!(outcome, Ok(0), "the harness's native engine stops");
        }
    }
}

/// The harness's scratch folders, `<temp>/engine-harness-<pid>-<n>/{data, staging, library/Movies, tuning, volumes}`,
/// made empty (PF-2 names the root).
struct Folders {
    root: PathBuf,
    data: PathBuf,
    staging: PathBuf,
    library: PathBuf,
    tuning: PathBuf,
    volumes: PathBuf,
}

impl Folders {
    fn make() -> Folders {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!("engine-harness-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let folders = Folders {
            data: root.join("data"),
            staging: root.join("staging"),
            library: root.join("library").join("Movies"),
            tuning: root.join("tuning"),
            volumes: root.join("volumes"),
            root,
        };
        for dir in [&folders.data, &folders.staging, &folders.library, &folders.tuning, &folders.volumes] {
            fs::create_dir_all(dir).expect("the harness makes its scratch folders");
        }
        folders
    }

    /// The harness's settings defaults (R22.5): the scratch folders as `staging_root`, `movies_root` and
    /// `tuning_root`, `keep_tuning_evidence` false, `drive_settle_s` 0 and `drive_ready_timeout_s` 1.
    fn defaults(&self) -> Value {
        let text = |path: &Path| Value::String(path.display().to_string());
        json!({
            "staging_root": text(&self.staging), "movies_root": text(&self.library), "tuning_root": text(&self.tuning),
            "keep_tuning_evidence": false, "drive_settle_s": 0, "drive_ready_timeout_s": 1,
        })
    }

    /// The harness over these folders, its `data/settings.json` holding `defaults` with `settings` merged in (R22.5).
    fn harness(self, defaults: Value, settings: Value, runner: Runner) -> Harness {
        let mut merged = defaults;
        merge_settings(&mut merged, settings);
        let written = serde_json::to_vec_pretty(&merged).expect("the harness settings serialise");
        fs::write(self.data.join("settings.json"), written).expect("the harness writes data/settings.json");
        let (drives, script_end) = ScriptedDrives::new();
        let (ipc, connector) = MemoryIpc::new();
        Harness {
            root: self.root,
            data: self.data,
            staging: self.staging,
            library: self.library,
            volumes: self.volumes,
            tools: runner.tools,
            disc: runner.disc,
            lookups_wait: runner.lookups_wait,
            spawn_hooks: runner.spawn_hooks,
            spawns: runner.spawns,
            ctl: runner.ctl,
            scripted: runner.scripted,
            ejector: Arc::new(RecordingEjector::new().with_removals(script_end.clone())),
            script: script_end,
            drives: Arc::new(Mutex::new(drives)),
            queued: Mutex::default(),
            processes: runner.processes,
            awake: Arc::new(CountingKeepAwake::new()),
            shares: Arc::new(FakeShares::new()),
            secrets: Arc::new(MemorySecrets::new().with("tmdb_token", "test-token")),
            ipc: Mutex::new((Arc::new(ipc), connector)),
            console: Arc::default(),
            host: Arc::new(FakeHost::new("2026-10-23", 1_792_800_000)),
            http: Arc::new(runner.http),
            timers: Arc::new(ManualTimers::new()),
            steps: Arc::new(runner.steps),
            sent: Arc::default(),
            hook: Arc::default(),
            faults: Arc::default(),
            engine: Mutex::default(),
            binary_services: AtomicBool::new(false),
            #[cfg(windows)]
            native: Mutex::default(),
            #[cfg(windows)]
            pipe: None,
            #[cfg(windows)]
            console_handler: Arc::default(),
            #[cfg(windows)]
            native_slot: None,
        }
    }
}

/// What `new_scripted` and `new` set differently.
struct Runner {
    steps: ScriptedSteps,
    scripted: bool,
    processes: Arc<dyn ProcessGuard>,
    http: RecordedTransport,
    tools: PathBuf,
    disc: Option<(String, Value)>,
    lookups_wait: Arc<AtomicBool>,
    spawn_hooks: SpawnHooks,
    /// The `TestGuard`'s list of spawns.
    spawns: Arc<Mutex<Vec<Spawn>>>,
    /// The stand-in `lazymkvctl`, when a scenario holds `lazymkvctl.json`.
    ctl: Option<PathBuf>,
}

impl Runner {
    /// `new_scripted`'s: the script, `native_guard()`, a transport with no answers, no scenario, no spawn list and no
    /// `lazymkvctl`.
    fn scripted(steps: ScriptedSteps, tools: PathBuf) -> Runner {
        Runner {
            steps,
            scripted: true,
            processes: native_guard(),
            http: RecordedTransport::new(),
            tools,
            disc: None,
            lookups_wait: Arc::new(AtomicBool::new(true)),
            spawn_hooks: SpawnHooks::default(),
            spawns: Arc::default(),
            ctl: None,
        }
    }
}

/// Ruling R23.2: stand-in runs are faster than the recorded lookups' file writes. Before it spawns a scan (arguments
/// holding `info` and a `--profile=` argument), this guard waits, at most 5 s, until `<the profile's
/// folder>/lookup-status.txt` has one line per distinct URL of the merged `http.json` and no line ends in `000`. The
/// engine's planning still never waits, and the whole-job tests are deterministic. It relies on `run_round` applying
/// each outcome to the book before it saves the status file. Before any spawn it runs the first `on_spawn` action not
/// yet run whose needles the arguments hold (Task 24). Every spawn then goes to `native_guard()` through `retry_busy`
/// (Ruling R29.c4): the Job Object runner on Windows, so every Windows whole fake job runs it (Ruling W7), and
/// `StdProcessGuard` elsewhere. Before all of that it keeps a copy of the spawn for `Harness::spawns` (Task T3-22).
struct TestGuard {
    /// `scan_waits_for_lookups`.
    wait: Arc<AtomicBool>,
    /// The distinct URLs of the merged `http.json`; 0 waits for nothing.
    urls: usize,
    /// `Harness::on_spawn`'s actions.
    hooks: SpawnHooks,
    /// Every spawn asked for, in order (`Harness::spawns`).
    spawns: Arc<Mutex<Vec<Spawn>>>,
    /// `native_guard()`, which starts every spawn.
    inner: Arc<dyn ProcessGuard>,
}

impl ProcessGuard for TestGuard {
    fn spawn(&self, spawn: &Spawn) -> io::Result<Box<dyn GuardedChild>> {
        lock(&self.spawns).push(spawn.clone());
        let action = lock(&self.hooks)
            .iter_mut()
            .find(|(needles, action)| {
                action.is_some()
                    && needles.iter().all(|needle| spawn.args.iter().any(|arg| arg.contains(needle.as_str())))
            })
            .and_then(|(_, action)| action.take());
        if let Some(action) = action {
            action();
        }
        let scan = spawn.args.iter().any(|arg| arg == "info");
        let profile = spawn.args.iter().find_map(|arg| arg.strip_prefix("--profile="));
        if let (true, Some(profile)) = (scan && self.urls > 0 && self.wait.load(Ordering::SeqCst), profile) {
            let status = Path::new(profile).parent().map(|folder| folder.join("lookup-status.txt"));
            let deadline = Instant::now() + LOOKUP_WAIT_CAP;
            while !status.as_deref().is_some_and(|status| answered(status, self.urls)) && Instant::now() < deadline {
                thread::sleep(POLL);
            }
        }
        retry_busy(|| self.inner.spawn(spawn))
    }
}

/// Whether `status` has `urls` lines and none ends in `000` (R23.2).
fn answered(status: &Path, urls: usize) -> bool {
    fs::read_to_string(status).is_ok_and(|text| {
        let lines: Vec<&str> = text.lines().filter(|line| !line.trim().is_empty()).collect();
        lines.len() == urls && !lines.iter().any(|line| line.trim_end().ends_with("000"))
    })
}

/// Copies every file under `from` into `to`, keeping relative paths and replacing files already there.
fn copy_folder(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_folder(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// A scenario `disc.json`'s `label`, "" when absent.
fn label_of(spec: &Value) -> String {
    spec["label"].as_str().unwrap_or_default().to_string()
}

/// `text` as bytes, two hex digits each.
fn decode_hex(text: &str) -> io::Result<Vec<u8>> {
    let invalid = || io::Error::new(io::ErrorKind::InvalidData, format!("{text:?} is not hex"));
    if !text.is_ascii() || text.len() % 2 != 0 {
        return Err(invalid());
    }
    (0..text.len()).step_by(2).map(|at| u8::from_str_radix(&text[at..at + 2], 16).map_err(|_| invalid())).collect()
}

/// Builds the volume at `root` from a scenario's `disc.json` (`files`, each a `path` and one of `text`, `hex` or
/// `size` zero bytes), unless `root` is already there.
fn build_volume(root: &Path, spec: &Value) -> io::Result<()> {
    if root.is_dir() {
        return Ok(());
    }
    for file in spec["files"].as_array().into_iter().flatten() {
        let relative = file["path"].as_str().ok_or_else(|| io::Error::other("a disc.json file has no path"))?;
        let bytes = match (file["text"].as_str(), file["hex"].as_str()) {
            (Some(text), _) => text.as_bytes().to_vec(),
            (None, Some(hex)) => decode_hex(hex)?,
            (None, None) => vec![0u8; usize::try_from(file["size"].as_u64().unwrap_or(0)).unwrap_or(0)],
        };
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap_or(root))?;
        fs::write(&path, bytes)?;
    }
    Ok(())
}

/// The merged `http.json` with the vars of the scenario's disc, built at `root` (S31): `fingerprint` and `disc_id` in
/// upper-case hex, and `search`, `search_url` of the cleaned disc name from `bdmt_eng.xml` and the label; with the
/// number of distinct URLs in the file.
fn recorded_http(file: &Path, root: &Path, spec: &Value) -> (RecordedTransport, usize) {
    build_volume(root, spec).unwrap_or_else(|error| panic!("{}: {error}", root.display()));
    let files = list_files(root).expect("the harness lists the scenario's volume");
    let unit_key = unit_key_file(&files).and_then(|key| fs::read(root.join(&key.path)).ok());
    let label = label_of(spec);
    let meta = disc_meta(&files, &label, unit_key.as_deref(), 0);
    let bdmt = spec["files"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|file| file["path"].as_str().is_some_and(|path| path.ends_with("bdmt_eng.xml")))
        .and_then(|file| file["text"].as_str())
        .and_then(bdmt_name);
    let name = disc_name(bdmt.as_deref(), Some(&label)).expect("the scenario's disc has a name");
    let search = search_url(&name.cleaned);
    let fingerprint = meta.fingerprint.unwrap_or_default();
    let disc_id = meta.disc_id.unwrap_or_default();
    let vars = [("fingerprint", fingerprint.as_str()), ("disc_id", disc_id.as_str()), ("search", search.as_str())];
    let transport =
        RecordedTransport::from_file(file, &vars).unwrap_or_else(|error| panic!("the scenario's http.json: {error}"));
    let text = fs::read_to_string(file).expect("the scenario's http.json");
    let document: Value = serde_json::from_str(&text).expect("the scenario's http.json is JSON");
    let urls: BTreeSet<&str> =
        document["answers"].as_array().into_iter().flatten().filter_map(|answer| answer["url"].as_str()).collect();
    (transport, urls.len())
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
