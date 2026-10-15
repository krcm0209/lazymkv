//! Tests of core::audio::dataset: the ported Routine tests and Dataset's cached-NaN test of
//! scripts/phase0/test_audio_dataset.py (8), over in-memory trees; then (new) the job lines line for line against the
//! Python, the golden/ folder (Review Focus 2), --set's values and messages, the cache key formula of spec 3.3, the
//! snapshot reader and the closing lines. The harness itself is tested through `lazymkv-dev retune`
//! (crates/dev/src/cmd/retune/tests.rs), where its files are on disk as the Python's tests had them.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::json;

use super::{
    cache_key, cache_prefix, destination, find_folders, levels_args, parse_sets, read_snapshot, result_line,
    snapshot_line, why_phrase, Destination, Measurer, Overrides, SNAPSHOT_FORMAT, SNAPSHOT_FORMAT_V2,
};
use crate::audio::labels::{auto, load_fitted, open_for_change, FORMAT};
use crate::audio::measures::{Constants, Measures, Num};
use crate::audio::rule::AnalyseFn;
use crate::recording::{MemTree, Ordered, PathKey};
use crate::testsupport::{disc_d_files, entry, programme, with};

/// The Python test's probe_file(): an ffprobe-streams.json with a video stream and the given audio streams
/// (codec, profile, channels, language), numbered from 1.
fn probe_file(tree: &mut MemTree, path: &str, streams: &[(&str, Option<&str>, i64, &str)]) {
    let mut list = vec![json!({"index": 0, "codec_type": "video"})];
    for (k, (codec, profile, channels, language)) in streams.iter().enumerate() {
        list.push(json!({"index": k + 1, "codec_type": "audio", "codec_name": codec, "profile": profile,
                         "channels": channels, "tags": {"language": language}}));
    }
    tree.insert_file(path, serde_json::to_string(&json!({"streams": list})).unwrap());
}

/// The job lines of levels_args. Py: Routine.jobs.
fn jobs(tree: &MemTree, slug: &str, label: &str) -> Vec<String> {
    levels_args(tree, &PathKey::new(slug), label)
        .unwrap()
        .into_iter()
        .filter(|line| line.starts_with("scripts/phase0/job.sh"))
        .collect()
}

fn sets(items: &[&str]) -> Result<Overrides, String> {
    parse_sets(&items.iter().map(|item| item.to_string()).collect::<Vec<String>>())
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| item.to_string()).collect()
}

/// What the Python's levels_args printed for some-film with tmdb.json {"original_language": "es"}, 2026-10-13.
const COMMENTARY: &[&str] = &[
    "some-film/commentary: 3 audio streams (from ffprobe-streams.json); original language spa (TMDb es)",
    "  stream 1: dts DTS-HD MA 6 ch lang=spa",
    "  stream 2: dts DTS 6 ch lang=spa",
    "  stream 3: ac3 2 ch lang=eng",
    "  programme reference: stream 1 (dts DTS-HD MA 6 ch), placed first in the original language (spec 8.6 steps 3 and 4); candidates: every other stream in eng, spa or with no language, whatever its language",
    "scripts/phase0/job.sh audio-levels.ps1 audio-levels-some-film-commentary -Slug some-film -Label commentary -Main 1 -Cand 2,3",
    "scripts/phase0/wait.sh audio-levels-some-film-commentary",
];
const DUB_FIRST: &[&str] = &[
    "some-film/dub-first: 5 audio streams (from ffprobe-streams.json); original language spa (TMDb es)",
    "  stream 1: dts DTS-HD MA 6 ch lang=spa",
    "  stream 2: truehd 8 ch lang=eng",
    "  stream 3: ac3 2 ch lang=eng",
    "  stream 4: ac3 6 ch lang=fra",
    "  stream 5: ac3 2 ch lang=fra",
    "  programme reference: stream 1 (dts DTS-HD MA 6 ch), placed first in the original language (spec 8.6 steps 3 and 4); candidates: every other stream in eng, spa or with no language, whatever its language",
    "  dataset only: fra is not a wanted language; stream 4 against 5",
    "scripts/phase0/job.sh audio-levels.ps1 audio-levels-some-film-dub-first-1 -Slug some-film -Label dub-first -Main 1 -Cand 2,3",
    "scripts/phase0/wait.sh audio-levels-some-film-dub-first-1",
    "scripts/phase0/job.sh audio-levels.ps1 audio-levels-some-film-dub-first-4 -Slug some-film -Label dub-first -Main 4 -Cand 5",
    "scripts/phase0/wait.sh audio-levels-some-film-dub-first-4",
];
const UND_MAIN: &[&str] = &[
    "some-film/und-main: 2 audio streams (from ffprobe-streams.json); original language spa (TMDb es)",
    "  stream 1: dts DTS-HD MA 6 ch lang=und",
    "  stream 2: ac3 2 ch lang=eng",
    "  programme reference: stream 2 (ac3 2 ch), placed first in English (spec 8.6 steps 3 and 4); candidates: every other stream in eng, spa or with no language, whatever its language",
    "  note: the file has no stream in the original language (spa): the reference may not be the programme (an English-only rip of a foreign film can hold only a commentary)",
    "  note: the reference is a lossy stream of 2 channel(s), and stream 1 with no language code is lossless or has more than 2 channels: if it is the programme, the reference may be a commentary (EXP-13 addendum 9.1); measure as printed and say so in the report",
    "scripts/phase0/job.sh audio-levels.ps1 audio-levels-some-film-und-main -Slug some-film -Label und-main -Main 2 -Cand 1",
    "scripts/phase0/wait.sh audio-levels-some-film-und-main",
];

/// The rule's and the constants' names as --set's refusal lists them (Python, 2026-10-13).
const NOT_A_NAME: &str = "--set NOPE: neither a rule threshold (OVERLAP_MIN, MAIN_SILENT_MAX, GAP_S_MIN, DC_MAX, AG_MIN, TENTHS_MIN, TENTHS_SHARE, SMI_MIN, LS_MAX, ADDED_S_MIN, AD_DC_MIN, AD_ADL_MAX, AD_AG_MIN, AD_AG_S_MIN, AD_TENTHS_SHARE, NOTE_COMM_DC_MAX, NOTE_COMM_AG_MIN, NOTE_AD_DC_MIN, NOTE_AD_AG_MIN, NOTE_AD_AG_S_MIN) nor a measure constant (MATCH_R, MISMATCH_R, X_ACTIVE_BELOW, DLG_BELOW, GAP_MIN, GAP_TRIM, ADDED_DB, SILENT_DB, COARSE_LAG, BLOCK_LAG, WIN_LAG, LAG_MIN_R, GAIN_PCT, TENTH_GAP_S, TENTH_COMM_AG, TENTH_AD_AG)";

/// "lazymkv-rule:1" + json.dumps(constants, sort_keys=True), with the defaults and with SILENT_DB=-80, GAP_MIN=30.
const PREFIX_DEFAULT: &str = r#"lazymkv-rule:1{"ADDED_DB": 10.0, "BLOCK_LAG": 100, "COARSE_LAG": 100, "DLG_BELOW": 25.0, "GAIN_PCT": 0.75, "GAP_MIN": 25, "GAP_TRIM": 5, "LAG_MIN_R": 0.5, "MATCH_R": 0.7, "MISMATCH_R": 0.4, "SILENT_DB": -90.0, "TENTH_AD_AG": 0.05, "TENTH_COMM_AG": 0.4, "TENTH_GAP_S": 10.0, "WIN_LAG": 5, "X_ACTIVE_BELOW": 20.0}"#;
const PREFIX_SET: &str = r#"lazymkv-rule:1{"ADDED_DB": 10.0, "BLOCK_LAG": 100, "COARSE_LAG": 100, "DLG_BELOW": 25.0, "GAIN_PCT": 0.75, "GAP_MIN": 30, "GAP_TRIM": 5, "LAG_MIN_R": 0.5, "MATCH_R": 0.7, "MISMATCH_R": 0.4, "SILENT_DB": -80.0, "TENTH_AD_AG": 0.05, "TENTH_COMM_AG": 0.4, "TENTH_GAP_S": 10.0, "WIN_LAG": 5, "X_ACTIVE_BELOW": 20.0}"#;

#[test]
fn args_rank_by_channels_then_lossless_then_order_and_prefer_english() {
    let mut tree = MemTree::new(".", "tmp");
    probe_file(&mut tree, "some-film/all-tracks/ffprobe-streams.json",
               &[("truehd", None, 8, "eng"), ("ac3", None, 2, "eng"), ("ac3", None, 6, "fra"), ("dts", Some("DTS"), 6, "eng")]);
    probe_file(&mut tree, "some-film/one-each/ffprobe-streams.json", &[("ac3", None, 6, "fra"), ("ac3", None, 2, "eng")]);
    probe_file(&mut tree, "some-film/no-english/ffprobe-streams.json", &[("ac3", None, 2, "fra"), ("ac3", None, 6, "deu")]);
    probe_file(&mut tree, "some-film/lossless-second/ffprobe-streams.json",
               &[("dts", Some("DTS"), 6, "eng"), ("dts", Some("DTS-HD MA"), 6, "eng")]);
    probe_file(&mut tree, "some-film/dtsx/ffprobe-streams.json",
               &[("dts", Some("DTS"), 8, "eng"), ("dts", Some("DTS-HD MA + DTS:X"), 8, "eng")]);
    probe_file(&mut tree, "some-film/two-languages/ffprobe-streams.json",
               &[("ac3", None, 6, "eng"), ("ac3", None, 2, "eng"), ("ac3", None, 6, "fra"), ("ac3", None, 2, "fra")]);
    assert_eq!(jobs(&tree, "some-film", "all-tracks"), strings(&["scripts/phase0/job.sh audio-levels.ps1 audio-levels-some-film-all-tracks -Slug some-film -Label all-tracks -Main 1 -Cand 2,4"]));
    assert_eq!(jobs(&tree, "some-film", "one-each"), strings(&["scripts/phase0/job.sh audio-levels.ps1 audio-levels-some-film-one-each -Slug some-film -Label one-each -Main 2"]));
    assert!(jobs(&tree, "some-film", "no-english")[0].ends_with("-Main 2"));
    assert!(jobs(&tree, "some-film", "lossless-second")[0].ends_with("-Main 2 -Cand 1"));
    assert!(jobs(&tree, "some-film", "dtsx")[0].ends_with("-Main 2 -Cand 1"));
    let two: Vec<String> = jobs(&tree, "some-film", "two-languages")
        .iter()
        .map(|j| format!("{}{}", j.split_whitespace().nth(2).unwrap(), j.split(" -Main ").nth(1).unwrap()))
        .collect();
    assert_eq!(two, strings(&["audio-levels-some-film-two-languages-11 -Cand 2", "audio-levels-some-film-two-languages-33 -Cand 4"]));
}

#[test]
fn args_take_the_programme_reference_from_the_original_language() {
    let mut tree = MemTree::new(".", "tmp");
    probe_file(&mut tree, "some-film/commentary/ffprobe-streams.json",
               &[("dts", Some("DTS-HD MA"), 6, "spa"), ("dts", Some("DTS"), 6, "spa"), ("ac3", None, 2, "eng")]);
    probe_file(&mut tree, "some-film/dub-first/ffprobe-streams.json",
               &[("dts", Some("DTS-HD MA"), 6, "spa"), ("truehd", None, 8, "eng"), ("ac3", None, 2, "eng"),
                 ("ac3", None, 6, "fra"), ("ac3", None, 2, "fra")]);
    probe_file(&mut tree, "some-film/no-spanish/ffprobe-streams.json",
               &[("truehd", None, 8, "eng"), ("ac3", None, 2, "eng"), ("ac3", None, 2, "und")]);
    tree.insert_file("some-film/tmdb.json", r#"{"original_language": "es"}"#);
    let slug = PathKey::new("some-film");
    let lines = levels_args(&tree, &slug, "commentary").unwrap();
    assert!(lines[0].contains("original language spa (TMDb es)"), "{lines:?}");
    assert!(lines[4].contains("programme reference: stream 1 (dts DTS-HD MA 6 ch), placed first in the original language"), "{lines:?}");
    assert_eq!(jobs(&tree, "some-film", "commentary"), strings(&["scripts/phase0/job.sh audio-levels.ps1 audio-levels-some-film-commentary -Slug some-film -Label commentary -Main 1 -Cand 2,3"]));
    let tails: Vec<String> = jobs(&tree, "some-film", "dub-first").iter().map(|j| j.split(" -Main ").nth(1).unwrap().to_string()).collect();
    assert_eq!(tails, strings(&["1 -Cand 2,3", "4 -Cand 5"]));
    assert!(levels_args(&tree, &slug, "dub-first").unwrap().contains(&"  dataset only: fra is not a wanted language; stream 4 against 5".to_string()));
    assert!(jobs(&tree, "some-film", "no-spanish")[0].ends_with("-Main 1 -Cand 2,3"));
    let no_spanish = levels_args(&tree, &slug, "no-spanish").unwrap().join("\n");
    assert!(no_spanish.contains("placed first in English"), "{no_spanish}");
    assert!(no_spanish.contains("note: the file has no stream in the original language (spa)"), "{no_spanish}");
    assert!(!levels_args(&tree, &slug, "commentary").unwrap().join("\n").contains("note:"));
    tree.insert_file("some-film/tmdb.json", r#"{"original_language": "xx"}"#);
    assert!(levels_args(&tree, &slug, "commentary").unwrap()[0].contains("original language none (TMDb xx: a work without speech"));
    assert!(jobs(&tree, "some-film", "commentary")[0].ends_with("-Main 3")); // English then: the stereo track alone
    tree.insert_file("some-film/tmdb.json", r#"{"original_language": "qq"}"#);
    assert!(levels_args(&tree, &slug, "commentary").unwrap()[0].contains("original language unknown (TMDb 'qq'"));
}

#[test]
fn args_keep_a_lossy_stereo_track_from_becoming_the_reference() {
    let mut tree = MemTree::new(".", "tmp");
    probe_file(&mut tree, "old-film/mono-lossless/ffprobe-streams.json", &[("pcm_s24le", None, 1, "eng"), ("ac3", None, 2, "eng")]); // spec 8.6 G1
    probe_file(&mut tree, "old-film/mono-lossy/ffprobe-streams.json", &[("ac3", None, 1, "eng"), ("ac3", None, 2, "eng")]); // G2
    probe_file(&mut tree, "old-film/stereo-later/ffprobe-streams.json",
               &[("ac3", None, 2, "eng"), ("ac3", None, 1, "eng"), ("ac3", None, 2, "eng")]);
    assert!(jobs(&tree, "old-film", "mono-lossless")[0].ends_with("-Main 1 -Cand 2"));
    assert!(jobs(&tree, "old-film", "mono-lossy")[0].ends_with("-Main 1 -Cand 2"));
    assert!(jobs(&tree, "old-film", "stereo-later")[0].ends_with("-Main 1 -Cand 2,3"));
}

#[test]
fn args_take_every_code_of_the_original_language() {
    let mut tree = MemTree::new(".", "tmp");
    probe_file(&mut tree, "hk-film/t/ffprobe-streams.json",
               &[("dts", Some("DTS-HD MA"), 6, "chi"), ("ac3", None, 2, "yue"), ("ac3", None, 2, "eng")]);
    tree.insert_file("hk-film/tmdb.json", r#"{"original_language": "cn"}"#);
    assert!(jobs(&tree, "hk-film", "t")[0].ends_with("-Main 1 -Cand 2,3"));
}

#[test]
fn args_g1_alone_and_the_notes_on_a_reference_that_may_not_be_the_programme() {
    let mut tree = MemTree::new(".", "tmp");
    probe_file(&mut tree, "old-film/g1-alone/ffprobe-streams.json", &[("ac3", None, 2, "eng"), ("pcm_s24le", None, 1, "eng")]); // G2 would keep stream 1
    probe_file(&mut tree, "old-film/score-first/ffprobe-streams.json",
               &[("pcm_s24le", None, 1, "eng"), ("dts", Some("DTS-HD MA"), 2, "eng"), ("ac3", None, 2, "eng")]);
    probe_file(&mut tree, "old-film/und-main/ffprobe-streams.json", &[("dts", Some("DTS-HD MA"), 6, "und"), ("ac3", None, 2, "eng")]);
    probe_file(&mut tree, "old-film/malayalam/ffprobe-streams.json", &[("dts", Some("DTS-HD MA"), 6, "mal"), ("ac3", None, 2, "eng")]);
    let slug = PathKey::new("old-film");
    assert!(jobs(&tree, "old-film", "g1-alone")[0].ends_with("-Main 2 -Cand 1"));
    assert!(!levels_args(&tree, &slug, "g1-alone").unwrap().join("\n").contains("note:")); // G1 passed over a lossy stereo track
    let score = levels_args(&tree, &slug, "score-first").unwrap().join("\n");
    assert!(jobs(&tree, "old-film", "score-first")[0].ends_with("-Main 2 -Cand 1,3"));
    assert!(score.contains("note: the reference is not the first eng stream of the file (stream 1, pcm_s24le 1 ch, comes first)"), "{score}");
    let und = levels_args(&tree, &slug, "und-main").unwrap().join("\n");
    assert!(jobs(&tree, "old-film", "und-main")[0].ends_with("-Main 2 -Cand 1"));
    assert!(und.contains("note: the reference is a lossy stream of 2 channel(s), and stream 1 with no language code is lossless or has more than 2 channels"), "{und}");
    tree.insert_file("old-film/tmdb.json", r#"{"original_language": "en"}"#);
    assert!(levels_args(&tree, &slug, "und-main").unwrap()[0].contains("original language English (TMDb en)"));
    tree.insert_file("old-film/tmdb.json", r#"{"original_language": "ml"}"#);
    assert!(levels_args(&tree, &slug, "malayalam").unwrap()[0].contains("original language mal (TMDb ml)"));
    assert!(jobs(&tree, "old-film", "malayalam")[0].ends_with("-Main 1 -Cand 2"));
}

#[test]
fn args_without_a_probe_take_a_rip_of_the_same_title_and_size_or_say_how_to_probe() {
    let mut tree = MemTree::new(".", "tmp");
    probe_file(&mut tree, "some-film/all-tracks/ffprobe-streams.json", &[("truehd", None, 8, "eng"), ("ac3", None, 2, "eng")]);
    for (label, title, size) in [("all-tracks", 3, 900), ("all-tracks-512", 3, 900), ("share-drop", 5, 300)] {
        // json.dumps written as utf-8-sig: a byte-order mark first
        tree.insert_file(&format!("some-film/{label}/rip-summary.json"),
                         format!("\u{feff}{{\"ok\": true, \"title\": {title}, \"bytes\": {size}}}"));
    }
    let slug = PathKey::new("some-film");
    let lines = levels_args(&tree, &slug, "all-tracks-512").unwrap();
    assert!(lines[0].contains("(from all-tracks/ffprobe-streams.json: a rip of the same title 3 with the same size, 900 bytes)"), "{lines:?}");
    assert!(lines[lines.len() - 2].ends_with("-Label all-tracks-512 -Main 1 -Cand 2"), "{lines:?}");
    let stop = levels_args(&tree, &slug, "share-drop").unwrap_err();
    assert!(stop.contains("scripts/phase0/job.sh probe-title.ps1 probe-share-drop -Slug some-film -Label share-drop -SkipRemux"), "{stop}");
    tree.insert_file("some-film/extract/mkvmerge.json", serde_json::to_string(&json!({"tracks": [
        {"id": 0, "type": "video"},
        {"id": 1, "type": "audio", "codec": "DTS-HD Master Audio", "properties": {"audio_channels": 6, "language": "eng"}},
        {"id": 2, "type": "audio", "codec": "DTS", "properties": {"audio_channels": 6, "language": "eng"}}]})).unwrap());
    assert!(jobs(&tree, "some-film", "extract")[0].ends_with("-Main 1 -Cand 2"));
}

#[test]
fn auto_labels_duplicates_and_derived_cores_and_keeps_existing_labels() {
    let mut tree = MemTree::new(".", "tmp");
    for (path, bytes) in disc_d_files() {
        tree.insert_file(&format!("disc-d/{path}"), bytes);
    }
    tree.insert_file("disc-d/audio-labels.json", serde_json::to_string(&json!({
        "format": FORMAT, "slug": "disc-d", "titles": {},
        "tracks": [with(entry("t0", 3, "duplicate"), "note", json!("the owner's"))]})).unwrap());
    let slug = PathKey::new("disc-d");
    let opened = open_for_change(&tree, &slug).unwrap();
    let fitted = load_fitted(&tree, &PathKey::root(), Some("fit-disc")).unwrap();
    let (lines, write) = auto(&tree, &slug, &opened, "2026-10-05", &fitted).unwrap();
    let data = write.expect("auto labelled three tracks, so the file is written");
    assert_eq!(lines, strings(&[
        "labelled t0 stream 5: duplicate (md5), counts=False (a duplicate track never counts)",
        "labelled t0 stream 6: derived-core (flag), counts=False ('flag' is an observation, not a check of the track's content (row 13, D23))",
        "labelled t0 stream 7: duplicate (md5), counts=False (a duplicate track never counts)",
        "t1: derived cores not labelled: the file has 2 audio streams and the scan's title 1 has 3",
    ]));
    let got: BTreeSet<(i64, String, String)> = data["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| (t["stream"].as_i64().unwrap(), t["kind"].as_str().unwrap().to_string(), t["how"].as_str().unwrap().to_string()))
        .collect();
    let want: BTreeSet<(i64, String, String)> = [
        (3, "duplicate", "listened"), (5, "duplicate", "md5"), (6, "derived-core", "flag"), (7, "duplicate", "md5"),
    ]
    .iter()
    .map(|(n, k, h)| (*n, k.to_string(), h.to_string()))
    .collect();
    assert_eq!(got, want);
}

#[test]
fn a_cached_measure_comes_back_with_its_nan() {
    let (mix, centre) = programme(420, 1);
    let mut copy = vec![-100.0; 3];
    copy.extend(mix[..mix.len() - 3].iter().map(|v| v - 6.0));
    let consts = Constants::default();
    let mut cache: BTreeMap<String, Measures> = BTreeMap::new();
    let mut meas = Measurer::new(&mut cache, &consts, true);
    let first = meas.measure(&mix, &copy, Some(&centre[..]));
    assert!(first.measures.run().expect("a measured pair").smi.is_nan());
    let text = serde_json::to_string(&cache).unwrap();
    assert!(text.contains("\"smi\":null"), "NaN is written as null");
    let mut again_cache: BTreeMap<String, Measures> = serde_json::from_str(&text).unwrap();
    let mut again = Measurer::new(&mut again_cache, &consts, true);
    let second = again.measure(&mix, &copy, Some(&centre[..]));
    assert!(second.measures.run().expect("a measured pair").smi.is_nan());
    assert_eq!((again.computed, again.reused), (0, 1));
    assert_eq!(second.key, first.key);
}

#[test]
fn args_lines_are_the_pythons_line_for_line() {
    let mut tree = MemTree::new(".", "tmp");
    probe_file(&mut tree, "some-film/commentary/ffprobe-streams.json",
               &[("dts", Some("DTS-HD MA"), 6, "spa"), ("dts", Some("DTS"), 6, "spa"), ("ac3", None, 2, "eng")]);
    probe_file(&mut tree, "some-film/dub-first/ffprobe-streams.json",
               &[("dts", Some("DTS-HD MA"), 6, "spa"), ("truehd", None, 8, "eng"), ("ac3", None, 2, "eng"),
                 ("ac3", None, 6, "fra"), ("ac3", None, 2, "fra")]);
    probe_file(&mut tree, "some-film/und-main/ffprobe-streams.json", &[("dts", Some("DTS-HD MA"), 6, "und"), ("ac3", None, 2, "eng")]);
    tree.insert_file("some-film/tmdb.json", r#"{"original_language": "es"}"#);
    let slug = PathKey::new("some-film");
    assert_eq!(levels_args(&tree, &slug, "commentary").unwrap(), strings(COMMENTARY));
    assert_eq!(levels_args(&tree, &slug, "dub-first").unwrap(), strings(DUB_FIRST));
    assert_eq!(levels_args(&tree, &slug, "und-main").unwrap(), strings(UND_MAIN));
    assert_eq!(levels_args(&tree, &slug, "silent").unwrap_err(), concat!(
        "some-film/silent: no ffprobe-streams.json or mkvmerge.json, and no other rip of the same title with the same size has one. Probe the rip first, then run args again:\n",
        "scripts/phase0/job.sh probe-title.ps1 probe-silent -Slug some-film -Label silent -SkipRemux\n",
        "scripts/phase0/wait.sh probe-silent",
    ));
    probe_file(&mut tree, "some-film/no-audio/ffprobe-streams.json", &[]);
    assert_eq!(levels_args(&tree, &slug, "no-audio").unwrap_err(), "some-film/no-audio: the file has no audio stream");
}

#[test]
fn args_never_take_a_golden_folder() {
    // Review Focus 2: spec section 5 reserves golden/. It is never a rip, and never the rip of the same title a
    // probe is borrowed from.
    let mut tree = MemTree::new(".", "tmp");
    for label in ["all-tracks-512", "golden"] {
        tree.insert_file(&format!("some-film/{label}/rip-summary.json"), r#"{"ok": true, "title": 3, "bytes": 900}"#);
    }
    probe_file(&mut tree, "some-film/golden/ffprobe-streams.json", &[("truehd", None, 8, "eng"), ("ac3", None, 2, "eng")]);
    let slug = PathKey::new("some-film");
    let stop = levels_args(&tree, &slug, "all-tracks-512").unwrap_err();
    assert!(stop.contains("scripts/phase0/job.sh probe-title.ps1 probe-all-tracks-512 -Slug some-film -Label all-tracks-512 -SkipRemux"), "{stop}");
    assert_eq!(levels_args(&tree, &slug, "golden").unwrap_err(), "some-film/golden: golden/ holds the goldens of spec section 5, never a rip");
}

#[test]
fn find_folders_skips_golden_and_what_is_not_a_slug() {
    let mut tree = MemTree::new("corpus/owner", "owner");
    for folder in [
        "disc-b/all-tracks/audio-levels-20ms-1-2-3",
        "disc-b/all-tracks/audio-levels-20ms-1-2",
        "disc-b/golden/audio-levels-20ms-1-3",
        "golden/t0/audio-levels-20ms-1-2",
        "Not_A_Slug/t0/audio-levels-20ms-1",
        "disc-a/t1/audio-levels-20ms-4",
    ] {
        tree.insert_file(&format!("{folder}/manifest.txt"), "# lazymkv audio-levels v2 (experiment 13)\n");
    }
    tree.insert_file("disc-b/all-tracks/audio-levels-20ms-9.txt", "a file, not a level folder\n");
    tree.insert_dir("disc-b/notes");
    let found: Vec<(String, String, String)> =
        find_folders(&tree).into_iter().map(|(slug, label, folder)| (slug, label, folder.to_string())).collect();
    let want: Vec<(String, String, String)> = [
        ("disc-a", "t1", "disc-a/t1/audio-levels-20ms-4"),
        ("disc-b", "all-tracks", "disc-b/all-tracks/audio-levels-20ms-1-2"),
        ("disc-b", "all-tracks", "disc-b/all-tracks/audio-levels-20ms-1-2-3"),
    ]
    .iter()
    .map(|(s, l, f)| (s.to_string(), l.to_string(), f.to_string()))
    .collect();
    assert_eq!(found, want);
}

#[test]
fn parse_sets_gives_the_pythons_values_and_messages() {
    let o = sets(&["dc_max=0.25", "SILENT_DB=-80", "tenths_min=7.0", "gap_min=30", "DC_MAX=1", "added_db= 1e1 "]).unwrap();
    // A key given twice keeps its first place and takes the last value, as a Python dict.
    assert_eq!(o.rule, Ordered(vec![("dc_max".to_string(), Num::Float(1.0)), ("tenths_min".to_string(), Num::Int(7))]));
    assert_eq!(o.constants, Ordered(vec![
        ("SILENT_DB".to_string(), Num::Float(-80.0)), ("GAP_MIN".to_string(), Num::Int(30)), ("ADDED_DB".to_string(), Num::Float(10.0)),
    ]));
    let shown: Vec<String> = o.shown().iter().map(|(k, v)| format!("{}={v}", k.to_uppercase())).collect();
    assert_eq!(shown, strings(&["DC_MAX=1.0", "TENTHS_MIN=7", "SILENT_DB=-80.0", "GAP_MIN=30", "ADDED_DB=10.0"]));
    let (rule, consts) = o.effective();
    assert_eq!((rule.dc_max, rule.tenths_min), (1.0, 7));
    assert_eq!((consts.SILENT_DB, consts.GAP_MIN, consts.ADDED_DB), (-80.0, 30, 10.0));
    assert!(!o.is_empty());
    assert!(sets(&[]).unwrap().is_empty());
    for (bad, message) in [
        ("NOPE=1", NOT_A_NAME),
        ("DC_MAX=nan", "--set DC_MAX=nan: not a finite number"),
        ("SILENT_DB=inf", "--set SILENT_DB=inf: not a finite number"),
        ("DC_MAX=-inf", "--set DC_MAX=-inf: not a finite number"),
        ("TENTHS_MIN=1.5", "--set TENTHS_MIN=1.5: tenths_min takes a whole number"),
        ("DC_MAX=x", "--set DC_MAX=x: 'x' is not a number"),
        ("DC_MAX", "--set DC_MAX: write NAME=VALUE"),
        ("gap_min=2.5", "--set gap_min=2.5: GAP_MIN takes a whole number"),
    ] {
        assert_eq!(sets(&[bad]).unwrap_err(), message, "{bad}");
    }
}

#[test]
fn the_cache_key_is_the_spec_formula() {
    // sha1("lazymkv-rule:" + RULE_VERSION + canonical constants JSON + per series its little-endian f64 bytes and
    // "|", "|" alone for none): the values Python computed with json.dumps(sort_keys=True), array("d") and hashlib.
    let prefix = cache_prefix(&Constants::default());
    assert_eq!(prefix, PREFIX_DEFAULT);
    let key = "b5f571f4d286aa5d0eb2381a3197081a4b6d54a4";
    assert_eq!(cache_key(&prefix, &[1.0, -2.5, -100.0], &[0.1], None), key);
    assert_eq!(cache_key(&prefix, &[1.0, -2.5, -100.0], &[0.1], Some(&[])), key); // no curve and an empty one alike
    let mut consts = Constants::default();
    consts.set("SILENT_DB", -80.0).unwrap();
    consts.set("GAP_MIN", 30.0).unwrap();
    assert_eq!(cache_prefix(&consts), PREFIX_SET);
    assert_eq!(cache_key(&cache_prefix(&consts), &[1.0, -2.5, -100.0], &[0.1], None), "330a291b091dbdd012b1d4f101ae0325a51071ce");
}

#[test]
fn a_snapshot_is_read_back_as_v3_or_v2_and_never_with_a_byte_order_mark() {
    let v3 = r#"{"format": "lazymkv audio dataset snapshot v3", "created": "2026-10-13T12:00:00", "corpus": "corpus/owner", "code": {"rule_version": 1}, "tracks": [{"slug": "disc-b", "label": "t0", "folder": "audio-levels-20ms-1-2", "role": "candidate", "stream": 2, "verdict": null, "kind": null, "outcome": "unlabelled"}], "unlabelled_verdicts": {"disc-b/t0/audio-levels-20ms-1-2/candidate/2": "keep"}, "measures_cache": {"abc": {"frames": 10, "error": "too short"}}}"#;
    let s = read_snapshot(v3).expect("a v3 snapshot");
    assert_eq!((s.format.as_str(), s.created.as_str(), s.code_phrase.as_str()), (SNAPSHOT_FORMAT, "2026-10-13T12:00:00", "rule version 1"));
    assert_eq!(s.tracks.len(), 1);
    assert_eq!(s.unlabelled_verdicts.get("disc-b/t0/audio-levels-20ms-1-2/candidate/2"), Some(&json!("keep")));
    assert_eq!(s.measures_cache.len(), 1);
    assert_eq!(s.measures_cache["abc"].error(), Some("too short"));
    let v2 = r#"{"format": "lazymkv audio dataset snapshot v2", "created": "2026-10-13T14:16:49", "code": {"audio_similarity_sha1": "3b6278d937d90000000000000000000000000000"}, "tracks": [], "measures_cache": {"abc": {"frames": 10, "error": "too short"}}}"#;
    let s = read_snapshot(v2).expect("a v2 snapshot");
    assert_eq!((s.format.as_str(), s.code_phrase.as_str()), (SNAPSHOT_FORMAT_V2, "code 3b6278d937d9"));
    assert!(s.measures_cache.is_empty(), "a v2 file is never a cache seed (spec 3.3)");
    assert!(read_snapshot(&format!("\u{feff}{v3}")).is_none(), "read as plain utf-8, as the Python");
    assert!(read_snapshot("not JSON").is_none());
    assert!(read_snapshot(r#"{"format": "lazymkv audio dataset snapshot v1"}"#).is_none());
    assert!(read_snapshot("[1, 2]").is_none());
}

#[test]
fn the_destination_and_the_last_lines() {
    let gone = strings(&["2 labelled track(s) not measured", "1 track(s) of the previous snapshot are gone"]);
    assert_eq!(destination(true, true, &gone), Destination::Given);
    assert_eq!(destination(false, true, &gone), Destination::Trial);
    assert_eq!(destination(false, false, &gone), Destination::Unchecked);
    assert_eq!(destination(false, false, &gone[..1]), Destination::Baseline); // "not measured" alone keeps the baseline
    assert_eq!(destination(false, false, &[]), Destination::Baseline);
    let base = "corpus/owner/audio-dataset.json";
    assert_eq!(why_phrase(Destination::Unchecked, base, &gone),
               " (the baseline corpus/owner/audio-dataset.json was NOT replaced: 1 track(s) of the previous snapshot are gone; to replace it on purpose, run again with --snapshot corpus/owner/audio-dataset.json)");
    assert_eq!(why_phrase(Destination::Trial, base, &gone), " (a trial: the baseline is not replaced)");
    assert_eq!(why_phrase(Destination::Given, base, &gone), "");
    assert_eq!(why_phrase(Destination::Baseline, base, &[]), "");
    assert_eq!(snapshot_line(base, "", true), "snapshot: corpus/owner/audio-dataset.json");
    assert_eq!(snapshot_line("t.json", "", false), "snapshot: t.json (unchanged but for its time: not rewritten)");
    let fails = strings(&["1 programme track(s) got a drop verdict or the commentary pattern: disc-b/t0 stream 3"]);
    let unread = strings(&["1 folder(s) cannot be read", "2 labelled track(s) not measured"]);
    assert_eq!(result_line(&fails, &unread, true), (
        "RESULT FAILED: 1 programme track(s) got a drop verdict or the commentary pattern: disc-b/t0 stream 3; not checked: 1 folder(s) cannot be read, 2 labelled track(s) not measured".to_string(), 1));
    assert_eq!(result_line(&[], &unread, false), (
        "RESULT FAILED: not everything was checked: 1 folder(s) cannot be read, 2 labelled track(s) not measured (see 'Folders that cannot be read', 'Labels without a measurement' and the changes above)".to_string(), 3));
    assert_eq!(result_line(&[], &unread, true), (
        "RESULT ok (not checked, allowed by --allow-unmeasured: 1 folder(s) cannot be read, 2 labelled track(s) not measured)".to_string(), 0));
    assert_eq!(result_line(&[], &[], false), ("RESULT ok".to_string(), 0));
}
