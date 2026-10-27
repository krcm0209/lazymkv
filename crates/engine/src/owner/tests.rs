//! The job-owner loop over ScriptedSteps (design parts 4.4, 5.4, 7.3; K8, L1a, L2).

use super::*;
use crate::testkit::{Harness, StepScript};
use crate::StepResult;
use lazymkv_core::job::{Action, Event, Failure, FailureClass, NotifyEvent, State, WaitReason};
use lazymkv_core::plan::Mode;
use serde_json::json;

fn ok(event: Event) -> StepScript {
    StepScript::Result(StepResult { event: Some(event), message: None })
}

fn failure(class: FailureClass) -> Failure {
    Failure { class, output: None }
}

/// One title, from RunScan to Ejected.
fn disc_phase() -> Vec<(&'static str, StepScript)> {
    vec![
        ("RunScan", ok(Event::ScanDone)),
        ("Plan", ok(Event::PlanDone { mode: Mode::Direct })),
        ("CheckSpace", ok(Event::SpaceOk)),
        ("RipTitle", ok(Event::TitleRipped)),
        ("VerifyFile", ok(Event::TitleVerified { last: true })),
        ("Eject", ok(Event::Ejected)),
    ]
}

/// Ends a script that reaches Ejected: from Task 24 on, the file-phase slot runs Probe after Ejected, and this entry
/// fails the job there so that it releases the slot. Before Task 24 it is never used.
fn probe_fails() -> (&'static str, StepScript) {
    ("Probe", ok(Event::ProbeFailed { failure: failure(FailureClass::Unknown) }))
}

/// Whether a transition into `state` was persisted. The tests wait on this rather than on the current state, because
/// from Task 24 on a job moves past Ejected at once.
fn reached(h: &Harness, id: &str, state: &str) -> bool {
    h.transitions(id).iter().any(|line| line.new.as_str() == state)
}

fn id_of(h: &Harness, disc: &str) -> String {
    format!("{}-1", h.fingerprint(disc).to_ascii_lowercase())
}

fn arrival(drive: &str, fingerprint: &str) -> Arrival {
    let meta = DiscMeta {
        volume_label: "X".into(), format: Some("bluray".into()), fingerprint: Some(fingerprint.into()),
        disc_id: None, file_count: 1, total_bytes: 1, captured_unix: 1,
    };
    Arrival { drive: drive.into(), root: PathBuf::from("/x"), number: 1, meta, label: "X".into() }
}

#[test]
fn a_new_disc_persists_detected_then_fingerprinted() {
    let h = Harness::new_scripted(vec![("RunScan", StepScript::Hang)], json!({}));
    h.insert("tok0", "disc-a");
    let id = id_of(&h, "disc-a");
    h.wait("RunScan called", |h| h.steps().calls() == ["RunScan"]);
    let lines = h.transitions(&id);
    assert_eq!(lines.len(), 2);
    assert_eq!((lines[0].event.clone(), lines[0].old.clone()), (None, None));
    assert_eq!((lines[0].new.clone(), lines[0].actions.clone()), (State::Detected, vec![Action::Fingerprint]));
    assert_eq!(lines[1].event, Some(Event::FingerprintDone));
    assert_eq!(lines[1].old, Some(State::Detected));
    assert_eq!((lines[1].new.clone(), lines[1].actions.clone()), (State::Fingerprinted, vec![Action::RunScan]));
    let record = h.record(&id);
    assert_eq!((record.format.as_str(), record.drive.as_str(), record.profile.as_str()), ("lazymkv job v1", "tok0", "job.mmcp.xml"));
    assert_eq!(record.fingerprint, h.fingerprint("disc-a"));
    for file in ["meta.json", "files.json", "nav/BDMV/index.bdmv", "settings.json", "state.json", "transitions.jsonl"] {
        assert!(h.job_dir(&id).join(file).is_file(), "{file}");
    }
}

#[test]
fn already_ripped_sends_already_ripped_and_ejects_outside_the_reducer() {
    let h = Harness::new_scripted(disc_phase(), json!({}));
    h.insert("tok0", "disc-a");
    let id = id_of(&h, "disc-a");
    h.wait("Ejected", |h| reached(h, &id, "Ejected"));
    h.remove("tok0");
    h.settle();
    let lines = h.transitions(&id).len();
    h.insert("tok0", "disc-a");
    h.wait("already_ripped", |h| h.notifications().iter().any(|n| n.event == NotifyEvent::AlreadyRipped));
    h.settle();
    let sent = h.notifications();
    let events: Vec<NotifyEvent> = sent.iter().map(|n| n.event.clone()).collect();
    assert_eq!(events, vec![NotifyEvent::DiscDone, NotifyEvent::AlreadyRipped]);
    let done = &sent[0];
    assert_eq!((done.job_id.as_deref(), done.drive.as_deref()), (Some(id.as_str()), Some("tok0")));
    assert_eq!(done.fingerprint.as_deref(), Some(h.fingerprint("disc-a").as_str()));
    assert_eq!(done.message, "the disc phase is done and the disc was ejected");
    assert_eq!(sent[1].message, format!("the disc was already ripped as job {id} and was ejected"));
    assert_eq!(h.ejector().calls(), vec!["tok0".to_string()]);
    assert_eq!(h.transitions(&id).len(), lines);
    assert_eq!(h.job_ids(), vec![id]);
    assert_eq!(h.drives_json()["drives"]["tok0"], json!({"fingerprint": null, "handled": false}));
}

#[test]
fn a_second_drives_disc_queues_and_starts_at_the_first_jobs_ejected() {
    let mut token = DiscToken::default();
    assert!(token.take("a-1"));
    assert!(!token.take("b-1"));
    token.wait(Waiting::Arrival(arrival("tok1", "B")));
    assert!(token.after("a-1", &State::Ripping, &[Action::VerifyFile]).is_none());
    assert!(token.after("a-1", &State::RipVerified { after_copy: false }, &[Action::Eject]).is_none());
    let next = token.after("a-1", &State::Ejected { after_copy: false }, &[Action::Notify { event: NotifyEvent::DiscDone }]);
    let Some(Waiting::Arrival(found)) = next else { panic!("the queued arrival was not taken at Ejected") };
    assert_eq!(found.drive, "tok1");
    assert_eq!(token.holder(), None);
    assert!(!keeps_token(&State::Ripping, &[Action::WaitForDisc]));
    assert!(!keeps_token(&State::AwaitingSpace { return_to: Box::new(State::Planned { mode: Mode::Direct }) }, &[]));
}

#[test]
fn a_queued_disc_that_is_removed_leaves_the_queue() {
    let h = Harness::new_scripted(vec![("RunScan", StepScript::Hang)], json!({}));
    h.insert("tok0", "disc-a");
    let a = id_of(&h, "disc-a");
    h.wait("disc-a scanning", |h| h.steps().calls() == ["RunScan"]);
    h.insert("tok1", "disc-b");
    let b = h.fingerprint("disc-b");
    h.wait("disc-b queued", |h| h.drives_json()["drives"]["tok1"]["fingerprint"] == json!(b));
    assert_eq!(h.drives_json()["drives"]["tok1"]["handled"], json!(false));
    h.remove("tok1");
    h.wait("tok1 empty", |h| h.drives_json()["drives"]["tok1"]["fingerprint"] == json!(null));
    h.remove("tok0");
    h.settle();
    assert_eq!(h.job_ids(), vec![a]);
    assert_eq!(h.steps().calls(), ["RunScan"]);
}

#[test]
fn a_disc_pulled_mid_rip_waits_for_its_disc_and_frees_the_token() {
    let mut script = disc_phase();
    script[3] = ("RipTitle", StepScript::Hang);
    script.push(("RunScan", StepScript::Hang));
    let h = Harness::new_scripted(script, json!({}));
    h.insert("tok0", "disc-a");
    let a = id_of(&h, "disc-a");
    h.wait("ripping", |h| h.steps().calls().last().map(String::as_str) == Some("RipTitle"));
    h.remove("tok0");
    h.wait("MediaRemoved", |h| h.transitions(&a).last().unwrap().event == Some(Event::MediaRemoved));
    let last = h.transitions(&a).pop().unwrap();
    assert_eq!((last.new, last.actions), (State::Ripping, vec![Action::WaitForDisc]));
    h.insert("tok0", "disc-b");
    h.wait("disc-b scanning", |h| h.steps().calls().len() == 5);
    assert_eq!(h.steps().calls(), ["RunScan", "Plan", "CheckSpace", "RipTitle", "RunScan"]);
    assert_eq!(h.job_ids().len(), 2);
}

#[test]
fn reinserting_routes_media_arrived_and_rips_again() {
    let mut script = disc_phase();
    script.insert(3, ("RipTitle", StepScript::Hang));
    script.push(probe_fails());
    let h = Harness::new_scripted(script, json!({}));
    h.insert("tok0", "disc-a");
    let a = id_of(&h, "disc-a");
    h.wait("ripping", |h| h.steps().calls().last().map(String::as_str) == Some("RipTitle"));
    h.remove("tok0");
    h.wait("waiting for the disc", |h| h.transitions(&a).last().unwrap().event == Some(Event::MediaRemoved));
    h.insert("tok0", "disc-a");
    h.wait("Ejected", |h| reached(h, &a, "Ejected"));
    let arrived = h.transitions(&a).into_iter().find(|line| line.event == Some(Event::MediaArrived)).unwrap();
    assert_eq!((arrived.new, arrived.actions), (State::Ripping, vec![Action::EmptyTitleDirectory, Action::RipTitle]));
    assert_eq!(
        h.steps().calls()[..8],
        ["RunScan", "Plan", "CheckSpace", "RipTitle", "EmptyTitleDirectory", "RipTitle", "VerifyFile", "Eject"]
    );
    assert_eq!(h.job_ids(), vec![a]);
}

#[test]
fn a_waiting_jobs_disc_is_only_noted_and_takes_no_token() {
    let script = vec![
        ("RunScan", ok(Event::ScanDone)),
        ("Plan", ok(Event::PlanDone { mode: Mode::Direct })),
        ("CheckSpace", ok(Event::SpaceShort)),
        ("RunScan", StepScript::Hang),
    ];
    let h = Harness::new_scripted(script, json!({}));
    h.insert("tok0", "disc-a");
    let a = id_of(&h, "disc-a");
    h.wait("AwaitingSpace", |h| h.state_of(&a) == "AwaitingSpace");
    h.remove("tok0");
    h.settle();
    h.insert("tok0", "disc-a");
    h.settle();
    let lines = h.transitions(&a);
    assert!(lines.iter().all(|line| line.event != Some(Event::MediaRemoved) && line.event != Some(Event::MediaArrived)));
    assert_eq!(h.drives_json()["drives"]["tok0"]["handled"], json!(true));
    h.insert("tok1", "disc-b");
    h.wait("disc-b scanning", |h| h.steps().calls().len() == 4);
    assert_eq!(h.transitions(&a).len(), lines.len());
}

#[test]
fn a_media_read_error_jobs_reinserted_disc_gets_media_arrived() {
    let script = vec![
        ("RunScan", ok(Event::ScanDone)),
        ("Plan", ok(Event::PlanDone { mode: Mode::Direct })),
        ("CheckSpace", ok(Event::SpaceOk)),
        ("RipTitle", ok(Event::TitleRipFailed { failure: failure(FailureClass::MediaReadError) })),
        ("RipTitle", StepScript::Hang),
    ];
    let h = Harness::new_scripted(script, json!({}));
    h.insert("tok0", "disc-a");
    let a = id_of(&h, "disc-a");
    h.wait("Failed", |h| h.state_of(&a) == "Failed");
    assert_eq!(h.record(&a).state, State::Failed { class: FailureClass::MediaReadError, from: Box::new(State::Ripping) });
    h.remove("tok0");
    h.insert("tok1", "disc-a");
    h.wait("MediaArrived", |h| h.transitions(&a).last().unwrap().event == Some(Event::MediaArrived));
    let last = h.transitions(&a).pop().unwrap();
    assert_eq!(last.new, State::Ripping);
    assert_eq!(
        last.actions,
        vec![Action::EmptyTitleDirectory, Action::RipTitle, Action::Notify { event: NotifyEvent::Resumed }]
    );
    assert_eq!(h.record(&a).drive, "tok1");
}

#[test]
fn drives_json_records_fingerprint_and_handled_at_arrival_removal_and_queue_change() {
    let h = Harness::new_scripted(vec![("RunScan", StepScript::Hang), ("RunScan", StepScript::Hang)], json!({}));
    let (fa, fb) = (h.fingerprint("disc-a"), h.fingerprint("disc-b"));
    h.insert("tok0", "disc-a");
    h.wait("a routed", |h| h.drives_json()["drives"]["tok0"] == json!({"fingerprint": fa, "handled": true}));
    h.insert("tok1", "disc-b");
    h.wait("b queued", |h| h.drives_json()["drives"]["tok1"] == json!({"fingerprint": fb, "handled": false}));
    h.remove("tok0");
    h.wait("b taken", |h| h.drives_json()["drives"]["tok1"] == json!({"fingerprint": fb, "handled": true}));
    let drives = h.drives_json();
    assert_eq!(drives["format"], json!("lazymkv drives v1"));
    assert_eq!(drives["drives"]["tok0"], json!({"fingerprint": null, "handled": false}));
    assert_eq!(drives["drive_incapable"], json!([]));
    h.remove("tok1");
    h.wait("tok1 empty", |h| h.drives_json()["drives"]["tok1"] == json!({"fingerprint": null, "handled": false}));
}

#[test]
fn an_unreadable_volume_sends_failed_with_a_null_job_id() {
    let h = Harness::new_scripted(vec![], json!({}));
    let root = h.data().parent().unwrap().join("volumes/blank");
    std::fs::create_dir_all(root.join("BDMV")).unwrap();
    std::fs::write(root.join("BDMV/index.bdmv"), b"INDX0200").unwrap();
    h.insert_root("tok0", &root);
    h.wait("failed", |h| !h.notifications().is_empty());
    h.settle();
    let sent = h.notifications();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].event, NotifyEvent::Failed { class: FailureClass::Unknown });
    assert_eq!((sent[0].job_id.clone(), sent[0].fingerprint.clone()), (None, None));
    assert_eq!(sent[0].drive.as_deref(), Some("tok0"));
    assert!(h.engine_log().contains(
        "tok0: the disc's fingerprint could not be computed: the volume holds no BDMV/STREAM or VIDEO_TS files"));
    assert_eq!(std::fs::read_dir(h.data().join("jobs")).map(|d| d.count()).unwrap_or(0), 0);
    assert!(h.ejector().calls().is_empty(), "the disc stays in (design part 5.4 step 2)");
}

/// Review Focus 5. A regular file stands for the volume root, so the walk fails as it does when the disc is pulled
/// mid-walk; the removal that follows finds no arrival left to cancel, and nothing else is sent.
#[test]
fn a_disc_removed_during_its_fingerprint_walk_sends_one_failed_with_a_null_job_id_and_makes_no_job() {
    let h = Harness::new_scripted(vec![], json!({}));
    let pulled = h.data().parent().unwrap().join("volumes/pulled");
    std::fs::create_dir_all(pulled.parent().unwrap()).unwrap();
    std::fs::write(&pulled, b"the volume is gone").unwrap();
    h.insert_root("tok0", &pulled);
    h.remove("tok0");
    h.wait("failed", |h| !h.notifications().is_empty());
    h.settle();
    let sent = h.notifications();
    assert_eq!(sent.len(), 1);
    assert_eq!((sent[0].event.clone(), sent[0].job_id.clone()), (NotifyEvent::Failed { class: FailureClass::Unknown }, None));
    assert_eq!(h.engine_log().matches("tok0: the disc's fingerprint could not be computed: ").count(), 1);
    assert_eq!(std::fs::read_dir(h.data().join("jobs")).map(|d| d.count()).unwrap_or(0), 0);
    assert_eq!(h.drives_json()["drives"]["tok0"], json!({"fingerprint": null, "handled": false}));
    assert!(h.ejector().calls().is_empty(), "the disc stays in (design part 5.4 step 2)");
}

#[test]
fn a_volume_that_never_appears_fails_after_drive_ready_timeout_s() {
    let h = Harness::new_scripted(vec![], json!({}));
    let missing = h.data().parent().unwrap().join("volumes/never-mounted");
    h.insert_root("tok0", &missing);
    h.wait("failed", |h| !h.notifications().is_empty());
    h.settle();
    let sent = h.notifications();
    assert_eq!(sent.len(), 1);
    assert_eq!((sent[0].event.clone(), sent[0].job_id.clone()), (NotifyEvent::Failed { class: FailureClass::Unknown }, None));
    assert_eq!(h.engine_log().matches("tok0: the volume did not appear within drive_ready_timeout_s (1 s)").count(), 1);
    assert_eq!(std::fs::read_dir(h.data().join("jobs")).map(|d| d.count()).unwrap_or(0), 0);
    assert!(h.ejector().calls().is_empty());
}

#[test]
fn no_job_starts_while_settings_have_a_problem_or_staging_root_is_unset() {
    let h = Harness::new_scripted(vec![], json!({"staging_root": null}));
    h.insert("tok0", "disc-a");
    h.wait("refused", |h| h.engine_log().contains("tok0: no job starts: staging_root is unset"));
    assert!(h.job_ids().is_empty());
    let h = Harness::new_scripted(vec![], json!({"keep_tuning_evidence": true, "tuning_root": null}));
    h.insert("tok0", "disc-a");
    h.wait("refused", |h| h.engine_log().contains("tok0: no job starts: the settings have a problem: tuning_root"));
    assert!(h.job_ids().is_empty());
    assert_eq!(h.drives_json()["drives"]["tok0"]["handled"], json!(false));
}

#[test]
fn keep_awake_is_held_while_any_job_is_active() {
    let mut script = disc_phase();
    script.insert(3, ("RipTitle", StepScript::Hang));
    script.push(probe_fails());
    let h = Harness::new_scripted(script, json!({}));
    h.insert("tok0", "disc-a");
    let a = id_of(&h, "disc-a");
    h.wait("ripping", |h| h.steps().calls().last().map(String::as_str) == Some("RipTitle"));
    assert_eq!(h.awake().holders(), 1);
    h.remove("tok0");
    h.wait("waiting for the disc", |h| h.transitions(&a).last().unwrap().event == Some(Event::MediaRemoved));
    h.settle();
    assert_eq!(h.awake().holders(), 0);
    h.insert("tok0", "disc-a");
    h.wait("Ejected", |h| reached(h, &a, "Ejected"));
    h.settle();
    assert_eq!(h.awake().holders(), 0);
}

#[test]
fn each_transition_is_persisted_before_its_actions_run() {
    let mut script = disc_phase();
    script.push(probe_fails());
    let h = Harness::new_scripted(script, json!({}));
    h.insert("tok0", "disc-a");
    let a = id_of(&h, "disc-a");
    h.wait("Ejected", |h| reached(h, &a, "Ejected"));
    h.settle();
    let lines = h.transitions(&a);
    let seen = h.steps().seen();
    let first: Vec<&str> = seen.iter().take(6).map(|call| call.action.as_str()).collect();
    assert_eq!(first, ["RunScan", "Plan", "CheckSpace", "RipTitle", "VerifyFile", "Eject"]);
    for call in seen {
        let line = &lines[call.lines - 1];
        assert_eq!(call.state, line.new.as_str(), "{}", call.action);
        let tags: Vec<String> = line.actions.iter().map(|a| serde_json::to_value(a).unwrap()["action"].as_str().unwrap().to_string()).collect();
        assert!(tags.contains(&call.action), "{} not in the persisted line's actions {tags:?}", call.action);
    }
}

#[test]
fn posted_edits_and_events_are_taken_once_in_order() {
    let h = Harness::new_scripted(disc_phase(), json!({}));
    h.insert("tok0", "disc-a");
    h.run_until("Planned");
    let mut record = h.record(&id_of(&h, "disc-a"));
    let outbox = Outbox::default();
    outbox.edit("a-1", |record| record.message = Some("one".into()));
    outbox.edit("a-1", |record| record.message = Some(format!("{} then two", record.message.take().unwrap_or_default())));
    let java = Event::ToolMissing { tool: lazymkv_core::job::Tool::Java };
    outbox.feed_first("a-1", java.clone());
    outbox.feed_first("a-1", Event::ShareLost);
    let posted = outbox.take("a-1");
    assert_eq!(posted.first, vec![java, Event::ShareLost]);
    for edit in posted.edits {
        edit(&mut record);
    }
    assert_eq!(record.message.as_deref(), Some("one then two"));
    let again = outbox.take("a-1");
    assert!(again.edits.is_empty() && again.first.is_empty());
}

#[test]
fn run_until_stops_before_the_states_actions_run() {
    let h = Harness::new_scripted(disc_phase(), json!({}));
    h.insert("tok0", "disc-a");
    h.run_until("Planned");
    let a = id_of(&h, "disc-a");
    let last = h.transitions(&a).pop().unwrap();
    assert_eq!((last.new, last.actions), (State::Planned { mode: Mode::Direct }, vec![Action::CheckSpace]));
    assert_eq!(h.record(&a).state, State::Planned { mode: Mode::Direct });
    assert_eq!(h.steps().calls(), ["RunScan", "Plan"]);
    h.settle();
    assert_eq!(h.steps().calls(), ["RunScan", "Plan"], "the stopped engine ran CheckSpace");
}

#[test]
fn restart_keeps_the_data_folder_and_the_drive_script() {
    let mut script = disc_phase();
    script[3] = ("RipTitle", StepScript::Hang);
    let h = Harness::new_scripted(script, json!({}));
    h.insert("tok0", "disc-a");
    h.run_until("Planned");
    let a = id_of(&h, "disc-a");
    let before = h.transitions(&a);
    h.restart();
    h.settle();
    assert_eq!(h.job_ids(), vec![a.clone()]);
    assert_eq!(h.transitions(&a)[..before.len()], before[..]);
    assert_eq!(h.drives_json()["drives"]["tok0"]["fingerprint"], json!(h.fingerprint("disc-a")));
    assert_eq!(h.steps().calls()[..2], ["RunScan", "Plan"]);
}

#[test]
fn the_engine_logs_its_start_and_stop() {
    let h = Harness::new_scripted(vec![], json!({}));
    let started = format!("engine started (lazymkv {})", env!("CARGO_PKG_VERSION"));
    h.wait("started", |h| h.engine_log().contains(&started));
    h.restart();
    h.wait("started again", |h| h.engine_log().matches(&started).count() == 2);
    assert_eq!(h.engine_log().matches("engine stopped").count(), 1);
}

/// A step that panics fails its job as Unknown with the panic's text (design part 5.2; ruling at the final review),
/// writes its job.log and engine.log lines and gives the disc-phase token back, so the engine still stops and a later
/// disc still starts.
#[test]
fn a_panicking_step_fails_its_job_and_gives_the_token_back() {
    let script = vec![("RunScan", StepScript::Panic("scripted panic")), ("RunScan", StepScript::Hang)];
    let h = Harness::new_scripted(script, json!({}));
    h.insert("tok0", "disc-a");
    let a = id_of(&h, "disc-a");
    h.wait("Failed", |h| h.state_of(&a) == "Failed");
    let record = h.record(&a);
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Fingerprinted) });
    assert_eq!(record.message.as_deref(), Some("the step panicked: scripted panic"));
    assert!(matches!(h.transitions(&a).pop().unwrap().event, Some(Event::ScanFailed { .. })));
    let job_log = std::fs::read_to_string(h.job_dir(&a).join("job.log")).unwrap_or_default();
    assert!(job_log.contains("RunScan returned no event: the step panicked: scripted panic"), "{job_log}");
    assert!(h.engine_log().contains(&format!("{a}: the step panicked: scripted panic")), "{}", h.engine_log());
    h.wait("the failed notification", |h| {
        h.notifications().iter().any(|n| n.event == NotifyEvent::Failed { class: FailureClass::Unknown })
    });
    h.settle();
    assert_eq!(h.awake().holders(), 0);
    h.insert("tok1", "disc-b");
    h.wait("disc-b scanning", |h| h.steps().calls() == ["RunScan", "RunScan"]);
    h.restart();
    h.settle();
    assert_eq!(h.job_ids().len(), 2);
}

/// Design part 5.3 ("After any kill the engine waits 10 s before the next `makemkvcon` call"; ruling at the final
/// review): a disc pulled mid-rip has its rip killed, and the token passes to the next disc only once that stale step
/// has reported, after its pause, so the next disc's first `makemkvcon` never starts beside the killed one.
#[test]
fn a_pulled_discs_killed_step_holds_the_token_until_it_reports() {
    let pause = Duration::from_secs(3);
    let mut script = disc_phase();
    script[3] = ("RipTitle", StepScript::After(pause, StepResult { event: Some(Event::TitleRipped), message: None }));
    script.push(("RunScan", StepScript::Hang));
    let h = Harness::new_scripted(script, json!({}));
    h.insert("tok0", "disc-a");
    let a = id_of(&h, "disc-a");
    h.wait("ripping", |h| h.steps().calls().last().map(String::as_str) == Some("RipTitle"));
    let ripping = Instant::now();
    h.insert("tok1", "disc-b");
    let b = h.fingerprint("disc-b");
    h.wait("disc-b queued", |h| h.drives_json()["drives"]["tok1"]["fingerprint"] == json!(b));
    h.remove("tok0");
    h.wait("MediaRemoved", |h| h.transitions(&a).last().unwrap().event == Some(Event::MediaRemoved));
    thread::sleep(Duration::from_millis(300));
    assert!(ripping.elapsed() < pause, "the test took too long to see the removal: {:?}", ripping.elapsed());
    assert_eq!(h.drives_json()["drives"]["tok1"]["handled"], json!(false), "disc-b was routed beside the killed rip");
    assert_eq!(h.steps().calls().len(), 4, "{:?}", h.steps().calls());
    h.wait("disc-b scanning", |h| h.steps().calls().len() == 5);
    assert!(ripping.elapsed() >= pause - Duration::from_millis(200), "disc-b started after {:?}", ripping.elapsed());
    assert_eq!(h.steps().calls(), ["RunScan", "Plan", "CheckSpace", "RipTitle", "RunScan"]);
    assert!(h.transitions(&a).iter().all(|line| line.event != Some(Event::TitleRipped)), "the stale result was fed");
}

/// The final review's re-review (N2): a step that returns no event gives the token back although a pulled disc's
/// stale step of the same job is still listed; only a step of the job's current generation keeps it.
#[test]
fn a_no_event_step_gives_the_token_back_beside_a_stale_step_of_its_job() {
    let planned = StepResult { event: Some(Event::PlanDone { mode: Mode::Direct }), message: None };
    let script = vec![
        ("RunScan", ok(Event::ScanDone)),
        ("Plan", StepScript::After(Duration::from_secs(3), planned)), // stale once disc-a is pulled
        ("RunScan", StepScript::Hang),                                  // disc-b
    ];
    let h = Harness::new_scripted(script, json!({}));
    h.insert("tok0", "disc-a");
    let a = id_of(&h, "disc-a");
    h.wait("planning", |h| h.steps().calls() == ["RunScan", "Plan"]);
    let planning = Instant::now();
    h.remove("tok0");
    h.wait("MediaRemoved", |h| h.transitions(&a).last().unwrap().event == Some(Event::MediaRemoved));
    h.insert("tok0", "disc-a");
    // The second Plan has no scripted result, so it returns no event while the first still runs.
    h.wait("planning again", |h| h.steps().calls() == ["RunScan", "Plan", "Plan"]);
    h.wait("the no-event line", |h| {
        std::fs::read_to_string(h.job_dir(&a).join("job.log")).unwrap_or_default().contains("Plan returned no event")
    });
    assert!(planning.elapsed() < Duration::from_secs(3), "the stale Plan ended first: {:?}", planning.elapsed());
    h.insert("tok1", "disc-b");
    h.wait("disc-b scanning", |h| h.steps().calls().len() == 4);
    assert_eq!(h.steps().calls(), ["RunScan", "Plan", "Plan", "RunScan"]);
}

/// R22.7: only a job that waits for its disc gets MediaArrived (design part 5.4 step 3), so a second arrival of a disc
/// that is already in a drive feeds nothing and starts no second rip.
#[test]
fn a_second_arrival_of_a_present_disc_feeds_nothing() {
    let mut script = disc_phase();
    script[3] = ("RipTitle", StepScript::Hang);
    let h = Harness::new_scripted(script, json!({}));
    h.insert("tok0", "disc-a");
    let a = id_of(&h, "disc-a");
    h.wait("ripping", |h| h.steps().calls().last().map(String::as_str) == Some("RipTitle"));
    h.insert("tok1", "disc-a");
    h.wait("tok1 handled", |h| h.drives_json()["drives"]["tok1"]["handled"] == json!(true));
    h.settle();
    assert_eq!(h.steps().calls().iter().filter(|call| *call == "RipTitle").count(), 1, "{:?}", h.steps().calls());
    assert!(h.transitions(&a).iter().all(|line| line.event != Some(Event::MediaArrived)));
    assert_eq!(h.drives_json()["drives"]["tok1"], json!({"fingerprint": h.fingerprint("disc-a"), "handled": true}));
    assert!(h.engine_log().contains(&format!("tok1: the disc of job {a} is already in tok0")), "{}", h.engine_log());
    assert_eq!(h.record(&a).drive, "tok0");
}

/// R29.c2a: a transition is persisted before its actions run, so a StartTimer timer can start just after a test sees
/// the state; `fire_timers` waits for it instead of firing nothing.
#[test]
fn fire_timers_waits_for_a_timer_that_starts_after_it_is_called() {
    use crate::Timers;
    let h = Harness::new_scripted(Vec::new(), json!({}));
    let fired = Arc::new(AtomicBool::new(false));
    let timers = h.timers();
    thread::scope(|scope| {
        let fired = Arc::clone(&fired);
        scope.spawn(move || {
            thread::sleep(Duration::from_millis(200));
            timers.start(60, Box::new(move || fired.store(true, Ordering::SeqCst)));
        });
        h.fire_timers();
    });
    assert!(fired.load(Ordering::SeqCst));
    assert!(h.timers().pending().is_empty());
}

/// R29.c2a: with no timer to fire, `fire_timers` says so instead of returning as if it had fired one.
#[test]
#[should_panic(expected = "fire_timers: no timer became pending within")]
fn fire_timers_panics_when_no_timer_becomes_pending() {
    let h = Harness::new_scripted(Vec::new(), json!({}));
    h.fire_timers();
}

/// Review round 1 (V11, RR1-12): a CheckSpace that panics in Planned, whose state takes no failure row, still feeds
/// ShareLost: the job waits for the share with the panic's text and gives the disc-phase token back.
#[test]
fn a_panicking_space_check_before_the_rip_waits_for_the_share_and_gives_the_token_back() {
    let script = vec![
        ("RunScan", ok(Event::ScanDone)),
        ("Plan", ok(Event::PlanDone { mode: Mode::Direct })),
        ("CheckSpace", StepScript::Panic("scripted space check panic")),
        ("RunScan", StepScript::Hang),
    ];
    let h = Harness::new_scripted(script, json!({}));
    h.insert("tok0", "disc-a");
    let a = id_of(&h, "disc-a");
    h.wait("AwaitingShare", |h| h.state_of(&a) == "AwaitingShare");
    let record = h.record(&a);
    assert_eq!(record.state, State::AwaitingShare { return_to: Box::new(State::Planned { mode: Mode::Direct }) });
    assert_eq!(record.message.as_deref(), Some("the step panicked: scripted space check panic"));
    let job_log = std::fs::read_to_string(h.job_dir(&a).join("job.log")).unwrap_or_default();
    let no_event = "CheckSpace returned no event: the step panicked: scripted space check panic";
    assert!(job_log.contains(no_event), "{job_log}");
    let share = NotifyEvent::Waiting { reason: WaitReason::Share };
    h.wait("the waiting notification", |h| h.notifications().iter().any(|n| n.event == share));
    let waiting = h.notifications().into_iter().find(|n| n.event == share).unwrap();
    assert_eq!(waiting.message, "waiting: share: the step panicked: scripted space check panic");
    h.insert("tok1", "disc-b");
    h.wait("disc-b scanning", |h| h.steps().calls().len() == 4);
    assert_eq!(h.steps().calls(), ["RunScan", "Plan", "CheckSpace", "RunScan"]);
}

/// Disc-a scanned in tok0 and waiting in AwaitingNetwork, then pulled and inserted in tok1, where the waiting job only
/// has it noted (design part 5.4 step 3): no MediaRemoved and no MediaArrived is fed.
fn moved_while_waiting() -> (Harness, String) {
    let script = vec![("RunScan", ok(Event::ScanDone)), ("Plan", ok(Event::NetworkNeeded)), ("Plan", StepScript::Hang)];
    let h = Harness::new_scripted(script, json!({}));
    h.insert("tok0", "disc-a");
    let a = id_of(&h, "disc-a");
    h.wait("AwaitingNetwork", |h| h.state_of(&a) == "AwaitingNetwork");
    h.remove("tok0");
    h.wait("tok0 emptied", |h| h.drives_json()["drives"]["tok0"]["fingerprint"].is_null());
    let fingerprint = h.fingerprint("disc-a");
    h.insert("tok1", "disc-a");
    h.wait("tok1 noted", |h| h.drives_json()["drives"]["tok1"] == json!({"fingerprint": fingerprint, "handled": true}));
    let lines = h.transitions(&a);
    let disc_event = |event: &Option<Event>| matches!(event, Some(Event::MediaRemoved | Event::MediaArrived));
    assert!(!lines.iter().any(|line| disc_event(&line.event)), "{lines:?}");
    (h, a)
}

/// The persisted line of `id`'s first transition with `event`, once there is one.
fn line_of(h: &Harness, id: &str, event: &Event) -> Transition {
    h.wait("the transition", |h| h.transitions(id).iter().any(|line| line.event.as_ref() == Some(event)));
    h.transitions(id).into_iter().find(|line| line.event.as_ref() == Some(event)).unwrap()
}

/// Review round 1 (V2): a waiting job whose disc is noted in another drive takes that drive as its record's drive, so
/// its return lists, scans and ejects the drive the disc is in.
#[test]
fn a_waiting_jobs_disc_moved_to_another_drive_becomes_its_records_drive() {
    let (h, a) = moved_while_waiting();
    h.fire_timers();
    let back = line_of(&h, &a, &Event::NetworkRetry);
    let resumed = vec![Action::Plan, Action::Notify { event: NotifyEvent::Resumed }];
    assert_eq!((back.new, back.actions), (State::Scanned, resumed));
    assert_eq!(h.record(&a).drive, "tok1");
    h.wait("planning again", |h| h.steps().calls() == ["RunScan", "Plan", "Plan"]);
}

/// Review round 1 (V2): at a restart, a waiting job that returns to the disc phase takes the drive its disc is in as
/// its record's drive, written with its EngineRestart line, although the drive is noted rather than an arrival.
#[test]
fn a_restart_makes_a_waiting_jobs_moved_disc_drive_its_records_drive() {
    let (h, a) = moved_while_waiting();
    h.restart();
    let restarted = line_of(&h, &a, &Event::EngineRestart { disc_present: true });
    assert_eq!(restarted.new, State::AwaitingNetwork { return_to: Box::new(State::Scanned) });
    assert_eq!(h.record(&a).drive, "tok1");
    h.fire_timers();
    let back = line_of(&h, &a, &Event::NetworkRetry);
    assert_eq!(back.new, State::Scanned);
    assert_eq!(h.record(&a).drive, "tok1");
}

/// Sends one request over a new IPC connection and returns its reply, skipping the snapshot and pushes before it. The
/// lines are read on a helper thread and each is waited for 20 s, times `ci_factor` (R29.c2, R29.c5; the final review
/// of review round 1, C4), so an engine that never answers fails the test instead of hanging the binary.
fn request(h: &Harness, kind: &str, payload: serde_json::Value) -> serde_json::Value {
    use lazymkv_platform::IpcConnector;
    use std::io::{BufRead, Write};
    use std::sync::mpsc::{channel, RecvTimeoutError};
    let stream = h.client().connect().unwrap_or_else(|_| panic!("the engine accepts the client"));
    let (mut reader, mut writer) = (stream.reader, stream.writer);
    let (send, lines) = channel();
    std::thread::spawn(move || loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                if send.send(line).is_err() {
                    break;
                }
            }
        }
    });
    let line = json!({"v": 1, "type": kind, "id": 1, "payload": payload});
    writer.write_all(format!("{line}\n").as_bytes()).unwrap();
    writer.flush().unwrap();
    let wait = std::time::Duration::from_secs(20) * crate::testkit::ci_factor();
    loop {
        let line = match lines.recv_timeout(wait) {
            Ok(line) => line,
            Err(RecvTimeoutError::Disconnected) => panic!("the engine closed the connection"),
            Err(RecvTimeoutError::Timeout) => panic!("no line from the engine within {wait:?}"),
        };
        let reply: serde_json::Value = serde_json::from_str(&line).unwrap();
        if reply["id"] == json!(1) {
            return reply;
        }
    }
}

/// Review round 1 (V1): a rip result whose transition cannot be persisted feeds the state's failure row, persisted as
/// usual, so the job fails as Unknown and the disc-phase token passes to the next disc; its Retry waits for the token.
#[test]
fn a_rip_result_that_cannot_be_persisted_fails_the_job_and_gives_the_token_back() {
    let mut script = disc_phase();
    script.truncate(4); // RunScan, Plan, CheckSpace, then a RipTitle whose TitleRipped is refused
    script.push(("RunScan", StepScript::Hang)); // disc-b holds the token
    script.push(("RipTitle", StepScript::Hang)); // disc-a's retry
    let h = Harness::new_scripted(script, json!({}));
    h.refuse_persists(|_, transition| transition.event == Some(Event::TitleRipped));
    h.insert("tok0", "disc-a");
    let a = id_of(&h, "disc-a");
    h.wait("Failed", |h| h.state_of(&a) == "Failed");
    let record = h.record(&a);
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Ripping) });
    let message = "the TitleRipped transition into Ripping could not be persisted: refused by the test";
    assert_eq!(record.message.as_deref(), Some(message));
    let lines = h.transitions(&a);
    assert!(matches!(lines.last().unwrap().event, Some(Event::TitleRipFailed { .. })), "{lines:?}");
    assert!(lines.iter().all(|line| line.event != Some(Event::TitleRipped)), "{lines:?}");
    let refused = format!("{a}: the transition into Ripping could not be persisted, so none of its actions ran");
    assert!(h.engine_log().contains(&refused), "{}", h.engine_log());
    h.insert("tok1", "disc-b");
    h.wait("disc-b scanning", |h| h.steps().calls().len() == 5);
    assert_eq!(request(&h, "retry", json!({"job": a}))["type"], json!("ok"));
    h.settle();
    let retried = h.transitions(&a).iter().any(|line| line.event == Some(Event::Retry));
    assert!(!retried, "the Retry did not wait for the token");
    h.remove("tok1");
    h.wait("disc-a ripping again", |h| h.steps().calls().iter().filter(|call| *call == "RipTitle").count() == 2);
    let retried = line_of(&h, &a, &Event::Retry);
    assert_eq!((retried.new, retried.actions), (State::Ripping, vec![Action::EmptyTitleDirectory, Action::RipTitle]));
}

/// Review round 1 (V1, RR1-3): a transition whose state.json was replaced but whose transitions line could not be
/// appended counts as persisted: its actions run, and engine.log names the lost line.
#[test]
fn a_transition_whose_log_line_is_lost_still_runs_its_actions() {
    let script = vec![("RunScan", ok(Event::ScanDone)), ("Plan", StepScript::Hang)];
    let h = Harness::new_scripted(script, json!({}));
    h.lose_log_lines(|_, transition| transition.event == Some(Event::ScanDone));
    h.insert("tok0", "disc-a");
    let a = id_of(&h, "disc-a");
    h.wait("Plan called", |h| h.steps().calls() == ["RunScan", "Plan"]);
    assert_eq!(h.record(&a).state, State::Scanned);
    let lines = h.transitions(&a);
    let states: Vec<&str> = lines.iter().map(|line| line.new.as_str()).collect();
    assert_eq!(states, ["Detected", "Fingerprinted"]);
    let lost = format!("{a}: the transitions line of ScanDone into Scanned was lost: lost by the test");
    assert!(h.engine_log().contains(&lost), "{}", h.engine_log());
}
