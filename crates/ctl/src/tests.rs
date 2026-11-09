use super::*;
use lazymkv_core::testsupport::{scan_text, Scratch, ScratchExt};
use lazymkv_dev::evidence::Ctx;
use serde_json::json;

fn words(text: &[&str]) -> Vec<String> { text.iter().map(|w| w.to_string()).collect() }

/// lazymkvctl over `ctx` with no connector: (exit code, stdout, stderr).
fn ctl(ctx: &Ctx, argv: &[&str]) -> (i32, Vec<u8>, Vec<u8>) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let mut stdin: &[u8] = b"";
    let code = run(&words(argv), Env { ctx, connector: None, stdin: &mut stdin, out: &mut out, err: &mut err });
    (code, out, err)
}

/// lazymkv-dev's dispatcher over the same context.
fn dev(ctx: &Ctx, argv: &[&str]) -> (i32, Vec<u8>, Vec<u8>) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = lazymkv_dev::dispatch(ctx, &words(argv), &mut out, &mut err);
    (code, out, err)
}

/// A made-up recording recs/disc-a with the rip label eng-only, as dev's decide tests build it (`recording` in crates/dev/src/cmd/decide.rs).
fn recording(s: &Scratch) {
    s.write("recs/disc-a/scan.txt", concat!(
        "TINFO:0,8,0,\"2\"\nTINFO:0,9,0,\"0:10:00\"\n",
        "SINFO:0,0,1,6201,\"Video\"\nSINFO:0,0,6,0,\"Mpeg4\"\n",
        "SINFO:0,1,1,6202,\"Audio\"\nSINFO:0,1,3,0,\"eng\"\nSINFO:0,1,6,0,\"DTS-HD MA\"\nSINFO:0,1,14,0,\"6\"\n",
        "SINFO:0,2,1,6202,\"Audio\"\nSINFO:0,2,3,0,\"eng\"\nSINFO:0,2,6,0,\"DD\"\nSINFO:0,2,14,0,\"2\"\n",
        "SINFO:0,3,1,6202,\"Audio\"\nSINFO:0,3,3,0,\"spa\"\nSINFO:0,3,6,0,\"DD\"\nSINFO:0,3,14,0,\"2\"\n",
        "SINFO:0,4,1,6203,\"Subtitles\"\nSINFO:0,4,3,0,\"eng\"\nSINFO:0,4,6,0,\"PGS\"\n"));
    s.write("recs/disc-a/eng-only/rip-summary.json",
            json!({"ok": true, "title": 0, "profile": "C:\\made-up\\eng-only.mmcp.xml"}).to_string());
    s.write("recs/disc-a/eng-only/mkvmerge.json", json!({
        "tracks": [
            {"id": 0, "type": "video", "codec": "AVC", "properties": {"language": "und"}},
            {"id": 1, "type": "audio", "codec": "DTS-HD MA", "properties": {"language": "eng", "audio_channels": 6}},
            {"id": 2, "type": "audio", "codec": "AC-3", "properties": {"language": "eng", "audio_channels": 2}},
            {"id": 3, "type": "subtitles", "codec": "HDMV PGS", "properties": {"language": "eng"}}
        ],
        "chapters": [{"num_entries": 2}],
        "container": {"properties": {"duration": 600_000_000_000_i64}}
    }).to_string());
    s.write("recs/disc-a/eng-only/rip.txt", "MSG:5011,0,0,\"made-up line\",\"made-up line\"\n");
}

#[test]
fn local_plan_prints_what_lazymkv_dev_plan_prints() {
    let s = Scratch::new("ctl-plan");
    let scan = scan_text(
        &[(2, "SYNTHETIC_MOVIE")],
        &[(0, &[(9, "1:59:10"), (8, "12"), (16, "00800.mpls")][..],
           &[(0, &[(1, "Video")][..]), (1, &[(1, "Audio"), (3, "eng")][..])][..])],
    );
    s.write("scan.txt", scan);
    let ctx = Ctx::for_test(&s.0);
    let local = ctl(&ctx, &["plan", "scan.txt"]);
    assert!(!local.1.is_empty());
    assert_eq!(local, dev(&ctx, &["plan", "scan.txt"]));
}

#[test]
fn local_decide_and_retune_match_lazymkv_dev() {
    let s = Scratch::new("ctl-decide");
    recording(&s);
    let ctx = Ctx::for_test(&s.0);
    let decide = ctl(&ctx, &["decide", "--rec", "recs/disc-a/eng-only"]);
    assert_eq!(decide.0, 0);
    assert!(decide.1.starts_with(b"{"));
    assert_eq!(decide, dev(&ctx, &["decide", "--rec", "recs/disc-a/eng-only"]));
    std::fs::create_dir_all(s.0.join("empty")).unwrap();
    for argv in [&["retune", "--evidence", "empty"][..], &["retune", "--subtitles", "--evidence", "empty"][..]] {
        assert_eq!(ctl(&ctx, argv), dev(&ctx, argv), "{argv:?}");
    }
}

#[test]
fn another_dev_command_name_is_refused_with_exit_2() {
    let s = Scratch::new("ctl-other");
    let ctx = Ctx::for_test(&s.0);
    let (code, out, err) = ctl(&ctx, &["scan", "x"]);
    assert_eq!((code, out), (2, vec![]));
    assert_eq!(String::from_utf8(err).unwrap(), "lazymkvctl: scan is a lazymkv-dev command, not a lazymkvctl command\n");
    let (code, _, err) = ctl(&ctx, &["nonsense"]);
    assert_eq!(code, 2);
    assert!(String::from_utf8(err).unwrap().starts_with("lazymkvctl: unknown command nonsense\nusage: lazymkvctl"));
}

#[test]
fn title_without_job_is_an_argument_error() {
    let s = Scratch::new("ctl-title");
    recording(&s);
    let ctx = Ctx::for_test(&s.0);
    for (argv, message) in [
        (&["decide", "--rec", "recs/disc-a/eng-only", "--title", "t00"][..], "argument --title: only with --job"),
        (&["decide", "--job", "abc-1"][..], "argument --title: required with decide --job"),
        (&["plan", "--job", "abc-1", "--title", "t00"][..], "argument --title: not allowed with plan"),
        (&["retune", "--job", "abc-1"][..], "argument --job: not allowed with retune; use --jobs"),
        (&["plan", "--jobs"][..], "argument --jobs: only with retune"),
        (&["retune", "--jobs"][..], "the following arguments are required: --spec"),
    ] {
        let (code, out, err) = ctl(&ctx, argv);
        assert_eq!((code, out), (2, vec![]), "{argv:?}");
        let name = argv[0];
        assert_eq!(String::from_utf8(err).unwrap(), format!("lazymkvctl {name}: error: {message}\n"), "{argv:?}");
    }
}

#[test]
fn an_ambiguous_prefix_jo_is_refused() {
    let s = Scratch::new("ctl-jo");
    let ctx = Ctx::for_test(&s.0);
    let (code, _, err) = ctl(&ctx, &["plan", "--jo", "abc-1"]);
    assert_eq!(code, 2);
    assert_eq!(String::from_utf8(err).unwrap(), "lazymkvctl plan: error: ambiguous option: --jo could match --job, --jobs\n");
}

#[test]
fn an_engine_form_without_a_connector_exits_4_with_no_transport() {
    let s = Scratch::new("ctl-engine-form");
    let ctx = Ctx::for_test(&s.0);
    for argv in [&["plan", "--job", "abc-1"][..], &["decide", "--job", "abc-1", "--title", "t00-00800"][..],
                 &["retune", "--jobs", "--spec", "spec.md"][..]] {
        let (code, out, err) = ctl(&ctx, argv);
        assert_eq!((code, out), (4, vec![]), "{argv:?}");
        assert_eq!(String::from_utf8(err).unwrap(), format!("{NO_TRANSPORT}\n"));
    }
}

#[test]
fn no_arguments_print_the_usage_and_exit_2_and_help_exits_0() {
    let s = Scratch::new("ctl-usage");
    let ctx = Ctx::for_test(&s.0);
    let (code, out, err) = ctl(&ctx, &[]);
    assert_eq!((code, out), (2, vec![]));
    assert!(String::from_utf8(err).unwrap().starts_with("usage: lazymkvctl <command> [arguments]\n"));
    let (code, out, _) = ctl(&ctx, &["--help"]);
    assert_eq!(code, 0);
    assert!(String::from_utf8(out).unwrap().contains("decide --job ID --title KEY"));
}

#[test]
fn the_local_commands_are_the_three_of_design_part_9_5() {
    assert_eq!(LOCAL_COMMANDS, ["plan", "decide", "retune"]);
    for name in LOCAL_COMMANDS {
        assert!(lazymkv_dev::COMMANDS.iter().any(|(command, _, _)| *command == name), "{name}");
    }
}

/// V10's drift guard: the relay reads a relayed plan's and decide's words with the flag lists the children declare,
/// so a flag added to dev fails here until `relayed_paths` classes it as a path or not.
#[test]
fn the_relay_reads_plans_and_decides_own_flag_lists() {
    assert_eq!(
        lazymkv_dev::cmd::plan::FLAGS,
        ["--evidence", "--settings", "--tmdb-search", "--tmdb-record", "--remembered"]
    );
    assert_eq!(
        lazymkv_dev::cmd::decide::FLAGS,
        ["--rec", "--text", "--selection", "--settings", "--tmdb-search", "--tmdb-record", "--remembered", "--evidence"]
    );
}

/// V10's drift guard for retune (call 13; R5-code-72): `RETUNE_RELAY_FLAGS` is retune's own list plus `--jobs`, so a
/// flag added to retune fails here until `relayed_paths` classes it as a path or not.
#[test]
fn the_relay_reads_retunes_own_flag_list() {
    assert_eq!(
        lazymkv_dev::cmd::retune::FLAGS,
        ["--evidence", "--set", "--snapshot", "--compare", "--spec", "--allow-unmeasured", "--subtitles", "--text"]
    );
    let mut expected = lazymkv_dev::cmd::retune::FLAGS.to_vec();
    expected.push("--jobs");
    assert_eq!(RETUNE_RELAY_FLAGS.to_vec(), expected);
    assert_eq!(RETUNE_PATH_FLAGS, ["--snapshot", "--compare", "--text"]);
}

#[test]
fn ctl_plan_over_a_folder_with_a_stale_copy_and_no_token_exits_3() {
    let s = Scratch::new("ctl-stale");
    recording(&s);
    s.write("recs/disc-a/lookup-status.txt", "tmdb movie/6537 200\n");
    s.write("recs/disc-a/tmdb-record.json",
            r#"{"id": 6537, "title": "Made Up Film", "kind": "movie", "fetched_at": "2026-01-01"}"#);
    let ctx = Ctx::for_test(&s.0);
    let local = ctl(&ctx, &["plan", "recs/disc-a"]);
    assert_eq!(local.0, 3);
    assert_eq!(String::from_utf8(local.2.clone()).unwrap(),
               "lazymkv-dev plan: error: recs/disc-a/tmdb-record.json: the TMDb copy is more than six months old \
                (fetched_at 2026-01-01) and TMDB_TOKEN is not set to fetch it again\n");
    assert_eq!(local, dev(&ctx, &["plan", "recs/disc-a"]));
}

use super::licenses::{licenses_text, CRATE_NOTICES, MIT_TEXT, TMDB_ADDRESS, TMDB_NOTICE};
use super::models::export_models;
use lazymkv_ocr_models::{
    ATTRIBUTION, DETECTION, DETECTION_LEN, DETECTION_NAME, DETECTION_SHA256, LICENCE_TEXT, RECOGNITION,
    RECOGNITION_LEN, RECOGNITION_NAME, RECOGNITION_SHA256,
};
use sha2::{Digest, Sha256};

/// `text` with a final newline, added when it has none (as licenses_text ends each part).
fn ended(text: &str) -> String {
    if text.ends_with('\n') { text.to_string() } else { format!("{text}\n") }
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The four parts every `licenses` prints, in order; licenses_text joins its parts with "\n".
fn four_parts() -> Vec<String> {
    vec![
        format!("lazymkv: the MIT License\n\n{}", ended(MIT_TEXT)),
        format!("The ocrs models: CC-BY-SA-4.0\n\n{ATTRIBUTION}\n{}", ended(LICENCE_TEXT)),
        format!("The Movie Database (TMDB)\n\n{TMDB_NOTICE}\n{TMDB_ADDRESS}\n"),
        ended(CRATE_NOTICES),
    ]
}

#[test]
fn licenses_prints_its_five_parts_in_order() {
    let s = Scratch::new("ctl-licenses-five");
    s.write("release/licenses/zlib.txt", "made-up zlib licence\n");
    s.write("release/licenses/GPL-2.0.txt", "made-up GPL licence");
    s.write("release/licenses/nested/inner.txt", "made-up nested file\n");
    let exe = s.0.join("release").join("bin").join("lazymkvctl.exe");
    let mut parts = four_parts();
    parts.push("Bundled tool licence: GPL-2.0.txt\n\nmade-up GPL licence\n".to_string());
    parts.push("Bundled tool licence: zlib.txt\n\nmade-up zlib licence\n".to_string());
    assert_eq!(licenses_text(Some(&exe)).unwrap(), parts.join("\n"));
}

#[test]
fn the_bundled_tools_licences_print_only_with_a_licenses_folder_beside_bin() {
    let s = Scratch::new("ctl-licenses-beside");
    s.write("release/licenses/GPL-2.0.txt", "made-up GPL licence\n");
    let four = four_parts().join("\n");
    assert_eq!(licenses_text(None).unwrap(), four);
    assert_eq!(licenses_text(Some(&s.0.join("other").join("bin").join("lazymkvctl.exe"))).unwrap(), four);
    assert_eq!(licenses_text(Some(&s.0.join("release").join("tools").join("lazymkvctl.exe"))).unwrap(), four);
    assert_eq!(
        licenses_text(Some(&s.0.join("release").join("bin").join("lazymkvctl.exe"))).unwrap(),
        format!("{four}\nBundled tool licence: GPL-2.0.txt\n\nmade-up GPL licence\n")
    );
}

#[test]
fn licenses_prints_tmdbs_sentence_verbatim_with_its_address() {
    assert_eq!(TMDB_NOTICE, "This product uses the TMDB API but is not endorsed or certified by TMDB.");
    assert_eq!(TMDB_ADDRESS, "https://www.themoviedb.org");
    let text = licenses_text(None).unwrap();
    let lines = "\nThis product uses the TMDB API but is not endorsed or certified by TMDB.\nhttps://www.themoviedb.org\n";
    assert_eq!(text.matches(lines).count(), 1);
}

#[test]
fn the_embedded_notices_are_cargo_abouts_file_without_the_workspaces_own_packages() {
    let lines: Vec<&str> = CRATE_NOTICES.lines().collect();
    assert_eq!(lines[0], "Notices of the Rust crates compiled into lazymkv, generated by cargo-about from Cargo.lock.");
    assert!(lines.contains(&"Used by:"));
    for name in ["  ocrs", "  rten", "  serde_json"] {
        assert!(lines.contains(&name), "{name}");
    }
    assert!(!lines.iter().any(|line| line.starts_with("  lazymkv")), "the workspace's own packages are left out");
}

#[test]
fn export_models_writes_both_files_with_the_pinned_sha256() {
    let s = Scratch::new("ctl-export");
    let ctx = Ctx::for_test(&s.0);
    let (code, out, err) = ctl(&ctx, &["export-models", "models"]);
    let detection = s.0.join("models").join(DETECTION_NAME);
    let recognition = s.0.join("models").join(RECOGNITION_NAME);
    assert_eq!((code, String::from_utf8(err).unwrap()), (0, String::new()));
    assert_eq!(
        String::from_utf8(out).unwrap(),
        format!("wrote {}\nwrote {}\n", detection.display(), recognition.display())
    );
    let (d, r) = (std::fs::read(&detection).unwrap(), std::fs::read(&recognition).unwrap());
    assert_eq!((d.len(), r.len(), d.len() + r.len()), (DETECTION_LEN, RECOGNITION_LEN, 12_212_656));
    assert_eq!(sha256_hex(&d), DETECTION_SHA256);
    assert_eq!(sha256_hex(&r), RECOGNITION_SHA256);
    assert!(d == DETECTION && r == RECOGNITION, "byte-identical to the embedded models");
}

#[test]
fn export_models_refuses_an_existing_file_and_writes_neither() {
    for existing in [DETECTION_NAME, RECOGNITION_NAME] {
        let s = Scratch::new("ctl-export-refuse");
        s.write(&format!("models/{existing}"), "made-up model");
        let target = s.0.join("models").join(existing);
        let message = format!("{} exists; export-models writes neither file over an existing one", target.display());
        let ctx = Ctx::for_test(&s.0);
        let (code, out, err) = ctl(&ctx, &["export-models", "models"]);
        assert_eq!((code, out), (2, vec![]), "{existing}");
        assert_eq!(String::from_utf8(err).unwrap(), format!("lazymkvctl export-models: error: {message}\n"));
        assert_eq!(export_models(&s.0.join("models")), Err(message));
        assert_eq!(std::fs::read(&target).unwrap(), b"made-up model");
        let names: Vec<std::ffi::OsString> =
            std::fs::read_dir(s.0.join("models")).unwrap().map(|entry| entry.unwrap().file_name()).collect();
        assert_eq!(names, [std::ffi::OsString::from(existing)], "neither model was written");
    }
}

#[test]
fn licenses_and_export_models_take_exactly_their_arguments() {
    let s = Scratch::new("ctl-own-args");
    let ctx = Ctx::for_test(&s.0);
    for (argv, message) in [
        (&["licenses", "extra"][..], "lazymkvctl licenses: error: unrecognized arguments: extra"),
        (&["export-models"][..], "lazymkvctl export-models: error: the following arguments are required: DIR"),
        (&["export-models", "a", "b", "c"][..], "lazymkvctl export-models: error: unrecognized arguments: b c"),
        (&["export-models", "--help"][..], "lazymkvctl export-models: error: unrecognized arguments: --help"),
        (&["export-models", "-h"][..], "lazymkvctl export-models: error: unrecognized arguments: -h"),
    ] {
        let (code, out, err) = ctl(&ctx, argv);
        assert_eq!((code, out), (2, vec![]), "{argv:?}");
        assert_eq!(String::from_utf8(err).unwrap(), format!("{message}\n"), "{argv:?}");
    }
    assert!(!s.0.join("a").exists(), "an argument error writes nothing");
    assert!(!s.0.join("--help").exists() && !s.0.join("-h").exists(), "a flag is not taken as DIR");
}

#[test]
fn help_lists_licenses_and_export_models_among_the_commands_run_here() {
    let s = Scratch::new("ctl-own-help");
    let ctx = Ctx::for_test(&s.0);
    let (code, out, _) = ctl(&ctx, &["--help"]);
    let help = String::from_utf8(out).unwrap();
    assert_eq!(code, 0);
    assert!(help.contains("\ncommands run here (design parts 9.5, 9.6):\n  plan "), "{help}");
    assert!(help.contains("\n  licenses\n  export-models DIR\n\ncommands sent to the engine (design part 9.6):\n"), "{help}");
}

#[test]
fn licenses_and_export_models_need_no_engine() {
    let s = Scratch::new("ctl-own-local");
    let ctx = Ctx::for_test(&s.0);
    let exe = std::env::current_exe().unwrap();
    let (code, out, err) = ctl(&ctx, &["licenses"]);
    assert_eq!((code, String::from_utf8(err).unwrap()), (0, String::new()));
    assert_eq!(String::from_utf8(out).unwrap(), licenses_text(Some(&exe)).unwrap());
    assert_eq!(ctl(&ctx, &["export-models", "out"]).0, 0, "no connector, and still not exit 4");
    assert_eq!(OWN_COMMANDS, ["licenses", "export-models"]);
    for name in OWN_COMMANDS {
        assert!(!LOCAL_COMMANDS.contains(&name), "{name}");
        assert!(!engine_cmds::ENGINE_COMMANDS.iter().any(|(command, _)| *command == name), "{name}");
        assert!(!lazymkv_dev::COMMANDS.iter().any(|(command, _, _)| *command == name), "{name}");
    }
}

/// E1 and call 26: once the notices are committed, the CI job fails on a difference and still uploads the
/// regenerated file first, so the refresh is one commit of that artifact.
#[test]
fn the_ci_notices_job_fails_when_the_committed_notices_differ() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../.github/workflows/ci.yml");
    let ci = std::fs::read_to_string(path).unwrap_or_else(|error| panic!("{path}: {error}"));
    let job = &ci[ci.find("  notices:").expect("the notices job")..];
    let upload = job.find("- name: Upload the generated notices").expect("the upload step");
    let compare = job.find("- name: Compare with the committed notices").expect("the comparison step");
    assert!(upload < compare, "the artifact is uploaded before the comparison can fail");
    assert!(job[compare..].contains("if ! diff -u crates/ctl/crate-notices.txt crate-notices.txt; then"));
    assert!(job[compare..].contains("exit 1"));
    let lower = ci.to_lowercase();
    assert!(!lower.contains("report only") && !lower.contains("is not committed yet"));
}
