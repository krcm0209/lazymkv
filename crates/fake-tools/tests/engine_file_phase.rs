//! The file phase on stand-ins, from the slot to the filed file (design parts 5.7, 6.4, 6.5, 8.3, 8.4).

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use common::Client;
use lazymkv_core::job::{Action, Event, Failure, FailureClass, Held, NotifyEvent, OutputCause, State, Tool, WaitReason};
use lazymkv_core::lookup::tmdb::{TmdbIdentity, TmdbKind};
use lazymkv_core::lookup::IdentityStep;
use lazymkv_core::review::{Note, Noted, ReviewReason};
use lazymkv_dev::cmd::decide::{decide_doc, read_decide_inputs, rec_folder, DecideInputs, DecisionEntry};
use lazymkv_dev::cmd::plan::PlanOptions;
use lazymkv_dev::evidence::Ctx;
use lazymkv_dev::pyjson::{dumps, Style};
use lazymkv_engine::testkit::ci_factor;
use lazymkv_engine::testkit::Harness;
use lazymkv_platform::Spawn;
use lazymkv_platform::{Host, Reach, SecretStore};
use lazymkv_tools::exe_name;
use lazymkv_tools::ffprobe::packets_args;
use lazymkv_tools::mkvmerge::extract_args;
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

/// Every file under `dir`, relative to it with '/' separators; empty when `dir` does not exist.
fn files_under(dir: &Path) -> BTreeSet<String> {
    fn walk(root: &Path, dir: &Path, found: &mut BTreeSet<String>) {
        let Ok(entries) = fs::read_dir(dir) else { return };
        for entry in entries {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, found);
            } else {
                let relative = path.strip_prefix(root).unwrap().to_str().unwrap().replace('\\', "/");
                found.insert(relative);
            }
        }
    }
    let mut found = BTreeSet::new();
    walk(dir, dir, &mut found);
    found
}

/// The rips the job asked of makemkvcon: each spawn whose arguments hold "mkv".
fn rips(h: &Harness) -> Vec<String> {
    h.spawns().into_iter().filter(|s| s.args.iter().any(|a| a == "mkv")).map(|s| s.args.join(" ")).collect()
}

fn final_path(h: &Harness) -> PathBuf {
    let folder = "Synthetic Movie (2001) {tmdb-900001}";
    h.library().join(folder).join(format!("{folder} - 1080p.mkv"))
}

/// Rewrites `<job>/<file>` with its `fetched_at` set to 2026-01-01, past the six-month limit on the harness's date
/// (2026-10-23).
fn stale(h: &Harness, id: &str, file: &str) {
    let path = h.job_dir(id).join(file);
    let mut copy = json_of(&path);
    copy["fetched_at"] = json!("2026-01-01");
    fs::write(&path, serde_json::to_vec_pretty(&copy).unwrap()).unwrap();
}

/// Removes the harness's `tmdb_token`, so the engine has none.
fn no_token(h: &Harness) {
    h.secrets().forget("tmdb_token");
}

/// The level folder step 10a writes for movie-direct: main stream 1, no candidate (levels-args' first line).
const LEVELS: &str = "audio-levels-20ms-1";

/// The files of movie-direct's level folder in the job folder, in name order: phase 0's names for a main stream with
/// a centre channel and no candidate, without the audio-proxy.json that only the dataset copy adds (T5-R13).
const LEVEL_FILES: [&str; 10] = [
    "command.txt", "ffmpeg.txt", "ffprobe.txt", "graph.txt", "main-fc.txt", "main-mix.txt", "manifest.txt",
    "proxy-check.json", "streamhash.txt", "streams.json",
];

/// movie-direct's one audio stream's packet MD5 (corpus/scenarios/movie-direct/streamhash-1.txt).
const MD5: &str = "0f343b0931126a20f133d67c2b018a3b";

/// The harness's tuning evidence on (its defaults keep it off, R5-code-79).
fn evidence_on() -> Value {
    json!({"keep_tuning_evidence": true})
}

/// `<staging>/<job id>/tuning/<KEY>`, the staged tuning folder (design part 7.5).
fn staged(h: &Harness, id: &str) -> PathBuf {
    h.staging().join(id).join("tuning").join(KEY)
}

/// `<data>/work/levels-<job id>-<KEY>`, step 10a's work folder (T5-R39).
fn work(h: &Harness, id: &str) -> PathBuf {
    h.data().join("work").join(format!("levels-{id}-{KEY}"))
}

fn job_log(h: &Harness, id: &str) -> String {
    fs::read_to_string(h.job_dir(id).join("job.log")).unwrap()
}

/// The spawns whose arguments hold every needle, each as a substring of some argument (as the stand-ins match).
fn spawns_holding(h: &Harness, needles: &[&str]) -> Vec<Spawn> {
    h.spawns().into_iter().filter(|s| needles.iter().all(|n| s.args.iter().any(|a| a.contains(n)))).collect()
}

/// R-B1: decide.json without its two extra keys is what dev's readers render over the finished job folder.
fn assert_dev_renders_decide_json(h: &Harness, id: &str) {
    let dir = h.job_dir(id);
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

/// The made-up word of every stand-in record and of the panicking stand-in's standard error (RP21: never subtitle text).
const SENTINEL: &str = "text-sentinel-3e8f";
/// The refused stand-in's first standard error line, the reason its exit 3 gives (T5-R8).
const REFUSAL: &str = "content encoding 3 of track 3 is not zlib compression or header stripping";
/// The cap scenarios with the harness's override of 2 s (T5-R58).
const CAP: [&str; 2] = ["movie-direct-two-subs-cap", "movie-direct-two-subs-cap-2s"];

/// movie-direct with `movie-direct-two-subs` and `overlays` copied over it.
fn two_subs(overlays: &[&str], settings: Value) -> Harness {
    let mut variants = vec!["movie-direct-two-subs"];
    variants.extend_from_slice(overlays);
    harness(&variants, settings)
}

fn job_id(h: &Harness) -> String {
    format!("{}-1", h.fingerprint("movie-direct").to_ascii_lowercase())
}

/// Inserts the disc and stops the engine once the job's DecisionDone is persisted, before its actions run.
fn decided(h: &Harness) -> String {
    h.insert("tok0", "movie-direct");
    h.run_until_event("DecisionDone", |event| matches!(event, Event::DecisionDone { .. }));
    job_id(h)
}

/// A file at the final path holds the job with path_collision, so it waits in NeedsReview with its engine running.
fn held(h: &Harness) -> String {
    let target = final_path(h);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(&target, b"already here").unwrap();
    run_to(h, "NeedsReview")
}

/// `<staging>/<job id>/tuning/<key>/subs-ocr`, where the text lives (design part 10.12 item 1).
fn staged_ocr(h: &Harness, id: &str) -> PathBuf {
    h.staging().join(id).join("tuning").join(KEY).join("subs-ocr")
}

/// `<data>/work/ocr-<job id>-<key>`, the title's work folder (B1 (the worker's folder), T5-R4).
fn work_of(h: &Harness, id: &str) -> PathBuf {
    h.data().join("work").join(format!("ocr-{id}-{KEY}"))
}

fn unread_notes(h: &Harness, id: &str) -> Vec<Noted> {
    let decision = h.record(id).titles[0].decision.clone().expect("the title was decided");
    decision.notes.into_iter().filter(|noted| noted.note == Note::SubtitleTextUnread).collect()
}

fn unread(stream: u32, reason: &str) -> Noted {
    Noted { note: Note::SubtitleTextUnread, stream: Some(stream), detail: format!("stream {stream} was not read: {reason}") }
}

/// The reason of a run killed at the cap scenarios' override after `read` of its 10 events.
fn cap_reason_of(read: usize) -> String {
    let cap = 2.0 * f64::from(ci_factor());
    format!("time cap of {cap:.1} s reached after {read} of 10 events")
}

/// Every file under `dir` whose bytes hold `needle`.
fn holding(dir: &Path, needle: &str) -> Vec<String> {
    let found = |name: &String| fs::read(dir.join(name)).unwrap().windows(needle.len()).any(|w| w == needle.as_bytes());
    files_under(dir).into_iter().filter(found).collect()
}

fn ocr_spawns(h: &Harness) -> Vec<Spawn> {
    h.spawns().into_iter().filter(|spawn| spawn.program.file_stem().is_some_and(|stem| stem == "lazymkv-ocr")).collect()
}

fn replay_files(dir: &Path) -> PlanOptions {
    PlanOptions {
        settings: Some(dir.join("settings.json")),
        tmdb_search: Some(dir.join("tmdb-search.json")),
        tmdb_record: Some(dir.join("tmdb-record.json")),
        remembered: None,
    }
}

/// `lazymkv-dev decide`'s inputs over the finished job folder, with `text` as its `--text`.
fn read_back(h: &Harness, id: &str, text: Option<&Path>) -> DecideInputs {
    let dir = h.job_dir(id);
    let ctx = Ctx::for_test(h.data());
    let (rec, label) = rec_folder(&ctx, &dir.join(KEY).display().to_string()).unwrap_or_else(|r| panic!("{}", r.line));
    read_decide_inputs(&ctx, &rec, &label, text, None, &replay_files(&dir)).unwrap_or_else(|r| panic!("{}", r.line))
}

fn set(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|name| name.to_string()).collect()
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
    assert!(!job_log(&h, &id).contains("step 10a did not run in this version"));
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
fn step_10a_writes_the_level_folder_and_the_proxy_into_the_staged_folder_and_the_level_folder_into_the_job_folder() {
    let h = harness(&[], evidence_on());
    let id = run_to(&h, "Filed");
    let (levels, tuning) = (h.job_dir(&id).join(KEY).join(LEVELS), staged(&h, &id));
    assert_eq!(files_under(&levels), LEVEL_FILES.iter().map(|name| name.to_string()).collect::<BTreeSet<_>>());
    let mut expected: BTreeSet<String> = LEVEL_FILES.iter().map(|name| format!("{LEVELS}/{name}")).collect();
    expected.extend([format!("{LEVELS}/audio-proxy.json"), "audio-proxy.mka".to_string(), "audio-proxy.json".to_string()]);
    assert_eq!(files_under(&tuning), expected);
    for name in LEVEL_FILES {
        assert_eq!(fs::read(levels.join(name)).unwrap(), fs::read(tuning.join(LEVELS).join(name)).unwrap(), "{name}");
    }
    assert_eq!(fs::read(tuning.join("audio-proxy.mka")).unwrap(), b"lazymkv-test-proxy");
    assert_eq!(fs::read(tuning.join(LEVELS).join("audio-proxy.json")).unwrap(), fs::read(tuning.join("audio-proxy.json")).unwrap());
    let proxy = json_of(&tuning.join("audio-proxy.json"));
    assert_eq!((&proxy["format"], &proxy["slug"], &proxy["label"]), (&json!("lazymkv audio proxy v1"), &json!(id), &json!(KEY)));
    assert_eq!((&proxy["encoding"]["gain_db"], &proxy["proxy"]["file"]), (&json!(-18), &json!("audio-proxy.mka")));
    let tracks: Vec<(&Value, &Value)> =
        proxy["tracks"].as_array().unwrap().iter().map(|t| (&t["source_stream"], &t["series"])).collect();
    assert_eq!(tracks, [(&json!(1), &json!("mix")), (&json!(1), &json!("fc"))]);
    assert_eq!(fs::read_to_string(levels.join("streamhash.txt")).unwrap(), format!("0,a,MD5={MD5}\n"));
    let mix = fs::read_to_string(levels.join("main-mix.txt")).unwrap();
    assert_eq!(mix.split("\r\n").skip(1).collect::<Vec<_>>(), ["-20.1", "-21.4", "-100.0", "-35.6", "-19.9", ""]);
    assert!(!h.job_dir(&id).join(KEY).join("streamhash.txt").exists(), "the bare copy is the setting-off pass's");
    let pass = spawns_holding(&h, &["-/filter_complex"]);
    assert_eq!(pass.len(), 1);
    assert!(pass[0].below_normal && pass[0].args.iter().any(|a| a == "graph.txt"));
    assert_eq!(pass[0].cwd.as_deref(), Some(work(&h, &id).as_path()));
    assert!(!work(&h, &id).exists());
    assert_eq!(fs::read_dir(h.data().join("work")).unwrap().count(), 0);
}

#[test]
fn the_staged_level_folders_audio_proxy_json_is_written_last() {
    let h = harness(&[], evidence_on());
    let id = format!("{}-1", h.fingerprint("movie-direct").to_ascii_lowercase());
    // A folder where the copy beside audio-proxy.mka lands: its rename fails, so the copies stop there.
    let blocker = staged(&h, &id).join("audio-proxy.json");
    h.on_spawn(&["-/filter_complex"], move || fs::create_dir_all(&blocker).unwrap());
    h.insert("tok0", "movie-direct");
    h.wait("DecisionFailed", |h| events(h, &id).iter().any(|e| matches!(e, Event::DecisionFailed { .. })));
    let tuning = staged(&h, &id);
    for name in LEVEL_FILES {
        assert!(tuning.join(LEVELS).join(name).is_file(), "{name}");
    }
    assert!(tuning.join("audio-proxy.mka").is_file());
    assert!(!tuning.join(LEVELS).join("audio-proxy.json").exists(), "C3's marker comes after every other file");
    assert!(!h.job_dir(&id).join(KEY).join(LEVELS).join("audio-proxy.json").exists());
}

#[test]
fn with_the_setting_off_step_10a_writes_only_streamhash_txt_in_both_places() {
    let h = harness(&[], json!({}));
    let id = run_to(&h, "Filed");
    let key_dir = h.job_dir(&id).join(KEY);
    assert_eq!(fs::read_to_string(key_dir.join("streamhash.txt")).unwrap(), format!("0,a,MD5={MD5}\n"));
    assert_eq!(files_under(&staged(&h, &id)), BTreeSet::from(["streamhash.txt".to_string()]));
    assert_eq!(fs::read(staged(&h, &id).join("streamhash.txt")).unwrap(), fs::read(key_dir.join("streamhash.txt")).unwrap());
    assert!(fs::read_dir(&key_dir).unwrap().all(|e| !e.unwrap().file_name().to_string_lossy().starts_with("audio-levels-")));
    let raw = h.staging().join(&id).join("rip").join(KEY).join("title_t00.mkv");
    let pass = spawns_holding(&h, &["0:a", "streamhash"]);
    assert_eq!(pass.len(), 1);
    assert_eq!(
        pass[0].args,
        ["-i", raw.to_str().unwrap(), "-map", "0:a", "-c", "copy", "-f", "streamhash", "-hash", "md5", "streamhash.txt"]
    );
    assert!(pass[0].below_normal);
    assert_eq!(pass[0].cwd.as_deref(), Some(work(&h, &id).as_path()));
    assert!(spawns_holding(&h, &["-/filter_complex"]).is_empty());
}

#[test]
fn with_the_setting_off_the_streamhash_pass_is_recorded_beside_streamhash_txt() {
    let h = harness(&[], json!({}));
    let id = run_to(&h, "Filed");
    let key_dir = h.job_dir(&id).join(KEY);
    for suffix in ["", ".stderr", ".timing.jsonl", ".exit.json"] {
        assert!(key_dir.join(format!("streamhash-ffmpeg.txt{suffix}")).is_file(), "{suffix}");
    }
    assert_eq!(json_of(&key_dir.join("streamhash-ffmpeg.txt.exit.json"))["exit_code"], json!(0));
    let log = job_log(&h, &id);
    assert!(log.contains(&format!("run {KEY}/streamhash-ffmpeg.txt: ")), "{log}");
    assert!(log.contains(&format!("ended {KEY}/streamhash-ffmpeg.txt: exit 0 after ")), "{log}");
    assert!(!staged(&h, &id).join("streamhash-ffmpeg.txt").exists(), "the capture is the job folder's only");
}

#[test]
fn a_proxy_that_clips_at_minus_18_is_made_again_at_minus_30_and_nothing_else_changes() {
    let plain = harness(&[], evidence_on());
    let plain_id = run_to(&plain, "Filed");
    let h = harness(&["movie-direct-clip"], evidence_on());
    let id = run_to(&h, "Filed");
    let tuning = staged(&h, &id);
    assert_eq!(fs::read(tuning.join("audio-proxy.mka")).unwrap(), b"lazymkv-test-proxy-redo");
    let proxy = json_of(&tuning.join("audio-proxy.json"));
    assert_eq!((&proxy["encoding"]["gain_db"], &proxy["proxy"]["file"]), (&json!(-30), &json!("audio-proxy.mka")));
    let passes = spawns_holding(&h, &["-/filter_complex"]);
    assert_eq!(passes.len(), 2, "the first pass and the redo");
    assert!(passes[0].args.iter().any(|a| a == "null") && !passes[1].args.iter().any(|a| a == "null"), "only the first measures");
    assert_eq!(passes[1].cwd.as_deref(), Some(work(&h, &id).join("redo").as_path()));
    let (levels, plain_levels) = (h.job_dir(&id).join(KEY).join(LEVELS), plain.job_dir(&plain_id).join(KEY).join(LEVELS));
    for name in ["main-mix.txt", "main-fc.txt", "streamhash.txt", "graph.txt"] {
        assert_eq!(fs::read(levels.join(name)).unwrap(), fs::read(plain_levels.join(name)).unwrap(), "{name}");
    }
    let audio = |h: &Harness, id: &str| json_of(&h.job_dir(id).join(KEY).join("decide.json"))["audio"].clone();
    assert_eq!(audio(&h, &id), audio(&plain, &plain_id));
    let line = format!("{KEY}: the audio proxy of stream 1 (mix) clips at -18 dB, so the title's proxy is made again at -30 dB");
    assert!(job_log(&h, &id).contains(&line));
}

#[test]
fn a_proxy_that_clips_at_minus_30_too_keeps_no_proxy_and_job_log_names_the_stream() {
    let h = harness(&["movie-direct-clip", "movie-direct-clip-twice"], evidence_on());
    let id = run_to(&h, "Filed");
    let tuning = staged(&h, &id);
    assert!(!tuning.join("audio-proxy.mka").exists());
    let proxy = json_of(&tuning.join("audio-proxy.json"));
    assert_eq!((&proxy["proxy"], &proxy["gains_tried"], &proxy["encoding"]["gain_db"]), (&Value::Null, &json!([-18, -30]), &json!(-30)));
    assert_eq!(
        fs::read(tuning.join(LEVELS).join("audio-proxy.json")).unwrap(),
        fs::read(tuning.join("audio-proxy.json")).unwrap(),
        "the staged folder reads complete (call 22)"
    );
    let line = format!("{KEY}: the audio proxy of stream 1 (mix) clips at -30 dB too, so the dataset gets no proxy for this title");
    assert!(job_log(&h, &id).contains(&line));
    assert_eq!(spawns_holding(&h, &["-/filter_complex"]).len(), 2, "no third pass");
}

#[test]
fn step_10_reads_before_step_10a_decodes() {
    let h = harness(&["movie-direct-two-subs"], json!({}));
    let id = format!("{}-1", h.fingerprint("movie-direct").to_ascii_lowercase());
    h.insert("tok0", "movie-direct");
    h.wait("DecisionDone", |h| events(h, &id).iter().any(|e| matches!(e, Event::DecisionDone { .. })));
    let spawns = h.spawns();
    let worker = |s: &Spawn| s.program.file_stem().is_some_and(|stem| stem == "lazymkv-ocr");
    let last_read = spawns.iter().rposition(worker).expect("the worker ran");
    let pass = spawns.iter().position(|s| s.args.iter().any(|a| a == "streamhash")).expect("step 10a ran");
    assert!(last_read < pass, "worker {last_read}, pass {pass}");
    assert!(h.job_dir(&id).join(KEY).join("streamhash.txt").is_file());
}

#[test]
fn the_decision_carries_the_packet_md5s_and_the_measures_of_step_10a() {
    let h = harness(&[], evidence_on());
    let id = run_to(&h, "Filed");
    let doc = json_of(&h.job_dir(&id).join(KEY).join("decide.json"));
    assert_eq!(doc["decision"]["facts"]["md5_streams"], json!([1]));
    assert_eq!(doc["audio"].as_array().unwrap().len(), 1);
    assert_eq!((&doc["audio"][0]["folder"], &doc["audio"][0]["frames"]), (&json!(LEVELS), &json!(5)));
    assert_dev_renders_decide_json(&h, &id);
    let off = harness(&[], json!({}));
    let off_id = run_to(&off, "Filed");
    let off_doc = json_of(&off.job_dir(&off_id).join(KEY).join("decide.json"));
    assert_eq!((&off_doc["decision"]["facts"]["md5_streams"], &off_doc["audio"]), (&json!([1]), &json!([])));
    assert_dev_renders_decide_json(&off, &off_id);
}

#[test]
fn after_a_restart_the_decision_reads_them_back_from_the_job_folder() {
    let first = harness(&[], evidence_on());
    let first_id = run_to(&first, "Filed");
    let expected = json_of(&first.job_dir(&first_id).join(KEY).join("decide.json"));
    let h = harness(&[], evidence_on());
    h.insert("tok0", "movie-direct");
    h.run_until("Decided");
    let id = h.job_ids().pop().unwrap();
    h.restart(); // resume(Decided) runs Decide alone, over read_back_inputs (design part 5.10)
    h.wait("Filed", |h| h.state_of(&id) == "Filed");
    let doc = json_of(&h.job_dir(&id).join(KEY).join("decide.json"));
    assert_eq!((&doc["audio"], &doc["decision"]["facts"]), (&expected["audio"], &expected["decision"]["facts"]));
    assert!(job_log(&h, &id).contains(&format!("{KEY}: decide inputs read back from the job folder")));
    assert_eq!(spawns_holding(&h, &["-/filter_complex"]).len(), 1, "the restart decodes nothing again");
}

#[test]
fn a_failed_ffmpeg_pass_is_decision_failed_with_its_class() {
    let h = harness(&["movie-direct-ffmpeg-error"], json!({}));
    let id = run_to(&h, "Failed");
    let record = h.record(&id);
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Inspecting) });
    assert_eq!(
        record.message.as_deref(),
        Some("t00-00800: step 10a's ffmpeg pass failed (exit 1): title_t00.mkv: Invalid data found when processing input")
    );
    assert!(!h.job_dir(&id).join(KEY).join("decide.json").exists());
    assert!(!staged(&h, &id).join("streamhash.txt").exists());
    assert!(!work(&h, &id).exists());
}

#[test]
fn a_staging_share_that_stops_answering_during_step_10a_waits_as_awaiting_share() {
    let h = harness(&[], json!({}));
    h.insert("tok0", "movie-direct");
    h.run_until("Ejected");
    let id = h.job_ids().pop().unwrap();
    let tuning = staged(&h, &id);
    // The share goes away during the pass: the staged folder can take no file.
    h.on_spawn(&["0:a", "streamhash"], move || {
        let _ = fs::remove_dir_all(&tuning);
        fs::write(&tuning, b"").unwrap();
    });
    h.shares().set_reach(&h.staging().display().to_string(), Reach::Unreachable("cable out".into()));
    h.restart();
    h.wait("AwaitingShare", |h| h.state_of(&id) == "AwaitingShare");
    assert_eq!(h.record(&id).state, State::AwaitingShare { return_to: Box::new(State::Inspecting) });
    assert!(events(&h, &id).iter().any(|e| matches!(e, Event::DecisionFailed { failure } if failure.class == FailureClass::Output)));
    assert!(h.job_dir(&id).join(KEY).join("streamhash.txt").is_file(), "the job folder's copy lands first");
}

#[test]
fn a_missing_ffmpeg_waits_as_tool_missing() {
    let h = harness(&[], json!({"tool_paths": {"ffmpeg": null}}));
    let id = run_to(&h, "AwaitingTool");
    h.settle();
    assert_eq!(h.record(&id).state, State::AwaitingTool { return_to: Box::new(State::Inspecting) });
    assert!(events(&h, &id).contains(&Event::ToolMissing { tool: Tool::Ffmpeg }));
    assert!(!h.job_dir(&id).join(KEY).join("decide.json").exists());
}

/// R5-code-33 (ENG-19): step 10a reads the job's own `keep_tuning_evidence`, not the engine's. The engine's settings
/// turn it off while the engine is stopped before Inspecting; the job keeps the settings it was made under.
#[test]
fn step_10a_follows_the_jobs_own_keep_tuning_evidence() {
    let h = harness(&[], evidence_on());
    h.insert("tok0", "movie-direct");
    h.run_until("Ejected");
    let id = h.job_ids().pop().unwrap();
    let path = h.data().join("settings.json");
    let mut settings = json_of(&path);
    settings["keep_tuning_evidence"] = json!(false);
    fs::write(&path, serde_json::to_vec_pretty(&settings).unwrap()).unwrap();
    h.restart();
    h.wait("Filed", |h| h.state_of(&id) == "Filed");
    assert_eq!(json_of(&path)["keep_tuning_evidence"], json!(false), "the engine's own setting stayed off");
    assert!(staged(&h, &id).join(LEVELS).join("audio-proxy.json").is_file());
    assert!(staged(&h, &id).join("audio-proxy.json").is_file());
    assert!(h.job_dir(&id).join(KEY).join(LEVELS).is_dir());
    assert!(!h.job_dir(&id).join(KEY).join("streamhash.txt").exists(), "no setting-off pass ran");
}

/// A setting-off pass that exits 0 and writes no hash file fails the step with Step 13's sentence.
#[test]
fn a_pass_that_leaves_no_hash_file_is_decision_failed() {
    let h = harness(&["movie-direct-ffmpeg-silent"], json!({}));
    let id = run_to(&h, "Failed");
    let record = h.record(&id);
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Inspecting) });
    assert_eq!(record.message.as_deref(), Some("t00-00800: ffmpeg wrote no streamhash.txt"));
    assert!(!h.job_dir(&id).join(KEY).join("decide.json").exists());
    assert!(!work(&h, &id).exists());
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

/// Ruling T4-R21, the second half of engine.md's V14 item, accepted with no code: Decide reads only the job's own
/// copy (R-B1), so a copy Plan could not write plans with the remembered identity and titles, decides without the
/// track overrides (audio part 0.95, not the Keep's 1.00), and job.log names the copy.
#[test]
fn an_unwritable_remembered_copy_decides_without_overrides_and_logs_it() {
    let h = harness(&[], json!({}));
    let fp = h.fingerprint("movie-direct");
    fs::create_dir_all(h.data().join("remembered")).unwrap();
    fs::write(h.data().join("remembered").join(format!("{fp}.json")), remembered_keep(&fp)).unwrap();
    h.insert("tok0", "movie-direct");
    h.run_until("Scanned");
    let id = format!("{}-1", fp.to_ascii_lowercase());
    let dir = h.job_dir(&id);
    fs::create_dir_all(dir.join(format!("remembered.json{}", lazymkv_platform::TEMP_SUFFIX))).unwrap();
    h.restart();
    h.wait("Filed", |h| h.state_of(&id) == "Filed");
    assert_eq!(h.record(&id).plan.unwrap().identity.step, IdentityStep::Remembered);
    assert!(!dir.join("remembered.json").exists());
    let doc = json_of(&dir.join(KEY).join("decide.json"));
    assert_eq!(doc["decision"]["decision"]["audio"]["confidence"], json!(0.95));
    let log = fs::read_to_string(dir.join("job.log")).unwrap();
    assert!(log.contains(&format!("{}: ", dir.join("remembered.json").display())), "{log}");
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

/// The final review of review round 1 (C6; R-B1, RR1-14): the engine's first-pass Decide, which holds the probe it
/// ran, reads the job's remembered.json, never the data folder's file. That file is removed after the job is planned
/// and ejected, and the engine restarted before the file phase, so the Decide that follows runs over the probe of its
/// own run: decide.json still carries the job copy's Keep (audio part 1.00), and no read-back line is written.
#[test]
fn a_first_pass_decide_applies_the_jobs_remembered_copy_not_the_data_folders_file() {
    let h = harness(&[], json!({}));
    let fp = h.fingerprint("movie-direct");
    let data_copy = h.data().join("remembered").join(format!("{fp}.json"));
    fs::create_dir_all(data_copy.parent().unwrap()).unwrap();
    fs::write(&data_copy, remembered_keep(&fp)).unwrap();
    h.insert("tok0", "movie-direct");
    h.run_until("Ejected");
    let id = h.job_ids().pop().unwrap();
    let dir = h.job_dir(&id);
    assert!(dir.join("remembered.json").is_file());
    assert!(!dir.join(KEY).join("decide.json").exists());
    fs::remove_file(&data_copy).unwrap();
    h.restart();
    h.wait("Filed", |h| h.state_of(&id) == "Filed");
    let doc = json_of(&dir.join(KEY).join("decide.json"));
    assert_eq!(doc["decision"]["decision"]["audio"]["confidence"], json!(1.0));
    let log = fs::read_to_string(dir.join("job.log")).unwrap();
    assert!(!log.contains("decide inputs read back"), "{log}");
}

/// Design part 10.2 steps 3 to 5 (S32 removed): both English PGS streams are read, their text and crops land in the
/// staged folder, the counts in the job folder, and decide's subtitles section is the text rule's.
#[test]
fn two_english_subtitle_streams_are_read_by_the_worker_and_decided_with_their_text() {
    let h = two_subs(&[], json!({}));
    let id = held(&h);
    let label = h.job_dir(&id).join(KEY);
    let doc = json_of(&label.join("decide.json"));
    assert_eq!(doc["subtitles"]["reading"], json!("subs-ocr"));
    let streams: Vec<(i64, String, i64)> = doc["subtitles"]["streams"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| (s["n"].as_i64().unwrap(), s["status"].as_str().unwrap().to_string(), s["events"].as_i64().unwrap()))
        .collect();
    assert_eq!(streams, [(0, "read".to_string(), 2), (1, "read".to_string(), 2)]);
    assert!(unread_notes(&h, &id).is_empty());
    assert_eq!(files_under(&label.join("subs-ocr")), set(&["sub-0.frames.json", "sub-1.frames.json", "summary.json"]));
    let frames = json_of(&label.join("subs-ocr").join("sub-0.frames.json"));
    let keys: Vec<&str> = frames[1].as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!((frames.as_array().unwrap().len(), keys, &frames[1]["lines"]),
               (2, vec!["frame", "file", "t", "pts", "chars", "lines", "markers"], &json!(2)));
    let staged = staged_ocr(&h, &id);
    assert_eq!(files_under(&staged), set(&[
        "s0/f-000001000.png", "s0/f-000005000.png", "s1/f-000002000.png", "s1/f-000006000.png",
        "sub-0.ocr.jsonl", "sub-0.run.json", "sub-1.ocr.jsonl", "sub-1.run.json", "summary.json",
    ]));
    assert_eq!(fs::read(staged.join("sub-0.ocr.jsonl")).unwrap(), fs::read(h.tools().join("records-s0.txt")).unwrap());
    assert!(!work_of(&h, &id).exists());
    let log = fs::read_to_string(h.job_dir(&id).join("job.log")).unwrap();
    assert!(!log.contains("subtitle evidence (step 10) is not built in this version"), "{log}");
}

/// Design part 10.7, K2, R5-code-35: one run per stream, the next only after the last ended, below normal with
/// RTEN_NUM_THREADS=3 alone, in the title's work folder with the job's sub_clear_max_bytes.
#[test]
fn the_worker_runs_once_per_english_or_no_language_stream_one_at_a_time_below_normal_with_rten_num_threads_3() {
    let h = two_subs(&[], json!({}));
    let id = decided(&h);
    let work = work_of(&h, &id);
    let spawns = ocr_spawns(&h);
    assert_eq!(spawns.len(), 2);
    for (n, spawn) in spawns.iter().enumerate() {
        let want = [
            work.join("evidence.mks").to_string_lossy().into_owned(),
            "--stream".to_string(),
            n.to_string(),
            "--clear-max".to_string(),
            "64".to_string(),
            "--out".to_string(),
            work.join(format!("s{n}")).to_string_lossy().into_owned(),
        ];
        assert_eq!(spawn.args, want);
        assert_eq!(spawn.program, h.tools().join(format!("lazymkv-ocr{}", std::env::consts::EXE_SUFFIX)));
        assert_eq!(spawn.env, [("RTEN_NUM_THREADS".to_string(), "3".to_string())]);
        assert!(spawn.below_normal);
    }
    let log = fs::read_to_string(h.job_dir(&id).join("job.log")).unwrap();
    let at = |needle: &str| log.find(needle).unwrap_or_else(|| panic!("job.log lacks {needle}: {log}"));
    assert!(at("run lazymkv-ocr s0: ") < at("ended lazymkv-ocr s0: exit 0 after "));
    assert!(at("ended lazymkv-ocr s0: exit 0 after ") < at("run lazymkv-ocr s1: "));
    assert!(at("run lazymkv-ocr s1: ") < at("ended lazymkv-ocr s1: exit 0 after "));
}

/// Design part 10.8, T5-R9, T5-R58: past its cap the worker is killed, its partial output deleted, and the note's
/// detail counts its whole records; the note raises no review, so the decision's reviews are those of the same folders
/// with the unread streams taken out.
#[test]
fn a_worker_past_its_cap_leaves_the_stream_unread_with_its_note_and_no_review() {
    let h = two_subs(&CAP, json!({}));
    let id = decided(&h);
    assert_eq!(unread_notes(&h, &id)[0], unread(2, &cap_reason_of(3)));
    assert_eq!(files_under(&staged_ocr(&h, &id)), set(&["summary.json"]));
    assert!(!work_of(&h, &id).exists());
    let mut inputs = read_back(&h, &id, None);
    assert_eq!(inputs.file.unread, Ok(vec![(0, cap_reason_of(3)), (1, cap_reason_of(3))]));
    inputs.file.unread = Ok(Vec::new());
    let DecisionEntry::Results(without) = decide_doc(&inputs).decision else { panic!("the decision was skipped") };
    let decision = h.record(&id).titles[0].decision.clone().unwrap();
    assert_eq!(decision.reviews, without.decision.reviews);
    assert!(without.decision.notes.iter().all(|noted| noted.note != Note::SubtitleTextUnread));
}

/// Review Focus 2, T5-R9: a kill in the middle of a record counts only the whole records before it.
#[test]
fn a_killed_worker_with_a_torn_last_record_counts_only_whole_records() {
    let h = two_subs(&CAP, json!({}));
    let records = fs::read_to_string(h.tools().join("records-cap-s1.txt")).unwrap();
    assert_eq!((records.matches('\n').count(), records.ends_with('\n')), (3, false), "the fixture's fourth record is torn");
    let id = decided(&h);
    assert_eq!(unread_notes(&h, &id)[1], unread(3, &cap_reason_of(3)));
}

/// Review Focus 2, T5-R8, design part 10.12: a panic's standard error can hold text, so the reason is the exit code
/// alone and the sentinel reaches no job file, engine.log or the snapshot; no capture file reaches the job folder.
#[test]
fn a_worker_that_panics_leaves_the_stream_unread_with_its_exit_code_and_no_stderr_text() {
    let h = two_subs(&["movie-direct-two-subs-panic"], json!({}));
    assert!(fs::read_to_string(h.tools().join("panic-s1.txt")).unwrap().contains(SENTINEL));
    let id = held(&h);
    assert_eq!(unread_notes(&h, &id), [unread(3, "lazymkv-ocr exited with code 101")]);
    let jobs = h.data().join("jobs");
    assert_eq!(holding(&jobs, SENTINEL), Vec::<String>::new());
    assert!(!h.engine_log().contains(SENTINEL));
    assert!(!Client::open(&h).snapshot().to_string().contains(SENTINEL));
    assert!(files_under(&jobs).iter().all(|name| !name.contains("capture")), "{:?}", files_under(&jobs));
}

/// Review Focus 2, design part 10.3: records the listing does not have leave the stream unread with numbers only.
#[test]
fn a_worker_that_writes_more_records_than_events_is_unread_by_the_cross_check() {
    let h = two_subs(&["movie-direct-two-subs-extra"], json!({}));
    let id = decided(&h);
    assert_eq!(unread_notes(&h, &id)[1], unread(3, "3 events recorded, 2 listed"));
    assert!(!staged_ocr(&h, &id).join("sub-1.ocr.jsonl").exists());
}

/// Design part 10.3, call 24: the sizes are compared too, never count and order alone.
#[test]
fn a_size_disagreement_with_ffprobe_leaves_the_stream_unread() {
    let h = two_subs(&["movie-direct-two-subs-extra"], json!({}));
    let id = decided(&h);
    assert_eq!(unread_notes(&h, &id)[0], unread(2, "event 1: size 1300 recorded, 1200 listed"));
    assert!(!staged_ocr(&h, &id).join("sub-0.ocr.jsonl").exists());
}

/// T5-R8, call 5: exit 3's first standard error line is the note's reason.
#[test]
fn an_exit_3_refusal_is_the_notes_reason() {
    let h = two_subs(&["movie-direct-two-subs-refused"], json!({}));
    let id = decided(&h);
    assert_eq!(unread_notes(&h, &id), [unread(3, REFUSAL)]);
}

/// T5-R8, Step 11 item e: a worker that exits 0 and writes no records leaves each stream unread with that sentence.
#[test]
fn a_worker_that_exits_0_without_records_leaves_each_stream_unread() {
    let h = two_subs(&["movie-direct-two-subs-silent"], json!({}));
    let id = decided(&h);
    assert_eq!(
        unread_notes(&h, &id),
        [unread(2, "lazymkv-ocr wrote no sub-0.ocr.jsonl"), unread(3, "lazymkv-ocr wrote no sub-1.ocr.jsonl")]
    );
    assert_eq!(files_under(&staged_ocr(&h, &id)), set(&["summary.json"]));
    assert!(!work_of(&h, &id).exists());
}

/// Design parts 10.8, 10.12, P19: an unread stream's partial records go; summary.json says why, with its exit code,
/// in the job folder and, byte for byte, in the staged folder.
#[test]
fn an_unread_stream_keeps_no_records_and_summary_json_says_why() {
    let h = two_subs(&["movie-direct-two-subs-panic"], json!({}));
    let id = decided(&h);
    let label = h.job_dir(&id).join(KEY);
    let staged = staged_ocr(&h, &id);
    assert_eq!(files_under(&staged), set(&[
        "s0/f-000001000.png", "s0/f-000005000.png", "sub-0.ocr.jsonl", "sub-0.run.json", "summary.json",
    ]));
    assert!(!label.join("subs-ocr").join("sub-1.frames.json").exists());
    let summary = json_of(&label.join("subs-ocr").join("summary.json"));
    let index = json_of(&label.join("subs").join("index.json"));
    let s1 = &summary["streams"][1];
    assert_eq!((&s1["n"], &s1["codec"], &s1["read"], &s1["why"], &s1["exit"]),
               (&json!(1), &index[1]["codec"], &json!(false), &json!("lazymkv-ocr exited with code 101"), &json!(101)));
    assert_eq!((&summary["streams"][0]["read"], &summary["streams"][0]["exit"]), (&json!(true), &json!(0)));
    assert_eq!(fs::read(staged.join("summary.json")).unwrap(), fs::read(label.join("subs-ocr").join("summary.json")).unwrap());
}

/// T5-R51: the title's subtitles section is the unread line, with no text, and the read stream's records are kept.
#[test]
fn a_title_with_an_unread_stream_has_the_unread_line_as_its_subtitles_section() {
    let h = two_subs(&["movie-direct-two-subs-refused"], json!({}));
    let id = decided(&h);
    let decide = h.job_dir(&id).join(KEY).join("decide.json");
    assert_eq!(json_of(&decide)["subtitles"], json!({"skipped": format!("stream s1 eng: not read by the engine: {REFUSAL}")}));
    assert!(!fs::read_to_string(&decide).unwrap().contains(SENTINEL));
    let staged = staged_ocr(&h, &id);
    assert!(fs::read_to_string(staged.join("sub-0.ocr.jsonl")).unwrap().contains(SENTINEL));
    assert!(!staged.join("sub-1.ocr.jsonl").exists());
}

/// T5-R59: the job's own sub_clear_max_bytes reaches the worker, the cross-check and the subtitles section, so a
/// 50-byte packet is an event at 40 for all three. (The text's reviews stay out at any threshold but 64: dev's
/// `text_decisions` uses no text when `settings.text_rule()` is not the default, RP15.)
#[test]
fn a_job_whose_clear_max_is_not_64_decides_with_the_workers_events() {
    let h = two_subs(&["movie-direct-two-subs-clear-40"], json!({"sub_clear_max_bytes": 40}));
    let id = decided(&h);
    for spawn in ocr_spawns(&h) {
        assert!(spawn.args.windows(2).any(|pair| pair == ["--clear-max", "40"]), "{:?}", spawn.args);
    }
    assert!(unread_notes(&h, &id).is_empty());
    let staged = fs::read_to_string(staged_ocr(&h, &id).join("sub-1.ocr.jsonl")).unwrap();
    assert_eq!(staged.lines().count(), 3);
    let doc = json_of(&h.job_dir(&id).join(KEY).join("decide.json"));
    assert_eq!((&doc["subtitles"]["streams"][0]["events"], &doc["subtitles"]["streams"][1]["events"]), (&json!(2), &json!(3)));
}

/// T5-R12: the staged summary.json, C3's marker, lands only after the job folder's: when the job folder's cannot be
/// written the step fails and the staged folder holds the streams' files but no summary.json.
#[test]
fn the_staged_summary_json_is_written_after_the_job_folders() {
    let h = two_subs(&[], json!({}));
    let id = job_id(&h);
    let blocker = h.job_dir(&id).join(KEY).join("subs-ocr").join("summary.json");
    let s1 = work_of(&h, &id).join("s1").display().to_string();
    let made = blocker.clone();
    h.on_spawn(&[&s1], move || fs::create_dir_all(&made).unwrap());
    let id = run_to(&h, "Failed");
    let record = h.record(&id);
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Inspecting) });
    let message = record.message.unwrap();
    assert!(message.starts_with(&format!("{}: ", blocker.display())), "{message}");
    let staged = staged_ocr(&h, &id);
    assert!(staged.join("sub-0.ocr.jsonl").is_file() && staged.join("sub-1.ocr.jsonl").is_file());
    assert!(!staged.join("summary.json").exists());
    assert!(!work_of(&h, &id).exists());
}

/// Design part 10.12 item 3: the text is in the staged folder and in no file under jobs\, engine.log or the snapshot.
#[test]
fn no_ocr_text_reaches_the_job_folder_logs_state_or_snapshot() {
    let h = two_subs(&[], json!({}));
    let id = held(&h);
    assert!(fs::read_to_string(staged_ocr(&h, &id).join("sub-0.ocr.jsonl")).unwrap().contains(SENTINEL));
    assert_eq!(holding(&h.data().join("jobs"), SENTINEL), Vec::<String>::new());
    assert!(!h.engine_log().contains(SENTINEL));
    assert!(!Client::open(&h).snapshot().to_string().contains(SENTINEL));
}

/// T5-R57: the worker's capture is written to the work folder only and deleted with it.
#[test]
fn no_worker_capture_reaches_the_job_folder() {
    let h = two_subs(&[], json!({}));
    let id = decided(&h);
    for root in [h.data().join("jobs"), h.staging().to_path_buf()] {
        let files = files_under(&root);
        assert!(files.iter().all(|name| !name.contains("capture")), "{files:?}");
    }
    assert!(!work_of(&h, &id).exists());
}

/// Design part 10.7, ENG-13: a cancel kills the running worker, deletes its partial files and ends the job as
/// cancelled by the owner.
#[test]
fn a_cancel_kills_the_worker_and_deletes_its_work_folder() {
    let h = two_subs(&["movie-direct-two-subs-cap"], json!({}));
    let mut c = Client::open(&h);
    c.snapshot();
    h.insert("tok0", "movie-direct");
    h.deliver();
    let id = job_id(&h);
    let first = work_of(&h, &id).join("s0").join("sub-0.ocr.jsonl");
    h.wait("the worker's first records", |_| first.is_file());
    c.ok("cancel", json!({"job": id}));
    let record = h.record(&id);
    assert!(matches!(record.state, State::Failed { class: FailureClass::Unknown, .. }), "{:?}", record.state);
    assert_eq!(record.message.as_deref(), Some("cancelled by the owner"));
    h.wait("the work folder removed", |h| !work_of(h, &id).exists());
    assert_eq!(files_under(&staged_ocr(&h, &id)), BTreeSet::new());
}

/// T5-R8: a worker that is not beside the engine fails the decision before any extraction.
#[test]
fn a_missing_worker_fails_the_decision_with_ocr_missing() {
    let h = two_subs(&[], json!({}));
    fs::remove_file(h.tools().join(format!("lazymkv-ocr{}", std::env::consts::EXE_SUFFIX))).unwrap();
    let id = run_to(&h, "Failed");
    let record = h.record(&id);
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Inspecting) });
    assert_eq!(record.message.as_deref(), Some("lazymkv-ocr is not beside the engine, so step 10 cannot read subtitles"));
    assert!(!h.staging().join(&id).join("evidence").exists());
    assert!(ocr_spawns(&h).is_empty());
}

const TWO_SUBS: &str = "movie-direct-two-subs";

/// Inserts movie-direct and waits until its Decide, or the step before it, fed DecisionDone or DecisionFailed; until
/// Task 15 a title that needs step 10 fails there with S32's sentence.
fn until_decided(h: &Harness) -> String {
    h.insert("tok0", "movie-direct");
    let id = format!("{}-1", h.fingerprint("movie-direct").to_ascii_lowercase());
    let decided = |h: &Harness| {
        h.transitions(&id).iter().any(|line| {
            matches!(line.event, Some(Event::DecisionDone { .. }) | Some(Event::DecisionFailed { .. }))
        })
    };
    h.wait("the decision", decided);
    h.settle();
    id
}

/// The arguments of every spawn holding the argument `word` exactly, in order.
fn spawns_with(h: &Harness, word: &str) -> Vec<Vec<String>> {
    h.spawns().into_iter().filter(|s| s.args.iter().any(|a| a == word)).map(|s| s.args).collect()
}

/// `<staging>/<id>/evidence/<KEY>.mks` and the staged raw file `<staging>/<id>/rip/<KEY>/title_t00.mkv`.
fn evidence_and_raw(h: &Harness, id: &str) -> (PathBuf, PathBuf) {
    let job = h.staging().join(id);
    (job.join("evidence").join(format!("{KEY}.mks")), job.join("rip").join(KEY).join("title_t00.mkv"))
}

#[test]
fn step_10_writes_the_evidence_file_the_subtitle_index_and_one_packet_list_per_stream() {
    let h = harness(&[TWO_SUBS], json!({}));
    let id = until_decided(&h);
    let (evidence, raw) = evidence_and_raw(&h, &id);
    assert_eq!(fs::read(&evidence).unwrap(), b"lazymkv-test-mks");
    assert_eq!(spawns_with(&h, "-D"), vec![extract_args(&evidence, &[2, 3], &raw)]);
    assert_eq!(spawns_with(&h, "-select_streams"), vec![packets_args(0, &evidence), packets_args(1, &evidence)]);
    let dir = h.job_dir(&id).join(KEY);
    assert_eq!(fs::read(dir.join("evidence-mkvmerge.txt")).unwrap(), b"#GUI#progress 100%\n");
    for n in 0..2 {
        let packets = dir.join("subs").join(format!("sub-{n}.packets.json"));
        assert_eq!(fs::read(&packets).unwrap(), fs::read(h.tools().join(format!("packets-s{n}.json"))).unwrap());
        assert!(dir.join("subs").join(format!("sub-{n}.packets.json.exit.json")).is_file());
    }
    let index = dir.join("subs").join("index.json");
    assert_eq!(json_of(&index), json!([
        {"n": 0, "track_id": 2, "language": "eng", "codec": "HDMV PGS", "track_name": null, "forced": null, "default": null},
        {"n": 1, "track_id": 3, "language": "eng", "codec": "HDMV PGS", "track_name": null, "forced": null, "default": null}
    ]));
    assert!(fs::read_to_string(&index).unwrap().ends_with("}\n]\n"), "pretty JSON and a final newline");
    let log = fs::read_to_string(h.job_dir(&id).join("job.log")).unwrap();
    assert!(log.contains(&format!("run {KEY}/evidence-mkvmerge.txt: ")), "{log}");
    assert!(log.contains(&format!("ended {KEY}/subs/sub-1.packets.json: exit 0 after ")), "{log}");
}

#[test]
fn probe_summary_json_holds_its_seven_keys_and_no_remux_test_key() {
    let h = harness(&[TWO_SUBS], json!({}));
    let id = until_decided(&h);
    let path = h.job_dir(&id).join(KEY).join("probe-summary.json");
    let summary = json_of(&path);
    let keys: Vec<&str> = summary.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(keys, ["ok", "result", "file", "file_bytes", "subtitle_tracks", "evidence_bytes", "evidence_ms"]);
    let (_, raw) = evidence_and_raw(&h, &id);
    assert_eq!(
        (&summary["ok"], &summary["result"], &summary["file"]),
        (&json!(true), &json!("ok"), &json!(raw.display().to_string()))
    );
    assert_eq!((&summary["file_bytes"], &summary["subtitle_tracks"]), (&json!(16), &json!(2)));
    assert_eq!(summary["evidence_bytes"], json!(16));
    assert!(summary["evidence_ms"].is_u64(), "{summary}");
    assert!(fs::read_to_string(&path).unwrap().ends_with("}\n"));
}

#[test]
fn a_title_with_one_english_subtitle_stream_runs_no_extraction() {
    let h = harness(&["movie-direct-one-sub"], json!({}));
    let id = until_decided(&h);
    assert!(spawns_with(&h, "-D").is_empty() && spawns_with(&h, "-select_streams").is_empty());
    assert!(!h.staging().join(&id).join("evidence").exists());
    let dir = h.job_dir(&id).join(KEY);
    for absent in ["subs", "probe-summary.json", "evidence-mkvmerge.txt"] {
        assert!(!dir.join(absent).exists(), "{absent}");
    }
}

#[test]
fn mkvmerge_exit_1_is_accepted_and_exit_2_fails_the_decision() {
    let warned = harness(&[TWO_SUBS, "movie-direct-two-subs-extract-warn"], json!({}));
    let id = until_decided(&warned);
    assert_eq!(spawns_with(&warned, "-select_streams").len(), 2);
    assert_eq!(json_of(&warned.job_dir(&id).join(KEY).join("probe-summary.json"))["ok"], json!(true));

    let h = harness(&[TWO_SUBS, "movie-direct-two-subs-extract-error"], json!({}));
    let id = until_decided(&h);
    let record = h.record(&id);
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Inspecting) });
    assert_eq!(record.message.as_deref(), Some("mkvmerge exited with code 2"));
    assert!(spawns_with(&h, "-select_streams").is_empty());
    let summary = json_of(&h.job_dir(&id).join(KEY).join("probe-summary.json"));
    assert_eq!((&summary["ok"], &summary["result"]), (&json!(false), &json!("FAILED: mkvmerge exited with code 2")));
    assert_eq!((&summary["subtitle_tracks"], &summary["evidence_bytes"]), (&json!(2), &Value::Null));
    assert!(!h.job_dir(&id).join(KEY).join("subs").join("index.json").exists());
}

#[test]
fn a_missing_mkvmerge_waits_as_tool_missing() {
    let h = harness(&[TWO_SUBS], json!({}));
    let mkvmerge = h.tools().join(exe_name(Tool::Mkvmerge));
    // Removed when the probe starts, so the disc phase had it and only step 10 finds it missing.
    h.on_spawn(&["-show_streams"], move || fs::remove_file(&mkvmerge).unwrap());
    let id = run_to(&h, "AwaitingTool");
    h.settle();
    assert_eq!(h.record(&id).state, State::AwaitingTool { return_to: Box::new(State::Inspecting) });
    let found = events(&h, &id);
    let probed = found.iter().position(|e| *e == Event::ProbeDone).expect("the probe ran");
    let missing = found.iter().position(|e| *e == Event::ToolMissing { tool: Tool::Mkvmerge }).unwrap_or_else(|| panic!("no ToolMissing {{ Mkvmerge }}: {found:?}"));
    assert!(probed < missing, "{found:?}");
    assert!(spawns_with(&h, "-select_streams").is_empty());
}

/// Ruling T5-R56: a write error on the staging share during step 10 waits for the share, as RipTitle's does.
#[test]
fn a_staging_share_that_stops_answering_during_the_extraction_waits_as_awaiting_share() {
    let h = harness(&[TWO_SUBS], json!({}));
    h.insert("tok0", "movie-direct");
    h.run_until("Inspecting");
    let id = h.job_ids().pop().unwrap();
    let folder = h.staging().join(&id).join("evidence");
    fs::write(&folder, b"").unwrap(); // a file where the evidence folder goes, so creating it fails
    let staging = h.staging().to_str().unwrap().to_string();
    h.shares().set_reach(&staging, Reach::Unreachable("cable out".into()));
    h.restart();
    h.wait("AwaitingShare", |h| h.state_of(&id) == "AwaitingShare");
    let record = h.record(&id);
    assert_eq!(record.state, State::AwaitingShare { return_to: Box::new(State::Inspecting) });
    let share = Failure { class: FailureClass::Output, output: Some(OutputCause::ShareUnreachable) };
    assert!(events(&h, &id).contains(&Event::DecisionFailed { failure: share }));
    let message = record.message.unwrap();
    assert!(message.starts_with(&format!("{}: ", folder.display())), "{message}");
    assert!(spawns_with(&h, "-D").is_empty());
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

/// Review round 1 of pull request 3 (R25; Rulings W15, M12): Remux makes only the partial folder inside an existing
/// library root and never the root, so a missing root fails the remux naming it, the staged file stays, and a retry
/// once the folder exists files the title. On Windows the partial folder's create_dir under the missing root gives os
/// error 3.
#[test]
fn a_missing_library_root_fails_the_remux_and_is_never_created() {
    let h = harness(&[], json!({}));
    fs::remove_dir(h.library()).unwrap();
    let id = run_to(&h, "Failed");
    let record = h.record(&id);
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Remuxing) });
    assert!(events(&h, &id).iter().any(|e| matches!(e, Event::RemuxFailed { .. })));
    let message = record.message.unwrap_or_default();
    let head = format!("{KEY}: the library root {} is not a folder (", h.library().display());
    let tail = "); the engine never creates a library root, so the remux was not started, and a retry once the folder \
                exists runs it";
    assert!(message.starts_with(&head) && message.ends_with(tail), "{message}");
    assert!(!h.library().exists());
    assert!(h.staging().join(&id).join("rip").join(KEY).join("title_t00.mkv").is_file());
    fs::create_dir(h.library()).unwrap();
    Client::open(&h).ok("retry", json!({"job": id}));
    h.wait("Filed", |h| h.state_of(&id) == "Filed");
    assert!(final_path(&h).is_file());
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

/// Stops the engine at the first transition into `state` (before its actions) and puts a regular file in place of the
/// title's staging folder `<staging>/<id>/rip/<KEY>`, whose own files are moved aside, so listing the folder fails on
/// every OS with an error other than NotFound (review round 1, V3). Returns the job and the folder.
fn block_the_title_folder(h: &Harness, state: &str) -> (String, PathBuf) {
    h.insert("tok0", "movie-direct");
    h.run_until(state);
    let id = h.job_ids().pop().unwrap();
    let dir = h.staging().join(&id).join("rip").join(KEY);
    fs::rename(&dir, dir.with_extension("aside")).unwrap();
    fs::write(&dir, b"").unwrap();
    (id, dir)
}

/// `block_the_title_folder`, then the engine restarted and run until the job fails.
fn restart_with_the_title_folder_blocked(h: &Harness, state: &str) -> (String, PathBuf) {
    let (id, dir) = block_the_title_folder(h, state);
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

/// The final review of review round 1 (N1): when the staged sizes cannot be read and the plan's estimate does not fit
/// the library, the job waits in AwaitingSpace saying what any shortage says, `waiting: space`, and the read error is
/// only a job.log line.
#[test]
fn an_unreadable_title_folder_at_a_short_remux_check_waits_naming_only_the_shortage() {
    let h = harness(&[], json!({}));
    let (id, dir) = block_the_title_folder(&h, "Decided");
    h.shares().set_free(h.library().to_str().unwrap(), 1);
    h.restart();
    h.wait("AwaitingSpace", |h| h.state_of(&id) == "AwaitingSpace");
    h.settle();
    let record = h.record(&id);
    assert_eq!(record.message, None);
    let waiting = h.notifications().into_iter().last().unwrap();
    assert_eq!(waiting.event, NotifyEvent::Waiting { reason: WaitReason::Space });
    assert_eq!(waiting.message, "waiting: space");
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

/// A folder this process cannot open (L7), closed once by `close` (which creates it first) and opened again by `open`
/// or a drop: Unix mode 0o000 and back to 0o755; Windows a handle to the folder opened with share mode 0 and
/// FILE_FLAG_BACKUP_SEMANTICS (0x0200_0000, which opening a folder needs), so the listing and the rename into it fail
/// with a sharing violation until `open` drops it. Windows does not use an access-list deny because the mini PC's
/// elevated SSH session holds the backup and restore privileges, which bypass one (Ruling T3-18-a). So `holds` is
/// always true on Windows, and its guard matters only on Unix, where root opens a folder of mode 0o000.
struct Closed {
    folder: PathBuf,
    once: AtomicBool, // `close` has acted
    shut: AtomicBool, // closed now
    #[cfg(windows)]
    held: std::sync::Mutex<Option<fs::File>>, // the handle that closes the folder
}

impl Closed {
    fn new(folder: PathBuf) -> Closed {
        Closed {
            folder,
            once: AtomicBool::new(false),
            shut: AtomicBool::new(false),
            #[cfg(windows)]
            held: std::sync::Mutex::new(None),
        }
    }

    /// Creates the folder and closes it, on the first call only.
    fn close(&self) {
        if self.once.swap(true, Ordering::SeqCst) {
            return;
        }
        fs::create_dir_all(&self.folder).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.folder, fs::Permissions::from_mode(0o000)).unwrap();
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            let mut options = fs::OpenOptions::new();
            let handle = options.read(true).share_mode(0).custom_flags(0x0200_0000).open(&self.folder).unwrap();
            *self.held.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(handle);
        }
        self.shut.store(true, Ordering::SeqCst);
    }

    /// Closed, and `fs::read_dir` of the folder fails (false for root on Unix, which opens a folder of mode 0o000).
    fn holds(&self) -> bool {
        self.shut.load(Ordering::SeqCst) && fs::read_dir(&self.folder).is_err()
    }

    /// Opens the folder again, only while it is closed.
    fn open(&self) {
        if !self.shut.swap(false, Ordering::SeqCst) {
            return;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.folder, fs::Permissions::from_mode(0o755)).unwrap();
        }
        #[cfg(windows)]
        drop(self.held.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take());
    }
}

impl Drop for Closed {
    fn drop(&mut self) {
        self.open();
    }
}

/// Runs movie-direct to Failed with its film folder closed just before File (the hook fires on VerifyFile's
/// `mkvmerge -J <partial>`, after Decide read the library). Returns the job, the guard and the final path.
fn run_with_the_film_folder_closed(h: &Harness) -> (String, Arc<Closed>, PathBuf) {
    let target = final_path(h);
    let closed = Arc::new(Closed::new(target.parent().unwrap().to_path_buf()));
    let later = Arc::downgrade(&closed);
    h.on_spawn(&["-J", ".partial"], move || {
        if let Some(closed) = later.upgrade() {
            closed.close();
        }
    });
    let id = run_to(h, "Failed");
    (id, closed, target)
}

/// Ruling W22 (R-B1; engine.md:591): a library folder that cannot be read fails Decide as Unknown naming it, instead of
/// reading as a folder holding nothing.
#[test]
fn a_library_folder_that_cannot_be_read_fails_decide_rather_than_reading_as_empty() {
    let h = harness(&[], json!({}));
    let film = final_path(&h).parent().unwrap().to_path_buf();
    fs::create_dir_all(film.parent().unwrap()).unwrap();
    fs::write(&film, b"").unwrap();
    let id = run_to(&h, "Failed");
    let record = h.record(&id);
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Inspecting) });
    let message = record.message.unwrap_or_default();
    assert!(message.starts_with(&format!("the library folder {} could not be read: ", film.display())), "{message}");
    assert!(events(&h, &id).iter().any(|e| matches!(e, Event::DecisionFailed { .. })));
}

/// Ruling W22: a film folder that does not exist still reads as no file, so the job files as before.
#[test]
fn an_absent_library_folder_still_reads_as_no_file() {
    let h = harness(&[], json!({}));
    let id = run_to(&h, "Filed");
    let decision = h.record(&id).titles[0].decision.clone().unwrap();
    assert!(!decision.reviews.contains(&ReviewReason::PathCollision), "{:?}", decision.reviews);
    assert_eq!(decision.path.map(|path| PathBuf::from(path.path)), Some(final_path(&h)));
}

/// Ruling W22 (V7, design part 6.1): Decide asks `reach` of the library root before it reads the library, so a share
/// that does not answer fails Decide naming it instead of reading as an empty folder.
#[test]
fn decide_asks_the_library_share_before_reading_it() {
    let h = harness(&[], json!({}));
    h.insert("tok0", "movie-direct");
    h.run_until("Inspecting");
    let id = h.job_ids().pop().unwrap();
    let library = h.library().to_str().unwrap().to_string();
    h.shares().set_reach(&library, Reach::Unreachable("cable out".into()));
    h.restart();
    h.wait("Failed", |h| h.state_of(&id) == "Failed");
    let record = h.record(&id);
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Inspecting) });
    let why = format!(
        "the library share holding {library} does not answer (cable out); the library was not read, and a retry once \
         the share answers decides again"
    );
    assert_eq!(record.message.as_deref(), Some(why.as_str()));
}

/// Review round 1 of pull request 3 (R28; Ruling W22): a library share that answers Decide's first `reach` and stops
/// answering before its reads are done fails Decide naming the share, and nothing is written: no decide.json and no
/// decision in the record, so a share that drops during the reads never reads as an empty library. If an earlier
/// caller used up the count, the first reach's message appears instead and the test fails.
#[test]
fn a_library_share_that_stops_answering_during_decide_fails_it_and_writes_nothing() {
    let h = harness(&[], json!({}));
    h.insert("tok0", "movie-direct");
    h.run_until("Inspecting");
    let id = h.job_ids().pop().unwrap();
    let library = h.library().to_str().unwrap().to_string();
    h.shares().set_reach_after(&library, 1, Reach::Unreachable("cable out".into()));
    h.restart();
    h.wait("Failed", |h| h.state_of(&id) == "Failed");
    let record = h.record(&id);
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Inspecting) });
    let why = format!(
        "the library share holding {library} does not answer (cable out); what was read of the library was not used, \
         and a retry once the share answers decides again"
    );
    assert_eq!(record.message.as_deref(), Some(why.as_str()));
    assert!(!h.job_dir(&id).join(KEY).join("decide.json").exists());
    assert_eq!(record.titles[0].decision, None);
}

/// L7 (design parts 2.10, 6.5; Ruling W28): the title's folder is present but cannot be opened, so File fails as
/// Unknown naming that folder (never the library root, so LIB-7 stands), the partial file stays in
/// `.lazymkv-partial`, and nothing is renamed.
#[test]
fn a_present_but_inaccessible_folder_fails_filing_and_keeps_the_partial() {
    let h = harness(&[], json!({}));
    let (id, closed, target) = run_with_the_film_folder_closed(&h);
    if !closed.holds() {
        return; // root on Unix opens a folder of mode 0o000
    }
    let record = h.record(&id);
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Verified) });
    let partial = h.library().join(".lazymkv-partial").join(format!("{id}-{KEY}.mkv.partial"));
    let film = target.parent().unwrap();
    let message = record.message.unwrap_or_default();
    let head = format!("{} is present but cannot be opened (", film.display());
    let tail = format!("); the partial file stays at {}, and a retry tries again", partial.display());
    assert!(message.starts_with(&head) && message.ends_with(&tail), "{message}");
    assert_eq!(fs::read(&partial).unwrap(), b"lazymkv-test-mkv");
    assert!(record.collisions.is_empty(), "{:?}", record.collisions);
    closed.open();
    assert!(!target.exists());
}

/// L7 (design part 6.5): a `retry` once the folder opens files the title.
#[test]
fn a_retry_after_the_folder_opens_files_the_title() {
    let h = harness(&[], json!({}));
    let (id, closed, target) = run_with_the_film_folder_closed(&h);
    if !closed.holds() {
        return; // root on Unix opens a folder of mode 0o000
    }
    closed.open();
    Client::open(&h).ok("retry", json!({"job": id}));
    h.wait("Filed", |h| h.state_of(&id) == "Filed");
    assert_eq!(fs::read(&target).unwrap(), b"lazymkv-test-mkv");
}

#[test]
fn a_first_decide_with_a_stale_copy_it_cannot_fetch_waits_for_its_lookup() {
    let h = harness(&[], json!({}));
    let id = format!("{}-1", h.fingerprint("movie-direct").to_ascii_lowercase());
    h.insert("tok0", "movie-direct");
    h.run_until("Inspecting");
    stale(&h, &id, "tmdb-record.json");
    no_token(&h);
    h.restart();
    // The store replaces state.json before it appends the transition's line, so the wait is on the event (Ruling T9-R2).
    h.wait("LookupPending", |h| events(h, &id).contains(&Event::LookupPending));
    assert_eq!(h.state_of(&id), "AwaitingLookup");
    let message = h.record(&id).message.unwrap();
    assert!(message.contains("tmdb-record.json: the TMDb copy is more than six months old (fetched_at 2026-01-01) and could \
        not be fetched again: no tmdb_token is set"), "{message}");
    assert!(!message.contains("test-token"));
    assert!(events(&h, &id).contains(&Event::LookupPending));
}

/// Sets `season-1 000` in the job's lookup-status.txt beside its fresh TV record copies, so a restart's book owes
/// season 1 (PR #15 review round 1, R2), and has every later request for season 1 time out.
fn owe_season_1(h: &Harness, id: &str) {
    let path = h.job_dir(id).join("lookup-status.txt");
    let status = fs::read_to_string(&path).unwrap();
    assert!(status.contains("season-1 200\n"), "{status}");
    fs::write(&path, status.replace("season-1 200\n", "season-1 000\n")).unwrap();
    h.http().fail(&lazymkv_clients::season_url(900101, 1), lazymkv_clients::TransportError::Timeout);
}

/// PR #15 review round 1, R2 follow-up (Ruling RR1-R2a): a first Decide whose fresh TV record still owes a season the
/// refetch cannot fetch waits for its lookup, and never decides with the season missing.
#[test]
fn a_first_decide_with_an_owed_season_it_cannot_fetch_waits_for_its_lookup() {
    let h = harness(&["tv-direct-series"], json!({}));
    let id = format!("{}-1", h.fingerprint("movie-direct").to_ascii_lowercase());
    h.insert("tok0", "movie-direct");
    h.run_until("Inspecting");
    owe_season_1(&h, &id);
    h.restart();
    // The store replaces state.json before it appends the transition's line, so the wait is on the event (Ruling T9-R2).
    h.wait("LookupPending", |h| events(h, &id).contains(&Event::LookupPending));
    assert_eq!(h.state_of(&id), "AwaitingLookup");
    let message = h.record(&id).message.unwrap();
    assert!(message.contains("tmdb-record.json: season 1 could not be fetched again: no answer within 10 s"), "{message}");
}

#[test]
fn a_decide_in_decided_after_a_restart_with_a_stale_copy_it_cannot_fetch_fails_unknown() {
    let h = harness(&[], json!({}));
    let id = format!("{}-1", h.fingerprint("movie-direct").to_ascii_lowercase());
    h.insert("tok0", "movie-direct");
    h.run_until("Decided");
    stale(&h, &id, "tmdb-record.json");
    no_token(&h);
    h.restart(); // resume(Decided) runs Decide (core job.rs:741)
    // The store replaces state.json before it appends the transition's line, so the wait is on the event (Ruling T9-R2).
    let failed = |h: &Harness| events(h, &id).iter().any(|e| matches!(e, Event::DecisionFailed { failure } if failure.class == FailureClass::Unknown));
    h.wait("DecisionFailed", failed);
    assert_eq!(h.state_of(&id), "Failed");
    assert!(failed(&h));
    let message = h.record(&id).message.unwrap();
    assert!(message.contains("tmdb-record.json: the TMDb copy is more than six months old"), "{message}");
    assert!(!message.contains("test-token"));
}

#[test]
fn a_plan_again_from_scanned_with_a_deleted_copy_fetches_it_and_plans() {
    let h = harness(&[], json!({}));
    let id = format!("{}-1", h.fingerprint("movie-direct").to_ascii_lowercase());
    h.insert("tok0", "movie-direct");
    h.run_until("Scanned");
    fs::remove_file(h.job_dir(&id).join("tmdb-record.json")).unwrap();
    let movie = TmdbIdentity { kind: TmdbKind::Movie, id: 900001 };
    let record_asks = |h: &Harness| h.http().requests().iter().filter(|(url, _)| *url == lazymkv_clients::record_url(movie)).count();
    let before = record_asks(&h);
    h.restart();
    h.wait("Planned", |h| h.transitions(&id).iter().any(|l| l.event.as_ref().is_some_and(|e| matches!(e, Event::PlanDone { .. }))));
    assert_eq!(record_asks(&h), before + 1, "the deleted copy is fetched again, once");
    assert_eq!(json_of(&h.job_dir(&id).join("tmdb-record.json"))["fetched_at"], json!("2026-10-23"));
    let plan = h.record(&id).plan.unwrap();
    assert_eq!((plan.identity.tmdb, plan.identity.lookup_pending), (Some(movie), false));
    let log = fs::read_to_string(h.job_dir(&id).join("job.log")).unwrap();
    assert!(log.contains("the TMDb copy tmdb-record.json was fetched again"), "{log}");
    assert!(!log.contains("the TMDb copy tmdb.json was fetched again"), "only listed copies are named: {log}");
}

#[test]
fn a_plan_again_from_scanned_with_a_stale_copy_it_cannot_fetch_fails_unknown() {
    let h = harness(&[], json!({}));
    let id = format!("{}-1", h.fingerprint("movie-direct").to_ascii_lowercase());
    h.insert("tok0", "movie-direct");
    h.run_until("Scanned");
    stale(&h, &id, "tmdb-record.json");
    no_token(&h);
    h.restart();
    // The store replaces state.json before it appends the transition's line, so the wait is on the event (Ruling T9-R2).
    let failed = |h: &Harness| events(h, &id).iter().any(|e| matches!(e, Event::PlanFailed { failure } if failure.class == FailureClass::Unknown));
    h.wait("PlanFailed", failed);
    assert_eq!(h.state_of(&id), "Failed");
    assert!(failed(&h));
    let message = h.record(&id).message;
    assert_eq!(message.as_deref(), Some("tmdb-record.json: the TMDb copy is more than six months old \
        (fetched_at 2026-01-01) and could not be fetched again: no tmdb_token is set"));
    assert!(!message.unwrap().contains("test-token"));
    h.secrets().set("tmdb_token", "test-token").unwrap();
    let mut c = Client::open(&h);
    c.snapshot();
    c.ok("retry", json!({ "job": id }));
    h.wait("Planned", |h| h.transitions(&id).iter().any(|l| l.event.as_ref().is_some_and(|e| matches!(e, Event::PlanDone { .. }))));
}

#[test]
fn a_stale_copy_fetched_again_replaces_the_file_and_the_decision_uses_it() {
    let h = harness(&[], json!({}));
    let id = format!("{}-1", h.fingerprint("movie-direct").to_ascii_lowercase());
    h.insert("tok0", "movie-direct");
    h.run_until("Inspecting");
    let path = h.job_dir(&id).join("tmdb-record.json");
    let mut copy = json_of(&path);
    copy["title"] = json!("Stale Title");
    copy["fetched_at"] = json!("2026-01-01");
    fs::write(&path, serde_json::to_vec_pretty(&copy).unwrap()).unwrap();
    h.restart();
    h.wait("Filed", |h| h.state_of(&id) == "Filed");
    let fresh = json_of(&path);
    assert_eq!((&fresh["title"], &fresh["fetched_at"]), (&json!("Synthetic Movie"), &json!("2026-10-23")));
    assert!(!fs::read_to_string(h.job_dir(&id).join(KEY).join("decide.json")).unwrap().contains("Stale Title"));
    assert!(final_path(&h).is_file());
    assert!(fs::read_to_string(h.job_dir(&id).join("job.log")).unwrap().contains("the TMDb copy tmdb-record.json was fetched again"));
}

#[test]
fn the_engine_never_reads_tmdb_token_from_its_environment() {
    let h = harness(&[], json!({}));
    no_token(&h);
    std::env::set_var("TMDB_TOKEN", "token-sentinel-5d2e");
    h.scan_waits_for_lookups(false);
    let id = run_to(&h, "AwaitingLookup");
    std::env::remove_var("TMDB_TOKEN");
    let requests = h.http().requests();
    assert!(requests.iter().all(|(url, _)| !url.starts_with(lazymkv_clients::TMDB)), "{requests:?}");
    assert!(requests.iter().flat_map(|(_, headers)| headers).all(|(_, value)| !value.contains("token-sentinel-5d2e")));
    let message = h.record(&id).message.unwrap();
    assert!(message.contains("search: no tmdb_token is set"), "{message}");
    assert!(!message.contains("token-sentinel-5d2e") && !message.contains("test-token"));
}

#[test]
fn an_identified_disc_with_two_feature_length_playlists_rips_only_its_main_title() {
    let h = harness(&["movie-direct-main-playlist"], json!({}));
    let id = run_to(&h, "Filed");
    let rips = rips(&h);
    assert_eq!(rips.len(), 1, "{rips:?}");
    assert!(rips[0].contains("t00-00801") && !rips[0].contains("t00-00800"), "{rips:?}");
    assert!(fs::read_to_string(h.job_dir(&id).join("lookup-status.txt")).unwrap().contains("dischash 200"));
    assert!(!h.record(&id).message.unwrap_or_default().contains("copy mode"));
}

#[test]
fn an_identified_disc_with_three_same_length_playlists_rips_the_one_its_record_names() {
    let h = harness(&["movie-direct-three-playlists"], json!({}));
    let id = run_to(&h, "Filed");
    let rips = rips(&h);
    assert_eq!(rips.len(), 1, "{rips:?}");
    assert!(rips[0].contains("t00-00802"), "{rips:?}");
    assert!(!h.record(&id).message.unwrap_or_default().contains("copy mode"));
}

#[test]
fn a_tv_disc_identified_through_its_seasons_is_held_with_no_path() {
    let h = harness(&["tv-direct-series"], json!({})); // the harness leaves tv_root unset
    let id = run_to(&h, "NeedsReview");
    let status = fs::read_to_string(h.job_dir(&id).join("lookup-status.txt")).unwrap();
    assert!(status.contains("tmdb tv/900101 200") && status.contains("season-1 200"), "{status}");
    let record = json_of(&h.job_dir(&id).join("tmdb-record.json"));
    assert_eq!(record["seasons"][0]["episodes"], json!([{"episode_number": 1, "runtime": 119}]));
    assert!(h.record(&id).titles.iter().all(|t| t.filed.is_none()));
    assert!(files_under(h.library()).is_empty(), "nothing is written under movies_root (A5, design part 6.6)");
}
