//! The disc phase on stand-ins, from arrival to eject (design parts 5.4 to 5.6, 5.8).

use std::fs;
use std::path::Path;

use lazymkv_core::job::{Event, FailureClass, NotifyEvent, State, Tool, WaitReason};
use lazymkv_core::lookup::IdentityStep;
use lazymkv_core::plan::Mode;
use lazymkv_core::review::{Note, ReviewReason};
use lazymkv_engine::testkit::Harness;
use lazymkv_platform::{Host, Reach};
use serde_json::{json, Value};

const KEY: &str = "t00-00800";

fn harness(variants: &[&str], settings: Value) -> Harness {
    let mut scenarios = vec!["movie-direct"];
    scenarios.extend_from_slice(variants);
    Harness::new(Path::new(env!("CARGO_BIN_EXE_fake-tool")), &scenarios, settings)
}

fn run_to(h: &Harness, state: &str) -> String {
    h.insert("tok0", "movie-direct");
    let id = format!("{}-1", h.fingerprint("movie-direct").to_ascii_lowercase());
    // A persisted transition into `state`, not the current state: from Task 24 on a job moves past Ejected at once.
    h.wait(state, |h| h.transitions(&id).iter().any(|line| line.new.as_str() == state));
    id
}

fn json_of(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))).unwrap()
}

fn runs(h: &Harness, stem: &str, response: &str) -> u64 {
    fs::read(h.tools().join(format!("{stem}.calls.json")))
        .map(|bytes| serde_json::from_slice::<Value>(&bytes).unwrap()[response].as_u64().unwrap_or(0))
        .unwrap_or(0)
}

fn events(h: &Harness, id: &str) -> Vec<Event> {
    h.transitions(id).into_iter().filter_map(|line| line.event).collect()
}

fn root(path: &Path) -> String {
    path.to_str().unwrap().to_string()
}

#[test]
fn a_movie_disc_reaches_ejected_with_its_recording() {
    let h = harness(&[], json!({}));
    let id = run_to(&h, "Ejected");
    let dir = h.job_dir(&id);
    h.wait("lookups answered", |h| fs::read_to_string(h.job_dir(&id).join("lookup-status.txt")).map(|t| t.lines().count() == 4).unwrap_or(false));
    for file in ["meta.json", "files.json", "nav/BDMV/index.bdmv", "nav/BDMV/PLAYLIST/00800.mpls", "nav/BDMV/META/DL/bdmt_eng.xml",
        "drive-list.txt", "scan.txt", "job.mmcp.xml", "settings.json", "thediscdb-hash.json", "thediscdb-discid.json",
        "tmdb.json", "tmdb-search.json", "tmdb-record.json", "lookup-status.txt", "t00-00800/rip.txt",
        "t00-00800/mkvmerge.json", "t00-00800/rip-summary.json"] {
        assert!(dir.join(file).is_file(), "{file}");
    }
    assert!(!dir.join("nav/AACS").exists());
    for run in ["drive-list.txt", "scan.txt", "t00-00800/rip.txt", "t00-00800/mkvmerge.json"] {
        for suffix in [".stderr", ".timing.jsonl", ".exit.json"] {
            assert!(dir.join(format!("{run}{suffix}")).is_file(), "{run}{suffix}");
        }
    }
    let summary = json_of(&dir.join(KEY).join("rip-summary.json"));
    assert_eq!(
        (&summary["ok"], &summary["result"], &summary["slug"], &summary["label"], &summary["title"], &summary["profile"], &summary["bytes"]),
        (&json!(true), &json!("ok"), &json!(id), &json!(KEY), &json!(0), &json!("job.mmcp.xml"), &json!(16))
    );
    assert!(summary.get("rip_index").is_none());
    assert_eq!(fs::read(h.staging().join(&id).join("rip").join(KEY).join("title_t00.mkv")).unwrap(), b"lazymkv-test-mkv");
    let record = h.record(&id);
    assert_eq!(record.plan.unwrap().mode, Mode::Direct);
    let title = &record.titles[0];
    assert_eq!((title.key.as_str(), title.scan_title, title.attempts, title.verified), (KEY, 0, 1, true));
    assert!(title.rip_reviews.is_empty());
}

#[test]
fn job_mmcp_xml_carries_the_all_tracks_selection_and_a_job_unique_name() {
    let h = harness(&[], json!({}));
    let id = run_to(&h, "Ejected");
    let xml = fs::read_to_string(h.job_dir(&id).join("job.mmcp.xml")).unwrap();
    assert!(xml.contains(&format!("<name lang=\"eng\">lazymkv-{id}</name>")));
    assert_eq!(xml.matches("defaultSelection=\"+sel:all,-sel:mvcvideo,=100:all\"").count(), 2);
}

#[test]
fn the_drive_listing_runs_again_before_every_makemkvcon_call() {
    let h = harness(&[], json!({}));
    h.settle();
    let before = runs(&h, "makemkvcon64", "0");
    run_to(&h, "Ejected");
    assert_eq!(runs(&h, "makemkvcon64", "0"), before + 2);
    assert_eq!((runs(&h, "makemkvcon64", "1"), runs(&h, "makemkvcon64", "2")), (1, 1));
}

#[test]
fn a_1009_on_the_first_scan_switches_every_later_call_to_fallback_mmcp_xml() {
    let h = harness(&["movie-direct-fallback"], json!({}));
    let id = run_to(&h, "Ejected");
    let dir = h.job_dir(&id);
    let xml = fs::read_to_string(dir.join("fallback.mmcp.xml")).unwrap();
    assert_eq!(xml.matches("defaultSelection=\"+sel:all,-sel:mvcvideo\"").count(), 2);
    assert!(xml.contains(&format!("<name lang=\"eng\">lazymkv-{id}</name>")));
    assert_eq!(h.record(&id).profile, "fallback.mmcp.xml");
    assert_eq!(json_of(&dir.join(KEY).join("rip-summary.json"))["profile"], json!("fallback.mmcp.xml"));
    assert_eq!((runs(&h, "makemkvcon64", "1"), runs(&h, "makemkvcon64", "2"), runs(&h, "makemkvcon64", "3")), (1, 1, 1));
    assert!(fs::read_to_string(dir.join("job.log")).unwrap().contains("Failed to load profile"));
}

#[test]
fn a_1009_on_both_scans_fails_scan_failed_unknown_quoting_makemkv() {
    let h = harness(&["movie-direct-fallback-twice"], json!({}));
    let id = run_to(&h, "Failed");
    let record = h.record(&id);
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Fingerprinted) });
    assert!(record.message.unwrap().contains("Failed to load profile"));
    assert_eq!(runs(&h, "makemkvcon64", "1"), 2);
    assert!(!h.job_dir(&id).join(KEY).exists());
}

#[test]
fn a_copy_plan_fails_plan_failed_naming_whole_disc_copy_mode_and_keeps_the_disc_in() {
    let h = harness(&["movie-direct-copy-plan"], json!({}));
    let id = run_to(&h, "Failed");
    h.settle();
    let record = h.record(&id);
    assert_eq!(record.plan.as_ref().unwrap().mode, Mode::Copy);
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Scanned) });
    assert!(record.message.unwrap().contains("whole-disc copy mode"));
    let failed = h.notifications().into_iter().last().unwrap();
    assert_eq!(failed.event, NotifyEvent::Failed { class: FailureClass::Unknown });
    assert!(failed.message.contains("whole-disc copy mode"));
    assert!(h.ejector().calls().is_empty());
    assert!(!h.staging().join(&id).exists());
}

#[test]
fn a_first_verify_failure_moves_the_attempt_aside_and_rips_again() {
    let h = harness(&["movie-direct-rerip"], json!({}));
    let id = run_to(&h, "Ejected");
    let title = h.job_dir(&id).join(KEY);
    assert!(title.join("attempt-1/rip.txt").is_file());
    assert!(title.join("attempt-1/mkvmerge.json").is_file());
    assert!(title.join("rip.txt").is_file() && !title.join("attempt-2").exists());
    let record = h.record(&id);
    assert_eq!((record.titles[0].attempts, record.titles[0].verified), (2, true));
    assert!(record.titles[0].rip_reviews.is_empty());
    assert!(events(&h, &id).contains(&Event::TitleVerifyFailed { second: false, last: true }));
}

#[test]
fn a_second_verify_failure_goes_on_and_records_verify_failed() {
    let h = harness(&["movie-direct-verify-twice"], json!({}));
    let id = run_to(&h, "Ejected");
    assert_eq!(h.record(&id).titles[0].rip_reviews, vec![ReviewReason::VerifyFailed]);
    let found: Vec<Event> = events(&h, &id).into_iter().filter(|e| matches!(e, Event::TitleVerifyFailed { .. })).collect();
    assert_eq!(found, vec![Event::TitleVerifyFailed { second: false, last: true }, Event::TitleVerifyFailed { second: true, last: true }]);
}

#[test]
fn index_drift_rescans_as_rescan_2_and_rips_the_mapped_index() {
    let h = harness(&["movie-direct-drift"], json!({}));
    let id = run_to(&h, "Ejected");
    let dir = h.job_dir(&id);
    assert!(dir.join("rescan-2.txt").is_file() && !dir.join("rescan-3.txt").exists());
    let summary = json_of(&dir.join(KEY).join("rip-summary.json"));
    assert_eq!((&summary["title"], &summary["rip_index"]), (&json!(0), &json!(1)));
    assert_eq!(h.record(&id).titles[0].rip_index, Some(1));
}

#[test]
fn a_third_mismatch_fails_the_title_as_unknown() {
    let h = harness(&["movie-direct-drift-thrice"], json!({}));
    let id = run_to(&h, "Failed");
    let record = h.record(&id);
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Ripping) });
    assert_eq!(record.message.as_deref(), Some("the index guard found t00-00800 moved three times"));
    let dir = h.job_dir(&id);
    assert!(dir.join("rescan-2.txt").is_file() && dir.join("rescan-3.txt").is_file() && !dir.join("rescan-4.txt").exists());
}

#[test]
fn short_staging_space_waits_in_awaiting_space() {
    let h = harness(&[], json!({}));
    h.shares().set_free(&root(h.staging()), 100);
    let id = run_to(&h, "AwaitingSpace");
    h.settle();
    assert_eq!(h.record(&id).state, State::AwaitingSpace { return_to: Box::new(State::Planned { mode: Mode::Direct }) });
    assert!(h.notifications().iter().any(|n| n.event == NotifyEvent::Waiting { reason: WaitReason::Space }));
}

#[test]
fn an_unreachable_root_at_check_space_feeds_share_lost() {
    let h = harness(&[], json!({}));
    h.shares().set_reach(&root(h.staging()), Reach::Unreachable("gone".into()));
    let id = run_to(&h, "AwaitingShare");
    assert_eq!(h.record(&id).state, State::AwaitingShare { return_to: Box::new(State::Planned { mode: Mode::Direct }) });
    assert!(events(&h, &id).contains(&Event::ShareLost));
}

#[test]
fn no_java_records_label_rule_unavailable() {
    let h = harness(&[], json!({"tool_paths": {"java": null}}));
    let id = run_to(&h, "Ejected");
    let notes = h.record(&id).notes;
    let noted = notes.iter().find(|n| n.note == Note::LabelRuleUnavailable).expect("label_rule_unavailable");
    assert_eq!((noted.stream, noted.detail.as_str()),
        (None, "no Java runtime was found, so label rule 8.3.2 does not apply (spec 10.2)"));
    assert_eq!(events(&h, &id).iter().filter(|e| **e == Event::ToolMissing { tool: Tool::Java }).count(), 1);
}

#[test]
fn the_scan_records_its_version_and_java_use_in_job_log() {
    let h = harness(&["movie-direct-java"], json!({}));
    let id = run_to(&h, "Ejected");
    assert!(fs::read_to_string(h.job_dir(&id).join("job.log")).unwrap().contains("scan: MakeMKV 2.0.0; java in use: true"));
    let h = harness(&[], json!({}));
    let id = run_to(&h, "Ejected");
    assert!(fs::read_to_string(h.job_dir(&id).join("job.log")).unwrap().contains("scan: MakeMKV 2.0.0; java in use: false"));
}

#[test]
fn removing_the_disc_mid_rip_kills_makemkvcon_and_feeds_only_media_removed() {
    let h = harness(&["movie-direct-hang"], json!({}));
    let id = run_to(&h, "Ripping");
    let rip = h.job_dir(&id).join(KEY).join("rip.txt");
    h.wait("the rip runs", |_| rip.is_file());
    h.remove("tok0");
    h.wait("MediaRemoved", |h| events(h, &id).contains(&Event::MediaRemoved));
    let exit = h.job_dir(&id).join(KEY).join("rip.txt.exit.json");
    h.wait("the killed rip's exit file", |_| exit.is_file());
    h.settle();
    assert_ne!(json_of(&exit)["exit_code"], json!(0));
    let found = events(&h, &id);
    assert!(!found.iter().any(|e| matches!(e, Event::TitleRipFailed { .. })), "{found:?}");
    let last = h.transitions(&id).pop().unwrap();
    assert_eq!((last.event, last.new), (Some(Event::MediaRemoved), State::Ripping));
}

#[test]
fn a_disc_name_in_windows_1252_reaches_ejected_with_scan_txt_unchanged() {
    let h = harness(&["movie-direct-cp1252"], json!({}));
    let id = run_to(&h, "Ejected");
    let scenario = fs::read(h.tools().join("scan.txt")).unwrap();
    assert!(String::from_utf8(scenario.clone()).is_err(), "the variant's scan.txt holds the raw byte 0xE9");
    assert_eq!(fs::read(h.job_dir(&id).join("scan.txt")).unwrap(), scenario);
    assert_eq!(h.record(&id).plan.unwrap().mode, Mode::Direct);
    // No engine output carries CINFO 2, so this pins the planner's reading of the recorded bytes, Step 4 item 5's calls.
    let read = lazymkv_core::disc::disc(&lazymkv_core::robot::parse(&String::from_utf8_lossy(&scenario))).unwrap();
    assert_eq!(read.name.as_deref(), Some("Synthetic Movie Caf\u{fffd}"));
}

#[test]
fn no_makemkvcon_waits_in_awaiting_tool() {
    let h = harness(&[], json!({"tool_paths": {"makemkvcon": null}}));
    let id = run_to(&h, "AwaitingTool");
    h.settle();
    assert_eq!(h.record(&id).state, State::AwaitingTool { return_to: Box::new(State::Fingerprinted) });
    assert!(h.notifications().iter().any(|n| n.event == NotifyEvent::Waiting { reason: WaitReason::Tool }));
    assert!(!h.job_dir(&id).join("scan.txt").exists());
}

#[test]
fn a_makemkvcon_below_the_minimum_version_waits_in_awaiting_tool() {
    let h = harness(&[], json!({"makemkv_min_version": "2.0.1"}));
    let id = run_to(&h, "AwaitingTool");
    let record = h.record(&id);
    assert_eq!(record.state, State::AwaitingTool { return_to: Box::new(State::Fingerprinted) });
    assert_eq!(record.message.as_deref(), Some("makemkvcon64 is version 2.0.0, below makemkv_min_version 2.0.1"));
    assert!(!h.job_dir(&id).join("scan.txt").exists());
}

#[test]
fn a_scan_licence_message_waits_in_awaiting_makemkv() {
    let h = harness(&["movie-direct-licence"], json!({}));
    let id = run_to(&h, "AwaitingMakeMkv");
    h.settle();
    assert_eq!(h.record(&id).state, State::AwaitingMakeMkv { return_to: Box::new(State::Fingerprinted) });
    let waiting = h.notifications().into_iter().last().unwrap();
    assert_eq!(waiting.event, NotifyEvent::Waiting { reason: WaitReason::Makemkv });
    assert!(waiting.message.contains("Evaluation period has expired"));
}

#[test]
fn a_licence_message_during_a_rip_waits_in_awaiting_makemkv() {
    let h = harness(&["movie-direct-rip-licence"], json!({}));
    let id = run_to(&h, "AwaitingMakeMkv");
    assert_eq!(h.record(&id).state, State::AwaitingMakeMkv { return_to: Box::new(State::Ripping) });
    assert!(events(&h, &id).contains(&Event::LicenceMessage));
}

#[test]
fn the_eject_fake_failure_sends_eject_failed_and_ends_the_disc_phase() {
    let h = harness(&[], json!({}));
    h.ejector().fail_next("stuck");
    let id = run_to(&h, "Ejected");
    h.settle();
    assert!(events(&h, &id).contains(&Event::EjectFailed));
    let sent: Vec<NotifyEvent> = h.notifications().into_iter().map(|n| n.event).collect();
    assert!(sent.contains(&NotifyEvent::EjectFailed) && !sent.contains(&NotifyEvent::DiscDone));
    assert_eq!(h.awake().holders(), 0);
}

#[test]
fn the_removal_its_own_eject_causes_is_not_fed() {
    let h = harness(&[], json!({}));
    let id = run_to(&h, "Ejected");
    h.wait("removal seen", |h| h.drives_json()["drives"]["tok0"]["fingerprint"] == json!(null));
    h.settle();
    assert!(!events(&h, &id).contains(&Event::MediaRemoved));
    assert_eq!(h.ejector().calls(), vec!["tok0".to_string()]);
}

#[test]
fn the_scan_does_not_wait_for_a_slow_lookup() {
    let h = harness(&[], json!({}));
    h.scan_waits_for_lookups(false);
    let gate = h.http().gate(&lazymkv_clients::dischash_url(&h.fingerprint("movie-direct")));
    let id = run_to(&h, "Ejected");
    assert!(h.record(&id).plan.unwrap().identity.lookup_pending);
    gate.open();
}

#[test]
fn a_remembered_decision_is_applied_and_copied_as_remembered_json() {
    let h = harness(&[], json!({}));
    let fp = h.fingerprint("movie-direct");
    let text = format!(concat!(
        r#"{{"format":"lazymkv remembered v1","fingerprint":"{fp}","tmdb":{{"kind":"movie","id":900001}},"titles":[{{"#,
        r#""source_file":"00800.mpls","dvd_title":null,"segment_map":"1","seconds":7150,"role":{{"role":"feature"}},"#,
        r#""audio":[],"subtitles":[]}}]}}"#), fp = fp);
    fs::create_dir_all(h.data().join("remembered")).unwrap();
    fs::write(h.data().join("remembered").join(format!("{fp}.json")), &text).unwrap();
    let id = run_to(&h, "Ejected");
    let plan = h.record(&id).plan.unwrap();
    assert_eq!((plan.identity.step, plan.identity.confidence), (IdentityStep::Remembered, 1.0));
    assert_eq!(fs::read(h.job_dir(&id).join("remembered.json")).unwrap(), text.as_bytes());
}

#[test]
fn a_remembered_file_of_another_major_version_is_ignored_and_reported_in_engine_log() {
    let h = harness(&[], json!({}));
    let fp = h.fingerprint("movie-direct");
    fs::create_dir_all(h.data().join("remembered")).unwrap();
    fs::write(h.data().join("remembered").join(format!("{fp}.json")),
        format!(r#"{{"format":"lazymkv remembered v2","fingerprint":"{fp}","tmdb":{{"kind":"movie","id":900001}},"titles":[]}}"#)).unwrap();
    let id = run_to(&h, "Ejected");
    assert_ne!(h.record(&id).plan.unwrap().identity.step, IdentityStep::Remembered);
    assert!(!h.job_dir(&id).join("remembered.json").exists());
    assert!(h.engine_log().contains("lazymkv remembered v2"));
}

#[test]
fn with_tuning_evidence_on_every_makemkvcon_call_gets_debug_and_its_log_is_saved() {
    let h = harness(&[], json!({"keep_tuning_evidence": true}));
    let log = h.tools().join("MakeMKV_log.txt");
    fs::write(&log, b"debug KEY-123 line\n").unwrap();
    // FakeHost's clock is fixed, so dating the log 60 s after it makes the log newer than every call (S61).
    let after_start = std::time::UNIX_EPOCH + std::time::Duration::from_secs(h.host().now_unix() + 60);
    fs::File::options().write(true).open(&log).unwrap().set_modified(after_start).unwrap();
    h.host().set_debug_log(&log);
    h.host().set_app_key("KEY-123");
    let id = run_to(&h, "Ejected");
    let dir = h.job_dir(&id);
    let job_log = fs::read_to_string(dir.join("job.log")).unwrap();
    let makemkv_runs: Vec<&str> =
        job_log.lines().filter(|line| line.starts_with("run ") && line.contains("makemkvcon64")).collect();
    // The listing before the scan, the scan, the listing before the rip, the rip.
    assert_eq!(makemkv_runs.len(), 4, "{job_log}");
    assert!(makemkv_runs.iter().all(|line| line.split(' ').any(|arg| arg == "--debug")), "{job_log}");
    for run in ["drive-list.txt", "drive-list-2.txt", "scan.txt", "t00-00800/rip.txt"] {
        assert_eq!(fs::read(dir.join(format!("{run}.makemkv-debug.txt"))).unwrap(), b"debug [app_Key removed] line\n", "{run}");
    }
}

#[test]
fn a_missing_debug_log_only_writes_a_job_log_line() {
    let h = harness(&[], json!({"keep_tuning_evidence": true}));
    let log = h.tools().join("MakeMKV_log.txt");
    h.host().set_debug_log(&log);
    let id = run_to(&h, "Ejected");
    let dir = h.job_dir(&id);
    let job_log = fs::read_to_string(dir.join("job.log")).unwrap();
    let line = format!("scan.txt: the MakeMKV debug log was not saved: {}: no debug log was written", log.display());
    assert!(job_log.contains(&line), "{job_log}");
    assert!(!dir.join("scan.txt.makemkv-debug.txt").exists());
    assert!(events(&h, &id).contains(&Event::Ejected));
}

/// RP-1: every later listing is recorded as drive-list-<n>.txt.
#[test]
fn later_drive_listings_are_recorded_as_drive_list_n() {
    let h = harness(&[], json!({}));
    let id = run_to(&h, "Ejected");
    let dir = h.job_dir(&id);
    for run in ["drive-list.txt", "drive-list-2.txt"] {
        for suffix in ["", ".stderr", ".timing.jsonl", ".exit.json"] {
            assert!(dir.join(format!("{run}{suffix}")).is_file(), "{run}{suffix}");
        }
    }
    assert!(!dir.join("drive-list-3.txt").exists());
    let log = fs::read_to_string(dir.join("job.log")).unwrap();
    assert!(log.contains("drive listing 1: tok0 is index 0") && log.contains("drive listing 2: tok0 is index 0"), "{log}");
}

/// RP-2: a listing with no DRV line for the job's drive fails the scan as Unknown.
#[test]
fn a_missing_drive_index_fails_the_scan_naming_the_drive() {
    let h = harness(&[], json!({}));
    h.insert("tok1", "movie-direct");
    let id = format!("{}-1", h.fingerprint("movie-direct").to_ascii_lowercase());
    h.wait("Failed", |h| h.transitions(&id).iter().any(|line| line.new.as_str() == "Failed"));
    let record = h.record(&id);
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Fingerprinted) });
    assert_eq!(record.message.as_deref(), Some("MakeMKV's drive listing has no drive tok1"));
    assert!(!h.job_dir(&id).join("scan.txt").exists());
}

/// RP-3: a missing appdata.tar waits in AwaitingTool.
#[test]
fn a_missing_appdata_tar_waits_for_makemkvcon() {
    let h = harness(&[], json!({}));
    fs::remove_file(h.tools().join("appdata.tar")).unwrap();
    let id = run_to(&h, "AwaitingTool");
    let record = h.record(&id);
    assert_eq!(record.state, State::AwaitingTool { return_to: Box::new(State::Fingerprinted) });
    let message = record.message.unwrap_or_default();
    assert!(message.contains("appdata.tar"), "{message}");
    assert!(!h.job_dir(&id).join("job.mmcp.xml").exists() && !h.job_dir(&id).join("scan.txt").exists());
}

/// RP-5: the engine, not the driver, retries a scan past its limit, after a fresh drive listing. The hanging scan is
/// killed only at the harness's 10 s scan limit, so this test takes more than 10 s and waits with `run_until`'s 60 s
/// cap (PF-5).
#[test]
fn a_scan_past_its_limit_is_killed_listed_again_and_retried_once() {
    let h = harness(&["movie-direct-scan-hang"], json!({}));
    h.settle();
    let before = runs(&h, "makemkvcon64", "0");
    h.insert("tok0", "movie-direct");
    h.run_until("Ejected");
    let id = format!("{}-1", h.fingerprint("movie-direct").to_ascii_lowercase());
    // The hanging scan and the retry, both counted under their first matching response.
    assert_eq!(runs(&h, "makemkvcon64", "1"), 2);
    assert_eq!(runs(&h, "makemkvcon64", "3"), 1, "one rip");
    assert_eq!(runs(&h, "makemkvcon64", "0"), before + 3, "a listing before each scan and before the rip");
    let log = fs::read_to_string(h.job_dir(&id).join("job.log")).unwrap();
    assert_eq!(log.lines().filter(|line| line.starts_with("run scan.txt:")).count(), 2, "{log}");
}
