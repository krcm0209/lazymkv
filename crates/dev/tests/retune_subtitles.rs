//! `lazymkv-dev retune --subtitles` (RP15; spec 15 "Rules against the stored evidence"; section 17 row 14; 8.7 "Text";
//! phase 1 Task 18): row 14 of the design spec read by `exp14_requirements`; the command over a synthetic evidence
//! folder and OCR text written to a scratch folder (always run); and the subtitle harness over the evidence clone (each
//! prints "skipped: <reason>" and passes when an input is absent): the clone as it is, the clone's rips with the labels
//! of row 14's result column and no OCR text, and the same with the OCR text the goldens were written from. Inputs:
//! LAZYMKV_EVIDENCE, else corpus/owner under the repository root; LAZYMKV_OCR_TEXT, laid out as
//! <DIR>/<slug>/<label>/subs-ocr/sub-<n>.ocr.jsonl.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use lazymkv_core::disc::disc;
use lazymkv_core::recording::files::{rip_label, MkvmergeIdent, SubsIndexEntry};
use lazymkv_core::recording::{read_json, read_text, Encoding, Errors, MemTree, PathKey, Tree};
use lazymkv_core::robot::parse;
use lazymkv_core::settings::Settings;
use lazymkv_core::subtitle::labels::{exp14_requirements, harness, Row14, SubtitleHarness, FORMAT};
use lazymkv_core::testsupport::{scan_text, sha1_hex, write, Scratch};
use lazymkv_core::verify::{selected_streams, subs_streams, subtitle_events, verify_rip};
use lazymkv_dev::cmd::retune::run;
use lazymkv_dev::evidence::{skip, Ctx, DiskTree, EVIDENCE_SHOWN};

/// The design spec, relative to this crate.
const DESIGN_SPEC: &str = "../../docs/superpowers/specs/2026-10-04-lazymkv-design.md";

/// retune's usage line, as every argument error prints it.
const USAGE_LINE: &str = "usage: lazymkv-dev retune [--evidence DIR] [--set NAME=VALUE] [--snapshot FILE] [--compare FILE] [--spec FILE] [--allow-unmeasured] [--subtitles] [--text DIR]";

/// Made-up English lines for the OCR text (no line of any real subtitle), as in core's harness tests: alone in its
/// title, this dialogue is placed as regular by the text rule without a review.
const LINES: [&str; 6] = [
    "The cat and the dog went to the shop.",
    "You know that is not what I said.",
    "We have to go back to the house now.",
    "It was the best day of the year.",
    "He said that you would be there.",
    "What do you want me to do with this.",
];

/// MakeMKV's log of the synthetic rip: one made-up message, no MSG:4047 removal.
const RIP: &str = "MSG:5011,0,0,\"made-up line\",\"made-up line\"\n";

/// The synthetic disc's title and streams: 2 chapters, 10 minutes, a playlist; AVC video and one English PGS stream.
const BD_TITLE: &[(u32, &str)] = &[(8, "2"), (9, "0:10:00"), (16, "00001.mpls")];
const AVC: &[(u32, &str)] = &[(1, "Video"), (5, "V_MPEG4/ISO/AVC"), (6, "Mpeg4"), (7, "Mpeg4 AVC High@L4.1")];
const ENG_PGS: &[(u32, &str)] = &[(1, "Subtitles"), (3, "eng"), (5, "S_HDMV/PGS"), (6, "PGS"), (7, "HDMV PGS Subtitles")];

/// One labelled track of the clone's rips: the first English subs/index.json entry with this mkvmerge codec name, the
/// kind row 14's result column gives it, and the words the report gives it: 8.7's role, and the text rule's role once
/// the goldens' OCR text is read.
struct Labelled {
    codec: &'static str,
    kind: &'static str,
    rule: &'static str,
    text: &'static str,
}

/// mkvmerge's codec name of the closed captions MakeMKV converts to text (spec 6.2 step 7): a text stream, never read
/// by OCR.
const CAPTIONS_CODEC: &str = "SubRip/SRT";

/// The Office's SDH track: 8.7 keeps it as regular (one full English track), the text rule reads it as sdh.
const OFFICE_SDH: &[Labelled] = &[Labelled { codec: "HDMV PGS", kind: "sdh", rule: "regular", text: "sdh" }];

/// The clone's rips labelled here, with row 14's result column: (slug, rip label, labelled tracks). Each track's file
/// track, scan stream and event count are derived when the test runs (`found`), never written here (RP21).
const OVERLAY: [(&str, &str, &[Labelled]); 5] = [
    (
        "pans-labyrinth-2006-uhd",
        "eng-only",
        &[Labelled { codec: "HDMV PGS", kind: "translation", rule: "regular", text: "regular" }],
    ),
    (
        "short-circuit-2-1988-dvd",
        "all-tracks-2",
        &[
            Labelled { codec: CAPTIONS_CODEC, kind: "closed-captions", rule: "sdh", text: "set aside" },
            Labelled { codec: "VobSub", kind: "regular", rule: "regular", text: "regular" },
        ],
    ),
    ("the-office-2005-s3d3-bluray", "e16-stereo", OFFICE_SDH),
    ("the-office-2005-s3d3-bluray", "rehearsal", OFFICE_SDH),
    ("the-office-2005-s3d3-bluray", "t3-all-tracks", OFFICE_SDH),
];

/// The report of the synthetic evidence below with its OCR text (core's runs::the_report_counts_each_kind... gives the
/// same layout for three recordings).
const REPORT: [&str; 35] = [
    "subtitle labels: corpus/owner: 1 recordings with subtitle-labels.json, 1 labelled tracks: 1 judged, 0 not judged, 0 in another language",
    "rule: 8.7 with sub_clear_max_bytes 64, subs_tolerance_ms 300, subs_same_min 0.7, subs_diff_max 0.3; the text rule (EXP-14-design.md section 3) with the same four and its other thresholds as in the code, all provisional",
    "",
    "Tracks (the owner's label; 8.7 as the app decides before experiment 14 answers; the text rule as row 14 (a) judges it):",
    "  disc-a/eng-only stream 1 (scan stream 1, 40 events): labelled sdh (read); 8.7 regular; text regular; COSTLY sdh_as_regular",
    "",
    "Experiment 14 (spec section 17 row 14 (a); tracks that count: judged here, labelled from text read or watched before the verdict, the owner sure, never on the-office-2005-s3d3-bluray or pans-labyrinth-2006-uhd; the packaging, the menu, a flag, the player and TheDiscDB are observations and do not count on their own, D23):",
    "  regular               0 of 6",
    "  sdh                   1 of 6",
    "  forced                0, with events 0 of 1",
    "  commentary            0 of 1",
    "  trivia                0 (every one the owner has; checked by hand)",
    "  lyrics                0 (every one the owner has; checked by hand)",
    "  translation           0 (every second translation the owner has; checked by hand)",
    "  dub-transcript        0 (every one the owner has; checked by hand)",
    "  closed-captions       0 (with its DVD's VobSub tracks)",
    "  other                 0",
    "  regular and SDH       0 of 3 titles, from 0 of 2 discs; feature films 0 of 1, TV episodes 0 of 1",
    "  DVD VobSub            0 of 1 DVDs with a counted English VobSub track",
    "  UK-style SDH          0 of 1 counted SDH tracks labelled style uk (only if the owner has a UK disc)",
    "  placed, no review     1 of 1 judged tracks (row 14 (a) asks for at least half)",
    "  costly verdicts       1",
    "  as it stands: no: 1 costly verdict(s)",
    "  not counted: none",
    "",
    "Costly verdicts (row 14 (a): the text rule's verdict, or 8.7's for a stream the text rule sets aside):",
    "  COSTLY disc-a/eng-only stream 1: labelled sdh, an SDH track placed as regular without review",
    "",
    "Labels that cannot be judged:",
    "  none",
    "",
    "Labels in another language (row 14 judges English and no-language tracks only):",
    "  none",
    "",
    "RESULT FAILED: 1 costly verdict(s): disc-a/eng-only stream 1 (sdh_as_regular)",
];

/// The report as the command prints it: every line of REPORT ending in a newline.
fn printed_report() -> String {
    REPORT.iter().map(|line| format!("{line}\n")).collect()
}

/// The design spec's path.
fn spec_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(DESIGN_SPEC)
}

/// Writes `<root>/corpus/disc-a`, a film with one English PGS stream (scan stream 1, file track 1) of 40 events labelled
/// sdh from its read text before the verdict, with no style (RP25), and its OCR text under `<root>/text/disc-a/eng-only/subs-ocr/`.
fn write_disc_a(root: &Path) {
    let rec = root.join("corpus/disc-a");
    write(&rec.join("scan.txt"), scan_text(&[(1, "Blu-ray disc")], &[(0, BD_TITLE, &[(0, AVC), (1, ENG_PGS)])]));
    write(&rec.join("tmdb.json"), json!({"title": "Made Up Film", "original_language": "en"}).to_string());
    let labels = json!({"format": FORMAT, "slug": "disc-a", "tracks": [
        {"label": "eng-only", "stream": 1, "kind": "sdh", "how": "read", "sure": true, "date": "2026-10-16",
         "saw_verdict": false, "note": null, "style": null}
    ]});
    write(&rec.join("subtitle-labels.json"), labels.to_string());
    let label = rec.join("eng-only");
    write(&label.join("rip-summary.json"), json!({"ok": true, "title": 0, "profile": "C:\\made-up\\eng-only.mmcp.xml"}).to_string());
    let ident = json!({
        "tracks": [
            {"id": 0, "type": "video", "codec": "AVC/H.264/MPEG-4p10", "properties": {}},
            {"id": 1, "type": "subtitles", "codec": "HDMV PGS", "properties": {"language": "eng"}}
        ],
        "chapters": [{"num_entries": 2}],
        "container": {"properties": {"duration": 600_000_000_000_i64}}
    });
    write(&label.join("mkvmerge.json"), ident.to_string());
    write(&label.join("rip.txt"), RIP);
    let index = json!([{"n": 0, "track_id": 1, "language": "eng", "codec": "HDMV PGS", "track_name": null,
                        "forced": false, "default": false}]);
    write(&label.join("subs/index.json"), index.to_string());
    let mut packets = Vec::new();
    let mut ocr = String::new();
    for k in 0..40_usize {
        let t = 10.0 + 3.0 * k as f64;
        packets.push(json!({"pts_time": format!("{t:.6}"), "duration_time": "2.000000", "size": "3000"}));
        packets.push(json!({"pts_time": format!("{:.6}", t + 2.0), "duration_time": "N/A", "size": "30"}));
        ocr.push_str(&json!({"frame": k, "t": t, "pts": t, "lines": [LINES[k % 6]]}).to_string());
        ocr.push('\n');
    }
    write(&label.join("subs/sub-0.packets.json"), json!({"packets": packets}).to_string());
    write(&root.join("text/disc-a/eng-only/subs-ocr/sub-0.ocr.jsonl"), ocr);
}

/// `retune` with cwd `root` and the environment `env`: (exit code, stdout, stderr).
fn call(root: &Path, args: &[&str], env: &[(&str, String)]) -> (i32, String, String) {
    let mut ctx = Ctx::for_test(root);
    for (name, value) in env {
        ctx.env.insert(name.to_string(), value.clone());
    }
    let args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
    let (mut out, mut err): (Vec<u8>, Vec<u8>) = (Vec::new(), Vec::new());
    let code = run(&ctx, &args, &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

/// A scratch folder with disc-a written, and its corpus, text and spec paths as the command takes them.
fn scratch(tag: &str) -> (Scratch, String, String, String) {
    let tmp = Scratch::new(tag);
    write_disc_a(&tmp.0);
    let corpus = tmp.0.join("corpus").display().to_string();
    let text = tmp.0.join("text").display().to_string();
    (tmp, corpus, text, spec_path().display().to_string())
}

/// The evidence clone: LAZYMKV_EVIDENCE, else corpus/owner under the repository root; None, with the skip line
/// written, when it is absent.
fn evidence() -> Option<PathBuf> {
    let dir = match std::env::var("LAZYMKV_EVIDENCE") {
        Ok(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/owner"),
    };
    if !dir.is_dir() {
        skip(&format!("no evidence clone at {}", dir.display()));
        return None;
    }
    Some(dir)
}

/// Row 14 of the design spec.
fn spec_row14() -> Row14 {
    exp14_requirements(&fs::read_to_string(spec_path()).expect("the design spec is in the repository")).expect("row 14 reads")
}

/// Copies `<root>/<rel>` into `tree` at `rel`; false when it cannot be read.
fn copy(tree: &mut MemTree, root: &Path, rel: &str) -> bool {
    match fs::read(root.join(rel)) {
        Ok(bytes) => {
            tree.insert_file(rel, bytes);
            true
        }
        Err(_) => false,
    }
}

/// Copies every `sub-*<suffix>` file of `<root>/<folder>`, in name order.
fn copy_all(tree: &mut MemTree, root: &Path, folder: &str, suffix: &str) {
    let Ok(entries) = fs::read_dir(root.join(folder)) else { return };
    let mut names: Vec<String> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("sub-") && name.ends_with(suffix))
        .collect();
    names.sort();
    for name in names {
        copy(tree, root, &format!("{folder}/{name}"));
    }
}

/// The labelled track `labelled` names in a rip's subs/index.json: the first English entry with its codec name.
fn english<'i>(index: &'i [SubsIndexEntry], labelled: &Labelled, at: &str) -> &'i SubsIndexEntry {
    index
        .iter()
        .find(|entry| entry.language.as_deref() == Some("eng") && entry.codec.as_deref() == Some(labelled.codec))
        .unwrap_or_else(|| panic!("{at}: no English {} stream in subs/index.json", labelled.codec))
}

/// The subs/index.json of `<slug>/<label>` in `tree`.
fn index_of(tree: &MemTree, slug: &str, label: &str) -> Vec<SubsIndexEntry> {
    read_json(tree, &PathKey::new(&format!("{slug}/{label}/subs/index.json"))).expect("subs/index.json reads")
}

/// The clone's OVERLAY rips in memory, shown as corpus/owner: for each recording scan.txt and tmdb.json, for each rip
/// label the files the harness reads (rip-summary.json, mkvmerge.json, rip.txt, subs/index.json, the packets and the
/// OCR frame counts), and a subtitle-labels.json with OVERLAY's labels on the file tracks their subs/index.json entries
/// name, each read and sure before the verdict, with no style (RP25). None, with the skip line written, when a file is
/// absent.
fn overlay(root: &Path) -> Option<MemTree> {
    let mut tree = MemTree::new(EVIDENCE_SHOWN, "owner");
    for (slug, label, _) in OVERLAY {
        let needed = [
            format!("{slug}/scan.txt"),
            format!("{slug}/{label}/rip-summary.json"),
            format!("{slug}/{label}/mkvmerge.json"),
            format!("{slug}/{label}/rip.txt"),
            format!("{slug}/{label}/subs/index.json"),
        ];
        for rel in &needed {
            if !copy(&mut tree, root, rel) {
                skip(&format!("no {} (the evidence clone lacks this recording's file)", root.join(rel).display()));
                return None;
            }
        }
        copy(&mut tree, root, &format!("{slug}/tmdb.json"));
        copy_all(&mut tree, root, &format!("{slug}/{label}/subs"), ".packets.json");
        copy_all(&mut tree, root, &format!("{slug}/{label}/subs-ocr"), ".frames.json");
    }
    let mut labels: Vec<(&str, Vec<Value>)> = Vec::new();
    for (slug, label, tracks) in OVERLAY {
        let index = index_of(&tree, slug, label);
        let entries: Vec<Value> = tracks
            .iter()
            .map(|labelled| {
                let at = format!("{slug}/{label}");
                let track = english(&index, labelled, &at).track_id.unwrap_or_else(|| panic!("{at}: no track_id"));
                json!({"label": label, "stream": track, "kind": labelled.kind, "how": "read", "sure": true,
                       "date": "2026-10-16", "saw_verdict": false, "note": null, "style": null})
            })
            .collect();
        match labels.iter().position(|(known, _)| *known == slug) {
            Some(at) => labels[at].1.extend(entries),
            None => labels.push((slug, entries)),
        }
    }
    for (slug, tracks) in labels {
        let file = json!({"format": FORMAT, "slug": slug, "tracks": tracks});
        tree.insert_file(&format!("{slug}/subtitle-labels.json"), file.to_string());
    }
    Some(tree)
}

/// One OVERLAY track as the harness reports it, with what `found` derived for it.
struct Found {
    slug: &'static str,
    label: &'static str,
    labelled: &'static Labelled,
    /// subs/index.json's `n`, the file track (its `track_id`), the scan stream and the event count.
    n: i64,
    track: i64,
    stream: u32,
    events: usize,
}

impl Found {
    /// "<slug>/<rip label> stream <file track>", the report's key.
    fn key(&self) -> String {
        format!("{}/{} stream {}", self.slug, self.label, self.track)
    }

    /// The Tracks section's line of this track, with `tail` after its label.
    fn line(&self, tail: &str) -> String {
        format!(
            "  {} (scan stream {}, {} events): labelled {} (read); {tail}",
            self.key(),
            self.stream,
            self.events,
            self.labelled.kind
        )
    }
}

/// Every OVERLAY track in the report's order (slug, rip label, file track), each with its file track, scan stream and
/// event count derived from the copied files the harness reads: the rip verified with `verify_rip` over
/// `selected_streams` (with its rip.txt), subs/index.json's `n` mapped through `subs_streams`, and the events of
/// `subtitle_events` with spec 14's `sub_clear_max_bytes`. None of these values is copied from the clone into this file
/// (RP21).
fn found(tree: &MemTree) -> Vec<Found> {
    let mut all = Vec::new();
    for (slug, label, tracks) in OVERLAY {
        let at = format!("{slug}/{label}");
        let slug_dir = PathKey::new(slug);
        let rip = rip_label(tree, &slug_dir, label).expect("the rip label reads");
        let log = read_text(tree, &slug_dir.join("scan.txt"), Encoding::Utf8Sig, Errors::Replace).expect("scan.txt reads");
        let scan = parse(&log);
        let typed = disc(&scan).expect("the scan gives a typed disc");
        let number = u32::try_from(rip.title).expect("the summary names a title");
        let title = typed.title(number).expect("the scan lists the rip's title");
        let selection = selected_streams(&scan, number, rip.selection).expect("the selection reads");
        let folder = slug_dir.join(label);
        let ident: MkvmergeIdent = read_json(tree, &folder.join("mkvmerge.json")).expect("mkvmerge.json reads");
        let rip_log = read_text(tree, &folder.join("rip.txt"), Encoding::Utf8Sig, Errors::Replace).expect("rip.txt reads");
        let report = verify_rip(title, &ident, &selection, Some(&rip_log), typed.is_dvd);
        assert!(report.ok, "{at}: the rip does not verify: {:?}", report.mismatches);
        let index = index_of(tree, slug, label);
        let streams = subs_streams(&index, &report);
        let events = subtitle_events(tree, &folder.join("subs"), &report, Settings::default().sub_clear_max_bytes)
            .expect("the packets read");
        for labelled in tracks {
            let entry = english(&index, labelled, &at);
            let stream = *streams.get(&entry.n).unwrap_or_else(|| panic!("{at}: n {} maps to no scan stream", entry.n));
            all.push(Found {
                slug,
                label,
                labelled,
                n: entry.n,
                track: entry.track_id.unwrap_or_else(|| panic!("{at}: n {} has no track_id", entry.n)),
                stream,
                events: events.get(&stream).map_or(0, Vec::len),
            });
        }
    }
    all.sort_by(|a, b| (a.slug, a.label, a.track).cmp(&(b.slug, b.label, b.track)));
    all
}

/// LAZYMKV_OCR_TEXT when it holds the OCR text of every labelled English bitmap stream of `tree`'s OVERLAY rips (the
/// closed captions are a text stream, never read by OCR) with the SHA-1 its golden subs.json names; None, with the skip
/// line written, otherwise.
fn golden_text(root: &Path, tree: &MemTree) -> Option<PathBuf> {
    let Some(dir) = std::env::var("LAZYMKV_OCR_TEXT").ok().filter(|dir| !dir.is_empty()).map(PathBuf::from) else {
        skip("no OCR text (LAZYMKV_OCR_TEXT is not set)");
        return None;
    };
    for track in found(tree).iter().filter(|track| track.labelled.codec != CAPTIONS_CODEC) {
        let name = format!("sub-{}.ocr.jsonl", track.n);
        let path = dir.join(track.slug).join(track.label).join("subs-ocr").join(&name);
        let Ok(bytes) = fs::read(&path) else {
            skip(&format!("no OCR text at {}", path.display()));
            return None;
        };
        let golden_path = root.join(track.slug).join("golden").join(track.label).join("subs.json");
        let golden: Option<Value> = fs::read_to_string(&golden_path).ok().and_then(|text| serde_json::from_str(&text).ok());
        let Some(golden) = golden else {
            skip(&format!("no golden {}", golden_path.display()));
            return None;
        };
        if golden["text_sha1"][name.as_str()].as_str() != Some(sha1_hex(&bytes).as_str()) {
            skip(&format!("the OCR text {} is not the one the goldens were written from", path.display()));
            return None;
        }
    }
    Some(dir)
}

/// The lines of the section whose header starts with `header`, up to the next empty line.
fn section<'h>(h: &'h SubtitleHarness, header: &str) -> Vec<&'h str> {
    let start = h
        .lines
        .iter()
        .position(|line| line.starts_with(header))
        .unwrap_or_else(|| panic!("no section {header} in {:#?}", h.lines));
    h.lines[start + 1..].iter().take_while(|line| !line.is_empty()).map(String::as_str).collect()
}

/// The one line starting with `start`.
fn line<'h>(h: &'h SubtitleHarness, start: &str) -> &'h str {
    h.lines
        .iter()
        .find(|line| line.starts_with(start))
        .map(String::as_str)
        .unwrap_or_else(|| panic!("no line {start} in {:#?}", h.lines))
}

#[test]
fn the_design_specs_row_14_gives_its_counts_and_the_two_fitting_discs() {
    assert_eq!(
        spec_row14(),
        Row14 {
            pair_titles: 3,
            pair_discs: 2,
            pair_features: 1,
            pair_tv: 1,
            sdh: 6,
            regular: 6,
            forced: 1,
            commentary: 1,
            dvd_vobsub: 1,
            uk_sdh: 1,
            fitting_discs: vec!["the-office-2005-s3d3-bluray".to_string(), "pans-labyrinth-2006-uhd".to_string()],
        }
    );
}

#[test]
fn retune_subtitles_prints_the_report_and_exits_1_on_a_costly_verdict() {
    let (tmp, corpus, text, spec) = scratch("retune-subs-report");
    let args = ["--subtitles", "--evidence", corpus.as_str(), "--spec", spec.as_str(), "--text", text.as_str()];
    assert_eq!(call(&tmp.0, &args, &[]), (1, printed_report(), String::new()));
}

#[test]
fn without_ocr_text_retune_subtitles_exits_3_and_says_how_to_give_it() {
    let (tmp, corpus, _, spec) = scratch("retune-subs-no-text");
    let (code, out, err) = call(&tmp.0, &["--subtitles", "--evidence", corpus.as_str(), "--spec", spec.as_str()], &[]);
    assert_eq!((code, err.as_str()), (3, ""));
    let lines: Vec<&str> = out.lines().collect();
    assert!(lines.contains(&"  disc-a/eng-only stream 1: no OCR text (give --text DIR or LAZYMKV_OCR_TEXT)"), "{out}");
    assert!(
        lines.contains(
            &"  disc-a/eng-only stream 1 (scan stream 1, 40 events): labelled sdh (read); 8.7 regular; text not read; not judged"
        ),
        "{out}"
    );
    assert_eq!(
        lines.last().copied(),
        Some("RESULT FAILED: not everything was judged: 1 labelled track(s) (see Labels that cannot be judged)")
    );
}

#[test]
fn the_ocr_text_may_come_from_lazymkv_ocr_text_and_must_exist() {
    let (tmp, corpus, text, spec) = scratch("retune-subs-env");
    let args = ["--subtitles", "--evidence", corpus.as_str(), "--spec", spec.as_str()];
    assert_eq!(call(&tmp.0, &args, &[("LAZYMKV_OCR_TEXT", text)]), (1, printed_report(), String::new()));
    let nope = tmp.0.join("nope").display().to_string();
    let args = ["--subtitles", "--evidence", corpus.as_str(), "--spec", spec.as_str(), "--text", nope.as_str()];
    assert_eq!(
        call(&tmp.0, &args, &[]),
        (2, String::new(), format!("the OCR text folder {nope} does not exist (give --text DIR or LAZYMKV_OCR_TEXT)\n"))
    );
}

#[test]
fn subtitles_takes_no_audio_flag_and_text_needs_subtitles() {
    let (tmp, corpus, text, spec) = scratch("retune-subs-args");
    let refused = format!(
        "{USAGE_LINE}\nlazymkv-dev retune: error: argument --subtitles: not allowed with --set, --snapshot, --compare or --allow-unmeasured\n"
    );
    let audio_flags: [&[&str]; 4] =
        [&["--set", "DC_MAX=0.2"], &["--snapshot", "trial.json"], &["--compare", "old.json"], &["--allow-unmeasured"]];
    for extra in audio_flags {
        let mut args = vec!["--subtitles", "--evidence", corpus.as_str(), "--spec", spec.as_str()];
        args.extend_from_slice(extra);
        assert_eq!(call(&tmp.0, &args, &[]), (2, String::new(), refused.clone()), "{extra:?}");
    }
    let args = ["--evidence", corpus.as_str(), "--spec", spec.as_str(), "--text", text.as_str()];
    assert_eq!(
        call(&tmp.0, &args, &[]),
        (2, String::new(), format!("{USAGE_LINE}\nlazymkv-dev retune: error: argument --text: only with --subtitles\n"))
    );
}

#[test]
fn a_missing_evidence_clone_exits_3_and_a_spec_without_row_14_exits_2() {
    let (tmp, corpus, _, spec) = scratch("retune-subs-refused");
    let absent = tmp.0.join("absent").display().to_string();
    assert_eq!(
        call(&tmp.0, &["--subtitles", "--evidence", absent.as_str(), "--spec", spec.as_str()], &[]),
        (
            3,
            String::new(),
            format!(
                "{absent}: no such folder; the evidence repository lazymkv-evidence is not cloned there (git clone https://github.com/krcm0209/lazymkv-evidence {absent})\n"
            )
        )
    );
    write(&tmp.0.join("bad.md"), "# spec\n| 13 | a row |\n");
    let bad = tmp.0.join("bad.md").display().to_string();
    assert_eq!(
        call(&tmp.0, &["--subtitles", "--evidence", corpus.as_str(), "--spec", bad.as_str()], &[]),
        (2, String::new(), "spec section 17 has no row starting '| 14 |'\n".to_string())
    );
}

#[test]
fn the_retune_row_of_the_command_table_names_subtitles_and_text() {
    let row = lazymkv_dev::COMMANDS.iter().find(|(name, _, _)| *name == "retune").map(|(_, _, synopsis)| *synopsis);
    assert_eq!(
        row,
        Some("[--evidence DIR] [--set NAME=VALUE ...] [--snapshot FILE] [--compare FILE] [--spec FILE] [--allow-unmeasured] [--subtitles] [--text DIR]")
    );
}

#[test]
fn the_clone_without_subtitle_labels_has_nothing_to_judge() {
    let Some(root) = evidence() else { return };
    let mut labelled: Vec<String> = fs::read_dir(&root)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().join("subtitle-labels.json").is_file())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    labelled.sort();
    if !labelled.is_empty() {
        skip(&format!(
            "not covered: the clone has subtitle labels now ({}); give them a test in crates/dev/tests/retune_subtitles.rs",
            labelled.join(", ")
        ));
        return;
    }
    let tree = DiskTree::open(&root, EVIDENCE_SHOWN).unwrap();
    let h = harness(&tree, None, &spec_row14(), &Settings::default());
    assert_eq!(
        h.lines[0],
        "subtitle labels: corpus/owner: 0 recordings with subtitle-labels.json, 0 labelled tracks: 0 judged, 0 not judged, 0 in another language"
    );
    assert_eq!(h.lines.last().map(String::as_str), Some("RESULT FAILED: no subtitle label to judge (lazymkv-dev labels --subtitles set)"));
    assert_eq!((h.labelled, h.costly.len(), h.exit_code), (0, 0, 3));
}

#[test]
fn the_clones_rips_with_row_14s_labels_are_decided_by_8_7_and_wait_for_their_ocr_text() {
    let Some(root) = evidence() else { return };
    let Some(tree) = overlay(&root) else { return };
    let tracks = found(&tree);
    let h = harness(&tree, None, &spec_row14(), &Settings::default());
    let expected: Vec<String> =
        tracks.iter().map(|track| track.line(&format!("8.7 {}; text not read; not judged", track.labelled.rule))).collect();
    assert_eq!(section(&h, "Tracks ("), expected);
    let no_text = "no OCR text (give --text DIR or LAZYMKV_OCR_TEXT)";
    let unjudged: Vec<String> = tracks.iter().map(|track| format!("  {}: {no_text}", track.key())).collect();
    assert_eq!(section(&h, "Labels that cannot be judged:"), unjudged);
    assert_eq!(line(&h, "  not counted: "), "  not counted: pans-labyrinth-2006-uhd 1, short-circuit-2-1988-dvd 2, the-office-2005-s3d3-bluray 3");
    assert_eq!(
        h.lines.last().map(String::as_str),
        Some("RESULT FAILED: not everything was judged: 6 labelled track(s) (see Labels that cannot be judged)")
    );
    assert_eq!((h.labelled, h.costly.len(), h.exit_code), (6, 0, 3));
}

#[test]
fn with_the_goldens_ocr_text_the_clones_labelled_tracks_get_no_costly_verdict() {
    let Some(root) = evidence() else { return };
    let Some(tree) = overlay(&root) else { return };
    let Some(text_dir) = golden_text(&root, &tree) else { return };
    let text = DiskTree::open(&text_dir, &text_dir.display().to_string()).unwrap();
    let h = harness(&tree, Some(&text as &dyn Tree), &spec_row14(), &Settings::default());
    let expected: Vec<String> = found(&tree)
        .iter()
        .map(|track| track.line(&format!("8.7 {}; text {}; placed", track.labelled.rule, track.labelled.text)))
        .collect();
    assert_eq!(section(&h, "Tracks ("), expected);
    assert_eq!(line(&h, "  regular "), "  regular               1 of 6");
    assert_eq!(line(&h, "  closed-captions "), "  closed-captions       1 (with its DVD's VobSub tracks)");
    assert_eq!(line(&h, "  DVD VobSub "), "  DVD VobSub            1 of 1 DVDs with a counted English VobSub track");
    assert_eq!(line(&h, "  placed, no review "), "  placed, no review     6 of 6 judged tracks (row 14 (a) asks for at least half)");
    assert_eq!(
        line(&h, "  as it stands: "),
        "  as it stands: not answered: too few titles with regular and SDH, discs with such titles, feature films with such a title, TV episodes with such a title, SDH tracks, regular tracks, forced tracks with events, commentary tracks"
    );
    assert_eq!(line(&h, "  not counted: "), "  not counted: pans-labyrinth-2006-uhd 1, the-office-2005-s3d3-bluray 3");
    assert_eq!(section(&h, "Costly verdicts ("), ["  none"]);
    assert_eq!(h.lines.last().map(String::as_str), Some("RESULT ok"));
    assert_eq!((h.labelled, h.placed_without_review, h.costly.len(), h.exit_code), (6, 6, 0, 0));
}
