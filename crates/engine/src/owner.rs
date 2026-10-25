//! The job-owner thread (design parts 4.4, 5.2, 5.4, 5.7, 7.3, 8.3; K8, L2): it holds every job, runs
//! `core::job::reduce` on each event, persists the transition before any of its actions run, performs the actions that
//! only touch the record or the notifier itself and hands every other action to a `StepRunner` worker. It also routes
//! arrivals, keeps the one disc-phase token with its queue and the file-phase slot with its queue, runs the lookup
//! rounds of design part 8.3, and holds KeepAwake while a job is active.

use std::any::Any;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use lazymkv_clients::{HttpTransport, LookupBook};
use lazymkv_core::ipc::{job_summary, JobSummary, Notification};
use lazymkv_core::job::{
    job_id, job_lookup, new_job, reduce, Action, Event, FailureClass, JobLookup, NotifyEvent, State, NETWORK_RETRY_MIN,
};
use lazymkv_core::profile::JOB_PROFILE_FILE;
use lazymkv_core::record::{DriveRecord, DrivesFile, JobRecord, Transition, DRIVES_FORMAT, JOB_FORMAT};
use lazymkv_core::review::{Note, Noted};
use lazymkv_core::settings::Settings;
use lazymkv_core::snapshot::DiscMeta;
use lazymkv_platform::{DriveEvent, DriveWatcher, Ejector, Host, KeepAwake, Platform, SecretStore};
use lazymkv_tools::CancelToken;

use crate::arrival::{spawn_arrival, spawn_recording, Arrival};
use crate::file_phase::{refreshed_plan, waiting_message, SlotQueue};
use crate::notify::{self, spawn_notifier, NotifySink};
use crate::space::{Promises, PromisedStep};
use crate::steps::{spawn_round, RealSteps};
use crate::store::Store;
use crate::timers::Timers;
use crate::EngineConfig;

/// How long the drive thread waits for each drive event before it looks at its stop flag again.
const DRIVE_POLL: Duration = Duration::from_millis(50);
/// How often `EngineHandle::wait_until` looks at the jobs.
const WAIT_POLL: Duration = Duration::from_millis(20);

/// The actions whose step returns no event, so their result writes no `returned no event` line (design part 5.2, PF-6:
/// `ExtractSubtitleEvidence` runs with `Decide` as one unit). Def: T22. Use: T22, T24.
pub(crate) const NO_EVENT_ACTIONS: [&str; 3] = ["EmptyTitleDirectory", "DeletePartialFile", "ExtractSubtitleEvidence"];

/// The start of the message of a step, recording or eject that panicked (R22.11): the owner writes an engine.log line
/// naming the job for every result whose message holds it. Def: T22. Use: T22.
pub(crate) const PANICKED: &str = "the step panicked: ";

/// The text a caught panic carries: its `&str` or `String` payload, else a fixed sentence (R22.11). Def: T22. Use: T22.
pub(crate) fn panic_text(payload: &(dyn Any + Send)) -> String {
    match (payload.downcast_ref::<&str>(), payload.downcast_ref::<String>()) {
        (Some(text), _) => text.to_string(),
        (None, Some(text)) => text.clone(),
        (None, None) => "a panic with no message".to_string(),
    }
}

/// A step's outcome (design part 5.2): the event to feed, if any, and the sentence for the job record and notification.
/// Def: T22. Use: T23, T24, T26.
#[derive(Clone, Debug, PartialEq)]
pub struct StepResult {
    /// The event the owner feeds to the reducer; None for a step that reports none (design part 5.2).
    pub event: Option<Event>,
    /// One plain sentence: the job's message when the event leads to Failed or a waiting state.
    pub message: Option<String>,
}

/// What a step sees of its job: a copy of the record, the job folder, the job's own settings (ENG-19), the drive's
/// volume root while its disc is in, and the lookup book (empty until the job's first RunScan, Task 23). Def: T22.
/// Use: T23, T24.
pub struct JobContext {
    /// The job's record as the owner held it when it handed the step.
    pub record: JobRecord,
    /// `<data>/jobs/<id>` (design part 7.1).
    pub job_dir: PathBuf,
    /// The settings the job started with (ENG-19).
    pub settings: Settings,
    /// The volume root of the drive holding the job's disc, None while the disc is out.
    pub drive_root: Option<PathBuf>,
    /// The job's lookups (design part 8.2).
    pub lookups: Arc<Mutex<LookupBook>>,
}

/// Runs every action the owner thread does not perform itself (design part 5.2, the sheet's owner/runner split): all
/// but Notify, StartTimer, RecordNote, ArmRipAgain, WaitForDisc, RefreshIdentity and a new job's Fingerprint.
/// `progress` takes a title key and a fraction. A step stops soon after `cancel` is raised. Def: T22. Use: T23, T24.
pub trait StepRunner: Send + Sync {
    /// Runs `action` for `job` and returns its outcome.
    fn run(
        &self,
        job: &JobContext,
        action: &Action,
        cancel: CancelToken,
        progress: &(dyn Fn(Option<&str>, f64) + Sync),
    ) -> StepResult;
}

/// The engine's outside world (design part 4.2). `steps: None` means the real runner (Task 23). Def: T22. Use: T22 to
/// T29.
pub struct Services {
    /// The operating system's seams.
    pub platform: Platform,
    /// The lookups' transport (design part 8.1).
    pub http: Arc<dyn HttpTransport>,
    /// Where notifications go (design part 8.6).
    pub notify: Box<dyn NotifySink>,
    /// StartTimer's timers (design part 5.2).
    pub timers: Arc<dyn Timers>,
    /// The step runner; None for the real one (Task 23).
    pub steps: Option<Arc<dyn StepRunner>>,
}

/// Called by the owner after each persisted transition, before its actions run; true makes the owner run none of
/// them and stop as `EngineHandle::stop` does. Only the testkit passes one (`Harness::run_until`). Def: T22. Use: T22.
pub(crate) type StopAfter = Arc<dyn Fn(&Transition) -> bool + Send + Sync>;

/// Everything that reaches the owner thread, on its one channel (design part 4.4). Def: T22. Use: T22, T24.
pub(crate) enum Msg {
    /// A drive event from the drive thread.
    Drive(DriveEvent),
    /// A fingerprinted arrival, or a failed one as (drive, sentence).
    Arrival(Result<Arrival, (String, String)>),
    /// A worker's result, for the job's `generation` when it was handed.
    StepDone { job: String, generation: u64, result: StepResult },
    /// A running step's progress (title key, fraction).
    Progress { job: String, title: Option<String>, fraction: f64 },
    /// A StartTimer timer of the job ran out.
    Timer { job: String },
    /// A lookup round of the job ended (design part 8.3): `book.pending()`, or why the round could not run.
    RoundDone { job: String, pending: Result<(bool, bool), String> },
    /// The eject of an already-ripped disc returned.
    Ejected { drive: String },
    /// `EngineHandle::stop`.
    Stop,
}

/// Starts the drive, owner and notifier threads over `config.data` (design part 4.4); `start_until(config, services,
/// None)`. Def: T22. Use: T22 to T29.
pub fn start(config: EngineConfig, services: Services) -> io::Result<EngineHandle> {
    start_until(config, services, None)
}

/// `start` with the testkit's stop hook. It writes the engine.log line `engine started (lazymkv <version>)` before it
/// returns (design part 3.5). With `services.steps` None it runs the real runner, `RealSteps` (Task 23), over the
/// owner's Outbox, the space promises and the last scan's Java use, which the owner keeps for Task 26's self-test.
/// Def: T22. Use: T22.
pub(crate) fn start_until(
    config: EngineConfig,
    services: Services,
    stop_after: Option<StopAfter>,
) -> io::Result<EngineHandle> {
    let Services { platform, http, notify, timers, steps } = services;
    let clock = Arc::clone(&platform.host);
    let store = Arc::new(Store::new(config.data.clone(), Arc::new(move || clock.now_unix()), None));
    store.log(&format!("engine started (lazymkv {})", env!("CARGO_PKG_VERSION")));
    let outbox = Outbox::default();
    let promises = Arc::new(Mutex::new(Promises::default()));
    let java_in_use = Arc::new(Mutex::new(None));
    let steps = steps.unwrap_or_else(|| {
        let (store, outbox, promises) = (Arc::clone(&store), outbox.clone(), Arc::clone(&promises));
        let (http, java_in_use) = (Arc::clone(&http), Arc::clone(&java_in_use));
        Arc::new(RealSteps::new(&config, &platform, http, store, outbox, promises, java_in_use))
    });
    let Platform { drives, ejector, awake, host, secrets, .. } = platform;
    let (delivered, notifier_done) = channel::<()>();
    let notifications = spawn_notifier(Box::new(Delivered { sink: notify, _done: delivered }));
    let (sender, inbox) = channel::<Msg>();
    let shown = Arc::new(Mutex::new(BTreeMap::new()));
    let busy = Arc::new(AtomicBool::new(false));
    let drive_stop = Arc::new(AtomicBool::new(false));
    let parts = Parts {
        host,
        ejector,
        awake,
        timers,
        steps,
        outbox,
        promises,
        java_in_use,
        http,
        secrets,
        busy: Arc::clone(&busy),
    };
    let owner = Owner::load(
        Arc::clone(&store),
        parts,
        sender.clone(),
        notifications,
        stop_after,
        Arc::clone(&shown),
        Arc::clone(&drive_stop),
    );
    let owner = thread::spawn(move || owner.run(inbox));
    let drive = spawn_drive_thread(drives, sender.clone(), Arc::clone(&drive_stop));
    Ok(EngineHandle { sender, owner, drive, drive_stop, notifier_done, busy, store, jobs: shown })
}

/// A running engine (design part 4.4). Def: T22. Use: T22 to T29.
pub struct EngineHandle {
    /// The owner thread's channel.
    sender: Sender<Msg>,
    /// The owner thread.
    owner: JoinHandle<()>,
    /// The drive thread.
    drive: JoinHandle<()>,
    /// Raised to end the drive thread.
    drive_stop: Arc<AtomicBool>,
    /// Closes when the notifier thread has delivered its last notification and ended.
    notifier_done: Receiver<()>,
    /// The owner's `busy`, set after each message it handles.
    busy: Arc<AtomicBool>,
    /// The data folder's store, for the `engine stopped` line.
    store: Arc<Store>,
    /// The owner's records, replaced after each persisted transition.
    jobs: Arc<Mutex<BTreeMap<String, JobRecord>>>,
}

impl EngineHandle {
    /// `job_summary` of each job's record, by id (design part 9.3). Def: T22. Use: T22 to T29.
    pub fn jobs(&self) -> Vec<JobSummary> {
        lock(&self.jobs).values().map(job_summary).collect()
    }

    /// Whether the owner, after the last message it handled, had a step or a lookup round outstanding: the testkit's
    /// `settle` waits until it has none (Task 24). Def: T24. Use: T24 to T29.
    pub(crate) fn busy(&self) -> bool {
        self.busy.load(Ordering::SeqCst)
    }

    /// Polls `jobs` every 20 ms until `done` holds, and gives true; false once `timeout` has passed. Def: T22. Use: T22
    /// to T29.
    pub fn wait_until(&self, timeout: Duration, done: &dyn Fn(&[JobSummary]) -> bool) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            if done(&self.jobs()) {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            thread::sleep(WAIT_POLL);
        }
    }

    /// Cancels every running step's CancelToken (CheckSpace and Eject included), drops their results, writes the
    /// engine.log line `engine stopped` and joins every thread it started but an arrival's walk (design part 3.5,
    /// R22.10). The drive thread ends first, so every drive event it handed over is handled before the owner stops;
    /// the owner then waits for every step, recording and eject it started, and the notifier delivers what was raised.
    /// An arrival still waiting for its volume is not waited for: it only reads, and its late result goes nowhere. A
    /// step, recording or eject that panics still reports (R22.11), so stop never waits on one. Err when the drive or
    /// owner thread panicked. Def: T22. Use: T22 to T29.
    pub fn stop(self) -> io::Result<()> {
        self.drive_stop.store(true, Ordering::SeqCst);
        let drive = self.drive.join();
        let _ = self.sender.send(Msg::Stop);
        drop(self.sender);
        let owner = self.owner.join();
        let _ = self.notifier_done.recv();
        self.store.log("engine stopped");
        if drive.is_err() || owner.is_err() {
            return Err(io::Error::other("lazymkv-engine: an engine thread panicked"));
        }
        Ok(())
    }
}

/// A step's changes to its job's record and the events it feeds before its own result, taken by the owner with the
/// step's result and persisted in that result's transition (design part 5.2). Def: T22. Use: T23, T24.
#[derive(Clone, Default)]
pub(crate) struct Outbox(Arc<Mutex<BTreeMap<String, Posted>>>);

/// What a step posted for its job (design part 5.2). Def: T22. Use: T23, T24.
#[derive(Default)]
pub(crate) struct Posted {
    /// Edits of the job's record, applied in order.
    pub(crate) edits: Vec<Box<dyn FnOnce(&mut JobRecord) + Send>>,
    /// Events fed, in order, before the step's own result.
    pub(crate) first: Vec<Event>,
}

impl Outbox {
    /// Posts an edit of `job`'s record (design part 5.2). Def: T22. Use: T23, T24.
    pub(crate) fn edit(&self, job: &str, edit: impl FnOnce(&mut JobRecord) + Send + 'static) {
        lock(&self.0).entry(job.to_string()).or_default().edits.push(Box::new(edit));
    }

    /// Posts an event fed before the step's result (design part 5.2). Def: T22. Use: T23, T24.
    pub(crate) fn feed_first(&self, job: &str, event: Event) {
        lock(&self.0).entry(job.to_string()).or_default().first.push(event);
    }

    /// Everything posted for `job`, in order; empties the job's entry (design part 5.2). Def: T22. Use: T22, T23, T24.
    pub(crate) fn take(&self, job: &str) -> Posted {
        lock(&self.0).remove(job).unwrap_or_default()
    }
}

/// The disc-phase token and its one FIFO queue (design part 5.4, L2). Def: T22. Use: T22, T25, T26.
#[derive(Default)]
pub(crate) struct DiscToken {
    /// The job that may run a disc-phase step, if any.
    holder: Option<String>,
    /// What waits for the token, in the order it came.
    queue: VecDeque<Waiting>,
}

/// What waits for the disc-phase token (design part 5.4). Def: T22. Use: T22, T25, T26.
pub(crate) enum Waiting {
    /// A fingerprinted arrival whose routing needs the token: a new job, or a disc routed with MediaArrived.
    Arrival(Arrival),
}

impl DiscToken {
    /// The job holding the token (design part 5.4). Def: T22. Use: T22, T25, T26.
    pub(crate) fn holder(&self) -> Option<&str> {
        self.holder.as_deref()
    }

    /// Takes the token for `job`: true when it was free, or is already held by `job` (design part 5.4). Def: T22.
    /// Use: T22, T25, T26.
    pub(crate) fn take(&mut self, job: &str) -> bool {
        match &self.holder {
            Some(holder) => holder == job,
            None => {
                self.holder = Some(job.to_string());
                true
            }
        }
    }

    /// Queues `waiting` behind everything already waiting (design part 5.4). Def: T22. Use: T22, T25, T26.
    pub(crate) fn wait(&mut self, waiting: Waiting) {
        self.queue.push_back(waiting);
    }

    /// Takes every queued arrival of `drive` out of the queue: a queued disc that is removed leaves it (design part
    /// 5.4). True when one was queued. Def: T22. Use: T22, T25, T26.
    pub(crate) fn remove_drive(&mut self, drive: &str) -> bool {
        let before = self.queue.len();
        self.queue.retain(|waiting| match waiting {
            Waiting::Arrival(arrival) => arrival.drive != drive,
        });
        self.queue.len() != before
    }

    /// After a transition of `job`: releases the token when `job` holds it and `keeps_token` is false, then, while the
    /// token is free, pops the next in the queue (design part 5.4, L2). Def: T22. Use: T22, T25, T26.
    pub(crate) fn after(&mut self, job: &str, state: &State, actions: &[Action]) -> Option<Waiting> {
        if self.holder.as_deref() == Some(job) && !keeps_token(state, actions) {
            self.holder = None;
        }
        self.next_free()
    }

    /// The next in the queue while the token is free, else None.
    fn next_free(&mut self) -> Option<Waiting> {
        if self.holder.is_none() {
            self.queue.pop_front()
        } else {
            None
        }
    }
}

/// True while the job may still run a disc-phase step: a disc-phase state whose actions do not include WaitForDisc
/// (design part 5.4, L2: the token is released on any wait, on WaitForDisc, and when the disc phase ends or fails).
/// Def: T22. Use: T22, T25, T26.
pub(crate) fn keeps_token(state: &State, actions: &[Action]) -> bool {
    state.is_disc_phase() && !actions.contains(&Action::WaitForDisc)
}

/// The serde tag of `action`, the name `transitions.jsonl` writes (design part 7.3). Def: T22. Use: T22, T26.
pub(crate) fn action_tag(action: &Action) -> String {
    serde_json::to_value(action)
        .ok()
        .and_then(|value| value["action"].as_str().map(str::to_string))
        .unwrap_or_default()
}

/// The serde tag of `event`, the name `transitions.jsonl` writes (design part 7.3).
fn event_tag(event: &Event) -> String {
    serde_json::to_value(event).ok().and_then(|value| value["event"].as_str().map(str::to_string)).unwrap_or_default()
}

/// The detail of a note a RecordNote action adds (spec 10.2, 6.3).
fn note_detail(note: Note) -> String {
    match note {
        Note::LabelRuleUnavailable => {
            "no Java runtime was found, so label rule 8.3.2 does not apply (spec 10.2)".to_string()
        }
        Note::DirectModeFallback => {
            "the whole-disc copy failed twice, so the job switched to direct mode (spec 6.3)".to_string()
        }
        other => other.as_str().to_string(),
    }
}

/// Adds `note` with its detail unless the job already holds it, keeping the notes sorted by note (S41).
fn add_note(notes: &mut Vec<Noted>, note: Note) {
    if !notes.iter().any(|noted| noted.note == note) {
        notes.push(Noted { note, stream: None, detail: note_detail(note) });
    }
    notes.sort_by_key(|noted| noted.note);
}

/// A new job's record (design part 7.3): no plan, no titles, no notes, created and updated `now`.
fn new_record(id: &str, meta: &DiscMeta, drive: &str, now: u64) -> JobRecord {
    JobRecord {
        format: JOB_FORMAT.to_string(),
        id: id.to_string(),
        fingerprint: meta.fingerprint.clone().unwrap_or_default(),
        disc_id: meta.disc_id.clone(),
        drive: drive.to_string(),
        created_unix: now,
        updated_unix: now,
        state: State::Detected,
        message: None,
        rip_again: false,
        profile: JOB_PROFILE_FILE.to_string(),
        plan: None,
        titles: Vec::new(),
        filed_unix: None,
        collisions: Vec::new(),
        notes: Vec::new(),
    }
}

/// The plan's identity title, the `title` of a job's notifications (S42).
fn title_of(record: &JobRecord) -> Option<String> {
    record.plan.as_ref().and_then(|plan| plan.identity.title.clone())
}

/// The settings a loaded job started with, from its own settings.json (ENG-19), else the engine's.
fn job_settings(store: &Store, job_dir: &Path) -> Settings {
    fs::read_to_string(job_dir.join("settings.json"))
        .ok()
        .and_then(|text| Settings::from_json(&text).ok())
        .map(|(settings, _)| settings)
        .unwrap_or_else(|| store.settings().0)
}

/// Locks `mutex`, ignoring a poison a panicked thread left, so one failed step never stops the owner.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The drive thread (design part 4.4): it hands each drive event to the owner until `stop` is raised.
fn spawn_drive_thread(mut drives: Box<dyn DriveWatcher>, sender: Sender<Msg>, stop: Arc<AtomicBool>) -> JoinHandle<()> {
    thread::spawn(move || {
        while !stop.load(Ordering::SeqCst) {
            if let Some(event) = drives.next(DRIVE_POLL) {
                if sender.send(Msg::Drive(event)).is_err() {
                    break;
                }
            }
        }
    })
}

/// The notifier's sink with a channel that closes when the notifier thread ends, so `stop` can wait for it.
struct Delivered {
    sink: Box<dyn NotifySink>,
    _done: Sender<()>,
}

impl NotifySink for Delivered {
    fn deliver(&mut self, notification: &Notification) {
        self.sink.deliver(notification);
    }
}

/// The platform parts and services the owner thread uses, and what it shares with the step runner.
struct Parts {
    host: Arc<dyn Host>,
    ejector: Arc<dyn Ejector>,
    awake: Arc<dyn KeepAwake>,
    timers: Arc<dyn Timers>,
    steps: Arc<dyn StepRunner>,
    /// The steps' posts, taken with each result (design part 5.2).
    outbox: Outbox,
    /// The space promised to unfinished steps (design part 5.8), released here as steps finish.
    promises: Arc<Mutex<Promises>>,
    /// The last scan's MSG:3344 verdict (design part 5.4 step 4), which `RealSteps` writes.
    #[expect(dead_code, reason = "Task 26's self-test reads Java use from the last scan (design part 9.4)")]
    java_in_use: Arc<Mutex<Option<bool>>>,
    /// The lookups' transport, for the owner's lookup rounds (design part 8.3).
    http: Arc<dyn HttpTransport>,
    /// The secrets, for the rounds' `tmdb_token` (design part 8.1).
    secrets: Arc<dyn SecretStore>,
    /// Whether a step or a lookup round is outstanding, set after each message (`EngineHandle::busy`).
    busy: Arc<AtomicBool>,
}

/// One job as the owner holds it.
struct Job {
    record: JobRecord,
    /// The settings the job started with (ENG-19).
    settings: Settings,
    /// Bumped when the engine stops the job's running step, so its result is dropped (R-A7).
    generation: u64,
    /// The drive holding the job's disc, None while it is out.
    disc: Option<String>,
    lookups: Arc<Mutex<LookupBook>>,
}

/// One drive as the owner knows it.
#[derive(Default)]
struct Drive {
    /// Bumped at each arrival and removal, so a stale arrival is dropped.
    number: u64,
    /// The volume root while a disc is in.
    root: Option<PathBuf>,
    /// Set while an eject of this drive runs: its removal is not fed (design part 5.2's Eject row).
    ejecting: bool,
}

/// A step or recording thread the owner started and has not heard back from.
struct Worker {
    job: String,
    generation: u64,
    /// The tags of the actions it runs, in order.
    tags: Vec<String>,
    /// The drive it ejects, when its actions include Eject.
    eject_drive: Option<String>,
    cancel: CancelToken,
}

/// The job-owner thread's state (design part 4.4).
struct Owner {
    store: Arc<Store>,
    parts: Parts,
    sender: Sender<Msg>,
    notifications: Sender<Notification>,
    stop_after: Option<StopAfter>,
    shown: Arc<Mutex<BTreeMap<String, JobRecord>>>,
    drive_stop: Arc<AtomicBool>,
    jobs: BTreeMap<String, Job>,
    drives: BTreeMap<String, Drive>,
    drive_records: DrivesFile,
    /// False when drives.json was refused: the records are then kept in memory only (design part 7.4).
    drives_writable: bool,
    token: DiscToken,
    /// The file-phase slot and its queue (design part 5.7).
    slots: SlotQueue,
    /// The jobs whose lookup round for their current Ejected entry has run and left nothing pending (R24.1).
    rounds_done: BTreeSet<String>,
    /// The jobs with a lookup round running (design part 8.3).
    rounds_running: BTreeSet<String>,
    workers: Vec<Worker>,
    /// Ejects of already-ripped discs not yet returned.
    ejects: usize,
    threads: Vec<JoinHandle<()>>,
    awake_held: bool,
    stopping: bool,
}

impl Owner {
    /// The owner over the store's jobs and drives.json (design parts 5.10, 7.4). A refused job is logged and left
    /// alone; a refused drives.json is logged and the drives are kept in memory only. Restarts are Task 25's.
    fn load(
        store: Arc<Store>,
        parts: Parts,
        sender: Sender<Msg>,
        notifications: Sender<Notification>,
        stop_after: Option<StopAfter>,
        shown: Arc<Mutex<BTreeMap<String, JobRecord>>>,
        drive_stop: Arc<AtomicBool>,
    ) -> Owner {
        let mut jobs = BTreeMap::new();
        for loaded in store.load_jobs() {
            match loaded {
                Ok(record) => {
                    let settings = job_settings(&store, &store.job_dir(&record.id));
                    lock(&shown).insert(record.id.clone(), record.clone());
                    let job = Job { record, settings, generation: 0, disc: None, lookups: Arc::default() };
                    jobs.insert(job.record.id.clone(), job);
                }
                Err(error) => store.log(&format!("a job is refused and left alone: {error}")),
            }
        }
        let slots = SlotQueue::new(store.settings().0.file_phase_concurrency as usize);
        let (drive_records, drives_writable) = match store.drives() {
            Ok(drives) => (drives, true),
            Err(error) => {
                store.log(&format!("drives.json is refused, so the drives are kept in memory only: {error}"));
                let format = DRIVES_FORMAT.to_string();
                (DrivesFile { format, drives: BTreeMap::new(), drive_incapable: Vec::new() }, false)
            }
        };
        Owner {
            store,
            parts,
            sender,
            notifications,
            stop_after,
            shown,
            drive_stop,
            jobs,
            drives: BTreeMap::new(),
            drive_records,
            drives_writable,
            token: DiscToken::default(),
            slots,
            rounds_done: BTreeSet::new(),
            rounds_running: BTreeSet::new(),
            workers: Vec::new(),
            ejects: 0,
            threads: Vec::new(),
            awake_held: false,
            stopping: false,
        }
    }

    /// The loop: one message at a time, until `Msg::Stop` or the stop hook.
    fn run(mut self, inbox: Receiver<Msg>) {
        while let Ok(message) = inbox.recv() {
            match message {
                Msg::Stop => break,
                Msg::Drive(DriveEvent::Arrived { drive, root }) => self.on_arrived(drive, root),
                Msg::Drive(DriveEvent::Removed { drive }) => self.on_removed(&drive),
                Msg::Arrival(outcome) => self.on_arrival(outcome),
                Msg::StepDone { job, generation, result } => self.on_step_done(&job, generation, result),
                // Task 26 pushes progress to clients; until then it is read and dropped.
                Msg::Progress { job, title, fraction } => drop((job, title, fraction)),
                Msg::Timer { job } => self.on_timer(&job),
                Msg::RoundDone { job, pending } => self.on_round_done(&job, pending),
                Msg::Ejected { drive } => {
                    self.ejects = self.ejects.saturating_sub(1);
                    self.drive(&drive).ejecting = false;
                }
            }
            self.threads.retain(|thread| !thread.is_finished());
            self.keep_awake();
            let busy = !self.workers.is_empty() || !self.rounds_running.is_empty();
            self.parts.busy.store(busy, Ordering::SeqCst);
            if self.stopping {
                break;
            }
        }
        self.shut_down(&inbox);
    }

    /// Cancels every running step, drops their results and waits for every step, recording and eject the owner
    /// started (design part 3.5), then releases KeepAwake.
    fn shut_down(&mut self, inbox: &Receiver<Msg>) {
        self.drive_stop.store(true, Ordering::SeqCst);
        for worker in &self.workers {
            worker.cancel.cancel();
        }
        while !self.workers.is_empty() || self.ejects > 0 {
            match inbox.recv() {
                Ok(Msg::StepDone { job, generation, result }) => {
                    self.log_panic(&job, &result);
                    self.finish_worker(&job, generation);
                    drop(self.parts.outbox.take(&job));
                }
                Ok(Msg::Ejected { .. }) => self.ejects = self.ejects.saturating_sub(1),
                Ok(_) => {}
                Err(_) => break,
            }
        }
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
        if self.awake_held {
            self.parts.awake.release();
            self.awake_held = false;
        }
    }

    /// The owner's record of `drive`.
    fn drive(&mut self, drive: &str) -> &mut Drive {
        self.drives.entry(drive.to_string()).or_default()
    }

    /// Rewrites `drive`'s record in drives.json (design part 7.4, L1a), unless drives.json was refused.
    fn record_drive(&mut self, drive: &str, fingerprint: Option<String>, handled: bool) {
        self.drive_records.drives.insert(drive.to_string(), DriveRecord { fingerprint, handled });
        if self.drives_writable {
            if let Err(error) = self.store.write_drives(&self.drive_records) {
                self.store.log(&format!("drives.json could not be written: {error}"));
            }
        }
    }

    /// Design part 5.4's arrival: a new arrival number, a stale queued arrival of the drive dropped, and the
    /// fingerprint walk on its own thread after `drive_settle_s`, waiting up to `drive_ready_timeout_s`.
    fn on_arrived(&mut self, drive: String, root: PathBuf) {
        let slot = self.drive(&drive);
        slot.number += 1;
        slot.root = Some(root.clone());
        let number = slot.number;
        self.token.remove_drive(&drive);
        let (settings, _) = self.store.settings();
        let settle = Duration::from_secs(u64::from(settings.drive_settle_s));
        let ready = Duration::from_secs(u64::from(settings.drive_ready_timeout_s));
        spawn_arrival(drive, root, number, settle, ready, Arc::clone(&self.parts.host), self.sender.clone());
    }

    /// Whether `arrival` is still the disc in its drive.
    fn is_current(&self, arrival: &Arrival) -> bool {
        self.drives.get(&arrival.drive).is_some_and(|drive| drive.number == arrival.number && drive.root.is_some())
    }

    /// A walked arrival (design part 5.4 steps 2 and 3). A failure is never dropped: it is logged and sends `failed`
    /// with no job. A stale success is dropped with a log line; a current one is recorded in drives.json and routed.
    fn on_arrival(&mut self, outcome: Result<Arrival, (String, String)>) {
        match outcome {
            Err((drive, sentence)) => {
                self.store.log(&format!("{drive}: {sentence}"));
                let failed = NotifyEvent::Failed { class: FailureClass::Unknown };
                self.send(Notification {
                    event: failed,
                    job_id: None,
                    fingerprint: None,
                    title: None,
                    drive: Some(drive),
                    message: sentence,
                    time: self.parts.host.now_unix(),
                });
            }
            Ok(arrival) if !self.is_current(&arrival) => {
                self.store.log(&format!("{}: the disc was removed before its arrival was handled", arrival.drive));
            }
            Ok(arrival) => {
                self.record_drive(&arrival.drive, arrival.meta.fingerprint.clone(), false);
                self.route(arrival);
            }
        }
    }

    /// Design part 5.4 step 3: the settings gate, then `job_lookup` over the jobs of the fingerprint.
    fn route(&mut self, arrival: Arrival) {
        let drive = arrival.drive.clone();
        let (settings, problems) = self.store.settings();
        if settings.staging_root.is_none() {
            self.store.log(&format!("{drive}: no job starts: staging_root is unset"));
            return;
        }
        if let Some(problem) = problems.first() {
            self.store.log(&format!(
                "{drive}: no job starts: the settings have a problem: {}: {}",
                problem.key, problem.problem
            ));
            return;
        }
        let fingerprint = arrival.meta.fingerprint.clone().unwrap_or_default();
        let earlier: Vec<(String, State, bool)> = self
            .jobs
            .values()
            .filter(|job| job.record.fingerprint == fingerprint)
            .map(|job| (job.record.id.clone(), job.record.state.clone(), job.record.rip_again))
            .collect();
        match job_lookup(&earlier) {
            JobLookup::AlreadyRipped(id) => self.already_ripped(&id, &drive, fingerprint),
            JobLookup::Resume(id) if self.jobs.get(&id).is_some_and(|job| job.record.state.is_waiting()) => {
                // A waiting job only has its disc noted: the reducer takes MediaArrived in no waiting state.
                if let Some(job) = self.jobs.get_mut(&id) {
                    job.disc = Some(drive.clone());
                }
                self.record_drive(&drive, Some(fingerprint), true);
            }
            JobLookup::Resume(id) if self.present_drive(&id).is_some() => {
                // R22.7: only a job that waits for its disc gets MediaArrived (design part 5.4 step 3), so a repeat
                // arrival, or a second copy of the disc, starts nothing beside the steps that run.
                let old = self.present_drive(&id).unwrap_or_default();
                self.store.log(&format!("{drive}: the disc of job {id} is already in {old}"));
                self.record_drive(&drive, Some(fingerprint), true);
            }
            JobLookup::Resume(id) => {
                if !self.token.take(&id) {
                    self.token.wait(Waiting::Arrival(arrival));
                    return;
                }
                if let Some(job) = self.jobs.get_mut(&id) {
                    job.disc = Some(drive.clone());
                    job.record.drive = drive.clone();
                }
                self.record_drive(&drive, Some(fingerprint), true);
                self.feed(&id, Event::MediaArrived, None);
            }
            JobLookup::NewJob => {
                let id = job_id(&fingerprint, &self.known_ids().iter().map(String::as_str).collect::<Vec<_>>());
                if self.token.take(&id) {
                    self.create_job(id, arrival, settings);
                } else {
                    self.token.wait(Waiting::Arrival(arrival));
                }
            }
        }
    }

    /// The drive holding the disc of `id` when it is a disc-phase job with its disc in a drive: such a job is not
    /// waiting for its disc (R22.7). A job that failed with MediaReadError is routed its disc whatever this says.
    fn present_drive(&self, id: &str) -> Option<String> {
        self.jobs.get(id).filter(|job| job.record.state.is_disc_phase()).and_then(|job| job.disc.clone())
    }

    /// Every job ID in use: the owner's jobs and every job folder, since a folder the store refused keeps its ID
    /// (design part 7.4: it is never overwritten).
    fn known_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.jobs.keys().cloned().collect();
        let folders = self.store.job_dir("-").parent().map(Path::to_path_buf);
        if let Some(Ok(entries)) = folders.map(fs::read_dir) {
            let names = entries.filter_map(|entry| entry.ok()).map(|entry| entry.file_name());
            ids.extend(names.map(|name| name.to_string_lossy().into_owned()));
        }
        ids
    }

    /// `already_ripped` with the job's fields, and the eject on its own thread, outside the reducer (design part 5.4
    /// step 3). An eject error is only logged.
    fn already_ripped(&mut self, id: &str, drive: &str, fingerprint: String) {
        if let Some(job) = self.jobs.get(id) {
            let event = NotifyEvent::AlreadyRipped;
            self.send(Notification {
                message: notify::message(&event, Some(&job.record)),
                event,
                job_id: Some(id.to_string()),
                fingerprint: Some(job.record.fingerprint.clone()),
                title: title_of(&job.record),
                drive: Some(drive.to_string()),
                time: self.parts.host.now_unix(),
            });
        }
        self.drive(drive).ejecting = true;
        self.ejects += 1;
        let (ejector, store, sender) = (Arc::clone(&self.parts.ejector), Arc::clone(&self.store), self.sender.clone());
        let (drive_id, job) = (drive.to_string(), id.to_string());
        self.threads.push(thread::spawn(move || {
            match panic::catch_unwind(AssertUnwindSafe(|| ejector.eject(&drive_id))) {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    store.log(&format!("{drive_id}: the eject of an already ripped disc failed: {error}"));
                }
                Err(payload) => store.log(&format!(
                    "{drive_id}: the eject of the disc of job {job} panicked: {}",
                    panic_text(&*payload)
                )),
            }
            let _ = sender.send(Msg::Ejected { drive: drive_id });
        }));
        self.record_drive(drive, Some(fingerprint), true);
    }

    /// A new job (design part 5.4 step 3, S29): its folder with the settings it keeps, its record and its first
    /// transition, `{ event: None, old: None, new: Detected, actions: [Fingerprint] }`, whose Fingerprint is the
    /// recording of `spawn_recording`.
    fn create_job(&mut self, id: String, arrival: Arrival, settings: Settings) {
        let drive = arrival.drive.clone();
        let job_dir = match self.store.create_job(&id, &settings) {
            Ok(dir) => dir,
            Err(error) => {
                self.store.log(&format!("{drive}: the job {id} could not be created: {error}"));
                self.give_back(&id);
                return;
            }
        };
        let record = new_record(&id, &arrival.meta, &drive, self.parts.host.now_unix());
        let job = Job { record, settings, generation: 0, disc: Some(drive.clone()), lookups: Arc::default() };
        self.jobs.insert(id.clone(), job);
        self.record_drive(&drive, arrival.meta.fingerprint.clone(), true);
        let (state, actions) = new_job();
        self.commit(&id, None, state, actions, None, Some((arrival, job_dir)));
    }

    /// Feeds `event` to `id`'s reducer. A rejected event writes the job.log line `rejected <event> in <state>` and
    /// changes nothing.
    fn feed(&mut self, id: &str, event: Event, message: Option<String>) {
        let Some(job) = self.jobs.get(id) else {
            return;
        };
        match reduce(&job.record.state, &event) {
            Ok((new, actions)) => self.commit(id, Some(event), new, actions, message, None),
            Err(rejected) => {
                let line = format!("rejected {} in {}", event_tag(&event), rejected.state.as_str());
                self.store.job_log(id, &line);
            }
        }
    }

    /// One transition (design part 7.3, R22.3, R24.2): the record actions applied (RefreshIdentity's refreshed plan
    /// among them), `filed_unix` on entering Filed, `updated_unix`, the message, then `Store::persist`, all before any
    /// action runs; then the stop hook, the actions, the token and the file-phase slot.
    fn commit(
        &mut self,
        id: &str,
        event: Option<Event>,
        new: State,
        actions: Vec<Action>,
        message: Option<String>,
        recording: Option<(Arrival, PathBuf)>,
    ) {
        let Some(job) = self.jobs.get(id) else {
            return;
        };
        let now = self.parts.host.now_unix();
        let mut record = job.record.clone();
        let old = event.as_ref().map(|_| record.state.clone());
        for action in &actions {
            match action {
                Action::RecordNote { note } => add_note(&mut record.notes, *note),
                Action::ArmRipAgain => record.rip_again = true,
                Action::RefreshIdentity => self.refresh_identity(id, &mut record),
                _ => {}
            }
        }
        if new == State::Filed && record.state != State::Filed {
            record.filed_unix = Some(now);
        }
        if new.is_waiting() || matches!(new, State::Failed { .. }) {
            // R22.8: an event that leaves the state as it was and brings no sentence keeps the state's message.
            if message.is_some() || record.state != new {
                record.message = message;
            }
        } else {
            record.message = None;
        }
        record.state = new.clone();
        record.updated_unix = now;
        let transition = Transition { time_unix: now, event, old, new, actions };
        if let Err(error) = self.store.persist(&record, &transition) {
            self.store.log(&format!(
                "{id}: the transition into {} could not be persisted, so none of its actions ran: {error}",
                transition.new.as_str()
            ));
            if transition.old.is_none() {
                // A new job whose first line could not be written does not exist (S29): the arrival is unhandled.
                self.jobs.remove(id);
                lock(&self.shown).remove(id);
                self.record_drive(&record.drive, Some(record.fingerprint.clone()), false);
            }
            self.give_back(id);
            return;
        }
        lock(&self.shown).insert(id.to_string(), record.clone());
        if let Some(job) = self.jobs.get_mut(id) {
            job.record = record;
        }
        self.release_promises(id, &transition);
        if self.stop_after.as_ref().is_some_and(|stop| stop(&transition)) {
            self.stopping = true;
            self.drive_stop.store(true, Ordering::SeqCst);
            return;
        }
        self.perform(id, &transition.actions, recording);
        if self.stopping {
            return;
        }
        let next = self.token.after(id, &transition.new, &transition.actions);
        self.route_queue(next);
        if self.stopping {
            return;
        }
        self.file_slot(id, &transition);
    }

    /// RefreshIdentity (design part 8.4, R24.2): the record's plan replaced by `refreshed_plan` over the job's book and
    /// settings. A plan that cannot be refreshed stays, with a job.log line.
    fn refresh_identity(&self, id: &str, record: &mut JobRecord) {
        let (Some(job), Some(plan)) = (self.jobs.get(id), record.plan.as_ref()) else {
            return;
        };
        match refreshed_plan(plan, &self.store.job_dir(id), &job.lookups, &job.settings) {
            Ok(refreshed) => record.plan = Some(refreshed),
            Err(error) => self.store.job_log(id, &format!("the identity was not refreshed: {error}")),
        }
    }

    /// The file-phase slot after a transition (design part 5.7): a job entering `Ejected { after_copy: false }` joins
    /// the queue with no lookup round run for this entry; a holder reaching Filed, NeedsReview, Failed or Discarded
    /// frees its place. Either way the slot is granted again.
    fn file_slot(&mut self, id: &str, transition: &Transition) {
        let ejected = State::Ejected { after_copy: false };
        if transition.new == ejected && transition.old.as_ref() != Some(&ejected) {
            self.rounds_done.remove(id);
            self.slots.join(id);
            self.grant_next();
        } else if self.slots.after(id, &transition.new) {
            self.grant_next();
        }
    }

    /// Grants the file-phase slot while a place is free (design part 5.7, R24.1): a head of the queue whose persisted
    /// plan is lookup-pending gets one lookup round first (design part 8.3), and nothing is granted while that round
    /// runs; any other head is granted its place and fed FilePhaseSlot. A head that is no longer an Ejected job leaves
    /// the queue.
    fn grant_next(&mut self) {
        while let Some(head) = self.slots.next() {
            let Some(job) = self.jobs.get(&head).filter(|job| job.record.state == State::Ejected { after_copy: false })
            else {
                self.slots.leave(&head);
                continue;
            };
            let pending = job.record.plan.as_ref().is_some_and(|plan| plan.identity.lookup_pending);
            if pending && !self.rounds_done.contains(&head) {
                if !self.rounds_running.contains(&head) {
                    self.start_round(&head);
                }
                return;
            }
            self.slots.grant(&head);
            self.feed(&head, Event::FilePhaseSlot, None);
            if self.stopping {
                return;
            }
        }
    }

    /// A lookup round for `id` on its own thread (design part 8.3), with the `tmdb_token` secret and the host's date;
    /// its end comes back as `Msg::RoundDone`.
    fn start_round(&mut self, id: &str) {
        let Some(job) = self.jobs.get(id) else {
            return;
        };
        self.rounds_running.insert(id.to_string());
        let (sender, job_id) = (self.sender.clone(), id.to_string());
        spawn_round(
            self.store.job_dir(id),
            Arc::clone(&job.lookups),
            Arc::clone(&self.parts.http),
            self.parts.secrets.get("tmdb_token"),
            self.parts.host.local_date(),
            Arc::clone(&self.store),
            Box::new(move |pending| {
                let _ = sender.send(Msg::RoundDone { job: job_id, pending });
            }),
        );
    }

    /// The waiting message of `id`'s pending lookups, the book locked only to read it (design part 8.3, R24.6).
    fn lookup_message(&self, id: &str) -> Option<String> {
        self.jobs.get(id).map(|job| waiting_message(&lock(&job.lookups)))
    }

    /// A lookup round ended (design part 8.3). In `Ejected`, a lookup still pending takes the job out of the slot's
    /// queue and feeds LookupPending with the waiting message; otherwise the round is marked done, and a round that
    /// could not run is logged in job.log and marked done too, so the job goes on to the file phase, where its decision
    /// says what is missing. In AwaitingLookup, a lookup still pending feeds LookupPending, which restarts the timer,
    /// and none feeds LookupAnswered; a round that could not run feeds LookupPending with its sentence. The slot is
    /// granted again afterwards.
    fn on_round_done(&mut self, id: &str, pending: Result<(bool, bool), String>) {
        self.rounds_running.remove(id);
        let Some(state) = self.jobs.get(id).map(|job| job.record.state.clone()) else {
            return;
        };
        let waiting = matches!(pending, Ok((discdb, tmdb)) if discdb || tmdb);
        if let Err(error) = &pending {
            self.store.job_log(id, &format!("the lookup round could not run: {error}"));
        }
        match state {
            State::Ejected { after_copy: false } if waiting => {
                self.slots.leave(id);
                let message = self.lookup_message(id);
                self.feed(id, Event::LookupPending, message);
            }
            State::Ejected { after_copy: false } => {
                self.rounds_done.insert(id.to_string());
            }
            State::AwaitingLookup { .. } => match pending {
                Ok(_) if waiting => {
                    let message = self.lookup_message(id);
                    self.feed(id, Event::LookupPending, message);
                }
                Ok(_) => self.feed(id, Event::LookupAnswered, None),
                Err(error) => {
                    let message = format!("the lookup round could not run: {error}");
                    self.feed(id, Event::LookupPending, Some(message));
                }
            },
            _ => {}
        }
        if !self.stopping {
            self.grant_next();
        }
    }

    /// Design part 5.8's promises released as steps finish: leaving Ripping releases the job's Rip promise, leaving
    /// Remuxing its Remux promise, and a new Failed or waiting state releases both of its promises.
    fn release_promises(&self, id: &str, transition: &Transition) {
        let mut promises = lock(&self.parts.promises);
        if transition.old == Some(State::Ripping) && transition.new != State::Ripping {
            promises.release(id, PromisedStep::Rip);
        }
        if transition.old == Some(State::Remuxing) && transition.new != State::Remuxing {
            promises.release(id, PromisedStep::Remux);
        }
        if transition.new.is_waiting() || matches!(transition.new, State::Failed { .. }) {
            promises.release(id, PromisedStep::Rip);
            promises.release(id, PromisedStep::Remux);
        }
    }

    /// Releases the token `id` holds although it cannot go on (its folder or a transition could not be written), so
    /// it never holds every later disc behind it, and routes what waited.
    fn give_back(&mut self, id: &str) {
        let next = self.token.after(id, &State::Discarded, &[]);
        self.route_queue(next);
    }

    /// Routes what the token's queue gives while the token is free (design part 5.4).
    fn route_queue(&mut self, next: Option<Waiting>) {
        let mut next = next;
        while let Some(waiting) = next {
            match waiting {
                Waiting::Arrival(arrival) if !self.is_current(&arrival) => {
                    self.store.log(&format!("{}: the disc was removed before its arrival was handled", arrival.drive));
                }
                Waiting::Arrival(arrival) => self.route(arrival),
            }
            if self.stopping {
                return;
            }
            next = self.token.next_free();
        }
    }

    /// Design part 5.2: Notify, StartTimer and WaitForDisc here (RecordNote, ArmRipAgain and RefreshIdentity were
    /// applied before the persist); a new job's Fingerprint is its recording; every other action goes, in list order,
    /// to one worker.
    fn perform(&mut self, id: &str, actions: &[Action], recording: Option<(Arrival, PathBuf)>) {
        let mut recording = recording;
        let mut handed = Vec::new();
        for action in actions {
            match action {
                Action::Notify { event } => self.notify_job(id, event),
                Action::StartTimer { minutes } => self.start_timer(id, *minutes),
                Action::WaitForDisc => {
                    if let Some(job) = self.jobs.get_mut(id) {
                        job.disc = None;
                    }
                }
                Action::RecordNote { .. } | Action::ArmRipAgain | Action::RefreshIdentity => {}
                Action::Fingerprint => match recording.take() {
                    Some((arrival, job_dir)) => self.start_recording(id, arrival, job_dir),
                    None => handed.push(Action::Fingerprint),
                },
                other => handed.push(other.clone()),
            }
        }
        if !handed.is_empty() {
            self.hand(id, handed);
        }
    }

    /// A job's notification (design part 8.6, S42).
    fn notify_job(&self, id: &str, event: &NotifyEvent) {
        let Some(job) = self.jobs.get(id) else {
            return;
        };
        self.send(Notification {
            event: event.clone(),
            job_id: Some(id.to_string()),
            fingerprint: Some(job.record.fingerprint.clone()),
            title: title_of(&job.record),
            drive: Some(job.record.drive.clone()),
            message: notify::message(event, Some(&job.record)),
            time: self.parts.host.now_unix(),
        });
    }

    /// Hands a notification to the notifier thread.
    fn send(&self, notification: Notification) {
        let _ = self.notifications.send(notification);
    }

    /// StartTimer (design part 5.2): the timer sends `Msg::Timer` for the job.
    fn start_timer(&self, id: &str, minutes: u32) {
        let (sender, job) = (self.sender.clone(), id.to_string());
        self.parts.timers.start(
            minutes,
            Box::new(move || {
                let _ = sender.send(Msg::Timer { job });
            }),
        );
    }

    /// A new job's recording (design part 5.4 step 3), counted as the job's running step.
    fn start_recording(&mut self, id: &str, arrival: Arrival, job_dir: PathBuf) {
        let generation = self.jobs.get(id).map_or(0, |job| job.generation);
        self.workers.push(Worker {
            job: id.to_string(),
            generation,
            tags: vec![action_tag(&Action::Fingerprint)],
            eject_drive: None,
            cancel: CancelToken::default(),
        });
        let now = self.parts.host.now_unix();
        spawn_recording(id.to_string(), generation, arrival, job_dir, now, self.sender.clone());
    }

    /// One worker runs `actions` one after another and returns the last result whose event is Some, else the last
    /// result (design part 5.2). Handing Eject marks the job's drive as ejecting.
    fn hand(&mut self, id: &str, actions: Vec<Action>) {
        let Some(job) = self.jobs.get(id) else {
            return;
        };
        let drive_root =
            job.disc.as_ref().and_then(|drive| self.drives.get(drive)).and_then(|drive| drive.root.clone());
        let context = JobContext {
            record: job.record.clone(),
            job_dir: self.store.job_dir(id),
            settings: job.settings.clone(),
            drive_root,
            lookups: Arc::clone(&job.lookups),
        };
        let generation = job.generation;
        let eject_drive = actions.contains(&Action::Eject).then(|| job.record.drive.clone());
        if let Some(drive) = &eject_drive {
            self.drive(drive).ejecting = true;
        }
        let cancel = CancelToken::default();
        self.workers.push(Worker {
            job: id.to_string(),
            generation,
            tags: actions.iter().map(action_tag).collect(),
            eject_drive,
            cancel: cancel.clone(),
        });
        let (steps, sender, job) = (Arc::clone(&self.parts.steps), self.sender.clone(), id.to_string());
        self.threads.push(thread::spawn(move || {
            let (progress_to, progress_job) = (sender.clone(), job.clone());
            let progress = move |title: Option<&str>, fraction: f64| {
                let title = title.map(str::to_string);
                let _ = progress_to.send(Msg::Progress { job: progress_job.clone(), title, fraction });
            };
            let run = panic::catch_unwind(AssertUnwindSafe(|| {
                let mut kept: Option<StepResult> = None;
                for action in &actions {
                    if cancel.is_cancelled() {
                        break;
                    }
                    let result = steps.run(&context, action, cancel.clone(), &progress);
                    if result.event.is_some() || kept.as_ref().is_none_or(|kept| kept.event.is_none()) {
                        kept = Some(result);
                    }
                }
                kept.unwrap_or(StepResult { event: None, message: None })
            }));
            // R22.11: a panic is reported as a result with no event, so the owner never waits for it.
            let result = run.unwrap_or_else(|payload| StepResult {
                event: None,
                message: Some(format!("{PANICKED}{}", panic_text(&*payload))),
            });
            let _ = sender.send(Msg::StepDone { job, generation, result });
        }));
    }

    /// Takes the worker of (`id`, `generation`) off the list, clearing the ejecting mark of the drive it ejected.
    fn finish_worker(&mut self, id: &str, generation: u64) -> Option<Worker> {
        let at = self.workers.iter().position(|worker| worker.job == id && worker.generation == generation)?;
        let worker = self.workers.remove(at);
        if let Some(drive) = &worker.eject_drive {
            self.drive(drive).ejecting = false;
        }
        Some(worker)
    }

    /// A worker's result (design part 5.2). A stale one is dropped with what its step posted (R-A7), unless a newer
    /// step of the job runs, whose posts it must not take. Otherwise the posted edits are applied, each posted event
    /// is fed, then the result's event. A result with no event, from an action other than those that report none,
    /// writes `<tag> returned no event` to job.log, followed by `: <message>` when it has one, and gives the token
    /// back, since no step of the job runs any more (L2).
    fn on_step_done(&mut self, id: &str, generation: u64, result: StepResult) {
        self.log_panic(id, &result);
        let worker = self.finish_worker(id, generation);
        if self.jobs.get(id).map(|job| job.generation) != Some(generation) {
            if !self.workers.iter().any(|worker| worker.job == id) {
                drop(self.parts.outbox.take(id));
            }
            return;
        }
        let posted = self.parts.outbox.take(id);
        if let Some(job) = self.jobs.get_mut(id) {
            for edit in posted.edits {
                edit(&mut job.record);
            }
        }
        for event in posted.first {
            self.feed(id, event, None);
            if self.stopping {
                return;
            }
        }
        match result.event {
            Some(event) => self.feed(id, event, result.message),
            None => {
                let tag = worker.and_then(|worker| worker.tags.last().cloned()).unwrap_or_default();
                if !NO_EVENT_ACTIONS.contains(&tag.as_str()) {
                    let line = match &result.message {
                        Some(message) => format!("{tag} returned no event: {message}"),
                        None => format!("{tag} returned no event"),
                    };
                    self.store.job_log(id, &line);
                    self.give_back(id);
                }
            }
        }
    }

    /// The engine.log line `<id>: <message>` of a step or recording that panicked (R22.11).
    fn log_panic(&self, id: &str, result: &StepResult) {
        if let Some(message) = result.message.as_deref().filter(|message| message.contains(PANICKED)) {
            self.store.log(&format!("{id}: {message}"));
        }
    }

    /// Design part 5.4's removal: the drive leaves the queue and its record empties. An ejecting drive is not fed.
    /// The token holder whose disc it held, in a disc-phase state, has its running step cancelled and dropped and is
    /// fed MediaRemoved; any other job whose disc was in the drive only has it marked absent.
    fn on_removed(&mut self, drive: &str) {
        let slot = self.drive(drive);
        slot.number += 1;
        slot.root = None;
        let ejecting = slot.ejecting;
        self.token.remove_drive(drive);
        self.record_drive(drive, None, false);
        let holder = self.token.holder().map(str::to_string);
        let held: Vec<String> = self
            .jobs
            .values()
            .filter(|job| job.disc.as_deref() == Some(drive))
            .map(|job| job.record.id.clone())
            .collect();
        for id in held {
            let Some(job) = self.jobs.get_mut(&id) else {
                continue;
            };
            job.disc = None;
            if ejecting || holder.as_deref() != Some(id.as_str()) || !job.record.state.is_disc_phase() {
                continue;
            }
            job.generation += 1;
            for worker in self.workers.iter().filter(|worker| worker.job == id) {
                worker.cancel.cancel();
            }
            self.feed(&id, Event::MediaRemoved, None);
            if self.stopping {
                return;
            }
        }
    }

    /// A StartTimer timer ran out (design part 5.2). AwaitingLookup gets a lookup round, unless one runs (design part
    /// 8.3). AwaitingNetwork gets NetworkRetry once its disc is in and the disc-phase token is free (L2); until then
    /// the timer runs again.
    fn on_timer(&mut self, id: &str) {
        let Some(job) = self.jobs.get(id) else {
            return;
        };
        if matches!(job.record.state, State::AwaitingLookup { .. }) {
            if !self.rounds_running.contains(id) {
                self.start_round(id);
            }
            return;
        }
        let State::AwaitingNetwork { return_to } = &job.record.state else {
            return;
        };
        let ready = !return_to.is_disc_phase() || (job.disc.is_some() && self.token.take(id));
        if ready {
            self.feed(id, Event::NetworkRetry, None);
        } else {
            self.start_timer(id, NETWORK_RETRY_MIN);
        }
    }

    /// KeepAwake is held while a step runs, the token is held or a file-phase slot is held (design parts 4.2, 5.7,
    /// R22.4).
    fn keep_awake(&mut self) {
        let active = !self.workers.is_empty() || self.token.holder().is_some() || self.slots.held();
        if active != self.awake_held {
            if active {
                self.parts.awake.acquire();
            } else {
                self.parts.awake.release();
            }
            self.awake_held = active;
        }
    }
}

#[cfg(test)]
mod tests;
