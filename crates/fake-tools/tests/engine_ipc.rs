//! The IPC server over the in-memory transport (design parts 3.7, 3.8, 9.1 to 9.4; Ruling S12).

use std::collections::VecDeque;
use std::fs;
use std::io::{BufRead, Write};
use std::path::Path;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use lazymkv_core::decision::Disposition;
use lazymkv_core::ipc::{read_reply, Envelope, Reply, CANCELLED, MAX_LINE_BYTES};
use lazymkv_core::job::{Event, Failure, FailureClass, NotifyEvent, State};
use lazymkv_core::plan::Mode;
use lazymkv_core::record::{JobRecord, JOB_FORMAT};
use lazymkv_core::review::ReviewReason;
use lazymkv_engine::testkit::{ci_factor, Harness, StepScript};
use lazymkv_engine::StepResult;
use lazymkv_platform::{IpcConnector, Reach};
use serde_json::{json, Value};

const WAIT: Duration = Duration::from_secs(20);
/// Task 22's disc names: the base scenario's disc, and two discs with their own fingerprints.
const DISC_A: &str = "movie-direct";
const DISC_B: &str = "disc-b";
const DISC_C: &str = "disc-c";
const FINAL: &str = "Synthetic Movie (2001) {tmdb-900001}/Synthetic Movie (2001) {tmdb-900001} - 1080p.mkv";

fn settings() -> Value { json!({"keep_tuning_evidence": false, "drive_settle_s": 0, "drive_ready_timeout_s": 1}) }

fn stand_in(scenarios: &[&str], settings: Value) -> Harness {
    Harness::new(Path::new(env!("CARGO_BIN_EXE_fake-tool")), scenarios, settings)
}

fn step(event: Event) -> StepScript { StepScript::Result(StepResult { event: Some(event), message: None }) }
fn none() -> StepScript { StepScript::Result(StepResult { event: None, message: None }) }
fn unknown() -> Failure { Failure { class: FailureClass::Unknown, output: None } }

/// The main line to Filed, in the order its actions run.
fn main_line() -> Vec<(&'static str, StepScript)> {
    vec![
        ("RunScan", step(Event::ScanDone)), ("Plan", step(Event::PlanDone { mode: Mode::Direct })),
        ("CheckSpace", step(Event::SpaceOk)), ("RipTitle", step(Event::TitleRipped)),
        ("VerifyFile", step(Event::TitleVerified { last: true })), ("Eject", step(Event::Ejected)),
        ("Probe", step(Event::ProbeDone)), ("ExtractSubtitleEvidence", none()),
        ("Decide", step(Event::DecisionDone { disposition: Disposition::AutoFile, reasons: vec![], has_path: true })),
        ("CheckSpace", step(Event::SpaceOk)), ("Remux", step(Event::RemuxDone)),
        ("VerifyFile", step(Event::FileVerified)), ("File", step(Event::FileDone)),
    ]
}

/// The main line up to the `nth` (from 0) entry of `tag`, which hangs; nothing after it.
fn hanging_at(tag: &str, nth: usize) -> Vec<(&'static str, StepScript)> {
    let mut seen = 0;
    let mut out = Vec::new();
    for (t, s) in main_line() {
        if t == tag {
            if seen == nth { out.push((t, StepScript::Hang)); return out; }
            seen += 1;
        }
        out.push((t, s));
    }
    panic!("no {tag} #{nth} in the main line")
}

fn scripted(script: Vec<(&'static str, StepScript)>) -> Harness {
    Harness::new_scripted(script, settings())
}

fn wait_for_call(h: &Harness, tag: &str, count: usize) {
    let start = Instant::now();
    while h.steps().calls().iter().filter(|c| c.as_str() == tag).count() < count {
        assert!(start.elapsed() < WAIT, "{tag} was not called {count} times: {:?}", h.steps().calls());
        std::thread::sleep(Duration::from_millis(5));
    }
}

struct Client { lines: Receiver<String>, writer: Box<dyn Write + Send>, next: u64, pushes: VecDeque<Envelope> }

impl Client {
    fn open(h: &Harness) -> Client {
        let stream = h.client().connect().unwrap_or_else(|_| panic!("the engine accepts the client"));
        let (send, lines) = channel();
        let mut reader = stream.reader;
        std::thread::spawn(move || loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => if send.send(line).is_err() { break },
            }
        });
        Client { lines, writer: stream.writer, next: 1, pushes: VecDeque::new() }
    }
    /// The next line, or None once the engine closed the connection.
    fn line(&mut self) -> Option<String> {
        // 20 s, 80 s under CI (Ruling R29.c2).
        let wait = WAIT * ci_factor();
        match self.lines.recv_timeout(wait) {
            Ok(line) => Some(line),
            Err(RecvTimeoutError::Disconnected) => None,
            Err(RecvTimeoutError::Timeout) => panic!("no line from the engine within {wait:?}"),
        }
    }
    fn raw(&mut self, line: &str) {
        self.writer.write_all(line.as_bytes()).unwrap();
        self.writer.write_all(b"\n").unwrap();
        self.writer.flush().unwrap();
    }
    fn request(&mut self, kind: &str, payload: Value) -> Reply {
        let id = self.next;
        self.next += 1;
        self.raw(&json!({"v": 1, "type": kind, "id": id, "payload": payload}).to_string());
        loop {
            match read_reply(self.line().expect("a reply").trim_end()).unwrap() {
                Reply::Push(envelope) => self.pushes.push_back(envelope),
                reply @ (Reply::Ok { id: Some(n), .. } | Reply::Error { id: Some(n), .. }) if n == id => return reply,
                other => panic!("unexpected {other:?}"),
            }
        }
    }
    fn ok(&mut self, kind: &str, payload: Value) -> Value {
        match self.request(kind, payload) { Reply::Ok { result, .. } => result, other => panic!("{kind}: {other:?}") }
    }
    fn err(&mut self, kind: &str, payload: Value) -> String {
        match self.request(kind, payload) { Reply::Error { message, .. } => message, other => panic!("{kind}: {other:?}") }
    }
    fn push(&mut self) -> Envelope {
        if let Some(envelope) = self.pushes.pop_front() { return envelope; }
        match read_reply(self.line().expect("a push").trim_end()).unwrap() {
            Reply::Push(envelope) => envelope,
            other => panic!("expected a push, got {other:?}"),
        }
    }
    fn snapshot(&mut self) -> Value {
        let envelope = self.push();
        assert_eq!(envelope.kind, "snapshot");
        envelope.payload
    }
    /// job_update pushes until one whose state is `state` (for `job` when given); returns its payload.
    fn until_state(&mut self, job: Option<&str>, state: &str) -> Value {
        loop {
            let envelope = self.push();
            if envelope.kind == "job_update" && envelope.payload["state"]["state"] == json!(state)
                && job.map_or(true, |id| envelope.payload["id"] == json!(id)) {
                return envelope.payload;
            }
        }
    }
}

// ---- Envelope (design part 9.1) ----

#[test]
fn a_wrong_v_is_answered_and_the_connection_stays_open() {
    let h = scripted(vec![]);
    let mut c = Client::open(&h);
    c.snapshot();
    c.raw(r#"{"v":2,"type":"history","id":7,"payload":{}}"#);
    assert!(matches!(read_reply(c.line().unwrap().trim_end()).unwrap(), Reply::Error { id: Some(7), .. }));
    assert!(c.ok("history", json!({})).as_array().unwrap().is_empty());
}

#[test]
fn a_line_over_4_mib_is_answered_then_the_client_is_dropped() {
    let h = scripted(vec![]);
    let mut c = Client::open(&h);
    c.snapshot();
    c.raw(&"x".repeat(MAX_LINE_BYTES + 1));
    assert!(matches!(read_reply(c.line().unwrap().trim_end()).unwrap(), Reply::Error { id: None, .. }));
    assert_eq!(c.line(), None);
}

#[test]
fn two_clients_each_get_every_push() {
    let h = scripted(main_line());
    let (mut a, mut b) = (Client::open(&h), Client::open(&h));
    a.snapshot();
    b.snapshot();
    h.insert("tok0", DISC_A);
    h.deliver();
    let states = |c: &mut Client| {
        let mut seen = Vec::new();
        loop {
            let e = c.push();
            if e.kind == "job_update" {
                let state = e.payload["state"]["state"].as_str().unwrap().to_string();
                seen.push(state.clone());
                if state == "Filed" { return seen; }
            }
        }
    };
    let (sa, sb) = (states(&mut a), states(&mut b));
    assert_eq!(sa, sb);
    assert_eq!(sa.first().map(String::as_str), Some("Detected"));
}

#[test]
fn a_reconnect_gets_a_fresh_snapshot() {
    let h = scripted(main_line());
    let mut a = Client::open(&h);
    assert!(a.snapshot()["jobs"].as_array().unwrap().is_empty());
    h.insert("tok0", DISC_A);
    h.deliver();
    a.until_state(None, "Filed");
    drop(a);
    let mut b = Client::open(&h);
    let jobs = b.snapshot()["jobs"].as_array().unwrap().clone();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0]["state"], json!({"state": "Filed"}));
}

#[test]
fn a_path_collision_job_update_carries_the_collisions() {
    let h = stand_in(&["movie-direct"], settings());
    let target = h.library().join(FINAL);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(&target, b"owner's").unwrap();
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    h.deliver();
    let held = c.until_state(None, "NeedsReview");
    let collisions = held["collisions"].as_array().unwrap();
    assert_eq!(collisions.len(), 1);
    assert!(collisions[0].as_str().unwrap().ends_with("Synthetic Movie (2001) {tmdb-900001} - 1080p.mkv"));
    assert!(held["review_reasons"].as_array().unwrap().contains(&json!("path_collision")));
}

// ---- Snapshot and history (design part 9.2) ----

fn seed(h: &Harness, n: u64, state: State) -> String {
    let fp = format!("{n:032X}");
    let id = format!("{}-1", fp.to_ascii_lowercase());
    let r = JobRecord {
        format: JOB_FORMAT.to_string(), id: id.clone(), fingerprint: fp, disc_id: None, drive: "tok9".to_string(),
        created_unix: n, updated_unix: n, state, message: None, rip_again: false, profile: "job.mmcp.xml".to_string(),
        plan: None, titles: vec![], filed_unix: None, collisions: vec![], notes: vec![],
    };
    let dir = h.data().join("jobs").join(&id);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("state.json"), serde_json::to_vec(&r).unwrap()).unwrap();
    id
}

#[test]
fn a_client_gets_a_snapshot_with_unfinished_and_50_recent_jobs() {
    let h = scripted(vec![]);
    for n in 1..=52 { seed(&h, n, State::Filed); }
    let waiting = seed(&h, 60, State::AwaitingTool { return_to: Box::new(State::Fingerprinted) });
    h.restart();
    let mut c = Client::open(&h);
    let snapshot = c.snapshot();
    let jobs = snapshot["jobs"].as_array().unwrap();
    assert_eq!(jobs.len(), 51);
    assert_eq!(jobs[0]["id"], json!(waiting));
    assert_eq!(jobs[1]["updated_unix"], json!(52));
    assert_eq!(jobs[50]["updated_unix"], json!(3));
    for key in ["jobs", "settings", "problems", "secrets", "selftest"] { assert!(snapshot.get(key).is_some(), "{key}"); }
}

#[test]
fn secrets_show_set_and_only_the_webhook_host() {
    let h = scripted(vec![]);
    let mut c = Client::open(&h);
    c.snapshot();
    assert_eq!(c.ok("secret_set", json!({"name": "ha_webhook_url", "value": "http://ha.local:8123/api/webhook/abc123"})), Value::Null);
    let mut d = Client::open(&h);
    let line = d.line().unwrap();
    assert!(!line.contains("abc123"));
    let secrets: Value = serde_json::from_str::<Value>(&line).unwrap()["payload"]["secrets"].clone();
    assert_eq!(secrets["ha_webhook_url"], json!({"set": true, "host": "ha.local"}));
    assert_eq!(secrets["webhook_url"], json!({"set": false, "host": null}));
}

#[test]
fn history_pages_at_most_50_before_a_job_over_ipc() {
    let h = scripted(vec![]);
    let ids: Vec<String> = (1..=60).map(|n| seed(&h, n, State::Filed)).collect();
    h.restart();
    let mut c = Client::open(&h);
    c.snapshot();
    let first = c.ok("history", json!({}));
    assert_eq!(first.as_array().unwrap().len(), 50);
    let next = c.ok("history", json!({"before": ids[10]}));
    assert_eq!(next.as_array().unwrap().len(), 10);
    assert_eq!(c.err("history", json!({"before": "nope-1"})), "no job nope-1");
}

// ---- Settings and secrets (design part 9.4) ----

#[test]
fn settings_set_with_a_problem_changes_nothing_and_lists_problems() {
    let h = scripted(vec![]);
    let before = fs::read(h.data().join("settings.json")).unwrap();
    let mut c = Client::open(&h);
    c.snapshot();
    match c.request("settings_set", json!({"file_phase_concurrency": 0})) {
        Reply::Error { message, problems, .. } => {
            assert_eq!(message, "settings_set refused: 1 problems");
            assert_eq!(problems.len(), 1);
            assert_eq!(problems[0].key, "file_phase_concurrency");
        }
        other => panic!("{other:?}"),
    }
    match c.request("settings_set", json!({"nope": 1})) {
        Reply::Error { problems, .. } => assert!(problems.is_empty()),
        other => panic!("{other:?}"),
    }
    assert_eq!(fs::read(h.data().join("settings.json")).unwrap(), before);
}

#[test]
fn settings_set_clearing_the_last_problem_turns_present_discs_into_arrivals() {
    let h = Harness::new_scripted(main_line(), json!({"keep_tuning_evidence": false, "drive_settle_s": 0,
                                                "drive_ready_timeout_s": 1, "file_phase_concurrency": 0}));
    let mut c = Client::open(&h);
    assert_eq!(c.snapshot()["problems"][0]["key"], json!("file_phase_concurrency"));
    h.insert("tok0", DISC_A);
    h.deliver();
    let result = c.ok("settings_set", json!({"file_phase_concurrency": 1}));
    assert_eq!(result["file_phase_concurrency"], json!(1));
    c.until_state(None, "Filed");
    assert_eq!(h.job_ids().len(), 1);
}

#[test]
fn settings_set_does_not_turn_a_handled_disc_into_an_arrival_again() {
    let h = stand_in(&["movie-direct", "movie-direct-copy-plan"], settings());
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    h.deliver();
    let id = c.until_state(None, "Failed")["id"].as_str().unwrap().to_string();
    h.settle();
    let lines = h.transitions(&id).len();
    let path = h.data().join("settings.json");
    let mut stored: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    stored["file_phase_concurrency"] = json!(0);
    fs::write(&path, serde_json::to_vec(&stored).unwrap()).unwrap();
    assert_eq!(c.ok("settings_set", json!({"file_phase_concurrency": 1}))["file_phase_concurrency"], json!(1));
    h.settle();
    assert_eq!(h.job_ids(), vec![id.clone()]);
    assert_eq!(h.transitions(&id).len(), lines);
    assert!(h.ejector().calls().is_empty());
}

#[test]
fn a_settings_set_mid_job_leaves_the_running_jobs_flags() {
    let h = stand_in(&["movie-direct", "movie-direct-hang"], settings());
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    h.deliver();
    let id = c.until_state(None, "Ripping")["id"].as_str().unwrap().to_string();
    c.ok("settings_set", json!({"cache_mb": 512}));
    c.ok("cancel", json!({"job": id}));
    c.ok("retry", json!({"job": id}));
    c.until_state(Some(&id), "Ripping");
    let log = fs::read_to_string(h.job_dir(&id).join("job.log")).unwrap();
    assert!(log.contains("--cache=1024"));
    assert!(!log.contains("--cache=512"));
    let job: Value = serde_json::from_slice(&fs::read(h.job_dir(&id).join("settings.json")).unwrap()).unwrap();
    assert_eq!(job["cache_mb"], json!(1024));
    let data: Value = serde_json::from_slice(&fs::read(h.data().join("settings.json")).unwrap()).unwrap();
    assert_eq!(data["cache_mb"], json!(512));
    c.ok("cancel", json!({"job": id}));
}

#[test]
fn secret_set_refuses_an_unknown_name_and_never_echoes_the_value() {
    let h = scripted(vec![]);
    let mut c = Client::open(&h);
    c.snapshot();
    let message = c.err("secret_set", json!({"name": "tmdb", "value": "s3cret-value"}));
    assert_eq!(message, "tmdb is not a secret name (design part 9.4)");
    assert_eq!(c.ok("secret_set", json!({"name": r"share:\\nas\media", "value": "user\npw"})), Value::Null);
    assert_eq!(c.ok("secret_set", json!({"name": "tmdb_token", "value": "s3cret-value"})), Value::Null);
    let log = fs::read_to_string(h.data().join("engine.log")).unwrap_or_default();
    assert!(!log.contains("s3cret-value"));
    let mut d = Client::open(&h);
    assert!(!d.line().unwrap().contains("s3cret-value"));
}

// ---- Retry (design part 9.4, L3) ----

#[test]
fn retry_on_failed_resumes_when_the_token_is_free() {
    let mut script = vec![("RunScan", step(Event::ScanDone)), ("RunScan", step(Event::ScanDone)),
                          ("Plan", step(Event::PlanDone { mode: Mode::Direct })), ("Plan", step(Event::PlanDone { mode: Mode::Direct })),
                          ("CheckSpace", step(Event::SpaceOk)), ("CheckSpace", step(Event::SpaceOk)),
                          ("RipTitle", step(Event::TitleRipFailed { failure: unknown() })), ("RipTitle", StepScript::Hang),
                          ("RipTitle", step(Event::TitleRipped))];
    script.extend(main_line().into_iter().skip(4).filter(|(t, _)| *t != "RipTitle"));
    let h = scripted(script);
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    h.deliver();
    let a = c.until_state(None, "Failed")["id"].as_str().unwrap().to_string();
    h.insert("tok1", DISC_B);
    wait_for_call(&h, "RipTitle", 2);
    let b = h.job_ids().into_iter().find(|id| *id != a).unwrap();
    c.ok("retry", json!({"job": a}));
    assert!(matches!(h.record(&a).state, State::Failed { .. }));
    c.ok("cancel", json!({"job": b}));
    c.until_state(Some(&a), "Ripping");
}

#[test]
fn retry_in_awaiting_space_feeds_space_freed_when_space_returns() {
    let h = stand_in(&["movie-direct"], settings());
    let staging = h.staging().to_str().unwrap().to_string();
    h.shares().set_free(&staging, 1);
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    h.deliver();
    let id = c.until_state(None, "AwaitingSpace")["id"].as_str().unwrap().to_string();
    let lines = h.transitions(&id).len();
    c.ok("retry", json!({"job": id}));
    assert_eq!(h.transitions(&id).len(), lines);
    h.shares().set_free(&staging, u64::MAX);
    c.ok("retry", json!({"job": id}));
    c.until_state(Some(&id), "Filed");
    assert!(h.transitions(&id).iter().any(|t| t.event == Some(Event::SpaceFreed)));
}

#[test]
fn retry_in_each_waiting_state_rechecks_its_condition() {
    // Share: the staging share comes back.
    let h = stand_in(&["movie-direct"], settings());
    let staging = h.staging().to_str().unwrap().to_string();
    h.shares().set_reach(&staging, Reach::Unreachable("gone".to_string()));
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    h.deliver();
    let id = c.until_state(None, "AwaitingShare")["id"].as_str().unwrap().to_string();
    h.shares().set_reach(&staging, Reach::Reachable);
    c.ok("retry", json!({"job": id}));
    c.until_state(Some(&id), "Filed");
    assert!(h.transitions(&id).iter().any(|t| t.event == Some(Event::ShareBack)));
    drop((c, h));

    // MakeMkv: the check after the retry reads v2.0.1 (listing 3 of the variant).
    let h = stand_in(&["movie-direct", "movie-direct-licence-update"], settings());
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    h.deliver();
    let id = c.until_state(None, "AwaitingMakeMkv")["id"].as_str().unwrap().to_string();
    c.ok("retry", json!({"job": id}));
    c.until_state(Some(&id), "Fingerprinted");
    assert!(h.transitions(&id).iter().any(|t| t.event == Some(Event::MakeMkvChanged)));
    drop((c, h));

    // Tool: makemkvcon is moved away before the insertion and back before the second retry.
    let h = stand_in(&["movie-direct"], settings());
    let stored: Value = serde_json::from_slice(&fs::read(h.data().join("settings.json")).unwrap()).unwrap();
    let tool = std::path::PathBuf::from(stored["tool_paths"]["makemkvcon"].as_str().unwrap());
    let aside = tool.with_extension("aside");
    let mut c = Client::open(&h);
    c.snapshot();
    fs::rename(&tool, &aside).unwrap();
    h.insert("tok0", DISC_A);
    h.deliver();
    let id = c.until_state(None, "AwaitingTool")["id"].as_str().unwrap().to_string();
    let lines = h.transitions(&id).len();
    c.ok("retry", json!({"job": id}));
    assert_eq!(h.transitions(&id).len(), lines);
    fs::rename(&aside, &tool).unwrap();
    c.ok("retry", json!({"job": id}));
    c.until_state(Some(&id), "Fingerprinted");
    assert!(h.transitions(&id).iter().any(|t| matches!(t.event, Some(Event::ToolFound { .. }))));
    drop((c, h));

    // Lookup: a retry runs a round over the recorded transport; the lookup stays pending, so it feeds nothing (S81).
    let mut script = main_line();
    script.truncate(8);
    script.push(("Decide", step(Event::LookupPending)));
    let h = scripted(script);
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    h.deliver();
    let id = c.until_state(None, "AwaitingLookup")["id"].as_str().unwrap().to_string();
    let asked = h.http().requests().len();
    let lines = h.transitions(&id).len();
    c.ok("retry", json!({"job": id}));
    h.wait("the retry's lookup round", |h| h.http().requests().len() > asked);
    h.settle();
    assert_eq!(h.transitions(&id).len(), lines);
    assert_eq!(h.state_of(&id), "AwaitingLookup");
}

#[test]
fn retry_of_a_file_phase_failure_waits_for_the_slot() {
    let mut script = main_line();
    script.truncate(6); // A's disc phase
    script.extend(main_line().into_iter().take(6)); // B's disc phase
    script.push(("Probe", step(Event::ProbeFailed { failure: unknown() }))); // A
    script.extend([("Probe", step(Event::ProbeDone)), ("ExtractSubtitleEvidence", none()),
                   ("Decide", step(Event::DecisionDone { disposition: Disposition::AutoFile, reasons: vec![], has_path: true })),
                   ("CheckSpace", step(Event::SpaceOk)), ("Remux", StepScript::Hang)]); // B holds the slot
    script.extend([("Probe", step(Event::ProbeDone))]); // A after its retry
    let h = scripted(script);
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    h.insert("tok1", DISC_B);
    wait_for_call(&h, "Remux", 1);
    let ids = h.job_ids();
    let a = ids.iter().find(|id| matches!(h.record(id).state, State::Failed { .. })).unwrap().clone();
    let b = ids.iter().find(|id| **id != a).unwrap().clone();
    c.ok("retry", json!({"job": a}));
    assert!(matches!(h.record(&a).state, State::Failed { .. }));
    c.ok("cancel", json!({"job": b}));
    c.until_state(Some(&a), "Inspecting");
}

#[test]
fn a_retry_with_the_disc_absent_feeds_media_removed_and_frees_the_token() {
    let mut script = main_line();
    script[3] = ("RipTitle", step(Event::TitleRipFailed { failure: unknown() }));
    script.extend(main_line()); // a second disc goes the whole way
    let h = scripted(script);
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    h.deliver();
    let a = c.until_state(None, "Failed")["id"].as_str().unwrap().to_string();
    h.remove("tok0");
    h.deliver();
    h.wait("tok0's record emptied", |h| h.drives_json()["drives"]["tok0"]["fingerprint"].is_null());
    c.ok("retry", json!({"job": a}));
    let lines = h.transitions(&a);
    let n = lines.len();
    assert_eq!(lines[n - 2].event, Some(Event::Retry));
    assert_eq!(lines[n - 1].event, Some(Event::MediaRemoved));
    assert_eq!(lines[n - 1].actions, vec![lazymkv_core::job::Action::WaitForDisc]);
    assert_eq!(h.steps().calls().iter().filter(|c| c.as_str() == "RipTitle").count(), 1);
    h.insert("tok1", DISC_B);
    h.deliver();
    c.until_state(None, "Ejected");
}

#[test]
fn a_retry_queues_behind_an_earlier_arrival() {
    let mut script = main_line();
    script[3] = ("RipTitle", step(Event::TitleRipFailed { failure: unknown() }));
    script.extend(main_line().into_iter().take(3));
    script.push(("RipTitle", StepScript::Hang)); // B holds the token
    script.extend(main_line()); // C, then A
    let h = scripted(script);
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    h.deliver();
    let a = c.until_state(None, "Failed")["id"].as_str().unwrap().to_string();
    h.insert("tok1", DISC_B);
    wait_for_call(&h, "RipTitle", 2);
    let b = h.job_ids().into_iter().find(|id| *id != a).unwrap();
    h.insert("tok2", DISC_C);
    h.deliver();
    let c_fingerprint = h.fingerprint(DISC_C);
    h.wait("disc C queued", |h| {
        let record = &h.drives_json()["drives"]["tok2"];
        record["fingerprint"] == json!(c_fingerprint) && record["handled"] == json!(false)
    });
    c.ok("retry", json!({"job": a}));
    c.ok("cancel", json!({"job": b}));
    let mut order = Vec::new();
    while order.len() < 2 {
        let e = c.push();
        if e.kind != "job_update" { continue; }
        let id = e.payload["id"].as_str().unwrap().to_string();
        if id != b && !order.contains(&id) && e.payload["state"]["state"] != json!("Failed") { order.push(id); }
    }
    assert_ne!(order[0], a);
    assert_eq!(order[1], a);
}

#[test]
fn rip_again_arms_the_job_and_the_next_insertion_starts_a_new_job() {
    let mut script = main_line();
    script.extend(main_line());
    let h = scripted(script);
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    h.deliver();
    let id = c.until_state(None, "Filed")["id"].as_str().unwrap().to_string();
    assert_eq!(c.ok("rip_again", json!({"job": id})), Value::Null);
    assert!(h.record(&id).rip_again);
    h.remove("tok0");
    h.insert("tok0", DISC_A);
    h.deliver();
    let second = c.until_state(None, "Detected")["id"].as_str().unwrap().to_string();
    assert_eq!(second, id.replace("-1", "-2"));
}

// ---- Cancel (design part 9.4, R-A7) ----

fn assert_cancelled(h: &Harness, id: &str, event_tag: &str, from: &str) {
    let last = h.transitions(id).pop().unwrap();
    let event = serde_json::to_value(&last.event).unwrap();
    assert_eq!(event["event"], json!(event_tag), "{from}");
    assert_eq!(event["failure"]["class"], json!("Unknown"), "{from}");
    assert!(matches!(&last.new, State::Failed { class: FailureClass::Unknown, from: f } if f.as_str() == from), "{from}");
    assert_eq!(h.record(id).message.as_deref(), Some(CANCELLED));
    assert!(matches!(h.notifications().last().unwrap().event, NotifyEvent::Failed { class: FailureClass::Unknown }));
}

#[test]
fn each_in_flight_action_cancels_to_its_design_part_9_4_event() {
    for (tag, nth, from, event) in [
        ("RunScan", 0, "Fingerprinted", "ScanFailed"), ("Plan", 0, "Scanned", "PlanFailed"),
        ("RipTitle", 0, "Ripping", "TitleRipFailed"), ("VerifyFile", 0, "Ripping", "TitleRipFailed"),
        ("Probe", 0, "Inspecting", "ProbeFailed"), ("Decide", 0, "Inspecting", "DecisionFailed"),
        ("Remux", 0, "Remuxing", "RemuxFailed"), ("VerifyFile", 1, "Remuxing", "FileVerifyFailed"),
        ("File", 0, "Verified", "FileFailed"),
    ] {
        let h = scripted(hanging_at(tag, nth));
        let mut c = Client::open(&h);
        c.snapshot();
        h.insert("tok0", DISC_A);
        wait_for_call(&h, tag, nth + 1);
        let id = h.job_ids().pop().unwrap();
        c.ok("cancel", json!({"job": id}));
        assert_cancelled(&h, &id, event, from);
    }
    // Fingerprint: a restart in Detected hands Fingerprint to the runner.
    let h = scripted(vec![("Fingerprint", StepScript::Hang)]);
    h.insert("tok0", DISC_A);
    h.run_until("Detected");
    h.restart();
    wait_for_call(&h, "Fingerprint", 1);
    let id = h.job_ids().pop().unwrap();
    let mut c = Client::open(&h);
    c.snapshot();
    c.ok("cancel", json!({"job": id}));
    assert_cancelled(&h, &id, "FingerprintFailed", "Detected");
    // Decide in Decided, after an approval.
    let mut script = hanging_at("Decide", 0);
    script.pop();
    script.push(("Decide", step(Event::DecisionDone { disposition: Disposition::NeedsReview,
                                                      reasons: vec![ReviewReason::IdentityUnknown], has_path: true })));
    script.push(("Decide", StepScript::Hang));
    let h = scripted(script);
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    h.deliver();
    let id = c.until_state(None, "NeedsReview")["id"].as_str().unwrap().to_string();
    c.ok("review_action", json!({"job": id, "action": "approve"}));
    wait_for_call(&h, "Decide", 2);
    c.ok("cancel", json!({"job": id}));
    assert_cancelled(&h, &id, "DecisionFailed", "Decided");
}

#[test]
fn cancel_during_check_space_or_eject_is_refused() {
    for (tag, action) in [("CheckSpace", "CheckSpace"), ("Eject", "Eject")] {
        let h = scripted(hanging_at(tag, 0));
        let mut c = Client::open(&h);
        c.snapshot();
        h.insert("tok0", DISC_A);
        wait_for_call(&h, tag, 1);
        let id = h.job_ids().pop().unwrap();
        assert_eq!(c.err("cancel", json!({"job": id})),
                   format!("job {id} is running {action}, which cancel does not stop (design part 9.4)"));
    }
}

#[test]
fn a_cancel_with_no_running_step_is_refused_naming_the_state() {
    let mut script = main_line();
    script[2] = ("CheckSpace", step(Event::SpaceShort));
    let h = scripted(script);
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    h.deliver();
    let id = c.until_state(None, "AwaitingSpace")["id"].as_str().unwrap().to_string();
    assert_eq!(c.err("cancel", json!({"job": id})), format!("job {id} has no running step: it is AwaitingSpace"));
    assert_eq!(c.err("cancel", json!({"job": "nope-1"})), "no job nope-1");
}

#[test]
fn a_cancelled_disc_phase_job_is_not_ejected() {
    let h = stand_in(&["movie-direct", "movie-direct-hang"], settings());
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    h.deliver();
    let id = c.until_state(None, "Ripping")["id"].as_str().unwrap().to_string();
    c.ok("cancel", json!({"job": id}));
    assert!(h.ejector().calls().is_empty());
}

#[test]
fn cancel_during_a_rip_kills_it_and_fails_with_cancelled_by_the_owner() {
    let h = stand_in(&["movie-direct", "movie-direct-hang"], settings());
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    h.deliver();
    let id = c.until_state(None, "Ripping")["id"].as_str().unwrap().to_string();
    // Ripping is pushed before the step's drive listing; rip.txt exists once the rip itself is spawned.
    h.wait("the rip to start", |h| h.job_dir(&id).join("t00-00800").join("rip.txt").is_file());
    let start = Instant::now();
    c.ok("cancel", json!({"job": id}));
    assert!(start.elapsed() < Duration::from_secs(10));
    assert_cancelled(&h, &id, "TitleRipFailed", "Ripping");
    let exit = fs::read_dir(h.job_dir(&id)).unwrap().map(|e| e.unwrap().path())
        .map(|p| p.join("rip.txt.exit.json")).find(|p| p.is_file()).unwrap();
    let exit: Value = serde_json::from_slice(&fs::read(exit).unwrap()).unwrap();
    assert_ne!(exit["exit_code"], json!(0));
}

// ---- Review actions (design part 9.4, P25, R-A8) ----

fn held_script() -> Vec<(&'static str, StepScript)> {
    let mut script = hanging_at("Decide", 0);
    script.pop();
    script.push(("Decide", step(Event::DecisionDone { disposition: Disposition::NeedsReview,
                                                      reasons: vec![ReviewReason::IdentityUnknown], has_path: true })));
    script
}

#[test]
fn approve_delivers_review_approved_when_the_slot_is_free() {
    let mut script = held_script();
    script.extend(main_line().into_iter().take(6)); // B's disc phase
    script.extend([("Probe", step(Event::ProbeDone)), ("ExtractSubtitleEvidence", none()),
                   ("Decide", step(Event::DecisionDone { disposition: Disposition::AutoFile, reasons: vec![], has_path: true })),
                   ("CheckSpace", step(Event::SpaceOk)), ("Remux", StepScript::Hang)]);
    let h = scripted(script);
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    h.deliver();
    let held = c.until_state(None, "NeedsReview");
    let a = held["id"].as_str().unwrap().to_string();
    let reasons = held["review_reasons"].clone();
    h.insert("tok1", DISC_B);
    wait_for_call(&h, "Remux", 1);
    let b = h.job_ids().into_iter().find(|id| *id != a).unwrap();
    c.ok("review_action", json!({"job": a, "action": "approve", "overrides": {}}));
    assert!(matches!(h.record(&a).state, State::NeedsReview { .. }));
    c.ok("cancel", json!({"job": b}));
    c.until_state(Some(&a), "Decided");
    let approved = h.transitions(&a).into_iter().find(|t| matches!(t.event, Some(Event::ReviewApproved { .. }))).unwrap();
    let event = serde_json::to_value(&approved.event).unwrap();
    assert_eq!(event["approved_reasons"], reasons);
}

#[test]
fn an_approved_job_decides_again_from_its_kept_inputs_and_files() {
    let h = stand_in(&["movie-direct"], settings());
    let target = h.library().join(FINAL);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(&target, b"owner's").unwrap();
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    h.deliver();
    let id = c.until_state(None, "NeedsReview")["id"].as_str().unwrap().to_string();
    c.ok("review_action", json!({"job": id, "action": "approve"}));
    c.until_state(Some(&id), "NeedsReview"); // path_collision holds it again (blocks_filing)
    fs::remove_file(&target).unwrap();
    c.ok("review_action", json!({"job": id, "action": "approve"}));
    c.until_state(Some(&id), "Filed");
    let log = fs::read_to_string(h.job_dir(&id).join("job.log")).unwrap();
    assert!(!log.contains("read back from the job folder"));
    assert_eq!(fs::read(&target).unwrap().len(), 16);
}

#[test]
fn approve_is_refused_outside_the_disposition_gate_naming_the_state() {
    let h = scripted(main_line());
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    h.deliver();
    let id = c.until_state(None, "Filed")["id"].as_str().unwrap().to_string();
    assert_eq!(c.err("review_action", json!({"job": id, "action": "approve"})),
               format!("job {id} is Filed, not held at the disposition gate"));
}

#[test]
fn approve_with_overrides_is_refused_naming_phase_3() {
    let h = scripted(held_script());
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    h.deliver();
    let id = c.until_state(None, "NeedsReview")["id"].as_str().unwrap().to_string();
    assert_eq!(c.err("review_action", json!({"job": id, "action": "approve", "overrides": {"season": 2}})),
               "approve waits for phase 3's review commands (R-A8)");
    assert_eq!(c.err("review_action", json!({"job": id, "action": "set_identity"})),
               "set_identity waits for phase 3's review commands (R-A8)");
}

#[test]
fn discard_is_refused_naming_phase_3() {
    let h = scripted(vec![]);
    let mut c = Client::open(&h);
    c.snapshot();
    assert_eq!(c.err("discard", json!({"job": "abc-1"})), "discard waits for phase 3's review commands (P2, R-A1)");
}

#[test]
fn ui_quit_is_refused_naming_phase_5() {
    let h = scripted(vec![]);
    let mut c = Client::open(&h);
    c.snapshot();
    assert_eq!(c.err("ui_quit", Value::Null), "ui_quit waits for phase 5's user interface (R-A8)");
}

// ---- Self-test (design parts 3.7, 9.4) ----

fn check<'a>(selftest: &'a Value, name: &str) -> &'a Value {
    selftest["checks"].as_array().unwrap().iter().find(|c| c["name"] == json!(name)).unwrap()
}

#[test]
fn the_engine_runs_the_makemkv_check_at_start_into_the_snapshot() {
    let h = stand_in(&["movie-direct", "movie-direct-licence-update"], settings());
    let mut c = Client::open(&h);
    let snapshot = c.snapshot();
    let makemkv = check(&snapshot["selftest"], "makemkvcon");
    assert_eq!(makemkv["ok"], json!(true));
    assert_eq!(makemkv["version"], json!("2.0.0"));
    assert_eq!(makemkv["found"], json!("settings"));
    assert_eq!(makemkv["busy"], json!(false));
    let names: Vec<&str> = snapshot["selftest"]["checks"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["makemkvcon", "mkvmerge", "ffprobe", "ffmpeg", "java", "staging_root", "tuning_root", "movies_root", "tmdb_token"]);
}

#[test]
fn run_selftest_pushes_results_then_delivers_tool_found() {
    let h = stand_in(&["movie-direct"], settings());
    let stored: Value = serde_json::from_slice(&fs::read(h.data().join("settings.json")).unwrap()).unwrap();
    let tool = std::path::PathBuf::from(stored["tool_paths"]["makemkvcon"].as_str().unwrap());
    let aside = tool.with_extension("aside");
    let mut c = Client::open(&h);
    c.snapshot();
    fs::rename(&tool, &aside).unwrap();
    h.insert("tok0", DISC_A);
    h.deliver();
    let id = c.until_state(None, "AwaitingTool")["id"].as_str().unwrap().to_string();
    fs::rename(&aside, &tool).unwrap();
    let result = c.ok("run_selftest", Value::Null);
    let pushed = c.pushes.iter().find(|e| e.kind == "selftest_result").expect("selftest_result before the reply");
    assert_eq!(pushed.payload, result);
    c.until_state(Some(&id), "Fingerprinted");
}

#[test]
fn another_discs_listing_with_a_changed_version_returns_a_job_from_awaiting_makemkv_when_the_token_is_free() {
    let h = stand_in(&["movie-direct", "movie-direct-licence-update"], settings());
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    h.deliver();
    let a = c.until_state(None, "AwaitingMakeMkv")["id"].as_str().unwrap().to_string();
    h.insert("tok1", DISC_B);
    h.deliver();
    let mut b_ejected = false;
    loop {
        let e = c.push();
        if e.kind != "job_update" { continue; }
        if e.payload["id"] != json!(a) && e.payload["state"]["state"] == json!("Ejected") { b_ejected = true; }
        if e.payload["id"] == json!(a) && e.payload["state"]["state"] != json!("AwaitingMakeMkv") {
            assert!(b_ejected, "the job left AwaitingMakeMkv while the other disc held the token");
            break;
        }
    }
    assert!(h.transitions(&a).iter().any(|t| t.event == Some(Event::MakeMkvChanged)));
}

#[test]
fn a_selftest_while_makemkvcon_runs_reports_the_last_check_marked_busy() {
    let h = stand_in(&["movie-direct", "movie-direct-hang"], settings());
    let mut c = Client::open(&h);
    let at_start = check(&c.snapshot()["selftest"], "makemkvcon").clone();
    h.insert("tok0", DISC_A);
    h.deliver();
    let id = c.until_state(None, "Ripping")["id"].as_str().unwrap().to_string();
    let result = c.ok("run_selftest", Value::Null);
    let busy = check(&result, "makemkvcon");
    assert_eq!(busy["busy"], json!(true));
    assert_eq!((busy["ok"].clone(), busy["version"].clone()), (at_start["ok"].clone(), at_start["version"].clone()));
    c.ok("cancel", json!({"job": id}));
}

#[test]
fn each_tool_check_reports_the_version_its_stand_in_prints() {
    let h = stand_in(&["movie-direct"], settings());
    let mut c = Client::open(&h);
    let at_start = c.snapshot();
    assert_eq!(check(&at_start["selftest"], "mkvmerge")["detail"], json!("not checked since the engine started"));
    let result = c.ok("run_selftest", Value::Null);
    for (tool, version) in [("mkvmerge", "88.0"), ("ffprobe", "7.1"), ("ffmpeg", "7.1"), ("java", "21.0.2")] {
        let found = check(&result, tool);
        assert_eq!((&found["ok"], &found["version"], &found["found"]), (&json!(true), &json!(version), &json!("settings")), "{tool}");
        assert!(found["path"].as_str().unwrap().contains(tool), "{tool}");
        for suffix in ["", ".stderr", ".timing.jsonl", ".exit.json"] {
            assert!(h.data().join(format!("selftest/{tool}-version.txt{suffix}")).is_file(), "{tool}{suffix}");
        }
    }
}

#[test]
fn the_selftest_java_check_reports_the_last_scans_java_use() {
    let h = stand_in(&["movie-direct", "movie-direct-java"], settings());
    let mut c = Client::open(&h);
    c.snapshot();
    let before = c.ok("run_selftest", Value::Null);
    assert!(check(&before, "java")["detail"].as_str().unwrap().contains("no scan yet"));
    h.insert("tok0", DISC_A);
    h.deliver();
    // Filed: no stand-in of the job runs beside the self-test's version calls (the stand-ins' counter files).
    c.until_state(None, "Filed");
    let after = c.ok("run_selftest", Value::Null);
    assert!(check(&after, "java")["detail"].as_str().unwrap().contains("java in use at the last scan: true"));
}

/// Ruling R26.c1 (design part 3.7): the MakeMKV check holds the disc-phase token while its listing runs, so a disc
/// that arrives then waits in the token's queue, and its job's RunScan lists the drives only after the check's
/// listing ended. The other tools are unset, so the self-test runs no stand-in beside the job's.
#[test]
fn a_makemkv_check_holds_the_disc_phase_token_until_its_listing_ends() {
    let unset = json!({"mkvmerge": null, "ffprobe": null, "ffmpeg": null, "java": null});
    let h = stand_in(&["movie-direct"], json!({"keep_tuning_evidence": false, "drive_settle_s": 0,
                                              "drive_ready_timeout_s": 1, "tool_paths": unset}));
    let mut c = Client::open(&h);
    c.snapshot();
    let (entered, in_listing) = channel::<()>();
    let (release, gate) = channel::<()>();
    let gate = std::sync::Mutex::new(gate);
    h.on_spawn(&["info", "disc:9999"], move || {
        let _ = entered.send(());
        let _ = gate.lock().unwrap().recv_timeout(WAIT);
    });
    c.raw(&json!({"v": 1, "type": "run_selftest", "id": 99, "payload": null}).to_string());
    in_listing.recv_timeout(WAIT).expect("the self-test's listing starts");
    h.insert("tok0", DISC_A);
    h.deliver();
    h.wait("the arrival to be walked", |h| h.drives_json()["drives"]["tok0"]["fingerprint"].is_string());
    h.settle();
    assert!(h.job_ids().is_empty(), "a job started while the check's listing ran: {:?}", h.job_ids());
    assert_eq!(h.drives_json()["drives"]["tok0"]["handled"], json!(false));
    release.send(()).unwrap();
    let selftest = loop {
        match read_reply(c.line().expect("a line").trim_end()).unwrap() {
            Reply::Ok { id: Some(99), result } => break result,
            Reply::Push(envelope) => c.pushes.push_back(envelope),
            other => panic!("unexpected {other:?}"),
        }
    };
    assert_eq!((&check(&selftest, "makemkvcon")["ok"], &check(&selftest, "makemkvcon")["busy"]), (&json!(true), &json!(false)));
    let id = c.until_state(None, "Scanned")["id"].as_str().unwrap().to_string();
    assert!(h.job_dir(&id).join("drive-list.txt").is_file());
}

/// The running check's CancelToken is raised at stop (design part 5.3): a listing that hangs ends with it.
#[test]
fn stop_cancels_a_makemkv_check_whose_listing_hangs() {
    let h = stand_in(&["movie-direct", "movie-direct-listing-hang"], settings());
    let mut c = Client::open(&h);
    c.snapshot();
    let out = h.data().join("selftest").join("drive-list.txt");
    let at_start = fs::read(&out).unwrap();
    c.raw(&json!({"v": 1, "type": "run_selftest", "id": 99, "payload": null}).to_string());
    h.wait("the self-test's listing to hang", |_| fs::read(&out).is_ok_and(|now| !now.is_empty() && now != at_start));
    let (stopped, done) = channel();
    let started = Instant::now();
    std::thread::spawn(move || {
        drop(h);
        let _ = stopped.send(());
    });
    assert!(done.recv_timeout(WAIT).is_ok(), "stop waited for the hanging listing");
    assert!(started.elapsed() < Duration::from_secs(10));
}

#[test]
fn a_restart_with_a_changed_makemkv_version_returns_awaiting_makemkv_jobs() {
    let h = stand_in(&["movie-direct", "movie-direct-licence-update"], settings());
    let recorded = |h: &Harness| -> Value {
        serde_json::from_slice(&fs::read(h.data().join("selftest/makemkv-check.json")).unwrap()).unwrap()
    };
    h.insert("tok0", DISC_A);
    h.run_until("AwaitingMakeMkv");
    let id = h.job_ids().pop().unwrap();
    assert_eq!(recorded(&h), json!({"version": "2.0.0", "licence": null}));
    h.restart();
    h.wait("MakeMkvChanged", |h| h.transitions(&id).iter().any(|t| t.event == Some(Event::MakeMkvChanged)));
    let events: Vec<Event> = h.transitions(&id).into_iter().filter_map(|t| t.event).collect();
    let at = events.iter().position(|e| *e == Event::MakeMkvChanged).unwrap();
    assert!(matches!(events[at - 1], Event::EngineRestart { .. }), "{events:?}");
    assert_eq!(recorded(&h), json!({"version": "2.0.1", "licence": null}));
}

#[test]
fn a_selftest_probes_each_library_root_inside_lazymkv_partial() {
    let h = stand_in(&["movie-direct"], settings());
    let mut c = Client::open(&h);
    c.snapshot();
    let result = c.ok("run_selftest", Value::Null);
    for name in ["staging_root", "tuning_root", "movies_root"] { assert_eq!(check(&result, name)["ok"], json!(true), "{name}"); }
    assert!(h.library().join(".lazymkv-partial").is_dir());
    let left: Vec<_> = fs::read_dir(h.library()).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(left, vec![std::ffi::OsString::from(".lazymkv-partial")]);
}

#[test]
fn the_token_check_runs_over_the_engines_transport() {
    let h = stand_in(&["movie-direct"], settings());
    h.http().answer(&lazymkv_clients::authentication_url(), 200, b"{\"success\":true}");
    let mut c = Client::open(&h);
    c.snapshot();
    c.ok("secret_set", json!({"name": "tmdb_token", "value": "test-token"}));
    let result = c.ok("run_selftest", Value::Null);
    assert_eq!(check(&result, "tmdb_token")["ok"], json!(true));
    assert!(h.http().requests().iter().any(|(url, _)| url == "https://api.themoviedb.org/3/authentication"));
}

// ---- Shutdown, progress, command and notifications ----

#[test]
fn shutdown_is_refused_while_a_step_runs_and_force_cancels_and_deletes_partials() {
    let h = scripted(hanging_at("Remux", 0));
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    wait_for_call(&h, "Remux", 1);
    let id = h.job_ids().pop().unwrap();
    let partial = h.library().join(".lazymkv-partial");
    fs::create_dir_all(&partial).unwrap();
    fs::write(partial.join(format!("{id}-t00-00800.mkv.partial")), b"x").unwrap();
    fs::write(partial.join("other-1-t00-00800.mkv.partial"), b"y").unwrap();
    let staged = h.staging().join(&id).join("rip").join("t00-00800");
    fs::create_dir_all(&staged).unwrap();
    fs::write(staged.join("title_t00.mkv"), b"raw").unwrap();
    assert_eq!(c.err("shutdown", json!({"force": false})),
               format!("a step is running for {id}; stop --force cancels it (design part 3.8)"));
    assert_eq!(c.ok("shutdown", json!({"force": true})), Value::Null);
    assert_eq!(c.line(), None);
    assert!(!partial.join(format!("{id}-t00-00800.mkv.partial")).exists());
    assert!(partial.join("other-1-t00-00800.mkv.partial").exists());
    assert_eq!(fs::read(staged.join("title_t00.mkv")).unwrap(), b"raw");
    let stopped = h.record(&id);
    assert_eq!(stopped.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Remuxing) });
    assert_eq!(stopped.message.as_deref(), Some(CANCELLED));
}

#[test]
fn shutdown_without_force_succeeds_when_only_waiting_held_failed_or_filed_jobs_exist() {
    // DISC_A is filed, DISC_B fails at its scan, DISC_C waits for space, and disc-d is held for review.
    let mut script = main_line();
    script.push(("RunScan", step(Event::ScanFailed { failure: unknown() })));
    script.extend([("RunScan", step(Event::ScanDone)), ("Plan", step(Event::PlanDone { mode: Mode::Direct })),
                   ("CheckSpace", step(Event::SpaceShort))]);
    let mut held = hanging_at("Decide", 0);
    held.pop();
    held.push(("Decide", step(Event::DecisionDone { disposition: Disposition::NeedsReview,
                                                    reasons: vec![ReviewReason::IdentityUnknown], has_path: true })));
    script.extend(held);
    let h = scripted(script);
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    h.deliver();
    c.until_state(None, "Filed");
    h.insert("tok1", DISC_B);
    h.deliver();
    c.until_state(None, "Failed");
    h.insert("tok2", DISC_C);
    h.deliver();
    c.until_state(None, "AwaitingSpace");
    h.insert("tok3", "disc-d");
    h.deliver();
    c.until_state(None, "NeedsReview");
    assert_eq!(c.ok("shutdown", json!({"force": false})), Value::Null);
    assert_eq!(c.line(), None);
    let mut states: Vec<String> = h.job_ids().iter().map(|id| h.state_of(id)).collect();
    states.sort();
    assert_eq!(states, ["AwaitingSpace", "Failed", "Filed", "NeedsReview"]);
}

#[test]
fn a_forced_shutdown_lets_an_in_flight_eject_finish_then_persists_it() {
    let mut script = hanging_at("Eject", 0);
    script.pop();
    script.push(("Eject", StepScript::After(Duration::from_millis(500), StepResult { event: Some(Event::Ejected), message: None })));
    let h = scripted(script);
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    wait_for_call(&h, "Eject", 1);
    let id = h.job_ids().pop().unwrap();
    assert_eq!(c.err("shutdown", json!({"force": false})),
               format!("a step is running for {id}; stop --force cancels it (design part 3.8)"));
    assert_eq!(c.ok("shutdown", json!({"force": true})), Value::Null);
    assert_eq!(c.line(), None);
    let last = h.transitions(&id).pop().unwrap();
    assert_eq!((last.event, last.new), (Some(Event::Ejected), State::Ejected { after_copy: false }));
}

#[test]
fn progress_pushes_name_the_action_and_the_title() {
    let h = stand_in(&["movie-direct"], settings());
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    h.deliver();
    let progress = loop {
        let e = c.push();
        if e.kind == "progress" { break e; }
    };
    assert_eq!(progress.id, None);
    let fraction = progress.payload["fraction"].as_f64().unwrap();
    assert!((0.0..=1.0).contains(&fraction));
    assert!(["RipTitle", "Remux"].contains(&progress.payload["step"].as_str().unwrap()));
}

#[test]
fn command_is_refused_until_the_run_folder_exists() {
    let h = scripted(vec![]);
    let mut c = Client::open(&h);
    c.snapshot();
    assert_eq!(c.err("command", json!({"words": ["plan", "--job", "abc-1"]})),
               "command needs the engine's run folder, which arrives in pull request 3 (S35)");
}

#[test]
fn notifications_reach_the_sink_in_order() {
    let mut script = main_line();
    script[6] = ("Probe", step(Event::ProbeFailed { failure: unknown() }));
    let h = scripted(script);
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    h.deliver();
    c.until_state(None, "Failed");
    let events: Vec<&str> = h.notifications().iter().map(|n| n.event.as_str()).collect();
    assert_eq!(events, ["disc_done", "failed"]);
}

#[test]
fn a_media_read_error_failure_message_says_try_the_other_drive() {
    let mut script = main_line();
    script[3] = ("RipTitle", step(Event::TitleRipFailed { failure: Failure { class: FailureClass::MediaReadError, output: None } }));
    let h = scripted(script);
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", DISC_A);
    h.deliver();
    c.until_state(None, "Failed");
    let failed = h.notifications().pop().unwrap();
    assert_eq!(failed.event, NotifyEvent::Failed { class: FailureClass::MediaReadError });
    assert!(failed.message.ends_with("; try the other drive"), "{}", failed.message);
}
