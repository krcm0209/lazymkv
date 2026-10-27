//! The job-owner thread (design parts 4.4, 5.2, 5.4, 5.7, 7.3, 8.3, 9.1 to 9.4; K8, L2): it holds every job, runs
//! `core::job::reduce` on each event, persists the transition before any of its actions run, performs the actions that
//! only touch the record or the notifier itself and hands every other action to a `StepRunner` worker. It also routes
//! arrivals, keeps the one disc-phase token with its queue and the file-phase slot with its queue, runs the lookup
//! rounds of design part 8.3, holds KeepAwake while a job is active, broadcasts `job_update` and `progress` to the IPC
//! clients and answers their requests (Task 26).

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
use lazymkv_core::ipc::{
    is_secret_name, job_summary, push_line, reply_error, reply_ok, JobSummary, JobUpdate, Notification, Progress, Push,
    Request, SelfTest, Snapshot, CANCELLED, SECRET_NAMES,
};
use lazymkv_core::job::{
    job_id, job_lookup, new_job, reduce, Action, Event, FailureClass, Held, JobLookup, NotifyEvent, State, Tool,
};
use lazymkv_core::profile::JOB_PROFILE_FILE;
use lazymkv_core::record::{DriveRecord, DrivesFile, JobRecord, Transition, DRIVES_FORMAT, JOB_FORMAT};
use lazymkv_core::remembered::Overrides;
use lazymkv_core::review::{Note, Noted};
use lazymkv_core::settings::Settings;
use lazymkv_core::snapshot::DiscMeta;
use lazymkv_platform::{
    DriveEvent, DriveWatcher, Ejector, Host, IpcStream, KeepAwake, Platform, SecretStore, ShareConnector,
};
use lazymkv_tools::CancelToken;
use serde_json::{Map, Value};

use crate::arrival::{read_arrival, spawn_arrival, spawn_recording, Arrival};
use crate::file_phase::{refreshed_plan, waiting_message, SlotQueue, PARTIAL_FOLDER};
use crate::ipc_server::{
    cancel_event, history_page, main_tag, no_job, no_retry, no_rip_again, no_running_step, not_a_secret,
    not_cancellable, not_held, secret_states, settings_refused, snapshot_jobs, spawn_reader, spawn_serve,
    staging_unset, steps_running, waits_for_review, Clients, ProgressGate, COMMAND_REFUSED, DISCARD_REFUSED, STOPPING,
    UI_QUIT_REFUSED,
};
use crate::notify::{self, spawn_notifier, NotifySink};
use crate::restart::{plan_restart, rebuild_book};
use crate::selftest::{first_selftest, not_checked, CheckEnv, MakeMkv, MakeMkvState, RecheckJob};
use crate::space::{rebuild_promises, Promises, PromisedStep};
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

/// The disc-phase token's holder while a MakeMKV check's listing runs (Ruling R26.c1, design part 3.7): it is never a
/// job ID, which is a lower-case fingerprint, a dash and a number (K3). Def: T26. Use: T26.
pub(crate) const CHECK_HOLDER: &str = "the MakeMKV check";

/// The disc-phase token's holder from the removal of a disc whose `makemkvcon` step was killed until that stale step
/// has reported (design part 5.3: "After any kill the engine waits 10 s before the next `makemkvcon` call"; the pause
/// runs inside the killed step): never a job ID, as `CHECK_HOLDER`. Def: T22. Use: T22.
pub(crate) const KILLED_HOLDER: &str = "the killed MakeMKV step";

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

/// Everything that reaches the owner thread, on its one channel (design part 4.4). Def: T22. Use: T22, T24, T26.
pub(crate) enum Msg {
    /// A drive event from the drive thread.
    Drive(DriveEvent),
    /// A fingerprinted arrival, or a failed one as (drive, sentence).
    Arrival(Result<Arrival, (String, String)>),
    /// A worker's result, for the job's `generation` when it was handed.
    StepDone { job: String, generation: u64, result: StepResult },
    /// A running step's progress: the running action's serde tag, the title key and the fraction (S7).
    Progress { job: String, step: String, title: Option<String>, fraction: f64 },
    /// A StartTimer timer of the job ran out.
    Timer { job: String },
    /// A lookup round of the job ended (design part 8.3): `book.pending()`, or why the round could not run.
    RoundDone { job: String, pending: Result<(bool, bool), String> },
    /// The eject of an already-ripped disc returned.
    Ejected { drive: String },
    /// A client the serve thread accepted (design part 9.2).
    Connect(IpcStream),
    /// A client's request, with the request's id (design part 9.3).
    Request { client: u64, id: Option<u64>, request: Request },
    /// A self-test or a retry's re-check ended, for the request `id` of `client` (design part 9.4, S81).
    Checked { client: u64, id: Option<u64>, outcome: CheckOutcome },
    /// The running self-test's MakeMKV check ended, so the disc-phase token it holds is free again (Ruling R26.c1).
    CheckListed,
    /// `EngineHandle::stop`.
    Stop,
}

/// How a self-test or a retry's re-check ended (design part 9.4, S81). Def: T26. Use: T26.
pub(crate) enum CheckOutcome {
    /// The self-test's results.
    Selftest(SelfTest),
    /// A waiting job's re-check, with the event its cleared condition gives.
    Recheck { job: String, event: Option<Event> },
    /// The check's thread panicked, with the panic's text.
    Panicked(String),
}

/// Starts the drive, owner and notifier threads over `config.data` (design part 4.4); `start_until(config, services,
/// None)`. Def: T22. Use: T22 to T29.
pub fn start(config: EngineConfig, services: Services) -> io::Result<EngineHandle> {
    start_until(config, services, None)
}

/// `start` with the testkit's stop hook. It writes the engine.log line `engine started (lazymkv <version>)`, runs the
/// MakeMKV check to its end (design part 3.7, S79, PF-28) and then the restart sequence (design part 5.10, Ruling S70;
/// `Owner::load`) before it returns, so a restart's EngineRestart lines are persisted by then (design part 3.5); the
/// IPC server's serve thread starts last (Task 26). With `services.steps` None it runs the real runner, `RealSteps`
/// (Task 23), over the owner's Outbox, the space promises, the last scan's Java use and the MakeMKV state, which the
/// owner keeps for the self-test. Def: T22. Use: T22.
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
    let makemkv = Arc::new(MakeMkv::new(MakeMkvState::open(&config.data)));
    let env = CheckEnv {
        data: config.data.clone(),
        resources: config.resources.clone(),
        path_var: config.path_var.clone(),
        limits_override: config.limits_override,
        processes: Arc::clone(&platform.processes),
        host: Arc::clone(&platform.host),
        http: Arc::clone(&http),
        secrets: Arc::clone(&platform.secrets),
        java_in_use: Arc::clone(&java_in_use),
        makemkv: Arc::clone(&makemkv),
        cancel: CancelToken::default(),
    };
    // PF-28: the MakeMKV check at start runs to its end before any step runs or any client is accepted (S79).
    let selftest = first_selftest(env.makemkv_check(&store.settings().0, false));
    let steps = steps.unwrap_or_else(|| {
        let (store, outbox, promises) = (Arc::clone(&store), outbox.clone(), Arc::clone(&promises));
        let (http, java_in_use, makemkv) = (Arc::clone(&http), Arc::clone(&java_in_use), Arc::clone(&makemkv));
        Arc::new(RealSteps::new(&config, &platform, http, store, outbox, promises, java_in_use, makemkv))
    });
    let Platform { drives, ejector, awake, host, secrets, shares, ipc, .. } = platform;
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
        http,
        secrets,
        shares,
        busy: Arc::clone(&busy),
        env,
        clients: Arc::new(Clients::new()),
    };
    // L1: the discs already in a drive, read before the drive thread drains any event (design part 4.2).
    let present = drives.present();
    let owner = Owner::load(
        Arc::clone(&store),
        parts,
        sender.clone(),
        notifications,
        stop_after,
        Arc::clone(&shown),
        Arc::clone(&drive_stop),
        present,
        selftest,
    );
    let owner = thread::spawn(move || owner.run(inbox));
    let drive = spawn_drive_thread(drives, sender.clone(), Arc::clone(&drive_stop));
    let serve_stop = Arc::new(AtomicBool::new(false));
    let serve = spawn_serve(ipc, Arc::clone(&serve_stop), sender.clone());
    Ok(EngineHandle {
        sender,
        owner,
        drive,
        drive_stop,
        notifier_done,
        busy,
        store,
        jobs: shown,
        serve,
        serve_stop,
        waker: Mutex::new(None),
    })
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
    /// The IPC server's serve thread (design part 4.4).
    serve: JoinHandle<()>,
    /// Raised to end the serve thread.
    serve_stop: Arc<AtomicBool>,
    /// How `stop` wakes the serve thread from the transport's waiting `accept`; None leaves the thread to end with
    /// its transport.
    waker: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}

impl EngineHandle {
    /// `job_summary` of each job's record, by id (design part 9.3). Def: T22. Use: T22 to T29.
    pub fn jobs(&self) -> Vec<JobSummary> {
        lock(&self.jobs).values().map(job_summary).collect()
    }

    /// Whether the owner, after the last message it handled, had a step or a lookup round outstanding; false once it
    /// has shut down. While it is busy the testkit's `settle` asks for 2 s of quiet instead of 300 ms, and waits no
    /// longer than that for it to clear (Ruling R24.c1). Def: T24. Use: T24 to T29.
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

    /// Sets how `stop` wakes the IPC serve thread, which waits in the transport's `accept` (design part 4.4): the
    /// testkit connects once through its `MemoryConnector`, because `MemoryIpc::accept` holds its lock while it waits.
    /// Without a waker, `stop` leaves the serve thread to end with its transport. Def: T26. Use: T26.
    pub(crate) fn set_ipc_waker(&self, wake: Box<dyn FnOnce() + Send>) {
        *lock(&self.waker) = Some(wake);
    }

    /// Cancels every running step's CancelToken (CheckSpace and Eject included), drops their results, writes the
    /// engine.log line `engine stopped` and joins every thread it started but an arrival's walk and a client's reader
    /// and writer, which end with the client (design part 3.5, R22.10). The serve thread ends first, woken by the IPC
    /// waker, so no client connects while the engine stops; then the drive thread, so every drive event it handed over
    /// is handled before the owner stops; the owner then waits for every step, recording, eject and check it started,
    /// closes every client, and the notifier delivers what was raised. An arrival still waiting for its volume is not
    /// waited for: it only reads, and its late result goes nowhere. A step, recording or eject that panics still
    /// reports (R22.11), so stop never waits on one. Err when the serve, drive or owner thread panicked. Def: T22.
    /// Use: T22 to T29.
    pub fn stop(self) -> io::Result<()> {
        let EngineHandle { sender, owner, drive, drive_stop, notifier_done, store, serve, serve_stop, waker, .. } =
            self;
        serve_stop.store(true, Ordering::SeqCst);
        let served = match waker.into_inner().unwrap_or_else(PoisonError::into_inner) {
            Some(wake) => {
                wake();
                serve.join().is_ok()
            }
            None => true,
        };
        drive_stop.store(true, Ordering::SeqCst);
        let drive = drive.join();
        let _ = sender.send(Msg::Stop);
        drop(sender);
        let owner = owner.join();
        let _ = notifier_done.recv();
        store.log("engine stopped");
        if drive.is_err() || owner.is_err() || !served {
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

/// What waits for the disc-phase token, in one queue (design part 5.4, way (d)). Def: T22. Use: T22, T25, T26.
pub(crate) enum Waiting {
    /// A fingerprinted arrival whose routing needs the token: a new job, or a disc routed with MediaArrived.
    Arrival(Arrival),
    /// A Retry of the failed job, whose failed step is a disc-phase step (design part 9.4, L3).
    Retry(String),
    /// The return of the waiting job to a disc-phase state, with the event that returns it (design parts 3.7, 5.10).
    Return(String, Event),
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

    /// Hands the token to `holder`, a sentinel that is never a job ID, so that a job's release leaves it held
    /// (`KILLED_HOLDER`, design part 5.3). Def: T22. Use: T22.
    pub(crate) fn hold_for(&mut self, holder: &str) {
        self.holder = Some(holder.to_string());
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
            Waiting::Retry(_) | Waiting::Return(..) => true,
        });
        self.queue.len() != before
    }

    /// Whether a Retry or return of `job` waits in the queue (design part 5.4). Def: T26. Use: T26.
    pub(crate) fn queued_job(&self, job: &str) -> bool {
        self.queue.iter().any(|waiting| match waiting {
            Waiting::Retry(id) | Waiting::Return(id, _) => id == job,
            Waiting::Arrival(_) => false,
        })
    }

    /// Whether an arrival of `drive` waits in the queue (design part 5.4). Def: T26. Use: T26.
    pub(crate) fn queued_arrival(&self, drive: &str) -> bool {
        self.queue.iter().any(|waiting| matches!(waiting, Waiting::Arrival(arrival) if arrival.drive == drive))
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
    /// The lookups' transport, for the owner's lookup rounds (design part 8.3).
    http: Arc<dyn HttpTransport>,
    /// The secrets, for the rounds' `tmdb_token` (design part 8.1).
    secrets: Arc<dyn SecretStore>,
    /// The shares, for the space promises rebuilt at a restart or a return (design part 5.8, S75).
    shares: Arc<dyn ShareConnector>,
    /// Whether a step, a lookup round or a check is outstanding, set after each message and cleared at shutdown
    /// (`EngineHandle::busy`, Ruling R24.c1).
    busy: Arc<AtomicBool>,
    /// What the self-test and the re-checks run with, the last scan's Java use and the MakeMKV state among it (design
    /// part 9.4).
    env: CheckEnv,
    /// The IPC clients (design part 9.2).
    clients: Arc<Clients>,
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
    /// Set when the owner cancelled it (`cancel`, or a forced shutdown).
    cancelled: Option<Cancelled>,
}

impl Worker {
    /// Whether the worker runs a `makemkvcon`: a RunScan or RipTitle, whose drive listing is part of it (design parts
    /// 3.7, 5.3, S80).
    fn runs_makemkv(&self) -> bool {
        self.tags.iter().any(|tag| tag == "RunScan" || tag == "RipTitle")
    }
}

/// A running step the owner cancelled (design part 9.4, R-A7): the failure fed once the step has ended, in place of
/// its own result, and the `cancel` requests answered then (S78).
struct Cancelled {
    event: Event,
    replies: Vec<(u64, Option<u64>)>,
}

/// A self-test or a retry's re-check waiting for its turn: they run one at a time.
struct PendingCheck {
    client: u64,
    id: Option<u64>,
    work: CheckWork,
}

/// What a check runs (design part 9.4, S81).
enum CheckWork {
    /// `run_selftest`.
    Selftest,
    /// A `retry` of the waiting job.
    Recheck(String),
}

/// A forced shutdown waiting for its steps to end (design part 3.8, S83): the `shutdown` request to answer and the
/// jobs whose step it cancelled, each with that step's main action tag.
struct Quiet {
    client: u64,
    id: Option<u64>,
    cancelled: Vec<(String, String)>,
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
    /// At most one progress push a second per job (design part 9.2).
    gate: ProgressGate,
    /// The last self-test, which every snapshot shows (design part 9.2, S80).
    selftest: SelfTest,
    /// The share names a `secret_set` of this run stored, which the snapshot lists (Ruling S77).
    share_secrets: BTreeSet<String>,
    /// The self-tests and re-checks waiting for their turn.
    checks: VecDeque<PendingCheck>,
    /// Whether a self-test or re-check runs.
    checking: bool,
    /// The running check's CancelToken, raised at a stop or a forced shutdown (design parts 3.8, 5.3).
    check_cancel: Option<CancelToken>,
    /// The Retry or ReviewApproved a job waiting in the slot's queue is fed with its place (design part 5.7).
    slot_events: BTreeMap<String, Event>,
    /// The jobs whose running lookup round a `retry` started (Ruling S81).
    retry_rounds: BTreeSet<String>,
    /// The (drive, arrival number) of each walk a `settings_set` started (design part 9.4, L1a).
    rescans: BTreeSet<(String, u64)>,
    /// A forced shutdown waiting for its steps (design part 3.8).
    quiet: Option<Quiet>,
}

impl Owner {
    /// The owner over the store's jobs and drives.json (design parts 5.10, 7.4), after the restart sequence over the
    /// discs `present` in the drives (`Owner::restart`), showing `selftest` until the first `run_selftest`. Each loaded
    /// job but a Filed or Discarded one has its lookup book rebuilt from its folder (`rebuild_book`, S73). A refused
    /// job is logged and left alone; a refused drives.json is logged and the drives are kept in memory only.
    #[allow(clippy::too_many_arguments)]
    fn load(
        store: Arc<Store>,
        parts: Parts,
        sender: Sender<Msg>,
        notifications: Sender<Notification>,
        stop_after: Option<StopAfter>,
        shown: Arc<Mutex<BTreeMap<String, JobRecord>>>,
        drive_stop: Arc<AtomicBool>,
        present: Vec<(String, PathBuf)>,
        selftest: SelfTest,
    ) -> Owner {
        let mut jobs = BTreeMap::new();
        for loaded in store.load_jobs() {
            match loaded {
                Ok(record) => {
                    let job_dir = store.job_dir(&record.id);
                    let settings = job_settings(&store, &job_dir);
                    // S73, widened at the final review: every job that can still run a step, a failed or held one
                    // included, has its book rebuilt from its folder, so a Retry's Plan or Decide reads the saved
                    // answers (design part 8.4). A filed or discarded job runs no step.
                    let book = match record.state {
                        State::Filed | State::Discarded => LookupBook::default(),
                        _ => rebuild_book(&job_dir),
                    };
                    lock(&shown).insert(record.id.clone(), record.clone());
                    let lookups = Arc::new(Mutex::new(book));
                    let job = Job { record, settings, generation: 0, disc: None, lookups };
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
        let mut owner = Owner {
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
            gate: ProgressGate::new(),
            selftest,
            share_secrets: BTreeSet::new(),
            checks: VecDeque::new(),
            checking: false,
            check_cancel: None,
            slot_events: BTreeMap::new(),
            retry_rounds: BTreeSet::new(),
            rescans: BTreeSet::new(),
            quiet: None,
        };
        owner.restart(present);
        owner
    }

    /// The restart sequence (design part 5.10, L1, L1a, Rulings S70 to S75), run before `start` returns:
    /// 1. each disc in a drive is fingerprinted as an arrival's walk would be (`read_arrival`), and a drive record
    ///    whose drive holds no disc now is emptied, as a removal would;
    /// 2. `plan_restart` over the loaded jobs, those discs and drives.json;
    /// 3. each fed job has its disc noted (the token job's drive becomes its record's drive), and its EngineRestart
    ///    transition is persisted through the stop hook, whose true answer stops the engine before any resume action
    ///    runs; then, in each noted drive no fed job holds, the disc of the latest job of its fingerprint is noted when
    ///    that job failed in its disc phase, and the drive becomes its record's drive, so a Retry uses the disc
    ///    (spec 10.3);
    /// 4. the space promises of the resumed jobs (`rebuild_promises`), the token and the slot are
    ///    rebuilt, and the noted drives are recorded handled;
    /// 5. the resume actions are performed, the slot is granted, then the arrivals are routed as fresh insertions.
    fn restart(&mut self, present: Vec<(String, PathBuf)>) {
        let host = Arc::clone(&self.parts.host);
        let mut present = present;
        present.sort();
        let mut walked = Vec::new();
        for (drive, root) in present {
            let slot = self.drive(&drive);
            slot.number += 1;
            slot.root = Some(root.clone());
            let number = slot.number;
            walked.push(read_arrival(&drive, &root, number, host.as_ref()).map_err(|sentence| (drive, sentence)));
        }
        let discs: Vec<(String, Option<String>)> = walked
            .iter()
            .map(|walk| match walk {
                Ok(arrival) => (arrival.drive.clone(), arrival.meta.fingerprint.clone()),
                Err((drive, _)) => (drive.clone(), None),
            })
            .collect();
        let emptied: Vec<String> = self
            .drive_records
            .drives
            .iter()
            .filter(|(drive, record)| record.fingerprint.is_some() && !discs.iter().any(|(id, _)| id == *drive))
            .map(|(drive, _)| drive.clone())
            .collect();
        for drive in emptied {
            self.record_drive(&drive, None, false);
        }
        let records: Vec<JobRecord> = self.jobs.values().map(|job| job.record.clone()).collect();
        let plan = plan_restart(&records, &discs, &self.drive_records);
        let mut resumed = Vec::new();
        for (id, disc_present) in &plan.feeds {
            let Some(job) = self.jobs.get_mut(id) else {
                continue;
            };
            let fingerprint = Some(job.record.fingerprint.as_str());
            let carrier = discs.iter().find(|(_, found)| found.as_deref() == fingerprint);
            let drive = carrier.map(|(drive, _)| drive.clone()).filter(|_| *disc_present);
            let token = plan.token.as_deref() == Some(id.as_str());
            if let (true, Some(drive)) = (token, &drive) {
                job.record.drive = drive.clone();
            }
            job.disc = drive;
            let event = Event::EngineRestart { disc_present: *disc_present };
            let (new, actions) = match reduce(&job.record.state, &event) {
                Ok(next) => next,
                Err(rejected) => {
                    let line = format!("rejected {} in {}", event_tag(&event), rejected.state.as_str());
                    self.store.job_log(id, &line);
                    continue;
                }
            };
            let Some(transition) = self.persist_transition(id, Some(event), new, actions, None) else {
                if self.stopping {
                    return;
                }
                continue;
            };
            if token {
                self.token.take(id);
            }
            self.publish(id);
            resumed.push((id.clone(), transition.actions));
        }
        self.note_failed_discs(&plan.noted, &discs);
        {
            let mut promises = lock(&self.parts.promises);
            for (id, _) in &resumed {
                let Some(job) = self.jobs.get(id) else {
                    continue;
                };
                let (shares, job_dir) = (self.parts.shares.as_ref(), self.store.job_dir(id));
                rebuild_promises(&mut promises, shares, &job.record, &job.settings, &job_dir);
            }
        }
        self.slots.seed(&plan.slot_holders, &plan.slot_queue);
        for drive in &plan.noted {
            let fingerprint = discs.iter().find(|(id, _)| id == drive).and_then(|(_, found)| found.clone());
            let record = DriveRecord { fingerprint: fingerprint.clone(), handled: true };
            if self.drive_records.drives.get(drive) != Some(&record) {
                self.record_drive(drive, fingerprint, true);
            }
        }
        for (id, actions) in resumed {
            self.perform(&id, &actions, None);
        }
        self.grant_next();
        let arrivals = walked.into_iter().filter(|walk| {
            let drive = match walk {
                Ok(arrival) => &arrival.drive,
                Err((drive, _)) => drive,
            };
            plan.arrivals.contains(drive)
        });
        for outcome in arrivals.collect::<Vec<_>>() {
            if self.stopping {
                return;
            }
            self.on_arrival(outcome);
        }
    }

    /// Step 3's second half (spec 10.3, L1a): in each of the `noted` drives that no job's disc is in, the disc's
    /// latest job, by (`created_unix`, `id`), has its disc noted there when it is `Failed` from a disc-phase state, and
    /// that drive becomes its record's drive (written with its next transition). A Retry then plans or rips with the
    /// disc, as it would have before the restart, instead of waiting for a removal and a reinsertion.
    fn note_failed_discs(&mut self, noted: &[String], discs: &[(String, Option<String>)]) {
        for drive in noted {
            let Some(fingerprint) = discs.iter().find(|(id, _)| id == drive).and_then(|(_, found)| found.clone()) else {
                continue;
            };
            if self.jobs.values().any(|job| job.disc.as_deref() == Some(drive.as_str())) {
                continue;
            }
            let latest = self
                .jobs
                .values_mut()
                .filter(|job| job.record.fingerprint == fingerprint)
                .max_by(|a, b| (a.record.created_unix, &a.record.id).cmp(&(b.record.created_unix, &b.record.id)));
            let Some(job) = latest else {
                continue;
            };
            if job.disc.is_none() && matches!(&job.record.state, State::Failed { from, .. } if from.is_disc_phase()) {
                job.disc = Some(drive.clone());
                job.record.drive = drive.clone();
            }
        }
    }

    /// The loop: one message at a time, until `Msg::Stop`, the stop hook, which may already have stopped the restart
    /// sequence (`Owner::restart`), or a `shutdown` request. After the restart sequence and after each message, a
    /// changed MakeMKV version or key state is delivered (design part 3.7). While a forced shutdown waits for its
    /// steps, only `quiet_message` runs, and the shutdown ends once no step runs (design part 3.8).
    fn run(mut self, inbox: Receiver<Msg>) {
        self.makemkv_changed();
        self.after_message();
        while !self.stopping {
            let Ok(message) = inbox.recv() else {
                break;
            };
            if matches!(message, Msg::Stop) {
                break;
            }
            if self.quiet.is_some() {
                self.quiet_message(message);
            } else {
                self.on_message(message);
            }
            if self.quiet.is_some() && self.workers.is_empty() {
                self.finish_shutdown();
            }
            self.makemkv_changed();
            self.after_message();
        }
        self.shut_down(&inbox);
    }

    /// One message, outside a forced shutdown.
    fn on_message(&mut self, message: Msg) {
        match message {
            Msg::Stop => {}
            Msg::Drive(DriveEvent::Arrived { drive, root }) => self.on_arrived(drive, root),
            Msg::Drive(DriveEvent::Removed { drive }) => self.on_removed(&drive),
            Msg::Arrival(outcome) => self.on_arrival(outcome),
            Msg::StepDone { job, generation, result } => self.on_step_done(&job, generation, result),
            Msg::Progress { job, step, title, fraction } => self.on_progress(job, step, title, fraction),
            Msg::Timer { job } => self.on_timer(&job),
            Msg::RoundDone { job, pending } => self.on_round_done(&job, pending),
            Msg::Ejected { drive } => self.on_ejected(&drive),
            Msg::Connect(stream) => self.on_connect(stream),
            Msg::Request { client, id, request } => self.on_request(client, id, request),
            Msg::Checked { client, id, outcome } => self.on_checked(client, id, outcome),
            Msg::CheckListed => self.release_check_token(),
        }
    }

    /// One message while a forced shutdown waits for its steps (design part 3.8, PF-29): a step's end is fed, with
    /// only its notifications performed, progress still goes out, and nothing new starts: drive events, arrivals,
    /// timers and lookup rounds are dropped, a new client is turned away and every request is refused.
    fn quiet_message(&mut self, message: Msg) {
        match message {
            Msg::StepDone { job, generation, result } => self.on_step_done(&job, generation, result),
            Msg::Progress { job, step, title, fraction } => self.on_progress(job, step, title, fraction),
            Msg::Ejected { drive } => self.on_ejected(&drive),
            Msg::RoundDone { job, .. } => {
                self.rounds_running.remove(&job);
                self.retry_rounds.remove(&job);
            }
            Msg::Request { client, id, .. } => self.parts.clients.send(client, reply_error(id, STOPPING, &[])),
            Msg::Checked { client, id, .. } => {
                self.checking = false;
                self.check_cancel = None;
                self.release_check_token();
                self.parts.clients.send(client, reply_error(id, STOPPING, &[]));
            }
            Msg::CheckListed => self.release_check_token(),
            Msg::Connect(stream) => drop(stream),
            Msg::Drive(_) | Msg::Arrival(_) | Msg::Timer { .. } | Msg::Stop => {}
        }
    }

    /// The eject of an already-ripped disc returned.
    fn on_ejected(&mut self, drive: &str) {
        self.ejects = self.ejects.saturating_sub(1);
        self.drive(drive).ejecting = false;
    }

    /// After the restart sequence and after each message: finished threads dropped, KeepAwake held or released, and
    /// the busy flag set (Ruling R24.c1).
    fn after_message(&mut self) {
        self.threads.retain(|thread| !thread.is_finished());
        self.keep_awake();
        let busy = !self.workers.is_empty() || !self.rounds_running.is_empty() || self.checking;
        self.parts.busy.store(busy, Ordering::SeqCst);
    }

    /// Cancels every running step and the running check, drops their results and waits for every step, recording,
    /// eject and check the owner started (design part 3.5), then releases KeepAwake and closes every client.
    fn shut_down(&mut self, inbox: &Receiver<Msg>) {
        self.drive_stop.store(true, Ordering::SeqCst);
        for worker in &self.workers {
            worker.cancel.cancel();
        }
        if let Some(cancel) = &self.check_cancel {
            cancel.cancel();
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
        self.parts.clients.close_all();
        // No step runs any more, and a lookup round's late end goes nowhere: a stopped engine is not busy (R24.c1).
        self.parts.busy.store(false, Ordering::SeqCst);
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
        self.rescans.retain(|(rescanned, _)| *rescanned != drive);
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
    /// with no job. A stale success is dropped with a log line; a current one is recorded in drives.json and routed,
    /// except that the walk a `settings_set` started only notes a disc whose drive record holds its fingerprint with
    /// `handled` true (design part 9.4, L1a).
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
                let rescan = self.rescans.remove(&(arrival.drive.clone(), arrival.number));
                let handled = DriveRecord { fingerprint: arrival.meta.fingerprint.clone(), handled: true };
                if rescan && self.drive_records.drives.get(&arrival.drive) == Some(&handled) {
                    return;
                }
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

    /// One transition (design part 7.3, R22.3, R24.2): `persist_transition`, then the actions, the `job_update`
    /// push (S84), the token and the file-phase slot. During a forced shutdown only the transition's notifications
    /// are performed and nothing new starts (PF-29).
    fn commit(
        &mut self,
        id: &str,
        event: Option<Event>,
        new: State,
        actions: Vec<Action>,
        message: Option<String>,
        recording: Option<(Arrival, PathBuf)>,
    ) {
        let Some(transition) = self.persist_transition(id, event, new, actions, message) else {
            return;
        };
        if self.quiet.is_some() {
            self.notify_only(id, &transition.actions);
            self.publish(id);
            return;
        }
        self.perform(id, &transition.actions, recording);
        self.publish(id);
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

    /// A transition persisted before any of its actions runs (design part 7.3, R22.3, R24.2): the record actions
    /// applied (RefreshIdentity's refreshed plan among them), `filed_unix` on entering Filed, `updated_unix`, the
    /// message, then `Store::persist`; then the promises released or rebuilt and the stop hook. None when it could not
    /// be persisted, or when the stop hook stopped the engine.
    fn persist_transition(
        &mut self,
        id: &str,
        event: Option<Event>,
        new: State,
        actions: Vec<Action>,
        message: Option<String>,
    ) -> Option<Transition> {
        let job = self.jobs.get(id)?;
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
            return None;
        }
        lock(&self.shown).insert(id.to_string(), record.clone());
        if let Some(job) = self.jobs.get_mut(id) {
            job.record = record;
        }
        self.release_promises(id, &transition);
        self.promise_on_return(id, &transition);
        if self.stop_after.as_ref().is_some_and(|stop| stop(&transition)) {
            self.stopping = true;
            self.drive_stop.store(true, Ordering::SeqCst);
            return None;
        }
        Some(transition)
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
    /// frees its place, unless it waits in the queue for a Retry or an approval. Either way the slot is granted again.
    fn file_slot(&mut self, id: &str, transition: &Transition) {
        let ejected = State::Ejected { after_copy: false };
        if transition.new == ejected && transition.old.as_ref() != Some(&ejected) {
            self.rounds_done.remove(id);
            self.slots.join(id);
            self.grant_next();
        } else if !self.slot_events.contains_key(id) && self.slots.after(id, &transition.new) {
            self.grant_next();
        }
    }

    /// Grants the file-phase slot while a place is free (design part 5.7, R24.1): a head that waits with a Retry or an
    /// approval is granted its place and fed it (Task 26); a head whose persisted plan is lookup-pending gets one
    /// lookup round first (design part 8.3), and nothing is granted while that round runs; any other head is granted
    /// its place and fed FilePhaseSlot. A head that is no longer an Ejected job, or whose waiting event its state no
    /// longer takes, leaves the queue. Nothing is granted during a forced shutdown (PF-29).
    fn grant_next(&mut self) {
        if self.quiet.is_some() {
            return;
        }
        while let Some(head) = self.slots.next() {
            if let Some(event) = self.slot_events.remove(&head) {
                let state = self.jobs.get(&head).map(|job| job.record.state.clone());
                if state.as_ref().is_some_and(|state| reduce(state, &event).is_ok()) {
                    self.slots.grant(&head);
                    self.feed(&head, event, None);
                    if self.stopping {
                        return;
                    }
                } else {
                    self.slots.leave(&head);
                    let state = state.as_ref().map_or("no state", State::as_str);
                    self.store.job_log(&head, &format!("{} left the slot's queue: it is {state}", event_tag(&event)));
                }
                continue;
            }
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

    /// A lookup round ended (design part 8.3). A round that could not run writes `the lookup round could not run:
    /// <error>` to job.log. In `Ejected`, a lookup still pending takes the job out of the slot's queue and feeds
    /// LookupPending with the waiting message, or with that sentence when the round could not run; otherwise the round
    /// is marked done. A round that could not run is judged by the book's own `pending()` (Ruling R24.c3). In
    /// AwaitingLookup, a lookup still pending feeds LookupPending, which restarts the timer, and none feeds
    /// LookupAnswered; a round that could not run feeds LookupPending with its sentence. A round a `retry` started
    /// feeds only LookupAnswered: its lookup still pending, or the round not run, feeds nothing (Ruling S81). The slot
    /// is granted again afterwards.
    fn on_round_done(&mut self, id: &str, pending: Result<(bool, bool), String>) {
        self.rounds_running.remove(id);
        let by_retry = self.retry_rounds.remove(id);
        let Some(job) = self.jobs.get(id) else {
            return;
        };
        let state = job.record.state.clone();
        let (discdb, tmdb) = match &pending {
            Ok(found) => *found,
            Err(_) => lock(&job.lookups).pending(),
        };
        let waiting = discdb || tmdb;
        if let Err(error) = &pending {
            self.store.job_log(id, &format!("the lookup round could not run: {error}"));
        }
        match state {
            State::Ejected { after_copy: false } if waiting => {
                self.slots.leave(id);
                let message = match &pending {
                    Ok(_) => self.lookup_message(id),
                    Err(error) => Some(format!("the lookup round could not run: {error}")),
                };
                self.feed(id, Event::LookupPending, message);
            }
            State::Ejected { after_copy: false } => {
                self.rounds_done.insert(id.to_string());
            }
            State::AwaitingLookup { .. } => match pending {
                Ok(_) if waiting && by_retry => {}
                Ok(_) if waiting => {
                    let message = self.lookup_message(id);
                    self.feed(id, Event::LookupPending, message);
                }
                Ok(_) => self.feed(id, Event::LookupAnswered, None),
                Err(_) if by_retry => {}
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

    /// A return from a wait, or a Retry, runs no space check, so the promises of the state it returns to are rebuilt
    /// as at a restart (design parts 5.8, 5.10, Rulings S75, R25.c2: `rebuild_promises`), after `release_promises`
    /// released what the job held when it started to wait or failed.
    fn promise_on_return(&self, id: &str, transition: &Transition) {
        let back = transition.old.as_ref().is_some_and(|old| old.is_waiting() || matches!(old, State::Failed { .. }));
        let (true, Some(job)) = (back, self.jobs.get(id)) else {
            return;
        };
        let mut promises = lock(&self.parts.promises);
        let (shares, job_dir) = (self.parts.shares.as_ref(), self.store.job_dir(id));
        rebuild_promises(&mut promises, shares, &job.record, &job.settings, &job_dir);
    }

    /// Releases the token `id` holds although it cannot go on (its folder or a transition could not be written), so
    /// it never holds every later disc behind it, and routes what waited.
    fn give_back(&mut self, id: &str) {
        let next = self.token.after(id, &State::Discarded, &[]);
        self.route_queue(next);
    }

    /// Routes what the token's queue gives while the token is free (design part 5.4): an arrival is routed, a Retry or
    /// return delivered. Nothing is routed during a forced shutdown (PF-29).
    fn route_queue(&mut self, next: Option<Waiting>) {
        if self.quiet.is_some() {
            return;
        }
        let mut next = next;
        while let Some(waiting) = next {
            match waiting {
                Waiting::Arrival(arrival) if !self.is_current(&arrival) => {
                    self.store.log(&format!("{}: the disc was removed before its arrival was handled", arrival.drive));
                }
                Waiting::Arrival(arrival) => self.route(arrival),
                Waiting::Retry(id) => self.deliver(&id, Event::Retry),
                Waiting::Return(id, event) => self.deliver(&id, event),
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
            cancelled: None,
        });
        let now = self.parts.host.now_unix();
        spawn_recording(id.to_string(), generation, arrival, job_dir, now, self.sender.clone());
    }

    /// One worker runs `actions` one after another and returns the last result whose event is Some, else the last
    /// result (design part 5.2). Each action's progress goes to the owner with its tag (S7). Handing Eject marks the
    /// job's drive as ejecting.
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
            cancelled: None,
        });
        let (steps, sender, job) = (Arc::clone(&self.parts.steps), self.sender.clone(), id.to_string());
        self.threads.push(thread::spawn(move || {
            let run = panic::catch_unwind(AssertUnwindSafe(|| {
                let mut kept: Option<StepResult> = None;
                for action in &actions {
                    if cancel.is_cancelled() {
                        break;
                    }
                    let (progress_to, progress_job, step) = (sender.clone(), job.clone(), action_tag(action));
                    let progress = move |title: Option<&str>, fraction: f64| {
                        let (job, step, title) = (progress_job.clone(), step.clone(), title.map(str::to_string));
                        let _ = progress_to.send(Msg::Progress { job, step, title, fraction });
                    };
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

    /// A worker's result (design part 5.2), `step_done`; then, once no `makemkvcon` step is listed, the token
    /// `KILLED_HOLDER` held for a pulled disc's killed step is freed and what waited is routed (design part 5.3). The
    /// release follows every result, a cancelled step's included, and comes after the job's own result is handled, so
    /// a queued reinsertion of the same disc never resumes the job before its step's failure is fed.
    fn on_step_done(&mut self, id: &str, generation: u64, result: StepResult) {
        self.step_done(id, generation, result);
        if !self.stopping {
            self.release_killed_token();
        }
    }

    /// One worker's result. The result of a step the owner cancelled is dropped with what it posted, and the cancel's
    /// failure is fed with the message `cancelled by the owner`, then each waiting `cancel` gets its `ok` (design part
    /// 9.4, R-A7, S78). A stale one is dropped with what its step posted (R-A7), unless a newer step of the job runs,
    /// whose posts it must not take. Otherwise the posted edits are applied, each posted event is fed, then the
    /// result's event. A result with no event, from an action other than those that report none, writes `<tag>
    /// returned no event` to job.log, followed by `: <message>` when it has one; then, unless another step of the
    /// job's current generation runs (one a posted event handed out; a stale step does not count), a step that
    /// panicked fails its job (`panic_failure`, design part 5.2) and any other gives the token back, since no step of
    /// the job runs any more (L2).
    fn step_done(&mut self, id: &str, generation: u64, result: StepResult) {
        self.log_panic(id, &result);
        let mut worker = self.finish_worker(id, generation);
        if let Some(cancelled) = worker.as_mut().and_then(|worker| worker.cancelled.take()) {
            if !self.workers.iter().any(|worker| worker.job == id) {
                drop(self.parts.outbox.take(id));
            }
            self.feed(id, cancelled.event, Some(CANCELLED.to_string()));
            for (client, request) in cancelled.replies {
                self.parts.clients.send(client, reply_ok(request, Value::Null));
            }
            return;
        }
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
                let tags = worker.map(|worker| worker.tags).unwrap_or_default();
                let tag = tags.last().cloned().unwrap_or_default();
                if NO_EVENT_ACTIONS.contains(&tag.as_str()) {
                    return;
                }
                let line = match &result.message {
                    Some(message) => format!("{tag} returned no event: {message}"),
                    None => format!("{tag} returned no event"),
                };
                self.store.job_log(id, &line);
                // This result's generation is the job's current one (the stale branch returned above).
                if self.workers.iter().any(|worker| worker.job == id && worker.generation == generation) {
                    // A posted event handed the job a new step, which holds what the job holds and reports for itself.
                    return;
                }
                let panicked = result.message.as_deref().is_some_and(|message| message.contains(PANICKED));
                match self.panic_failure(id, &tags).filter(|_| panicked) {
                    Some(event) => self.feed(id, event, result.message),
                    None if self.quiet.is_none() => self.give_back(id),
                    None => {}
                }
            }
        }
    }

    /// The failure a step that panicked feeds, so its job leaves the working state it would otherwise keep with no
    /// step, the token and the file-phase slot with it (design part 5.2; ruling at the final review): `cancel_event`'s
    /// row for the worker's main action, with EjectFailed for an Eject and ShareLost for a CheckSpace, which have
    /// none. None when the job's state does not take it.
    fn panic_failure(&self, id: &str, tags: &[String]) -> Option<Event> {
        let state = &self.jobs.get(id)?.record.state;
        let event = match main_tag(tags).as_str() {
            "Eject" => Event::EjectFailed,
            "CheckSpace" => Event::ShareLost,
            tag => cancel_event(tag, state),
        };
        reduce(state, &event).is_ok().then_some(event)
    }

    /// Frees the token `KILLED_HOLDER` holds once no `makemkvcon` step is listed any more, and routes what waited
    /// (design part 5.3: the killed step's after-kill pause has then passed).
    fn release_killed_token(&mut self) {
        if self.token.holder() == Some(KILLED_HOLDER) && !self.makemkv_busy() {
            self.give_back(KILLED_HOLDER);
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
    /// fed MediaRemoved; any other job whose disc was in the drive only has it marked absent. When the cancelled step
    /// runs a `makemkvcon`, the token passes to `KILLED_HOLDER` until that step has reported, so the next
    /// `makemkvcon` call waits for the killed one's after-kill pause (design part 5.3).
    fn on_removed(&mut self, drive: &str) {
        self.rescans.retain(|(rescanned, _)| rescanned != drive);
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
            let mut killed = false;
            for worker in self.workers.iter().filter(|worker| worker.job == id) {
                worker.cancel.cancel();
                killed |= worker.runs_makemkv();
            }
            if killed {
                self.token.hold_for(KILLED_HOLDER);
            }
            self.feed(&id, Event::MediaRemoved, None);
            if self.stopping {
                return;
            }
        }
    }

    /// A StartTimer timer ran out (design part 5.2). AwaitingLookup gets a lookup round, unless one runs (design part
    /// 8.3); a running round a `retry` started becomes the timer's, so its end starts the timer again. AwaitingNetwork
    /// gets NetworkRetry through `deliver`: a return to a disc-phase state waits for the token in its queue (design
    /// part 5.4, way (d), L2).
    fn on_timer(&mut self, id: &str) {
        let Some(job) = self.jobs.get(id) else {
            return;
        };
        match job.record.state {
            State::AwaitingLookup { .. } if self.rounds_running.contains(id) => {
                self.retry_rounds.remove(id);
            }
            State::AwaitingLookup { .. } => self.start_round(id),
            State::AwaitingNetwork { .. } => self.deliver(id, Event::NetworkRetry),
            _ => {}
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

/// The IPC clients' side of the owner (design parts 3.7, 3.8, 9.1 to 9.4; Rulings S76 to S84; Task 26): the
/// pushes, the snapshot, each request's answer, the deliveries a Retry, a return or an approval waits for, the checks
/// and the shutdown.
impl Owner {
    /// Broadcasts `job_update` of `id`'s record after its persisted transition (design part 9.2, S84): the summary, the
    /// plan, and `collisions` only in NeedsReview with a list that is not empty.
    fn publish(&self, id: &str) {
        let clients = &self.parts.clients;
        let Some(job) = self.jobs.get(id).filter(|_| clients.count() > 0) else {
            return;
        };
        let record = &job.record;
        let held = matches!(record.state, State::NeedsReview { .. }) && !record.collisions.is_empty();
        let collisions = held.then(|| record.collisions.clone());
        let update = JobUpdate { summary: job_summary(record), plan: record.plan.clone(), collisions };
        clients.broadcast(&push_line(&Push::JobUpdate(update), None));
    }

    /// Performs only the Notify actions of `actions`: a transition during a forced shutdown (PF-29), or a delivery
    /// that runs no disc-phase action without its disc (design part 5.4).
    fn notify_only(&self, id: &str, actions: &[Action]) {
        for action in actions {
            if let Action::Notify { event } = action {
                self.notify_job(id, event);
            }
        }
    }

    /// A running step's progress, pushed as `progress` when the job pushed none in the last second (design part 9.2,
    /// S7).
    fn on_progress(&mut self, job: String, step: String, title: Option<String>, fraction: f64) {
        if !fraction.is_finite() || self.parts.clients.count() == 0 || !self.gate.admit(&job, Instant::now()) {
            return;
        }
        let progress = Progress { job, step, title, fraction: fraction.clamp(0.0, 1.0) };
        self.parts.clients.broadcast(&push_line(&Push::Progress(progress), None));
    }

    /// A client the serve thread accepted (design part 9.2): added to the clients and sent the snapshot before
    /// anything else, then read by its own reader thread.
    fn on_connect(&mut self, stream: IpcStream) {
        let IpcStream { reader, writer } = stream;
        let clients = Arc::clone(&self.parts.clients);
        let client = clients.add(writer);
        clients.send(client, push_line(&Push::Snapshot(self.snapshot()), None));
        spawn_reader(client, reader, clients, self.sender.clone());
    }

    /// Every job's record, as the owner holds it.
    fn records(&self) -> Vec<JobRecord> {
        self.jobs.values().map(|job| job.record.clone()).collect()
    }

    /// The snapshot (design part 9.2, S77): `snapshot_jobs`, the settings in force, `Settings::validate`'s problems and
    /// the unset `staging_root`'s, the secrets' states and the last self-test.
    fn snapshot(&self) -> Snapshot {
        let (settings, mut problems) = self.store.settings();
        if settings.staging_root.is_none() {
            problems.push(staging_unset());
        }
        let secrets = secret_states(self.parts.secrets.as_ref(), &self.share_secrets);
        Snapshot { jobs: snapshot_jobs(&self.records()), settings, problems, secrets, selftest: self.selftest.clone() }
    }

    /// Sends `client` the `ok` of `answer`'s result, or its `error` with no problems.
    fn reply(&self, client: u64, id: Option<u64>, answer: Result<Value, String>) {
        let line = match answer {
            Ok(result) => reply_ok(id, result),
            Err(message) => reply_error(id, &message, &[]),
        };
        self.parts.clients.send(client, line);
    }

    /// One request (design part 9.4): answered here, or later by the step, check or shutdown it waits for (S78).
    fn on_request(&mut self, client: u64, id: Option<u64>, request: Request) {
        let answer = match request {
            Request::SettingsSet { changes } => return self.settings_set(client, id, changes),
            Request::SecretSet { name, value } => self.secret_set(&name, &value),
            Request::Retry { job } => return self.retry(client, id, &job),
            Request::Cancel { job } => return self.cancel(client, id, &job),
            Request::Discard { .. } => Err(DISCARD_REFUSED.to_string()),
            Request::RipAgain { job } => self.rip_again(&job),
            Request::RunSelftest => return self.queue_check(client, id, CheckWork::Selftest),
            Request::History { before } => history_page(&self.records(), before.as_deref())
                .map(|page| serde_json::to_value(page).unwrap_or(Value::Null)),
            Request::Shutdown { force } => return self.shutdown(client, id, force),
            Request::Command { .. } => Err(COMMAND_REFUSED.to_string()),
            Request::ReviewAction { job, action, overrides } => self.review_action(&job, &action, overrides.as_ref()),
            Request::UiQuit => Err(UI_QUIT_REFUSED.to_string()),
        };
        self.reply(client, id, answer);
    }

    /// `settings_set` (design part 9.4, ENG-19): `changes` over the settings in force, one level deep, so an object
    /// value such as `tool_paths` changes only the keys it names (ruling at the final review), read through
    /// `Settings::from_json`. An unreadable result is refused with its error, one with problems with `settings_set
    /// refused: <n> problems` and the problems; then nothing changes. Otherwise settings.json is written, the slot
    /// takes the new `file_phase_concurrency`, the reply carries the new settings, and every disc already in a drive is
    /// an arrival, but one whose drive record says it was handled (L1, L1a). Each job keeps its own settings.
    fn settings_set(&mut self, client: u64, id: Option<u64>, changes: Map<String, Value>) {
        let (current, _) = self.store.settings();
        let mut merged = serde_json::to_value(&current).unwrap_or_else(|_| Value::Object(Map::new()));
        if let Value::Object(object) = &mut merged {
            for (key, value) in changes {
                if let (Some(Value::Object(current)), Value::Object(inner)) = (object.get_mut(&key), &value) {
                    current.extend(inner.clone());
                    continue;
                }
                object.insert(key, value);
            }
        }
        let line = match Settings::from_json(&merged.to_string()) {
            Err(error) => reply_error(id, &error, &[]),
            Ok((_, problems)) if !problems.is_empty() => reply_error(id, &settings_refused(problems.len()), &problems),
            Ok((settings, _)) => match self.store.write_settings(&settings) {
                Err(error) => reply_error(id, &format!("settings.json could not be written: {error}"), &[]),
                Ok(()) => {
                    self.slots.set_capacity(settings.file_phase_concurrency as usize);
                    let result = serde_json::to_value(&settings).unwrap_or(Value::Null);
                    self.parts.clients.send(client, reply_ok(id, result));
                    self.grant_next();
                    self.present_discs_arrive(&settings);
                    return;
                }
            },
        };
        self.parts.clients.send(client, line);
    }

    /// Design part 9.4's arrival rule after a `settings_set`: each drive with a disc in, not ejecting and not queued
    /// already, gets a new arrival number and a fingerprint walk at once; its result is routed as a fresh insertion
    /// unless the drive record holds its fingerprint with `handled` true (`on_arrival`, L1a).
    fn present_discs_arrive(&mut self, settings: &Settings) {
        let ready = Duration::from_secs(u64::from(settings.drive_ready_timeout_s));
        let present: Vec<(String, PathBuf)> = self
            .drives
            .iter()
            .filter(|(drive, slot)| !slot.ejecting && !self.token.queued_arrival(drive))
            .filter_map(|(drive, slot)| slot.root.clone().map(|root| (drive.clone(), root)))
            .collect();
        for (drive, root) in present {
            let slot = self.drive(&drive);
            slot.number += 1;
            let number = slot.number;
            self.rescans.insert((drive.clone(), number));
            let (host, sender) = (Arc::clone(&self.parts.host), self.sender.clone());
            spawn_arrival(drive, root, number, Duration::ZERO, ready, host, sender);
        }
    }

    /// `secret_set` (design part 9.4, S77): a name `is_secret_name` takes is stored through `SecretStore`, and a share
    /// name joins the names the snapshot lists; the value is never echoed or logged.
    fn secret_set(&mut self, name: &str, value: &str) -> Result<Value, String> {
        if !is_secret_name(name) {
            return Err(not_a_secret(name));
        }
        self.parts.secrets.set(name, value).map_err(|error| format!("{name} could not be stored: {error}"))?;
        if !SECRET_NAMES.contains(&name) {
            self.share_secrets.insert(name.to_string());
        }
        Ok(Value::Null)
    }

    /// `retry` (design part 9.4, L3, S81). A failed job gets Retry: through the token's queue when its failed step is a
    /// disc-phase step, through the slot's queue otherwise. A waiting job's condition is checked again: a lookup round
    /// in AwaitingLookup, NetworkRetry in AwaitingNetwork, and a re-check (`CheckEnv::recheck`) for the others, whose
    /// reply waits for it. Every other state is refused.
    fn retry(&mut self, client: u64, id: Option<u64>, job_id: &str) {
        let Some(job) = self.jobs.get(job_id) else {
            return self.reply(client, id, Err(no_job(job_id)));
        };
        let state = job.record.state.clone();
        match &state {
            State::Failed { from, .. } if reduce(&state, &Event::Retry).is_ok() => {
                if from.is_disc_phase() {
                    self.deliver(job_id, Event::Retry);
                } else {
                    self.wait_for_slot(job_id, Event::Retry);
                }
                self.reply(client, id, Ok(Value::Null));
            }
            State::AwaitingLookup { .. } => {
                if !self.rounds_running.contains(job_id) {
                    self.start_round(job_id);
                    self.retry_rounds.insert(job_id.to_string());
                }
                self.reply(client, id, Ok(Value::Null));
            }
            State::AwaitingNetwork { .. } => {
                self.deliver(job_id, Event::NetworkRetry);
                self.reply(client, id, Ok(Value::Null));
            }
            waiting if waiting.is_waiting() => self.queue_check(client, id, CheckWork::Recheck(job_id.to_string())),
            other => self.reply(client, id, Err(no_retry(job_id, other))),
        }
    }

    /// Delivers `event`, a Retry or a waiting state's return, to `id` (design part 5.4, way (d), L3). An event that
    /// re-enters a disc-phase state needs the disc-phase token: while another job holds it, it waits in the token's
    /// queue (once per job); with the token, a job whose disc is in no drive runs no disc-phase action
    /// (`without_disc`). Any other event is fed at once.
    fn deliver(&mut self, id: &str, event: Event) {
        let Some(job) = self.jobs.get(id) else {
            return;
        };
        let new = match reduce(&job.record.state, &event) {
            Ok((new, _)) => new,
            Err(rejected) => {
                let line = format!("rejected {} in {}", event_tag(&event), rejected.state.as_str());
                return self.store.job_log(id, &line);
            }
        };
        if !new.is_disc_phase() {
            return self.feed(id, event, None);
        }
        if !self.token.take(id) {
            if !self.token.queued_job(id) {
                let waiting = match event {
                    Event::Retry => Waiting::Retry(id.to_string()),
                    other => Waiting::Return(id.to_string(), other),
                };
                self.token.wait(waiting);
            }
            return;
        }
        if job.disc.is_none() {
            return self.without_disc(id, event);
        }
        self.feed(id, event, None);
    }

    /// A Retry or return delivered while the job's disc is in no drive (design part 5.4): its transition is persisted
    /// with only its notifications performed, then MediaRemoved is fed, whose WaitForDisc frees the token.
    fn without_disc(&mut self, id: &str, event: Event) {
        let Some(job) = self.jobs.get(id) else {
            return;
        };
        let Ok((new, actions)) = reduce(&job.record.state, &event) else {
            return;
        };
        let Some(transition) = self.persist_transition(id, Some(event), new, actions, None) else {
            return;
        };
        self.notify_only(id, &transition.actions);
        self.publish(id);
        self.feed(id, Event::MediaRemoved, None);
    }

    /// Queues `id` for the file-phase slot with `event`, a Retry of a file-phase failure or an approval, fed when the
    /// slot is free for it (design part 5.7); the job then holds the slot.
    fn wait_for_slot(&mut self, id: &str, event: Event) {
        self.slot_events.insert(id.to_string(), event);
        self.slots.join(id);
        self.grant_next();
    }

    /// `cancel` (design part 9.4, R-A7, S78): refused for a job with no running step, naming its state, and for a
    /// CheckSpace or Eject. The running step is the worker of the job's current generation, or one an earlier `cancel`
    /// is stopping; a stale step left by a removal is not one. Otherwise the step's CancelToken is raised, the job's
    /// generation bumped so the step's own result is stale, and the `ok` waits until the step has ended and its
    /// failure is persisted (`on_step_done`).
    fn cancel(&mut self, client: u64, id: Option<u64>, job_id: &str) {
        let Some(job) = self.jobs.get_mut(job_id) else {
            return self.reply(client, id, Err(no_job(job_id)));
        };
        let generation = job.generation;
        let live = self.workers.iter().position(|worker| worker.job == job_id && worker.generation == generation);
        let stopping = || self.workers.iter().position(|worker| worker.job == job_id && worker.cancelled.is_some());
        let at = live.or_else(stopping);
        let Some(worker) = at.map(|at| &mut self.workers[at]) else {
            let refusal = no_running_step(job_id, &job.record.state);
            return self.reply(client, id, Err(refusal));
        };
        let tag = main_tag(&worker.tags);
        if tag == "CheckSpace" || tag == "Eject" {
            return self.reply(client, id, Err(not_cancellable(job_id, &tag)));
        }
        match &mut worker.cancelled {
            Some(cancelled) => cancelled.replies.push((client, id)),
            None => {
                let event = cancel_event(&tag, &job.record.state);
                worker.cancelled = Some(Cancelled { event, replies: vec![(client, id)] });
                worker.cancel.cancel();
                job.generation += 1;
            }
        }
    }

    /// `rip_again` (design part 9.4): RipAgain for a job past its disc phase, which arms it; refused otherwise.
    fn rip_again(&mut self, job_id: &str) -> Result<Value, String> {
        let job = self.jobs.get(job_id).ok_or_else(|| no_job(job_id))?;
        if reduce(&job.record.state, &Event::RipAgain).is_err() {
            return Err(no_rip_again(job_id, &job.record.state));
        }
        self.feed(job_id, Event::RipAgain, None);
        Ok(Value::Null)
    }

    /// `review_action` (design part 9.4, P25, R-A8, S82): only `approve` with no overrides (absent, null or empty) of
    /// a job held at the disposition gate, which waits for the slot with `ReviewApproved` of default overrides and the
    /// job's summary's review reasons. Every other action, and `approve` with overrides, waits for phase 3.
    fn review_action(&mut self, job_id: &str, action: &str, overrides: Option<&Value>) -> Result<Value, String> {
        let empty = match overrides {
            None | Some(Value::Null) => true,
            Some(Value::Object(fields)) => fields.is_empty(),
            Some(Value::Array(items)) => items.is_empty(),
            Some(_) => false,
        };
        if action != "approve" || !empty {
            return Err(waits_for_review(action));
        }
        let job = self.jobs.get(job_id).ok_or_else(|| no_job(job_id))?;
        if job.record.state != (State::NeedsReview { held: Held::DispositionGate }) {
            return Err(not_held(job_id, &job.record.state));
        }
        let approved_reasons = job_summary(&job.record).review_reasons;
        self.wait_for_slot(job_id, Event::ReviewApproved { overrides: Overrides::default(), approved_reasons });
        Ok(Value::Null)
    }

    /// Queues a self-test or a re-check; they run one at a time, each on its own thread (design part 4.4).
    fn queue_check(&mut self, client: u64, id: Option<u64>, work: CheckWork) {
        self.checks.push_back(PendingCheck { client, id, work });
        self.start_check();
    }

    /// Whether a `makemkvcon` runs for a job: a RunScan or RipTitle worker, whose listing is part of it (design part
    /// 3.7, S80).
    fn makemkv_busy(&self) -> bool {
        self.workers.iter().any(Worker::runs_makemkv)
    }

    /// Whether `work` runs the MakeMKV check's drive listing: a self-test, or the re-check of a job in AwaitingMakeMkv
    /// or in AwaitingTool returning to the disc phase or an extraction (Ruling S81).
    fn lists_drives(&self, work: &CheckWork) -> bool {
        let CheckWork::Recheck(job) = work else {
            return true;
        };
        match self.jobs.get(job).map(|job| &job.record.state) {
            Some(State::AwaitingMakeMkv { .. }) => true,
            Some(State::AwaitingTool { return_to }) => {
                return_to.is_disc_phase() || matches!(**return_to, State::Extracting)
            }
            _ => false,
        }
    }

    /// Starts the next queued check on its own thread, unless one runs: a self-test over the engine's settings, or a
    /// re-check of the job with its own settings; its end comes back as `Msg::Checked`. A check that lists the drives
    /// takes the disc-phase token for its MakeMKV check, so no step runs a `makemkvcon` beside it; while a job holds
    /// the token, or a `makemkvcon` runs for a job, it takes nothing and its MakeMKV check reports the last one marked
    /// busy (Ruling R26.c1, design part 3.7, S79). A self-test frees the token once its MakeMKV check has ended
    /// (`Msg::CheckListed`); a re-check, when it ends. The check gets a CancelToken of its own (design part 5.3).
    fn start_check(&mut self) {
        if self.checking || self.quiet.is_some() {
            return;
        }
        let Some(PendingCheck { client, id, work }) = self.checks.pop_front() else {
            return;
        };
        let busy = self.lists_drives(&work) && (self.makemkv_busy() || !self.token.take(CHECK_HOLDER));
        let cancel = CancelToken::default();
        let env = CheckEnv { cancel: cancel.clone(), ..self.parts.env.clone() };
        let run: Box<dyn FnOnce() -> CheckOutcome + Send> = match work {
            CheckWork::Selftest => {
                let settings = self.store.settings().0;
                let sender = self.sender.clone();
                Box::new(move || {
                    let listed = move || {
                        let _ = sender.send(Msg::CheckListed);
                    };
                    CheckOutcome::Selftest(env.selftest(&settings, busy, &listed))
                })
            }
            CheckWork::Recheck(job) => {
                let Some(held) = self.jobs.get(&job) else {
                    self.release_check_token();
                    self.reply(client, id, Err(no_job(&job)));
                    return self.start_check();
                };
                let input = RecheckJob {
                    record: held.record.clone(),
                    settings: held.settings.clone(),
                    job_dir: self.store.job_dir(&job),
                };
                let (shares, promises) = (Arc::clone(&self.parts.shares), Arc::clone(&self.parts.promises));
                Box::new(move || {
                    let event = env.recheck(&input, busy, shares.as_ref(), &promises);
                    CheckOutcome::Recheck { job, event }
                })
            }
        };
        self.checking = true;
        self.check_cancel = Some(cancel);
        let sender = self.sender.clone();
        self.threads.push(thread::spawn(move || {
            let outcome = panic::catch_unwind(AssertUnwindSafe(run))
                .unwrap_or_else(|payload| CheckOutcome::Panicked(panic_text(&*payload)));
            let _ = sender.send(Msg::Checked { client, id, outcome });
        }));
    }

    /// A check ended (design part 9.4, S78, S81). The disc-phase token it may still hold is freed first. A self-test
    /// becomes the last one, is pushed to every client as `selftest_result`, then answers its request with itself, and
    /// every AwaitingTool job whose re-check passes gets ToolFound. A re-check's event is delivered, then its `retry`
    /// gets `ok`. The next check starts.
    fn on_checked(&mut self, client: u64, id: Option<u64>, outcome: CheckOutcome) {
        self.checking = false;
        self.check_cancel = None;
        self.release_check_token();
        match outcome {
            CheckOutcome::Selftest(selftest) => {
                self.selftest = selftest.clone();
                self.parts.clients.broadcast(&push_line(&Push::SelftestResult(selftest.clone()), None));
                self.reply(client, id, Ok(serde_json::to_value(&selftest).unwrap_or(Value::Null)));
                self.deliver_tool_found(&selftest);
            }
            CheckOutcome::Recheck { job, event } => {
                if let Some(event) = event {
                    self.deliver(&job, event);
                }
                self.reply(client, id, Ok(Value::Null));
            }
            CheckOutcome::Panicked(text) => {
                self.store.log(&format!("a check panicked: {text}"));
                self.reply(client, id, Err(format!("the check panicked: {text}")));
            }
        }
        if !self.stopping {
            self.start_check();
        }
    }

    /// Frees the disc-phase token when a check holds it, through the token's normal path, so what waits in its queue
    /// is routed (Ruling R26.c1).
    fn release_check_token(&mut self) {
        if self.token.holder() == Some(CHECK_HOLDER) {
            self.give_back(CHECK_HOLDER);
        }
    }

    /// After a self-test, each AwaitingTool job whose re-check passes gets ToolFound (Ruling S81, design part 9.4): its
    /// tools found with its own settings, and the self-test's MakeMKV check for a disc-phase return.
    fn deliver_tool_found(&mut self, selftest: &SelfTest) {
        let makemkv = selftest.checks.iter().find(|check| check.name == Tool::Makemkvcon.as_str()).cloned();
        let waiting: Vec<(String, State, Settings)> = self
            .jobs
            .values()
            .filter_map(|job| match &job.record.state {
                State::AwaitingTool { return_to } => {
                    Some((job.record.id.clone(), (**return_to).clone(), job.settings.clone()))
                }
                _ => None,
            })
            .collect();
        for (id, return_to, settings) in waiting {
            let check = makemkv.clone().unwrap_or_else(|| not_checked(Tool::Makemkvcon.as_str()));
            if let Some(event) = self.parts.env.tool_found(&settings, &return_to, || check) {
                self.deliver(&id, event);
                if self.stopping {
                    return;
                }
            }
        }
    }

    /// When a MakeMKV check or listing found the version or licence state changed, every AwaitingMakeMkv job gets
    /// MakeMkvChanged, delivered once the token is free (design part 3.7, S79).
    fn makemkv_changed(&mut self) {
        if self.stopping || self.quiet.is_some() || !self.parts.env.makemkv.take_changed() {
            return;
        }
        let waiting: Vec<String> = self
            .jobs
            .values()
            .filter(|job| matches!(job.record.state, State::AwaitingMakeMkv { .. }))
            .map(|job| job.record.id.clone())
            .collect();
        for id in waiting {
            self.deliver(&id, Event::MakeMkvChanged);
            if self.stopping {
                return;
            }
        }
    }

    /// `shutdown` (design part 3.8, S83). With no step running it replies `ok`, closes every client and stops. With a
    /// step running and no `force`, it is refused naming the jobs. With `force`, each running step is cancelled as
    /// `cancel` does, but a CheckSpace or Eject is let finish, and a running check is cancelled; the owner then waits
    /// for every step (`quiet_message`) and ends in `finish_shutdown`.
    fn shutdown(&mut self, client: u64, id: Option<u64>, force: bool) {
        let mut running: Vec<String> = self.workers.iter().map(|worker| worker.job.clone()).collect();
        running.sort();
        running.dedup();
        if running.is_empty() {
            self.reply(client, id, Ok(Value::Null));
            return self.close_and_stop();
        }
        if !force {
            return self.reply(client, id, Err(steps_running(&running)));
        }
        if let Some(cancel) = &self.check_cancel {
            cancel.cancel();
        }
        let mut cancelled = Vec::new();
        for worker in &mut self.workers {
            let tag = main_tag(&worker.tags);
            if tag == "CheckSpace" || tag == "Eject" {
                continue;
            }
            cancelled.push((worker.job.clone(), tag.clone()));
            if worker.cancelled.is_none() {
                let state = self.jobs.get(&worker.job).map_or(State::Discarded, |job| job.record.state.clone());
                worker.cancelled = Some(Cancelled { event: cancel_event(&tag, &state), replies: Vec::new() });
                worker.cancel.cancel();
                if let Some(job) = self.jobs.get_mut(&worker.job) {
                    job.generation += 1;
                }
            }
        }
        self.quiet = Some(Quiet { client, id, cancelled });
    }

    /// The end of a forced shutdown, once no step runs (S83, narrowed at the final review): the partial files of each
    /// cancelled job are deleted, but not those of a job whose cancelled step was File, whose partial files are the
    /// verified remuxes its Retry renames (design part 3.8: what stays is for Retry); then the `shutdown` gets its
    /// `ok`, every client is closed and the owner stops.
    fn finish_shutdown(&mut self) {
        let Some(quiet) = self.quiet.take() else {
            return;
        };
        for (job, tag) in &quiet.cancelled {
            if tag != "File" {
                self.delete_partials(job);
            }
        }
        self.reply(quiet.client, quiet.id, Ok(Value::Null));
        self.close_and_stop();
    }

    /// Refuses every queued check, closes every client after what is queued for it, and stops the owner (design part
    /// 3.8).
    fn close_and_stop(&mut self) {
        for PendingCheck { client, id, .. } in std::mem::take(&mut self.checks) {
            self.reply(client, id, Err(STOPPING.to_string()));
        }
        self.parts.clients.close_all();
        self.stopping = true;
        self.drive_stop.store(true, Ordering::SeqCst);
    }

    /// Deletes `<root>/.lazymkv-partial/<id>-*.mkv.partial` in every library root of the engine's and the job's
    /// settings (S83); a file that cannot be deleted is logged.
    fn delete_partials(&self, id: &str) {
        let (engine, _) = self.store.settings();
        let mut roots: BTreeSet<String> = [engine.movies_root, engine.tv_root].into_iter().flatten().collect();
        if let Some(job) = self.jobs.get(id) {
            roots.extend([job.settings.movies_root.clone(), job.settings.tv_root.clone()].into_iter().flatten());
        }
        let prefix = format!("{id}-");
        for root in roots {
            let Ok(entries) = fs::read_dir(Path::new(&root).join(PARTIAL_FOLDER)) else {
                continue;
            };
            for entry in entries.filter_map(|entry| entry.ok()) {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.starts_with(&prefix) && name.ends_with(".mkv.partial") {
                    if let Err(error) = fs::remove_file(entry.path()) {
                        let shown = entry.path().display().to_string();
                        self.store.log(&format!("{shown}: the partial file was not deleted: {error}"));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
