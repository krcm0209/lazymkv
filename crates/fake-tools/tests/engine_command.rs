//! The `command` relay on a stand-in lazymkvctl (S35, S76; design parts 9.1, 9.3, 9.5; Rulings W10, W24, W25,
//! T3-22-a to T3-22-d). `Harness::new` copies `fake-tool` as `<tools>/lazymkvctl` when a scenario holds
//! `lazymkvctl.json`; the tests choose its responses with marker words the relay passes through (`--relay-...`).

mod common;

use std::env::consts::EXE_SUFFIX;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use common::{Client, WAIT};
use lazymkv_core::ipc::Reply;
use lazymkv_core::testsupport::Scratch;
use lazymkv_ctl::{run, Env};
use lazymkv_dev::evidence::Ctx;
use lazymkv_engine::testkit::{ci_factor, Harness};
use lazymkv_platform::Spawn;
use serde_json::{json, Value};

const KEY: &str = "t00-00800";
/// movie-direct's disc and the stand-in lazymkvctl.
const RELAY: &[&str] = &["movie-direct", "command-relay"];
const REPLAY: [&str; 4] = ["settings.json", "tmdb-search.json", "tmdb-record.json", "remembered.json"];
// The relay's texts (Rulings W24, W25, T3-22-d), as the engine's crate-private constants hold them.
const BUSY: &str = "a command is already running; one runs at a time (design part 9.5)";
const NO_CTL: &str = "command needs lazymkvctl beside the engine, and this engine has none";
const RETUNE: &str = "retune --jobs needs the evidence trees of pull request 5 (design part 9.5)";
const FORMS: &str = "command needs plan --job, decide --job --title or retune --jobs (design part 9.5)";
const WRITABLE: &str =
    "lazymkvctl's folder can be written by a non-elevated process, so the engine will not run it (design part 3.6)";
const STOPPING: &str = "the engine is stopping (design part 3.8)";

fn settings() -> Value { json!({"keep_tuning_evidence": false, "drive_settle_s": 0, "drive_ready_timeout_s": 1}) }

fn stand_in(scenarios: &[&str]) -> Harness {
    Harness::new(Path::new(env!("CARGO_BIN_EXE_fake-tool")), scenarios, settings())
}

/// A harness on RELAY whose movie-direct job is Filed and whose engine was restarted, so no step runs, with the job's
/// id (as ctl_engine.rs's `engine(true)`).
fn filed() -> (Harness, String) {
    let h = stand_in(RELAY);
    h.insert("tok0", "movie-direct");
    h.run_until("Filed");
    h.restart();
    let id = h.job_ids().pop().expect("the filed job");
    (h, id)
}

fn text(path: PathBuf) -> String { path.to_string_lossy().into_owned() }

/// Leaves exactly the replay files `given` of REPLAY in the job folder `job`; one it adds holds `{}`.
fn replay_files(job: &Path, given: &[&str]) {
    for name in REPLAY {
        let path = job.join(name);
        if !given.contains(&name) {
            let _ = fs::remove_file(&path);
        } else if !path.is_file() {
            fs::write(&path, "{}").unwrap();
        }
    }
}

/// `head`, then the flag and path of each replay file of `given`, in design part 9.5's order.
fn built(head: &[String], job: &Path, given: &[&str]) -> Vec<String> {
    let mut words = head.to_vec();
    for (flag, name) in ["--settings", "--tmdb-search", "--tmdb-record", "--remembered"].into_iter().zip(REPLAY) {
        if given.contains(&name) { words.extend([flag.to_string(), text(job.join(name))]); }
    }
    words
}

/// Every spawn of the stand-in lazymkvctl, in order.
fn ctl_spawns(h: &Harness) -> Vec<Spawn> {
    h.spawns().into_iter().filter(|spawn| spawn.program.file_stem().is_some_and(|stem| stem == "lazymkvctl")).collect()
}

/// The lines of `stream` among a command's outputs, in order.
fn of(outputs: &[(String, String)], stream: &str) -> Vec<String> {
    outputs.iter().filter(|(of, _)| of == stream).map(|(_, line)| line.clone()).collect()
}

fn exit(code: i32, id: u64) -> Reply { Reply::Ok { id: Some(id), result: json!({"exit_code": code}) } }

fn pair(stream: &str, line: &str) -> (String, String) { (stream.to_string(), line.to_string()) }

/// A client that has read its snapshot.
fn client(h: &Harness) -> Client {
    let mut c = Client::open(h);
    c.snapshot();
    c
}

/// The stand-in lazymkvctl's path in the harness's tools folder.
fn ctl_path(h: &Harness) -> PathBuf { h.tools().join(format!("lazymkvctl{EXE_SUFFIX}")) }

#[test]
fn plan_job_runs_the_child_with_the_job_folder_and_its_existing_replay_flags() {
    let (h, id) = filed();
    let job = h.job_dir(&id);
    replay_files(&job, &REPLAY);
    let mut c = client(&h);
    let (outputs, reply) = c.command(&["plan", "--job", &id]);
    assert_eq!(reply, exit(0, 1));
    assert_eq!(ctl_spawns(&h).pop().unwrap().args, built(&["plan".to_string(), text(job.clone())], &job, &REPLAY));
    assert_eq!(of(&outputs, "stdout"), ["relay plan line 1", "relay plan line 2"]);
    assert_eq!(of(&outputs, "stderr"), ["relay plan warning"]);
}

#[test]
fn decide_job_title_runs_decide_on_the_title_folder() {
    let (h, id) = filed();
    let job = h.job_dir(&id);
    replay_files(&job, &REPLAY);
    let mut c = client(&h);
    let (outputs, reply) = c.command(&["decide", "--job", &id, "--title", KEY]);
    assert_eq!(reply, exit(0, 1));
    let head = ["decide".to_string(), "--rec".to_string(), text(job.join(KEY))];
    assert_eq!(ctl_spawns(&h).pop().unwrap().args, built(&head, &job, &REPLAY));
    assert_eq!(outputs, [pair("stdout", "relay decide line 1")]);
}

#[test]
fn a_missing_replay_file_adds_no_flag() {
    let (h, id) = filed();
    let job = h.job_dir(&id);
    let given = ["settings.json", "tmdb-search.json"];
    replay_files(&job, &given);
    let mut c = client(&h);
    let (_, reply) = c.command(&["plan", "--job", &id]);
    assert_eq!(reply, exit(0, 1));
    assert_eq!(ctl_spawns(&h).pop().unwrap().args, built(&["plan".to_string(), text(job.clone())], &job, &given));
}

#[test]
fn the_clients_own_words_come_after_the_built_ones() {
    let (h, id) = filed();
    let job = h.job_dir(&id);
    replay_files(&job, &REPLAY);
    let mut c = client(&h);
    let (_, reply) = c.command(&["plan", "--job", &id, "--settings", "mine.json", "--evidence", "e"]);
    assert_eq!(reply, exit(0, 1));
    let mut expected = built(&["plan".to_string(), text(job.clone())], &job, &REPLAY);
    expected.extend(["--settings", "mine.json", "--evidence", "e"].map(str::to_string));
    assert_eq!(ctl_spawns(&h).pop().unwrap().args, expected);
}

/// Ruling W10: `command_output` goes only to the client that asked, with its command's id. B's queue is in order, so
/// a line sent to B would come before the reply to B's later `history`, whose read fails on a `command_output` of
/// another id.
#[test]
fn each_child_line_is_a_command_output_with_the_commands_id_to_that_client_only() {
    let (h, id) = filed();
    let (mut a, mut b) = (client(&h), client(&h));
    a.next = 7;
    let (outputs, reply) = a.command(&["plan", "--job", &id]);
    assert_eq!(reply, exit(0, 7));
    assert_eq!(outputs.len(), 3, "{outputs:?}");
    b.ok("history", json!({}));
    assert!(b.pushes.iter().all(|envelope| envelope.kind != "command_output"), "{:?}", b.pushes);
}

#[test]
fn a_watch_client_sees_no_command_output() {
    let (h, id) = filed();
    let mut watcher = client(&h);
    let mut a = client(&h);
    let (_, reply) = a.command(&["plan", "--job", &id]);
    assert_eq!(reply, exit(0, 1));
    a.ok("run_selftest", Value::Null);
    loop {
        let envelope = watcher.push();
        assert_ne!(envelope.kind, "command_output", "{envelope:?}");
        if envelope.kind == "selftest_result" {
            break;
        }
    }
}

/// Ruling T3-22-a: the relay waits for room in the client's queue, so 1000 lines, four times `CLIENT_QUEUE`, all
/// arrive in order and the client is kept.
#[test]
fn a_burst_of_child_lines_larger_than_the_client_queue_all_reach_the_client() {
    let (h, id) = filed();
    let mut c = client(&h);
    let (outputs, reply) = c.command(&["plan", "--job", &id, "--relay-burst"]);
    assert_eq!(reply, exit(0, 1));
    let expected: Vec<String> = (1..=1000).map(|n| format!("relay burst line {n:04}")).collect();
    assert_eq!(of(&outputs, "stdout"), expected);
}

#[test]
fn the_reply_carries_the_childs_exit_code() {
    let (h, id) = filed();
    let mut c = client(&h);
    let (_, reply) = c.command(&["plan", "--job", &id, "--relay-exit-3"]);
    assert_eq!(reply, exit(3, 1));
}

#[test]
fn a_second_command_while_one_runs_is_refused() {
    let (h, id) = filed();
    let mut a = client(&h);
    a.send("command", json!({"words": ["plan", "--job", id, "--relay-hang"]}));
    assert_eq!(a.output(), pair("stdout", "relay hang line"));
    let mut b = client(&h);
    assert_eq!(b.err("command", json!({"words": ["plan", "--job", id]})), BUSY);
    assert_eq!(ctl_spawns(&h).len(), 1);
}

#[test]
fn an_unknown_job_is_refused_with_no_job() {
    let h = stand_in(RELAY);
    let mut c = client(&h);
    assert_eq!(c.err("command", json!({"words": ["plan", "--job", "nosuch-1"]})), "no job nosuch-1");
    assert_eq!(c.err("command", json!({"words": ["decide", "--job", "nosuch-1", "--title", KEY]})), "no job nosuch-1");
    assert!(ctl_spawns(&h).is_empty());
}

#[test]
fn an_unknown_title_is_refused() {
    let (h, id) = filed();
    let mut c = client(&h);
    for key in ["t99-00000", ".."] {
        assert_eq!(c.err("command", json!({"words": ["decide", "--job", id, "--title", key]})),
                   format!("job {id} has no title {key}"));
    }
    assert!(ctl_spawns(&h).is_empty());
}

#[test]
fn words_without_job_or_jobs_are_refused() {
    let h = stand_in(RELAY);
    let mut c = client(&h);
    let forms: [&[&str]; 4] = [&[], &["plan", "some-slug"], &["status"], &["retune", "--set", "x=1"]];
    for words in forms {
        assert_eq!(c.err("command", json!({ "words": words })), FORMS, "{words:?}");
    }
    assert_eq!(c.err("command", json!({"words": ["plan", "--jobs"]})), "argument --jobs: only with retune");
    assert!(ctl_spawns(&h).is_empty());
}

#[test]
fn retune_jobs_is_refused_until_pull_request_5() {
    let h = stand_in(RELAY);
    let mut c = client(&h);
    assert_eq!(c.err("command", json!({"words": ["retune", "--jobs", "--spec", "spec.md"]})), RETUNE);
    assert!(ctl_spawns(&h).is_empty());
}

#[test]
fn an_engine_without_ctl_refuses_command() {
    let h = stand_in(&["movie-direct"]);
    assert!(!ctl_path(&h).exists(), "movie-direct holds no lazymkvctl.json, so no lazymkvctl is copied");
    let mut c = client(&h);
    assert_eq!(c.err("command", json!({"words": ["plan", "--job", "abc-1"]})), NO_CTL);
}

#[test]
fn a_ctl_in_a_writable_folder_is_refused() {
    let h = stand_in(RELAY);
    h.host().set_writable(h.tools());
    let mut c = client(&h);
    assert_eq!(c.err("command", json!({"words": ["plan", "--job", "abc-1"]})), WRITABLE);
    assert!(ctl_spawns(&h).is_empty());
}

/// Ruling T3-22-b's last refusal: the spawn's error.
#[test]
fn a_missing_lazymkvctl_is_refused_as_not_started() {
    let (h, id) = filed();
    fs::remove_file(ctl_path(&h)).unwrap();
    let mut c = client(&h);
    let refusal = c.err("command", json!({"words": ["plan", "--job", id]}));
    assert!(refusal.starts_with("lazymkvctl could not be started: "), "{refusal}");
}

/// S43: the engine adds no environment variable to a child.
#[test]
fn the_child_runs_below_normal() {
    let (h, id) = filed();
    let mut c = client(&h);
    let (_, reply) = c.command(&["plan", "--job", &id]);
    assert_eq!(reply, exit(0, 1));
    let spawns = ctl_spawns(&h);
    assert_eq!(spawns.len(), 1);
    assert_eq!(spawns[0].program, ctl_path(&h));
    assert!(spawns[0].below_normal);
    assert!(spawns[0].env.is_empty(), "{:?}", spawns[0].env);
}

/// Ruling T3-22-c: the stand-in sleeps until killed after its line, so the relay ends, and B's command runs, only once
/// A's leaving killed it.
#[test]
fn a_client_that_disconnects_kills_its_child() {
    let (h, id) = filed();
    let mut a = client(&h);
    a.send("command", json!({"words": ["plan", "--job", id, "--relay-hang"]}));
    assert_eq!(a.output(), pair("stdout", "relay hang line"));
    drop(a);
    let mut b = client(&h);
    let wait = WAIT * ci_factor();
    let deadline = Instant::now() + wait;
    loop {
        match b.command(&["plan", "--job", &id]).1 {
            Reply::Error { message, .. } if message == BUSY => {
                assert!(Instant::now() < deadline, "the command of the client that left still ran after {wait:?}");
                std::thread::sleep(Duration::from_millis(50));
            }
            Reply::Ok { result, .. } => {
                assert_eq!(result, json!({"exit_code": 0}));
                break;
            }
            other => panic!("unexpected {other:?}"),
        }
    }
}

/// Ruling T3-22-d: a `shutdown` that is not refused kills the running command and answers it `STOPPING`, after the
/// `shutdown`'s own `ok` (Ruling PF3-S21-22-4).
#[test]
fn a_shutdown_kills_the_running_command_and_answers_it_stopping() {
    let (h, id) = filed();
    let mut a = client(&h);
    let command = a.send("command", json!({"words": ["plan", "--job", id, "--relay-hang"]}));
    assert_eq!(a.output(), pair("stdout", "relay hang line"));
    let mut b = client(&h);
    assert_eq!(b.ok("shutdown", json!({"force": false})), Value::Null);
    match a.reply_to(command).1 {
        Reply::Error { message, .. } => assert_eq!(message, STOPPING),
        other => panic!("the command was answered {other:?}"),
    }
}

#[test]
fn ctl_plan_job_prints_the_childs_lines_and_exits_with_its_code() {
    let (h, id) = filed();
    let scratch = Scratch::new("ctl-relay");
    let ctx = Ctx::for_test(&scratch.0);
    let connector = h.client();
    let argv: Vec<String> = ["plan", "--job", id.as_str(), "--relay-exit-3"].map(str::to_string).into();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code =
        run(&argv, Env { ctx: &ctx, connector: Some(&connector), stdin: &mut &b""[..], out: &mut out, err: &mut err });
    let printed = (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap());
    assert_eq!(printed, (3, "relay plan line 1\nrelay plan line 2\n".to_string(), "relay plan warning\n".to_string()));
}
