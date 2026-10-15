//! The robot-mode parser over the pinned public scan logs of `corpus/thediscdb` (spec section 5: "title count and
//! the duration of every title against their parsed forms"). Each log `discNN.txt` has TheDiscDB's parsed form
//! beside it, `discNN.json`, whose `Titles` list gives every title's `Index` and `Duration` ("H:MM:SS", MakeMKV's
//! attribute 9). The `discNN-summary.txt` files are TheDiscDB's prose summaries, not logs. Needs no evidence clone.

use std::fs;
use std::path::{Path, PathBuf};

use lazymkv_core::recording::{decode_text, Encoding, Errors};
use lazymkv_core::robot::{duration_seconds, parse};

/// Every pinned log in Python's `Path` order, with its title count and the sum of its titles' durations in
/// seconds, as `robot.parse` and `robot.duration_seconds` gave them on 2026-10-13.
const LOGS: [(&str, usize, u32); 9] = [
    ("movie/John Wick (2014)/4k-ultra-hd--blu-ray/disc01.txt", 10, 15287),
    ("movie/John Wick (2014)/4k-ultra-hd--blu-ray/disc02.txt", 9, 9214),
    ("movie/Knowing (2009)/2010-dvd/disc01.txt", 7, 9454),
    ("movie/Smile (2022)/2022-4k/disc01.txt", 236, 20514),
    ("movie/The Expendables 3 (2014)/the-expendables-3-film-collection/disc01.txt", 64, 77981),
    ("series/Chernobyl (2019)/2020-4k-US/disc01.txt", 33, 24875),
    ("series/Chernobyl (2019)/2020-4k-US/disc02.txt", 31, 18509),
    ("series/Firefly (2002)/2003-dvd-the-complete-series/disc01.txt", 5, 10385),
    ("series/Firefly (2002)/2003-dvd-the-complete-series/disc02.txt", 6, 10465),
];

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/thediscdb")
}

/// Every `*.txt` below `dir` that is not a `-summary.txt`, as its path components below the corpus.
fn scan_logs(dir: &Path, prefix: &[String], found: &mut Vec<Vec<String>>) {
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().to_string_lossy().into_owned();
        let mut components = prefix.to_vec();
        components.push(name.clone());
        if entry.path().is_dir() {
            scan_logs(&entry.path(), &components, found);
        } else if name.ends_with(".txt") && !name.ends_with("-summary.txt") {
            found.push(components);
        }
    }
}

#[test]
fn the_nine_pinned_logs_are_here() {
    let mut found = Vec::new();
    scan_logs(&corpus(), &[], &mut found);
    // Component by component, each name by code point: the order Python gives Path objects.
    found.sort();
    let found: Vec<String> = found.iter().map(|components| components.join("/")).collect();
    let pinned: Vec<&str> = LOGS.iter().map(|(path, _, _)| *path).collect();
    assert_eq!(found, pinned);
}

#[test]
fn every_log_gives_its_parsed_forms_titles_and_durations() {
    for (log, titles_pinned, seconds_pinned) in LOGS {
        let path = corpus().join(log);
        let text = decode_text(&fs::read(&path).unwrap(), Encoding::Utf8, Errors::Replace).unwrap();
        let scan = parse(&text);
        assert!(scan.skipped.is_empty(), "{log}: skipped {:?}", scan.skipped);
        let parsed: serde_json::Value =
            serde_json::from_slice(&fs::read(path.with_extension("json")).unwrap()).unwrap();
        let titles = parsed["Titles"].as_array().unwrap();
        assert_eq!(scan.titles.len(), titles.len(), "{log}: title count against the parsed form");
        assert_eq!(scan.titles.len(), titles_pinned, "{log}: pinned title count");
        let mut seconds = 0;
        for title in titles {
            let index = u32::try_from(title["Index"].as_u64().unwrap()).unwrap();
            let duration = title["Duration"].as_str().unwrap();
            let attrs = scan
                .titles
                .get(&index)
                .unwrap_or_else(|| panic!("{log}: title {index} of the parsed form is not in the scan"));
            assert_eq!(attrs.get(&9).map(String::as_str), Some(duration), "{log}: title {index}");
            let expected = duration_seconds(duration)
                .unwrap_or_else(|| panic!("{log}: title {index}: {duration} is not H:MM:SS"));
            assert_eq!(duration_seconds(&attrs[&9]), Some(expected), "{log}: title {index}");
            seconds += expected;
        }
        assert_eq!(seconds, seconds_pinned, "{log}: pinned sum of the title durations");
    }
}
