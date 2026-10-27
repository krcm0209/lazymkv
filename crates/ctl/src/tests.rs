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
