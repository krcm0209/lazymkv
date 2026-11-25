//! Stand-in jobs compared through the relay's words (pull request 7's notes, calls 8 and 9). A job rips on the
//! stand-ins and files; its tuning copy lands; `plan --job` and `decide --job --title` are sent as a `command`, and the
//! stand-in lazymkvctl's spawned arguments (the relay's words) are run through the real `lazymkv-dev` to give the saved
//! outputs, which `compare-job` then compares with the job's own state.json and decide.json.

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{file_with_one_subtitle, first_kept_subtitle, hold_two_subs, subtitle_item_id, Client};
use lazymkv_core::ipc::Reply;
use lazymkv_core::record::TuningCopy;
use lazymkv_core::review_doc::{answers_of, ReviewDoc};
use lazymkv_dev::evidence::Ctx;
use lazymkv_engine::testkit::Harness;
use lazymkv_platform::Spawn;
use serde_json::{json, Value};

const KEY: &str = "t00-00800";

/// Pull request 7's notes, call 9: `--text` is for a title whose subtitle text was read.
const TEXT: &str = "--text";

/// The disc, the stand-in lazymkvctl and `variants` over the disc.
fn harness(variants: &[&str]) -> Harness {
    let mut scenarios = vec!["movie-direct"];
    scenarios.extend_from_slice(variants);
    scenarios.push("command-relay");
    let settings = json!({"keep_tuning_evidence": true, "drive_settle_s": 0, "drive_ready_timeout_s": 1});
    Harness::new(Path::new(env!("CARGO_BIN_EXE_fake-tool")), &scenarios, settings)
}

/// A harness whose movie-direct job stopped at `stop`, `Filed` or, as the two-stream job does, `NeedsReview` (its
/// stand-in file cannot be verified, so it is not approved), whose tuning copy is Done and whose engine was restarted,
/// so no step runs, with the job's id. A job that stops anywhere else fails the test (item G of the whole-branch
/// review), so a single-stream job that is held is caught.
fn filed_with_copy(variants: &[&str], stop: &str) -> (Harness, String) {
    settled(harness(variants), stop)
}

/// `filed_with_copy` over a harness the caller has set up.
fn settled(h: Harness, stop: &str) -> (Harness, String) {
    h.insert("tok0", "movie-direct");
    let id = format!("{}-1", h.fingerprint("movie-direct").to_ascii_lowercase());
    let stops = ["Filed", "NeedsReview", "Failed"];
    h.wait("the job stops", |h| stops.contains(&h.state_of(&id).as_str()));
    assert_eq!(h.state_of(&id), stop, "{:?}", h.record(&id).message);
    h.wait("the copy's state", |h| h.record(&id).tuning_copy == Some(TuningCopy::Done));
    h.restart();
    (h, id)
}

/// A client that has read its snapshot.
fn client(h: &Harness) -> Client {
    let mut c = Client::open(h);
    c.snapshot();
    c
}

/// The words of the stand-in lazymkvctl's last spawn.
fn last_words(h: &Harness) -> Vec<String> {
    let spawns: Vec<Spawn> =
        h.spawns().into_iter().filter(|spawn| spawn.program.file_stem().is_some_and(|stem| stem == "lazymkvctl")).collect();
    spawns.last().expect("a lazymkvctl spawn").args.clone()
}

/// Sends `words` as a `command`, runs the words the relay gave the stand-in through the real `lazymkv-dev`, requires
/// exit 0 and writes its output to a scratch file, which it returns with those words.
fn saved(h: &Harness, id: &str, words: &[&str], file: &str) -> (PathBuf, Vec<String>) {
    let mut c = client(h);
    let (_, reply) = c.command(words);
    assert_eq!(reply, Reply::Ok { id: Some(1), result: json!({"exit_code": 0}) });
    let spawned = last_words(h);
    let ctx = Ctx::for_test(&h.job_dir(id));
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = lazymkv_dev::dispatch(&ctx, &spawned, &mut out, &mut err);
    assert_eq!((code, String::from_utf8(err).unwrap()), (0, String::new()), "{spawned:?}");
    let path = h.data().join(file);
    fs::write(&path, out).unwrap();
    (path, spawned)
}

/// Runs `compare-job` over the job folder with the saved plan and the saved decide of KEY.
fn compared(h: &Harness, id: &str, plan: &Path, decide: &Path) -> (i32, String, String) {
    let ctx = Ctx::for_test(&h.job_dir(id));
    let job = h.job_dir(id).display().to_string();
    let pair = format!("{KEY}={}", decide.display());
    let argv: Vec<String> =
        ["compare-job", &job, "--plan", &plan.display().to_string(), "--decide", &pair].map(String::from).to_vec();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = lazymkv_dev::dispatch(&ctx, &argv, &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

/// One job's saved outputs through the relay's words: the plan's file, the decide's file and the decide's words.
fn outputs(h: &Harness, id: &str) -> (PathBuf, PathBuf, Vec<String>) {
    let (plan, _) = saved(h, id, &["plan", "--job", id], "saved-plan.json");
    let (decide, words) = saved(h, id, &["decide", "--job", id, "--title", KEY], "saved-decide.json");
    (plan, decide, words)
}

/// One English subtitle stream: step 10 does not run, so the relayed decide gets no `--text` and its output agrees with
/// the engine's own decide.json (pull request 7's notes, call 9).
#[test]
fn a_single_stream_job_compares_equal_through_the_relays_words() {
    let (h, id) = filed_with_copy(&[], "Filed");
    let (plan, decide, words) = outputs(&h, &id);
    assert_eq!(compared(&h, &id, &plan, &decide), (0, "compare-job: no difference\n".to_string(), String::new()));
    assert!(!words.iter().any(|word| word == TEXT), "{words:?}");
}

/// Two English subtitle streams: step 10 runs, so the relayed decide gets `--text <tuning root>/dataset` and its output
/// agrees with the engine's own decide.json (pull request 7's notes, call 9).
#[test]
fn a_two_stream_job_compares_equal_through_the_relays_words() {
    let (h, id) = filed_with_copy(&["movie-direct-two-subs"], "NeedsReview");
    let (plan, decide, words) = outputs(&h, &id);
    let at = words.iter().position(|word| word == TEXT).unwrap_or_else(|| panic!("no --text in {words:?}"));
    assert_eq!(words[at + 1], h.tuning().join("dataset").display().to_string());
    assert_eq!(compared(&h, &id, &plan, &decide), (0, "compare-job: no difference\n".to_string(), String::new()));
}

/// A saved plan whose title confidence was changed is a difference: exit 1 and one line about that value.
#[test]
fn a_changed_saved_value_is_a_difference() {
    let (h, id) = filed_with_copy(&[], "Filed");
    let (plan, decide, _) = outputs(&h, &id);
    let mut value: Value = serde_json::from_slice(&fs::read(&plan).unwrap()).unwrap();
    value["plan"]["title_confidence"] = json!(0.0);
    fs::write(&plan, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    let (code, out, err) = compared(&h, &id, &plan, &decide);
    assert_eq!((code, err.as_str()), (1, ""));
    // The whole output: the one line about that value, then the verdict (item F of the whole-branch review).
    assert_eq!(out, "plan: /title_confidence: 0.85 != 0.0\ncompare-job: 1 difference(s)\n");
}

/// A job planned with a lookup pending (the scenario of engine_file_phase.rs's
/// `a_lookup_pending_job_decides_with_the_refreshed_plan`): its plan was refreshed before the decision, so only the
/// plan's identity is compared, and the relayed outputs agree with it (pull request 7's notes, call 4; the final review
/// of pull request 7, CR-4).
#[test]
fn a_lookup_pending_job_compares_its_identity_only_through_the_relays_words() {
    let h = harness(&["movie-direct-lookup-late"]);
    h.scan_waits_for_lookups(false);
    let (h, id) = settled(h, "Filed");
    let (plan, decide, _) = outputs(&h, &id);
    let pending = "the job was planned with a lookup pending: only the plan's identity is compared (design part 12.7)";
    assert_eq!(
        compared(&h, &id, &plan, &decide),
        (0, format!("{pending}\ncompare-job: no difference\n"), String::new())
    );
}

/// A job held with two English subtitle streams, reviewed with an answer for one track (no collision answer: a replay
/// does not look at the library) and filed: its tuning copy lands and the engine restarts, so no step runs.
fn reviewed_with_a_track_answer() -> (Harness, String) {
    let h = harness(&["movie-direct-two-subs"]);
    let (id, target) = hold_two_subs(&h);
    let mut c = client(&h);
    let doc: ReviewDoc = serde_json::from_value(c.ok("review", json!({"job": id}))).unwrap();
    let item = subtitle_item_id(&doc, first_kept_subtitle(&h, &id));
    let answers = answers_of(&doc, &[(item, "drop".to_string())]).unwrap();
    assert_eq!(answers.overrides.subtitles.len(), 1);
    fs::remove_file(&target).unwrap();
    file_with_one_subtitle(&h, &id);
    let overrides = serde_json::to_value(&answers.overrides).unwrap();
    c.ok("review_action", json!({"job": id, "action": "approve", "overrides": overrides}));
    h.wait("Filed or Failed", |h| ["Filed", "Failed"].contains(&h.state_of(&id).as_str()));
    assert_eq!(h.state_of(&id), "Filed", "{:?}", h.record(&id).message);
    h.wait("the copy's state", |h| h.record(&id).tuning_copy == Some(TuningCopy::Done));
    drop(c);
    h.restart();
    (h, id)
}

/// A job filed after a review with a track answer: the relayed `decide --job` is run with the job's review.json as its
/// `--overrides`, so its output agrees with the decide.json the engine's own Decide wrote with that answer.
#[test]
fn a_reviewed_jobs_relayed_decide_equals_its_decide_json() {
    let (h, id) = reviewed_with_a_track_answer();
    let (plan, decide, words) = outputs(&h, &id);
    let at = words.iter().position(|word| word == "--overrides").unwrap_or_else(|| panic!("no --overrides in {words:?}"));
    assert_eq!(words[at + 1], h.job_dir(&id).join("review.json").display().to_string());
    assert!(fs::read_to_string(h.job_dir(&id).join("review.json")).unwrap().contains("\"drop\""));
    assert_eq!(compared(&h, &id, &plan, &decide), (0, "compare-job: no difference\n".to_string(), String::new()));
}
