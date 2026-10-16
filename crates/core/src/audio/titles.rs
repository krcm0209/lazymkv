//! The `titles` listing of a recorded scan (EXP-13 addendum 9.1): every title longest first, its audio streams by
//! language without the streams MakeMKV derived from another, a mark when one language has two or more streams or
//! the wanted languages have two or more together, the labels it was ripped under, and the titles it shares
//! segments with (not on a DVD). Py: audio_similarity.segments, audio_by_title, dvd_scan, titles_report.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::lang::{iso639, NO_LANGUAGE};
use crate::recording::{
    decode_text, py_int, py_isdigit, py_str, py_strip, py_truthy, read_json, read_text, Encoding, Errors, PathKey,
    Tree,
};
use crate::robot::{duration_seconds, int_attr, parse, Scan};

/// Scan flag (attribute 22) of a stream MakeMKV derived from another, such as the DTS core of a DTS-HD MA track.
/// Py: DERIVED.
const DERIVED: i64 = 2048;

/// The listing's second line, verbatim.
const AUDIO_LINE: &str =
    "  audio: scan stream numbers by language, leaving out streams MakeMKV derived from another (flag 2048)";

/// The listing's line for a DVD, verbatim.
const DVD_LINE: &str = "  segments: a DVD's are the cells of each title's own program chain, numbered from 1 in every title, so shared segments are not looked for (a play-all title is not marked)";

/// A scan's segment map (title attribute 26), such as "7", "10,11" or "1-3", as segment numbers; a part that is not
/// a number or a range of numbers is skipped. Py: segments.
pub fn segments(text: &str) -> Vec<i64> {
    let mut out = Vec::new();
    for part in text.split(',') {
        let part = py_strip(part);
        // part.partition("-")
        let (lo, hi) = match part.find('-') {
            Some(at) => (&part[..at], &part[at + 1..]),
            None => (part, ""),
        };
        if py_isdigit(lo) && (hi.is_empty() || py_isdigit(hi)) {
            // A number too large for i64 is skipped (the Python would build a range it cannot hold).
            if let (Some(first), Some(last)) = (py_int(lo), py_int(if hi.is_empty() { lo } else { hi })) {
                out.extend(first..=last);
            }
        }
    }
    out
}

/// One title's duration, segments and audio streams by language (derived streams, flag 2048, left out; "" is
/// "und"). Py: the tuples of audio_by_title.
#[derive(Clone, Debug, PartialEq)]
pub struct TitleAudio {
    pub seconds: u32,
    pub segments: Vec<i64>,
    pub languages: BTreeMap<String, Vec<u32>>,
}

/// Every title of a scan; a title whose duration does not parse has 0 seconds. Err where int_attr(22) raised (the
/// Python's ValueError). Py: audio_by_title.
pub fn audio_by_title(scan: &Scan) -> Result<BTreeMap<u32, TitleAudio>, String> {
    let mut out = BTreeMap::new();
    for (&title, a) in &scan.titles {
        let seconds = duration_seconds(a.get(&9).map(String::as_str).unwrap_or("0:00:00")).unwrap_or(0);
        let segs = segments(a.get(&26).map(String::as_str).unwrap_or(""));
        out.insert(title, TitleAudio { seconds, segments: segs, languages: BTreeMap::new() });
    }
    for (&(title, stream), a) in &scan.streams {
        // `title not in out or a.get(1) != "Audio" or int(a.get(22, "0") or 0) & DERIVED`, in that order: the
        // flags attribute is read only for an audio stream of a listed title.
        let Some(entry) = out.get_mut(&title) else { continue };
        if a.get(&1).map(String::as_str) != Some("Audio") {
            continue;
        }
        if int_attr(a, 22)? & DERIVED != 0 {
            continue;
        }
        let language = match a.get(&3) {
            Some(code) if !code.is_empty() => code.clone(),
            _ => "und".to_string(),
        };
        entry.languages.entry(language).or_default().push(stream);
    }
    Ok(out)
}

/// True for the scan of a DVD: no title names its source file (attribute 16, a Blu-ray's playlist), and the disc
/// says DVD (disc attribute 1) or a stream is DVD video or DVD subtitles (codec V_MPEG2 or S_VOBSUB). Py: dvd_scan.
pub fn dvd_scan(scan: &Scan) -> bool {
    if scan.titles.values().any(|a| a.contains_key(&16)) {
        return false;
    }
    scan.disc.get(&1).map_or(false, |disc| disc.contains("DVD"))
        || scan
            .streams
            .values()
            .any(|s| matches!(s.get(&5).map(String::as_str), Some("V_MPEG2") | Some("S_VOBSUB")))
}

/// The `titles` listing of `<recording>/scan.txt`, line for line. Err when scan.txt or tmdb.json cannot be read, or
/// a rip-summary.json cannot be read from disk or is JSON but not an object, or tmdb.json is not an object or its
/// original_language is a list or an object (where the Python crashed). Py: titles_report.
pub fn titles_report(tree: &dyn Tree, recording: &PathKey) -> Result<Vec<String>, String> {
    let scan = parse(&read_text(tree, &recording.join("scan.txt"), Encoding::Utf8Sig, Errors::Replace)?);

    // The labels each title was ripped under: <label>/rip-summary.json with an int "title", " (failed)" unless "ok".
    let mut ripped: BTreeMap<i64, Vec<String>> = BTreeMap::new();
    for path in tree.glob(recording, "*/rip-summary.json") {
        let bytes = tree.read(&path).map_err(|e| e.to_string())?;
        // json.loads(path.read_text(encoding="utf-8-sig")) under `except ValueError: continue`: a decoding error
        // (UnicodeDecodeError is a ValueError) and a JSON syntax error skip the file.
        let Ok(text) = decode_text(&bytes[..], Encoding::Utf8Sig, Errors::Strict) else { continue };
        let Ok(rip) = serde_json::from_str::<Value>(&text) else { continue };
        let Some(rip) = rip.as_object() else {
            return Err(format!("{}: not a JSON object", tree.display(&path)));
        };
        // isinstance(rip.get("title"), int): a JSON integer, or a bool (True is the int 1 in Python).
        let title = match rip.get("title") {
            Some(Value::Number(n)) => n.as_i64(),
            Some(Value::Bool(b)) => Some(i64::from(*b)),
            _ => None,
        };
        let Some(title) = title else { continue };
        let ok = rip.get("ok").map_or(false, py_truthy);
        let label = format!("{}{}", path.parent().name(), if ok { "" } else { " (failed)" });
        ripped.entry(title).or_default().push(label);
    }

    // TMDb's original language: the wanted languages are English and its codes.
    let tmdb = recording.join("tmdb.json");
    let original = if tree.exists(&tmdb) {
        match read_json::<Value>(tree, &tmdb)? {
            Value::Object(doc) => doc.get("original_language").cloned().unwrap_or(Value::Null),
            _ => return Err(format!("{}: not a JSON object", tree.display(&tmdb))),
        }
    } else {
        Value::Null
    };
    let codes: &[&str] = match &original {
        Value::String(code) => iso639(code).unwrap_or(&[]),
        Value::Array(_) | Value::Object(_) => {
            return Err(format!("{}: original_language is not a language code", tree.display(&tmdb)));
        }
        _ => &[],
    };
    let wanted = |lang: &str| lang == "eng" || codes.contains(&lang);

    let info = audio_by_title(&scan)?;
    let dvd = dvd_scan(&scan);
    let shown = if py_truthy(&original) { py_str(&original) } else { "unknown".to_string() };
    let mut lines = vec![
        format!("{}: {} titles, longest first; TMDb original language: {}", recording.name(), info.len(), shown),
        AUDIO_LINE.to_string(),
    ];
    if dvd {
        lines.push(DVD_LINE.to_string());
    }
    let mut order: Vec<u32> = info.keys().copied().collect();
    // sorted(info, key=lambda t: (-info[t][0], t))
    order.sort_by(|a, b| info[b].seconds.cmp(&info[a].seconds).then(a.cmp(b)));
    for title in order {
        let t = &info[&title];
        let audio: Vec<String> = t
            .languages
            .iter()
            .map(|(lang, streams)| {
                let numbers: Vec<String> = streams.iter().map(|s| s.to_string()).collect();
                format!("{} {}", lang, numbers.join(","))
            })
            .collect();
        let audio = if audio.is_empty() { "none".to_string() } else { audio.join("; ") };
        let mut notes: Vec<String> = t
            .languages
            .iter()
            .filter(|(_, streams)| streams.len() >= 2)
            .map(|(lang, streams)| format!("<- {} x{}", lang, streams.len()))
            .collect();
        let mine: Vec<(&String, &Vec<u32>)> = t
            .languages
            .iter()
            .filter(|(lang, _)| wanted(lang.as_str()) || NO_LANGUAGE.contains(&lang.as_str()))
            .collect();
        let together: usize = mine.iter().map(|(_, streams)| streams.len()).sum();
        if together >= 2 && mine.iter().all(|(_, streams)| streams.len() < 2) {
            let each: Vec<String> = mine.iter().map(|(lang, streams)| format!("{} {}", lang, streams.len())).collect();
            notes.push(format!("<- wanted x{} ({})", together, each.join(", ")));
        }
        if let Some(labels) = ripped.get(&i64::from(title)) {
            notes.push(format!("ripped as {}", labels.join(", ")));
        }
        let shared: Vec<String> = if dvd {
            Vec::new()
        } else {
            info.iter()
                .filter(|&(&other, o)| other != title && t.segments.iter().any(|s| o.segments.contains(s)))
                .map(|(other, _)| other.to_string())
                .collect()
        };
        if !shared.is_empty() {
            notes.push(format!("shares segments with title {}", shared.join(", ")));
        }
        let segs: Vec<String> = t.segments.iter().map(|s| s.to_string()).collect();
        let place = if segs.is_empty() { "?".to_string() } else { segs.join(",") };
        let duration = scan.titles[&title].get(&9).map(String::as_str).unwrap_or("?");
        let line = format!("  title {:>3}  {:>8}  segments {:<8}  audio {}", title, duration, place, audio);
        lines.push(if notes.is_empty() { line } else { format!("{}   {}", line, notes.join("; ")) });
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recording::{py_split, MemTree, PathKey};
    use crate::robot::parse;
    use std::collections::BTreeMap;

    /// Titles.SCAN of test_audio_similarity.py, line for line: four titles, title 2 a play-all of segments 4 and 7,
    /// derived DTS cores (flag 2304 = 2048 + 256) on titles 0 and 1, and title 3 with no flags attribute at all.
    const SCAN: &str = concat!(
        "TINFO:0,9,0,\"0:21:49\"\n",
        "TINFO:0,26,0,\"7\"\n",
        "TINFO:1,9,0,\"0:42:44\"\n",
        "TINFO:1,26,0,\"4\"\n",
        "TINFO:2,9,0,\"1:04:33\"\n",
        "TINFO:2,26,0,\"4,7\"\n",
        "TINFO:3,9,0,\"0:05:10\"\n",
        "TINFO:3,26,0,\"9\"\n",
        "SINFO:0,0,1,0,\"Video\"\n",
        "SINFO:0,1,1,0,\"Audio\"\n",
        "SINFO:0,1,3,0,\"eng\"\n",
        "SINFO:0,1,22,0,\"1024\"\n",
        "SINFO:0,2,1,0,\"Audio\"\n",
        "SINFO:0,2,3,0,\"eng\"\n",
        "SINFO:0,2,22,0,\"2304\"\n",
        "SINFO:0,3,1,0,\"Audio\"\n",
        "SINFO:0,3,3,0,\"eng\"\n",
        "SINFO:0,3,22,0,\"0\"\n",
        "SINFO:1,1,1,0,\"Audio\"\n",
        "SINFO:1,1,3,0,\"eng\"\n",
        "SINFO:1,1,22,0,\"1024\"\n",
        "SINFO:1,2,1,0,\"Audio\"\n",
        "SINFO:1,2,3,0,\"eng\"\n",
        "SINFO:1,2,22,0,\"2304\"\n",
        "SINFO:1,3,1,0,\"Audio\"\n",
        "SINFO:1,3,3,0,\"fra\"\n",
        "SINFO:1,3,22,0,\"0\"\n",
        "SINFO:2,1,1,0,\"Audio\"\n",
        "SINFO:2,1,3,0,\"eng\"\n",
        "SINFO:2,1,22,0,\"0\"\n",
        "SINFO:3,1,1,0,\"Audio\"\n",
        "SINFO:3,1,3,0,\"eng\"\n",
        "SINFO:3,2,1,0,\"Audio\"\n",
        "SINFO:3,2,3,0,\"eng\"\n",
    );

    /// The Python's titles_report of SCAN as <tmp>/some-disc, with e16/rip-summary.json {"ok": true, "title": 0}.
    const SOME_DISC: [&str; 6] = [
        "some-disc: 4 titles, longest first; TMDb original language: unknown",
        "  audio: scan stream numbers by language, leaving out streams MakeMKV derived from another (flag 2048)",
        "  title   2   1:04:33  segments 4,7       audio eng 1   shares segments with title 0, 1",
        "  title   1   0:42:44  segments 4         audio eng 1; fra 3   shares segments with title 2",
        "  title   0   0:21:49  segments 7         audio eng 1,3   <- eng x2; ripped as e16; shares segments with title 2",
        "  title   3   0:05:10  segments 9         audio eng 1,2   <- eng x2",
    ];

    /// The Python's titles_report of SCAN with title 1's fra stream made spa, as <tmp>/some-film with tmdb.json
    /// {"original_language": "es"}.
    const SOME_FILM_ES: [&str; 6] = [
        "some-film: 4 titles, longest first; TMDb original language: es",
        "  audio: scan stream numbers by language, leaving out streams MakeMKV derived from another (flag 2048)",
        "  title   2   1:04:33  segments 4,7       audio eng 1   shares segments with title 0, 1",
        "  title   1   0:42:44  segments 4         audio eng 1; spa 3   <- wanted x2 (eng 1, spa 1); shares segments with title 2",
        "  title   0   0:21:49  segments 7         audio eng 1,3   <- eng x2; shares segments with title 2",
        "  title   3   0:05:10  segments 9         audio eng 1,2   <- eng x2",
    ];

    /// The Python's titles_report of the pinned Firefly DVD scan copied to <tmp>/firefly/scan.txt.
    const FIREFLY: [&str; 8] = [
        "firefly: 5 titles, longest first; TMDb original language: unknown",
        "  audio: scan stream numbers by language, leaving out streams MakeMKV derived from another (flag 2048)",
        "  segments: a DVD's are the cells of each title's own program chain, numbered from 1 in every title, so shared segments are not looked for (a play-all title is not marked)",
        "  title   0   1:26:26  segments 1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25  audio eng 1,4; fre 2; spa 3   <- eng x2",
        "  title   2   0:43:47  segments 1,2,3,4,5,6,7,8,9,10,11,12,13,14,15  audio eng 1; fre 2; spa 3",
        "  title   1   0:42:34  segments 1,2,3,4,5,6,7,8,9,10,11,12,13,14,15  audio eng 1,4; fre 2; spa 3   <- eng x2",
        "  title   4   0:00:11  segments 1,2       audio eng 1",
        "  title   3   0:00:07  segments 1,2       audio none",
    ];

    /// A made-up scan for the edge cases: title 1 without segments or audio, title 5 without a duration, an
    /// unparsable flags attribute on a video stream (never read), a stream without a language code, an empty flags
    /// attribute, and a stream of a title the scan does not list (9).
    const EXTRA: &str = concat!(
        "TINFO:1,9,0,\"0:01:00\"\n",
        "TINFO:5,26,0,\"\"\n",
        "TINFO:6,9,0,\"0:30:00\"\n",
        "TINFO:6,26,0,\"2-3\"\n",
        "TINFO:7,9,0,\"0:30:00\"\n",
        "TINFO:7,26,0,\"3,8\"\n",
        "SINFO:6,0,1,0,\"Video\"\n",
        "SINFO:6,0,22,0,\"x\"\n",
        "SINFO:6,1,1,0,\"Audio\"\n",
        "SINFO:6,1,3,0,\"eng\"\n",
        "SINFO:6,2,1,0,\"Audio\"\n",
        "SINFO:6,2,3,0,\"\"\n",
        "SINFO:7,1,1,0,\"Audio\"\n",
        "SINFO:7,1,3,0,\"deu\"\n",
        "SINFO:7,2,1,0,\"Audio\"\n",
        "SINFO:7,2,3,0,\"deu\"\n",
        "SINFO:7,2,22,0,\"\"\n",
        "SINFO:9,1,1,0,\"Audio\"\n",
        "SINFO:9,1,3,0,\"eng\"\n",
    );

    /// The Python's titles_report of EXTRA as <tmp>/extra-disc with tmdb.json {"original_language": "de", ...} and
    /// the rip summaries of edge_cases_of_rip_summaries_languages_and_durations_are_the_pythons.
    const EXTRA_DISC: [&str; 6] = [
        "extra-disc: 4 titles, longest first; TMDb original language: de",
        "  audio: scan stream numbers by language, leaving out streams MakeMKV derived from another (flag 2048)",
        "  title   6   0:30:00  segments 2,3       audio eng 1; und 2   <- wanted x2 (eng 1, und 1); ripped as a-ok, fail (failed); shares segments with title 7",
        "  title   7   0:30:00  segments 3,8       audio deu 1,2   <- deu x2; ripped as bom; shares segments with title 6",
        "  title   1   0:01:00  segments ?         audio none   ripped as bool (failed)",
        "  title   5         ?  segments ?         audio none",
    ];

    fn langs(pairs: &[(&str, &[u32])]) -> BTreeMap<String, Vec<u32>> {
        pairs.iter().map(|(lang, streams)| (lang.to_string(), streams.to_vec())).collect()
    }

    fn firefly_scan() -> Vec<u8> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../corpus/thediscdb/series/Firefly (2002)/2003-dvd-the-complete-series/disc01.txt");
        std::fs::read(&path).unwrap_or_else(|e| panic!("the pinned Firefly scan {}: {e}", path.display()))
    }

    #[test]
    fn titles_list_audio_by_language_without_derived_streams_longest_first() {
        let info = audio_by_title(&parse(SCAN)).unwrap();
        assert_eq!(info[&0], TitleAudio { seconds: 1309, segments: vec![7], languages: langs(&[("eng", &[1, 3])]) });
        assert_eq!(info[&1].languages, langs(&[("eng", &[1]), ("fra", &[3])]));
        let mut tree = MemTree::new("tmp", "tmp");
        tree.insert_file("some-disc/scan.txt", SCAN);
        tree.insert_file("some-disc/e16/rip-summary.json", r#"{"ok": true, "title": 0}"#);
        let lines = titles_report(&tree, &PathKey::new("some-disc")).unwrap();
        let order: Vec<&str> = lines[2..].iter().map(|line| py_split(line)[1]).collect();
        assert_eq!(order, ["2", "1", "0", "3"]);
        assert!(lines[2].contains("shares segments with title 0, 1"), "{}", lines[2]);
        assert!(!lines[3].contains("<-"), "{}", lines[3]);
        assert!(lines[4].contains("<- eng x2; ripped as e16"), "{}", lines[4]);
        assert!(lines[5].contains("<- eng x2"), "{}", lines[5]); // no flags attribute at all: not derived
    }

    #[test]
    fn a_dvds_segments_are_not_compared_across_titles() {
        // A DVD title's segments are the cells of its own program chain, numbered from 1 in every title.
        assert!(!dvd_scan(&parse(SCAN)));
        let dvd = format!("CINFO:1,6206,\"DVD disc\"\n{SCAN}");
        assert!(dvd_scan(&parse(&dvd)));
        assert!(dvd_scan(&parse(&format!("{SCAN}SINFO:0,0,5,0,\"V_MPEG2\"\n"))));
        assert!(!dvd_scan(&parse(&format!("{dvd}TINFO:0,16,0,\"00800.mpls\"\n")))); // a source file: a Blu-ray
        let mut tree = MemTree::new("tmp", "tmp");
        tree.insert_file("firefly/scan.txt", firefly_scan());
        let lines = titles_report(&tree, &PathKey::new("firefly")).unwrap();
        assert!(lines[2].contains("so shared segments are not looked for"), "{}", lines[2]);
        assert!(lines.iter().all(|line| !line.contains("shares segments")), "{lines:#?}");
        let one: Vec<&String> = lines.iter().filter(|line| py_split(line).starts_with(&["title", "1"])).collect();
        assert_eq!(one.len(), 1, "{lines:#?}");
        // the episode with a commentary, not skipped
        assert!(one[0].contains("audio eng 1,4; fre 2; spa 3   <- eng x2"), "{}", one[0]);
    }

    #[test]
    fn a_title_with_one_stream_in_each_wanted_language_is_marked() {
        let mut tree = MemTree::new("tmp", "tmp");
        tree.insert_file("some-film/scan.txt", SCAN.replace("SINFO:1,3,3,0,\"fra\"", "SINFO:1,3,3,0,\"spa\""));
        let film = PathKey::new("some-film");
        // original language unknown: English alone is wanted
        assert!(!titles_report(&tree, &film).unwrap()[3].contains("<-"));
        tree.insert_file("some-film/tmdb.json", r#"{"original_language": "es"}"#);
        let lines = titles_report(&tree, &film).unwrap();
        assert!(lines[0].contains("TMDb original language: es"), "{}", lines[0]);
        assert_eq!(py_split(&lines[3])[1], "1");
        assert!(lines[3].contains("audio eng 1; spa 3   <- wanted x2 (eng 1, spa 1)"), "{}", lines[3]);
    }

    #[test]
    fn segments_reads_numbers_and_ranges_and_skips_malformed_parts() {
        // Each case as the Python's segments() gave it.
        let cases: [(&str, &[i64]); 12] = [
            ("7", &[7]),
            ("10,11", &[10, 11]),
            ("1-3", &[1, 2, 3]),
            (" 4 , 5-6 ", &[4, 5, 6]),
            ("x,2", &[2]),
            ("3-x", &[]),
            ("", &[]),
            ("5-3", &[]),
            ("-5", &[]),
            ("3-", &[3]),
            ("1-2,3,4-6", &[1, 2, 3, 4, 5, 6]),
            ("2-2", &[2]),
        ];
        for (text, want) in cases {
            assert_eq!(segments(text), want, "segments({text:?})");
        }
    }

    #[test]
    fn titles_reports_are_the_pythons_line_for_line() {
        let mut tree = MemTree::new("tmp", "tmp");
        tree.insert_file("some-disc/scan.txt", SCAN);
        tree.insert_file("some-disc/e16/rip-summary.json", r#"{"ok": true, "title": 0}"#);
        tree.insert_file("some-film/scan.txt", SCAN.replace("SINFO:1,3,3,0,\"fra\"", "SINFO:1,3,3,0,\"spa\""));
        tree.insert_file("some-film/tmdb.json", r#"{"original_language": "es"}"#);
        tree.insert_file("firefly/scan.txt", firefly_scan());
        assert_eq!(titles_report(&tree, &PathKey::new("some-disc")).unwrap(), SOME_DISC);
        assert_eq!(titles_report(&tree, &PathKey::new("some-film")).unwrap(), SOME_FILM_ES);
        assert_eq!(titles_report(&tree, &PathKey::new("firefly")).unwrap(), FIREFLY);
    }

    #[test]
    fn edge_cases_of_rip_summaries_languages_and_durations_are_the_pythons() {
        let info = audio_by_title(&parse(EXTRA)).unwrap();
        assert_eq!(info[&1], TitleAudio { seconds: 60, segments: vec![], languages: BTreeMap::new() });
        assert_eq!(info[&5], TitleAudio { seconds: 0, segments: vec![], languages: BTreeMap::new() });
        assert_eq!(info[&6].languages, langs(&[("eng", &[1]), ("und", &[2])]));
        assert_eq!(info[&7], TitleAudio { seconds: 1800, segments: vec![3, 8], languages: langs(&[("deu", &[1, 2])]) });
        assert!(!info.contains_key(&9));
        let mut tree = MemTree::new("tmp", "tmp");
        tree.insert_file("extra-disc/scan.txt", EXTRA);
        tree.insert_file("extra-disc/tmdb.json", r#"{"original_language": "de", "fetched_at": "2026-10-01"}"#);
        // not JSON: skipped (the Python's `except ValueError`)
        tree.insert_file("extra-disc/bad/rip-summary.json", "{");
        // a title that is not an int: skipped
        tree.insert_file("extra-disc/str/rip-summary.json", r#"{"ok": true, "title": "6"}"#);
        tree.insert_file("extra-disc/fail/rip-summary.json", r#"{"ok": false, "title": 6}"#);
        // isinstance(True, int) holds in Python: title 1; no "ok": failed
        tree.insert_file("extra-disc/bool/rip-summary.json", r#"{"title": true}"#);
        // a byte-order mark is read past (utf-8-sig); ok 1 is truthy
        tree.insert_file("extra-disc/bom/rip-summary.json", b"\xef\xbb\xbf{\"ok\": 1, \"title\": 7}".to_vec());
        // a float is not an int: skipped
        tree.insert_file("extra-disc/float/rip-summary.json", r#"{"ok": true, "title": 7.0}"#);
        tree.insert_file("extra-disc/a-ok/rip-summary.json", r#"{"ok": true, "title": 6}"#);
        assert_eq!(titles_report(&tree, &PathKey::new("extra-disc")).unwrap(), EXTRA_DISC);
    }

    #[test]
    fn a_golden_folder_is_never_listed_as_a_rip() {
        // Review Focus item 2: spec section 5 reserves the folder name golden/, and Tree::glob never returns it.
        let mut tree = MemTree::new("tmp", "tmp");
        tree.insert_file("some-disc/scan.txt", SCAN);
        tree.insert_file("some-disc/golden/rip-summary.json", r#"{"ok": true, "title": 0}"#);
        let lines = titles_report(&tree, &PathKey::new("some-disc")).unwrap();
        assert!(lines.iter().all(|line| !line.contains("ripped as")), "{lines:#?}");
    }

    #[test]
    fn an_unreadable_flags_attribute_on_an_audio_stream_is_an_error() {
        // int(a.get(22, "0") or 0) raised ValueError in the Python (a crash); the command exits 2 on it.
        let scan = parse("TINFO:0,9,0,\"0:01:00\"\nSINFO:0,1,1,0,\"Audio\"\nSINFO:0,1,22,0,\"x\"\n");
        let error = audio_by_title(&scan).unwrap_err();
        assert!(error.contains("invalid literal for int() with base 10: 'x'"), "{error}");
    }
}
