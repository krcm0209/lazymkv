//! Tests of `core::audio::labels`. `spec` and `labels` port six tests of test_audio_dataset.py's Spec and Labels
//! classes one for one, under their Python names (the other six run the command or read the repository's spec: they
//! are in crates/dev/src/cmd/labels/tests.rs). `validation`, `edits` and `walkers` are new; every expected sentence,
//! line and file in them was printed by the Python of commit 75fa76a from the same input, except the script's name in
//! two sentences: "(audio_labels.py kinds)" is "(lazymkv-dev labels kinds)" and "update audio_labels.py" is
//! "update core::audio::labels" (port spec 3.4 and 6: a line that names a deleted script names the Rust).

use super::*;
use crate::recording::MemTree;
use serde_json::{json, Value};

/// The numbers row 13 of the test spec states. Py: the keyword arguments of test_audio_dataset.spec.
#[derive(Clone, Copy)]
struct Numbers {
    verdict_discs: i64,
    comm: i64,
    discs: i64,
    features: i64,
    prog: i64,
    sep: i64,
    centre: i64,
    heavy: i64,
}

impl Default for Numbers {
    fn default() -> Numbers {
        Numbers { verdict_discs: 3, comm: 8, discs: 4, features: 2, prog: 20, sep: 6, centre: 1, heavy: 1 }
    }
}

/// A spec holding only section 17's table head and row 13. Py: test_audio_dataset.spec (ROW13).
fn spec_text(n: Numbers) -> String {
    format!(
        concat!(
            "# spec\n\n## 17. Experiments (phase 0)\n\n| # | Question | If no |\n|---|---|---|\n",
            "| 13 | Measured over whole titles on the owner's own discs, does the content rule give its commentary verdict to ",
            "**no** programme track, and to at least one commentary on each of at least {verdict_discs} different discs? Before ",
            "the answer may be yes, the labelled tracks must include: at least {comm} commentaries from at least {discs} discs, ",
            "at least {features} of them on feature films and at least 1 with the film clearly audible under the voices; at ",
            "least 1 audio-description track if the owner has a disc with one (all that the owner has, up to 3); at least ",
            "{prog} programme tracks other than the reference, at least {sep} of them separately encoded lossy companions or ",
            "alternate mixes; at least {centre} title whose reference has no centre channel (mono or 2.0) and at least ",
            "{heavy} dialogue-heavy or speech-only title, each with an extra track. Every threshold was fitted on two tracks of ",
            "one disc (`fit-disc`: the S03E16 commentary), so no track of that disc counts. | fallback |\n",
        ),
        verdict_discs = n.verdict_discs,
        comm = n.comm,
        discs = n.discs,
        features = n.features,
        prog = n.prog,
        sep = n.sep,
        centre = n.centre,
        heavy = n.heavy,
    )
}

/// Py: SPEC.
fn spec_default() -> String {
    spec_text(Numbers::default())
}

/// Row 13 with the optional dubs phrase. Py: DUB_SPEC.
fn dub_spec() -> String {
    spec_default().replace(
        "each with an extra track.",
        "each with an extra track; at least 3 dubs in a wanted language from at least 2 discs, at least 1 of them on a dialogue-heavy title.",
    )
}

/// A track label: how "listened", sure, the verdict not seen, dated 2026-10-05. Py: test_audio_dataset.entry.
fn entry(label: &str, stream: i64, kind: &str) -> Value {
    json!({"label": label, "stream": stream, "kind": kind, "how": "listened", "sure": true, "date": "2026-10-05",
           "saw_verdict": false, "note": ""})
}

/// `dict(value, key=v)`: the key's value replaced in place, or the key added at the end.
fn with(mut value: Value, key: &str, v: Value) -> Value {
    value.as_object_mut().expect("an object").insert(key.to_string(), v);
    value
}

/// The object without `key`, the order of the other keys kept.
fn without(value: &Value, key: &str) -> Value {
    Value::Object(
        value.as_object().expect("an object").iter().filter(|(k, _)| k.as_str() != key).map(|(k, v)| (k.clone(), v.clone())).collect(),
    )
}

/// A title's facts. Py: test_audio_dataset.title.
fn title(kind: &str, heavy: Value) -> Value {
    json!({"title_kind": kind, "dialogue_heavy": heavy, "note": ""})
}

/// The problems of a labels file for disc-b holding `tracks` and `titles`, plus `more` top-level keys. Py: Labels.check.
fn check(tracks: Vec<Value>, titles: Value, more: &[(&str, Value)]) -> Vec<String> {
    let mut data = json!({"format": FORMAT, "slug": "disc-b", "titles": titles, "tracks": tracks});
    for (key, value) in more {
        data = with(data, key, value.clone());
    }
    problems(&data, Some("disc-b"))
}

/// The typed view of a valid entry.
fn typed(value: &Value) -> LabelEntry {
    LabelEntry::from_value(value).expect("a valid entry")
}

/// Owned strings, to compare with the functions' Vec<String>.
fn strings(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|line| line.to_string()).collect()
}

/// A value as compact JSON in its key order (serde_json's `==` on objects ignores the order of their keys).
fn text(value: &Value) -> String {
    serde_json::to_string(value).expect("serialisable")
}

/// An evidence root printed as corpus/owner.
fn tree() -> MemTree {
    MemTree::new("corpus/owner", "owner")
}

/// Writes a JSON file into the tree.
fn put(tree: &mut MemTree, path: &str, value: &Value) {
    tree.insert_file(path, serde_json::to_vec(value).expect("serialisable"));
}

/// A fitted set from (slug, label, stream) rows.
fn fitted_set(rows: &[(&str, Option<&str>, Option<i64>)]) -> Fitted {
    rows.iter().map(|(slug, label, stream)| (slug.to_string(), label.map(str::to_string), *stream)).collect()
}

/// An entry of audio-fitted.json.
fn fitted_entry(slug: &str, label: Option<&str>, stream: Option<i64>) -> Value {
    json!({"slug": slug, "label": label, "stream": stream, "date": "2026-10-05", "rule": "test", "note": ""})
}

mod spec {
    use super::*;

    #[test]
    fn test_row_13_gives_the_totals_and_the_fitting_disc() {
        let req = exp13_requirements(&spec_default()).unwrap();
        assert_eq!((req.commentaries, req.commentary_discs, req.commentary_features, req.commentary_audible), (8, 4, 2, 1));
        assert_eq!((req.ad_min, req.ad_max, req.programme, req.programme_separate), (1, 3, 20, 6));
        assert_eq!((req.no_centre_titles, req.dialogue_heavy_titles, req.verdict_discs), (1, 1, 3));
        assert_eq!(req.fitting_slug, "fit-disc");
        let decoy = spec_default().replace("**no** programme track", "**no** programme track (`decoy-disc`)");
        assert_eq!(exp13_requirements(&decoy).unwrap().fitting_slug, "fit-disc");
    }

    #[test]
    fn test_row_13_may_set_the_dubs_a_cross_language_verdict_needs() {
        let req = exp13_requirements(&spec_default()).unwrap();
        assert_eq!([req.dubs, req.dub_discs, req.dub_heavy], [None, None, None]);
        let req = exp13_requirements(&dub_spec()).unwrap();
        assert_eq!([req.dubs, req.dub_discs, req.dub_heavy], [Some(3), Some(2), Some(1)]);
    }

    #[test]
    fn test_a_reworded_row_is_refused_by_name() {
        let stop = exp13_requirements(&spec_default().replace("commentaries from at least", "commentaries over")).unwrap_err();
        assert!(stop.contains("no longer contains the phrase"), "{stop}");
        assert!(exp13_requirements(&spec_default().replace("| 13 |", "| 14 |")).is_err());
    }
}

mod labels {
    use super::*;

    #[test]
    fn test_a_good_file_has_no_problem() {
        let old = with(entry("t0", 3, "lossy-companion"), "replaced", json!("2026-10-06"));
        let commentary = with(with(entry("t0", 3, "commentary"), "film_audible", json!(false)), "history", json!([old]));
        let duplicate = with(entry("t0", 2, "duplicate"), "how", json!("md5"));
        let removed = with(with(entry("t0", 9, "commentary"), "removed", json!("2026-10-06")), "why", json!("typo"));
        let found = check(vec![commentary, duplicate], json!({"t0": title("feature", Value::Null)}), &[("removed", json!([removed]))]);
        assert_eq!(found, Vec::<String>::new());
    }

    #[test]
    fn test_each_rule_is_enforced() {
        let cases: Vec<(Value, &str)> = vec![
            (entry("t0", 3, "comentary"), "not in the vocabulary"),
            (with(entry("t0", 3, "commentary"), "how", json!("told")), "how 'told' is not one of"),
            (with(entry("t0", 3, "commentary"), "how", json!("md5")), "how md5 only says"),
            (with(entry("t0", 3, "lossy-companion"), "film_audible", json!(true)), "film_audible is only for"),
            (with(entry("t0", 3, "commentary"), "film_audible", json!("yes")), "film_audible must be"),
            (with(entry("t0", 3, "commentary"), "sure", json!("yes")), "sure must be true or false"),
            (with(entry("t0", 3, "commentary"), "saw_verdict", Value::Null), "saw_verdict must be true or false"),
            (with(entry("t0", 3, "commentary"), "date", json!("5 Oct")), "is not YYYY-MM-DD"),
            (with(entry("t0", 3, "commentary"), "colour", json!("red")), "unknown keys"),
            (with(entry("t0", 3, "commentary"), "counts", json!(true)), "unknown keys"),
            (with(entry("t0", 3, "commentary"), "stream", json!("3")), "stream is not"),
            (without(&entry("t0", 3, "commentary"), "sure"), "missing ['sure']"),
            (with(entry("t0", 3, "commentary"), "history", json!([entry("t0", 3, "unknown")])), "replaced None is not YYYY-MM-DD"),
        ];
        for (track, words) in cases {
            let found = check(vec![track.clone()], json!({}), &[]);
            assert!(found.iter().any(|p| p.contains(words)), "{track} {found:?}");
        }
        let twice = check(vec![entry("t0", 3, "commentary"), entry("t0", 3, "commentary")], json!({}), &[]);
        assert!(twice.join(" ").contains("labelled twice"), "{twice:?}");
        let film = check(vec![], json!({"t0": {"title_kind": "film"}}), &[]);
        assert!(film.join(" ").contains("title_kind"), "{film:?}");
        let heavy = check(vec![], json!({"t0": with(title("episode", json!(false)), "dialogue_heavy", json!("yes"))}), &[]);
        assert!(heavy.join(" ").contains("dialogue_heavy must be"), "{heavy:?}");
        let extra = check(vec![], json!({}), &[("extra", json!(1))]);
        assert!(extra.join(" ").contains("unknown top-level keys ['extra']"), "{extra:?}");
        let removed = check(vec![], json!({}), &[("removed", json!([entry("t0", 9, "commentary")]))]);
        assert!(removed.join(" ").contains("removed 0: removed"), "{removed:?}");
        let moved = problems(&json!({"format": FORMAT, "slug": "disc-x", "titles": {}, "tracks": []}), Some("disc-b"));
        assert!(moved.join(" ").contains("is in corpus/owner/disc-b"), "{moved:?}");
    }

    #[test]
    fn test_whether_a_label_counts_is_worked_out_from_row_13_and_the_fitted_list() {
        let fitted = fitted_set(&[("fit-disc", None, None), ("disc-h", Some("t0"), Some(2))]);
        let why = |slug: &str, e: Value| not_counted_because(&typed(&e), slug, &fitted);
        assert_eq!(why("disc-b", entry("t0", 3, "commentary")), "");
        assert_eq!(why("disc-b", entry("t0", 3, "derived-core")), "");
        assert!(why("disc-b", entry("t0", 3, "main")).contains("never counts"));
        assert!(why("disc-b", with(entry("t0", 3, "duplicate"), "how", json!("md5"))).contains("never counts"));
        assert!(why("disc-b", entry("t0", 3, "unknown")).contains("never counts"));
        assert!(why("disc-b", with(entry("t0", 3, "commentary"), "how", json!("inferred"))).contains("is not a check of the track's content"));
        assert!(why("disc-b", with(entry("t0", 3, "commentary"), "saw_verdict", json!(true))).contains("saw the verdict"));
        assert!(why("disc-b", with(entry("t0", 3, "commentary"), "sure", json!(false))).contains("not sure"));
        assert!(why("fit-disc", entry("t0", 3, "commentary")).contains("fitted on it"));
        assert!(why("disc-h", entry("t0", 2, "commentary")).contains("fitted on it"));
        assert_eq!(why("disc-h", entry("t0", 3, "commentary")), "");
    }
}

mod validation {
    use super::*;

    #[test]
    fn the_reworded_row_names_the_phrase_verbatim() {
        let stop = exp13_requirements(&spec_default().replace("commentaries from at least", "commentaries over")).unwrap_err();
        assert_eq!(
            stop,
            concat!(
                "spec section 17 row 13 no longer contains the phrase /at least (\\d+|one|two|three|four|five|six|seven|eight|nine|ten) ",
                "commentaries from at least (\\d+|one|two|three|four|five|six|seven|eight|nine|ten) discs, at least ",
                "(\\d+|one|two|three|four|five|six|seven|eight|nine|ten) of them on feature films and at least ",
                "(\\d+|one|two|three|four|five|six|seven|eight|nine|ten) with the film clearly audible/; update core::audio::labels",
            )
        );
        assert_eq!(
            exp13_requirements(&spec_default().replace("| 13 |", "| 14 |")).unwrap_err(),
            "spec section 17 has no row starting '| 13 |'"
        );
        assert_eq!(
            exp13_requirements(&spec_default().replace("Every threshold was fitted on", "The thresholds came from")).unwrap_err(),
            "spec section 17 row 13 no longer names the disc the thresholds were fitted on"
        );
    }

    #[test]
    fn a_number_in_words_counts_in_row_13() {
        let words = spec_default().replace("at least 8 commentaries", "at least eight commentaries");
        let req = exp13_requirements(&words).unwrap();
        assert_eq!(req.commentaries, 8);
        let serialised = serde_json::to_string(&exp13_requirements(&spec_default()).unwrap()).unwrap();
        assert_eq!(
            serialised,
            concat!(
                r#"{"commentaries":8,"commentary_discs":4,"commentary_features":2,"commentary_audible":1,"ad_min":1,"ad_max":3,"#,
                r#""programme":20,"programme_separate":6,"no_centre_titles":1,"dialogue_heavy_titles":1,"verdict_discs":3,"#,
                r#""fitting_slug":"fit-disc","dubs":null,"dub_discs":null,"dub_heavy":null}"#,
            )
        );
    }

    #[test]
    fn date_ok_wants_a_real_iso_date() {
        let cases: Vec<(Value, bool)> = vec![
            (json!("2026-10-05"), true),
            (json!("2026-02-29"), false),
            (json!("2024-02-29"), true),
            (json!("2026-13-01"), false),
            (json!("0000-01-01"), false),
            (json!("2026-1-05"), false),
            (json!(20261005), false),
            (Value::Null, false),
            (json!("2026-10-05\n"), false),
            (json!("\u{ff12}\u{ff10}\u{ff12}\u{ff16}-10-05"), false),
            (json!("1900-02-29"), false),
            (json!("2000-02-29"), true),
            (json!("9999-12-31"), true),
            (json!("2026-04-31"), false),
        ];
        for (value, ok) in cases {
            assert_eq!(date_ok(&value), ok, "{value}");
        }
    }

    #[test]
    fn a_slug_is_lowercase_letters_digits_and_hyphens() {
        for (text, ok) in [
            ("disc-b", true),
            ("disc-b\n", true),
            ("-disc", false),
            ("Disc", false),
            ("", false),
            ("disc b", false),
            ("disc-b\n\n", false),
            ("0", true),
            ("d\u{e9}", false),
        ] {
            assert_eq!(is_slug(text), ok, "{text:?}");
        }
    }

    #[test]
    fn every_problem_is_the_pythons_sentence() {
        let removed = with(with(with(entry("t0", 9, "commentary"), "removed", json!("2026-02-30")), "why", json!("typo")), "extra", json!(1));
        let data = json!({
            "format": "lazymkv audio labels v0",
            "slug": "Disc B",
            "titles": {
                "T0": {"title_kind": "film", "dialogue_heavy": "yes", "note": 5, "colour": "red"},
                "t1": [1],
                "t\u{ed}": {"title_kind": "extra"}
            },
            "tracks": [
                {"label": "t0", "stream": 3, "kind": "commenta\u{ef}re", "how": "told", "sure": "yes", "date": "5 Oct",
                 "saw_verdict": null, "note": null, "film_audible": "yes", "colour": "red"},
                {"label": "t0", "stream": 3.0, "kind": "lossy-companion", "how": "md5", "sure": true, "date": "2026-10-05",
                 "saw_verdict": false, "note": "", "film_audible": true,
                 "history": [{"label": "t0", "stream": 3, "kind": "x", "how": "y", "sure": true, "date": "2026-10-05",
                              "saw_verdict": false, "note": ""}, "h"]},
                "t",
                {"stream": -1}
            ],
            "removed": [removed, 7],
            "extra": 1
        });
        assert_eq!(
            problems(&data, Some("disc-b")),
            strings(&[
                "unknown top-level keys ['extra']",
                "format is 'lazymkv audio labels v0', not 'lazymkv audio labels v1'",
                "slug 'Disc B' is not lowercase letters, digits and hyphens",
                "title T0: not a rip label",
                "title T0: unknown keys ['colour']",
                "title T0: title_kind 'film' is not one of feature, episode, extra, other",
                "title T0: dialogue_heavy must be true, false or null",
                "title T0: note is not text",
                "title t1: not an object",
                "title t\u{ed}: not a rip label",
                "removed 0: unknown keys ['extra']",
                "removed 0: removed '2026-02-30' is not YYYY-MM-DD",
                "removed 1: not an object",
                "track t0 stream 3: unknown keys ['colour']",
                "track t0 stream 3: date '5 Oct' is not YYYY-MM-DD",
                "track t0 stream 3: kind 'commenta\u{ef}re' is not in the vocabulary (lazymkv-dev labels kinds)",
                "track t0 stream 3: how 'told' is not one of listened, packaging, menu, flag, thediscdb, md5, inferred",
                "track t0 stream 3: saw_verdict must be true or false",
                "track t0 stream 3: sure must be true or false",
                "track t0 stream 3: note is not text",
                "track t0 stream 3: film_audible is only for commentary and partial-commentary",
                "track t0 stream 3.0: stream is not ffprobe's stream number (an integer, 0 or more)",
                "track t0 stream 3.0: how md5 only says that a track is a duplicate",
                "track t0 stream 3.0: film_audible is only for commentary and partial-commentary",
                "track t0 stream 3.0 history 0: replaced None is not YYYY-MM-DD",
                "track t0 stream 3.0: history 1 is not an object",
                "track t0 stream 3.0: labelled twice",
                "track 2: not an object",
                "track None stream -1: missing ['date', 'how', 'kind', 'label', 'note', 'saw_verdict', 'sure']",
                "track None stream -1: label is not a rip label",
                "track None stream -1: stream is not ffprobe's stream number (an integer, 0 or more)",
                "track None stream -1: date None is not YYYY-MM-DD",
                "track None stream -1: kind None is not in the vocabulary (lazymkv-dev labels kinds)",
                "track None stream -1: how None is not one of listened, packaging, menu, flag, thediscdb, md5, inferred",
                "track None stream -1: saw_verdict must be true or false",
                "track None stream -1: sure must be true or false",
            ])
        );
    }

    #[test]
    fn a_file_of_the_wrong_shape_has_the_pythons_problems() {
        let moved = json!({"format": FORMAT, "slug": "disc-x", "titles": {}, "tracks": []});
        assert_eq!(problems(&moved, Some("disc-b")), strings(&["slug is 'disc-x' but the file is in corpus/owner/disc-b"]));
        assert_eq!(problems(&moved, None), Vec::<String>::new());
        assert_eq!(problems(&json!([]), Some("disc-b")), strings(&["the file is not a JSON object"]));
        assert_eq!(
            problems(&json!({"format": FORMAT, "slug": "disc-b", "titles": [], "tracks": null, "removed": null}), Some("disc-b")),
            strings(&["titles is not an object", "removed is not a list", "tracks is not a list"])
        );
        assert_eq!(
            check(vec![with(entry("t0", 3, "commentary"), "history", Value::Null)], json!({}), &[]),
            strings(&["track t0 stream 3: history is not a list"])
        );
    }

    #[test]
    fn python_equality_decides_twice_and_true_false_or_null() {
        let titles = json!({
            "t0": {"title_kind": "feature", "dialogue_heavy": 1, "note": ""},
            "t1": {"title_kind": "extra", "dialogue_heavy": 0.0},
            "t2": {"title_kind": "other", "dialogue_heavy": 2}
        });
        let tracks = vec![
            with(entry("t0", 3, "commentary"), "film_audible", json!(0)),
            with(entry("t0", 4, "commentary"), "film_audible", json!(1.0)),
            entry("t0", 4, "commentary"),
        ];
        assert_eq!(
            check(tracks, titles, &[]),
            strings(&["title t2: dialogue_heavy must be true, false or null", "track t0 stream 4: labelled twice"])
        );
        let float = vec![entry("t0", 3, "commentary"), with(entry("t0", 3, "commentary"), "stream", json!(3.0))];
        assert_eq!(
            check(float, json!({}), &[]),
            strings(&[
                "track t0 stream 3.0: stream is not ffprobe's stream number (an integer, 0 or more)",
                "track t0 stream 3.0: labelled twice",
            ])
        );
    }

    #[test]
    fn fitted_problems_are_the_pythons_sentences() {
        let data = json!({"format": "lazymkv audio fitted v0", "fitted": [
            {"slug": "Disc", "label": null, "stream": 2, "date": "2026-10-05", "rule": "r", "note": ""},
            {"slug": "disc-h", "label": "T0", "stream": 2, "date": "2026-10-05", "rule": "r", "note": ""},
            {"slug": "disc-h", "label": "t0", "stream": true, "date": "2026-10-05", "rule": 1, "note": null},
            {"slug": "disc-h", "label": "t0", "stream": 2, "date": "2026-10-5", "rule": "r"},
            "x"
        ], "extra": 1});
        assert_eq!(
            fitted_problems(&data),
            strings(&[
                "unknown top-level keys ['extra']",
                "format is 'lazymkv audio fitted v0', not 'lazymkv audio fitted v1'",
                "entry 0: slug 'Disc' is not a disc slug",
                "entry 0: a stream needs a label",
                "entry 1: label 'T0' is not a rip label",
                "entry 2: a label needs ffprobe's stream number",
                "entry 2: rule is not text",
                "entry 2: note is not text",
                "entry 3: keys must be ['date', 'label', 'note', 'rule', 'slug', 'stream'], not ['date', 'label', 'rule', 'slug', 'stream']",
                "entry 3: date '2026-10-5' is not YYYY-MM-DD",
                "entry 3: note is not text",
                "entry 4: not an object",
            ])
        );
        assert_eq!(fitted_problems(&json!([])), strings(&["the file is not a JSON object"]));
        assert_eq!(fitted_problems(&json!({"format": FITTED_FORMAT})), strings(&["fitted is not a list"]));
        let good = json!({"format": FITTED_FORMAT, "fitted": [fitted_entry("disc-g", None, None), fitted_entry("disc-h", Some("t0"), Some(2))]});
        assert_eq!(fitted_problems(&good), Vec::<String>::new());
    }

    #[test]
    fn not_counted_because_gives_each_reason() {
        let fitted = fitted_set(&[("fit-disc", None, None), ("disc-h", Some("t0"), Some(2))]);
        let direct = |kind: &str, how: &str, saw_verdict: bool, sure: bool| LabelEntry {
            raw: Value::Null,
            label: "t0".to_string(),
            stream: 3,
            kind: kind.to_string(),
            how: how.to_string(),
            sure,
            saw_verdict,
            film_audible: None,
        };
        assert_eq!(not_counted_because(&direct("main", "listened", false, true), "disc-b", &fitted), "a main track never counts");
        assert_eq!(
            not_counted_because(&direct("commentary", "menu", false, true), "disc-b", &fitted),
            "'menu' is an observation, not a check of the track's content (row 13, D23)"
        );
        assert_eq!(
            not_counted_because(&direct("commentary", "md5", false, true), "disc-b", &fitted),
            "'md5' is not a check of the track's content (row 13, D23)"
        );
        assert_eq!(not_counted_because(&direct("commentary", "listened", true, true), "disc-b", &fitted), "the owner saw the verdict first");
        assert_eq!(not_counted_because(&direct("commentary", "listened", false, false), "disc-b", &fitted), "the owner is not sure");
        assert_eq!(
            not_counted_because(&typed(&entry("t0", 2, "commentary")), "disc-h", &fitted),
            "the thresholds were fitted on it: spec row 13 or audio-fitted.json"
        );
        assert_eq!(not_counted_because(&direct("commentary", "listened", false, true), "disc-b", &fitted), "");
    }

    #[test]
    fn typed_views_are_made_only_after_validation() {
        let commentary = with(entry("t0", 3, "commentary"), "film_audible", json!(true));
        let e = typed(&commentary);
        assert_eq!(
            (e.label.as_str(), e.stream, e.kind.as_str(), e.how.as_str(), e.sure, e.saw_verdict, e.film_audible),
            ("t0", 3, "commentary", "listened", true, false, Some(true))
        );
        assert_eq!(text(&e.raw), text(&commentary));
        // 1 passes `in (True, False, None)` but is not `is True`.
        assert_eq!(typed(&with(entry("t0", 3, "commentary"), "film_audible", json!(1))).film_audible, None);
        assert_eq!(typed(&entry("t0", 3, "commentary")).film_audible, None);
        assert!(LabelEntry::from_value(&entry("t0", 3, "comentary")).is_none());
        let mut t = tree();
        put(&mut t, "disc-b/audio-labels.json", &json!({"format": FORMAT, "slug": "disc-b",
            "titles": {"t1": title("extra", json!(1)), "t0": {"title_kind": "feature", "dialogue_heavy": true}},
            "tracks": [entry("t0", 3, "commentary")]}));
        let labels = load(&t, &PathKey::new("disc-b/audio-labels.json")).unwrap();
        assert_eq!(labels.slug, "disc-b");
        let titles: Vec<(&str, &TitleFacts)> = labels.titles.iter().collect();
        let t1 = TitleFacts { title_kind: "extra".to_string(), dialogue_heavy: None, note: String::new() };
        let t0 = TitleFacts { title_kind: "feature".to_string(), dialogue_heavy: Some(true), note: String::new() };
        assert_eq!(titles, vec![("t1", &t1), ("t0", &t0)]);
        assert_eq!(labels.tracks.len(), 1);
        put(&mut t, "disc-c/audio-labels.json", &json!({"format": FORMAT, "slug": "disc-b", "titles": {}, "tracks": []}));
        assert_eq!(
            load(&t, &PathKey::new("disc-c/audio-labels.json")).unwrap_err(),
            "corpus/owner/disc-c/audio-labels.json: slug is 'disc-b' but the file is in corpus/owner/disc-c"
        );
        t.insert_file("disc-d/audio-labels.json", "{not json");
        let e = load(&t, &PathKey::new("disc-d/audio-labels.json")).unwrap_err();
        assert!(e.starts_with("corpus/owner/disc-d/audio-labels.json: not valid JSON ("), "{e}");
    }
}

mod edits {
    use super::*;

    #[test]
    fn prepare_save_sorts_and_refuses_a_new_problem() {
        let mut t = tree();
        let data = json!({"format": FORMAT, "slug": "disc-b",
            "titles": {"t2": title("episode", json!(false)), "t0": title("feature", json!(true)), "t1": title("extra", Value::Null)},
            "tracks": [entry("t1", 0, "commentary"), "junk", entry("t0", 5, "commentary"),
                       with(entry("t0", 0, "commentary"), "stream", json!(2.5)), without(&entry("t0", 4, "commentary"), "label"),
                       with(entry("t0", 0, "commentary"), "stream", json!(true)), entry("t0", 2, "commentary")],
            "extra": 1});
        put(&mut t, "disc-b/audio-labels.json", &data);
        let opened = open_for_change(&t, &PathKey::new("disc-b")).unwrap();
        assert_eq!(
            opened.before,
            strings(&[
                "unknown top-level keys ['extra']",
                "track 1: not an object",
                "track t0 stream 2.5: stream is not ffprobe's stream number (an integer, 0 or more)",
                "track None stream 4: missing ['label']",
                "track None stream 4: label is not a rip label",
                "track t0 stream True: stream is not ffprobe's stream number (an integer, 0 or more)",
            ])
        );
        let (saved, left) = prepare_save(&opened, opened.data.clone()).unwrap();
        assert_eq!(left, opened.before);
        let sorted = json!({"format": FORMAT, "slug": "disc-b",
            "titles": {"t0": title("feature", json!(true)), "t1": title("extra", Value::Null), "t2": title("episode", json!(false))},
            "tracks": ["junk", without(&entry("t0", 4, "commentary"), "label"), with(entry("t0", 0, "commentary"), "stream", json!(2.5)),
                       with(entry("t0", 0, "commentary"), "stream", json!(true)), entry("t0", 2, "commentary"),
                       entry("t0", 5, "commentary"), entry("t1", 0, "commentary")],
            "extra": 1});
        assert_eq!(text(&saved), text(&sorted));
        // The sort moved "junk" to index 0: "track 0: not an object" is a new sentence, as in the Python.
        let mut more = saved;
        more["tracks"].as_array_mut().unwrap().push(entry("t0", 7, "x"));
        assert_eq!(
            prepare_save(&opened, more).unwrap_err(),
            concat!(
                "corpus/owner/disc-b/audio-labels.json: not written: track 0: not an object; ",
                "track t0 stream 7: kind 'x' is not in the vocabulary (lazymkv-dev labels kinds)",
            )
        );
    }

    #[test]
    fn set_track_replaces_a_label_and_keeps_it_in_history() {
        let file = json!({"format": FORMAT, "slug": "disc-b",
            "titles": {"t1": title("extra", Value::Null), "t0": title("feature", json!(true))},
            "tracks": [with(entry("t0", 3, "commentary"), "film_audible", json!(true)), entry("t0", 2, "lossy-companion")]});
        let mut t = tree();
        put(&mut t, "disc-b/audio-labels.json", &file);
        let opened = open_for_change(&t, &PathKey::new("disc-b")).unwrap();
        let args = SetTrack {
            label: "t0".to_string(),
            stream: 3,
            kind: "partial-commentary".to_string(),
            how: "listened".to_string(),
            saw_verdict: true,
            date: "2026-10-06".to_string(),
            sure: true,
            film_audible: FilmAudible::Absent,
            note: "second listen".to_string(),
        };
        let outcome = set_track(&opened, &args, &Fitted::new()).unwrap();
        let old = with(with(entry("t0", 3, "commentary"), "film_audible", json!(true)), "replaced", json!("2026-10-06"));
        let new = json!({"label": "t0", "stream": 3, "kind": "partial-commentary", "how": "listened", "sure": true,
                         "date": "2026-10-06", "saw_verdict": true, "note": "second listen", "film_audible": null,
                         "history": [old]});
        assert_eq!(text(&outcome.entry), text(&new));
        assert!(outcome.changed);
        assert_eq!(outcome.why, "the owner saw the verdict first");
        assert!(outcome.left.is_empty());
        let written = json!({"format": FORMAT, "slug": "disc-b",
            "titles": {"t0": title("feature", json!(true)), "t1": title("extra", Value::Null)},
            "tracks": [entry("t0", 2, "lossy-companion"), new.clone()]});
        assert_eq!(text(outcome.write.as_ref().unwrap()), text(&written));
        // The same substance on another date keeps the label as it is and writes nothing.
        let mut again = tree();
        put(&mut again, "disc-b/audio-labels.json", &written);
        let reopened = open_for_change(&again, &PathKey::new("disc-b")).unwrap();
        let same = set_track(&reopened, &SetTrack { date: "2026-10-09".to_string(), ..args.clone() }, &Fitted::new()).unwrap();
        assert_eq!((same.changed, same.write.is_none(), text(&same.entry)), (false, true, text(&new)));
        assert_eq!(same.why, "the owner saw the verdict first");
    }

    #[test]
    fn unset_track_moves_the_label_to_removed() {
        let replaced = with(entry("t0", 3, "unknown"), "replaced", json!("2026-10-06"));
        let labelled = with(entry("t0", 3, "commentary"), "history", json!([replaced]));
        let mut t = tree();
        put(&mut t, "disc-b/audio-labels.json", &json!({"format": FORMAT, "slug": "disc-b", "titles": {},
            "tracks": [entry("t0", 2, "lossy-companion"), labelled.clone()]}));
        let opened = open_for_change(&t, &PathKey::new("disc-b")).unwrap();
        let (data, left) = unset_track(&opened, "t0", 3, "wrong stream", "2026-10-07").unwrap();
        let gone = with(with(labelled, "removed", json!("2026-10-07")), "why", json!("wrong stream"));
        let expected = json!({"format": FORMAT, "slug": "disc-b", "titles": {}, "tracks": [entry("t0", 2, "lossy-companion")],
                              "removed": [gone.clone()]});
        assert_eq!(text(&data), text(&expected));
        assert!(left.is_empty());
        assert_eq!(
            unset_track(&opened, "t0", 7, "wrong stream", "2026-10-07").unwrap_err(),
            "corpus/owner/disc-b/audio-labels.json: t0 stream 7 has no label"
        );
        assert_eq!(unset_track(&opened, "t0", 3, "", "2026-10-07").unwrap_err(), "unset needs --note: why the label is taken away");
        // A second unset appends to the "removed" list.
        let mut later = tree();
        put(&mut later, "disc-b/audio-labels.json", &data);
        let reopened = open_for_change(&later, &PathKey::new("disc-b")).unwrap();
        let (data, _) = unset_track(&reopened, "t0", 2, "typo", "2026-10-08").unwrap();
        let second = with(with(entry("t0", 2, "lossy-companion"), "removed", json!("2026-10-08")), "why", json!("typo"));
        assert_eq!(text(&data["removed"]), text(&json!([gone, second])));
    }

    #[test]
    fn set_title_and_add_fitted_build_the_pythons_entries() {
        let mut t = tree();
        t.insert_dir("disc-n");
        let opened = open_for_change(&t, &PathKey::new("disc-n")).unwrap();
        assert_eq!(text(&opened.data), text(&json!({"format": FORMAT, "slug": "disc-n", "titles": {}, "tracks": []})));
        assert!(opened.before.is_empty());
        let (facts, data, left) = set_title(&opened, "t0", "feature", None, "\u{e9}t\u{e9}").unwrap();
        assert_eq!(text(&facts), text(&json!({"title_kind": "feature", "dialogue_heavy": null, "note": "\u{e9}t\u{e9}"})));
        assert_eq!(text(&data), text(&json!({"format": FORMAT, "slug": "disc-n", "titles": {"t0": facts.clone()}, "tracks": []})));
        assert!(left.is_empty());
        assert_eq!(
            set_title(&opened, "T0", "feature", Some(true), "").unwrap_err(),
            "corpus/owner/disc-n/audio-labels.json: not written: title T0: not a rip label"
        );
        let shown = "corpus/owner/audio-fitted.json";
        let (disc, file) = add_fitted(None, shown, "disc-g", None, None, "abc123", "DC_MAX re-fit", "2026-10-05").unwrap();
        assert_eq!(
            text(&disc),
            text(&json!({"slug": "disc-g", "label": null, "stream": null, "date": "2026-10-05", "rule": "abc123", "note": "DC_MAX re-fit"}))
        );
        assert_eq!(text(&file), text(&json!({"format": FITTED_FORMAT, "fitted": [disc.clone()]})));
        let (track, file) = add_fitted(Some(&file), shown, "disc-h", Some("t0"), Some(2), "abc123", "", "2026-10-05").unwrap();
        assert_eq!(text(&file["fitted"]), text(&json!([disc, track])));
        assert_eq!(
            add_fitted(Some(&json!({"format": FITTED_FORMAT})), shown, "disc-g", None, None, "r", "", "2026-10-05").unwrap_err(),
            "corpus/owner/audio-fitted.json: does not validate, so nothing is added: fitted is not a list"
        );
        assert_eq!(
            add_fitted(None, shown, "disc-h", Some("T0"), Some(2), "r", "", "2026-10-05").unwrap_err(),
            "corpus/owner/audio-fitted.json: not written: entry 0: label 'T0' is not a rip label"
        );
    }

    #[test]
    fn open_for_change_refuses_what_it_cannot_change() {
        let mut t = tree();
        assert_eq!(open_for_change(&t, &PathKey::new("disc-z")).unwrap_err(), "corpus/owner/disc-z: no such recording folder");
        t.insert_file("disc-b/audio-labels.json", "{\"tracks\": [");
        let e = open_for_change(&t, &PathKey::new("disc-b")).unwrap_err();
        assert!(e.starts_with("corpus/owner/disc-b/audio-labels.json: not valid JSON ("), "{e}");
        assert!(e.ends_with("); restore it from git (git checkout -- corpus/owner/disc-b/audio-labels.json)"), "{e}");
        t.insert_file("disc-c/audio-labels.json", "[]");
        assert_eq!(
            open_for_change(&t, &PathKey::new("disc-c")).unwrap_err(),
            concat!(
                "corpus/owner/disc-c/audio-labels.json: has no 'tracks' list or 'titles' object; ",
                "restore it from git (git checkout -- corpus/owner/disc-c/audio-labels.json)",
            )
        );
        put(&mut t, "disc-d/audio-labels.json", &json!({"format": FORMAT, "slug": "disc-x", "titles": {}, "tracks": []}));
        let opened = open_for_change(&t, &PathKey::new("disc-d")).unwrap();
        assert_eq!((opened.shown.as_str(), opened.folder_name.as_str()), ("corpus/owner/disc-d/audio-labels.json", "disc-d"));
        assert_eq!(opened.before, strings(&["slug is 'disc-x' but the file is in corpus/owner/disc-d"]));
        // PowerShell writes a byte-order mark: the file is read as utf-8-sig.
        let mut bom = b"\xef\xbb\xbf".to_vec();
        bom.extend_from_slice(br#"{"format": "lazymkv audio labels v1", "slug": "disc-e", "titles": {}, "tracks": []}"#);
        t.insert_file("disc-e/audio-labels.json", bom);
        assert!(open_for_change(&t, &PathKey::new("disc-e")).unwrap().before.is_empty());
    }
}

mod walkers {
    use super::*;

    /// A level folder's manifest.txt and streams.json as audio-levels.ps1 writes them, main stream 1 with six
    /// channels and streams.json listing every stream up to the highest measured one. Py: test_audio_dataset.folder,
    /// without the level series (`auto` reads none).
    fn level_folder(t: &mut MemTree, label_dir: &str, main_md5: &str, cands: &[(i64, &str)]) {
        let numbers: Vec<String> = std::iter::once(1).chain(cands.iter().map(|(n, _)| *n)).map(|n| n.to_string()).collect();
        let dir = format!("{label_dir}/audio-levels-20ms-{}", numbers.join("-"));
        let mut lines = vec![
            "# lazymkv audio-levels v2 (experiment 13)".to_string(),
            "file=x.mkv bytes=1 seconds=1 frame_ms=20 band_hz=300-3400".to_string(),
            format!("main=1 codec=dts profile=- channels=6 layout=5.1(side) language=eng fc=yes frames=100 md5={main_md5}"),
        ];
        for (n, md5) in cands {
            lines.push(format!("cand={n} codec=ac3 profile=- channels=2 layout=stereo language=eng frames=100 md5={md5}"));
        }
        t.insert_file(&format!("{dir}/manifest.txt"), lines.join("\n") + "\n");
        let top = cands.iter().map(|(n, _)| *n).max().unwrap_or(1).max(1);
        let mut streams = vec![json!({"index": 0, "codec_type": "video"})];
        for k in 1..=top {
            let (codec, channels) = if k == 1 { ("dts", 6) } else { ("ac3", 2) };
            let name = format!("t{k}");
            streams.push(json!({"index": k, "codec_type": "audio", "codec_name": codec, "channels": channels,
                                "tags": {"language": "eng", "title": name}}));
        }
        put(t, &format!("{dir}/streams.json"), &json!({"streams": streams}));
    }

    /// The recording of the Python's Routine auto test: t0 measured with streams 1 to 7 (2 and 5 the same packets,
    /// 3 and 7 the main's, 6 flagged 2304 by the scan), t1 with streams 1 and 2 while the scan's title 1 has three
    /// audio streams; `labels` is its audio-labels.json.
    fn disc_d(t: &mut MemTree, labels: &Value) {
        level_folder(t, "disc-d/t0", "aa", &[(2, "bb"), (3, "aa"), (4, "cc"), (5, "bb"), (6, "dd"), (7, "aa")]);
        t.insert_file("disc-d/t0/rip-summary.json", r#"{"ok": true, "title": 0}"#);
        level_folder(t, "disc-d/t1", "ee", &[(2, "ff")]);
        t.insert_file("disc-d/t1/rip-summary.json", r#"{"ok": true, "title": 1}"#);
        let flags = [(1, 0), (2, 0), (3, 0), (4, 0), (5, 0), (6, 2304), (7, 0)];
        let mut scan = vec!["TINFO:0,9,0,\"0:07:00\"".to_string(), "SINFO:0,0,1,0,\"Video\"".to_string()];
        scan.extend(flags.iter().map(|(s, _)| format!("SINFO:0,{s},1,0,\"Audio\"")));
        scan.extend(flags.iter().map(|(s, f)| format!("SINFO:0,{s},22,0,\"{f}\"")));
        scan.push("SINFO:1,0,1,0,\"Video\"".to_string());
        scan.extend((1..=3).map(|s| format!("SINFO:1,{s},1,0,\"Audio\"")));
        t.insert_file("disc-d/scan.txt", scan.join("\n") + "\n");
        put(t, "disc-d/audio-labels.json", labels);
    }

    /// disc-d's labels file before `auto`: the owner labelled t0 stream 3.
    fn owners_labels() -> Value {
        json!({"format": FORMAT, "slug": "disc-d", "titles": {}, "tracks": [with(entry("t0", 3, "duplicate"), "note", json!("the owner's"))]})
    }

    /// A label `auto` writes.
    fn automatic(stream: i64, kind: &str, how: &str, note: &str) -> Value {
        with(with(entry("t0", stream, kind), "how", json!(how)), "note", json!(note))
    }

    #[test]
    fn load_fitted_takes_row_13s_disc_and_the_file() {
        let mut t = tree();
        let root = PathKey::root();
        assert_eq!(load_fitted(&t, &root, Some("fit-disc")).unwrap(), fitted_set(&[("fit-disc", None, None)]));
        assert!(load_fitted(&t, &root, Some("")).unwrap().is_empty());
        assert!(load_fitted(&t, &root, None).unwrap().is_empty());
        put(&mut t, "audio-fitted.json", &json!({"format": FITTED_FORMAT, "fitted": [
            fitted_entry("disc-g", None, None), fitted_entry("disc-h", Some("t0"), Some(2)), fitted_entry("disc-g", None, None)]}));
        let fitted = load_fitted(&t, &root, Some("fit-disc")).unwrap();
        assert_eq!(fitted, fitted_set(&[("fit-disc", None, None), ("disc-g", None, None), ("disc-h", Some("t0"), Some(2))]));
        assert!(is_fitted(&fitted, "disc-g", "t9", 9));
        assert!(is_fitted(&fitted, "disc-h", "t0", 2));
        assert!(!is_fitted(&fitted, "disc-h", "t0", 3));
        assert!(!is_fitted(&fitted, "disc-h", "t1", 2));
        let mut bad = tree();
        put(&mut bad, "audio-fitted.json", &json!({"format": FITTED_FORMAT}));
        assert_eq!(load_fitted(&bad, &root, None).unwrap_err(), "corpus/owner/audio-fitted.json: fitted is not a list");
        let mut broken = tree();
        broken.insert_file("audio-fitted.json", "{");
        assert!(load_fitted(&broken, &root, None).unwrap_err().starts_with("corpus/owner/audio-fitted.json: not valid JSON ("));
    }

    #[test]
    fn check_lines_reports_each_file_and_skips_golden() {
        let mut t = tree();
        put(&mut t, "disc-b/audio-labels.json", &json!({"format": FORMAT, "slug": "disc-b",
            "titles": {"t0": title("feature", json!(true))}, "tracks": [entry("t0", 3, "commentary")]}));
        put(&mut t, "disc-c/audio-labels.json", &json!({"format": "x", "slug": "disc-c", "titles": {}, "tracks": []}));
        put(&mut t, "golden/audio-labels.json", &json!({}));
        put(&mut t, "audio-fitted.json", &json!({"format": FITTED_FORMAT, "fitted": [
            fitted_entry("disc-g", None, None), fitted_entry("disc-h", Some("t0"), Some(2))]}));
        t.insert_dir("disc-e");
        assert_eq!(
            check_lines(&t, &PathKey::root()),
            (
                strings(&[
                    "ok corpus/owner/disc-b/audio-labels.json (1 tracks, 1 titles)",
                    "corpus/owner/disc-c/audio-labels.json: format is 'x', not 'lazymkv audio labels v1'",
                    "ok corpus/owner/audio-fitted.json (2 entries)",
                ]),
                1
            )
        );
        assert_eq!(
            check_lines(&t, &PathKey::new("disc-b")),
            (strings(&["ok corpus/owner/disc-b/audio-labels.json (1 tracks, 1 titles)"]), 0)
        );
        assert_eq!(check_lines(&t, &PathKey::new("disc-e")), (strings(&["corpus/owner/disc-e: no audio-labels.json"]), 0));
    }

    #[test]
    fn scan_flags_says_why_it_cannot_match() {
        let mut t = tree();
        disc_d(&mut t, &owners_labels());
        let d = PathKey::new("disc-d");
        let expected: BTreeMap<i64, i64> = [(1, 0), (2, 0), (3, 0), (4, 0), (5, 0), (6, 2304), (7, 0)].into_iter().collect();
        assert_eq!(scan_flags(&t, &d, "t0").unwrap(), expected);
        assert_eq!(scan_flags(&t, &d, "t1").unwrap_err(), "the file has 2 audio streams and the scan's title 1 has 3");
        assert_eq!(
            scan_flags(&t, &d, "t9").unwrap_err(),
            "no scan or rip summary ([Errno 2] No such file or directory: 'corpus/owner/disc-d/t9/rip-summary.json')"
        );
        t.insert_file("disc-d/t2/rip-summary.json", r#"{"ok": true}"#);
        assert_eq!(scan_flags(&t, &d, "t2").unwrap_err(), "no scan or rip summary ('title')");
        t.insert_file("disc-d/t3/rip-summary.json", r#"{"ok": true, "title": 0}"#);
        assert_eq!(scan_flags(&t, &d, "t3").unwrap_err(), "no streams.json");
    }

    #[test]
    fn auto_labels_duplicates_and_derived_cores_once() {
        let mut t = tree();
        disc_d(&mut t, &owners_labels());
        let d = PathKey::new("disc-d");
        let fitted = fitted_set(&[("fit-disc", None, None)]);
        let opened = open_for_change(&t, &d).unwrap();
        let (lines, written) = auto(&t, &d, &opened, "2026-10-05", &fitted).unwrap();
        let t1_line = "t1: derived cores not labelled: the file has 2 audio streams and the scan's title 1 has 3";
        assert_eq!(
            lines,
            strings(&[
                "labelled t0 stream 5: duplicate (md5), counts=False (a duplicate track never counts)",
                "labelled t0 stream 6: derived-core (flag), counts=False ('flag' is an observation, not a check of the track's content (row 13, D23))",
                "labelled t0 stream 7: duplicate (md5), counts=False (a duplicate track never counts)",
                t1_line,
            ])
        );
        let expected = json!({"format": FORMAT, "slug": "disc-d", "titles": {}, "tracks": [
            with(entry("t0", 3, "duplicate"), "note", json!("the owner's")),
            automatic(5, "duplicate", "md5", "auto: same packets as stream 2 (bb)"),
            automatic(6, "derived-core", "flag", "auto: scan flags 2304 (2048: derived by MakeMKV)"),
            automatic(7, "duplicate", "md5", "auto: same packets as stream 1 (aa)"),
        ]});
        let written = written.expect("auto labelled three tracks");
        assert_eq!(text(&written), text(&expected));
        // Run again over the file it wrote: nothing more to label, nothing to write.
        let mut again = tree();
        disc_d(&mut again, &written);
        let reopened = open_for_change(&again, &d).unwrap();
        assert_eq!(auto(&again, &d, &reopened, "2026-10-05", &fitted).unwrap(), (strings(&[t1_line]), None));
        let mut empty = tree();
        empty.insert_dir("disc-e");
        let e = PathKey::new("disc-e");
        let opened = open_for_change(&empty, &e).unwrap();
        assert_eq!(auto(&empty, &e, &opened, "2026-10-05", &fitted).unwrap(), (strings(&["nothing to label automatically"]), None));
    }

    #[test]
    fn labels_json_says_whether_each_track_counts() {
        let mut t = tree();
        put(&mut t, "disc-b/audio-labels.json", &json!({"format": FORMAT, "slug": "disc-b", "titles": {}, "tracks": [
            entry("t0", 3, "commentary"), with(entry("t0", 4, "lossy-companion"), "how", json!("menu")), entry("t0", 2, "commentary")]}));
        put(&mut t, "disc-c/audio-labels.json", &json!({"format": "x", "slug": "disc-c", "titles": {}, "tracks": []}));
        let fitted = fitted_set(&[("fit-disc", None, None), ("disc-b", Some("t0"), Some(2))]);
        let count = |stream: i64, kind: &str, how: &str, why: &str| LabelCount {
            label: "t0".to_string(),
            stream,
            kind: kind.to_string(),
            how: how.to_string(),
            not_counted_because: why.to_string(),
        };
        let got = labels_json(&t, &PathKey::new("disc-b"), &fitted);
        assert_eq!(
            got,
            LabelsJson {
                check: strings(&["ok corpus/owner/disc-b/audio-labels.json (3 tracks, 0 titles)"]),
                exit_code: 0,
                tracks: vec![
                    count(3, "commentary", "listened", ""),
                    count(4, "lossy-companion", "menu", "'menu' is an observation, not a check of the track's content (row 13, D23)"),
                    count(2, "commentary", "listened", "the thresholds were fitted on it: spec row 13 or audio-fitted.json"),
                ],
            }
        );
        assert_eq!(
            serde_json::to_string(&got.tracks[0]).unwrap(),
            r#"{"label":"t0","stream":3,"kind":"commentary","how":"listened","not_counted_because":""}"#
        );
        assert_eq!(
            labels_json(&t, &PathKey::new("disc-c"), &fitted),
            LabelsJson {
                check: strings(&["corpus/owner/disc-c/audio-labels.json: format is 'x', not 'lazymkv audio labels v1'"]),
                exit_code: 2,
                tracks: vec![],
            }
        );
    }

    #[test]
    fn kinds_left_and_yes_no_lines_are_the_pythons() {
        let lines = kinds_lines();
        assert_eq!(lines.len(), 19);
        assert_eq!(lines[0], "main                 programme  keep     the programme's main soundtrack; as a reference it is never judged");
        assert_eq!(lines[3], "laugh-track-version  programme  keep     the programme with or without its laugh track");
        assert_eq!(
            lines[4],
            concat!(
                "dub                  programme  keep     the programme in another language, tagged with it (an English dub of a ",
                "Spanish film); kept (R3, R4); content never drops a candidate in another language than the reference (EXP-13 addendum 9.2)",
            )
        );
        assert_eq!(lines[17], "unknown              unknown    unknown  the owner does not know");
        assert_eq!(
            lines[18],
            concat!(
                "how: listened = a person listened to it: the track's content was checked (counts); packaging = the packaging says ",
                "so (an observation: does not count on its own, D23); menu = the disc's menu says so (an observation: does not ",
                "count on its own, D23); flag = a flag in MakeMKV's scan (an observation: does not count on its own, D23); ",
                "thediscdb = TheDiscDB's track description (an observation: does not count on its own, D23); md5 = identical ",
                "packet MD5 (an exact duplicate; no listening needed); inferred = inferred, for example from the disc's layout; ",
                "never counts",
            )
        );
        assert_eq!(left_lines(&[]), Vec::<String>::new());
        assert_eq!(
            left_lines(&strings(&["a", "b"])),
            strings(&["warning: the file still has 2 problem(s): a; b"])
        );
        assert_eq!(yes_no_unknown("YES", false), Ok(Some(true)));
        assert_eq!(yes_no_unknown("No", true), Ok(Some(false)));
        assert_eq!(yes_no_unknown("unknown", true), Ok(None));
        assert_eq!(yes_no_unknown("unknown", false), Err("'unknown': say yes or no".to_string()));
        assert_eq!(yes_no_unknown("maybe", true), Err("'maybe': say yes or no or unknown".to_string()));
    }
}
