//! The file phase on stand-ins, from the slot to the filed file (design parts 5.7, 6.4, 6.5, 8.3, 8.4).

use std::fs;
use std::path::{Path, PathBuf};

use lazymkv_core::job::{Action, Event, FailureClass, Held, NotifyEvent, State, Tool, WaitReason};
use lazymkv_core::lookup::IdentityStep;
use lazymkv_core::review::ReviewReason;
use lazymkv_dev::cmd::decide::{decide_doc, read_decide_inputs, rec_folder};
use lazymkv_dev::cmd::plan::PlanOptions;
use lazymkv_dev::evidence::Ctx;
use lazymkv_dev::pyjson::{dumps, Style};
use lazymkv_engine::testkit::Harness;
use lazymkv_platform::Host;
use serde_json::{json, Value};

const KEY: &str = "t00-00800";

fn harness(variants: &[&str], settings: Value) -> Harness {
    let mut scenarios = vec!["movie-direct"];
    scenarios.extend_from_slice(variants);
    Harness::new(Path::new(env!("CARGO_BIN_EXE_fake-tool")), &scenarios, settings)
}

/// Inserts movie-direct in tok0 and waits for a persisted transition into `state` (not the current state).
fn run_to(h: &Harness, state: &str) -> String {
    h.insert("tok0", "movie-direct");
    let id = format!("{}-1", h.fingerprint("movie-direct").to_ascii_lowercase());
    h.wait(state, |h| h.transitions(&id).iter().any(|line| line.new.as_str() == state));
    id
}

fn json_of(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))).unwrap()
}

fn events(h: &Harness, id: &str) -> Vec<Event> {
    h.transitions(id).into_iter().filter_map(|line| line.event).collect()
}

fn final_path(h: &Harness) -> PathBuf {
    let folder = "Synthetic Movie (2001) {tmdb-900001}";
    h.library().join(folder).join(format!("{folder} - 1080p.mkv"))
}

#[test]
fn a_lookup_pending_plan_gets_one_round_before_the_slot() {
    let h = harness(&["movie-direct-lookup-late"], json!({}));
    h.scan_waits_for_lookups(false);
    let id = run_to(&h, "Filed");
    let found = events(&h, &id);
    assert!(!found.contains(&Event::LookupPending));
    assert!(found.contains(&Event::FilePhaseSlot));
    let dischash = lazymkv_clients::dischash_url(&h.fingerprint("movie-direct"));
    assert_eq!(h.http().requests().iter().filter(|(url, _)| *url == dischash).count(), 2);
    assert!(fs::read_to_string(h.job_dir(&id).join("lookup-status.txt")).unwrap().contains("dischash 404"));
}

#[test]
fn still_pending_after_the_round_waits_in_awaiting_lookup_with_a_60_minute_timer() {
    let h = harness(&["movie-direct-lookup-down"], json!({}));
    h.scan_waits_for_lookups(false);
    let id = run_to(&h, "AwaitingLookup");
    h.settle();
    assert_eq!(h.record(&id).state, State::AwaitingLookup { return_to: Box::new(State::Ejected { after_copy: false }) });
    assert_eq!(h.timers().pending(), vec![60]);
    assert!(h.notifications().iter().any(|n| n.event == NotifyEvent::Waiting { reason: WaitReason::Lookup }));
}

#[test]
fn a_timer_expiry_that_answers_returns_with_refresh_identity_first() {
    let h = harness(&["movie-direct-lookup-later"], json!({}));
    h.scan_waits_for_lookups(false);
    let id = run_to(&h, "AwaitingLookup");
    h.fire_timers();
    h.wait("Filed", |h| h.state_of(&id) == "Filed");
    let answered = h.transitions(&id).into_iter().find(|l| l.event == Some(Event::LookupAnswered)).unwrap();
    assert_eq!(answered.new, State::Ejected { after_copy: false });
    assert_eq!(answered.actions, vec![Action::RefreshIdentity, Action::Notify { event: NotifyEvent::Resumed }]);
    assert!(!h.record(&id).plan.unwrap().identity.lookup_pending);
}

#[test]
fn an_answer_arriving_during_the_rip_is_saved_and_reaches_decide_without_lookup_answered() {
    let h = harness(&[], json!({}));
    h.scan_waits_for_lookups(false);
    let gate = h.http().gate(&lazymkv_clients::dischash_url(&h.fingerprint("movie-direct")));
    let id = run_to(&h, "Ejected");
    gate.open();
    h.wait("Filed", |h| h.state_of(&id) == "Filed");
    let found = events(&h, &id);
    assert!(!found.contains(&Event::LookupPending) && !found.contains(&Event::LookupAnswered));
    let record = h.record(&id);
    let plan = record.plan.unwrap();
    assert!(!plan.identity.lookup_pending);
    assert_eq!(plan.identity.step, IdentityStep::TmdbOne);
    assert!(record.titles[0].decision.as_ref().unwrap().path.is_some());
}

#[test]
fn a_refused_token_waits_with_a_message_naming_it() {
    let h = harness(&["movie-direct-lookup-refused"], json!({}));
    h.scan_waits_for_lookups(false);
    let id = run_to(&h, "AwaitingLookup");
    h.settle();
    let message = h.record(&id).message.unwrap();
    assert!(message.contains("search: TMDb refused the token (HTTP 401)"), "{message}");
    let waiting = h.notifications().into_iter().last().unwrap();
    assert_eq!(waiting.event, NotifyEvent::Waiting { reason: WaitReason::Lookup });
    assert!(waiting.message.contains("TMDb refused the token (HTTP 401)"), "{}", waiting.message);
}

#[test]
fn an_unparsed_answer_puts_the_parsers_sentence_in_job_log_and_the_waiting_message() {
    let h = harness(&["movie-direct-lookup-unparsed"], json!({}));
    h.scan_waits_for_lookups(false);
    let id = run_to(&h, "AwaitingLookup");
    let message = h.record(&id).message.unwrap();
    assert!(message.starts_with("dischash: "), "{message}");
    let sentence = message["dischash: ".len()..].split("; ").next().unwrap().to_string();
    let log = fs::read_to_string(h.job_dir(&id).join("job.log")).unwrap();
    assert!(log.contains(&format!("lookup dischash: {sentence}")), "{log}");
}

#[test]
fn a_partial_answer_is_saved_and_the_job_keeps_waiting() {
    let h = harness(&["movie-direct-lookup-partial"], json!({}));
    h.scan_waits_for_lookups(false);
    let id = run_to(&h, "AwaitingLookup");
    h.fire_timers();
    h.wait("dischash saved", |h| fs::read_to_string(h.job_dir(&id).join("lookup-status.txt")).unwrap().contains("dischash 404"));
    h.settle();
    assert_eq!(h.state_of(&id), "AwaitingLookup");
    let last = h.transitions(&id).pop().unwrap();
    assert_eq!((last.event, last.actions), (Some(Event::LookupPending), vec![Action::StartTimer { minutes: 60 }]));
    assert_eq!(h.timers().pending(), vec![60]);
}

#[test]
fn decide_json_is_the_renderers_document_with_two_extra_keys() {
    let h = harness(&[], json!({}));
    let id = run_to(&h, "Filed");
    let doc = json_of(&h.job_dir(&id).join(KEY).join("decide.json"));
    let keys: Vec<&str> = doc.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(keys, ["slug", "label", "reconcile", "audio", "subtitles", "reviews", "plan", "decision", "audio_rule_version", "lazymkv_version"]);
    assert_eq!((&doc["slug"], &doc["label"]), (&json!(id), &json!(KEY)));
    assert_eq!(doc["audio_rule_version"], json!(lazymkv_core::audio::rule::RULE_VERSION));
    assert_eq!(doc["lazymkv_version"], json!(env!("CARGO_PKG_VERSION")));
    assert_eq!(doc["audio"], json!([]));
    assert!(doc["subtitles"]["skipped"].as_str().unwrap().starts_with("no OCR text"));
    assert!(fs::read_to_string(h.job_dir(&id).join("job.log")).unwrap().contains("step 10a did not run in this version"));
}

#[test]
fn decide_json_uses_the_probe_the_engine_ran() {
    let h = harness(&[], json!({}));
    let id = run_to(&h, "Filed");
    let doc = json_of(&h.job_dir(&id).join(KEY).join("decide.json"));
    assert_eq!(doc["decision"]["facts"]["profile_streams"], json!([1]));
    assert!(h.job_dir(&id).join(KEY).join("ffprobe-streams.json").is_file());
}

#[test]
fn the_engines_decide_inputs_render_as_read_decide_inputs_over_the_finished_job_folder() {
    let h = harness(&[], json!({}));
    let id = run_to(&h, "Filed");
    let dir = h.job_dir(&id);
    let ctx = Ctx::for_test(h.data());
    let (rec, label) = rec_folder(&ctx, &dir.join(KEY).display().to_string()).unwrap_or_else(|r| panic!("{}", r.line));
    let options = PlanOptions {
        settings: Some(dir.join("settings.json")),
        tmdb_search: Some(dir.join("tmdb-search.json")),
        tmdb_record: Some(dir.join("tmdb-record.json")),
        remembered: None,
    };
    let read = read_decide_inputs(&ctx, &rec, &label, None, None, &options).unwrap_or_else(|r| panic!("{}", r.line));
    let mut written = json_of(&dir.join(KEY).join("decide.json"));
    let object = written.as_object_mut().unwrap();
    object.remove("audio_rule_version");
    object.remove("lazymkv_version");
    assert_eq!(
        dumps(&written, Style::INDENT1_ASCII),
        dumps(&serde_json::to_value(decide_doc(&read)).unwrap(), Style::INDENT1_ASCII)
    );
}

#[test]
fn a_lookup_pending_job_decides_with_the_refreshed_plan() {
    let h = harness(&["movie-direct-lookup-late"], json!({}));
    h.scan_waits_for_lookups(false);
    let id = run_to(&h, "Filed");
    let record = h.record(&id);
    let persisted = serde_json::to_value(record.plan.as_ref().unwrap()).unwrap();
    assert_eq!(persisted["identity"]["lookup_pending"], json!(false));
    let doc = json_of(&h.job_dir(&id).join(KEY).join("decide.json"));
    // RP-4: the plan section carries the persisted refreshed plan.
    assert_eq!(doc["plan"]["plan"], persisted);
}

/// A remembered decision for movie-direct's disc `fp` whose one title, the feature 00800.mpls, keeps its one audio
/// stream (English DTS-HD MA 5.1, signature position 0) first and as the default (spec 5, 11 "Override tracks").
fn remembered_keep(fp: &str) -> String {
    format!(concat!(
        r#"{{"format":"lazymkv remembered v1","fingerprint":"{fp}","tmdb":{{"kind":"movie","id":900001}},"titles":[{{"#,
        r#""source_file":"00800.mpls","dvd_title":null,"segment_map":"1","seconds":7150,"role":{{"role":"feature"}},"#,
        r#""audio":[{{"signature":{{"kind":"audio","language":"eng","codec":"dts-hd-ma","channels":6,"position":0}},"#,
        r#""action":"keep","order":0,"default":true}}],"subtitles":[]}}]}}"#), fp = fp)
}

/// The PR #6 review's V14 (spec 5, F10, RR1-14): a remembered decision's track overrides reach decide.json. The Keep
/// covers the title's only audio stream, so the audio part is 1.00 (0.95 without it); the track confidence stays 0.95,
/// the case of a title with no English subtitle stream. The filed file is the one without the override, and decide.json
/// is what `read_decide_inputs` renders over the finished job folder with the job's remembered.json (R-B1).
#[test]
fn a_remembered_decisions_track_overrides_reach_decide_json() {
    let plain = harness(&[], json!({}));
    let plain_id = run_to(&plain, "Filed");
    let plain_doc = json_of(&plain.job_dir(&plain_id).join(KEY).join("decide.json"));
    assert_eq!(plain_doc["decision"]["decision"]["audio"]["confidence"], json!(0.95));

    let h = harness(&[], json!({}));
    let fp = h.fingerprint("movie-direct");
    fs::create_dir_all(h.data().join("remembered")).unwrap();
    fs::write(h.data().join("remembered").join(format!("{fp}.json")), remembered_keep(&fp)).unwrap();
    let id = run_to(&h, "Filed");
    let dir = h.job_dir(&id);
    let doc = json_of(&dir.join(KEY).join("decide.json"));
    let decision = &doc["decision"]["decision"];
    assert_eq!(decision["audio"]["confidence"], json!(1.0));
    assert_eq!(decision["confidences"]["track"], json!(0.95));
    assert_eq!(decision["layout"], plain_doc["decision"]["decision"]["layout"]);
    assert_eq!(decision["audio"]["kept"], plain_doc["decision"]["decision"]["audio"]["kept"]);
    let record = h.record(&id);
    assert_eq!(Path::new(&record.titles[0].filed.as_ref().unwrap().path), final_path(&h).as_path());

    let ctx = Ctx::for_test(h.data());
    let (rec, label) = rec_folder(&ctx, &dir.join(KEY).display().to_string()).unwrap_or_else(|r| panic!("{}", r.line));
    // The replay files the engine reads, as `lazymkvctl decide` is given them (the engine's replay_options).
    let given = |name: &str| Some(dir.join(name)).filter(|path| path.is_file());
    assert!(given("remembered.json").is_some());
    let options = PlanOptions {
        settings: given("settings.json"),
        tmdb_search: given("tmdb-search.json"),
        tmdb_record: given("tmdb-record.json"),
        remembered: given("remembered.json"),
    };
    let read = read_decide_inputs(&ctx, &rec, &label, None, None, &options).unwrap_or_else(|r| panic!("{}", r.line));
    let mut written = doc.clone();
    let object = written.as_object_mut().unwrap();
    object.remove("audio_rule_version");
    object.remove("lazymkv_version");
    assert_eq!(
        dumps(&written, Style::INDENT1_ASCII),
        dumps(&serde_json::to_value(decide_doc(&read)).unwrap(), Style::INDENT1_ASCII)
    );
}

#[test]
fn two_english_subtitle_streams_fail_the_decision_until_step_10_exists() {
    let h = harness(&["movie-direct-two-subs"], json!({}));
    let id = run_to(&h, "Failed");
    let record = h.record(&id);
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Inspecting) });
    assert_eq!(record.message.as_deref(), Some("subtitle evidence (step 10) is not built in this version"));
    assert!(!h.job_dir(&id).join(KEY).join("decide.json").exists());
}

#[test]
fn a_collision_holds_the_job_with_path_collision_and_keeps_the_path() {
    let h = harness(&[], json!({}));
    let target = final_path(&h);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(&target, b"already here").unwrap();
    let id = run_to(&h, "NeedsReview");
    let record = h.record(&id);
    assert_eq!(record.state, State::NeedsReview { held: Held::DispositionGate });
    assert_eq!(record.collisions.iter().map(PathBuf::from).collect::<Vec<_>>(), vec![target.clone()]);
    assert!(record.titles[0].decision.as_ref().unwrap().reviews.contains(&ReviewReason::PathCollision));
    assert_eq!(fs::read(&target).unwrap(), b"already here");
    assert_eq!(fs::read_dir(h.library().join(".lazymkv-partial")).map(|d| d.count()).unwrap_or(0), 0);
}

/// R24.c2: the library facts lead the decision to an edition-named path, and the collision there holds the job.
#[test]
fn a_collision_at_the_path_the_library_facts_lead_to_holds_the_job() {
    let h = harness(&[], json!({"edition_style": "jellyfin"}));
    let folder = "Synthetic Movie (2001) {tmdb-900001}";
    let target = h.library().join(folder).join(format!("{folder} - 1080p Theatrical Cut.mkv"));
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(&target, b"already here").unwrap();
    let id = run_to(&h, "NeedsReview");
    let record = h.record(&id);
    assert_eq!(record.state, State::NeedsReview { held: Held::DispositionGate });
    let decision = record.titles[0].decision.as_ref().unwrap();
    assert!(decision.reviews.contains(&ReviewReason::PathCollision), "{:?}", decision.reviews);
    assert_eq!(decision.path.as_ref().map(|path| PathBuf::from(&path.path)), Some(target.clone()));
    assert_eq!(record.collisions.iter().map(PathBuf::from).collect::<Vec<_>>(), vec![target.clone()]);
    assert_eq!(fs::read(&target).unwrap(), b"already here");
    assert_eq!(fs::read_dir(h.library().join(".lazymkv-partial")).map(|d| d.count()).unwrap_or(0), 0);
}

#[test]
fn an_existing_final_file_fails_file_failed_unknown_and_keeps_the_partial() {
    let h = harness(&[], json!({}));
    let target = final_path(&h);
    let late = target.clone();
    h.on_spawn(&["-J", ".partial"], move || {
        fs::create_dir_all(late.parent().unwrap()).unwrap();
        fs::write(&late, b"late").unwrap();
    });
    let id = run_to(&h, "Failed");
    let record = h.record(&id);
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Verified) });
    assert!(record.message.unwrap().contains(&target.display().to_string()));
    assert_eq!(fs::read(&target).unwrap(), b"late");
    let partial = h.library().join(".lazymkv-partial").join(format!("{id}-{KEY}.mkv.partial"));
    assert_eq!(fs::read(partial).unwrap(), b"lazymkv-test-mkv");
    assert!(record.collisions.iter().any(|path| PathBuf::from(path) == target));
}

#[test]
fn filed_records_path_size_mtime_and_filed_unix() {
    let h = harness(&[], json!({}));
    let id = run_to(&h, "Filed");
    h.settle();
    let target = final_path(&h);
    assert_eq!(fs::read(&target).unwrap(), b"lazymkv-test-mkv");
    let mtime = fs::metadata(&target).unwrap().modified().unwrap().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let record = h.record(&id);
    let filed = record.titles[0].filed.clone().unwrap();
    assert_eq!((PathBuf::from(&filed.path), filed.size, filed.mtime_unix), (target.clone(), 16, mtime));
    assert_eq!(record.filed_unix, Some(h.host().now_unix()));
    let sent: Vec<NotifyEvent> = h.notifications().into_iter().map(|n| n.event).collect();
    assert_eq!(sent, vec![NotifyEvent::DiscDone, NotifyEvent::Filed]);
    assert!(h.staging().join(&id).join("rip").join(KEY).join("title_t00.mkv").is_file());
}

#[test]
fn the_remux_creates_the_partial_folder_when_the_library_root_has_none() {
    let h = harness(&[], json!({}));
    assert!(h.library().is_dir() && !h.library().join(".lazymkv-partial").exists());
    run_to(&h, "Filed");
    let partial = h.library().join(".lazymkv-partial");
    assert!(partial.is_dir());
    assert_eq!(fs::read_dir(partial).unwrap().count(), 0);
}

#[test]
fn a_probe_error_fails_probe_failed_unknown() {
    let h = harness(&["movie-direct-probe-error"], json!({}));
    let id = run_to(&h, "Failed");
    let record = h.record(&id);
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Inspecting) });
    assert_eq!(record.message.as_deref(), Some("ffprobe exited with code 1"));
    assert!(events(&h, &id).iter().any(|e| matches!(e, Event::ProbeFailed { .. })));
}

#[test]
fn a_remux_exit_2_fails_remux_failed_unknown_and_keeps_the_raw() {
    let h = harness(&["movie-direct-remux-error"], json!({}));
    let id = run_to(&h, "Failed");
    let record = h.record(&id);
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Remuxing) });
    assert_eq!(record.message.as_deref(), Some("mkvmerge exited with code 2"));
    assert!(events(&h, &id).iter().any(|e| matches!(e, Event::RemuxFailed { .. })));
    assert_eq!(fs::read(h.staging().join(&id).join("rip").join(KEY).join("title_t00.mkv")).unwrap(), b"lazymkv-test-mkv");
    assert!(!final_path(&h).exists());
}

/// Stops the engine at the first transition into `state` (before its actions), puts a regular file in place of the
/// title's staging folder `<staging>/<id>/rip/<KEY>`, whose own files are moved aside, and restarts the engine, so
/// listing the folder fails on every OS with an error other than NotFound (review round 1, V3). Returns the job and the
/// folder.
fn restart_with_the_title_folder_blocked(h: &Harness, state: &str) -> (String, PathBuf) {
    h.insert("tok0", "movie-direct");
    h.run_until(state);
    let id = h.job_ids().pop().unwrap();
    let dir = h.staging().join(&id).join("rip").join(KEY);
    fs::rename(&dir, dir.with_extension("aside")).unwrap();
    fs::write(&dir, b"").unwrap();
    h.restart();
    h.wait("Failed", |h| h.state_of(&id) == "Failed");
    h.settle();
    (id, dir)
}

/// Review round 1, V3 (design part 5.2, K9; RR1-6(a)): a title folder that cannot be listed fails Probe as Unknown
/// naming the folder and the error, not as a folder holding 0 `.mkv` files.
#[test]
fn an_unreadable_title_folder_fails_the_probe_with_its_error() {
    let h = harness(&[], json!({}));
    let (id, dir) = restart_with_the_title_folder_blocked(&h, "Inspecting");
    let record = h.record(&id);
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Inspecting) });
    let message = record.message.unwrap_or_default();
    assert!(message.starts_with(&format!("{}: ", dir.display())) && !message.contains("holds 0"), "{message}");
    let failed = h.notifications().into_iter().last().unwrap();
    assert_eq!((failed.event, failed.message), (NotifyEvent::Failed { class: FailureClass::Unknown }, message));
}

/// Review round 1, V3: the same for Remux, after a restart in Remuxing, whose promise rebuild falls back to the plan's
/// estimate (RR1-6(c)); nothing reaches the final path.
#[test]
fn an_unreadable_title_folder_fails_the_remux_with_its_error() {
    let h = harness(&[], json!({}));
    let (id, dir) = restart_with_the_title_folder_blocked(&h, "Remuxing");
    let record = h.record(&id);
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Remuxing) });
    let message = record.message.unwrap_or_default();
    assert!(message.starts_with(&format!("{}: ", dir.display())) && !message.contains("holds 0"), "{message}");
    assert!(!final_path(&h).exists());
}

/// Review round 1, V3 (S54, RR1-6(b)): with staging answering, the remux check that cannot read the staged sizes
/// promises the plan's estimate and says so in job.log; Remux then fails naming the folder.
#[test]
fn an_unreadable_title_folder_at_the_remux_check_uses_the_planned_promise() {
    let h = harness(&[], json!({}));
    let (id, dir) = restart_with_the_title_folder_blocked(&h, "Decided");
    let record = h.record(&id);
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Remuxing) });
    let message = record.message.unwrap_or_default();
    assert!(message.starts_with(&format!("{}: ", dir.display())), "{message}");
    let log = fs::read_to_string(h.job_dir(&id).join("job.log")).unwrap();
    let line = format!("check space: the staged files could not be read: {}: ", dir.display());
    assert!(log.contains(&line), "{log}");
}

#[test]
fn a_partial_whose_duration_differs_fails_file_verify_failed() {
    let h = harness(&["movie-direct-partial-long"], json!({}));
    let id = run_to(&h, "Failed");
    let record = h.record(&id);
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Remuxing) });
    assert!(events(&h, &id).iter().any(|e| matches!(e, Event::FileVerifyFailed { .. })));
    let message = record.message.unwrap_or_default();
    assert!(message.contains("duration: expected 7150 s, file 7153.0 s"), "{message}");
    assert!(!final_path(&h).exists());
}

#[test]
fn no_ffprobe_lets_the_disc_phase_finish_and_waits_in_awaiting_tool() {
    let h = harness(&[], json!({"tool_paths": {"ffprobe": null}}));
    let id = run_to(&h, "AwaitingTool");
    h.settle();
    assert_eq!(h.record(&id).state, State::AwaitingTool { return_to: Box::new(State::Inspecting) });
    let found = events(&h, &id);
    assert!(found.contains(&Event::Ejected) && found.contains(&Event::ToolMissing { tool: Tool::Ffprobe }), "{found:?}");
    assert!(h.notifications().iter().any(|n| n.event == NotifyEvent::Waiting { reason: WaitReason::Tool }));
}
