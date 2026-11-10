//! The `command` relay (S35; design parts 4.4, 9.1, 9.3, 9.5; Rulings W10, W24, W25 as G2 and G3 end it, T4-R9): the
//! child's words, the thread that fetches the job's, or the tuning recordings', stale TMDb copies again before the
//! child, the thread that streams its lines to the client that asked, and the reply.

use std::fs;
use std::io::{self, BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use lazymkv_clients::{
    reason_text, refetch_record, refetch_search, stale_copies, status_record, HttpTransport, LookupBook, PendingReason,
    Seasons,
};
use lazymkv_core::ipc::{push_line, OutputStream, Push};
use lazymkv_core::lookup::stale::TMDB_COPIES;
use lazymkv_core::plan::Plan;
use lazymkv_dev::forms::read_forms;
use lazymkv_platform::{GuardedChild, KillHandle};
use serde_json::{json, Value};

use crate::file_phase::{refetch_stale, replay_options, stale_sentence, OwedCheck, Refetched};
use crate::ipc_server::{no_job, Clients};
use crate::owner::Msg;
use crate::staged::write_checked;
use crate::store::Store;
use crate::tuning::{refresh_recordings, DATASET, RECORDINGS};

/// A second `command` while one runs (design part 9.5). Def: T3-22. Use: T3-22.
pub(crate) const COMMAND_BUSY: &str = "a command is already running; one runs at a time (design part 9.5)";
/// An engine whose `EngineConfig.ctl` is None (design part 9.5). Def: T3-22. Use: T3-22.
pub(crate) const COMMAND_NO_CTL: &str = "command needs lazymkvctl beside the engine, and this engine has none";
/// `retune --jobs` with no `tuning_root` in the engine's settings (design part 9.5; T5-R32). Def: T5-22. Use: T5-22.
pub(crate) const RETUNE_NO_TUNING_ROOT: &str =
    "retune --jobs needs tuning_root in the engine's settings (design part 9.5)";
/// A child that ended without an exit code (design part 9.3). Def: T3-22. Use: T3-22.
pub(crate) const NO_EXIT_CODE: &str = "lazymkvctl ended without an exit code";
/// Words that are none of design part 9.5's three engine forms (Ruling W24). Def: T3-22. Use: T3-22.
pub(crate) const COMMAND_FORMS: &str =
    "command needs plan --job, decide --job --title or retune --jobs (design part 9.5)";
/// A `lazymkvctl`, or its folder, that a non-elevated process could write (design part 3.6; Ruling W24; review round
/// 1 of pull request 3, R15). Def: T3-22. Use: T3-22.
pub(crate) const COMMAND_WRITABLE: &str =
    "lazymkvctl or its folder can be written by a non-elevated process, so the engine will not run it (design part 3.6)";

/// The exit code of a relayed command whose job's stale TMDb copies could not be fetched again (design part 9.5; Ruling
/// T4-R9; K15): no child ran. Def: T4-9. Use: T4-9.
pub(crate) const STALE_EXIT: i32 = 3;

/// `job <id> has no title <key>`, for a `decide --job --title` whose key is not one of the job record's titles, so no
/// key leads out of the job folder (Ruling W24). Def: T3-22. Use: T3-22.
pub(crate) fn no_title(id: &str, key: &str) -> String {
    format!("job {id} has no title {key}")
}

/// `lazymkvctl could not be started: <error>` (design part 9.5). Def: T3-22. Use: T3-22.
pub(crate) fn not_started(error: &io::Error) -> String {
    format!("lazymkvctl could not be started: {error}")
}

/// A path as one of the child's words.
fn word(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// A `tuning_root` with an empty or all-space value read as unset, as `Settings::validate` reads it (T5-R32).
fn set_root(root: Option<&str>) -> Option<&str> {
    root.filter(|root| !root.trim().is_empty())
}

/// A job the words may name (design part 9.5): its folder, its title keys, whether its dataset copy landed (T5-R14),
/// and its own settings' `tuning_root`, where that copy landed (ENG-19, call 14). Def: T5-22. Use: T5-22.
pub(crate) struct CommandJob {
    pub(crate) folder: PathBuf,
    pub(crate) keys: Vec<String>,
    pub(crate) dataset_done: bool,
    pub(crate) tuning_root: Option<String>,
}

/// The child's words for a `command`'s `words` (design part 9.5's table; Rulings W24, W25), or the refusal's text.
/// `job` gives a job the words may name (`CommandJob`), None for an unknown job; `tuning_root` is the engine's
/// settings' own, which only `retune --jobs` reads. Built words come first and the client's remaining words after
/// them, unchanged, so `Args`' last-wins rule lets a single-valued flag the client gave win, and a client's
/// `--evidence` adds a folder after the engine's (G2 amends Ruling W24 for `--evidence`). `retune --jobs` is `retune
/// --evidence <tuning_root>\recordings=<tuning_root>\dataset` and the client's words, refused with
/// `RETUNE_NO_TUNING_ROOT` when the engine's settings hold no `tuning_root` (T5-R32); a `decide --job --title` of a
/// job whose dataset copy landed (`tuning_copy` is `done`, T5-R14) gains `--text <tuning_root>\dataset` after the
/// replay flags (call 14), over the job's own settings' `tuning_root` (ENG-19). lazymkvctl sends a relative path
/// flag's value again, made absolute (V10's ruling); the engine resolves nothing and passes the words unchanged.
/// Def: T3-22, T5-22. Use: T3-22, T5-22.
pub(crate) fn child_words(
    words: &[String],
    job: &dyn Fn(&str) -> Option<CommandJob>,
    tuning_root: Option<&str>,
) -> Result<Vec<String>, String> {
    let Some((name, rest)) = words.split_first() else {
        return Err(COMMAND_FORMS.to_string());
    };
    if !["plan", "decide", "retune"].contains(&name.as_str()) {
        return Err(COMMAND_FORMS.to_string());
    }
    let forms = read_forms(name, rest)?;
    if forms.jobs {
        let Some(root) = set_root(tuning_root).map(Path::new) else {
            return Err(RETUNE_NO_TUNING_ROOT.to_string());
        };
        let trees = format!("{}={}", word(&root.join(RECORDINGS)), word(&root.join(DATASET)));
        let mut child = vec!["retune".to_string(), "--evidence".to_string(), trees];
        child.extend(forms.words);
        return Ok(child);
    }
    let Some(id) = forms.job else {
        return Err(COMMAND_FORMS.to_string());
    };
    let Some(CommandJob { folder, keys, dataset_done, tuning_root: job_root }) = job(&id) else {
        return Err(no_job(&id));
    };
    // `read_forms` lets `--title` through only with `decide --job` and requires it there, and refuses `--job` with
    // `retune`, so a title is decide's and no title is plan's.
    let decide = forms.title.is_some();
    let mut child = match forms.title {
        Some(key) if !keys.contains(&key) => return Err(no_title(&id, &key)),
        Some(key) => vec!["decide".to_string(), "--rec".to_string(), word(&folder.join(key))],
        None => vec!["plan".to_string(), word(&folder)],
    };
    let replay = replay_options(&folder);
    let flags = [
        ("--settings", replay.settings),
        ("--tmdb-search", replay.tmdb_search),
        ("--tmdb-record", replay.tmdb_record),
        ("--remembered", replay.remembered),
    ];
    for (flag, path) in flags {
        if let Some(path) = path {
            child.extend([flag.to_string(), word(&path)]);
        }
    }
    // Call 14 and T5-R14: the dataset copy landed under the job's own tuning_root, not the engine's (ENG-19).
    if let Some(root) = set_root(job_root.as_deref()).filter(|_| decide && dataset_done) {
        child.extend(["--text".to_string(), word(&Path::new(root).join(DATASET))]);
    }
    child.extend(forms.words);
    Ok(child)
}

/// The job `read_forms` reads from words `child_words` accepted (design part 9.5): `plan --job <id>` and `decide --job
/// <id> --title <key>` name it; None for words it does not read. Def: T4-9. Use: T4-9.
pub(crate) fn command_job(words: &[String]) -> Option<String> {
    let (name, rest) = words.split_first()?;
    read_forms(name, rest).ok()?.job
}

/// What the relay's refetch works with (design part 9.5; Ruling T4-R9): the job's folder, plan and lookup book, the
/// transport, the `tmdb_token` (None when none is set), the host's local date, and the store for job.log. Def: T4-9.
/// Use: T4-9.
pub(crate) struct CommandRefetch {
    pub(crate) job_dir: PathBuf,
    pub(crate) plan: Option<Plan>,
    pub(crate) lookups: Arc<Mutex<LookupBook>>,
    pub(crate) http: Arc<dyn HttpTransport>,
    pub(crate) token: Option<String>,
    pub(crate) today: String,
    pub(crate) store: Arc<Store>,
}

/// The job's stale TMDb copies fetched again before a relayed `plan` or `decide` (design part 9.5; Ruling T4-R9; K15),
/// on a thread of its own, so the owner thread never waits on a request: `refetch_stale` over `work`, each of its
/// `log_lines` in job.log, then `Msg::CommandRefetched` with the command's `client`, `id` and `words` and its
/// `for_decide`: Ok, or Err with the sentences of the seasons still owed and of the copies not fetched again, so no
/// child runs with a season missing (PR #15 review round 1, R2 follow-up, Ruling RR1-R2a). It checks the owed
/// seasons with `OwedCheck::Wait`, so a round that holds the book is waited for first, on this thread (Ruling
/// RR1-R2b). The
/// thread is detached and `shut_down` does not wait for it, as it does not wait for a lookup round: `refetch_stale`
/// reads no stop flag and may wait on TMDb requests, and a late `Msg::CommandRefetched` goes nowhere (PR #15 review
/// round 1, R1; Ruling T3-22-d).
/// Def: T4-9. Use: T4-9.
pub(crate) fn spawn_command_refetch(
    work: CommandRefetch,
    client: u64,
    id: Option<u64>,
    words: Vec<String>,
    owner: Sender<Msg>,
) {
    drop(thread::spawn(move || {
        let fetched = refetch_stale(
            &work.job_dir,
            work.plan.as_ref(),
            &work.lookups,
            work.http.as_ref(),
            work.token.as_deref(),
            &work.today,
            OwedCheck::Wait,
        );
        if let Some(job) = work.job_dir.file_name().and_then(|name| name.to_str()) {
            for line in fetched.log_lines() {
                work.store.job_log(job, &line);
            }
        }
        // A season still owed is the Err too, so no child runs with it missing (Ruling RR1-R2a).
        let refetched = fetched.for_decide();
        // An owner that is gone has stopped and answered the command already (Ruling T3-22-d).
        let _ = owner.send(Msg::CommandRefetched { client, id, words, refetched, warnings: Vec::new() });
    }));
}

/// What the relayed retune's refetch works with: the recordings folder, every job the owner holds with its refetch, and
/// the transport, token and date for the orphans (design part 9.5; G3, call 25). Def: T5-22. Use: T5-22.
pub(crate) struct RetuneRefetch {
    pub(crate) recordings: PathBuf,
    pub(crate) jobs: Vec<(String, CommandRefetch)>,
    pub(crate) http: Arc<dyn HttpTransport>,
    pub(crate) token: Option<String>,
    pub(crate) today: String,
}

/// `job <id>: <reason>`, an engine job's refusal of a relayed retune (design part 9.5; G3; R4-commands-44). Def: T5-22.
/// Use: T5-22.
pub(crate) fn job_refusal(id: &str, reason: &str) -> String {
    format!("job {id}: {reason}")
}

/// `<folder>: the TMDb copy <file> was fetched again`, an orphan's standard-error line (design parts 7.5 and 9.5;
/// call 25). Def: T5-22. Use: T5-22.
pub(crate) fn orphan_fetched(folder: &Path, file: &str) -> String {
    format!("{}: the TMDb copy {file} was fetched again", folder.display())
}

/// `<folder>: <stale sentence>; retune reports the folder as unmeasured`, an orphan's copy not fetched again, which
/// never refuses the run (design parts 7.5 and 9.5; call 25; T5-R40). Def: T5-22. Use: T5-22.
pub(crate) fn orphan_unmeasured(folder: &Path, sentence: &str) -> String {
    format!("{}: {sentence}; retune reports the folder as unmeasured", folder.display())
}

/// A JSON file's top-level `queries` strings, after one byte order mark; none when it cannot be read or holds none.
fn saved_queries(path: &Path) -> Vec<String> {
    let Ok(text) = fs::read_to_string(path) else { return Vec::new() };
    let Ok(saved) = serde_json::from_str::<Value>(text.strip_prefix('\u{feff}').unwrap_or(&text)) else {
        return Vec::new();
    };
    let queries = saved["queries"].as_array().map(Vec::as_slice).unwrap_or_default();
    queries.iter().filter_map(|query| query.as_str().map(str::to_string)).collect()
}

/// An orphan's stale TMDb copies fetched again into its recordings folder itself (design parts 7.5 and 9.5; call 25;
/// T5-R60), the folder being no job the owner holds, so no job folder, book or status file is touched. The status is
/// `<folder>\lookup-status.txt` with any byte order mark removed (empty when absent); the copies are `stale_copies`
/// over it read as a job folder (an orphan holds `state.json`, so a missing copy whose status line says 200 is fetched
/// too); none asks nothing. A listed `tmdb.json` or `tmdb-record.json` is fetched once for both with `refetch_record`
/// by the status file's record (`status_record`; none fails each listed record copy with `no TMDb ID names the
/// record`), every listed season with `tmdb-record.json` listed, none without; of the fetched files only the listed
/// copies are written, a `tmdb-record.json` whose fetch left a season pending failing with that season's sentence
/// instead. A listed `tmdb-search.json` is fetched with `refetch_search` over the copy's own `queries`. Each copy lands
/// through `write_checked`. Only `fetched` (the copies written, in `TMDB_COPIES` order) and `failed` (a
/// `stale_sentence` per copy not fetched again, or its write's error) are set. Def: T5-22. Use: T5-22.
pub(crate) fn refetch_orphan(folder: &Path, http: &dyn HttpTransport, token: Option<&str>, today: &str) -> Refetched {
    let status = fs::read_to_string(folder.join("lookup-status.txt")).unwrap_or_default();
    let status = status.strip_prefix('\u{feff}').unwrap_or(&status);
    let copies = stale_copies(status, &|file| fs::read(folder.join(file)).ok(), true, today);
    let mut done = Refetched::default();
    if copies.is_empty() {
        return done;
    }
    let listed = |file: &str| copies.iter().any(|copy| copy.file == file);
    let sentence = |file: &str, reason: &PendingReason| {
        let copy = copies.iter().find(|copy| copy.file == file);
        copy.map(|copy| stale_sentence(folder, copy, reason)).unwrap_or_default()
    };
    let write = |file: &str, bytes: &[u8]| {
        let path = folder.join(file);
        write_checked(&path, bytes).map_err(|error| {
            let why = format!("{}: the TMDb copy could not be written: {error}", path.display());
            sentence(file, &PendingReason::Unparsed(why))
        })
    };
    // Each listed copy's end: Ok when written, else the sentence `failed` holds for it.
    let mut ends: Vec<(String, Result<(), String>)> = Vec::new();
    if listed("tmdb.json") || listed("tmdb-record.json") {
        let seasons = if listed("tmdb-record.json") { Seasons::Listed } else { Seasons::None };
        let fetched = match status_record(status) {
            None => Err(PendingReason::Unparsed("no TMDb ID names the record".to_string())),
            Some(id) => refetch_record(http, token, id, seasons, today),
        };
        match fetched {
            Err(reason) => {
                for file in ["tmdb.json", "tmdb-record.json"].into_iter().filter(|file| listed(file)) {
                    ends.push((file.to_string(), Err(sentence(file, &reason))));
                }
            }
            Ok(refetched) => {
                for saved in refetched.files.iter().filter(|saved| listed(&saved.name)) {
                    let end = match &refetched.seasons.pending {
                        Some((n, reason)) if saved.name == "tmdb-record.json" => Err(format!(
                            "tmdb-record.json: season {n} could not be fetched again: {}",
                            reason_text(reason)
                        )),
                        _ => write(&saved.name, &saved.bytes),
                    };
                    ends.push((saved.name.clone(), end));
                }
            }
        }
    }
    if listed("tmdb-search.json") {
        let queries = saved_queries(&folder.join("tmdb-search.json"));
        let end = match refetch_search(http, token, &queries, today) {
            Ok((_, saved)) => write("tmdb-search.json", &saved.bytes),
            Err(reason) => Err(sentence("tmdb-search.json", &reason)),
        };
        ends.push(("tmdb-search.json".to_string(), end));
    }
    for file in TMDB_COPIES {
        for (_, end) in ends.iter().filter(|(name, _)| name == file) {
            match end {
                Ok(()) => done.fetched.push(file.to_string()),
                Err(sentence) => done.failed.push(sentence.clone()),
            }
        }
    }
    done
}

/// The folders of `recordings` in name order; a recordings folder that does not exist lists none.
fn recordings_folders(recordings: &Path) -> io::Result<Vec<PathBuf>> {
    let entries = match fs::read_dir(recordings) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut folders = Vec::new();
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            folders.push((entry.file_name(), entry.path()));
        }
    }
    folders.sort();
    Ok(folders.into_iter().map(|(_, path)| path).collect())
}

/// The relayed retune's refetch (design part 9.5; G3, call 25), on a thread of its own that it drops, as
/// `spawn_command_refetch` does, so the owner thread never lists the share or waits on a request. It lists
/// `work.recordings` (one that does not exist lists no folder; another error is the refusal `<recordings>: the tuning
/// recordings could not be listed: <error>`) and takes its folders in name order; a link or junction under
/// `recordings\` is not listed. A folder named for a job the owner holds is that job's mirror: its stale copies are
/// fetched again into the job folder with `refetch_stale` (`OwedCheck::Wait`), each `log_lines` line written to its
/// job.log; `for_decide`'s Err is the refusal `job_refusal(id, reason)`; otherwise, when anything was fetched,
/// `refresh_recordings` copies the job folder across once any owner refresh of that folder has ended (T5-R67), whose
/// error is the refusal `job <id>: the recordings copy could not be refreshed: <error>`, since the child would read
/// the stale mirror. Any other folder is an orphan, its copies fetched again into it with `refetch_orphan`: each copy
/// fetched is an `orphan_fetched` warning and each not fetched an `orphan_unmeasured` one, which never refuse the run.
/// Every folder is done before the one `Msg::CommandRefetched`, Ok with no refusal, else Err of the refusals joined
/// with "; ", and the warnings. Def: T5-22. Use: T5-22.
pub(crate) fn spawn_retune_refetch(
    work: RetuneRefetch,
    client: u64,
    id: Option<u64>,
    words: Vec<String>,
    owner: Sender<Msg>,
) {
    drop(thread::spawn(move || {
        let mut refusals = Vec::new();
        let mut warnings = Vec::new();
        match recordings_folders(&work.recordings) {
            Err(error) => refusals.push(format!(
                "{}: the tuning recordings could not be listed: {error}",
                work.recordings.display()
            )),
            Ok(folders) => {
                for folder in folders {
                    let name = folder.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
                    match work.jobs.iter().find(|(job_id, _)| *job_id == name) {
                        Some((job_id, job)) => {
                            let fetched = refetch_stale(
                                &job.job_dir,
                                job.plan.as_ref(),
                                &job.lookups,
                                job.http.as_ref(),
                                job.token.as_deref(),
                                &job.today,
                                OwedCheck::Wait,
                            );
                            let lines = fetched.log_lines();
                            for line in &lines {
                                job.store.job_log(job_id, line);
                            }
                            if let Err(reason) = fetched.for_decide() {
                                refusals.push(job_refusal(job_id, &reason));
                            } else if !lines.is_empty() {
                                if let Err(error) = refresh_recordings(&job.job_dir, &folder) {
                                    let reason = format!("the recordings copy could not be refreshed: {error}");
                                    refusals.push(job_refusal(job_id, &reason));
                                }
                            }
                        }
                        None => {
                            let fetched =
                                refetch_orphan(&folder, work.http.as_ref(), work.token.as_deref(), &work.today);
                            warnings.extend(fetched.fetched.iter().map(|file| orphan_fetched(&folder, file)));
                            warnings.extend(fetched.failed.iter().map(|sentence| orphan_unmeasured(&folder, sentence)));
                        }
                    }
                }
            }
        }
        let refetched = if refusals.is_empty() { Ok(()) } else { Err(refusals.join("; ")) };
        // An owner that is gone has stopped and answered the command already (Ruling T3-22-d).
        let _ = owner.send(Msg::CommandRefetched { client, id, words, refetched, warnings });
    }));
}

/// The reply to a `command` once its child ended (design parts 9.3, 9.5): `{"exit_code": <code>}`, `NO_EXIT_CODE`
/// for none, or `lazymkvctl could not be waited for: <error>`. Def: T3-22. Use: T3-22.
pub(crate) fn command_answer(exit: io::Result<Option<i32>>) -> Result<Value, String> {
    match exit {
        Ok(Some(code)) => Ok(json!({ "exit_code": code })),
        Ok(None) => Err(NO_EXIT_CODE.to_string()),
        Err(error) => Err(format!("lazymkvctl could not be waited for: {error}")),
    }
}

/// The command that runs (design part 9.5: one at a time): the client and request it answers and its child's kill
/// handle. Def: T3-22 (`kill` an Option: T4-9). Use: T3-22, T4-9.
pub(crate) struct RunningCommand {
    /// The client that sent the `command`.
    pub(crate) client: u64,
    /// The `command` request's id.
    pub(crate) id: Option<u64>,
    /// Kills the child `lazymkvctl` (Rulings T3-22-c, T3-22-d); None while the job's stale TMDb copies are fetched
    /// again, before the child starts (Ruling T4-R9).
    pub(crate) kill: Option<KillHandle>,
}

/// Streams `child`'s lines to `client` and reports its end, on a thread of its own (design parts 4.4, 9.1; Rulings
/// W10, W24): each line of either stream goes out as a `command_output` push carrying `id`, through
/// `Clients::send_waiting`; once both streams end, the child's `wait` goes to the owner as `Msg::CommandDone`.
/// Def: T3-22. Use: T3-22.
pub(crate) fn spawn_relay(
    child: Box<dyn GuardedChild>,
    client: u64,
    id: Option<u64>,
    clients: Arc<Clients>,
    owner: Sender<Msg>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut child = child;
        let kill = child.killer();
        let streams = [(OutputStream::Stdout, child.take_stdout()), (OutputStream::Stderr, child.take_stderr())];
        let forwarders: Vec<JoinHandle<()>> = streams
            .into_iter()
            .map(|(stream, reader)| {
                let (clients, kill) = (Arc::clone(&clients), kill.clone());
                thread::spawn(move || {
                    forward(reader, stream, client, id, &clients, &|| {
                        let _ = kill.kill();
                    })
                })
            })
            .collect();
        for forwarder in forwarders {
            let _ = forwarder.join();
        }
        let exit = child.wait();
        // An owner that is gone has stopped and answered the command already (Ruling T3-22-d).
        let _ = owner.send(Msg::CommandDone { client, id, exit });
    })
}

/// Sends each line of `reader` to `client` as a `command_output` of `stream` carrying `id`, until the end of the
/// stream or a read error. Each line loses its `\n` and the `\r` before it and is decoded with any byte that is not
/// UTF-8 replaced (PIPE-14). The first send that finds the client gone calls `kill`, which kills the child (Ruling
/// T3-22-c), and the lines after it are read and dropped, so the child never blocks on a full pipe. `kill` is a
/// closure, not the `KillHandle`, because no engine test can build a `KillHandle` (Ruling T8-d; PR #14 review round 1
/// (R19)).
fn forward(
    reader: Box<dyn Read + Send>,
    stream: OutputStream,
    client: u64,
    id: Option<u64>,
    clients: &Clients,
    kill: &dyn Fn(),
) {
    let mut reader = BufReader::new(reader);
    let mut delivering = true;
    let mut line = Vec::new();
    loop {
        line.clear();
        match reader.read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        if !delivering {
            continue;
        }
        let bytes = line.strip_suffix(b"\n").unwrap_or(&line[..]);
        let bytes = bytes.strip_suffix(b"\r").unwrap_or(bytes);
        let push = Push::CommandOutput { stream, line: String::from_utf8_lossy(bytes).into_owned() };
        if !clients.send_waiting(client, push_line(&push, id)) {
            delivering = false;
            kill();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc_server::held_writer::{lines_of, open, Held};
    use crate::ipc_server::{CLIENT_QUEUE, COMMAND_HEADROOM};
    use crate::testkit::ci_factor;
    use lazymkv_core::testsupport::{Scratch, ScratchExt};
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Condvar, Mutex};
    use std::time::{Duration, Instant};

    fn words(text: &[&str]) -> Vec<String> {
        text.iter().map(|word| word.to_string()).collect()
    }

    fn text(path: PathBuf) -> String {
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn the_child_words_are_the_job_folders_then_the_clients_own() {
        let s = Scratch::new("relay-words");
        s.write("a-1/settings.json", "{}");
        s.write("a-1/tmdb-search.json", "{}");
        let folder = s.0.join("a-1");
        let job = |id: &str| {
            (id == "a-1").then(|| CommandJob {
                folder: s.0.join(id),
                keys: vec!["t00-00800".to_string()],
                dataset_done: false,
                tuning_root: None,
            })
        };
        let flags = vec!["--settings".to_string(), text(folder.join("settings.json")), "--tmdb-search".to_string(),
                         text(folder.join("tmdb-search.json"))];
        let mut plan = vec!["plan".to_string(), text(folder.clone())];
        plan.extend(flags.clone());
        plan.extend(words(&["--evidence", "e", "--settings", "mine.json"]));
        let given = words(&["plan", "--job", "a-1", "--evidence", "e", "--settings", "mine.json"]);
        assert_eq!(child_words(&given, &job, None), Ok(plan));
        let mut decide = vec!["decide".to_string(), "--rec".to_string(), text(folder.join("t00-00800"))];
        decide.extend(flags);
        assert_eq!(child_words(&words(&["decide", "--job", "a-1", "--t", "t00-00800"]), &job, None), Ok(decide));
    }

    #[test]
    fn every_other_form_is_refused_with_its_text() {
        let job = |id: &str| {
            (id == "a-1").then(|| CommandJob {
                folder: PathBuf::from("jobs").join(id),
                keys: vec!["t00-00800".to_string()],
                dataset_done: false,
                tuning_root: None,
            })
        };
        let cases: Vec<(Vec<&str>, String)> = vec![
            (vec![], COMMAND_FORMS.to_string()),
            (vec!["status"], COMMAND_FORMS.to_string()),
            (vec!["plan", "some-slug"], COMMAND_FORMS.to_string()),
            (vec!["retune", "--set", "x=1"], COMMAND_FORMS.to_string()),
            (vec!["plan", "--jobs"], "argument --jobs: only with retune".to_string()),
            (vec!["plan", "--job", "b-1"], "no job b-1".to_string()),
            (vec!["decide", "--job", "a-1", "--title", "t99-00000"], "job a-1 has no title t99-00000".to_string()),
            (vec!["decide", "--job", "a-1", "--title", ".."], "job a-1 has no title ..".to_string()),
        ];
        for (given, refusal) in cases {
            assert_eq!(child_words(&words(&given), &job, Some("tuning-root")), Err(refusal), "{given:?}");
        }
    }

    /// One job, `a-1` in `folder`, with the title t00-00800, its dataset copy `done` or not, and `tuning-root` as its
    /// own settings' `tuning_root`.
    fn one_job(folder: PathBuf, done: bool) -> impl Fn(&str) -> Option<CommandJob> {
        rooted_job(folder, done, Some("tuning-root"))
    }

    /// `one_job` with its own settings' `tuning_root` given (ENG-19).
    fn rooted_job(folder: PathBuf, done: bool, root: Option<&str>) -> impl Fn(&str) -> Option<CommandJob> {
        let root = root.map(str::to_string);
        move |id: &str| {
            (id == "a-1").then(|| CommandJob {
                folder: folder.clone(),
                keys: vec!["t00-00800".to_string()],
                dataset_done: done,
                tuning_root: root.clone(),
            })
        }
    }

    /// `<root>\recordings=<root>\dataset`, the engine's `--evidence` value (design part 9.5; G2).
    fn trees(root: &str) -> String {
        format!("{}={}", text(Path::new(root).join("recordings")), text(Path::new(root).join("dataset")))
    }

    /// Design part 9.5's table: `retune --jobs` is `retune --evidence <tuning_root>\recordings=<tuning_root>\dataset`,
    /// then the client's words without `--jobs`.
    #[test]
    fn retune_jobs_is_relayed_with_the_tuning_trees_first() {
        let job = one_job(PathBuf::from("jobs").join("a-1"), false);
        let given = words(&["retune", "--jobs", "--spec", "spec.md"]);
        let expected = vec!["retune".to_string(), "--evidence".to_string(), trees("tuning-root"), "--spec".to_string(),
                            "spec.md".to_string()];
        assert_eq!(child_words(&given, &job, Some("tuning-root")), Ok(expected));
    }

    /// G2: the client's `--evidence` values follow the engine's, in the order given, so retune reads them all.
    #[test]
    fn the_clients_evidence_follows_the_engines_and_accumulates() {
        let job = one_job(PathBuf::from("jobs").join("a-1"), false);
        let given = words(&["retune", "--jobs", "--evidence", "clone", "--subtitles", "--evidence", "more=text",
                            "--spec", "spec.md"]);
        let mut expected = vec!["retune".to_string(), "--evidence".to_string(), trees("tuning-root")];
        expected.extend(words(&["--evidence", "clone", "--subtitles", "--evidence", "more=text", "--spec", "spec.md"]));
        assert_eq!(child_words(&given, &job, Some("tuning-root")), Ok(expected));
    }

    /// T5-R32: no tuning_root, an empty one or one of spaces refuses `retune --jobs` naming the setting.
    #[test]
    fn retune_jobs_without_tuning_root_is_refused_naming_it() {
        assert_eq!(RETUNE_NO_TUNING_ROOT, "retune --jobs needs tuning_root in the engine's settings (design part 9.5)");
        let job = one_job(PathBuf::from("jobs").join("a-1"), false);
        for root in [None, Some(""), Some("   ")] {
            assert_eq!(child_words(&words(&["retune", "--jobs", "--spec", "spec.md"]), &job, root),
                       Err(RETUNE_NO_TUNING_ROOT.to_string()), "{root:?}");
        }
    }

    /// Call 14 and T5-R14: `--text <tuning_root>\dataset` after the replay flags once the dataset copy is done, never
    /// before, never without a tuning_root and never for plan; the root is the job's own settings' `tuning_root`, where
    /// its copy landed (ENG-19), not the engine's; a client's own `--text` comes after it and wins.
    #[test]
    fn decide_job_gets_text_once_the_dataset_copy_is_done_and_not_before() {
        let s = Scratch::new("relay-text");
        s.write("a-1/settings.json", "{}");
        let folder = s.0.join("a-1");
        let replay = vec!["--settings".to_string(), text(folder.join("settings.json"))];
        let mut head = vec!["decide".to_string(), "--rec".to_string(), text(folder.join("t00-00800"))];
        head.extend(replay.clone());
        let given = words(&["decide", "--job", "a-1", "--title", "t00-00800"]);
        let mut with_text = head.clone();
        with_text.extend(["--text".to_string(), text(Path::new("tuning-root").join("dataset"))]);
        assert_eq!(child_words(&given, &one_job(folder.clone(), true), Some("tuning-root")), Ok(with_text.clone()));
        assert_eq!(child_words(&given, &one_job(folder.clone(), false), Some("tuning-root")), Ok(head.clone()));
        // The owner moved the engine's tuning_root after the copy landed: the job's own root still names its dataset.
        assert_eq!(child_words(&given, &one_job(folder.clone(), true), Some("moved-root")), Ok(with_text.clone()));
        assert_eq!(child_words(&given, &one_job(folder.clone(), true), None), Ok(with_text.clone()));
        assert_eq!(child_words(&given, &rooted_job(folder.clone(), true, None), Some("tuning-root")), Ok(head.clone()));
        assert_eq!(child_words(&given, &rooted_job(folder.clone(), true, Some("  ")), Some("tuning-root")), Ok(head));
        let mut mine = given.clone();
        mine.extend(words(&["--text", "mine"]));
        with_text.extend(words(&["--text", "mine"]));
        assert_eq!(child_words(&mine, &one_job(folder.clone(), true), Some("tuning-root")), Ok(with_text));
        let mut plan = vec!["plan".to_string(), text(folder.clone())];
        plan.extend(replay);
        let given = words(&["plan", "--job", "a-1"]);
        assert_eq!(child_words(&given, &one_job(folder, true), Some("tuning-root")), Ok(plan));
    }

    /// The burst's line count.
    const BURST: usize = 1000;

    /// A reader that hands out one line per `read`, `burst line 0001` to `burst line 1000`, and counts the lines it
    /// handed out; `BufReader` asks it once per empty buffer, so the count is the lines `forward` has taken.
    struct Burst {
        next: usize,
        taken: Arc<AtomicUsize>,
    }

    impl Read for Burst {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.next == BURST {
                return Ok(0);
            }
            self.next += 1;
            let line = format!("burst line {:04}\n", self.next);
            buf[..line.len()].copy_from_slice(line.as_bytes());
            self.taken.fetch_add(1, Ordering::SeqCst);
            Ok(line.len())
        }
    }

    /// Ruling T3-22-a: `forward` waits for room in a held client's queue, so the client is kept and every line reaches
    /// it in order; with the dropping `send`, the 257th line would drop the client and the writer would get at most
    /// `CLIENT_QUEUE` lines (PR #14 review round 1 (R19)).
    #[test]
    fn forward_waits_for_room_in_a_held_clients_queue_and_keeps_every_line() {
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let kept = Arc::new(Mutex::new(Vec::new()));
        let clients = Arc::new(Clients::new());
        let client = clients.add(Box::new(Held(gate.clone(), kept.clone())));
        let taken = Arc::new(AtomicUsize::new(0));
        let kills = Arc::new(AtomicUsize::new(0));
        let forwarder = {
            let reader = Burst { next: 0, taken: Arc::clone(&taken) };
            let (clients, kills) = (Arc::clone(&clients), Arc::clone(&kills));
            thread::spawn(move || {
                let kill = || {
                    kills.fetch_add(1, Ordering::SeqCst);
                };
                forward(Box::new(reader), OutputStream::Stdout, client, Some(1), &clients, &kill);
            })
        };
        let mark = CLIENT_QUEUE - COMMAND_HEADROOM;
        let wait = Duration::from_secs(10) * ci_factor();
        let deadline = Instant::now() + wait;
        while taken.load(Ordering::SeqCst) < mark {
            assert!(Instant::now() < deadline, "forward took {} lines in {wait:?}", taken.load(Ordering::SeqCst));
            thread::sleep(Duration::from_millis(5));
        }
        // A forward that dropped instead of waiting would run through the rest of the burst meanwhile.
        thread::sleep(Duration::from_millis(200));
        let held = taken.load(Ordering::SeqCst);
        assert!(held < BURST, "forward took all {held} lines while the client's writer was held");
        assert!(!forwarder.is_finished(), "forward ended while the client's writer was held");
        assert_eq!(clients.count(), 1);
        open(&gate);
        let deadline = Instant::now() + wait;
        while !forwarder.is_finished() {
            assert!(Instant::now() < deadline, "forward did not end within {wait:?} of the gate's opening");
            thread::sleep(Duration::from_millis(5));
        }
        forwarder.join().unwrap();
        let expected: Vec<String> = (1..=BURST)
            .map(|n| {
                let push = Push::CommandOutput { stream: OutputStream::Stdout, line: format!("burst line {n:04}") };
                push_line(&push, Some(1))
            })
            .collect();
        assert_eq!(lines_of(&kept, BURST), expected);
        assert_eq!(kills.load(Ordering::SeqCst), 0);
        assert_eq!(clients.count(), 1);
    }

    #[test]
    fn the_answer_is_the_exit_code_or_why_there_is_none() {
        assert_eq!(command_answer(Ok(Some(3))), Ok(json!({"exit_code": 3})));
        assert_eq!(command_answer(Ok(None)), Err(NO_EXIT_CODE.to_string()));
        assert_eq!(command_answer(Err(io::Error::other("gone"))),
                   Err("lazymkvctl could not be waited for: gone".to_string()));
    }

    /// H2: a link or junction under `recordings\` is neither an orphan nor a job's mirror, so it is not listed.
    #[test]
    fn the_recordings_listing_skips_a_link_to_another_folder() {
        let folders = Scratch::new("relay-recordings-link");
        let recordings = folders.0.join("recordings");
        let real = recordings.join("0000000000000000000000000000000a-1");
        fs::create_dir_all(&real).unwrap();
        let elsewhere = folders.0.join("elsewhere");
        fs::create_dir_all(&elsewhere).unwrap();
        let link = recordings.join("0000000000000000000000000000000b-1");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&elsewhere, &link).unwrap();
        #[cfg(windows)]
        {
            let made = std::process::Command::new("cmd").arg("/c").arg("mklink").arg("/J").arg(&link).arg(&elsewhere)
                .output().unwrap();
            assert!(made.status.success(), "mklink /J failed");
        }
        assert!(link.is_dir(), "the link reads as a folder when followed");
        assert_eq!(recordings_folders(&recordings).unwrap(), [real]);
    }
}
