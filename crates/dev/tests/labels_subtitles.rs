//! `lazymkv-dev labels --subtitles` (RP15; design spec 15 "Recordings", section 17 row 14, D20, D23) over synthetic
//! recordings in a scratch folder: `set` writes `<slug_dir>/subtitle-labels.json` beside `audio-labels.json`, keeps a
//! label with the same substance as it is, keeps a replaced label in its history, says when a source is only an
//! observation, records a UK-style SDH track with `--style uk` (RP25), and refuses a change that would add a problem; `unset` moves a label to `removed` with why and when;
//! `check` validates every subtitle labels file under a folder; `kinds` prints the vocabulary; the argument errors are
//! argparse's; the audio commands are as before. The subtitle words read no spec (what counts towards row 14 is
//! `retune --subtitles`'), so no test writes one. The last test runs `check` over the evidence clone and skips with
//! its reason without it.

use std::fs;
use std::path::{Path, PathBuf};

use lazymkv_core::subtitle::labels::KINDS;
use lazymkv_core::testsupport::{Scratch, ScratchExt};
use lazymkv_dev::cmd::labels::run;
use lazymkv_dev::evidence::{repo_root, skip, Ctx};
use lazymkv_dev::COMMANDS;
use serde_json::{json, Value};

/// An audio labels file of disc-b, which no subtitle command may touch.
const AUDIO_LABELS: &str = r#"{"format": "lazymkv audio labels v1", "slug": "disc-b", "titles": {}, "tracks": []}"#;

/// disc-b/subtitle-labels.json after `labels --subtitles set disc-b all-tracks 4 sdh --how read --saw-verdict no --date 2026-10-14
/// --note "sound cues in brackets"` on a recording without one: `pyjson::write_labels`' form (indent 2, UTF-8, a
/// final newline).
const FIRST_FILE: &str = concat!(
    "{\n",
    "  \"format\": \"lazymkv subtitle labels v1\",\n",
    "  \"slug\": \"disc-b\",\n",
    "  \"tracks\": [\n",
    "    {\n",
    "      \"label\": \"all-tracks\",\n",
    "      \"stream\": 4,\n",
    "      \"kind\": \"sdh\",\n",
    "      \"how\": \"read\",\n",
    "      \"sure\": true,\n",
    "      \"date\": \"2026-10-14\",\n",
    "      \"saw_verdict\": false,\n",
    "      \"note\": \"sound cues in brackets\",\n",
    "      \"style\": null\n",
    "    }\n",
    "  ]\n",
    "}\n",
);

/// The same label as FIRST_FILE's, written on one line by hand.
const ONE_LINE_FILE: &str = concat!(
    r#"{"format":"lazymkv subtitle labels v1","slug":"disc-b","tracks":[{"label":"all-tracks","stream":4,"kind":"sdh","#,
    r#""how":"read","sure":true,"date":"2026-10-14","saw_verdict":false,"note":"sound cues in brackets","style":null}]}"#,
);

/// Runs `lazymkv-dev labels <args>` with `cwd` as the current directory; (exit code, stdout, stderr).
fn call(cwd: &Path, args: &[&str]) -> (i32, String, String) {
    let args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run(&Ctx::for_test(cwd), &args, &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

/// A scratch folder holding the empty recording folder disc-b; returns the folder and disc-b's subtitle labels path.
fn scratch_with_disc_b(tag: &str) -> (Scratch, PathBuf) {
    let tmp = Scratch::new(tag);
    fs::create_dir_all(tmp.0.join("disc-b")).unwrap();
    let file = tmp.0.join("disc-b").join("subtitle-labels.json");
    (tmp, file)
}

/// A JSON file read back.
fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

#[test]
fn the_labels_row_of_commands_names_the_subtitles_option() {
    let row = COMMANDS.iter().find(|(name, _, _)| *name == "labels").expect("labels is registered");
    assert_eq!(row.2, "[--spec FILE] [--subtitles] kinds|check|set|unset|title|auto|fitted ...");
}

#[test]
fn set_writes_a_new_subtitle_labels_file_and_leaves_the_audio_labels_alone() {
    let (tmp, file) = scratch_with_disc_b("labels-subtitles");
    tmp.write("disc-b/audio-labels.json", AUDIO_LABELS);
    let args =
        ["--subtitles", "set", "disc-b", "all-tracks", "4", "sdh", "--how", "read", "--saw-verdict", "no", "--date", "2026-10-14",
        "--note", "sound cues in brackets"];
    assert_eq!(
        call(&tmp.0, &args),
        (0, "labelled all-tracks stream 4: sdh (read, 2026-10-14, saw verdict no, sure yes)\n".to_string(), String::new())
    );
    assert_eq!(fs::read_to_string(&file).unwrap(), FIRST_FILE);
    // --saw-verdict yes, --sure no, the default date (the test clock's 2026-10-13) and no note (null).
    let args =
        ["--subtitles", "set", "disc-b", "all-tracks", "5", "regular", "--how", "watched", "--saw-verdict", "yes", "--sure", "no"];
    assert_eq!(
        call(&tmp.0, &args),
        (0, "labelled all-tracks stream 5: regular (watched, 2026-10-13, saw verdict yes, sure no)\n".to_string(), String::new())
    );
    assert_eq!(
        read_json(&file)["tracks"][1],
        json!({"label": "all-tracks", "stream": 5, "kind": "regular", "how": "watched", "sure": false, "date": "2026-10-13",
               "saw_verdict": true, "note": null, "style": null})
    );
    assert_eq!(fs::read_to_string(tmp.0.join("disc-b/audio-labels.json")).unwrap(), AUDIO_LABELS);
}

#[test]
fn set_with_the_same_substance_writes_nothing() {
    let (tmp, file) = scratch_with_disc_b("labels-subtitles");
    tmp.write("disc-b/subtitle-labels.json", ONE_LINE_FILE);
    let args =
        ["--subtitles", "set", "disc-b", "all-tracks", "4", "sdh", "--how", "read", "--saw-verdict", "no", "--date", "2026-10-20",
        "--note", "sound cues in brackets"];
    assert_eq!(
        call(&tmp.0, &args),
        (0, "unchanged all-tracks stream 4: sdh (read, 2026-10-14, saw verdict no, sure yes)\n".to_string(), String::new())
    );
    // Not rewritten: the hand-written line is still one line.
    assert_eq!(fs::read_to_string(&file).unwrap(), ONE_LINE_FILE);
}

#[test]
fn set_replaces_a_label_and_keeps_the_old_one_in_its_history() {
    let (tmp, file) = scratch_with_disc_b("labels-subtitles");
    tmp.write("disc-b/subtitle-labels.json", ONE_LINE_FILE);
    let args =
        ["--subtitles", "set", "disc-b", "all-tracks", "4", "regular", "--how", "read", "--saw-verdict", "no", "--date", "2026-10-15",
        "--note", "no cues after all"];
    assert_eq!(
        call(&tmp.0, &args),
        (
            0,
            "labelled all-tracks stream 4: regular (read, 2026-10-15, saw verdict no, sure yes); replaces 1 earlier label(s), kept in its history\n"
                .to_string(),
            String::new()
        )
    );
    assert_eq!(
        read_json(&file)["tracks"],
        json!([{
            "label": "all-tracks", "stream": 4, "kind": "regular", "how": "read", "sure": true, "date": "2026-10-15",
            "saw_verdict": false, "note": "no cues after all", "style": null,
            "history": [{"label": "all-tracks", "stream": 4, "kind": "sdh", "how": "read", "sure": true, "date": "2026-10-14", "saw_verdict": false,
                         "note": "sound cues in brackets", "style": null, "replaced": "2026-10-15"}],
        }])
    );
}

#[test]
fn an_observation_is_labelled_with_why_it_does_not_count() {
    let (tmp, _) = scratch_with_disc_b("labels-subtitles");
    for (stream, how) in [("6", "packaging"), ("7", "menu"), ("8", "flag"), ("9", "player"), ("10", "thediscdb")] {
        let args =
            ["--subtitles", "set", "disc-b", "all-tracks", stream, "forced", "--how", how, "--saw-verdict", "no", "--date", "2026-10-14"];
        let line = format!(
            "labelled all-tracks stream {stream}: forced ({how}, 2026-10-14, saw verdict no, sure yes) ('{how}' is an observation, not a check of the track's text (row 14, D23))\n"
        );
        assert_eq!(call(&tmp.0, &args), (0, line, String::new()), "{how}");
    }
}

#[test]
fn set_records_a_uk_style_sdh_track_with_style_uk() {
    let (tmp, file) = scratch_with_disc_b("labels-subtitles");
    let args = [
        "--subtitles", "set", "disc-b", "all-tracks", "4", "sdh", "--how", "watched", "--saw-verdict", "no", "--date", "2026-10-14",
        "--style", "uk",
    ];
    assert_eq!(
        call(&tmp.0, &args),
        (0, "labelled all-tracks stream 4: sdh (watched, 2026-10-14, saw verdict no, sure yes, style uk)\n".to_string(), String::new())
    );
    assert_eq!(
        read_json(&file)["tracks"][0],
        json!({"label": "all-tracks", "stream": 4, "kind": "sdh", "how": "watched", "sure": true, "date": "2026-10-14",
               "saw_verdict": false, "note": null, "style": "uk"})
    );
    // The same label without --style is a new label (style null); the UK-style one is kept in its history.
    let args =
        ["--subtitles", "set", "disc-b", "all-tracks", "4", "sdh", "--how", "watched", "--saw-verdict", "no", "--date", "2026-10-15"];
    assert_eq!(
        call(&tmp.0, &args),
        (
            0,
            "labelled all-tracks stream 4: sdh (watched, 2026-10-15, saw verdict no, sure yes); replaces 1 earlier label(s), kept in its history\n"
                .to_string(),
            String::new()
        )
    );
    let written = read_json(&file);
    assert_eq!(written["tracks"][0]["style"], json!(null));
    assert_eq!(written["tracks"][0]["history"][0]["style"], json!("uk"));
}

#[test]
fn set_refuses_a_change_that_adds_a_problem_and_writes_nothing() {
    let (tmp, file) = scratch_with_disc_b("labels-subtitles");
    let cases: [(&[&str], &str); 3] = [
        (&["All-Tracks", "4", "sdh", "--date", "2026-10-14"], "track All-Tracks stream 4: label is not a rip label"),
        (&["all-tracks", "4", "sdh", "--date", "2026-13-01"], "track all-tracks stream 4: date '2026-13-01' is not YYYY-MM-DD"),
        (
            &["all-tracks", "-1", "sdh", "--date", "2026-10-14"],
            "track all-tracks stream -1: stream is not ffprobe's stream number (an integer, 0 or more)",
        ),
    ];
    for (words, problem) in cases {
        let mut args = vec!["--subtitles", "set", "disc-b"];
        args.extend_from_slice(words);
        args.extend_from_slice(&["--how", "read", "--saw-verdict", "no"]);
        assert_eq!(
            call(&tmp.0, &args),
            (2, String::new(), format!("disc-b/subtitle-labels.json: not written: {problem}\n")),
            "{words:?}"
        );
        assert!(!file.exists(), "{words:?}");
    }
}

#[test]
fn a_file_that_is_not_json_is_named_and_kept_and_a_byte_order_mark_is_read() {
    let (tmp, file) = scratch_with_disc_b("labels-subtitles");
    tmp.write("disc-b/subtitle-labels.json", "{broken");
    let args = ["--subtitles", "set", "disc-b", "all-tracks", "4", "sdh", "--how", "read", "--saw-verdict", "no"];
    let (code, out, err) = call(&tmp.0, &args);
    assert_eq!((code, out.as_str()), (2, ""));
    assert!(err.starts_with("disc-b/subtitle-labels.json: not valid JSON ("), "{err}");
    assert!(err.ends_with("); restore it from git (git checkout -- disc-b/subtitle-labels.json)\n"), "{err}");
    assert_eq!(fs::read_to_string(&file).unwrap(), "{broken");
    tmp.write("disc-b/subtitle-labels.json", format!("\u{feff}{ONE_LINE_FILE}"));
    let again =
        ["--subtitles", "set", "disc-b", "all-tracks", "4", "sdh", "--how", "read", "--saw-verdict", "no", "--note", "sound cues in brackets"];
    assert_eq!(call(&tmp.0, &again), (0, "unchanged all-tracks stream 4: sdh (read, 2026-10-14, saw verdict no, sure yes)\n".to_string(), String::new()));
}

#[test]
fn unset_moves_the_label_to_removed_with_why_and_when() {
    let (tmp, file) = scratch_with_disc_b("labels-subtitles");
    tmp.write("disc-b/subtitle-labels.json", ONE_LINE_FILE);
    let args = ["--subtitles", "unset", "disc-b", "all-tracks", "4", "--note", "wrong stream", "--date", "2026-10-16"];
    assert_eq!(call(&tmp.0, &args), (0, "unset all-tracks stream 4: the label is kept under 'removed'\n".to_string(), String::new()));
    assert_eq!(
        read_json(&file),
        json!({
            "format": "lazymkv subtitle labels v1", "slug": "disc-b", "tracks": [],
            "removed": [{"label": "all-tracks", "stream": 4, "kind": "sdh", "how": "read", "sure": true, "date": "2026-10-14", "saw_verdict": false,
                         "note": "sound cues in brackets", "style": null, "removed": "2026-10-16", "why": "wrong stream"}],
        })
    );
    assert_eq!(
        call(&tmp.0, &["--subtitles", "check", "disc-b"]),
        (0, "ok disc-b/subtitle-labels.json (0 tracks)\n".to_string(), String::new())
    );
}

#[test]
fn unset_and_set_errors_name_the_file_or_the_folder_and_change_nothing() {
    let (tmp, file) = scratch_with_disc_b("labels-subtitles");
    let cases: Vec<(Vec<&str>, &str)> = vec![
        (
            vec!["--subtitles", "set", "nope", "all-tracks", "4", "sdh", "--how", "read", "--saw-verdict", "no"],
            "nope: no such recording folder",
        ),
        (vec!["--subtitles", "unset", "disc-b", "all-tracks", "9", "--note", "x"], "disc-b/subtitle-labels.json: all-tracks stream 9 has no label"),
    ];
    for (args, line) in &cases {
        assert_eq!(call(&tmp.0, args), (2, String::new(), format!("{line}\n")), "{args:?}");
    }
    assert!(!file.exists());
    tmp.write("disc-b/subtitle-labels.json", ONE_LINE_FILE);
    let cases: Vec<(Vec<&str>, &str)> = vec![
        (
            vec!["--subtitles", "unset", "disc-b", "all-tracks", "4", "--note", ""],
            "disc-b/subtitle-labels.json: unset needs --note: why the label is taken away",
        ),
        (
            vec!["--subtitles", "unset", "disc-b", "all-tracks", "4", "--note", "wrong stream", "--date", "yesterday"],
            "disc-b/subtitle-labels.json: not written: removed 0: removed 'yesterday' is not YYYY-MM-DD",
        ),
    ];
    for (args, line) in &cases {
        assert_eq!(call(&tmp.0, args), (2, String::new(), format!("{line}\n")), "{args:?}");
        assert_eq!(fs::read_to_string(&file).unwrap(), ONE_LINE_FILE, "{args:?}");
    }
}

#[test]
fn check_validates_each_subtitle_labels_file() {
    let tmp = Scratch::new("labels-subtitles");
    tmp.write("corpus/disc-b/subtitle-labels.json", ONE_LINE_FILE);
    let bad = json!({"format": "lazymkv subtitle labels v1", "slug": "disc-x", "tracks": [
        {"label": "all-tracks", "stream": 4, "kind": "sdh", "how": "read", "sure": "yes", "date": "2026-10-14", "saw_verdict": false,
         "note": null, "style": null}]});
    tmp.write("corpus/disc-c/subtitle-labels.json", serde_json::to_string(&bad).unwrap());
    fs::create_dir_all(tmp.0.join("corpus/disc-d")).unwrap();
    tmp.write("corpus/disc-d/audio-labels.json", AUDIO_LABELS);
    let both = concat!(
        "ok corpus/disc-b/subtitle-labels.json (1 tracks)\n",
        "corpus/disc-c/subtitle-labels.json: slug is 'disc-x' but the file is in corpus/owner/disc-c; ",
        "track all-tracks stream 4: sure must be true or false\n",
    );
    assert_eq!(call(&tmp.0, &["--subtitles", "check", "corpus"]), (2, both.to_string(), String::new()));
    assert_eq!(
        call(&tmp.0, &["--subtitles", "check", "corpus/disc-b"]),
        (0, "ok corpus/disc-b/subtitle-labels.json (1 tracks)\n".to_string(), String::new())
    );
    assert_eq!(
        call(&tmp.0, &["--subtitles", "check", "corpus/disc-d"]),
        (0, "corpus/disc-d: no subtitle-labels.json\n".to_string(), String::new())
    );
    assert_eq!(call(&tmp.0, &["--subtitles", "check", "nope"]), (0, "nope: no subtitle-labels.json\n".to_string(), String::new()));
    // Run inside the recording, the slug is compared with the folder's real name.
    assert_eq!(
        call(&tmp.0.join("corpus/disc-b"), &["--subtitles", "check", "."]),
        (0, "ok subtitle-labels.json (1 tracks)\n".to_string(), String::new())
    );
}

#[test]
fn kinds_prints_the_subtitle_vocabulary_and_the_sources() {
    let tmp = Scratch::new("labels-subtitles");
    let (code, out, err) = call(&tmp.0, &["--subtitles", "kinds"]);
    assert_eq!((code, err.as_str()), (0, ""));
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 11, "{out}");
    assert_eq!(lines[0], "regular          keep     plain dialogue subtitles: the words spoken, with no sound cues or speaker labels (R5, D20)");
    assert_eq!(lines[8], "closed-captions  keep     the DVD's closed captions, which MakeMKV converts to text; SDH by type (8.7)");
    for (line, (kind, wanted, meaning)) in lines.iter().zip(KINDS.iter()) {
        assert_eq!(*line, format!("{kind:<16} {wanted:<8} {meaning}"));
    }
    assert!(lines[10].starts_with("how: read = a person read its OCR text (counts, D23); watched = "), "{out}");
    assert!(lines[10].ends_with("; thediscdb = TheDiscDB's track description (an observation: does not count on its own, D23)"), "{out}");
    let (code, out, err) = call(&tmp.0, &["--subtitles", "kinds", "extra"]);
    assert_eq!((code, out.as_str()), (2, ""));
    assert!(err.lines().any(|line| line == "lazymkv-dev labels: error: unrecognized arguments: extra"), "{err}");
}

#[test]
fn help_prints_the_subtitle_synopsis() {
    let tmp = Scratch::new("labels-subtitles");
    let cases = [
        (vec!["--subtitles", "kinds", "--help"], "labels --subtitles kinds"),
        (vec!["--subtitles", "check", "-h"], "labels --subtitles check <path> ..."),
        (
            vec!["--subtitles", "set", "--help"],
            "labels --subtitles set <slug_dir> <label> <stream> <kind> --how HOW --saw-verdict yes|no [--sure yes|no] [--date YYYY-MM-DD] [--note TEXT] [--style uk]",
        ),
        (vec!["--subtitles", "unset", "-h"], "labels --subtitles unset <slug_dir> <label> <stream> --note TEXT [--date YYYY-MM-DD]"),
        (vec!["--help"], "labels [--spec FILE] [--subtitles] kinds|check|set|unset|title|auto|fitted ..."),
    ];
    for (args, usage) in cases {
        assert_eq!(call(&tmp.0, &args), (0, format!("usage: lazymkv-dev {usage}\n"), String::new()), "{args:?}");
    }
}

#[test]
fn argument_errors_are_reported_as_argparse_reports_them() {
    let (tmp, file) = scratch_with_disc_b("labels-subtitles");
    let kinds = "'regular', 'sdh', 'forced', 'commentary', 'trivia', 'lyrics', 'translation', 'dub-transcript', 'closed-captions', 'other'";
    let only = "(with --subtitles, choose from 'kinds', 'check', 'set', 'unset')";
    let cases: Vec<(Vec<&str>, String)> = vec![
        (
            vec!["--subtitles", "set", "disc-b", "all-tracks", "4", "dialogue", "--how", "read", "--saw-verdict", "no"],
            format!("lazymkv-dev labels set: error: argument kind: invalid choice: 'dialogue' (choose from {kinds})"),
        ),
        (
            vec!["--subtitles", "set", "disc-b", "all-tracks", "4", "sdh", "--how", "listened", "--saw-verdict", "no"],
            "lazymkv-dev labels set: error: argument --how: invalid choice: 'listened' (choose from 'read', 'watched', 'packaging', 'menu', 'flag', 'player', 'thediscdb')"
                .to_string(),
        ),
        (
            vec!["--subtitles", "set", "disc-b", "all-tracks", "x", "sdh", "--how", "read", "--saw-verdict", "no"],
            "lazymkv-dev labels set: error: argument stream: invalid int value: 'x'".to_string(),
        ),
        (
            vec!["--subtitles", "set", "disc-b", "all-tracks", "4", "sdh", "--how", "read", "--saw-verdict", "no", "--sure", "maybe"],
            "lazymkv-dev labels set: error: argument --sure: 'maybe': say yes or no".to_string(),
        ),
        (
            vec!["--subtitles", "set", "disc-b", "all-tracks", "4", "sdh", "--how", "read", "--saw-verdict", "no", "--style", "british"],
            "lazymkv-dev labels set: error: argument --style: invalid choice: 'british' (choose from 'uk')".to_string(),
        ),
        (
            vec!["--subtitles", "set", "disc-b", "all-tracks"],
            "lazymkv-dev labels set: error: the following arguments are required: stream, kind, --how, --saw-verdict".to_string(),
        ),
        (
            vec!["--subtitles", "set", "disc-b", "all-tracks", "4", "sdh", "--how", "read", "--saw-verdict", "perhaps"],
            "lazymkv-dev labels set: error: argument --saw-verdict: 'perhaps': say yes or no".to_string(),
        ),
        (
            vec!["--subtitles", "set", "disc-b", "all-tracks", "4", "sdh", "--how", "read", "--saw-verdict", "no", "--film-audible", "yes"],
            "lazymkv-dev labels: error: unrecognized arguments: --film-audible".to_string(),
        ),
        (
            vec!["--subtitles", "unset", "disc-b", "all-tracks", "4"],
            "lazymkv-dev labels unset: error: the following arguments are required: --note".to_string(),
        ),
        (
            vec!["--subtitles", "check"],
            "lazymkv-dev labels check: error: the following arguments are required: paths".to_string(),
        ),
        (
            vec!["--subtitles", "title", "disc-b", "t0", "--title-kind", "feature"],
            format!("lazymkv-dev labels: error: argument cmd: 'title' does not take --subtitles {only}"),
        ),
        (vec!["--subtitles", "auto", "disc-b"], format!("lazymkv-dev labels: error: argument cmd: 'auto' does not take --subtitles {only}")),
        (
            vec!["--subtitles", "fitted", ".", "disc-b", "--rule", "x"],
            format!("lazymkv-dev labels: error: argument cmd: 'fitted' does not take --subtitles {only}"),
        ),
        (
            vec!["--subtitles", "frob"],
            "lazymkv-dev labels: error: argument cmd: invalid choice: 'frob' (choose from 'kinds', 'check', 'set', 'unset', 'title', 'auto', 'fitted')"
                .to_string(),
        ),
        (vec!["--subtitles"], "lazymkv-dev labels: error: the following arguments are required: cmd".to_string()),
        (
            vec!["--s", "spec.md", "kinds"],
            "lazymkv-dev labels: error: ambiguous option: --s could match --spec, --subtitles".to_string(),
        ),
        (
            vec!["--subtitles=yes", "kinds"],
            "lazymkv-dev labels: error: argument --subtitles: ignored explicit argument 'yes'".to_string(),
        ),
    ];
    for (args, line) in cases {
        let (code, out, err) = call(&tmp.0, &args);
        assert_eq!((code, out.as_str()), (2, ""), "{args:?}");
        assert!(err.starts_with("usage: lazymkv-dev labels"), "{args:?}: {err}");
        assert!(err.lines().any(|l| l == line), "{args:?}: {err}");
    }
    assert!(!file.exists());
}

#[test]
fn the_option_takes_argparses_prefixes_reads_no_spec_and_leaves_the_audio_words_as_they_were() {
    let tmp = Scratch::new("labels-subtitles");
    let (code, out, err) = call(&tmp.0, &["--su", "kinds"]);
    assert_eq!((code, out.lines().count(), err.as_str()), (0, 11, ""));
    // nospec.md does not exist: the subtitle words never read it.
    let (code, out, err) = call(&tmp.0, &["--sp", "nospec.md", "--subtitles", "check", "nope"]);
    assert_eq!((code, out.as_str(), err.as_str()), (0, "nope: no subtitle-labels.json\n", ""));
    // Without --subtitles: the audio vocabulary, and the audio words still read the spec.
    let (code, out, err) = call(&tmp.0, &["kinds"]);
    assert_eq!((code, out.lines().count(), err.as_str()), (0, 19, ""));
    assert!(out.starts_with("main                 programme  keep     "), "{out}");
    let (code, out, err) = call(&tmp.0, &["--spec", "nospec.md", "check", "."]);
    assert_eq!((code, out.as_str()), (2, ""));
    assert!(err.starts_with("cannot read the fitting disc from nospec.md: "), "{err}");
}

#[test]
fn the_subtitle_labels_of_the_evidence_clone_validate() {
    let Some(repo) = repo_root(Path::new(env!("CARGO_MANIFEST_DIR"))) else {
        skip("no repository root (a Cargo.toml with [workspace]) above crates/dev");
        return;
    };
    if !repo.join("corpus").join("owner").is_dir() {
        skip("the evidence repository is not cloned at corpus/owner");
        return;
    }
    let (code, out, err) = call(&repo, &["--subtitles", "check", "corpus/owner"]);
    assert_eq!((code, err.as_str()), (0, ""), "{out}");
    assert!(
        out == "corpus/owner: no subtitle-labels.json\n" || out.lines().all(|line| line.starts_with("ok corpus/owner/")),
        "{out}"
    );
}
