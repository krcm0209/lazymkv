//! lazymkvctl's engine commands against the real engine over the in-memory transport (design part 9.6, Task 28).

use lazymkv_core::testsupport::{Scratch, ScratchExt};
use lazymkv_ctl::{run, Env};
use lazymkv_dev::evidence::Ctx;
use lazymkv_engine::testkit::Harness;
use serde_json::{json, Value};
use std::path::Path;

/// An engine on the movie-direct scenario; with `filed_job`, its disc is taken to Filed and the engine restarted, so
/// that the job is Filed and no step runs. Returns the job's ID when there is one.
fn engine(filed_job: bool) -> (Harness, Option<String>) {
    let harness = Harness::new(
        Path::new(env!("CARGO_BIN_EXE_fake-tool")),
        &["movie-direct"],
        json!({"keep_tuning_evidence": false, "drive_settle_s": 0, "drive_ready_timeout_s": 1}),
    );
    if !filed_job {
        return (harness, None);
    }
    harness.insert("tok0", "movie-direct");
    harness.run_until("Filed");
    harness.restart();
    let id = std::fs::read_dir(harness.data().join("jobs")).unwrap().next().unwrap().unwrap().file_name();
    (harness, Some(id.into_string().unwrap()))
}

fn ctl(harness: &Harness, argv: &[&str], stdin: &[u8]) -> (i32, String, String) {
    let scratch = Scratch::new("ctl-engine");
    let ctx = Ctx::for_test(&scratch.0);
    let connector = harness.client();
    let mut input = stdin;
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let argv: Vec<String> = argv.iter().map(|word| word.to_string()).collect();
    let code = run(&argv, Env { ctx: &ctx, connector: Some(&connector), stdin: &mut input, out: &mut out, err: &mut err });
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

fn json_of(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

#[test]
fn status_prints_the_snapshot() {
    let (harness, id) = engine(true);
    let id = id.unwrap();
    let (code, out, err) = ctl(&harness, &["status"], b"");
    assert_eq!((code, err.as_str()), (0, ""));
    let snapshot = json_of(&out);
    let keys: Vec<&str> = snapshot.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(keys, ["jobs", "settings", "problems", "secrets", "selftest"]);
    assert_eq!(snapshot["jobs"][0]["id"], id.as_str());
    assert_eq!(snapshot["jobs"][0]["state"], json!({"state": "Filed"}));
    assert!(snapshot["selftest"]["checks"].is_array());
    assert!(out.starts_with("{\n \"jobs\": ["), "{out}");
}

#[test]
fn history_sends_before_and_prints_the_page() {
    let (harness, id) = engine(true);
    let id = id.unwrap();
    let (code, out, _) = ctl(&harness, &["history"], b"");
    let page = json_of(&out);
    assert_eq!((code, page.as_array().unwrap().len()), (0, 1));
    assert_eq!(page[0]["id"], id.as_str());
    let (code, out, _) = ctl(&harness, &["history", "--before", id.as_str()], b"");
    assert_eq!((code, json_of(&out)), (0, json!([])));
}

#[test]
fn settings_get_prints_the_snapshots_settings() {
    let (harness, _) = engine(false);
    let (code, out, err) = ctl(&harness, &["settings", "get"], b"");
    assert_eq!((code, err.as_str()), (0, ""));
    let settings = json_of(&out);
    assert_eq!(settings["keep_tuning_evidence"], false);
    assert_eq!(settings["drive_settle_s"], 0);
    assert_eq!(settings["cache_mb"], 1024);
    assert_eq!(settings, json_of(&ctl(&harness, &["status"], b"").1)["settings"]);
}

#[test]
fn settings_set_reads_a_file() {
    let (harness, _) = engine(false);
    let files = Scratch::new("ctl-settings-set");
    let good = files.write_path("good.json", r#"{"cache_mb": 512}"#);
    let bad = files.write_path("bad.json", r#"{"cache_mb": "big"}"#);
    let (code, out, err) = ctl(&harness, &["settings", "set", good.to_str().unwrap()], b"");
    assert_eq!((code, out.as_str(), err.as_str()), (0, "", ""));
    assert_eq!(json_of(&ctl(&harness, &["settings", "get"], b"").1)["cache_mb"], 512);
    let saved = json_of(&std::fs::read_to_string(harness.data().join("settings.json")).unwrap());
    assert_eq!(saved["cache_mb"], 512);
    let (code, _, err) = ctl(&harness, &["settings", "set", bad.to_str().unwrap()], b"");
    assert_eq!(code, 1);
    assert!(err.starts_with("lazymkvctl: "), "{err}");
    assert_eq!(json_of(&ctl(&harness, &["settings", "get"], b"").1)["cache_mb"], 512);
}

#[test]
fn secret_set_reads_the_value_from_stdin() {
    let (harness, _) = engine(false);
    let before = json_of(&ctl(&harness, &["status"], b"").1);
    assert_eq!(before["secrets"]["ha_webhook_url"]["set"], false);
    // PF-2: the secret part is a value that cannot occur in any path the status shows.
    let url = b"http://ha.local:8123/api/webhook/hook-secret-7f3a\n";
    let (code, out, err) = ctl(&harness, &["secret", "set", "ha_webhook_url"], url);
    assert_eq!((code, out.as_str(), err.as_str()), (0, "", ""));
    let (_, status, _) = ctl(&harness, &["status"], b"");
    assert_eq!(json_of(&status)["secrets"]["ha_webhook_url"]["set"], true);
    assert!(!status.contains("hook-secret-7f3a"), "{status}");
    let (code, out, err) = ctl(&harness, &["secret", "set", "nope"], b"hidden-value\n");
    assert_eq!(code, 1);
    assert!(err.starts_with("lazymkvctl: "), "{err}");
    assert!(!out.contains("hidden-value") && !err.contains("hidden-value"));
}

#[test]
fn selftest_sends_run_selftest_and_prints_the_result() {
    let (harness, _) = engine(false);
    let (code, out, err) = ctl(&harness, &["selftest"], b"");
    assert_eq!((code, err.as_str()), (0, ""));
    let result = json_of(&out);
    let checks = result["checks"].as_array().unwrap();
    let names: Vec<&str> = checks.iter().map(|check| check["name"].as_str().unwrap()).collect();
    for tool in ["makemkvcon", "mkvmerge", "ffprobe"] {
        assert!(names.contains(&tool), "{names:?}");
    }
    let makemkvcon = checks.iter().find(|check| check["name"] == "makemkvcon").unwrap();
    assert_eq!(makemkvcon["found"], "settings");
}

#[test]
fn retry_cancel_rip_again_approve_and_stop_map_ok_to_0_and_error_to_1() {
    let (harness, id) = engine(true);
    let id = id.unwrap();
    let cases: [(&[&str], i32, Option<&str>); 6] = [
        (&["rip-again", id.as_str()], 0, None),
        (&["retry", "nosuch-1"], 1, None),
        (&["cancel", id.as_str()], 1, Some("Filed")),
        (&["approve", id.as_str()], 1, Some("Filed")),
        (&["rip-again", "nosuch-1"], 1, None),
        (&["stop"], 0, None),
    ];
    for (argv, expected, names) in cases {
        let (code, out, err) = ctl(&harness, argv, b"");
        assert_eq!((code, out.as_str()), (expected, ""), "{argv:?}: {err}");
        if expected == 0 {
            assert_eq!(err, "", "{argv:?}");
        } else {
            assert!(err.starts_with("lazymkvctl: "), "{argv:?}: {err}");
            if let Some(state) = names {
                assert!(err.contains(state), "{argv:?}: {err}");
            }
        }
    }
}

#[test]
fn plan_job_with_a_refused_command_exits_2() {
    let (harness, _) = engine(false);
    let (code, out, err) = ctl(&harness, &["plan", "--job", "nosuch-1"], b"");
    assert_eq!((code, out.as_str()), (2, ""));
    assert!(err.starts_with("lazymkvctl: "), "{err}");
}
