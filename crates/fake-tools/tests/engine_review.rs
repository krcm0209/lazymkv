//! Whole jobs through a review, over the stand-ins (phase 3 design, parts 6.2 to 6.7): a disc with two English subtitle
//! streams is inserted and held at the disposition gate by a file at its final path; the owner's `review` document is
//! read from the engine, an answer is built from an item of that document, and the job is approved and filed.

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{first_kept_subtitle, hold_two_subs, remux_subtitle_tracks, subtitle_item_id, Client};
use lazymkv_core::job::{Event, Held, State};
use lazymkv_core::review::ReviewReason;
use lazymkv_core::review_doc::{answers_of, ItemKind, ReviewDoc};
use lazymkv_engine::testkit::Harness;
use serde_json::{json, Value};

const KEY: &str = "t00-00800";

/// The job.log line a Decide writes when it read its inputs back from the job folder (design part 5.10).
const READ_BACK: &str = "decide inputs read back from the job folder with lazymkv-dev's readers (design part 5.10)";

/// movie-direct with two English subtitle streams.
fn harness() -> Harness {
    let settings = json!({"keep_tuning_evidence": false, "drive_settle_s": 0, "drive_ready_timeout_s": 1});
    Harness::new(Path::new(env!("CARGO_BIN_EXE_fake-tool")), &["movie-direct", "movie-direct-two-subs"], settings)
}

/// A client that has read its snapshot.
fn client(h: &Harness) -> Client {
    let mut c = Client::open(h);
    c.snapshot();
    c
}

/// The engine's review document of `id`, read as the owner's tools read it.
fn document(c: &mut Client, id: &str) -> ReviewDoc {
    serde_json::from_value(c.ok("review", json!({"job": id}))).expect("the engine's document is a review document")
}

/// The `overrides` of an answer that drops the held job's first kept subtitle stream, built from that stream's item in
/// the engine's document with the word `drop` (the way `lazymkvctl review --answer` builds it).
fn drop_the_first_kept(h: &Harness, id: &str, doc: &ReviewDoc) -> Value {
    let item = subtitle_item_id(doc, first_kept_subtitle(h, id));
    let answers = answers_of(doc, &[(item, "drop".to_string())]).unwrap_or_else(|message| panic!("{message}"));
    assert_eq!(answers.overrides.subtitles.len(), 1, "{:?}", answers.overrides);
    assert!(answers.overrides.collision.is_empty() && answers.overrides.audio.is_empty());
    serde_json::to_value(&answers.overrides).unwrap()
}

/// Waits until `id` is Filed or Failed; the test fails, with the job's message, unless it is Filed.
fn filed(h: &Harness, id: &str) {
    h.wait("Filed or Failed", |h| ["Filed", "Failed"].contains(&h.state_of(id).as_str()));
    assert_eq!(h.state_of(id), "Filed", "{:?}", h.record(id).message);
}

fn events(h: &Harness, id: &str) -> Vec<Event> {
    h.transitions(id).into_iter().filter_map(|line| line.event).collect()
}

fn job_log(h: &Harness, id: &str) -> String {
    fs::read_to_string(h.job_dir(id).join("job.log")).unwrap()
}

/// The decision's subtitle part of the finished job: the kept streams and the dropped ones with their reasons.
fn decide_subtitles(h: &Harness, id: &str) -> Value {
    let text = fs::read_to_string(h.job_dir(id).join(KEY).join("decide.json")).unwrap();
    serde_json::from_str::<Value>(&text).unwrap()["decision"]["decision"]["subtitles"].clone()
}

/// The first answer dropped `stream` and the decision names the drop as the owner's.
fn assert_dropped_by_review(h: &Harness, id: &str, stream: u32) {
    let subtitles = decide_subtitles(h, id);
    assert_eq!(subtitles["kept"].as_array().unwrap().len(), 1, "{subtitles}");
    assert!(subtitles["dropped"].as_array().unwrap().contains(&json!([stream, {"why": "reviewed"}])), "{subtitles}");
}

/// The design's ending for this pull request: from the disc's insertion to Filed, through the engine's `review` and
/// `review_action`. The answer is built from an item of the engine's own document, and the stand-in mkvmerge is asked
/// to keep one subtitle track of the two.
#[test]
fn a_job_held_with_two_english_subtitle_tracks_is_reviewed_and_filed_with_one() {
    let h = harness();
    let (id, target) = hold_two_subs(&h);
    let mut c = client(&h);
    let doc = document(&mut c, &id);
    let subtitles = doc.titles.iter().flat_map(|title| &title.items).filter(|item| item.kind == ItemKind::Subtitle);
    assert_eq!(subtitles.count(), 2, "two subtitle items on the sheet: {doc:?}");
    let dropped = first_kept_subtitle(&h, &id);
    let overrides = drop_the_first_kept(&h, &id, &doc);
    fs::remove_file(&target).unwrap();
    common::file_with_one_subtitle(&h, &id);
    c.ok("review_action", json!({"job": id, "action": "approve", "overrides": overrides}));
    filed(&h, &id);
    assert_eq!(fs::read(&target).unwrap(), b"lazymkv-test-mkv");
    let record = h.record(&id);
    assert_eq!(record.titles[0].filed.as_ref().map(|filed| PathBuf::from(&filed.path)), Some(target));
    assert_eq!(record.titles[0].decision.as_ref().unwrap().subtitles.kept.len(), 1);
    assert_dropped_by_review(&h, &id, dropped);
    // The remux kept one subtitle track: a lone track ID, not a list.
    let kept = remux_subtitle_tracks(&h);
    assert_eq!(kept.len(), 1, "{kept:?}");
    assert!(!kept[0].is_empty() && !kept[0].contains(','), "one subtitle track kept: {kept:?}");
}

/// Review Focus 2: the engine stops after the approval and before its Decide persists, and starts again with the job in
/// Decided. The answer is read back from the job folder (review.json) and the job files with it; job.log holds the
/// line that says the Decide read its inputs back, which the Decide before the restart did not write.
#[test]
fn a_restart_between_the_approval_and_the_decision_keeps_the_answers() {
    let h = harness();
    let (id, target) = hold_two_subs(&h);
    let mut c = client(&h);
    let doc = document(&mut c, &id);
    let dropped = first_kept_subtitle(&h, &id);
    let overrides = drop_the_first_kept(&h, &id, &doc);
    fs::remove_file(&target).unwrap();
    common::file_with_one_subtitle(&h, &id);
    // The decision of the approved job and its failure cannot be persisted, so the job keeps the persisted state the
    // approval gave it, as it does when the engine stops there.
    let refused = id.clone();
    h.refuse_persists(move |record, transition| {
        record.id == refused
            && (matches!(transition.event, Some(Event::DecisionDone { .. })) || matches!(transition.new, State::Failed { .. }))
    });
    c.ok("review_action", json!({"job": id, "action": "approve", "overrides": overrides}));
    let kept = format!("{id}: keeps its persisted state Decided until a restart resumes it");
    h.wait("the approved job kept in Decided", |h| h.engine_log().contains(&kept));
    h.settle();
    assert!(matches!(h.record(&id).state, State::Decided { approved: Some(_) }), "{:?}", h.record(&id).state);
    assert!(h.job_dir(&id).join("review.json").is_file());
    assert!(!job_log(&h, &id).contains(READ_BACK), "the Decide before the restart kept its probes");
    drop(c);
    h.allow_persists();
    h.restart();
    filed(&h, &id);
    let restarted = h.transitions(&id).into_iter().find(|line| matches!(line.event, Some(Event::EngineRestart { .. })));
    let restarted = restarted.expect("an EngineRestart line");
    assert!(matches!(restarted.old, Some(State::Decided { approved: Some(_) })), "{:?}", restarted.old);
    assert!(job_log(&h, &id).contains(&format!("{KEY}: {READ_BACK}")), "{}", job_log(&h, &id));
    assert_eq!(fs::read(&target).unwrap(), b"lazymkv-test-mkv");
    assert_eq!(h.record(&id).titles[0].decision.as_ref().unwrap().subtitles.kept.len(), 1);
    assert_dropped_by_review(&h, &id, dropped);
    let kept = remux_subtitle_tracks(&h);
    assert_eq!(kept.len(), 1, "{kept:?}");
    assert!(!kept[0].is_empty() && !kept[0].contains(','), "one subtitle track kept: {kept:?}");
}

/// Review Focus 2: the first approval answers a track and the job is held again by a file that appeared at its final
/// path; the second approval answers only that collision, and the job files under the copy label with the first
/// review's track answer still in force.
#[test]
fn a_second_review_changes_one_track_and_keeps_the_other_answer() {
    let h = harness();
    let (id, target) = hold_two_subs(&h);
    let mut c = client(&h);
    let doc = document(&mut c, &id);
    let dropped = first_kept_subtitle(&h, &id);
    let first = drop_the_first_kept(&h, &id, &doc);
    fs::remove_file(&target).unwrap();
    common::file_with_one_subtitle(&h, &id);
    // The file comes back once the remux has made its partial file, before the rename.
    let late = target.clone();
    h.on_spawn(&["-J", ".partial"], move || {
        fs::create_dir_all(late.parent().unwrap()).unwrap();
        fs::write(&late, b"late").unwrap();
    });
    c.ok("review_action", json!({"job": id, "action": "approve", "overrides": first}));
    h.wait("FileCollided", |h| events(h, &id).iter().any(|event| matches!(event, Event::FileCollided { .. })));
    h.settle();
    let record = h.record(&id);
    assert_eq!(record.state, State::NeedsReview { held: Held::DispositionGate });
    assert!(record.titles[0].decision.as_ref().unwrap().reviews.contains(&ReviewReason::PathCollision));
    assert_eq!(fs::read(&target).unwrap(), b"late");
    // The second review answers the collision and nothing else.
    let doc = document(&mut c, &id);
    let collision = doc.titles.iter().flat_map(|title| &title.items).find(|item| item.kind == ItemKind::Collision);
    let collision = collision.unwrap_or_else(|| panic!("a collision item: {doc:?}"));
    let answers = answers_of(&doc, &[(collision.id.clone(), "keep_both".to_string())]).unwrap();
    assert!(answers.overrides.subtitles.is_empty() && answers.overrides.audio.is_empty(), "{:?}", answers.overrides);
    assert_eq!(answers.overrides.collision.len(), 1);
    let second = serde_json::to_value(&answers.overrides).unwrap();
    c.ok("review_action", json!({"job": id, "action": "approve", "overrides": second}));
    filed(&h, &id);
    let folder = "Synthetic Movie (2001) {tmdb-900001}";
    let copy = h.library().join(folder).join(format!("{folder} - 1080p 2.mkv"));
    assert_eq!(h.record(&id).titles[0].filed.as_ref().map(|filed| PathBuf::from(&filed.path)), Some(copy.clone()));
    assert_eq!(fs::read(&copy).unwrap(), b"lazymkv-test-mkv");
    assert_eq!(fs::read(&target).unwrap(), b"late");
    // review.json holds both approvals' answers, and the file filed keeps the track dropped by the first.
    let review: Value = serde_json::from_str(&fs::read_to_string(h.job_dir(&id).join("review.json")).unwrap()).unwrap();
    assert_eq!(review["overrides"]["subtitles"].as_array().unwrap().len(), 1, "{review}");
    assert_eq!(review["overrides"]["collision"].as_array().unwrap().len(), 1, "{review}");
    assert_eq!(review["reviews"].as_array().unwrap().len(), 2, "{review}");
    assert_dropped_by_review(&h, &id, dropped);
    let kept = remux_subtitle_tracks(&h);
    assert_eq!(kept.len(), 2, "one remux per approval: {kept:?}");
    for each in &kept {
        assert!(!each.is_empty() && !each.contains(','), "one subtitle track kept: {kept:?}");
    }
}
