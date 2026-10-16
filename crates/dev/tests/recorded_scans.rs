//! `lazymkv-dev scan` and the parser over scans recorded on the mini PC (UTF-8, CRLF line ends): the tracked
//! scenario `corpus/scenarios/disc-a-scan` through the built binary, against what the Python `summarize_scan` and the phase 0
//! plan's Task 12 Step 5 listing printed for it; and every `scan.txt` of the evidence clone with a byte-order mark
//! put in front (Review Focus 1), which skips with its reason when the clone is absent.

use std::path::{Path, PathBuf};
use std::process::Command;

use lazymkv_core::recording::{decode_text, Encoding, Errors};
use lazymkv_core::robot::parse;
use lazymkv_dev::evidence::skip;

/// What the Python `summarize_scan` printed for corpus/scenarios/disc-a-scan, 2026-10-13.
const DISC_A_SUMMARY: &str = concat!(
    "MakeMKV: MakeMKV v2.0.0 win(x64-release)\n",
    "Java in use: no (MSG:3344 absent)\n",
    "Disc: type=Blu-ray disc name=Pan's Labyrinth volume=PANS_LABYRINTH\n",
    "Message codes seen: [1004, 1005, 1011, 3007, 3025, 3307, 3338, 5011, 5085]\n",
    "\n",
    "  # source          duration chap         bytes segs aud sub  comment / segment map\n",
    "  0 00800.mpls       1:59:10   22   55613952000    1   3   4   [6]\n",
    "  1 00055.mpls       0:05:40   34     211617792   34   0   0   [27,28,29,30,31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,]\n",
);

/// The Step 5 heredoc for that folder and title 0, 2026-10-13 (no all-tracks/probe-summary.json there).
const DISC_A_TITLE_0: &str = concat!(
    "disc-a-scan title 0: subtitle streams in scan order; s<k> in lazymkv-dev align is line s<k>\n",
    "s0   stream  4  lang=eng  codec=PGS        flags=0     conversion=( Lossless conversion )\n",
    "s1   stream  5  lang=eng  codec=PGS        flags=6144  conversion=( Lossless conversion )  derived forced-only: set aside, not counted\n",
    "s2   stream  6  lang=spa  codec=PGS        flags=0     conversion=( Lossless conversion )\n",
    "s3   stream  7  lang=spa  codec=PGS        flags=6144  conversion=( Lossless conversion )  derived forced-only: set aside, not counted\n",
    "English or no-language subtitle streams, derived forced-only streams not counted: 1\n",
);

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The built lazymkv-dev, run from the repository root: (exit code, stdout, stderr).
fn lazymkv_dev(args: &[&str]) -> (Option<i32>, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_lazymkv-dev"))
        .args(args)
        .current_dir(repo_root())
        .env_remove("LAZYMKV_EVIDENCE")
        .output()
        .unwrap();
    (output.status.code(), String::from_utf8(output.stdout).unwrap(), String::from_utf8(output.stderr).unwrap())
}

#[test]
fn the_scenario_scan_is_summarised_as_summarize_scan_did() {
    let (code, out, err) = lazymkv_dev(&["scan", "corpus/scenarios/disc-a-scan"]);
    assert_eq!(code, Some(0), "stderr: {err}");
    assert_eq!(out, DISC_A_SUMMARY);
}

#[test]
fn the_scenario_scans_title_0_is_listed_as_the_step_5_listing_did() {
    let (code, out, err) = lazymkv_dev(&["scan", "corpus/scenarios/disc-a-scan", "--title", "0"]);
    assert_eq!(code, Some(0), "stderr: {err}");
    assert_eq!(out, DISC_A_TITLE_0);
}

#[test]
fn every_recorded_scan_parses_with_a_byte_order_mark_and_crlf() {
    let root = match std::env::var("LAZYMKV_EVIDENCE") {
        Ok(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => repo_root().join("corpus/owner"),
    };
    if !root.is_dir() {
        skip(&format!("no evidence clone at {}", root.display()));
        return;
    }
    let mut logs = Vec::new();
    for entry in std::fs::read_dir(&root).unwrap() {
        let dir = entry.unwrap().path();
        let log = dir.join("scan.txt");
        if dir.file_name().is_some_and(|name| name != "golden") && log.is_file() {
            logs.push(log);
        }
    }
    logs.sort();
    if logs.is_empty() {
        skip(&format!("no recording with a scan.txt under {}", root.display()));
        return;
    }
    let mut with_crlf = 0;
    for log in &logs {
        let mut bytes = b"\xef\xbb\xbf".to_vec();
        bytes.extend_from_slice(&std::fs::read(log).unwrap());
        let text = decode_text(&bytes, Encoding::Utf8, Errors::Replace).unwrap();
        if text.contains("\r\n") {
            with_crlf += 1;
        }
        let scan = parse(&text);
        // Never print the scan itself: its DRV lines can carry a drive serial.
        assert!(scan.skipped.is_empty(), "{}: {} lines skipped", log.display(), scan.skipped.len());
        let lines = |kind: &str| text.split('\n').filter(|l| l.trim_start_matches('\u{feff}').starts_with(kind)).count();
        assert_eq!(scan.messages.len(), lines("MSG:"), "{}: MSG lines", log.display());
        assert_eq!(scan.drives.len(), lines("DRV:"), "{}: DRV lines", log.display());
        let unix = text.trim_start_matches('\u{feff}').replace("\r\n", "\n");
        assert!(scan == parse(&unix), "{}: a CRLF line parsed differently from its LF form", log.display());
    }
    if with_crlf == 0 {
        skip("no recorded scan.txt has CRLF line ends; the byte-order mark alone was checked");
    }
}
