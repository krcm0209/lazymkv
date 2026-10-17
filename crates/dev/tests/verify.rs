//! Verification of a rip against its scan (spec 6.2 step 7, 9.4 messages 2003 and 4047; `lazymkv_core::verify`, phase 1
//! Task 12) over every rip label of the evidence clone: each label's `mkvmerge.json` against its recording's `scan.txt`
//! through the label's selection and `rip.txt`, the read errors of the DVD rip, the selection for `all` and `eng`, and
//! the subtitle streams of `subs/index.json` mapped onto scan streams with their event counts at the default
//! `sub_clear_max_bytes` and at 0. Each recording has a test with the values read from its files on 2026-10-16; a
//! recording added later is listed by recordings_without_a_test_are_listed_as_not_covered, and a rip label added later
//! by every_rip_label_of_the_clone_verifies, until they get their tests here. Without the clone, or without one
//! recording, a test prints "skipped: <reason>" and passes. A drive's name or serial (MSG:2003's second parameter, the
//! DRV lines) is never printed. Input: LAZYMKV_EVIDENCE, else corpus/owner under the repository root.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use lazymkv_core::disc::{disc, Title};
use lazymkv_core::recording::files::{rip_label, rip_labels, MkvmergeIdent, Selection, SubsIndexEntry};
use lazymkv_core::recording::{read_json, read_text, Encoding, Errors, PathKey, Tree};
use lazymkv_core::reconcile::RIP_LOG_NAMES;
use lazymkv_core::review::ReviewReason;
use lazymkv_core::robot::{parse, Scan};
use lazymkv_core::settings::Settings;
use lazymkv_core::subtitle::align::{events, is_text, load_packets};
use lazymkv_core::verify::{
    duration_ok, failure_reviews, read_errors, selected_streams, subs_streams, subtitle_events, verify_rip, ReadError,
    VerifyReport,
};
use lazymkv_dev::evidence::{skip, DiskTree};

/// The recordings with a test here.
const RECORDINGS: [&str; 4] =
    ["pans-labyrinth-2006-uhd", "short-circuit-1986-dvd", "short-circuit-2-1988-dvd", "the-office-2005-s3d3-bluray"];

/// The rip labels with a test here, as `<slug>/<label>`.
const RIP_LABELS: [&str; 9] = [
    "pans-labyrinth-2006-uhd/all-tracks",
    "pans-labyrinth-2006-uhd/eng-only",
    "pans-labyrinth-2006-uhd/rehearsal",
    "short-circuit-1986-dvd/all-tracks",
    "short-circuit-2-1988-dvd/all-tracks-2",
    "short-circuit-2-1988-dvd/eng-only",
    "the-office-2005-s3d3-bluray/e16-stereo",
    "the-office-2005-s3d3-bluray/rehearsal",
    "the-office-2005-s3d3-bluray/t3-all-tracks",
];

/// The read error short-circuit-2-1988-dvd's all-tracks-2 rip logged three times (spec 6.2 step 7).
const SCRAMBLED: &str = "Scsi error - ILLEGAL REQUEST:READ OF SCRAMBLED SECTOR WITHOUT AUTHENTICATION";

/// The evidence clone: LAZYMKV_EVIDENCE, else corpus/owner under the repository root.
fn evidence() -> PathBuf {
    match std::env::var("LAZYMKV_EVIDENCE") {
        Ok(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/owner"),
    }
}

/// The slugs of the clone's recordings that have a scan.txt (never the golden folder), sorted; None, with the skip line
/// written, when the clone is absent.
fn recorded_slugs() -> Option<Vec<String>> {
    let root = evidence();
    if !root.is_dir() {
        skip(&format!("no evidence clone at {}", root.display()));
        return None;
    }
    let mut slugs = Vec::new();
    for entry in std::fs::read_dir(&root).unwrap() {
        let dir = entry.unwrap().path();
        let name = dir.file_name().unwrap().to_string_lossy().into_owned();
        if name != "golden" && dir.join("scan.txt").is_file() {
            slugs.push(name);
        }
    }
    slugs.sort();
    Some(slugs)
}

/// One recording of the clone: a tree holding only its folder, the folder's key, its parsed scan and whether the scan
/// is a DVD's (`Disc::is_dvd`).
struct Rec {
    tree: DiskTree,
    dir: PathKey,
    scan: Scan,
    dvd: bool,
}

/// The recording `slug`; None, with the skip line written, when the clone or the recording is absent.
fn recording(slug: &str) -> Option<Rec> {
    let scan_path = evidence().join(slug).join("scan.txt");
    if !scan_path.is_file() {
        skip(&format!("no {} (the evidence clone or this recording is absent)", scan_path.display()));
        return None;
    }
    let tree = DiskTree::open_sub(&evidence(), "corpus/owner", slug).unwrap_or_else(|error| panic!("{slug}: {error}"));
    let dir = PathKey::new(slug);
    let text = read_text(&tree, &dir.join("scan.txt"), Encoding::Utf8Sig, Errors::Replace)
        .unwrap_or_else(|error| panic!("{slug}: {error}"));
    let scan = parse(&text);
    let dvd = disc(&scan).unwrap_or_else(|error| panic!("{slug}: {error}")).is_dvd;
    Some(Rec { tree, dir, scan, dvd })
}

/// A rip label verified as the engine verifies a rip: its title, its selection, its rip log and the report.
struct Verified {
    title: Title,
    selection: Vec<u32>,
    rip_log: String,
    report: VerifyReport,
}

/// `label` of `rec`: `rip_label` gives the title and the selection, `selected_streams` the selected streams, and
/// `verify_rip` checks `mkvmerge.json` against the scan's title with the label's rip log. A refusal fails the test.
fn verified(rec: &Rec, label: &str) -> Verified {
    let rip = rip_label(&rec.tree, &rec.dir, label).unwrap_or_else(|error| panic!("{error}"));
    let number = u32::try_from(rip.title).unwrap();
    let title = disc(&rec.scan).unwrap().title(number).unwrap().clone();
    let selection = selected_streams(&rec.scan, number, rip.selection).unwrap();
    let folder = rec.dir.join(label);
    let ident: MkvmergeIdent =
        read_json(&rec.tree, &folder.join("mkvmerge.json")).unwrap_or_else(|error| panic!("{label}: {error}"));
    let log = RIP_LOG_NAMES
        .iter()
        .map(|name| folder.join(name))
        .find(|path| rec.tree.is_file(path))
        .unwrap_or_else(|| panic!("{label}: no rip.txt or extract.txt"));
    let rip_log = read_text(&rec.tree, &log, Encoding::Utf8Sig, Errors::Replace).unwrap_or_else(|error| panic!("{error}"));
    let report = verify_rip(&title, &ident, &selection, Some(rip_log.as_str()), rec.dvd);
    Verified { title, selection, rip_log, report }
}

/// The pairs of a file holding `streams` in this order as its tracks 0, 1, 2 and on.
fn pairs(streams: &[u32]) -> Vec<(u32, Option<i64>)> {
    streams.iter().enumerate().map(|(id, &stream)| (stream, Some(i64::try_from(id).unwrap()))).collect()
}

/// The report verified, with no difference.
fn assert_verified(label: &str, report: &VerifyReport) {
    assert!(report.ok, "{label}: {:?}", report.mismatches);
    assert_eq!(report.mismatches, Vec::<String>::new(), "{label}");
    assert_eq!(report.extra_tracks, Vec::<i64>::new(), "{label}");
    assert_eq!(report.unknown_removed, Vec::<i64>::new(), "{label}");
}

/// subs/index.json of `label` mapped onto scan streams.
fn subs_map(rec: &Rec, label: &str, report: &VerifyReport) -> BTreeMap<i64, u32> {
    let index: Vec<SubsIndexEntry> = read_json(&rec.tree, &rec.dir.join(label).join("subs/index.json"))
        .unwrap_or_else(|error| panic!("{label}: {error}"));
    subs_streams(&index, report)
}

/// `seconds` to one decimal, the precision the public docs give a file's duration (RP21: never the clone's milliseconds).
fn tenths(seconds: f64) -> f64 {
    (seconds * 10.0).round() / 10.0
}

/// Per scan stream, the event count of `subtitle_events` at `clear_max_bytes`. Each stream's starts are checked against
/// `align::events` of its packets file, read here, with the limit its codec takes (0 where `align::is_text`), so the
/// keying and the limit are checked without a start time of the clone written into this file (RP21).
fn event_counts(rec: &Rec, label: &str, report: &VerifyReport, clear_max_bytes: i64) -> BTreeMap<u32, usize> {
    let subs = rec.dir.join(label).join("subs");
    let by_stream =
        subtitle_events(&rec.tree, &subs, report, clear_max_bytes).unwrap_or_else(|error| panic!("{label}: {error}"));
    let index: Vec<SubsIndexEntry> =
        read_json(&rec.tree, &subs.join("index.json")).unwrap_or_else(|error| panic!("{label}: {error}"));
    for (n, stream) in subs_streams(&index, report) {
        let entry = index.iter().find(|entry| entry.n == n).unwrap();
        let path = subs.join(&format!("sub-{n}.packets.json"));
        let text = read_text(&rec.tree, &path, Encoding::Utf8Sig, Errors::Strict).unwrap_or_else(|error| panic!("{error}"));
        let packets = load_packets(&text, &rec.tree.display(&path)).unwrap_or_else(|error| panic!("{error}"));
        let limit = if is_text(entry.codec.as_deref()) { 0 } else { clear_max_bytes };
        let starts = events(&packets, limit).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(by_stream.get(&stream), Some(&starts), "{label}: stream {stream}");
    }
    by_stream.into_iter().map(|(stream, starts)| (stream, starts.len())).collect()
}

#[test]
fn pans_labyrinth_2006_uhd() {
    let Some(rec) = recording("pans-labyrinth-2006-uhd") else { return };
    assert!(!rec.dvd);
    assert_eq!(selected_streams(&rec.scan, 0, Selection::All).unwrap(), [0, 1, 2, 3, 4, 5, 6, 7]);
    assert_eq!(selected_streams(&rec.scan, 0, Selection::Eng).unwrap(), [0, 3, 4, 5]);
    let default = Settings::default().sub_clear_max_bytes;
    assert_eq!(default, 64);

    let all = verified(&rec, "all-tracks");
    assert_eq!(all.selection, [0, 1, 2, 3, 4, 5, 6, 7]);
    assert_verified("all-tracks", &all.report);
    assert_eq!(all.report.pairs, pairs(&[0, 1, 2, 3, 4, 6]));
    assert_eq!((all.report.expected_seconds, tenths(all.report.file_seconds)), (7150.0, 7150.1));
    assert_eq!((all.report.scan_chapters, all.report.file_chapters), (22, 22));
    assert_eq!(all.report.read_errors, Vec::<ReadError>::new());
    assert_eq!(subs_map(&rec, "all-tracks", &all.report), BTreeMap::from([(0, 4), (1, 6)]));
    assert_eq!(event_counts(&rec, "all-tracks", &all.report, default), BTreeMap::from([(4, 887), (6, 884)]));
    // The docs publish stream 4's count at limit 0 (REPORT.md: 1774 = 2 x 887), not stream 6's. event_counts checks
    // stream 6 against align::events of its packets file, and at limit 0 it counts more events than at the default.
    let at_zero = event_counts(&rec, "all-tracks", &all.report, 0);
    assert_eq!(at_zero.keys().copied().collect::<Vec<u32>>(), [4, 6]);
    assert_eq!(at_zero.get(&4), Some(&1774));
    assert!(at_zero[&6] > 884);

    let eng = verified(&rec, "eng-only");
    assert_eq!(eng.selection, [0, 3, 4, 5]);
    assert_verified("eng-only", &eng.report);
    assert_eq!(eng.report.pairs, pairs(&[0, 3, 4]));
    assert_eq!((eng.report.expected_seconds, tenths(eng.report.file_seconds)), (7150.0, 7150.1));
    assert_eq!(subs_map(&rec, "eng-only", &eng.report), BTreeMap::from([(0, 4)]));
    assert_eq!(event_counts(&rec, "eng-only", &eng.report, default), BTreeMap::from([(4, 887)]));
    assert_eq!(event_counts(&rec, "eng-only", &eng.report, 0), BTreeMap::from([(4, 1774)]));

    let rehearsal = verified(&rec, "rehearsal");
    assert_eq!(rehearsal.title.number, 1);
    assert_eq!(rehearsal.selection, [0]);
    assert_verified("rehearsal", &rehearsal.report);
    assert_eq!(rehearsal.report.pairs, pairs(&[0]));
    // The file's duration is checked only through report.ok (the 2 s band): the docs publish no figure for it.
    assert_eq!(rehearsal.report.expected_seconds, 340.0);
    assert_eq!((rehearsal.report.scan_chapters, rehearsal.report.file_chapters), (34, 34));
}

#[test]
fn short_circuit_1986_dvd() {
    let Some(rec) = recording("short-circuit-1986-dvd") else { return };
    assert!(rec.dvd);
    let all = verified(&rec, "all-tracks");
    assert_eq!(all.selection, [0, 1, 2]);
    assert_verified("all-tracks", &all.report);
    assert_eq!(all.report.pairs, pairs(&[0, 1, 2]));
    assert_eq!((all.report.expected_seconds, tenths(all.report.file_seconds)), (5904.0, 5919.9));
    assert_eq!((all.report.scan_chapters, all.report.file_chapters), (20, 20));
    // The file runs 15.9 s longer than the scan's 5904 s: the DVD band passes it, the 2 s band would not.
    assert!(!duration_ok(all.report.expected_seconds, all.report.file_seconds, false));
    assert_eq!(all.report.read_errors, Vec::<ReadError>::new());
    assert_eq!(failure_reviews(&all.report, true), Vec::<ReviewReason>::new());
    assert!(!rec.tree.exists(&rec.dir.join("all-tracks/subs")));
}

#[test]
fn short_circuit_2_1988_dvd() {
    let Some(rec) = recording("short-circuit-2-1988-dvd") else { return };
    assert!(rec.dvd);
    assert_eq!(selected_streams(&rec.scan, 0, Selection::All).unwrap(), (0..=12).collect::<Vec<u32>>());
    assert_eq!(selected_streams(&rec.scan, 0, Selection::Eng).unwrap(), [0, 1, 5, 12]);
    let default = Settings::default().sub_clear_max_bytes;

    let all = verified(&rec, "all-tracks-2");
    assert_verified("all-tracks-2", &all.report);
    // The text-converted captions (stream 12) come right after the four audio streams.
    assert_eq!(all.report.pairs, pairs(&[0, 1, 2, 3, 4, 12, 5, 6, 7, 8, 9, 10, 11]));
    assert_eq!((all.report.expected_seconds, tenths(all.report.file_seconds)), (6606.0, 6623.6));
    assert_eq!((all.report.scan_chapters, all.report.file_chapters), (28, 28));
    assert_eq!(all.report.read_errors, vec![ReadError { offset: "1048576".to_string(), count: 3, message: SCRAMBLED.to_string() }]);
    assert_eq!(failure_reviews(&all.report, false), [ReviewReason::ReadErrors]);
    assert_eq!(
        subs_map(&rec, "all-tracks-2", &all.report),
        BTreeMap::from([(0, 12), (1, 5), (2, 6), (3, 7), (4, 8), (5, 9), (6, 10), (7, 11)])
    );
    let counts =
        BTreeMap::from([(5, 1742), (6, 1678), (7, 1736), (8, 1246), (9, 1774), (10, 1774), (11, 1775), (12, 1671)]);
    assert_eq!(event_counts(&rec, "all-tracks-2", &all.report, default), counts);
    // No VobSub packet of this disc is 64 bytes or less, and the text track counts every packet at any limit.
    assert_eq!(event_counts(&rec, "all-tracks-2", &all.report, 0), counts);

    let eng = verified(&rec, "eng-only");
    assert_eq!(eng.selection, [0, 1, 5, 12]);
    assert_verified("eng-only", &eng.report);
    assert_eq!(eng.report.pairs, pairs(&[0, 1, 12, 5]));
    assert_eq!(eng.report.read_errors, Vec::<ReadError>::new());
    assert_eq!(failure_reviews(&eng.report, false), Vec::<ReviewReason>::new());
    assert_eq!(subs_map(&rec, "eng-only", &eng.report), BTreeMap::from([(0, 12), (1, 5)]));
    assert_eq!(event_counts(&rec, "eng-only", &eng.report, default), BTreeMap::from([(5, 1742), (12, 1671)]));
}

#[test]
fn a_read_error_never_carries_the_drive_name() {
    let Some(rec) = recording("short-circuit-2-1988-dvd") else { return };
    let all = verified(&rec, "all-tracks-2");
    let log = parse(&all.rip_log);
    let read_error_lines: Vec<_> = log.messages.iter().filter(|message| message.code == 2003).collect();
    assert_eq!(read_error_lines.len(), 3);
    // Every drive field of the log: MSG:2003's second parameter and the DRV lines' text fields.
    let drive_fields: Vec<String> = read_error_lines
        .iter()
        .filter_map(|message| message.params.get(1).cloned())
        .chain(log.drives.iter().flatten().cloned())
        .filter(|field| field.len() >= 8)
        .collect();
    assert!(!drive_fields.is_empty());
    let errors = serde_json::to_string(&read_errors(&all.rip_log)).unwrap();
    let report = serde_json::to_string(&all.report).unwrap();
    let leaked = drive_fields.iter().filter(|field| errors.contains(field.as_str()) || report.contains(field.as_str())).count();
    assert_eq!(leaked, 0, "a drive field reached the read errors or the report (the field is not printed)");
}

#[test]
fn the_office_2005_s3d3_bluray() {
    let Some(rec) = recording("the-office-2005-s3d3-bluray") else { return };
    assert!(!rec.dvd);
    // Title 0 under the eng profile: the derived DTS core (2304, flag 256) is dropped.
    assert_eq!(selected_streams(&rec.scan, 0, Selection::Eng).unwrap(), [0, 1, 3, 4, 5]);
    let default = Settings::default().sub_clear_max_bytes;

    let e16 = verified(&rec, "e16-stereo");
    assert_eq!((e16.title.number, e16.selection.clone()), (0, vec![0, 1, 2, 3, 4, 5]));
    assert_verified("e16-stereo", &e16.report);
    assert_eq!(e16.report.pairs, pairs(&[0, 1, 2, 3, 4]));
    // File durations of the Office are checked only through report.ok (the 2 s band): the docs publish none.
    assert_eq!(e16.report.expected_seconds, 1309.0);
    // The docs publish no chapter count for this title: the report carries the scan's count and the file matches it.
    assert_eq!((e16.report.scan_chapters, e16.report.file_chapters), (e16.title.chapters, e16.title.chapters));
    assert_eq!(subs_map(&rec, "e16-stereo", &e16.report), BTreeMap::from([(0, 4)]));
    assert_eq!(event_counts(&rec, "e16-stereo", &e16.report, default), BTreeMap::from([(4, 345)]));
    assert_eq!(event_counts(&rec, "e16-stereo", &e16.report, 0), BTreeMap::from([(4, 690)]));

    let rehearsal = verified(&rec, "rehearsal");
    assert_eq!((rehearsal.title.number, rehearsal.selection.clone()), (1, vec![0, 1, 2, 3, 4, 5, 6]));
    assert_verified("rehearsal", &rehearsal.report);
    assert_eq!(rehearsal.report.pairs, pairs(&[0, 1, 2, 3, 4, 5]));
    // The docs publish no duration for this title: the expected duration is the scan's.
    assert_eq!(rehearsal.report.expected_seconds, f64::from(rehearsal.title.seconds.unwrap()));
    assert_eq!(subs_map(&rec, "rehearsal", &rehearsal.report), BTreeMap::from([(0, 5)]));
    assert_eq!(event_counts(&rec, "rehearsal", &rehearsal.report, default), BTreeMap::from([(5, 363)]));
    assert_eq!(event_counts(&rec, "rehearsal", &rehearsal.report, 0), BTreeMap::from([(5, 726)]));

    let t3 = verified(&rec, "t3-all-tracks");
    assert_eq!((t3.title.number, t3.selection.clone()), (3, vec![0, 1, 2, 3, 4, 5]));
    assert_verified("t3-all-tracks", &t3.report);
    assert_eq!(t3.report.pairs, pairs(&[0, 1, 2, 3, 4]));
    assert_eq!(t3.report.expected_seconds, 2564.0);
    assert_eq!(subs_map(&rec, "t3-all-tracks", &t3.report), BTreeMap::from([(0, 4)]));
    assert_eq!(event_counts(&rec, "t3-all-tracks", &t3.report, default), BTreeMap::from([(4, 710)]));
    assert_eq!(event_counts(&rec, "t3-all-tracks", &t3.report, 0), BTreeMap::from([(4, 1420)]));
}

#[test]
fn every_rip_label_of_the_clone_verifies() {
    let Some(slugs) = recorded_slugs() else { return };
    let mut seen = Vec::new();
    for slug in &slugs {
        let rec = recording(slug).unwrap();
        for rip in rip_labels(&rec.tree, &rec.dir) {
            let checked = verified(&rec, &rip.label);
            assert!(checked.report.ok, "{slug}/{}: {:?}", rip.label, checked.report.mismatches);
            seen.push(format!("{slug}/{}", rip.label));
        }
    }
    for known in RIP_LABELS {
        assert!(seen.iter().any(|label| label == known), "{known} is no longer a rip label of the clone");
    }
    for label in seen.iter().filter(|label| !RIP_LABELS.contains(&label.as_str())) {
        skip(&format!("not covered: {label} verifies but has no test in crates/dev/tests/verify.rs"));
    }
}

#[test]
fn recordings_without_a_test_are_listed_as_not_covered() {
    let Some(slugs) = recorded_slugs() else { return };
    for slug in slugs.iter().filter(|slug| !RECORDINGS.contains(&slug.as_str())) {
        skip(&format!("not covered: {slug} has a scan.txt but no test in crates/dev/tests/verify.rs"));
    }
}
