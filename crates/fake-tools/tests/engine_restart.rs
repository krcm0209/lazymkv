//! Restarts over the stand-in tools (design part 5.10, L1, L1a; Ruling S12).

use std::fs;
use std::path::{Path, PathBuf};

use lazymkv_core::job::{Action, Event, FailureClass, Held, State};
use lazymkv_core::record::{JobRecord, Transition};
use lazymkv_engine::testkit::{ci_factor, Harness};
use serde_json::json;

/// The disc of the base scenario, as Task 22's Harness::insert names it.
const DISC: &str = "movie-direct";
const FINAL: &str = "Synthetic Movie (2001) {tmdb-900001}/Synthetic Movie (2001) {tmdb-900001} - 1080p.mkv";

fn settings() -> serde_json::Value {
    json!({"keep_tuning_evidence": false, "drive_settle_s": 0, "drive_ready_timeout_s": 1})
}

fn harness(scenarios: &[&str], settings: serde_json::Value) -> Harness {
    Harness::new(Path::new(env!("CARGO_BIN_EXE_fake-tool")), scenarios, settings)
}

fn job_ids(h: &Harness) -> Vec<String> {
    let mut ids: Vec<String> = fs::read_dir(h.data().join("jobs")).map_or(vec![], |dir| {
        dir.map(|e| e.unwrap().file_name().into_string().unwrap()).collect()
    });
    ids.sort();
    ids
}

fn only_job(h: &Harness) -> String {
    let ids = job_ids(h);
    assert_eq!(ids.len(), 1, "{ids:?}");
    ids[0].clone()
}

fn transitions(dir: &Path) -> Vec<Transition> {
    fs::read_to_string(dir.join("transitions.jsonl")).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
}

fn state_json(dir: &Path) -> JobRecord {
    serde_json::from_slice(&fs::read(dir.join("state.json")).unwrap()).unwrap()
}

fn final_path(h: &Harness) -> PathBuf { h.library().join(FINAL) }

#[test]
fn a_restart_in_each_state_of_the_path_still_reaches_filed() {
    for state in ["Detected", "Fingerprinted", "Scanned", "Planned", "Ripping", "RipVerified", "Ejected", "Inspecting",
                  "Decided", "Remuxing", "Verified"] {
        let h = harness(&["movie-direct"], settings());
        h.insert("tok0", DISC);
        h.run_until(state);
        h.restart();
        h.run_until("Filed");
        let dir = h.job_dir(&only_job(&h));
        let lines = transitions(&dir);
        assert!(lines.iter().any(|t| matches!(t.event, Some(Event::EngineRestart { .. }))), "{state}");
        assert_eq!(lines.last().unwrap().new, State::Filed, "{state}");
        assert_eq!(fs::read(final_path(&h)).unwrap().len(), 16, "{state}");
        assert!(fs::read_dir(h.library().join(".lazymkv-partial")).unwrap().next().is_none(), "{state}");
    }
}

#[test]
fn a_restart_after_a_lost_last_transition_line_resumes_from_state_json_and_appends_after_it() {
    let h = harness(&["movie-direct"], settings());
    h.insert("tok0", DISC);
    h.run_until("Ripping");
    let dir = h.job_dir(&only_job(&h));
    let text = fs::read_to_string(dir.join("transitions.jsonl")).unwrap();
    let kept: Vec<&str> = text.lines().collect::<Vec<_>>()[..text.lines().count() - 1].to_vec();
    let kept_text: String = kept.iter().map(|l| format!("{l}\n")).collect();
    fs::write(dir.join("transitions.jsonl"), &kept_text).unwrap();
    assert_eq!(state_json(&dir).state, State::Ripping);

    h.restart();
    h.run_until("Filed");
    let after = fs::read_to_string(dir.join("transitions.jsonl")).unwrap();
    assert!(after.starts_with(&kept_text));
    let lines = transitions(&dir);
    let resumed = &lines[kept.len()];
    assert_eq!(resumed.event, Some(Event::EngineRestart { disc_present: true }));
    assert_eq!(resumed.old, Some(State::Ripping));
    assert_eq!(resumed.new, State::Ripping);
    assert_eq!(resumed.actions, vec![Action::EmptyTitleDirectory, Action::RipTitle]);
    assert_eq!(lines.last().unwrap().new, State::Filed);
}

/// RP-8: the next append first restores a torn last line's newline, so every line still parses.
#[test]
fn a_torn_last_line_gets_its_newline_before_the_next_append() {
    let h = harness(&["movie-direct"], settings());
    h.insert("tok0", DISC);
    h.run_until("Ripping");
    let dir = h.job_dir(&only_job(&h));
    let text = fs::read_to_string(dir.join("transitions.jsonl")).unwrap();
    fs::write(dir.join("transitions.jsonl"), text.strip_suffix('\n').unwrap()).unwrap();

    h.restart();
    h.run_until("Filed");
    assert!(fs::read_to_string(dir.join("transitions.jsonl")).unwrap().starts_with(&text));
    // `transitions` parses every line, so a joined line fails here.
    assert_eq!(transitions(&dir).last().unwrap().new, State::Filed);
}

#[test]
fn a_held_job_stays_held_after_a_restart() {
    let h = harness(&["movie-direct"], settings());
    fs::create_dir_all(final_path(&h).parent().unwrap()).unwrap();
    fs::write(final_path(&h), b"owner's").unwrap();
    h.insert("tok0", DISC);
    h.run_until("NeedsReview");
    let dir = h.job_dir(&only_job(&h));
    let before = fs::read(dir.join("transitions.jsonl")).unwrap();
    h.restart();
    assert_eq!(fs::read(dir.join("transitions.jsonl")).unwrap(), before);
    assert_eq!(state_json(&dir).state, State::NeedsReview { held: Held::DispositionGate });
    assert_eq!(fs::read(final_path(&h)).unwrap(), b"owner's");
}

#[test]
fn a_disc_left_in_after_a_copy_mode_failure_starts_nothing_at_a_restart() {
    let h = harness(&["movie-direct", "movie-direct-copy-plan"], settings());
    h.insert("tok0", DISC);
    h.run_until("Failed");
    let id = only_job(&h);
    let before = fs::read(h.job_dir(&id).join("transitions.jsonl")).unwrap();
    let ejects = h.ejector().calls().len();
    h.restart();
    assert_eq!(job_ids(&h), vec![id.clone()]);
    assert_eq!(fs::read(h.job_dir(&id).join("transitions.jsonl")).unwrap(), before);
    assert_eq!(h.ejector().calls().len(), ejects);
    let drives: serde_json::Value = serde_json::from_slice(&fs::read(h.data().join("drives.json")).unwrap()).unwrap();
    assert_eq!(drives["drives"]["tok0"]["handled"], json!(true));
}

#[test]
fn a_disc_left_in_after_a_failed_eject_starts_nothing_at_a_restart() {
    let h = harness(&["movie-direct"], settings());
    h.ejector().fail_next("stuck");
    h.insert("tok0", DISC);
    h.run_until("Filed");
    let id = only_job(&h);
    assert!(transitions(&h.job_dir(&id)).iter().any(|t| t.event == Some(Event::EjectFailed)));
    let before = fs::read(h.job_dir(&id).join("transitions.jsonl")).unwrap();
    let ejects = h.ejector().calls().len();
    h.restart();
    h.settle();
    assert_eq!(job_ids(&h), vec![id.clone()]);
    assert_eq!(fs::read(h.job_dir(&id).join("transitions.jsonl")).unwrap(), before);
    assert_eq!(h.ejector().calls().len(), ejects);
    let drives: serde_json::Value = serde_json::from_slice(&fs::read(h.data().join("drives.json")).unwrap()).unwrap();
    assert_eq!(drives["drives"]["tok0"]["handled"], json!(true));
}

#[test]
fn a_restart_after_a_late_answer_decides_with_the_refreshed_identity() {
    let h = harness(&["movie-direct", "movie-direct-lookup-late"], settings());
    h.scan_waits_for_lookups(false);
    h.insert("tok0", DISC);
    h.run_until("Inspecting");
    let dir = h.job_dir(&only_job(&h));
    assert!(fs::read_to_string(dir.join("lookup-status.txt")).unwrap().contains("dischash 404"));
    assert!(state_json(&dir).plan.unwrap().identity.lookup_pending, "the refresh before Decide has not run yet");
    h.restart();
    h.run_until("Filed");
    let record = state_json(&dir);
    assert!(!record.plan.unwrap().identity.lookup_pending);
    assert_eq!(Path::new(&record.titles[0].filed.as_ref().unwrap().path), final_path(&h).as_path());
}

#[test]
fn a_rebuilt_remux_promise_counts_against_another_jobs_rip_check() {
    // One volume for staging and the library. A's rip check needs 4301 + 4301 (S54) and passes; after the restart in
    // Remuxing, A's rebuilt Remux promise is write_bytes_needed(16) = 17, so B's identical check needs 8619 > 8610.
    let h = harness(&["movie-direct", "movie-direct-remux-hang"], settings());
    let (staging, movies) = (h.staging().to_str().unwrap().to_string(), h.library().to_str().unwrap().to_string());
    h.shares().same_volume(&staging, &movies);
    h.shares().set_free(&staging, 8610);
    h.shares().set_free(&movies, 8610);
    h.insert("tok0", DISC);
    h.run_until("Remuxing");
    let a = only_job(&h);
    h.restart();
    h.scan_waits_for_lookups(false);
    h.insert("tok0", "disc-b");
    h.wait("B waits for space", |h| h.job_ids().iter().any(|id| *id != a && h.state_of(id) == "AwaitingSpace"));
    assert_eq!(h.state_of(&a), "Remuxing");
}

#[test]
fn a_disc_already_in_a_drive_is_an_arrival_at_start() {
    let h = harness(&["movie-direct"], json!({"keep_tuning_evidence": false, "drive_settle_s": 0,
                                                   "drive_ready_timeout_s": 1, "file_phase_concurrency": 0}));
    h.insert("tok0", DISC);
    h.restart(); // the settings problem keeps the arrival waiting; no job folder exists
    assert!(job_ids(&h).is_empty());
    let path = h.data().join("settings.json");
    let mut stored: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    stored["file_phase_concurrency"] = json!(1);
    fs::write(&path, serde_json::to_vec(&stored).unwrap()).unwrap();
    h.restart();
    h.run_until("Filed");
    assert_eq!(job_ids(&h).len(), 1);
}

#[test]
fn a_restart_in_verified_with_the_file_already_filed_feeds_file_done() {
    let h = harness(&["movie-direct"], settings());
    h.insert("tok0", DISC);
    h.run_until("Verified");
    let id = only_job(&h);
    let partial = fs::read_dir(h.library().join(".lazymkv-partial")).unwrap().next().unwrap().unwrap().path();
    fs::create_dir_all(final_path(&h).parent().unwrap()).unwrap();
    fs::rename(&partial, final_path(&h)).unwrap();
    h.restart();
    h.run_until("Filed");
    let lines = transitions(&h.job_dir(&id));
    let n = lines.len();
    assert_eq!(lines[n - 2].event, Some(Event::EngineRestart { disc_present: false }));
    assert_eq!(lines[n - 1].event, Some(Event::FileDone));
    let record = state_json(&h.job_dir(&id));
    let filed = record.titles[0].filed.as_ref().unwrap();
    assert_eq!(Path::new(&filed.path), final_path(&h).as_path());
    assert_eq!(filed.size, 16);
    assert!(record.filed_unix.is_some());
}

#[test]
fn a_restart_in_verified_with_another_size_at_the_final_path_fails_as_unknown() {
    let h = harness(&["movie-direct"], settings());
    h.insert("tok0", DISC);
    h.run_until("Verified");
    let id = only_job(&h);
    let partial = fs::read_dir(h.library().join(".lazymkv-partial")).unwrap().next().unwrap().unwrap().path();
    fs::remove_file(partial).unwrap();
    fs::create_dir_all(final_path(&h).parent().unwrap()).unwrap();
    fs::write(final_path(&h), b"abc").unwrap();
    h.restart();
    h.run_until("Failed");
    let last = transitions(&h.job_dir(&id)).pop().unwrap();
    assert!(matches!(last.event, Some(Event::FileFailed { failure }) if failure.class == FailureClass::Unknown));
    assert_eq!(fs::read(final_path(&h)).unwrap(), b"abc");
}

/// Design part 6.5 with `rename_no_replace`'s two steps (hard link, then remove): a stop between them leaves both names
/// on one file, and File after the restart removes the partial name and records the filed file, with no collision.
#[test]
fn a_restart_after_a_rename_stopped_between_link_and_remove_finishes_it() {
    let h = harness(&["movie-direct"], settings());
    h.insert("tok0", DISC);
    h.run_until("Verified");
    let id = only_job(&h);
    let partial = fs::read_dir(h.library().join(".lazymkv-partial")).unwrap().next().unwrap().unwrap().path();
    fs::create_dir_all(final_path(&h).parent().unwrap()).unwrap();
    fs::hard_link(&partial, final_path(&h)).unwrap();
    h.restart();
    h.run_until("Filed");
    assert!(!partial.exists());
    assert_eq!(fs::read(final_path(&h)).unwrap().len(), 16);
    let record = state_json(&h.job_dir(&id));
    assert!(record.collisions.is_empty(), "{:?}", record.collisions);
    assert_eq!(record.titles[0].filed.as_ref().unwrap().size, 16);
}

#[test]
fn a_decide_after_restart_reads_its_inputs_back_with_devs_readers() {
    let h = harness(&["movie-direct"], settings());
    h.insert("tok0", DISC);
    h.run_until("Decided");
    let dir = h.job_dir(&only_job(&h));
    let key = fs::read_dir(&dir).unwrap().map(|e| e.unwrap().path()).find(|p| p.join("decide.json").is_file()).unwrap();
    let first = fs::read(key.join("decide.json")).unwrap();
    h.restart();
    h.run_until("Filed");
    assert!(fs::read_to_string(dir.join("job.log")).unwrap().contains(
        "decide inputs read back from the job folder with lazymkv-dev's readers (design part 5.10)"));
    assert_eq!(fs::read(key.join("decide.json")).unwrap(), first);
}

#[test]
fn awaiting_lookup_recreates_its_timer() {
    use lazymkv_engine::testkit::StepScript;
    use lazymkv_engine::StepResult;
    let steps = vec![
        ("RunScan", StepScript::Result(StepResult { event: Some(Event::ScanDone), message: None })),
        ("Plan", StepScript::Result(StepResult { event: Some(Event::PlanDone { mode: lazymkv_core::plan::Mode::Direct }), message: None })),
        ("CheckSpace", StepScript::Result(StepResult { event: Some(Event::SpaceOk), message: None })),
        ("RipTitle", StepScript::Result(StepResult { event: Some(Event::TitleRipped), message: None })),
        ("VerifyFile", StepScript::Result(StepResult { event: Some(Event::TitleVerified { last: true }), message: None })),
        ("Eject", StepScript::Result(StepResult { event: Some(Event::Ejected), message: None })),
        ("Probe", StepScript::Result(StepResult { event: Some(Event::ProbeDone), message: None })),
        ("ExtractSubtitleEvidence", StepScript::Result(StepResult { event: None, message: None })),
        ("Decide", StepScript::Result(StepResult { event: Some(Event::LookupPending), message: None })),
    ];
    let h = Harness::new_scripted(steps, settings());
    h.insert("tok0", DISC);
    h.run_until("AwaitingLookup");
    let dir = h.job_dir(&only_job(&h));
    // PF-4: the scripted Eject never calls the ejector, so this stands for the removal a real eject causes (R22.6).
    h.remove("tok0");
    h.restart();
    let restart = transitions(&dir).pop().unwrap();
    assert_eq!(restart.event, Some(Event::EngineRestart { disc_present: false }));
    assert_eq!(restart.actions, vec![Action::StartTimer { minutes: 60 }]);
    let before = transitions(&dir).len();
    h.fire_timers();
    let after = std::time::Instant::now();
    // 10 s, 40 s under CI (Ruling R29.c5).
    let wait = std::time::Duration::from_secs(10) * ci_factor();
    while transitions(&dir).len() == before && after.elapsed() < wait {
        std::thread::yield_now();
    }
    let fired = transitions(&dir).pop().unwrap();
    assert!(matches!(fired.old, Some(State::AwaitingLookup { .. })));
    assert!(matches!(fired.event, Some(Event::LookupPending | Event::LookupAnswered)));
}
