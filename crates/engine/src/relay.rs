//! The `command` relay (S35; design parts 4.4, 9.1, 9.3, 9.5; Rulings W10, W24, W25, T4-R9): the child's words, the
//! thread that fetches the job's stale TMDb copies again before the child, the thread that streams its lines to the
//! client that asked, and the reply.

use std::io::{self, BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use lazymkv_clients::{HttpTransport, LookupBook};
use lazymkv_core::ipc::{push_line, OutputStream, Push};
use lazymkv_core::plan::Plan;
use lazymkv_dev::forms::read_forms;
use lazymkv_platform::{GuardedChild, KillHandle};
use serde_json::{json, Value};

use crate::file_phase::{refetch_stale, replay_options};
use crate::ipc_server::{no_job, Clients};
use crate::owner::Msg;
use crate::store::Store;

/// A second `command` while one runs (design part 9.5). Def: T3-22. Use: T3-22.
pub(crate) const COMMAND_BUSY: &str = "a command is already running; one runs at a time (design part 9.5)";
/// An engine whose `EngineConfig.ctl` is None (design part 9.5). Def: T3-22. Use: T3-22.
pub(crate) const COMMAND_NO_CTL: &str = "command needs lazymkvctl beside the engine, and this engine has none";
/// `retune --jobs` before pull request 5 (Ruling W25). Def: T3-22. Use: T3-22.
pub(crate) const RETUNE_WAITS: &str = "retune --jobs needs the evidence trees of pull request 5 (design part 9.5)";
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

/// The child's words for a `command`'s `words` (design part 9.5's table; Rulings W24, W25), or the refusal's text.
/// `job` gives a job's folder and its title keys, None for an unknown job. Built words come first and the client's
/// remaining words after them, unchanged, so `Args`' last-wins rule lets a flag the client gave win. lazymkvctl sends a
/// relative path flag's value again, made absolute (V10's ruling); the engine resolves nothing and passes the words
/// unchanged. Def: T3-22. Use: T3-22.
pub(crate) fn child_words(
    words: &[String],
    job: &dyn Fn(&str) -> Option<(PathBuf, Vec<String>)>,
) -> Result<Vec<String>, String> {
    let Some((name, rest)) = words.split_first() else {
        return Err(COMMAND_FORMS.to_string());
    };
    if !["plan", "decide", "retune"].contains(&name.as_str()) {
        return Err(COMMAND_FORMS.to_string());
    }
    let forms = read_forms(name, rest)?;
    if forms.jobs {
        return Err(RETUNE_WAITS.to_string());
    }
    let Some(id) = forms.job else {
        return Err(COMMAND_FORMS.to_string());
    };
    let Some((folder, keys)) = job(&id) else {
        return Err(no_job(&id));
    };
    // `read_forms` lets `--title` through only with `decide --job` and requires it there, and refuses `--job` with
    // `retune`, so a title is decide's and no title is plan's.
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
/// child runs with a season missing (PR #15 review round 1, R2 follow-up, Ruling RR1-R2a). The
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
        );
        if let Some(job) = work.job_dir.file_name().and_then(|name| name.to_str()) {
            for line in fetched.log_lines() {
                work.store.job_log(job, &line);
            }
        }
        // A season still owed is the Err too, so no child runs with it missing (Ruling RR1-R2a).
        let refetched = fetched.for_decide();
        // An owner that is gone has stopped and answered the command already (Ruling T3-22-d).
        let _ = owner.send(Msg::CommandRefetched { client, id, words, refetched });
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
        let job = |id: &str| (id == "a-1").then(|| (s.0.join(id), vec!["t00-00800".to_string()]));
        let flags = vec!["--settings".to_string(), text(folder.join("settings.json")), "--tmdb-search".to_string(),
                         text(folder.join("tmdb-search.json"))];
        let mut plan = vec!["plan".to_string(), text(folder.clone())];
        plan.extend(flags.clone());
        plan.extend(words(&["--evidence", "e", "--settings", "mine.json"]));
        assert_eq!(child_words(&words(&["plan", "--job", "a-1", "--evidence", "e", "--settings", "mine.json"]), &job),
                   Ok(plan));
        let mut decide = vec!["decide".to_string(), "--rec".to_string(), text(folder.join("t00-00800"))];
        decide.extend(flags);
        assert_eq!(child_words(&words(&["decide", "--job", "a-1", "--t", "t00-00800"]), &job), Ok(decide));
    }

    #[test]
    fn every_other_form_is_refused_with_its_text() {
        let job = |id: &str| (id == "a-1").then(|| (PathBuf::from("jobs").join(id), vec!["t00-00800".to_string()]));
        let cases: Vec<(Vec<&str>, String)> = vec![
            (vec![], COMMAND_FORMS.to_string()),
            (vec!["status"], COMMAND_FORMS.to_string()),
            (vec!["plan", "some-slug"], COMMAND_FORMS.to_string()),
            (vec!["retune", "--set", "x=1"], COMMAND_FORMS.to_string()),
            (vec!["retune", "--jobs", "--spec", "spec.md"], RETUNE_WAITS.to_string()),
            (vec!["plan", "--jobs"], "argument --jobs: only with retune".to_string()),
            (vec!["plan", "--job", "b-1"], "no job b-1".to_string()),
            (vec!["decide", "--job", "a-1", "--title", "t99-00000"], "job a-1 has no title t99-00000".to_string()),
            (vec!["decide", "--job", "a-1", "--title", ".."], "job a-1 has no title ..".to_string()),
        ];
        for (given, refusal) in cases {
            assert_eq!(child_words(&words(&given), &job), Err(refusal), "{given:?}");
        }
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
}
