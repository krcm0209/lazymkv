//! The job store: the data folder's persisted files and engine.log (design parts 3.5, 7.3, 7.4).

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
#[cfg(windows)]
use std::time::Duration;

use lazymkv_core::record::{
    check_format, DrivesFile, JobRecord, Transition, DRIVES_FORMAT, JOB_FORMAT, REMEMBERED_FORMAT,
};
use lazymkv_core::remembered::Remembered;
use lazymkv_core::settings::{Settings, SettingsProblem};
use lazymkv_platform::write_atomic;
use serde::de::DeserializeOwned;
use serde::Serialize;

/// The jobs folder under the data folder.
const JOBS_DIR: &str = "jobs";
/// A job's state, replaced on every transition (design part 7.3).
const STATE_FILE: &str = "state.json";
/// A job's transition log, one line appended per transition (design part 7.3).
const TRANSITIONS_FILE: &str = "transitions.jsonl";
/// The drives' records (design part 7.4).
const DRIVES_FILE: &str = "drives.json";
/// The engine's settings (design parts 3.5, 7.4).
const SETTINGS_FILE: &str = "settings.json";
/// The remembered decisions, one file per fingerprint (design part 7.4).
const REMEMBERED_DIR: &str = "remembered";
/// The engine's own log (design part 3.5).
const ENGINE_LOG: &str = "engine.log";
/// A job's own log (design part 7.1).
const JOB_LOG: &str = "job.log";
/// How many times `persist` tries to replace a state.json another process holds open, and the pause between two tries
/// (RR1-2).
#[cfg(windows)]
const REPLACE_TRIES: u32 = 5;
#[cfg(windows)]
const REPLACE_PAUSE: Duration = Duration::from_millis(100);

/// What `Store::persist` wrote (design part 7.3; review round 1, V1, RR1-3). Def: PR #6 review (V1). Use: owner.rs.
#[derive(Debug)]
pub(crate) enum Persisted {
    /// state.json was replaced and the transitions line appended and flushed.
    Whole,
    /// state.json was replaced, but the transitions line could not be appended or flushed. The transition counts as
    /// persisted: design part 7.3's crash "loses at most one log line, never a state" (RR1-3).
    LogLineLost(io::Error),
}

/// A fault a test makes `Store::persist` meet (review round 1, V1). Def: PR #6 review (V1). Use: testkit.
#[cfg(any(test, feature = "testkit"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PersistFault {
    /// `persist` writes nothing and fails with `PermissionDenied`, "refused by the test".
    Refuse,
    /// `persist` replaces state.json and appends no transitions line, as a failed append would: `LogLineLost` with
    /// "lost by the test".
    LoseLogLine,
}

/// Which persists meet a fault, asked with the record and transition before each `persist` (review round 1, V1).
/// Def: PR #6 review (V1). Use: testkit, owner.rs.
#[cfg(any(test, feature = "testkit"))]
pub(crate) type PersistFaults = Arc<dyn Fn(&JobRecord, &Transition) -> Option<PersistFault> + Send + Sync>;

/// The data folder's persisted files (design parts 3.5, 7.3, 7.4): `<data>/jobs/<id>/state.json` and
/// `transitions.jsonl`, `<data>/drives.json`, `<data>/remembered/<fingerprint>.json`, `<data>/settings.json` and
/// `<data>/engine.log`. It is the one writer of each path, as `write_atomic` assumes: its writes take one lock, so two
/// threads never write one file at once. It holds no record in memory. Def: T21. Use: T22 to T26.
pub(crate) struct Store {
    /// The data folder.
    data: PathBuf,
    /// The Unix seconds of engine.log lines.
    now: Arc<dyn Fn() -> u64 + Send + Sync>,
    /// Standard error in a console run. Its lock also keeps engine.log's lines whole and in order.
    console: Mutex<Option<Box<dyn Write + Send>>>,
    /// Held across every write of a persisted file (design part 7.4), so the store is the one writer per path.
    writing: Mutex<()>,
    /// The faults a test makes `persist` meet (review round 1, V1); None unless the testkit set them.
    #[cfg(any(test, feature = "testkit"))]
    faults: Option<PersistFaults>,
}

impl Store {
    /// The store over `data` (design part 3.5). `now` gives the Unix seconds of engine.log lines. `console` is
    /// standard error in a console run. Touches no file. Def: T21. Use: T22, T23, T25, T26.
    pub(crate) fn new(
        data: PathBuf,
        now: Arc<dyn Fn() -> u64 + Send + Sync>,
        console: Option<Box<dyn Write + Send>>,
    ) -> Store {
        Store {
            data,
            now,
            console: Mutex::new(console),
            writing: Mutex::new(()),
            #[cfg(any(test, feature = "testkit"))]
            faults: None,
        }
    }

    /// The store with `faults`, which `persist` asks before each write (review round 1, V1). Def: PR #6 review (V1).
    /// Use: owner.rs (`start_until`), for the testkit.
    #[cfg(any(test, feature = "testkit"))]
    pub(crate) fn with_faults(mut self, faults: PersistFaults) -> Store {
        self.faults = Some(faults);
        self
    }

    /// `<data>/jobs/<id>` (design part 7.3). Def: T21. Use: T22, T23, T25, T26.
    pub(crate) fn job_dir(&self, id: &str) -> PathBuf {
        self.data.join(JOBS_DIR).join(id)
    }

    /// Makes `<data>/jobs/<id>/` and writes the settings the job starts with and keeps there as `settings.json`
    /// (design part 7.1, ENG-19): `write_atomic` of `to_vec_pretty` plus "\n". Returns the folder. Def: T22. Use: T22.
    pub(crate) fn create_job(&self, id: &str, settings: &Settings) -> io::Result<PathBuf> {
        let _writing = self.writing.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let dir = self.job_dir(id);
        fs::create_dir_all(&dir)?;
        write_atomic(&dir.join(SETTINGS_FILE), &pretty(settings)?)?;
        Ok(dir)
    }

    /// Appends "<line>\n" to `<data>/jobs/<id>/job.log` (design part 7.1: the engine's own lines about the job).
    /// Errors are ignored, as engine.log's are. Def: T22. Use: T22 to T26.
    pub(crate) fn job_log(&self, id: &str, line: &str) {
        let _writing = self.writing.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let dir = self.job_dir(id);
        let _ = fs::create_dir_all(&dir);
        if let Ok(mut file) = OpenOptions::new().append(true).create(true).open(dir.join(JOB_LOG)) {
            let _ = file.write_all(format!("{line}\n").as_bytes());
        }
    }

    /// Every `<data>/jobs/*/state.json` in folder-name order (design parts 7.3, 7.4). Each is read as a Value, passed
    /// through `check_format(JOB_FORMAT)` and then into JobRecord. An Err is "<state.json path>: <reason>". A folder
    /// with no state.json is skipped, and an absent jobs folder gives none. Each record holds `titles` by key and
    /// `notes` by note, the order state.json's writer uses (RP23), so a loaded record equals the one its own state.json
    /// reads back as after the next `persist`. Def: T21. Use: T22, T23, T25, T26.
    pub(crate) fn load_jobs(&self) -> Vec<Result<JobRecord, String>> {
        let jobs = self.data.join(JOBS_DIR);
        let entries = match fs::read_dir(&jobs) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Vec::new(),
            Err(error) => return vec![Err(at(&jobs, error))],
        };
        let mut folders: Vec<PathBuf> =
            entries.filter_map(|entry| entry.ok()).map(|entry| entry.path()).filter(|path| path.is_dir()).collect();
        folders.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
        folders
            .into_iter()
            .filter_map(|folder| {
                let path = folder.join(STATE_FILE);
                match read_checked::<JobRecord>(&path, JOB_FORMAT) {
                    Ok(None) => None,
                    Ok(Some((record, _))) => Some(in_written_order(&record).map_err(|error| at(&path, error))),
                    Err(error) => Some(Err(error)),
                }
            })
            .collect()
    }

    /// Persists one transition in design part 7.3's order: `state.json` is replaced with `write_atomic`
    /// (`to_vec_pretty` plus "\n"), then `to_string(transition)` plus "\n" is appended to transitions.jsonl and
    /// `sync_data` flushes it, all before the transition's actions run. It refuses (`ErrorKind::InvalidData`, the
    /// path and the check_format text) when an existing state.json fails `check_format(JOB_FORMAT)` or is not JSON,
    /// and then writes nothing (design part 7.4). RP-8: when transitions.jsonl does not end in "\n", a crash tore its
    /// last line, and the append writes that "\n" first. Amended at review round 1 (V1): on Windows the replace of
    /// state.json is tried again while another process holds the file (`replace`, RR1-2), and an append or flush that
    /// fails after the replace gives `Persisted::LogLineLost`, not an error, because state.json already holds the new
    /// state (RR1-3); the append is never tried again, so a line is never written twice. Under the testkit it first
    /// asks the faults `with_faults` set. Def: T21. Use: T22, T23, T25, T26.
    pub(crate) fn persist(&self, record: &JobRecord, transition: &Transition) -> io::Result<Persisted> {
        #[cfg(any(test, feature = "testkit"))]
        let lose_line = match self.faults.as_ref().and_then(|faults| faults(record, transition)) {
            Some(PersistFault::Refuse) => {
                return Err(io::Error::new(io::ErrorKind::PermissionDenied, "refused by the test"));
            }
            Some(PersistFault::LoseLogLine) => true,
            None => false,
        };
        #[cfg(not(any(test, feature = "testkit")))]
        let lose_line = false;
        let _writing = self.writing.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let dir = self.job_dir(&record.id);
        let state = dir.join(STATE_FILE);
        refuse_foreign(&state, JOB_FORMAT)?;
        fs::create_dir_all(&dir)?;
        replace(&state, &pretty(record)?)?;
        if lose_line {
            return Ok(Persisted::LogLineLost(io::Error::other("lost by the test")));
        }
        match append_line(&dir.join(TRANSITIONS_FILE), transition) {
            Ok(()) => Ok(Persisted::Whole),
            Err(error) => Ok(Persisted::LogLineLost(error)),
        }
    }

    /// `<data>/drives.json` through `check_format(DRIVES_FORMAT)` (design part 7.4). An Err is "<path>: <reason>". An
    /// absent file gives the empty record with DRIVES_FORMAT. Def: T21. Use: T22, T23, T25, T26.
    pub(crate) fn drives(&self) -> Result<DrivesFile, String> {
        match read_checked(&self.data.join(DRIVES_FILE), DRIVES_FORMAT)? {
            Some((drives, _)) => Ok(drives),
            None => {
                Ok(DrivesFile { format: DRIVES_FORMAT.to_string(), drives: BTreeMap::new(), drive_incapable: vec![] })
            }
        }
    }

    /// Replaces `<data>/drives.json` (design part 7.4). It refuses as `persist` does when the existing file fails the
    /// check; otherwise `write_atomic` of `to_vec_pretty` plus "\n". Def: T21. Use: T22, T23, T25, T26.
    pub(crate) fn write_drives(&self, drives: &DrivesFile) -> io::Result<()> {
        let _writing = self.writing.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let path = self.data.join(DRIVES_FILE);
        refuse_foreign(&path, DRIVES_FORMAT)?;
        fs::create_dir_all(&self.data)?;
        write_atomic(&path, &pretty(drives)?)
    }

    /// `<data>/remembered/<fingerprint>.json` through `check_format(REMEMBERED_FORMAT)` (design part 7.4), with the
    /// file's bytes for remembered.json. An Err is "<path>: <reason>". Absent gives Ok(None).
    /// Def: T21. Use: T22, T23, T25, T26.
    pub(crate) fn remembered(&self, fingerprint: &str) -> Result<Option<(Remembered, Vec<u8>)>, String> {
        let path = self.data.join(REMEMBERED_DIR).join(format!("{fingerprint}.json"));
        read_checked(&path, REMEMBERED_FORMAT)
    }

    /// `<data>/settings.json` through `Settings::from_json` (design parts 3.5, 7.4). Absent gives
    /// `Settings::default()` with its `validate()`. An unreadable file gives `Settings::default()` and one problem
    /// { key: "settings.json", problem: "<path>: <error>" } (Task 21's ruling), which is `settings_file`'s Err.
    /// Def: T21. Use: T22, T23, T25, T26.
    pub(crate) fn settings(&self) -> (Settings, Vec<SettingsProblem>) {
        self.settings_file().unwrap_or_else(|problem| (Settings::default(), vec![problem]))
    }

    /// `<data>/settings.json` as the file says (review round 1, V13): Ok with the file's settings and `validate()`'s
    /// problems, or with `Settings::default()` and its problems when the file is absent; Err with the one problem
    /// { key: "settings.json", problem: "<path>: <error>" } when it cannot be read: not JSON, an unknown or ill-typed
    /// key, not UTF-8, a byte-order mark, or a read error other than NotFound. `settings_set` never merges into the
    /// stand-in defaults of an unreadable file (design part 7.4: reported, never overwritten). Def: PR #6 review (V13).
    /// Use: owner.rs (`settings_set`, `route`).
    pub(crate) fn settings_file(&self) -> Result<(Settings, Vec<SettingsProblem>), SettingsProblem> {
        let path = self.data.join(SETTINGS_FILE);
        let unreadable = |error: String| SettingsProblem { key: SETTINGS_FILE.to_string(), problem: at(&path, error) };
        match fs::read_to_string(&path) {
            Ok(text) => Settings::from_json(&text).map_err(unreadable),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let settings = Settings::default();
                let problems = settings.validate();
                Ok((settings, problems))
            }
            Err(error) => Err(unreadable(error.to_string())),
        }
    }

    /// Replaces `<data>/settings.json` (design part 7.4): `write_atomic` of `to_vec_pretty(settings)` plus "\n", with
    /// no `format` key, because `Settings` refuses an unknown key. Def: T21. Use: T22, T23, T25, T26.
    pub(crate) fn write_settings(&self, settings: &Settings) -> io::Result<()> {
        let _writing = self.writing.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        fs::create_dir_all(&self.data)?;
        write_atomic(&self.data.join(SETTINGS_FILE), &pretty(settings)?)
    }

    /// Appends "<now> <line>\n" to engine.log (design part 3.5), and writes the same text to `console` when one is
    /// set. Errors are ignored. Def: T21. Use: T22, T23, T25, T26.
    pub(crate) fn log(&self, line: &str) {
        let text = format!("{} {line}\n", (self.now)());
        let mut console = self.console.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let _ = fs::create_dir_all(&self.data);
        if let Ok(mut file) = OpenOptions::new().append(true).create(true).open(self.data.join(ENGINE_LOG)) {
            let _ = file.write_all(text.as_bytes());
        }
        if let Some(console) = console.as_mut() {
            let _ = console.write_all(text.as_bytes());
            let _ = console.flush();
        }
    }
}

/// `write_atomic(path, bytes)`, tried again on Windows while another process holds the file (RR1-2): a sharing or lock
/// violation, or the access denied a rename over a file open without delete sharing gives (os errors 32, 33 and 5),
/// is tried again up to `REPLACE_TRIES` times in all, `REPLACE_PAUSE` apart, as an indexer or virus scanner holding
/// state.json for a moment would otherwise fail the transition. Every other error, and every error elsewhere, is
/// returned at once. The retry stays when pull request 3 swaps in `MoveFileExW`. It is the engine's first production
/// retry loop (R29.c3 and R29.c4 keep theirs in test code).
#[cfg(windows)]
fn replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut tries = 1;
    loop {
        match write_atomic(path, bytes) {
            Err(error) if tries < REPLACE_TRIES && matches!(error.raw_os_error(), Some(5 | 32 | 33)) => {
                tries += 1;
                std::thread::sleep(REPLACE_PAUSE);
            }
            result => return result,
        }
    }
}

/// `write_atomic(path, bytes)`: outside Windows no other process's handle stops a rename, so nothing is tried again.
#[cfg(not(windows))]
fn replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_atomic(path, bytes)
}

/// Appends `to_string(transition)` plus "\n" to the transitions log at `path` and flushes it with `sync_data`, first
/// writing the "\n" a torn last line lacks (RP-8).
fn append_line(path: &Path, transition: &Transition) -> io::Result<()> {
    let mut log = OpenOptions::new().read(true).append(true).create(true).open(path)?;
    let mut line = Vec::new();
    if log.metadata()?.len() > 0 {
        let mut last = [0u8; 1];
        log.seek(SeekFrom::End(-1))?;
        log.read_exact(&mut last)?;
        if last[0] != b'\n' {
            line.push(b'\n');
        }
    }
    serde_json::to_writer(&mut line, transition)?;
    line.push(b'\n');
    log.write_all(&line)?;
    log.sync_data()
}

/// "<path>: <reason>", the form of every error the store reports.
fn at(path: &Path, reason: impl std::fmt::Display) -> String {
    format!("{}: {reason}", path.display())
}

/// `value` as `to_vec_pretty` plus "\n", the form of every JSON file the store writes.
fn pretty<T: Serialize>(value: &T) -> io::Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// The versioned file at `path` (design part 7.4): Ok(None) when absent; else its bytes read as JSON, passed through
/// `check_format(format)` and read into `T`, with the bytes. Every Err is "<path>: <reason>".
fn read_checked<T: DeserializeOwned>(path: &Path, format: &str) -> Result<Option<(T, Vec<u8>)>, String> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(at(path, error)),
    };
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|error| at(path, error))?;
    check_format(&value, format).map_err(|error| at(path, error))?;
    let read = serde_json::from_value(value).map_err(|error| at(path, error))?;
    Ok(Some((read, bytes)))
}

/// Ok when no file is at `path`, or when it is JSON that passes `check_format(format)`. Otherwise
/// `ErrorKind::InvalidData` with "<path>: <reason>": design part 7.4's refused file, which is never overwritten. It
/// reads the file at each write, so a refusal needs no state in the store.
fn refuse_foreign(path: &Path, format: &str) -> io::Result<()> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let refused = |reason: String| io::Error::new(io::ErrorKind::InvalidData, at(path, reason));
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|error| refused(error.to_string()))?;
    check_format(&value, format).map_err(refused)
}

/// `record` read back through its serialised form, so `titles` and `notes` come in the order state.json's writer
/// puts them (RP23), whatever order the file held.
fn in_written_order(record: &JobRecord) -> Result<JobRecord, serde_json::Error> {
    serde_json::from_value(serde_json::to_value(record)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::fs;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Arc, Mutex};

    use lazymkv_core::job::{Action, Event, State};
    use lazymkv_core::lookup::tmdb::{TmdbIdentity, TmdbKind};
    use lazymkv_core::record::{DriveRecord, DrivesFile, JobRecord, TitleRecord, Transition, DRIVES_FORMAT, JOB_FORMAT};
    use lazymkv_core::remembered::Remembered;
    use lazymkv_core::review::{Note, Noted};
    use lazymkv_core::settings::Settings;
    use lazymkv_platform::TEMP_SUFFIX;
    use serde_json::{json, Value};

    /// A temporary folder removed on drop.
    struct Temp(PathBuf);
    impl Temp {
        fn new(tag: &str) -> Temp {
            static NEXT: AtomicU32 = AtomicU32::new(0);
            let n = NEXT.fetch_add(1, Ordering::SeqCst);
            let dir = std::env::temp_dir().join(format!("lazymkv-engine-{tag}-{}-{n}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Temp(dir)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// A console that keeps what it is given.
    #[derive(Clone, Default)]
    struct Shared(Arc<Mutex<Vec<u8>>>);
    impl Write for Shared {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn store_in(data: &Path) -> Store {
        Store::new(data.to_path_buf(), Arc::new(|| 7), None)
    }

    fn record(state: State, updated_unix: u64) -> JobRecord {
        JobRecord {
            format: JOB_FORMAT.to_string(), id: "abc-1".to_string(), fingerprint: "ABC".to_string(), disc_id: None,
            drive: "tok0".to_string(), created_unix: 7, updated_unix, state, message: None, rip_again: false,
            profile: "job.mmcp.xml".to_string(), plan: None, titles: Vec::new(), filed_unix: None,
            collisions: Vec::new(), notes: Vec::new(),
        }
    }

    fn first() -> Transition {
        Transition { time_unix: 7, event: None, old: None, new: State::Detected, actions: vec![Action::Fingerprint] }
    }

    fn empty_drives() -> DrivesFile {
        DrivesFile { format: DRIVES_FORMAT.to_string(), drives: BTreeMap::new(), drive_incapable: Vec::new() }
    }

    #[test]
    fn persist_replaces_state_json_then_appends_one_transition_line() {
        let tmp = Temp::new("persist");
        let store = store_in(&tmp.0);
        let second = Transition {
            time_unix: 8, event: Some(Event::FingerprintDone), old: Some(State::Detected), new: State::Fingerprinted,
            actions: vec![Action::RunScan],
        };
        store.persist(&record(State::Detected, 7), &first()).unwrap();
        store.persist(&record(State::Fingerprinted, 8), &second).unwrap();
        let dir = tmp.0.join("jobs/abc-1");
        assert_eq!(store.job_dir("abc-1"), dir);
        let state = fs::read(dir.join("state.json")).unwrap();
        assert!(
            state.starts_with(b"{\n  \"format\": \"lazymkv job v1\",\n  \"id\": \"abc-1\",\n"),
            "{}",
            String::from_utf8_lossy(&state)
        );
        assert_eq!(serde_json::from_slice::<JobRecord>(&state).unwrap(), record(State::Fingerprinted, 8));
        let lines = fs::read_to_string(dir.join("transitions.jsonl")).unwrap();
        let expected =
            format!("{}\n{}\n", serde_json::to_string(&first()).unwrap(), serde_json::to_string(&second).unwrap());
        assert_eq!(lines, expected);
        let names: Vec<String> =
            fs::read_dir(&dir).unwrap().map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert!(names.iter().all(|name| !name.ends_with(TEMP_SUFFIX)), "{names:?}");
        assert_eq!(store.load_jobs(), vec![Ok(record(State::Fingerprinted, 8))]);
    }

    /// RP-8: a crash in the middle of an append leaves the last line without its newline; the next append ends it
    /// first, so every line still parses.
    #[test]
    fn persist_ends_a_torn_last_line_before_it_appends() {
        let tmp = Temp::new("torn");
        let store = store_in(&tmp.0);
        let dir = tmp.0.join("jobs/abc-1");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("transitions.jsonl"), "{\"time_unix\":6").unwrap();
        store.persist(&record(State::Detected, 7), &first()).unwrap();
        let expected = format!("{{\"time_unix\":6\n{}\n", serde_json::to_string(&first()).unwrap());
        assert_eq!(fs::read_to_string(dir.join("transitions.jsonl")).unwrap(), expected);
    }

    /// The carried note of Task 21: a loaded record holds `titles` by key and `notes` by note, the order state.json's
    /// writer uses (RP23), so it equals what its own state.json reads back as after the next persist.
    #[test]
    fn a_loaded_record_holds_its_titles_and_notes_in_the_written_order() {
        let tmp = Temp::new("order");
        let store = store_in(&tmp.0);
        let title = |key: &str| TitleRecord {
            key: key.to_string(), scan_title: 0, rip_index: None, attempts: 0, verified: false, verify_failures: 0,
            read_errors: Vec::new(), rip_reviews: Vec::new(), decision: None, filed: None,
        };
        let noted = |note: Note| Noted { note, stream: None, detail: "d".to_string() };
        let mut sorted = record(State::Detected, 7);
        sorted.titles = vec![title("t00-00800"), title("t01-00801")];
        sorted.notes = vec![noted(Note::PossibleCommentary), noted(Note::TuningEvidencePending)];
        // A state.json some other writer left with both lists reversed.
        let mut value = serde_json::to_value(&sorted).unwrap();
        value["titles"] = json!([value["titles"][1].clone(), value["titles"][0].clone()]);
        value["notes"] = json!([value["notes"][1].clone(), value["notes"][0].clone()]);
        fs::create_dir_all(tmp.0.join("jobs/abc-1")).unwrap();
        fs::write(tmp.0.join("jobs/abc-1/state.json"), serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(store.load_jobs(), vec![Ok(sorted.clone())]);
        store.persist(&sorted, &first()).unwrap();
        assert_eq!(store.load_jobs(), vec![Ok(sorted)]);
    }

    #[test]
    fn a_job_folder_with_a_refused_format_is_reported_and_never_overwritten() {
        let tmp = Temp::new("job-format");
        let store = store_in(&tmp.0);
        let refused = r#"{"format":"lazymkv job v2","id":"abc-1"}"#;
        let missing = r#"{"id":"abc-2"}"#;
        for (id, text) in [("abc-1", refused), ("abc-2", missing)] {
            fs::create_dir_all(tmp.0.join("jobs").join(id)).unwrap();
            fs::write(tmp.0.join("jobs").join(id).join("state.json"), text).unwrap();
        }
        // A folder that never persisted its first transition holds no job.
        fs::create_dir_all(tmp.0.join("jobs/abc-3")).unwrap();
        fs::write(tmp.0.join("jobs/abc-3/meta.json"), "{}").unwrap();
        let loaded = store.load_jobs();
        assert_eq!(loaded.len(), 2, "{loaded:?}");
        let first_error = loaded[0].clone().unwrap_err();
        assert!(first_error.contains("abc-1") && first_error.contains("lazymkv job v2"), "{first_error}");
        assert!(first_error.contains("lazymkv job v1"), "{first_error}");
        assert!(loaded[1].clone().unwrap_err().contains("abc-2"), "{loaded:?}");
        let error = store.persist(&record(State::Detected, 9), &first()).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(fs::read_to_string(tmp.0.join("jobs/abc-1/state.json")).unwrap(), refused);
        assert!(!tmp.0.join("jobs/abc-1/transitions.jsonl").exists());
    }

    #[test]
    fn an_absent_drives_json_is_an_empty_record() {
        let tmp = Temp::new("drives-absent");
        let drives = store_in(&tmp.0).drives().unwrap();
        assert_eq!(
            serde_json::to_value(&drives).unwrap(),
            json!({"format": "lazymkv drives v1", "drives": {}, "drive_incapable": []})
        );
    }

    #[test]
    fn drives_json_is_rewritten_with_its_format() {
        let tmp = Temp::new("drives");
        let store = store_in(&tmp.0);
        let mut drives = empty_drives();
        drives.drives.insert("tok0".to_string(), DriveRecord { fingerprint: Some("ABC".to_string()), handled: false });
        store.write_drives(&drives).unwrap();
        drives.drives.get_mut("tok0").unwrap().handled = true;
        store.write_drives(&drives).unwrap();
        let text = fs::read_to_string(tmp.0.join("drives.json")).unwrap();
        assert!(text.starts_with("{\n  \"format\": \"lazymkv drives v1\",\n"), "{text}");
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            value,
            json!({"format": "lazymkv drives v1", "drives": {"tok0": {"fingerprint": "ABC", "handled": true}},
                   "drive_incapable": []})
        );
        assert_eq!(serde_json::to_value(store.drives().unwrap()).unwrap(), value);
    }

    #[test]
    fn a_drives_json_with_a_refused_format_is_reported_and_never_overwritten() {
        let tmp = Temp::new("drives-format");
        let store = store_in(&tmp.0);
        let text = r#"{"format":"lazymkv drives v2","drives":{}}"#;
        fs::write(tmp.0.join("drives.json"), text).unwrap();
        let error = store.drives().unwrap_err();
        assert!(error.contains("lazymkv drives v2"), "{error}");
        assert_eq!(store.write_drives(&empty_drives()).unwrap_err().kind(), io::ErrorKind::InvalidData);
        assert_eq!(fs::read_to_string(tmp.0.join("drives.json")).unwrap(), text);
    }

    #[test]
    fn a_remembered_file_of_another_major_version_is_refused_by_the_store() {
        let tmp = Temp::new("remembered");
        let store = store_in(&tmp.0);
        let remembered = Remembered {
            fingerprint: "ABC".to_string(),
            tmdb: TmdbIdentity { kind: TmdbKind::Movie, id: 900001 },
            titles: Vec::new(),
        };
        let with_format = |format: &str| {
            let mut value = serde_json::to_value(&remembered).unwrap();
            value.as_object_mut().unwrap().insert("format".to_string(), json!(format));
            serde_json::to_vec(&value).unwrap()
        };
        assert_eq!(store.remembered("ABC"), Ok(None));
        fs::create_dir_all(tmp.0.join("remembered")).unwrap();
        let v1 = with_format("lazymkv remembered v1");
        fs::write(tmp.0.join("remembered/ABC.json"), &v1).unwrap();
        assert_eq!(store.remembered("ABC"), Ok(Some((remembered.clone(), v1))));
        fs::write(tmp.0.join("remembered/ABC.json"), with_format("lazymkv remembered v2")).unwrap();
        let error = store.remembered("ABC").unwrap_err();
        assert!(error.contains("lazymkv remembered v2"), "{error}");
    }

    #[test]
    fn settings_json_round_trips_without_a_format_key() {
        let tmp = Temp::new("settings");
        let store = store_in(&tmp.0);
        let (defaults, problems) = store.settings();
        assert_eq!(defaults, Settings::default());
        assert_eq!(problems.iter().map(|problem| problem.key.as_str()).collect::<Vec<_>>(), ["tuning_root"]);
        let mut changed = Settings::default();
        changed.cache_mb = 512;
        changed.keep_tuning_evidence = false;
        store.write_settings(&changed).unwrap();
        let value: Value = serde_json::from_slice(&fs::read(tmp.0.join("settings.json")).unwrap()).unwrap();
        assert!(value.get("format").is_none(), "{value}");
        let problems = changed.validate();
        assert_eq!(store.settings(), (changed, problems));
    }

    #[test]
    fn an_unreadable_settings_json_is_one_problem_and_the_defaults() {
        let tmp = Temp::new("settings-bad");
        let store = store_in(&tmp.0);
        fs::write(tmp.0.join("settings.json"), "{").unwrap();
        let (settings, problems) = store.settings();
        assert_eq!(settings, Settings::default());
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert_eq!(problems[0].key, "settings.json");
        let path = tmp.0.join("settings.json");
        assert!(problems[0].problem.starts_with(&format!("{}: ", path.display())), "{problems:?}");
    }

    /// Review round 1 (V13): `settings_file` reads an absent file as the defaults and a readable one with its problems,
    /// and gives an unreadable one as its one `settings.json` problem, which `settings` turns into the defaults.
    #[test]
    fn settings_file_is_err_for_an_unreadable_file_and_ok_for_an_absent_one() {
        let tmp = Temp::new("settings-file");
        let store = store_in(&tmp.0);
        let path = tmp.0.join("settings.json");
        let keys =
            |problems: &[SettingsProblem]| problems.iter().map(|problem| problem.key.clone()).collect::<Vec<_>>();
        let (absent, problems) = store.settings_file().unwrap();
        assert_eq!((absent, keys(&problems)), (Settings::default(), vec!["tuning_root".to_string()]));
        fs::write(&path, r#"{"file_phase_concurrency": 0, "keep_tuning_evidence": false}"#).unwrap();
        let (read, problems) = store.settings_file().unwrap();
        assert_eq!((read.file_phase_concurrency, keys(&problems)), (0, vec!["file_phase_concurrency".to_string()]));
        for text in ["{", r#"{"movie_root": "x"}"#, "\u{feff}{}"] {
            fs::write(&path, text).unwrap();
            let problem = store.settings_file().unwrap_err();
            assert_eq!(problem.key, "settings.json", "{text}");
            assert!(problem.problem.starts_with(&format!("{}: ", path.display())), "{text}: {problem:?}");
            assert_eq!(store.settings(), (Settings::default(), vec![problem]), "{text}");
        }
    }

    /// Review round 1 (V1, RR1-2): a state.json another process holds open without delete sharing, as an indexer or a
    /// virus scanner may, is replaced once that process lets it go, within the store's five tries, and the
    /// transitions line is written once. The holder shares reading, so the format check before the replace still reads
    /// the file.
    #[cfg(windows)]
    #[test]
    fn persist_retries_a_state_json_another_process_holds_open() {
        use std::os::windows::fs::OpenOptionsExt;
        use std::time::Instant;
        const FILE_SHARE_READ: u32 = 0x1;
        const FILE_SHARE_WRITE: u32 = 0x2;
        let tmp = Temp::new("held");
        let store = store_in(&tmp.0);
        store.persist(&record(State::Detected, 7), &first()).unwrap();
        let state = tmp.0.join("jobs/abc-1/state.json");
        let held = OpenOptions::new().read(true).share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE).open(&state).unwrap();
        let release = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            drop(held);
        });
        let second = Transition {
            time_unix: 8, event: Some(Event::FingerprintDone), old: Some(State::Detected), new: State::Fingerprinted,
            actions: vec![Action::RunScan],
        };
        let started = Instant::now();
        let persisted = store.persist(&record(State::Fingerprinted, 8), &second);
        let took = started.elapsed();
        release.join().unwrap();
        assert!(matches!(persisted, Ok(Persisted::Whole)), "{persisted:?}");
        assert!(took >= Duration::from_millis(100), "the replace did not wait for the holder: {took:?}");
        let lines = fs::read_to_string(tmp.0.join("jobs/abc-1/transitions.jsonl")).unwrap();
        assert_eq!(lines.lines().count(), 2, "{lines}");
        let written: JobRecord = serde_json::from_slice(&fs::read(&state).unwrap()).unwrap();
        assert_eq!(written, record(State::Fingerprinted, 8));
    }

    #[test]
    fn engine_log_lines_also_go_to_stderr_in_a_console_run() {
        let tmp = Temp::new("log");
        let console = Shared::default();
        let store = Store::new(tmp.0.clone(), Arc::new(|| 7), Some(Box::new(console.clone())));
        store.log("engine started");
        store.log("an arrival in tok0 made no job: the volume could not be listed");
        let expected = "7 engine started\n7 an arrival in tok0 made no job: the volume could not be listed\n";
        assert_eq!(fs::read_to_string(tmp.0.join("engine.log")).unwrap(), expected);
        assert_eq!(String::from_utf8(console.0.lock().unwrap().clone()).unwrap(), expected);
        let quiet = Temp::new("log-quiet");
        Store::new(quiet.0.clone(), Arc::new(|| 8), None).log("x");
        assert_eq!(fs::read_to_string(quiet.0.join("engine.log")).unwrap(), "8 x\n");
    }
}
