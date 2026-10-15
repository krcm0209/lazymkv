//! Tests of `lazymkv-dev labels`. `spec` and `labels` port six tests of test_audio_dataset.py's Spec and Labels
//! classes one for one, under their Python names (they ran audio_labels.main through run_labels, or read the
//! repository's spec). `command` is new: Review Focus 4 (a note in another script and an unknown key survive an edit,
//! the file written with ensure_ascii off), auto's single write, argparse's sentences, the Python's error sentences and
//! the evidence clone. Every expected line and file was printed or written by the Python of commit 75fa76a (none
//! of them holds one of the two sentences whose script name core::audio::labels rewrites).

use super::*;
use crate::evidence::{repo_root, skip};
use lazymkv_core::audio::labels::{
    exp13_requirements, load_fitted, not_counted_because, problems, LabelEntry, COUNTING_HOW, FORMAT, OBSERVATION_HOW,
};
use serde_json::json;
use crate::testutil::{self, Scratch};
use std::fs;

/// The test spec: section 17's table head and row 13 with test_audio_dataset.SPEC's numbers. Py: SPEC.
const SPEC: &str = concat!(
    "# spec\n\n## 17. Experiments (phase 0)\n\n| # | Question | If no |\n|---|---|---|\n",
    "| 13 | Measured over whole titles on the owner's own discs, does the content rule give its commentary verdict to ",
    "**no** programme track, and to at least one commentary on each of at least 3 different discs? Before ",
    "the answer may be yes, the labelled tracks must include: at least 8 commentaries from at least 4 discs, ",
    "at least 2 of them on feature films and at least 1 with the film clearly audible under the voices; at ",
    "least 1 audio-description track if the owner has a disc with one (all that the owner has, up to 3); at least ",
    "20 programme tracks other than the reference, at least 6 of them separately encoded lossy companions or ",
    "alternate mixes; at least 1 title whose reference has no centre channel (mono or 2.0) and at least ",
    "1 dialogue-heavy or speech-only title, each with an extra track. Every threshold was fitted on two tracks of ",
    "one disc (`fit-disc`: the S03E16 commentary), so no track of that disc counts. | fallback |\n",
);

/// A fresh scratch folder holding spec.md (testutil's, removed when dropped).
fn scratch(tag: &str) -> Scratch {
    let tmp = Scratch::new(tag);
    fs::write(tmp.0.join("spec.md"), SPEC).unwrap();
    tmp
}

/// Runs `lazymkv-dev labels <args>` with `cwd` as the current directory; (exit code, stdout, stderr).
fn call(cwd: &Path, args: &[&str]) -> (i32, String, String) {
    testutil::call(run, &Ctx::for_test(cwd), args)
}

/// The exit code and stdout followed by stderr, as the Python test's run_labels.
fn call_all(cwd: &Path, args: &[&str]) -> (i32, String) {
    let (code, out, err) = call(cwd, args);
    (code, out + &err)
}

/// A JSON file read back.
fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

/// A track label: how "listened", sure, the verdict not seen, dated 2026-10-05. Py: test_audio_dataset.entry.
fn entry(label: &str, stream: i64, kind: &str) -> Value {
    json!({"label": label, "stream": stream, "kind": kind, "how": "listened", "sure": true, "date": "2026-10-05",
           "saw_verdict": false, "note": ""})
}

/// `dict(value, key=v)`.
fn with(mut value: Value, key: &str, v: Value) -> Value {
    value.as_object_mut().expect("an object").insert(key.to_string(), v);
    value
}

/// A labels file as test_audio_dataset.write_labels writes it (one line, the folder's name as slug).
fn write_labels_file(slug_dir: &Path, tracks: Vec<Value>, titles: Value) {
    fs::create_dir_all(slug_dir).unwrap();
    let slug = slug_dir.file_name().unwrap().to_str().unwrap();
    let data = json!({"format": FORMAT, "slug": slug, "titles": titles, "tracks": tracks});
    fs::write(slug_dir.join("audio-labels.json"), serde_json::to_string(&data).unwrap()).unwrap();
}

/// disc-d of the Python's Routine auto test on disk, without level series (auto reads none): t0 measured with streams
/// 1 to 7 (2 and 5 the same packets, 3 and 7 the main's, 6 flagged 2304), t1 with streams 1 and 2 while the scan's
/// title 1 has three audio streams; the owner labelled t0 stream 3.
fn write_disc_d(slug_dir: &Path) {
    let recordings: [(&str, &str, i64, &[(i64, &str)]); 2] = [
        ("t0", "aa", 0, &[(2, "bb"), (3, "aa"), (4, "cc"), (5, "bb"), (6, "dd"), (7, "aa")]),
        ("t1", "ee", 1, &[(2, "ff")]),
    ];
    for (label, main_md5, title, cands) in recordings {
        let numbers: Vec<String> = std::iter::once(1).chain(cands.iter().map(|(n, _)| *n)).map(|n| n.to_string()).collect();
        let dir = slug_dir.join(label).join(format!("audio-levels-20ms-{}", numbers.join("-")));
        fs::create_dir_all(&dir).unwrap();
        let mut lines = vec![
            "# lazymkv audio-levels v2 (experiment 13)".to_string(),
            "file=x.mkv bytes=1 seconds=1 frame_ms=20 band_hz=300-3400".to_string(),
            format!("main=1 codec=dts profile=- channels=6 layout=5.1(side) language=eng fc=yes frames=100 md5={main_md5}"),
        ];
        for (n, md5) in cands {
            lines.push(format!("cand={n} codec=ac3 profile=- channels=2 layout=stereo language=eng frames=100 md5={md5}"));
        }
        fs::write(dir.join("manifest.txt"), lines.join("\n") + "\n").unwrap();
        let top = cands.iter().map(|(n, _)| *n).max().unwrap_or(1);
        let mut streams = vec![json!({"index": 0, "codec_type": "video"})];
        for k in 1..=top {
            let (codec, channels) = if k == 1 { ("dts", 6) } else { ("ac3", 2) };
            let name = format!("t{k}");
            streams.push(json!({"index": k, "codec_type": "audio", "codec_name": codec, "channels": channels,
                                "tags": {"language": "eng", "title": name}}));
        }
        fs::write(dir.join("streams.json"), serde_json::to_string(&json!({"streams": streams})).unwrap()).unwrap();
        fs::write(slug_dir.join(label).join("rip-summary.json"), format!("{{\"ok\": true, \"title\": {title}}}")).unwrap();
    }
    let flags = [(1, 0), (2, 0), (3, 0), (4, 0), (5, 0), (6, 2304), (7, 0)];
    let mut scan = vec!["TINFO:0,9,0,\"0:07:00\"".to_string(), "SINFO:0,0,1,0,\"Video\"".to_string()];
    scan.extend(flags.iter().map(|(s, _)| format!("SINFO:0,{s},1,0,\"Audio\"")));
    scan.extend(flags.iter().map(|(s, f)| format!("SINFO:0,{s},22,0,\"{f}\"")));
    scan.push("SINFO:1,0,1,0,\"Video\"".to_string());
    scan.extend((1..=3).map(|s| format!("SINFO:1,{s},1,0,\"Audio\"")));
    fs::write(slug_dir.join("scan.txt"), scan.join("\n") + "\n").unwrap();
    write_labels_file(slug_dir, vec![with(entry("t0", 3, "duplicate"), "note", json!("the owner's"))], json!({}));
}

mod spec {
    use super::*;

    #[test]
    fn test_the_repositorys_spec_names_the_office_disc() {
        let spec = repo_root(Path::new(env!("CARGO_MANIFEST_DIR")))
            .map(|root| root.join("docs/superpowers/specs/2026-10-04-lazymkv-design.md"))
            .filter(|path| path.is_file());
        let Some(spec) = spec else {
            skip("the repository's spec is not beside this folder");
            return;
        };
        let text = decode_text(&fs::read(&spec).unwrap(), Encoding::Utf8Sig, Errors::Strict).unwrap();
        let req = exp13_requirements(&text).unwrap();
        assert_eq!(req.fitting_slug, "the-office-2005-s3d3-bluray");
        assert_eq!(req.commentaries, 8);
    }
}

mod labels {
    use super::*;

    #[test]
    fn test_only_a_check_of_the_tracks_content_counts_d23() {
        // Spec section 17 row 13 and D23: the packaging, the menu, a flag and TheDiscDB are observations, recorded with
        // the label, never counted on their own; listening to the track is the one source that counts.
        assert_eq!(COUNTING_HOW, ["listened"]);
        let mut how: Vec<&str> = HOW.iter().map(|(name, _)| *name).collect();
        how.sort_unstable();
        let mut sources: Vec<&str> = COUNTING_HOW.iter().chain(OBSERVATION_HOW.iter()).copied().chain(["md5", "inferred"]).collect();
        sources.sort_unstable();
        assert_eq!(how, sources);
        let why = |how: &str, kind: &str| {
            let e = LabelEntry::from_value(&with(entry("t0", 3, kind), "how", json!(how))).expect("a valid entry");
            not_counted_because(&e, "disc-b", &Fitted::new())
        };
        assert_eq!(why("listened", "commentary"), "");
        for how in ["packaging", "menu", "flag", "thediscdb"] {
            assert!(OBSERVATION_HOW.contains(&how));
            assert_eq!(why(how, "commentary"), format!("'{how}' is an observation, not a check of the track's content (row 13, D23)"));
            assert!(why(how, "lossy-companion").contains("observation"));
            assert!(HOW.iter().any(|(name, meaning)| *name == how && meaning.contains("does not count")));
        }
        assert!(why("flag", "derived-core").contains("observation")); // what `auto` writes for a derived core
        let recorded = json!({"format": FORMAT, "slug": "disc-b", "titles": {}, "tracks": [with(entry("t0", 3, "commentary"), "how", json!("packaging"))]});
        assert_eq!(problems(&recorded, Some("disc-b")), Vec::<String>::new()); // still a valid label: recorded, not counted
        let tmp = scratch("labels");
        fs::create_dir(tmp.0.join("disc-b")).unwrap();
        let (code, out) = call_all(
            &tmp.0,
            &["--spec", "spec.md", "set", "disc-b", "t0", "3", "commentary", "--how", "packaging", "--saw-verdict", "no", "--date", "2026-10-12"],
        );
        assert_eq!(code, 0, "{out}");
        assert!(
            out.contains(concat!(
                "labelled t0 stream 3: commentary (packaging, 2026-10-12, saw verdict no, sure yes), counts=False ",
                "('packaging' is an observation, not a check of the track's content (row 13, D23))",
            )),
            "{out}"
        );
    }

    #[test]
    fn test_set_adds_replaces_keeps_history_and_refuses_a_new_problem() {
        let tmp = scratch("labels");
        fs::create_dir(tmp.0.join("disc-b")).unwrap();
        let set = |more: &[&str]| {
            let mut args = vec!["--spec", "spec.md", "set", "disc-b", "t0", "3"];
            args.extend_from_slice(more);
            call_all(&tmp.0, &args)
        };
        let (code, out) = set(&["commentary", "--how", "listened", "--saw-verdict", "no", "--film-audible", "yes", "--date", "2026-10-05"]);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("labelled t0 stream 3: commentary (listened, 2026-10-05, saw verdict no, sure yes), counts=True"), "{out}");
        let (_, out) = set(&["commentary", "--how", "listened", "--saw-verdict", "no", "--film-audible", "yes", "--date", "2026-10-07"]);
        assert!(out.contains("unchanged t0 stream 3"), "{out}");
        let (code, out) = set(&["partial-commentary", "--how", "listened", "--saw-verdict", "yes", "--date", "2026-10-06"]);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("counts=False (the owner saw the verdict first); replaces 1 earlier label(s)"), "{out}");
        let (_, out) = set(&["unknown", "--how", "listened", "--saw-verdict", "no", "--sure", "no", "--date", "2026-10-07"]);
        assert!(out.contains("counts=False (a unknown track never counts)"), "{out}");
        let title = ["--spec", "spec.md", "title", "disc-b", "t0", "--title-kind", "feature", "--dialogue-heavy", "no"];
        assert_eq!(call_all(&tmp.0, &title).0, 0);
        let file = tmp.0.join("disc-b").join("audio-labels.json");
        let data = read_json(&file);
        let tracks = data["tracks"].as_array().unwrap();
        assert_eq!(tracks.len(), 1); // replaced, not added twice
        let t = &tracks[0];
        assert_eq!((t["kind"].as_str(), t["sure"].as_bool()), (Some("unknown"), Some(false)));
        let history: Vec<(&str, bool, &str)> = t["history"]
            .as_array()
            .unwrap()
            .iter()
            .map(|h| (h["kind"].as_str().unwrap(), h["saw_verdict"].as_bool().unwrap(), h["replaced"].as_str().unwrap()))
            .collect();
        assert_eq!(history, [("commentary", false, "2026-10-06"), ("partial-commentary", true, "2026-10-07")]);
        assert_eq!(t["history"][0]["film_audible"], json!(true)); // the blind label survives, with everything it said
        assert_eq!(data["titles"]["t0"], json!({"title_kind": "feature", "dialogue_heavy": false, "note": ""}));
        let before = fs::read(&file).unwrap();
        let (code, out) = call_all(
            &tmp.0,
            &["--spec", "spec.md", "set", "disc-b", "t0", "2", "lossy-companion", "--how", "menu", "--saw-verdict", "no", "--film-audible", "yes"],
        );
        assert_eq!(code, 2);
        assert!(out.contains("film_audible is only for"), "{out}");
        assert_eq!(fs::read(&file).unwrap(), before);
    }

    #[test]
    fn test_unset_keeps_the_history_of_a_label_that_was_replaced() {
        let tmp = scratch("labels");
        fs::create_dir(tmp.0.join("disc-b")).unwrap();
        let set = |more: &[&str]| {
            let mut args = vec!["--spec", "spec.md", "set", "disc-b", "t0", "3"];
            args.extend_from_slice(more);
            call_all(&tmp.0, &args)
        };
        assert_eq!(set(&["commentary", "--how", "listened", "--saw-verdict", "no", "--sure", "no"]).0, 0);
        assert_eq!(set(&["commentary", "--how", "listened", "--saw-verdict", "no"]).0, 0);
        let (code, out) = call_all(&tmp.0, &["--spec", "spec.md", "unset", "disc-b", "t0", "3", "--note", "wrong stream"]);
        assert_eq!(code, 0, "{out}");
        assert_eq!(call_all(&tmp.0, &["--spec", "spec.md", "check", "."]).0, 0);
        let data = read_json(&tmp.0.join("disc-b").join("audio-labels.json"));
        assert_eq!(data["tracks"], json!([]));
        assert_eq!(data["removed"][0]["history"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_a_broken_file_is_repaired_with_the_tool_one_entry_at_a_time() {
        let tmp = scratch("labels");
        let tracks = vec![entry("t0", 3, "comentary"), entry("t0", 9, "commentary"), with(entry("t0", 4, "commentary"), "how", json!("told"))];
        write_labels_file(&tmp.0.join("disc-b"), tracks, json!({}));
        let run_labels = |args: &[&str]| {
            let mut all = vec!["--spec", "spec.md"];
            all.extend_from_slice(args);
            call_all(&tmp.0, &all)
        };
        assert_eq!(run_labels(&["check", "."]).0, 2);
        let (code, out) = run_labels(&["set", "disc-b", "t0", "3", "commentary", "--how", "listened", "--saw-verdict", "no"]);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("warning: the file still has 1 problem(s): track t0 stream 4: how 'told'"), "{out}");
        let (code, out) = run_labels(&["unset", "disc-b", "t0", "4", "--note", "wrong source"]);
        assert_eq!(code, 0, "{out}");
        let (code, out) = run_labels(&["unset", "disc-b", "t0", "9", "--note", "typo: stream 3"]);
        assert_eq!(code, 0, "{out}");
        assert_eq!(run_labels(&["unset", "disc-b", "t0", "9", "--note", "again"]).0, 2);
        let (code, out) = run_labels(&["check", "."]);
        assert_eq!(code, 0, "{out}");
        let data = read_json(&tmp.0.join("disc-b").join("audio-labels.json"));
        let streams: Vec<i64> = data["tracks"].as_array().unwrap().iter().map(|t| t["stream"].as_i64().unwrap()).collect();
        assert_eq!(streams, [3]);
        assert_eq!(data["tracks"][0]["history"][0]["kind"], json!("comentary"));
        let removed: Vec<(i64, &str)> =
            data["removed"].as_array().unwrap().iter().map(|r| (r["stream"].as_i64().unwrap(), r["why"].as_str().unwrap())).collect();
        assert_eq!(removed, [(4, "wrong source"), (9, "typo: stream 3")]);
    }

    #[test]
    fn test_fitted_records_a_disc_or_a_track() {
        let tmp = scratch("labels");
        let (code, out) = call_all(&tmp.0, &["--spec", "spec.md", "fitted", ".", "disc-g", "--rule", "abc123", "--note", "DC_MAX re-fit"]);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("fitted: disc-g (the whole disc): no longer counts"), "{out}");
        assert_eq!(call_all(&tmp.0, &["--spec", "spec.md", "fitted", ".", "disc-h", "t0", "2", "--rule", "abc123"]).0, 0);
        assert_eq!(call_all(&tmp.0, &["--spec", "spec.md", "fitted", ".", "disc-h", "t0", "--rule", "x"]).0, 2);
        let tree = DiskTree::open(&tmp.0, ".").unwrap();
        let rows: [(&str, Option<&str>, Option<i64>); 3] = [("fit-disc", None, None), ("disc-g", None, None), ("disc-h", Some("t0"), Some(2))];
        let expected: Fitted = rows.iter().map(|(slug, label, stream)| (slug.to_string(), label.map(str::to_string), *stream)).collect();
        assert_eq!(load_fitted(&tree, &PathKey::root(), Some("fit-disc")).unwrap(), expected);
    }
}

mod command {
    use super::*;

    #[test]
    fn a_note_in_another_script_and_an_unknown_key_survive_an_edit() {
        // Review Focus 4: `labels check` reports the Python's sentence, an edit keeps the unknown key and the note's
        // characters, and the file is written with ensure_ascii off (the characters, never \u escapes).
        let tmp = scratch("labels");
        fs::create_dir(tmp.0.join("disc-b")).unwrap();
        let note = "caf\u{e9} na\u{ef}ve \u{2014} stra\u{df}e";
        let commentary = with(with(entry("t0", 3, "commentary"), "note", json!(note)), "film_audible", Value::Null);
        let data = json!({"format": FORMAT, "slug": "disc-b", "titles": {}, "tracks": [commentary], "extra": 1});
        let file = tmp.0.join("disc-b").join("audio-labels.json");
        fs::write(&file, serde_json::to_string(&data).unwrap()).unwrap();
        let problem = "disc-b/audio-labels.json: unknown top-level keys ['extra']\n".to_string();
        let check = ["--spec", "spec.md", "check", "."];
        assert_eq!(call(&tmp.0, &check), (2, problem.clone(), String::new()));
        let set = ["--spec", "spec.md", "set", "disc-b", "t0", "2", "lossy-companion", "--how", "listened", "--saw-verdict", "no", "--date", "2026-10-06"];
        let (code, out, err) = call(&tmp.0, &set);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(
            out,
            concat!(
                "labelled t0 stream 2: lossy-companion (listened, 2026-10-06, saw verdict no, sure yes), counts=True\n",
                "warning: the file still has 1 problem(s): unknown top-level keys ['extra']\n",
            )
        );
        let written = fs::read_to_string(&file).unwrap();
        assert_eq!(written, SET_FILE);
        assert!(written.contains(note) && !written.contains("\\u00e9"), "{written}");
        assert_eq!(call(&tmp.0, &check), (2, problem, String::new()));
        let title = ["--spec", "spec.md", "title", "disc-b", "t0", "--title-kind", "feature", "--dialogue-heavy", "unknown", "--note", "\u{e9}t\u{e9}"];
        let (code, out, _) = call(&tmp.0, &title);
        assert_eq!(
            (code, out.as_str()),
            (0, "title t0: feature, dialogue_heavy=None\nwarning: the file still has 1 problem(s): unknown top-level keys ['extra']\n")
        );
        assert_eq!(fs::read_to_string(&file).unwrap(), TITLE_FILE);
    }

    #[test]
    fn auto_labels_and_writes_the_file_once() {
        let tmp = scratch("labels");
        let slug_dir = tmp.0.join("disc-d");
        write_disc_d(&slug_dir);
        let args = ["--spec", "spec.md", "auto", "disc-d", "--date", "2026-10-05"];
        let t1_line = "t1: derived cores not labelled: the file has 2 audio streams and the scan's title 1 has 3\n";
        let (code, out, err) = call(&tmp.0, &args);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(
            out,
            [
                "labelled t0 stream 5: duplicate (md5), counts=False (a duplicate track never counts)\n",
                "labelled t0 stream 6: derived-core (flag), counts=False ('flag' is an observation, not a check of the track's content (row 13, D23))\n",
                "labelled t0 stream 7: duplicate (md5), counts=False (a duplicate track never counts)\n",
                t1_line,
            ]
            .concat()
        );
        let file = slug_dir.join("audio-labels.json");
        assert_eq!(fs::read_to_string(&file).unwrap(), AUTO_FILE);
        assert_eq!(call(&tmp.0, &args), (0, t1_line.to_string(), String::new()));
        assert_eq!(fs::read_to_string(&file).unwrap(), AUTO_FILE);
    }

    #[test]
    fn a_fitted_file_that_does_not_validate_stops_auto_before_it_writes() {
        let tmp = scratch("labels");
        let slug_dir = tmp.0.join("disc-d");
        write_disc_d(&slug_dir);
        fs::write(tmp.0.join("audio-fitted.json"), "{}").unwrap();
        let before = fs::read(slug_dir.join("audio-labels.json")).unwrap();
        let (code, out, err) = call(&tmp.0, &["--spec", "spec.md", "auto", "disc-d", "--date", "2026-10-05"]);
        assert_eq!((code, out.as_str()), (2, ""));
        assert!(err.ends_with("audio-fitted.json: format is None, not 'lazymkv audio fitted v1'; fitted is not a list\n"), "{err}");
        assert_eq!(fs::read(slug_dir.join("audio-labels.json")).unwrap(), before);
    }

    #[test]
    fn kinds_prints_the_vocabulary_and_help_names_the_rule_version() {
        let tmp = scratch("labels");
        let (code, out, err) = call(&tmp.0, &["kinds"]);
        assert_eq!((code, err.as_str()), (0, ""));
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 19);
        assert_eq!(lines[0], "main                 programme  keep     the programme's main soundtrack; as a reference it is never judged");
        assert!(lines[18].starts_with("how: listened = a person listened to it"), "{out}");
        let (code, _, err) = call(&tmp.0, &["kinds", "extra"]);
        assert_eq!(code, 2);
        assert!(err.lines().any(|line| line == "lazymkv-dev labels: error: unrecognized arguments: extra"), "{err}");
        let (code, out, _) = call(&tmp.0, &["fitted", "--help"]);
        assert_eq!(code, 0);
        assert!(out.contains(&format!("version {RULE_VERSION}")), "{out}");
    }

    #[test]
    fn argument_errors_are_argparses_with_exit_2() {
        let tmp = scratch("labels");
        fs::create_dir(tmp.0.join("disc-b")).unwrap();
        let kinds = concat!(
            "'main', 'lossy-companion', 'alternate-mix', 'laugh-track-version', 'dub', 'derived-core', 'duplicate', ",
            "'commentary', 'partial-commentary', 'audio-description', 'isolated-score', 'music-effects', 'same-language-dub', ",
            "'alternate-narration', 'foreign-dub', 'other-speech', 'sing-along', 'unknown'",
        );
        let cases: Vec<(Vec<&str>, String)> = vec![
            (
                vec!["--spec", "spec.md", "set", "disc-b", "t0", "x", "commentary", "--how", "listened", "--saw-verdict", "no"],
                "lazymkv-dev labels set: error: argument stream: invalid int value: 'x'".to_string(),
            ),
            (
                vec!["--spec", "spec.md", "set", "disc-b", "t0", "3", "comentary", "--how", "listened", "--saw-verdict", "no"],
                format!("lazymkv-dev labels set: error: argument kind: invalid choice: 'comentary' (choose from {kinds})"),
            ),
            (
                vec!["--spec", "spec.md", "set", "disc-b", "t0", "3", "commentary", "--how", "listened", "--saw-verdict", "maybe"],
                "lazymkv-dev labels set: error: argument --saw-verdict: 'maybe': say yes or no".to_string(),
            ),
            (
                vec!["--spec", "spec.md", "set", "disc-b", "t0"],
                "lazymkv-dev labels set: error: the following arguments are required: stream, kind, --how, --saw-verdict".to_string(),
            ),
            (
                vec!["--spec", "spec.md", "title", "disc-b", "t0", "--title-kind", "film"],
                "lazymkv-dev labels title: error: argument --title-kind: invalid choice: 'film' (choose from 'feature', 'episode', 'extra', 'other')"
                    .to_string(),
            ),
            (
                vec!["--spec", "spec.md", "frob"],
                "lazymkv-dev labels: error: argument cmd: invalid choice: 'frob' (choose from 'kinds', 'check', 'set', 'unset', 'title', 'auto', 'fitted')"
                    .to_string(),
            ),
            (vec!["--spec", "spec.md"], "lazymkv-dev labels: error: the following arguments are required: cmd".to_string()),
            (vec!["--spec", "spec.md", "check"], "lazymkv-dev labels check: error: the following arguments are required: paths".to_string()),
        ];
        for (args, line) in cases {
            let (code, out, err) = call(&tmp.0, &args);
            assert_eq!((code, out.as_str()), (2, ""), "{args:?}");
            assert!(err.starts_with("usage: lazymkv-dev labels"), "{args:?}: {err}");
            assert!(err.lines().any(|l| l == line), "{args:?}: {err}");
        }
    }

    #[test]
    fn the_pythons_error_sentences_with_exit_2() {
        let tmp = scratch("labels");
        write_labels_file(&tmp.0.join("disc-b"), vec![entry("t0", 2, "commentary")], json!({}));
        let cases: Vec<(Vec<&str>, &str)> = vec![
            (vec!["--spec", "spec.md", "set", "nope", "t0", "3", "commentary", "--how", "listened", "--saw-verdict", "no"], "nope: no such recording folder"),
            (vec!["--spec", "spec.md", "auto", "./nope/"], "nope: no such recording folder"),
            (vec!["--spec", "spec.md", "unset", "disc-b", "t0", "7", "--note", "x"], "disc-b/audio-labels.json: t0 stream 7 has no label"),
            (vec!["--spec", "spec.md", "unset", "disc-b", "t0", "2", "--note", ""], "unset needs --note: why the label is taken away"),
            (vec!["--spec", "spec.md", "fitted", "nope", "disc-g", "--rule", "x"], "nope: no such folder"),
            (vec!["--spec", "spec.md", "fitted", ".", "disc-h", "t0", "--rule", "x"], "fitted: give both <label> and <stream>, or neither (the whole disc)"),
        ];
        for (args, line) in cases {
            assert_eq!(call(&tmp.0, &args), (2, String::new(), format!("{line}\n")), "{args:?}");
        }
        let (code, out, err) = call(&tmp.0, &["--spec", "nospec.md", "check", "."]);
        assert_eq!((code, out.as_str()), (2, ""));
        assert!(err.starts_with("cannot read the fitting disc from nospec.md: "), "{err}");
        if cfg!(unix) {
            assert_eq!(err, "cannot read the fitting disc from nospec.md: [Errno 2] No such file or directory: 'nospec.md'\n");
        }
        assert_eq!(call(&tmp.0, &["--spec", "spec.md", "check", "nope"]), (0, "nope: no audio-labels.json\n".to_string(), String::new()));
    }

    #[test]
    fn check_validates_every_labels_file_of_the_evidence_clone() {
        let repo = repo_root(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("the workspace root");
        if !repo.join("corpus/owner").is_dir() {
            skip("the evidence repository is not cloned at corpus/owner");
            return;
        }
        let (code, out, err) = call(&repo, &["check", "corpus/owner"]);
        assert_eq!((code, err.as_str()), (0, ""), "{out}");
        assert!(out.lines().count() >= 1 && out.lines().all(|line| line.starts_with("ok corpus/owner/")), "{out}");
    }
}

/// The labels file after `set disc-b t0 2 lossy-companion ...` in a_note_in_another_script_..., as the Python wrote it.
const SET_FILE: &str = concat!(
    "{\n",
    "  \"format\": \"lazymkv audio labels v1\",\n",
    "  \"slug\": \"disc-b\",\n",
    "  \"titles\": {},\n",
    "  \"tracks\": [\n",
    "    {\n",
    "      \"label\": \"t0\",\n",
    "      \"stream\": 2,\n",
    "      \"kind\": \"lossy-companion\",\n",
    "      \"how\": \"listened\",\n",
    "      \"sure\": true,\n",
    "      \"date\": \"2026-10-06\",\n",
    "      \"saw_verdict\": false,\n",
    "      \"note\": \"\"\n",
    "    },\n",
    "    {\n",
    "      \"label\": \"t0\",\n",
    "      \"stream\": 3,\n",
    "      \"kind\": \"commentary\",\n",
    "      \"how\": \"listened\",\n",
    "      \"sure\": true,\n",
    "      \"date\": \"2026-10-05\",\n",
    "      \"saw_verdict\": false,\n",
    "      \"note\": \"caf\u{e9} na\u{ef}ve \u{2014} stra\u{df}e\",\n",
    "      \"film_audible\": null\n",
    "    }\n",
    "  ],\n",
    "  \"extra\": 1\n",
    "}\n",
);

/// The same file after the `title disc-b t0 --title-kind feature --dialogue-heavy unknown --note ...` of that test.
const TITLE_FILE: &str = concat!(
    "{\n",
    "  \"format\": \"lazymkv audio labels v1\",\n",
    "  \"slug\": \"disc-b\",\n",
    "  \"titles\": {\n",
    "    \"t0\": {\n",
    "      \"title_kind\": \"feature\",\n",
    "      \"dialogue_heavy\": null,\n",
    "      \"note\": \"\u{e9}t\u{e9}\"\n",
    "    }\n",
    "  },\n",
    "  \"tracks\": [\n",
    "    {\n",
    "      \"label\": \"t0\",\n",
    "      \"stream\": 2,\n",
    "      \"kind\": \"lossy-companion\",\n",
    "      \"how\": \"listened\",\n",
    "      \"sure\": true,\n",
    "      \"date\": \"2026-10-06\",\n",
    "      \"saw_verdict\": false,\n",
    "      \"note\": \"\"\n",
    "    },\n",
    "    {\n",
    "      \"label\": \"t0\",\n",
    "      \"stream\": 3,\n",
    "      \"kind\": \"commentary\",\n",
    "      \"how\": \"listened\",\n",
    "      \"sure\": true,\n",
    "      \"date\": \"2026-10-05\",\n",
    "      \"saw_verdict\": false,\n",
    "      \"note\": \"caf\u{e9} na\u{ef}ve \u{2014} stra\u{df}e\",\n",
    "      \"film_audible\": null\n",
    "    }\n",
    "  ],\n",
    "  \"extra\": 1\n",
    "}\n",
);

/// disc-d's labels file after `auto disc-d --date 2026-10-05`, as the Python wrote it.
const AUTO_FILE: &str = concat!(
    "{\n",
    "  \"format\": \"lazymkv audio labels v1\",\n",
    "  \"slug\": \"disc-d\",\n",
    "  \"titles\": {},\n",
    "  \"tracks\": [\n",
    "    {\n",
    "      \"label\": \"t0\",\n",
    "      \"stream\": 3,\n",
    "      \"kind\": \"duplicate\",\n",
    "      \"how\": \"listened\",\n",
    "      \"sure\": true,\n",
    "      \"date\": \"2026-10-05\",\n",
    "      \"saw_verdict\": false,\n",
    "      \"note\": \"the owner's\"\n",
    "    },\n",
    "    {\n",
    "      \"label\": \"t0\",\n",
    "      \"stream\": 5,\n",
    "      \"kind\": \"duplicate\",\n",
    "      \"how\": \"md5\",\n",
    "      \"sure\": true,\n",
    "      \"date\": \"2026-10-05\",\n",
    "      \"saw_verdict\": false,\n",
    "      \"note\": \"auto: same packets as stream 2 (bb)\"\n",
    "    },\n",
    "    {\n",
    "      \"label\": \"t0\",\n",
    "      \"stream\": 6,\n",
    "      \"kind\": \"derived-core\",\n",
    "      \"how\": \"flag\",\n",
    "      \"sure\": true,\n",
    "      \"date\": \"2026-10-05\",\n",
    "      \"saw_verdict\": false,\n",
    "      \"note\": \"auto: scan flags 2304 (2048: derived by MakeMKV)\"\n",
    "    },\n",
    "    {\n",
    "      \"label\": \"t0\",\n",
    "      \"stream\": 7,\n",
    "      \"kind\": \"duplicate\",\n",
    "      \"how\": \"md5\",\n",
    "      \"sure\": true,\n",
    "      \"date\": \"2026-10-05\",\n",
    "      \"saw_verdict\": false,\n",
    "      \"note\": \"auto: same packets as stream 1 (aa)\"\n",
    "    }\n",
    "  ]\n",
    "}\n",
);
