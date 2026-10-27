//! The IPC (inter-process communication) server (design parts 9.1 to 9.4): the serve thread that accepts each client
//! from the platform's `IpcTransport`, each client's reader thread and its writer thread behind a bounded queue
//! (design part 4.4, S52), the capped line reader, the snapshot's job list and the history pages, the host a webhook
//! secret shows, the progress gate and the engine's refusal texts (Ruling S76). The owner thread answers every request
//! (owner.rs); nothing here touches a job.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, BufRead, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Sender, SyncSender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use lazymkv_core::ipc::{
    job_summary, parse_request, reply_error, JobSummary, SecretState, HISTORY_PAGE, LINE_TOO_LONG, MAX_LINE_BYTES,
    SECRET_NAMES,
};
use lazymkv_core::job::{reduce, Event, Failure, FailureClass, State};
use lazymkv_core::record::JobRecord;
use lazymkv_core::settings::SettingsProblem;
use lazymkv_platform::{IpcTransport, SecretStore};

use crate::owner::{Msg, NO_EVENT_ACTIONS};

/// One client's queue length; a full queue drops the client (design part 4.4, S52). Def: T26. Use: T26.
pub(crate) const CLIENT_QUEUE: usize = 256;

/// How often a job may push progress (design part 9.2).
const PROGRESS_EVERY: Duration = Duration::from_secs(1);

/// The two secrets whose host the snapshot shows (design part 9.2).
const WEBHOOK_SECRETS: [&str; 2] = ["ha_webhook_url", "webhook_url"];

/// `discard`'s refusal (Ruling S76). Def: T26. Use: T26.
pub(crate) const DISCARD_REFUSED: &str = "discard waits for phase 3's review commands (P2, R-A1)";
/// `ui_quit`'s refusal (Ruling S76). Def: T26. Use: T26.
pub(crate) const UI_QUIT_REFUSED: &str = "ui_quit waits for phase 5's user interface (R-A8)";
/// `command`'s refusal until pull request 3 (Ruling S76). Def: T26. Use: T26.
pub(crate) const COMMAND_REFUSED: &str = "command needs the engine's run folder, which arrives in pull request 3 (S35)";
/// The reply to a request that comes while a forced shutdown waits for its steps (design part 3.8). Def: T26.
/// Use: T26.
pub(crate) const STOPPING: &str = "the engine is stopping (design part 3.8)";

/// `no job <id>` (Ruling S76). Def: T26. Use: T26.
pub(crate) fn no_job(id: &str) -> String {
    format!("no job {id}")
}

/// A `cancel` of a job with no running step names its state (Ruling S76). Def: T26. Use: T26.
pub(crate) fn no_running_step(id: &str, state: &State) -> String {
    format!("job {id} has no running step: it is {}", state.as_str())
}

/// A `cancel` during CheckSpace or Eject is refused (Ruling S76, design part 9.4). Def: T26. Use: T26.
pub(crate) fn not_cancellable(id: &str, tag: &str) -> String {
    format!("job {id} is running {tag}, which cancel does not stop (design part 9.4)")
}

/// An `approve` outside the disposition gate names the job's state (Ruling S76). Def: T26. Use: T26.
pub(crate) fn not_held(id: &str, state: &State) -> String {
    format!("job {id} is {}, not held at the disposition gate", state.as_str())
}

/// Every review action but `approve` with no overrides waits for phase 3 (Ruling S76, R-A8). Def: T26. Use: T26.
pub(crate) fn waits_for_review(action: &str) -> String {
    format!("{action} waits for phase 3's review commands (R-A8)")
}

/// A `shutdown` without `force` while steps run names their jobs (Ruling S76, design part 3.8). Def: T26. Use: T26.
pub(crate) fn steps_running(ids: &[String]) -> String {
    format!("a step is running for {}; stop --force cancels it (design part 3.8)", ids.join(", "))
}

/// A `secret_set` of a name `is_secret_name` refuses (Ruling S76). Def: T26. Use: T26.
pub(crate) fn not_a_secret(name: &str) -> String {
    format!("{name} is not a secret name (design part 9.4)")
}

/// The answer to a `cancel` whose failure transition could not be persisted (S78 as amended at review round 1, V1,
/// RR1-4): the job keeps its persisted state until a restart resumes it. Def: PR #6 review (V1). Use: owner.rs.
pub(crate) fn cancel_not_persisted(id: &str) -> String {
    format!("the step of {id} was cancelled, but its failure could not be persisted; a restart resumes it")
}

/// A `settings_set` whose result has problems (Ruling S76). Def: T26. Use: T26.
pub(crate) fn settings_refused(count: usize) -> String {
    format!("settings_set refused: {count} problems")
}

/// A `retry` of a job that is neither failed with a class Retry takes nor waiting (design part 9.4). Def: T26.
/// Use: T26.
pub(crate) fn no_retry(id: &str, state: &State) -> String {
    format!("job {id} is {}; retry takes a failed or waiting job that can run again (design part 9.4)", state.as_str())
}

/// A `rip_again` of a job still in its disc phase (design part 9.4). Def: T26. Use: T26.
pub(crate) fn no_rip_again(id: &str, state: &State) -> String {
    format!("job {id} is {}; rip_again takes a job past its disc phase (design part 9.4)", state.as_str())
}

/// The problem the snapshot adds while `staging_root` is unset (Ruling S77). Def: T26. Use: T26.
pub(crate) fn staging_unset() -> SettingsProblem {
    SettingsProblem {
        key: "staging_root".to_string(),
        problem: "staging_root is unset: the engine starts no job until it is set (design part 9.4)".to_string(),
    }
}

/// The tag of the action a worker's list runs as the job's step: the last that reports an event, else the last
/// (design part 5.2, PF-6). Def: T26. Use: T26.
pub(crate) fn main_tag(tags: &[String]) -> String {
    tags.iter()
        .rev()
        .find(|tag| !NO_EVENT_ACTIONS.contains(&tag.as_str()))
        .or_else(|| tags.last())
        .cloned()
        .unwrap_or_default()
}

/// The failure `cancel` feeds for the step `tag` running in `state`, class Unknown (design part 9.4's table, R-A7):
/// Fingerprint, RunScan, Plan, RipTitle, VerifyFile in Ripping (as TitleRipFailed), Probe, Decide with its subtitle
/// evidence, Remux, VerifyFile in Remuxing and File each give their step's failure; any other action the failure of
/// the step its state runs. Def: T26. Use: T26.
pub(crate) fn cancel_event(tag: &str, state: &State) -> Event {
    let failure = Failure { class: FailureClass::Unknown, output: None };
    match (tag, state) {
        ("Fingerprint", _) | (_, State::Detected) => Event::FingerprintFailed { failure },
        ("RunScan", _) | (_, State::Fingerprinted) => Event::ScanFailed { failure },
        ("Plan", _) | (_, State::Scanned) => Event::PlanFailed { failure },
        ("RipTitle" | "EmptyTitleDirectory" | "VerifyFile", State::Ripping) => Event::TitleRipFailed { failure },
        ("Probe", _) => Event::ProbeFailed { failure },
        ("ExtractSubtitleEvidence" | "Decide", _) | (_, State::Inspecting | State::Decided { .. }) => {
            Event::DecisionFailed { failure }
        }
        ("VerifyFile", State::Remuxing) => Event::FileVerifyFailed { failure },
        ("Remux" | "DeletePartialFile", _) | (_, State::Remuxing) => Event::RemuxFailed { failure },
        (_, State::Ripping) => Event::TitleRipFailed { failure },
        _ => Event::FileFailed { failure },
    }
}

/// The failure a step `tag` that ended with no event of its own feeds in `state`, so its job leaves the working state
/// it would otherwise keep with no step (design part 5.2; the final review's I2, amended at review round 1, V11,
/// RR1-12): EjectFailed for an Eject, else `cancel_event`'s row, when the reducer takes it in `state`; otherwise
/// ShareLost for a CheckSpace whose state takes no failure row (Planned); None when the state takes neither. A
/// CheckSpace in Decided gets `cancel_event`'s DecisionFailed, so it fails as Unknown and frees the file-phase slot.
/// With the tag "" it is the failure row of the state itself, which a transition that could not be persisted feeds
/// (review round 1, V1). Def: PR #6 review (V11). Use: owner.rs.
pub(crate) fn failure_event(tag: &str, state: &State) -> Option<Event> {
    let failed = if tag == "Eject" { Event::EjectFailed } else { cancel_event(tag, state) };
    if reduce(state, &failed).is_ok() {
        return Some(failed);
    }
    (tag == "CheckSpace" && reduce(state, &Event::ShareLost).is_ok()).then_some(Event::ShareLost)
}

/// Locks `mutex`, ignoring a poison a panicked thread left.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Every connected client's writer thread (design part 9.2): each client's lines go through a queue of
/// `CLIENT_QUEUE` lines that its own thread drains into its writer, so a slow client never slows the engine. A client
/// whose queue is full, or whose writer failed, is dropped (design part 4.4, S52). Def: T26. Use: T26.
pub(crate) struct Clients {
    /// Each client's queue, by client number.
    queues: Mutex<BTreeMap<u64, SyncSender<String>>>,
    /// The next client number.
    next: AtomicU64,
}

impl Clients {
    /// No client (design part 9.2). Def: T26. Use: T26.
    pub(crate) fn new() -> Clients {
        Clients { queues: Mutex::new(BTreeMap::new()), next: AtomicU64::new(1) }
    }

    /// Adds a client writing to `writer`, with a thread that drains its queue (`sync_channel(CLIENT_QUEUE)`), each
    /// line written with its newline in one write. The thread ends when the writer fails or the client is closed and
    /// its queue drained. Def: T26. Use: T26.
    pub(crate) fn add(&self, writer: Box<dyn Write + Send>) -> u64 {
        let (queue, lines) = sync_channel::<String>(CLIENT_QUEUE);
        let client = self.next.fetch_add(1, Ordering::SeqCst);
        lock(&self.queues).insert(client, queue);
        thread::spawn(move || {
            let mut writer = writer;
            for line in lines {
                let written = writer.write_all(format!("{line}\n").as_bytes()).and_then(|()| writer.flush());
                if written.is_err() {
                    break;
                }
            }
        });
        client
    }

    /// Queues `line` for `client` with `try_send`; a full queue, or a writer that ended, drops the client (S52).
    /// Def: T26. Use: T26.
    pub(crate) fn send(&self, client: u64, line: String) {
        let mut queues = lock(&self.queues);
        if queues.get(&client).is_some_and(|queue| queue.try_send(line).is_err()) {
            queues.remove(&client);
        }
    }

    /// `send` of `line` to every client (design part 9.2). Def: T26. Use: T26.
    pub(crate) fn broadcast(&self, line: &str) {
        lock(&self.queues).retain(|_, queue| queue.try_send(line.to_string()).is_ok());
    }

    /// The clients connected now (design part 9.2). Def: T26. Use: T26.
    pub(crate) fn count(&self) -> usize {
        lock(&self.queues).len()
    }

    /// Closes `client`: its thread writes what is queued, then ends and drops its writer, which ends the connection
    /// (design part 9.1). Def: T26. Use: T26.
    pub(crate) fn close(&self, client: u64) {
        lock(&self.queues).remove(&client);
    }

    /// `close` of every client, at shutdown (design part 3.8). Def: T26. Use: T26.
    pub(crate) fn close_all(&self) {
        lock(&self.queues).clear();
    }
}

/// At most one progress push a second per job (design part 9.2). Def: T26. Use: T26.
pub(crate) struct ProgressGate {
    /// When each job's last progress push was let through.
    last: BTreeMap<String, Instant>,
}

impl ProgressGate {
    /// No job pushed yet (design part 9.2). Def: T26. Use: T26.
    pub(crate) fn new() -> ProgressGate {
        ProgressGate { last: BTreeMap::new() }
    }

    /// True, and remembered, when `job` pushed no progress in the second before `now` (design part 9.2). Def: T26.
    /// Use: T26.
    pub(crate) fn admit(&mut self, job: &str, now: Instant) -> bool {
        let quiet = self.last.get(job).is_none_or(|last| now.saturating_duration_since(*last) >= PROGRESS_EVERY);
        if quiet {
            self.last.insert(job.to_string(), now);
        }
        quiet
    }
}

/// Reads one line of at most `cap` bytes before its newline; Ok(None) at end of stream, Ok(Some(Err(()))) when longer.
/// A longer line is reported once `cap + 1` bytes of it are read, without reading the rest (design part 9.1). The
/// newline, and a carriage return before it, are not part of the line; bytes that are not UTF-8 are replaced. A last
/// line without a newline is still a line. Def: T26. Use: T26.
pub(crate) fn read_line_capped(reader: &mut dyn BufRead, cap: usize) -> io::Result<Option<Result<String, ()>>> {
    let mut line = Vec::new();
    loop {
        let available = match reader.fill_buf() {
            Ok(available) => available,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        if available.is_empty() {
            if line.is_empty() {
                return Ok(None);
            }
            return Ok(Some(Ok(text_of(line))));
        }
        let end = available.iter().position(|&byte| byte == b'\n');
        let take = end.unwrap_or(available.len());
        if line.len() + take > cap {
            let room = cap + 1 - line.len();
            reader.consume(room);
            return Ok(Some(Err(())));
        }
        line.extend_from_slice(&available[..take]);
        match end {
            Some(end) => {
                reader.consume(end + 1);
                return Ok(Some(Ok(text_of(line))));
            }
            None => reader.consume(take),
        }
    }
}

/// A line's text without a trailing carriage return, with any byte that is not UTF-8 replaced.
fn text_of(mut line: Vec<u8>) -> String {
    if line.last() == Some(&b'\r') {
        line.pop();
    }
    String::from_utf8_lossy(&line).into_owned()
}

/// The host of a webhook URL, without scheme, user, port or path (design part 9.2). Def: T26. Use: T26.
pub(crate) fn secret_host(url: &str) -> Option<String> {
    let (_, rest) = url.split_once("://")?;
    let authority = &rest[..rest.find(['/', '?', '#']).unwrap_or(rest.len())];
    let host_port = authority.rsplit_once('@').map_or(authority, |(_, host)| host);
    let host = match host_port.strip_prefix('[') {
        Some(bracketed) => &bracketed[..bracketed.find(']')?],
        None => host_port.split_once(':').map_or(host_port, |(host, _)| host),
    };
    (!host.is_empty() && !host.contains(char::is_whitespace)).then(|| host.to_string())
}

/// Whether a job is finished: filed, failed or discarded (design part 9.2).
fn finished(record: &JobRecord) -> bool {
    matches!(record.state, State::Filed | State::Failed { .. } | State::Discarded)
}

/// The finished jobs, most recently updated first, by (updated_unix, id) descending.
fn finished_newest_first(jobs: &[JobRecord]) -> Vec<&JobRecord> {
    let mut done: Vec<&JobRecord> = jobs.iter().filter(|record| finished(record)).collect();
    done.sort_by(|a, b| (b.updated_unix, &b.id).cmp(&(a.updated_unix, &a.id)));
    done
}

/// The snapshot's jobs: every unfinished job by (created_unix, id), then the 50 most recently finished by
/// (updated_unix, id) descending (design part 9.2). Def: T26. Use: T26.
pub(crate) fn snapshot_jobs(jobs: &[JobRecord]) -> Vec<JobSummary> {
    let mut open: Vec<&JobRecord> = jobs.iter().filter(|record| !finished(record)).collect();
    open.sort_by(|a, b| (a.created_unix, &a.id).cmp(&(b.created_unix, &b.id)));
    let done = finished_newest_first(jobs);
    open.into_iter().chain(done.into_iter().take(HISTORY_PAGE)).map(job_summary).collect()
}

/// The finished jobs after `before` in that descending order, at most HISTORY_PAGE; Err("no job <id>"). Def: T26.
/// Use: T26.
pub(crate) fn history_page(jobs: &[JobRecord], before: Option<&str>) -> Result<Vec<JobSummary>, String> {
    let after = match before {
        None => None,
        Some(id) => {
            let record = jobs.iter().find(|record| record.id == id).ok_or_else(|| no_job(id))?;
            Some((record.updated_unix, record.id.as_str()))
        }
    };
    let page = finished_newest_first(jobs)
        .into_iter()
        .filter(|record| after.is_none_or(|after| (record.updated_unix, record.id.as_str()) < after))
        .take(HISTORY_PAGE)
        .map(job_summary)
        .collect();
    Ok(page)
}

/// The snapshot's secrets (Ruling S77): every `SECRET_NAMES` entry, set or not, and each share name in `shares`; a
/// webhook URL that is set shows only its host, and no value is ever read out. Def: T26. Use: T26.
pub(crate) fn secret_states(secrets: &dyn SecretStore, shares: &BTreeSet<String>) -> BTreeMap<String, SecretState> {
    let names = SECRET_NAMES.iter().map(|name| name.to_string()).chain(shares.iter().cloned());
    names
        .map(|name| {
            let value = secrets.get(&name);
            let host = value.as_deref().filter(|_| WEBHOOK_SECRETS.contains(&name.as_str())).and_then(secret_host);
            (name, SecretState { set: value.is_some(), host })
        })
        .collect()
}

/// The serve thread (design part 4.4): each stream `transport` accepts goes to the owner as `Msg::Connect`. It ends
/// when the transport fails, the owner is gone, or a stream comes after `stop` was raised: `EngineHandle::stop` raises
/// it, then wakes the waiting accept with a connection of its own. Def: T26. Use: T26.
pub(crate) fn spawn_serve(
    transport: Arc<dyn IpcTransport>,
    stop: Arc<AtomicBool>,
    owner: Sender<Msg>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        while let Ok(stream) = transport.accept() {
            if stop.load(Ordering::SeqCst) || owner.send(Msg::Connect(stream)).is_err() {
                break;
            }
        }
    })
}

/// A client's reader thread (design part 9.1): each line of at most `MAX_LINE_BYTES` that `parse_request` reads goes
/// to the owner as `Msg::Request`; a line it refuses gets its error reply at once. A longer line gets the error reply
/// `LINE_TOO_LONG` with no id, and the client is dropped. An empty line is skipped. When the client goes away, its
/// connection is closed. Def: T26. Use: T26.
pub(crate) fn spawn_reader(client: u64, reader: Box<dyn BufRead + Send>, clients: Arc<Clients>, owner: Sender<Msg>) {
    thread::spawn(move || {
        let mut reader = reader;
        loop {
            match read_line_capped(&mut *reader, MAX_LINE_BYTES) {
                Ok(None) | Err(_) => break,
                Ok(Some(Err(()))) => {
                    clients.send(client, reply_error(None, LINE_TOO_LONG, &[]));
                    break;
                }
                Ok(Some(Ok(line))) if line.trim().is_empty() => {}
                Ok(Some(Ok(line))) => match parse_request(&line) {
                    Err(reply) => clients.send(client, reply),
                    Ok((id, request)) => {
                        if owner.send(Msg::Request { client, id, request }).is_err() {
                            break;
                        }
                    }
                },
            }
        }
        clients.close(client);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Write};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Condvar, Mutex};
    use std::time::{Duration, Instant};

    /// A writer that blocks on its first write until released.
    struct Stuck(Arc<(Mutex<bool>, Condvar)>);
    impl Write for Stuck {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let (lock, cv) = &*self.0;
            let mut open = lock.lock().unwrap();
            while !*open { open = cv.wait(open).unwrap(); }
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }

    /// A writer that counts the lines it takes, by their newline bytes (PF-9).
    struct Counting(Arc<AtomicUsize>);
    impl Write for Counting {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.fetch_add(buf.iter().filter(|&&byte| byte == b'\n').count(), Ordering::SeqCst);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }

    #[test]
    fn a_client_that_falls_behind_is_dropped_without_slowing_the_engine() {
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let taken = Arc::new(AtomicUsize::new(0));
        let clients = Clients::new();
        clients.add(Box::new(Stuck(gate.clone())));
        clients.add(Box::new(Counting(taken.clone())));
        let mut spent = Duration::ZERO;
        let mut broadcast = |n: usize| {
            let started = Instant::now();
            clients.broadcast(&format!("{{\"n\":{n}}}"));
            spent += started.elapsed();
        };
        for n in 0..CLIENT_QUEUE { broadcast(n); }
        let deadline = Instant::now() + Duration::from_secs(10);
        while taken.load(Ordering::SeqCst) < CLIENT_QUEUE {
            assert!(Instant::now() < deadline, "the counting client took {} lines", taken.load(Ordering::SeqCst));
            std::thread::sleep(Duration::from_millis(5));
        }
        for n in CLIENT_QUEUE..(CLIENT_QUEUE + 10) { broadcast(n); }
        assert!(spent < Duration::from_secs(1));
        assert_eq!(clients.count(), 1);
        *gate.0.lock().unwrap() = true;
        gate.1.notify_all();
    }

    #[test]
    fn progress_is_pushed_at_most_once_a_second_per_job() {
        let mut gate = ProgressGate::new();
        let t0 = Instant::now();
        assert!(gate.admit("a-1", t0));
        assert!(!gate.admit("a-1", t0 + Duration::from_millis(500)));
        assert!(gate.admit("b-1", t0 + Duration::from_millis(500)));
        assert!(gate.admit("a-1", t0 + Duration::from_millis(1000)));
    }

    #[test]
    fn a_line_longer_than_the_cap_is_reported_without_reading_past_it() {
        let mut long = vec![b'x'; 11];
        long.extend_from_slice(b"\nnext\n");
        let mut reader = io::Cursor::new(long);
        assert_eq!(read_line_capped(&mut reader, 10).unwrap(), Some(Err(())));
        let mut short = io::Cursor::new(b"{\"v\":1}\n".to_vec());
        assert_eq!(read_line_capped(&mut short, 10).unwrap(), Some(Ok("{\"v\":1}".to_string())));
        assert_eq!(read_line_capped(&mut short, 10).unwrap(), None);
    }

    #[test]
    fn a_webhook_secret_shows_only_its_host() {
        assert_eq!(secret_host("http://ha.local:8123/api/webhook/abc123").as_deref(), Some("ha.local"));
        assert_eq!(secret_host("https://user:pw@hooks.example.org/x").as_deref(), Some("hooks.example.org"));
        assert_eq!(secret_host("not a url"), None);
    }

    fn record(n: u64, state: lazymkv_core::job::State) -> lazymkv_core::record::JobRecord {
        let fp = format!("{n:032X}");
        lazymkv_core::record::JobRecord {
            format: lazymkv_core::record::JOB_FORMAT.to_string(), id: format!("{}-1", fp.to_ascii_lowercase()),
            fingerprint: fp, disc_id: None, drive: "tok0".to_string(), created_unix: n, updated_unix: n, state,
            message: None, rip_again: false, profile: "job.mmcp.xml".to_string(), plan: None, titles: vec![],
            filed_unix: None, collisions: vec![], notes: vec![],
        }
    }

    #[test]
    fn the_snapshot_holds_every_unfinished_job_and_the_50_most_recently_finished() {
        use lazymkv_core::job::State;
        let mut jobs: Vec<_> = (1..=60).map(|n| record(n, State::Filed)).collect();
        jobs.push(record(61, State::Ejected { after_copy: false }));
        jobs.push(record(0, State::Ripping));
        let summaries = snapshot_jobs(&jobs);
        assert_eq!(summaries.len(), 52);
        assert_eq!(summaries[0].created_unix, 0);
        assert_eq!(summaries[1].created_unix, 61);
        assert_eq!(summaries[2].updated_unix, 60);
        assert_eq!(summaries[51].updated_unix, 11);
    }

    #[test]
    fn history_pages_at_most_50_before_a_job() {
        use lazymkv_core::job::State;
        let jobs: Vec<_> = (1..=60).map(|n| record(n, State::Filed)).collect();
        let first = history_page(&jobs, None).unwrap();
        assert_eq!(first.len(), 50);
        assert_eq!((first[0].updated_unix, first[49].updated_unix), (60, 11));
        let next = history_page(&jobs, Some(&first[49].id)).unwrap();
        assert_eq!(next.iter().map(|s| s.updated_unix).collect::<Vec<_>>(), (1..=10).rev().collect::<Vec<_>>());
        assert_eq!(history_page(&jobs, Some("nope-1")), Err("no job nope-1".to_string()));
    }

    /// Review round 1 (V11, RR1-12; V1): a step that ended with no event feeds its state's own failure row; ShareLost
    /// only for a CheckSpace whose state takes no failure (Planned); with no tag, the state's row or nothing.
    #[test]
    fn a_failure_event_is_the_states_failure_row_and_share_lost_only_for_a_space_check_in_planned() {
        use lazymkv_core::plan::Mode;
        let unknown = Failure { class: FailureClass::Unknown, output: None };
        let decision_failed = Some(Event::DecisionFailed { failure: unknown });
        assert_eq!(failure_event("CheckSpace", &State::Decided { approved: None }), decision_failed);
        assert_eq!(failure_event("CheckSpace", &State::Decided { approved: Some(vec![]) }), decision_failed);
        assert_eq!(failure_event("CheckSpace", &State::Planned { mode: Mode::Direct }), Some(Event::ShareLost));
        assert_eq!(failure_event("Eject", &State::RipVerified { after_copy: false }), Some(Event::EjectFailed));
        assert_eq!(failure_event("Remux", &State::Remuxing), Some(Event::RemuxFailed { failure: unknown }));
        assert_eq!(failure_event("RunScan", &State::Fingerprinted), Some(Event::ScanFailed { failure: unknown }));
        assert_eq!(failure_event("CheckSpace", &State::Filed), None);
        assert_eq!(failure_event("", &State::Remuxing), Some(Event::RemuxFailed { failure: unknown }));
        assert_eq!(failure_event("", &State::Ripping), Some(Event::TitleRipFailed { failure: unknown }));
        assert_eq!(failure_event("", &State::Planned { mode: Mode::Direct }), None);
        assert_eq!(failure_event("", &State::Ejected { after_copy: false }), None);
        let failed = State::Failed { class: FailureClass::Unknown, from: Box::new(State::Inspecting) };
        assert_eq!(failure_event("", &failed), None);
    }
}
