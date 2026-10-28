//! In-memory fakes of the platform traits (design part 4.2), behind the `fakes` feature: a scripted drive watcher, a
//! recording ejector, a counting keep-awake, fake shares, in-memory secrets, an in-memory pipe and a fake host. Every
//! engine and client test builds its `Platform` from these (Tasks 22 to 29).

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io::{self, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use lazymkv_core::job::Tool;

use crate::localtime::format_local_time;
use crate::{ConnectError, DriveEvent, DriveWatcher, Ejector, Host, IpcConnector, IpcStream, IpcTransport, KeepAwake,
    Reach, SecretStore, ShareConnector};

/// Locks `mutex`, ignoring a poison left by a panicked test thread, so one failed test does not cascade into others.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The queue and the present set a `ScriptedDrives` and its `DriveScript` share.
#[derive(Default)]
struct DriveState {
    events: VecDeque<DriveEvent>,
    present: BTreeMap<String, PathBuf>,
}

type SharedDrives = Arc<(Mutex<DriveState>, Condvar)>;

/// A drive watcher that plays back what a test scripts (spec 4.3, design part 4.2, Rulings T8-b, T8-f). It is the
/// engine's end; the test keeps the `DriveScript`. Its `present` lists the drives sorted by drive id (Ruling T8-b).
/// Def: T8. Use: T22 to T29.
pub struct ScriptedDrives {
    shared: SharedDrives,
}

/// The test's end of a `ScriptedDrives`: what it writes, the watcher hands out in order (design part 4.2, Rulings
/// T8-b, T8-f). Clones share one queue. Def: T8. Use: T22 to T29.
#[derive(Clone)]
pub struct DriveScript {
    shared: SharedDrives,
}

impl ScriptedDrives {
    /// A watcher with nothing present, and the script that feeds it (design part 4.2). Def: T8. Use: T22 to T29.
    pub fn new() -> (ScriptedDrives, DriveScript) {
        let shared: SharedDrives = Arc::default();
        (ScriptedDrives { shared: Arc::clone(&shared) }, DriveScript { shared })
    }
}

impl DriveWatcher for ScriptedDrives {
    fn next(&mut self, timeout: Duration) -> Option<DriveEvent> {
        let (state, wake) = &*self.shared;
        let (mut state, _) = wake
            .wait_timeout_while(lock(state), timeout, |state| state.events.is_empty())
            .unwrap_or_else(PoisonError::into_inner);
        state.events.pop_front()
    }

    fn present(&self) -> Vec<(String, PathBuf)> {
        present_of(&self.shared)
    }
}

fn present_of(shared: &SharedDrives) -> Vec<(String, PathBuf)> {
    lock(&shared.0).present.iter().map(|(drive, root)| (drive.clone(), root.clone())).collect()
}

impl DriveScript {
    /// Puts a disc in `drive`: `present` shows it at once and an `Arrived` event is queued. A drive that already holds
    /// a disc gets the new root (Ruling T8-f). Def: T8. Use: T22 to T29.
    pub fn arrive(&self, drive: &str, root: &Path) {
        let (state, wake) = &*self.shared;
        let mut state = lock(state);
        state.present.insert(drive.to_string(), root.to_path_buf());
        state.events.push_back(DriveEvent::Arrived { drive: drive.to_string(), root: root.to_path_buf() });
        wake.notify_all();
    }

    /// Takes the disc out of `drive`: `present` drops it at once and a `Removed` event is queued, even for a drive that
    /// holds no disc (Ruling T8-f). Def: T8. Use: T22 to T29.
    pub fn remove(&self, drive: &str) {
        let (state, wake) = &*self.shared;
        let mut state = lock(state);
        state.present.remove(drive);
        state.events.push_back(DriveEvent::Removed { drive: drive.to_string() });
        wake.notify_all();
    }

    /// The drives holding a disc now, as (drive, root), sorted by drive id (Ruling T8-b). Def: T8. Use: T22 to T29.
    pub fn present(&self) -> Vec<(String, PathBuf)> {
        present_of(&self.shared)
    }
}

/// What a `RecordingEjector` has seen and what it will do next.
#[derive(Default)]
struct EjectorState {
    calls: Vec<String>,
    fail: Option<String>,
}

/// An ejector that records every call and fails when told to (spec 4.3, design part 4.2, Ruling T8-c).
/// Def: T8. Use: T22 to T29.
pub struct RecordingEjector {
    state: Mutex<EjectorState>,
    removals: Option<DriveScript>,
}

impl RecordingEjector {
    /// An ejector that succeeds and pushes no removal (design part 4.2). Def: T8. Use: T22 to T29.
    pub fn new() -> RecordingEjector {
        RecordingEjector { state: Mutex::default(), removals: None }
    }

    /// Every drive passed to `eject` so far, in order, failed calls included (design part 4.2).
    /// Def: T8. Use: T22 to T29.
    pub fn calls(&self) -> Vec<String> {
        lock(&self.state).calls.clone()
    }

    /// Makes the next `eject` fail with `message`. Later ones succeed again (design part 4.2).
    /// Def: T8. Use: T22 to T29.
    pub fn fail_next(&self, message: &str) {
        lock(&self.state).fail = Some(message.to_string());
    }

    /// Makes each successful eject push `Removed` for its drive onto `script` (Ruling T8-c). A failed eject leaves the
    /// disc in. Def: T8. Use: T22 to T29.
    pub fn with_removals(mut self, script: DriveScript) -> RecordingEjector {
        self.removals = Some(script);
        self
    }
}

impl Default for RecordingEjector {
    fn default() -> RecordingEjector {
        RecordingEjector::new()
    }
}

impl Ejector for RecordingEjector {
    fn eject(&self, drive: &str) -> Result<(), String> {
        {
            let mut state = lock(&self.state);
            state.calls.push(drive.to_string());
            if let Some(message) = state.fail.take() {
                return Err(message);
            }
        }
        if let Some(script) = &self.removals {
            script.remove(drive);
        }
        Ok(())
    }
}

/// A keep-awake that counts its holders (spec 4.3, design part 4.2, Ruling T8-g). Def: T8. Use: T22 to T29.
pub struct CountingKeepAwake {
    holders: Mutex<usize>,
}

impl CountingKeepAwake {
    /// A keep-awake with no holder (design part 4.2). Def: T8. Use: T22 to T29.
    pub fn new() -> CountingKeepAwake {
        CountingKeepAwake { holders: Mutex::new(0) }
    }

    /// How many `acquire` calls have no `release` yet (design part 4.2). Def: T8. Use: T22 to T29.
    pub fn holders(&self) -> usize {
        *lock(&self.holders)
    }
}

impl Default for CountingKeepAwake {
    fn default() -> CountingKeepAwake {
        CountingKeepAwake::new()
    }
}

impl KeepAwake for CountingKeepAwake {
    fn acquire(&self) {
        *lock(&self.holders) += 1;
    }

    fn release(&self) {
        // Stops at zero (Ruling T8-g).
        let mut holders = lock(&self.holders);
        *holders = holders.saturating_sub(1);
    }
}

/// What a `FakeShares` has been told, keyed by the exact root string (Ruling T8-a).
#[derive(Default)]
struct SharesState {
    free: BTreeMap<String, u64>,
    reach: BTreeMap<String, Reach>,
    volumes: BTreeMap<String, String>,
}

/// Fake shares (spec 4.3, design part 4.2, Ruling T8-a): every root is reachable, has `u64::MAX` free bytes and is its
/// own volume until a test says otherwise. Roots are matched by their exact string, so no path rule runs.
/// Def: T8. Use: T22 to T29.
pub struct FakeShares {
    state: Mutex<SharesState>,
}

impl FakeShares {
    /// Shares where everything is reachable, empty of data and on separate volumes (Ruling T8-a).
    /// Def: T8. Use: T22 to T29.
    pub fn new() -> FakeShares {
        FakeShares { state: Mutex::default() }
    }

    /// Sets the free bytes `free_bytes(root)` reports (Ruling T8-a). Def: T8. Use: T22 to T29.
    pub fn set_free(&self, root: &str, bytes: u64) {
        lock(&self.state).free.insert(root.to_string(), bytes);
    }

    /// Sets what `reach(root)` reports. While it is `Unreachable`, `free_bytes(root)` fails with the same text (Ruling
    /// T8-a).
    /// Def: T8. Use: T22 to T29.
    pub fn set_reach(&self, root: &str, reach: Reach) {
        lock(&self.state).reach.insert(root.to_string(), reach);
    }

    /// Gives `b` the volume `a` has at the time of the call, so `volume(b)` then returns what `volume(a)` returned
    /// (S11, Ruling T8-a). It is not a live link: a later call that moves `a` leaves `b` where it was.
    /// Def: T8. Use: T22 to T29.
    pub fn same_volume(&self, a: &str, b: &str) {
        let mut state = lock(&self.state);
        let volume = state.volumes.get(a).cloned().unwrap_or_else(|| a.to_string());
        state.volumes.insert(b.to_string(), volume);
    }
}

impl Default for FakeShares {
    fn default() -> FakeShares {
        FakeShares::new()
    }
}

impl ShareConnector for FakeShares {
    fn reach(&self, root: &str) -> Reach {
        lock(&self.state).reach.get(root).cloned().unwrap_or(Reach::Reachable)
    }

    fn free_bytes(&self, root: &str) -> io::Result<u64> {
        let state = lock(&self.state);
        if let Some(Reach::Unreachable(message)) = state.reach.get(root) {
            return Err(io::Error::other(message.clone()));
        }
        Ok(state.free.get(root).copied().unwrap_or(u64::MAX))
    }

    fn volume(&self, root: &str) -> Option<String> {
        Some(lock(&self.state).volumes.get(root).cloned().unwrap_or_else(|| root.to_string()))
    }
}

/// A secret store that keeps its secrets in memory (spec 4.3, design part 4.2). Def: T8. Use: T22 to T29.
pub struct MemorySecrets {
    secrets: Mutex<BTreeMap<String, String>>,
}

impl MemorySecrets {
    /// An empty store (design part 4.2). Def: T8. Use: T22 to T29.
    pub fn new() -> MemorySecrets {
        MemorySecrets { secrets: Mutex::default() }
    }

    /// The store with `name` already set to `value` (design part 4.2). Def: T8. Use: T22 to T29.
    pub fn with(self, name: &str, value: &str) -> MemorySecrets {
        lock(&self.secrets).insert(name.to_string(), value.to_string());
        self
    }
}

impl Default for MemorySecrets {
    fn default() -> MemorySecrets {
        MemorySecrets::new()
    }
}

impl SecretStore for MemorySecrets {
    fn get(&self, name: &str) -> Option<String> {
        lock(&self.secrets).get(name).cloned()
    }

    fn set(&self, name: &str, value: &str) -> Result<(), String> {
        lock(&self.secrets).insert(name.to_string(), value.to_string());
        Ok(())
    }
}

/// A reader over a channel of byte chunks: it ends when every sender is gone, as a pipe does (Ruling T8-e).
struct ChannelReader {
    chunks: Receiver<Vec<u8>>,
    current: Vec<u8>,
    taken: usize,
}

impl Read for ChannelReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        while self.taken == self.current.len() {
            match self.chunks.recv() {
                Ok(chunk) => {
                    self.current = chunk;
                    self.taken = 0;
                }
                Err(_) => return Ok(0),
            }
        }
        let count = buf.len().min(self.current.len() - self.taken);
        buf[..count].copy_from_slice(&self.current[self.taken..self.taken + count]);
        self.taken += count;
        Ok(count)
    }
}

/// A writer that sends each write as one chunk. It fails with BrokenPipe once the reader is gone (Ruling T8-e).
struct ChannelWriter {
    chunks: Sender<Vec<u8>>,
}

impl Write for ChannelWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.chunks
            .send(buf.to_vec())
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "the other end of the stream is closed"))?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// One end of an in-memory duplex stream over a pair of byte channels.
fn stream_end(incoming: Receiver<Vec<u8>>, outgoing: Sender<Vec<u8>>) -> IpcStream {
    IpcStream {
        reader: Box::new(BufReader::new(ChannelReader { chunks: incoming, current: Vec::new(), taken: 0 })),
        writer: Box::new(ChannelWriter { chunks: outgoing }),
    }
}

/// The engine's end of an in-memory pipe (spec 4.3, design part 4.2, Ruling T8-e). `accept` hands out the server end of
/// each connection a `MemoryConnector` makes. Def: T8. Use: T22 to T29.
pub struct MemoryIpc {
    incoming: Mutex<Receiver<IpcStream>>,
}

/// The client's end of an in-memory pipe, or a connector that always fails (spec 4.3, design part 4.2, Ruling T8-e).
/// Clones connect to the same `MemoryIpc`. Def: T8. Use: T22 to T29.
#[derive(Clone)]
pub struct MemoryConnector {
    /// The way to the `MemoryIpc`; None for a connector built by `failing`.
    to_server: Option<Sender<IpcStream>>,
    /// What `connect` returns when there is no way through.
    error: ConnectError,
}

impl MemoryIpc {
    /// A pipe nobody has connected to, and the connector that connects to it (Ruling T8-e). Def: T8. Use: T22 to T29.
    pub fn new() -> (MemoryIpc, MemoryConnector) {
        let (to_server, incoming) = channel();
        let connector = MemoryConnector { to_server: Some(to_server), error: ConnectError::NotRunning };
        (MemoryIpc { incoming: Mutex::new(incoming) }, connector)
    }
}

impl IpcTransport for MemoryIpc {
    /// Waits for a client. Fails with BrokenPipe once every connector is dropped (Ruling T8-e).
    fn accept(&self) -> io::Result<IpcStream> {
        lock(&self.incoming)
            .recv()
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "every connector to the pipe is gone"))
    }
}

impl MemoryConnector {
    /// A connector whose `connect` always returns `error` (design part 4.2). Def: T8. Use: T22 to T29.
    pub fn failing(error: ConnectError) -> MemoryConnector {
        MemoryConnector { to_server: None, error }
    }
}

impl IpcConnector for MemoryConnector {
    /// Connects to the `MemoryIpc`, or returns the error of `failing`. A `MemoryIpc` that is gone is `NotRunning`.
    fn connect(&self) -> Result<IpcStream, ConnectError> {
        let to_server = self.to_server.as_ref().ok_or(self.error)?;
        let (client_writes, server_reads) = channel();
        let (server_writes, client_reads) = channel();
        to_server.send(stream_end(server_reads, server_writes)).map_err(|_| ConnectError::NotRunning)?;
        Ok(stream_end(client_reads, client_writes))
    }
}

/// What a `FakeHost` has been told besides its fixed date and time.
#[derive(Default)]
struct HostState {
    labels: BTreeMap<PathBuf, String>,
    running: Vec<String>,
    app_key: Option<String>,
    debug_log: Option<PathBuf>,
    writable: BTreeSet<PathBuf>,
}

/// A host with a fixed date and time, no registration key, no debug log, no running programs and no volume label until
/// a test sets one (spec 4.3, design part 4.2). It knows no default install path. Its clock in milliseconds is
/// `now_unix` times 1000, its local time is that clock read as UTC, and no path is user-writable until a test sets one.
/// Def: T8. Use: T11, T13, T14, T22 to T29.
pub struct FakeHost {
    local_date: String,
    now_unix: u64,
    state: Mutex<HostState>,
}

impl FakeHost {
    /// A host whose clock reads `now_unix` and whose calendar date is `local_date` (`yyyy-MM-dd`) (design part 4.2).
    /// Def: T8. Use: T11, T13, T14, T22 to T29.
    pub fn new(local_date: &str, now_unix: u64) -> FakeHost {
        FakeHost { local_date: local_date.to_string(), now_unix, state: Mutex::default() }
    }

    /// Gives the disc at `root` the volume label `label` (design part 4.2). Def: T8. Use: T11, T13, T14, T22 to T29.
    pub fn set_label(&self, root: &Path, label: &str) {
        lock(&self.state).labels.insert(root.to_path_buf(), label.to_string());
    }

    /// Sets the image names `running_processes` reports (design part 4.2). Def: T8. Use: T11, T13, T14, T22 to T29.
    pub fn set_running(&self, names: Vec<String>) {
        lock(&self.state).running = names;
    }

    /// Sets the registration key `makemkv_app_key` reports (design part 4.2). Def: T8. Use: T11, T13, T14, T22 to T29.
    pub fn set_app_key(&self, key: &str) {
        lock(&self.state).app_key = Some(key.to_string());
    }

    /// Sets the path `makemkv_debug_log` reports (design part 4.2). Def: T8. Use: T11, T13, T14, T22 to T29.
    pub fn set_debug_log(&self, path: &Path) {
        lock(&self.state).debug_log = Some(path.to_path_buf());
    }

    /// Makes `user_writable` answer true for exactly `path` (design part 3.6). Def: T3-7. Use: T3-22.
    pub fn set_writable(&self, path: &Path) {
        lock(&self.state).writable.insert(path.to_path_buf());
    }
}

impl Host for FakeHost {
    fn now_unix(&self) -> u64 {
        self.now_unix
    }

    fn now_ms(&self) -> u64 {
        self.now_unix * 1000
    }

    fn local_date(&self) -> String {
        self.local_date.clone()
    }

    fn local_time(&self, unix_ms: u64) -> String {
        format_local_time(unix_ms, 0)
    }

    fn makemkv_app_key(&self) -> Option<String> {
        lock(&self.state).app_key.clone()
    }

    fn makemkv_debug_log(&self) -> Option<PathBuf> {
        lock(&self.state).debug_log.clone()
    }

    fn running_processes(&self) -> Vec<String> {
        lock(&self.state).running.clone()
    }

    fn volume_label(&self, root: &Path) -> Option<String> {
        lock(&self.state).labels.get(root).cloned()
    }

    fn default_install_path(&self, _tool: Tool) -> Option<PathBuf> {
        None
    }

    fn user_writable(&self, path: &Path) -> bool {
        lock(&self.state).writable.contains(path)
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self, BufRead, Write};
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::time::Duration;

    use lazymkv_core::job::Tool;

    use super::*;
    use crate::{ConnectError, DriveEvent, DriveWatcher, Ejector, GuardedChild, Host, IpcConnector, IpcTransport, KeepAwake,
        Platform, ProcessGuard, Reach, SecretStore, ShareConnector, Spawn};

    const SHORT: Duration = Duration::from_millis(10);

    fn arrived(drive: &str, root: &str) -> Option<DriveEvent> {
        Some(DriveEvent::Arrived { drive: drive.to_string(), root: PathBuf::from(root) })
    }

    #[test]
    fn scripted_drives_replay_arrivals_and_removals_in_order() {
        let (mut drives, script) = ScriptedDrives::new();
        script.arrive("tok0", Path::new("/discs/a"));
        assert_eq!(script.present(), vec![("tok0".to_string(), PathBuf::from("/discs/a"))]);
        assert_eq!(drives.present(), script.present());
        script.remove("tok0");
        assert!(drives.present().is_empty());
        assert_eq!(drives.next(SHORT), arrived("tok0", "/discs/a"));
        assert_eq!(drives.next(SHORT), Some(DriveEvent::Removed { drive: "tok0".to_string() }));
        assert_eq!(drives.next(SHORT), None);
    }

    #[test]
    fn a_waiting_watcher_wakes_on_an_arrival() {
        let (mut drives, script) = ScriptedDrives::new();
        let later = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            script.arrive("tok1", Path::new("/discs/b"));
        });
        assert_eq!(drives.next(Duration::from_secs(5)), arrived("tok1", "/discs/b"));
        later.join().unwrap();
    }

    #[test]
    fn the_ejector_records_each_call_and_fails_on_request() {
        let ejector = RecordingEjector::new();
        assert_eq!(ejector.eject("tok0"), Ok(()));
        ejector.fail_next("stuck");
        assert_eq!(ejector.eject("tok0"), Err("stuck".to_string()));
        assert_eq!(ejector.eject("tok1"), Ok(()));
        assert_eq!(ejector.calls(), ["tok0", "tok0", "tok1"]);
    }

    #[test]
    fn an_ejector_with_removals_pushes_the_removal_it_causes() {
        let (mut drives, script) = ScriptedDrives::new();
        script.arrive("tok0", Path::new("/discs/a"));
        assert_eq!(drives.next(SHORT), arrived("tok0", "/discs/a"));
        let ejector = RecordingEjector::new().with_removals(script.clone());
        ejector.fail_next("stuck");
        assert_eq!(ejector.eject("tok0"), Err("stuck".to_string()));
        assert_eq!(drives.next(SHORT), None);
        assert_eq!(ejector.eject("tok0"), Ok(()));
        assert_eq!(drives.next(SHORT), Some(DriveEvent::Removed { drive: "tok0".to_string() }));
        assert!(script.present().is_empty());
    }

    #[test]
    fn keep_awake_counts_its_holders() {
        let awake = CountingKeepAwake::new();
        awake.acquire();
        awake.acquire();
        awake.release();
        assert_eq!(awake.holders(), 1);
        awake.release();
        awake.release();
        assert_eq!(awake.holders(), 0);
    }

    #[test]
    fn fake_shares_report_free_space_and_switchable_reach() {
        let shares = FakeShares::new();
        assert_eq!(shares.reach("/staging"), Reach::Reachable);
        assert_eq!(shares.free_bytes("/staging").unwrap(), u64::MAX);
        shares.set_free("/staging", 1_000);
        assert_eq!(shares.free_bytes("/staging").unwrap(), 1_000);
        assert_eq!(shares.free_bytes("/library").unwrap(), u64::MAX);
        shares.set_reach("/staging", Reach::Unreachable("cable out".to_string()));
        assert_eq!(shares.reach("/staging"), Reach::Unreachable("cable out".to_string()));
        assert_eq!(shares.free_bytes("/staging").unwrap_err().to_string(), "cable out");
        shares.set_reach("/staging", Reach::Reachable);
        assert_eq!(shares.free_bytes("/staging").unwrap(), 1_000);
    }

    #[test]
    fn roots_are_on_separate_volumes_until_joined() {
        let shares = FakeShares::new();
        assert_eq!(shares.volume("/staging"), Some("/staging".to_string()));
        assert_ne!(shares.volume("/staging"), shares.volume("/library"));
        shares.same_volume("/staging", "/library");
        assert_eq!(shares.volume("/library"), shares.volume("/staging"));
        assert_ne!(shares.volume("/tuning"), shares.volume("/staging"));
    }

    #[test]
    fn memory_secrets_round_trip() {
        let secrets = MemorySecrets::new().with("tmdb_token", "t0k");
        assert_eq!(secrets.get("tmdb_token"), Some("t0k".to_string()));
        assert_eq!(secrets.get("webhook_url"), None);
        secrets.set("webhook_url", "http://ha.local/api/webhook/x").unwrap();
        assert_eq!(secrets.get("webhook_url").as_deref(), Some("http://ha.local/api/webhook/x"));
    }

    #[test]
    fn memory_ipc_is_a_duplex_line_stream() {
        let (ipc, connector) = MemoryIpc::new();
        let mut client = connector.connect().unwrap();
        let mut server = ipc.accept().unwrap();
        client.writer.write_all(b"{\"v\":1}\n").unwrap();
        client.writer.flush().unwrap();
        let mut line = String::new();
        server.reader.read_line(&mut line).unwrap();
        assert_eq!(line, "{\"v\":1}\n");
        server.writer.write_all(b"ok\n").unwrap();
        line.clear();
        client.reader.read_line(&mut line).unwrap();
        assert_eq!(line, "ok\n");
        drop(client);
        line.clear();
        assert_eq!(server.reader.read_line(&mut line).unwrap(), 0);
        assert_eq!(server.writer.write_all(b"late\n").unwrap_err().kind(), io::ErrorKind::BrokenPipe);
        drop(connector);
        assert_eq!(ipc.accept().err().map(|e| e.kind()), Some(io::ErrorKind::BrokenPipe));
    }

    #[test]
    fn a_failing_connector_returns_its_error() {
        for error in [ConnectError::NotRunning, ConnectError::Refused, ConnectError::NotElevated, ConnectError::NoTransport] {
            assert_eq!(MemoryConnector::failing(error).connect().err(), Some(error));
        }
    }

    #[test]
    fn fake_host_reports_no_gui_no_key_and_the_fake_label() {
        let host = FakeHost::new("2026-10-23", 1_800_000_000);
        assert_eq!(host.now_unix(), 1_800_000_000);
        assert!(host.running_processes().is_empty());
        assert_eq!(host.makemkv_app_key(), None);
        assert_eq!(host.makemkv_debug_log(), None);
        assert_eq!(host.default_install_path(Tool::Makemkvcon), None);
        assert_eq!(host.volume_label(Path::new("/discs/a")), None);
        host.set_label(Path::new("/discs/a"), "SYNTHETIC_MOVIE");
        assert_eq!(host.volume_label(Path::new("/discs/a")).as_deref(), Some("SYNTHETIC_MOVIE"));
        host.set_running(vec!["makemkvcon64.exe".to_string()]);
        assert_eq!(host.running_processes(), ["makemkvcon64.exe"]);
        host.set_app_key("T-KEY");
        assert_eq!(host.makemkv_app_key().as_deref(), Some("T-KEY"));
        host.set_debug_log(Path::new("/home/MakeMKV_log.txt"));
        assert_eq!(host.makemkv_debug_log(), Some(PathBuf::from("/home/MakeMKV_log.txt")));
    }

    #[test]
    fn the_fake_host_gives_a_fixed_local_date() {
        let host = FakeHost::new("2026-10-23", 1_800_000_000);
        assert_eq!(host.local_date(), "2026-10-23");
        assert_eq!(host.local_date(), "2026-10-23");
    }

    #[test]
    fn the_fake_hosts_local_time_is_utc_of_its_clock() {
        let host = FakeHost::new("2026-10-23", 1_792_800_000);
        assert_eq!(host.now_ms(), 1_792_800_000_000);
        assert_eq!(host.local_time(host.now_ms()), "2026-10-24 00:00:00.000");
        assert_eq!(host.local_time(1_792_800_000_123), "2026-10-24 00:00:00.123");
        // The date is a setting of the fake, so it differs from its clock's UTC date on purpose.
        assert_eq!(host.local_date(), "2026-10-23");
    }

    #[test]
    fn a_path_is_writable_only_when_set() {
        let host = FakeHost::new("2026-10-23", 1_800_000_000);
        assert!(!host.user_writable(Path::new("/tools")));
        host.set_writable(Path::new("/tools"));
        assert!(host.user_writable(Path::new("/tools")));
        assert!(!host.user_writable(Path::new("/tools/mkvmerge")));
        assert!(!host.user_writable(Path::new("/other")));
    }

    struct NoProcesses;

    impl ProcessGuard for NoProcesses {
        fn spawn(&self, _: &Spawn) -> io::Result<Box<dyn GuardedChild>> {
            Err(io::Error::new(io::ErrorKind::Unsupported, "no processes in this test"))
        }
    }

    #[test]
    fn the_fakes_fill_a_platform_bundle() {
        let (drives, _script) = ScriptedDrives::new();
        let (ipc, _connector) = MemoryIpc::new();
        let platform = Platform { drives: Box::new(drives), ejector: Arc::new(RecordingEjector::new()),
            processes: Arc::new(NoProcesses), awake: Arc::new(CountingKeepAwake::new()), shares: Arc::new(FakeShares::new()),
            secrets: Arc::new(MemorySecrets::new()), ipc: Arc::new(ipc),
            host: Arc::new(FakeHost::new("2026-10-23", 1_800_000_000)) };
        assert_eq!(platform.host.local_date(), "2026-10-23");
        assert!(platform.drives.present().is_empty());
    }
}
