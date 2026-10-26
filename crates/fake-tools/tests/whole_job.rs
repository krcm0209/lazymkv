//! Design part 12.6 item 2's acceptance: a media-arrival event takes a synthetic movie disc to a filed placeholder
//! file, on stand-in tools, in-memory platform fakes and recorded HTTP answers, with no disc, no network and no
//! Windows API.

use lazymkv_clients::{dischash_url, discid_url, record_url, search_url};
use lazymkv_core::job::NotifyEvent;
use lazymkv_core::lookup::name::disc_name;
use lazymkv_core::lookup::tmdb::{TmdbIdentity, TmdbKind};
use lazymkv_dev::evidence::Ctx;
use lazymkv_dev::pyjson::{dumps, Style};
use lazymkv_engine::testkit::{ci_factor, Harness};
use lazymkv_platform::Host;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, UNIX_EPOCH};

const KEY: &str = "t00-00800";
const FILM: &str = "Synthetic Movie (2001) {tmdb-900001}";
/// The rip's own listing (RP-1, Ruling T29-b).
const LATER_LISTINGS: &[&str] = &["drive-list-2.txt"];

fn scenario_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/scenarios/movie-direct")
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

/// The names directly inside `dir`; empty when it does not exist.
fn names_in(dir: &Path) -> BTreeSet<String> {
    fs::read_dir(dir)
        .map(|entries| entries.map(|entry| entry.unwrap().file_name().into_string().unwrap()).collect())
        .unwrap_or_default()
}

fn set(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|name| name.to_string()).collect()
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

/// Starts the engine on movie-direct, inserts its disc in tok0, waits for the job's last notification and the
/// keep-awake release (Ruling T29-a), and returns the harness with the job folder.
fn filed_job() -> (Harness, PathBuf) {
    let harness = Harness::new(
        Path::new(env!("CARGO_BIN_EXE_fake-tool")),
        &["movie-direct"],
        json!({"keep_tuning_evidence": false, "drive_settle_s": 0, "drive_ready_timeout_s": 1}),
    );
    harness.insert("tok0", "movie-direct");
    // 120 s, 480 s under CI (Ruling R29.c5).
    let (start, cap) = (Instant::now(), Duration::from_secs(120) * ci_factor());
    loop {
        let last = harness.notifications().iter().any(|n| n.event != NotifyEvent::DiscDone);
        if last && harness.awake().holders() == 0 {
            break;
        }
        let sent: Vec<String> = harness.notifications().iter().map(|n| serde_json::to_string(n).unwrap()).collect();
        assert!(start.elapsed() < cap, "no last notification; sent: {sent:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
    let jobs: Vec<PathBuf> =
        fs::read_dir(harness.data().join("jobs")).unwrap().map(|entry| entry.unwrap().path()).collect();
    assert_eq!(jobs.len(), 1, "{jobs:?}");
    let job = jobs[0].clone();
    (harness, job)
}

#[test]
fn a_media_arrival_takes_a_synthetic_movie_disc_to_a_filed_placeholder() {
    let scenario_before = files_under(&scenario_dir());
    let (harness, job) = filed_job();
    let settings = read_json(&job.join("settings.json"));
    let root_text = |key: &str| settings[key].as_str().unwrap().to_string();
    let (movies_text, staging, tuning) =
        (root_text("movies_root"), PathBuf::from(root_text("staging_root")), PathBuf::from(root_text("tuning_root")));
    let movies = PathBuf::from(&movies_text);
    let meta = read_json(&job.join("meta.json"));
    let fp = meta["fingerprint"].as_str().unwrap().to_string();
    let id = format!("{}-1", fp.to_ascii_lowercase());
    assert_eq!(job.file_name().unwrap().to_str().unwrap(), id);

    // 1. The placeholder is filed at the namer's path (spec 8.9), joined with the root's own separator.
    let placeholder = fs::read(scenario_dir().join("placeholder.mkv")).unwrap();
    let filed = movies.join(FILM).join(format!("{FILM} - 1080p.mkv"));
    assert_eq!(fs::read(&filed).unwrap(), placeholder);
    let sep = if movies_text.contains('\\') { '\\' } else { '/' };
    let filed_text = format!("{movies_text}{sep}{FILM}{sep}{FILM} - 1080p.mkv");

    // 2. state.json.
    let state = read_json(&job.join("state.json"));
    assert_eq!(state["format"], "lazymkv job v1");
    assert_eq!(state["id"], id.as_str());
    assert_eq!(state["state"], json!({"state": "Filed"}));
    assert_eq!(state["filed_unix"], json!(harness.host().now_unix()));
    let mtime = fs::metadata(&filed).unwrap().modified().unwrap().duration_since(UNIX_EPOCH).unwrap().as_secs();
    assert_eq!(state["titles"][0]["key"], KEY);
    assert_eq!(
        state["titles"][0]["filed"],
        json!({"path": filed_text, "size": placeholder.len(), "mtime_unix": mtime})
    );

    // 3. transitions.jsonl: the main line of job.rs:802-842, in order.
    let lines: Vec<Value> = fs::read_to_string(job.join("transitions.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let pairs: Vec<(Option<&str>, &str)> =
        lines.iter().map(|line| (line["event"]["event"].as_str(), line["new"]["state"].as_str().unwrap())).collect();
    assert_eq!(
        pairs,
        vec![
            (None, "Detected"),
            (Some("FingerprintDone"), "Fingerprinted"),
            (Some("ScanDone"), "Scanned"),
            (Some("PlanDone"), "Planned"),
            (Some("SpaceOk"), "Ripping"),
            (Some("TitleRipped"), "Ripping"),
            (Some("TitleVerified"), "RipVerified"),
            (Some("Ejected"), "Ejected"),
            (Some("FilePhaseSlot"), "Inspecting"),
            (Some("ProbeDone"), "Inspecting"),
            (Some("DecisionDone"), "Decided"),
            (Some("SpaceOk"), "Remuxing"),
            (Some("RemuxDone"), "Remuxing"),
            (Some("FileVerified"), "Verified"),
            (Some("FileDone"), "Filed"),
        ]
    );
    assert!(lines[0]["old"].is_null() && lines[0]["event"].is_null());
    assert_eq!(lines[3]["event"], json!({"event": "PlanDone", "mode": "direct"}));
    assert_eq!(lines[6]["event"], json!({"event": "TitleVerified", "last": true}));
    assert_eq!((&lines[10]["event"]["disposition"], &lines[10]["event"]["has_path"]), (&json!("auto_file"), &json!(true)));
    for pair in lines.windows(2) {
        assert_eq!(pair[1]["old"], pair[0]["new"]);
    }
    for line in &lines {
        assert_eq!(line["time_unix"], json!(harness.host().now_unix()));
    }
    assert!(lines[14]["actions"].as_array().unwrap().contains(&json!({"action": "Notify", "event": {"event": "filed"}})),
        "{}", lines[14]["actions"]);

    // 4. Exactly disc_done then filed, with spec 12's fields.
    let sent = harness.notifications();
    let events: Vec<&NotifyEvent> = sent.iter().map(|n| &n.event).collect();
    assert_eq!(events, vec![&NotifyEvent::DiscDone, &NotifyEvent::Filed]);
    let payload_events: Vec<Value> = sent.iter().map(|n| serde_json::to_value(n).unwrap()["event"].clone()).collect();
    assert_eq!(payload_events, vec![json!("disc_done"), json!("filed")]);
    for n in &sent {
        assert!(!n.message.is_empty());
        assert_eq!(n.job_id.as_deref(), Some(id.as_str()));
        assert_eq!(n.fingerprint.as_deref(), Some(fp.as_str()));
        assert_eq!(n.drive.as_deref(), Some("tok0"));
        assert_eq!(n.title.as_deref(), Some("Synthetic Movie"));
        assert!(n.time > 0);
    }

    // 5. One eject, keep-awake released.
    assert_eq!(harness.ejector().calls(), vec!["tok0".to_string()]);
    assert_eq!(harness.awake().holders(), 0);

    // 6. The job folder holds exactly design part 7.1's files for this path.
    let mut expected = set(&[
        "meta.json", "files.json", "nav/BDMV/index.bdmv", "nav/BDMV/PLAYLIST/00800.mpls", "nav/BDMV/META/DL/bdmt_eng.xml",
        "thediscdb-hash.json", "thediscdb-discid.json", "tmdb.json", "tmdb-search.json", "tmdb-record.json",
        "lookup-status.txt", "settings.json", "state.json", "transitions.jsonl", "job.log", "job.mmcp.xml",
    ]);
    expected.insert(format!("{KEY}/rip-summary.json"));
    expected.insert(format!("{KEY}/decide.json"));
    let mut runs: Vec<String> = vec!["drive-list.txt".into(), "scan.txt".into()];
    runs.extend(LATER_LISTINGS.iter().map(|name| name.to_string()));
    for name in ["rip.txt", "mkvmerge.json", "ffprobe-streams.json", "remux-mkvmerge.txt", "partial-mkvmerge.json"] {
        runs.push(format!("{KEY}/{name}"));
    }
    for run in &runs {
        for suffix in ["", ".stderr", ".timing.jsonl", ".exit.json"] {
            expected.insert(format!("{run}{suffix}"));
        }
    }
    assert_eq!(files_under(&job), expected);

    // 7. R-B1 on stand-ins: lazymkv-dev decide over the job folder prints decide.json without its two extra keys.
    let mut decide = read_json(&job.join(KEY).join("decide.json"));
    let object = decide.as_object_mut().unwrap();
    let last_two: Vec<&str> = object.keys().rev().take(2).map(String::as_str).collect();
    assert_eq!(last_two, ["lazymkv_version", "audio_rule_version"]);
    assert_eq!(object["audio_rule_version"], json!(lazymkv_core::audio::rule::RULE_VERSION));
    // The workspace gives every package one version (Cargo.toml:7), so fake-tools' version is the engine's.
    assert_eq!(object["lazymkv_version"], json!(env!("CARGO_PKG_VERSION")));
    object.shift_remove("audio_rule_version");
    object.shift_remove("lazymkv_version");
    let path = |name: &str| job.join(name).to_str().unwrap().to_string();
    let mut argv: Vec<String> = vec![
        "decide".into(), "--rec".into(), path(KEY), "--settings".into(), path("settings.json"),
        "--tmdb-search".into(), path("tmdb-search.json"), "--tmdb-record".into(), path("tmdb-record.json"),
    ];
    if job.join("remembered.json").exists() {
        argv.extend(["--remembered".to_string(), path("remembered.json")]);
    }
    let ctx = Ctx::for_test(&job);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = lazymkv_dev::dispatch(&ctx, &argv, &mut out, &mut err);
    assert_eq!((code, String::from_utf8(err).unwrap()), (0, String::new()));
    assert_eq!(String::from_utf8(out).unwrap(), dumps(&decide, Style::INDENT1_ASCII) + "\n");

    // 8. Nothing written outside the scratch roots and the data folder.
    assert_eq!(files_under(&scenario_dir()), scenario_before);
    assert!(files_under(&tuning).is_empty());
    assert_eq!(names_in(harness.data().as_ref()), set(&["drives.json", "engine.log", "jobs", "selftest", "settings.json"]));
    // The MakeMKV check at engine start (S79) and its recorded state (RP-9).
    assert_eq!(
        files_under(&harness.data().join("selftest")),
        set(&["drive-list.txt", "drive-list.txt.stderr", "drive-list.txt.timing.jsonl", "drive-list.txt.exit.json",
              "makemkv-check.json"])
    );
    assert_eq!(names_in(&staging), set(&[id.as_str()]));
    assert_eq!(names_in(&movies), set(&[".lazymkv-partial", FILM]));

    // 9. The staged raw file stays (retention, design part 5.9); no partial file is left.
    assert_eq!(names_in(&staging.join(&id).join("rip").join(KEY)), set(&["title_t00.mkv"]));
    assert!(names_in(&movies.join(".lazymkv-partial")).is_empty());

    // 10. Upper-case fingerprints, no notes, and the drive record after the removal its own eject causes (Task 22 Step 3
    // item 6: every removal writes a null fingerprint and handled false).
    assert!(fp.len() == 32 && fp.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_lowercase()), "{fp}");
    assert_eq!(state["fingerprint"], fp.as_str());
    assert_eq!(state["notes"], json!([]));
    assert_eq!(
        read_json(&harness.data().join("drives.json")),
        json!({"format": "lazymkv drives v1", "drives": {"tok0": {"fingerprint": null, "handled": false}}, "drive_incapable": []})
    );
}

#[test]
fn the_whole_job_runs_with_no_unrecorded_url() {
    let (harness, job) = filed_job();
    let meta = read_json(&job.join("meta.json"));
    let name = disc_name(Some("Synthetic Movie"), Some("SYNTHETIC_MOVIE")).unwrap();
    let urls: Vec<String> = harness.http().requests().into_iter().map(|(url, _)| url).collect();
    assert_eq!(
        urls,
        vec![
            dischash_url(meta["fingerprint"].as_str().unwrap()),
            discid_url(meta["disc_id"].as_str().unwrap()),
            search_url(&name.cleaned),
            record_url(TmdbIdentity { kind: TmdbKind::Movie, id: 900001 }),
        ]
    );
    assert_eq!(harness.http().unrecorded(), Vec::<String>::new());
    assert!(harness.notifications().iter().all(|n| matches!(n.event, NotifyEvent::DiscDone | NotifyEvent::Filed)));
}
