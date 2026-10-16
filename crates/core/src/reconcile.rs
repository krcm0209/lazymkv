//! The scan-against-file reconciler of phase 0: `scripts/phase0/compare_tracks.py` (commit 75fa76a's tree) ported
//! line for line (port spec 3.3; RP6: the Python's rule, not the design spec's 6.2 step 7). Five verdicts (count,
//! order, video, chapters, duration), the symmetric 2-second duration band for every format, no closed-caption
//! exception, codecs not compared. No I/O: `lazymkv-dev reconcile` reads the files and prints the lines.

use std::collections::BTreeSet;

use crate::lang::norm_lang;
use crate::recording::files::{MkvmergeIdent, Selection};
use crate::recording::{py_fixed, py_isdigit, py_int, py_repr, py_repr_str, py_str, text_mode};
use crate::robot::{duration_seconds, int_attr, parse, Attrs, Scan};

/// Scan stream type -> file track type ("Video" -> "video", "Audio" -> "audio", "Subtitles" -> "subtitles"); any
/// other value is kept as it is. Py: TYPE.
pub const TYPE: [(&str, &str); 3] = [("Video", "video"), ("Audio", "audio"), ("Subtitles", "subtitles")];
/// The duration check's symmetric tolerance in seconds, for every format. Py: DURATION_TOLERANCE_S.
pub const DURATION_TOLERANCE_S: i64 = 2;
/// MakeMKV's log of the call that wrote the file, looked for beside mkvmerge.json in this order. Py: RIP_LOG_NAMES.
pub const RIP_LOG_NAMES: [&str; 2] = ["rip.txt", "extract.txt"];

/// A stream the scan predicts in the file. Field names and value types are the Python row's (reconcile.json).
/// Py: expected_rows' dict.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExpectedRow {
    pub stream: u32,
    /// TYPE of attribute 1, the raw value when not in TYPE, None when the stream has no attribute 1.
    pub r#type: Option<String>,
    /// norm_lang(lang_raw).
    pub lang: String,
    /// Attribute 3, else (video only) attribute 28, else "und"; an empty value counts as absent.
    pub lang_raw: String,
    /// Attribute 6 ("" when absent).
    pub codec: String,
    /// Attribute 41 ("" when absent).
    pub out: String,
    /// Attribute 14 ("" when absent).
    pub ch: String,
    /// int_attr(22).
    pub flags: i64,
}

/// A track of the file. Py: file_rows' dict.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FileRow {
    pub id: i64,
    pub r#type: String,
    /// norm_lang(properties.language).
    pub lang: String,
    /// properties.language, "und" when absent.
    pub lang_raw: String,
    pub codec: String,
    /// properties.audio_channels as mkvmerge wrote it (an int), or "" when absent; compared through py_str.
    pub ch: serde_json::Value,
    /// The property names starting "tag_", sorted.
    pub tags: Vec<String>,
}

/// The yes/no answers, in the Python's order. Py: checks' verdicts dict.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Verdicts {
    pub count: bool,
    pub order: bool,
    pub video: bool,
    pub chapters: bool,
    pub duration: bool,
}

impl Verdicts {
    /// Every verdict is yes. Py: all(verdicts.values()).
    pub fn all(&self) -> bool {
        self.count && self.order && self.video && self.chapters && self.duration
    }
}

/// The numbers behind them. Py: checks' numbers dict.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Numbers {
    pub scan_chapters: i64,
    pub file_chapters: i64,
    pub scan_seconds: u32,
    pub file_seconds: f64,
}

/// TYPE.get(raw, raw).
fn scan_type(raw: &str) -> String {
    TYPE.iter().find(|(scan, _)| *scan == raw).map_or(raw, |&(_, file)| file).to_string()
}

/// A present, non-empty attribute (Python's `a.get(n) or ...`).
fn truthy(value: Option<&String>) -> Option<&str> {
    value.map(String::as_str).filter(|text| !text.is_empty())
}

/// The scan's streams of `title`, in (title, stream) order, without 3D video (attribute 6 containing "MVC"), filtered
/// by `selection` as the Python does (eng: audio in eng/und without flags 256 and 1|2|4, subtitles in eng).
/// Err where int_attr raised (the Python crashed). Py: expected_rows.
pub fn expected_rows(scan: &Scan, title: u32, selection: Selection) -> Result<Vec<ExpectedRow>, String> {
    let mut rows = Vec::new();
    // sorted(scan.streams.items()): the BTreeMap iterates in (title, stream) order already.
    for (&(t, stream), a) in &scan.streams {
        if t != title {
            continue;
        }
        let kind = a.get(&1).map(|raw| scan_type(raw));
        let flags = int_attr(a, 22).map_err(|e| format!("scan stream {t},{stream}: attribute 22: {e}"))?;
        // A stream's language is attribute 3 (spec 8.6). Attribute 28 is the disc's metadata language ("eng" even on
        // the Chinese subtitles of the pinned John Wick scan), so it stands in only for video, which has no
        // attribute 3 and whose language is never compared.
        let video = kind.as_deref() == Some("video");
        let lang_raw = truthy(a.get(&3))
            .or(if video { truthy(a.get(&28)) } else { None })
            .unwrap_or("und")
            .to_string();
        let lang = norm_lang(Some(lang_raw.as_str()));
        let codec = a.get(&6).cloned().unwrap_or_default();
        if codec.contains("MVC") {
            continue;
        }
        if selection == Selection::Eng {
            let k = kind.as_deref();
            if k == Some("audio") && (!(lang == "eng" || lang == "und") || (flags & 256) != 0 || (flags & 7) != 0) {
                continue;
            }
            if k == Some("subtitles") && lang != "eng" {
                continue;
            }
        }
        rows.push(ExpectedRow {
            stream,
            r#type: kind,
            lang,
            lang_raw,
            codec,
            out: a.get(&41).cloned().unwrap_or_default(),
            ch: a.get(&14).cloned().unwrap_or_default(),
            flags,
        });
    }
    Ok(rows)
}

/// Output track indices MakeMKV left out of the file, from the rip log's MSG:4047 lines.
///
/// MakeMKV drops a forced-only subtitle track that turns out to be empty and says so: "AV sync issue in stream N at
/// T : Forced subtitles track #N turned out to be empty and was removed from output file". N counts the SELECTED
/// streams of the title in scan order (the output file's tracks before the removal), not the scan's stream numbers.
/// Other MSG:4047 lines are not removals. A first parameter too long for an i64 is left out (MakeMKV numbers output
/// tracks below 100). Py: removed_streams.
pub fn removed_streams(rip_text: &str) -> BTreeSet<i64> {
    parse(rip_text)
        .messages
        .iter()
        .filter(|m| m.code == 4047 && m.text.contains("removed from output file"))
        .filter_map(|m| m.params.first())
        .filter(|first| py_isdigit(first))
        .filter_map(|first| py_int(first))
        .collect()
}

/// Splits the predicted rows (the selected streams in scan order) into those still expected in the file and those
/// MakeMKV removed; `removed` holds positions in `rows` (see removed_streams). Py: without_removed.
pub fn without_removed(rows: &[ExpectedRow], removed: &BTreeSet<i64>) -> (Vec<ExpectedRow>, Vec<ExpectedRow>) {
    let kept = rows.iter().enumerate().filter(|(i, _)| !removed.contains(&(*i as i64))).map(|(_, row)| row.clone());
    let gone = rows.iter().enumerate().filter(|(i, _)| removed.contains(&(*i as i64))).map(|(_, row)| row.clone());
    (kept.collect(), gone.collect())
}

/// The file's tracks. Err when a track's language property is not a string (the Python crashed on it).
/// Py: file_rows.
pub fn file_rows(ident: &MkvmergeIdent) -> Result<Vec<FileRow>, String> {
    let mut rows = Vec::new();
    for t in &ident.tracks {
        let language = match t.properties.get("language") {
            None => None,
            Some(serde_json::Value::String(text)) => Some(text.as_str()),
            Some(other) => {
                return Err(format!("track {}: its language property is not a string: {}", t.id, py_repr(other)));
            }
        };
        let mut tags: Vec<String> = t.properties.keys().filter(|key| key.starts_with("tag_")).cloned().collect();
        tags.sort();
        rows.push(FileRow {
            id: t.id,
            r#type: t.r#type.clone(),
            lang: norm_lang(language),
            lang_raw: language.unwrap_or("und").to_string(),
            codec: t.codec.clone(),
            ch: t
                .properties
                .get("audio_channels")
                .cloned()
                .unwrap_or_else(|| serde_json::Value::String(String::new())),
            tags,
        });
    }
    Ok(rows)
}

/// Type must agree; language too, except on video; audio channel counts (py_str of each side) when both sides state
/// one. Py: same_track.
pub fn same_track(w: &ExpectedRow, g: &FileRow) -> bool {
    if w.r#type.as_deref() != Some(g.r#type.as_str()) {
        return false;
    }
    if w.r#type.as_deref() != Some("video") && w.lang != g.lang {
        return false;
    }
    if w.r#type.as_deref() == Some("audio") {
        let (want, got) = (w.ch.as_str(), py_str(&g.ch));
        if !want.is_empty() && !got.is_empty() {
            return want == got;
        }
    }
    true
}

/// The yes/no answers and the numbers behind them, computed in the Python's order. Nothing passes on empty input.
/// Err (exit 2) when attribute 8 is not an int or attribute 9 is not "H:MM:SS" (the Python crashed). Py: checks.
pub fn checks(want: &[ExpectedRow], got: &[FileRow], title: &Attrs, ident: &MkvmergeIdent) -> Result<(Verdicts, Numbers), String> {
    let scan_chapters = int_attr(title, 8).map_err(|e| format!("the title's chapter count (attribute 8): {e}"))?;
    let file_chapters: i64 = ident.chapters.iter().map(|c| c.num_entries).sum();
    let duration = title.get(&9).map_or("0:00:00", String::as_str);
    let scan_seconds = duration_seconds(duration)
        .ok_or_else(|| format!("the title's duration (attribute 9) is not H:MM:SS: {}", py_repr_str(duration)))?;
    let file_seconds = ident.container.properties.duration as f64 / 1e9;
    let numbers = Numbers { scan_chapters, file_chapters, scan_seconds, file_seconds };
    let same_count = want.len() == got.len() && !got.is_empty();
    let verdicts = Verdicts {
        count: same_count,
        order: same_count && want.iter().zip(got).all(|(w, g)| same_track(w, g)),
        video: got.iter().any(|g| g.r#type == "video"),
        chapters: numbers.scan_chapters == numbers.file_chapters,
        duration: numbers.scan_seconds > 0
            && (f64::from(numbers.scan_seconds) - numbers.file_seconds).abs() <= DURATION_TOLERANCE_S as f64,
    };
    Ok((verdicts, numbers))
}

/// "yes" or "no". Py: yes_no.
pub fn yes_no(ok: bool) -> &'static str {
    if ok {
        "yes"
    } else {
        "no"
    }
}

// The report's fixed texts, verbatim from compare_tracks.main.
const RIP_LOG: &str = "RIP LOG          ";
const NO_RIP_LOG: &str = "none found: tracks MakeMKV removed (MSG:4047) are not accounted for";
const REMOVED: &str = "REMOVED          ";
const REMOVED_NOT_EXPECTED: &str = "MakeMKV left it out of the file (MSG:4047), so it is not expected";
const REMOVED_UNKNOWN: &str = ", which the prediction does not have";
const TRACK_COUNT: &str = "TRACK COUNT      ";
const ORDER_MATCH: &str = "ORDER MATCH      ";
const ORDER_MEANS: &str = " (type, language except on video, audio channels; position by position)";
const CHAPTERS: &str = "CHAPTERS         ";
const DURATION: &str = "DURATION         ";
const VIDEO_TRACKS: &str = "VIDEO TRACKS     ";
const STATISTICS_TAGS: &str = "STATISTICS TAGS  ";
const VERDICT: &str = "VERDICT          ";
const CODECS_NOT_COMPARED: &str =
    "                 codecs are not compared: read the scan's out= column against the file's codec column";

impl Verdicts {
    /// (name, answer) in the Python dict's order, for the VERDICT line.
    fn named(&self) -> [(&'static str, bool); 5] {
        [
            ("count", self.count),
            ("order", self.order),
            ("video", self.video),
            ("chapters", self.chapters),
            ("duration", self.duration),
        ]
    }
}

/// MakeMKV's log of the rip: how it is printed on the RIP LOG line, and its text as Python's `read_text` gave it
/// ("\r\n" and a lone "\r" already turned into "\n"; `lazymkv-dev reconcile` does this before building it).
#[derive(Clone, Debug)]
pub struct RipLog<'a> {
    pub shown: String,
    pub text: &'a str,
}

/// The full report of one rip (port spec 3.3), which is also reconcile.json and decide's "reconcile". Field order is
/// the JSON order.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Report {
    pub title: u32,
    pub selection: Selection,
    /// The rip log as printed (corpus/owner/<slug>/<label>/rip.txt), None when none was found.
    pub rip_log: Option<String>,
    /// The predicted streams still expected (after the removal).
    pub want: Vec<ExpectedRow>,
    /// The predicted streams MakeMKV removed (the REMOVED lines).
    pub dropped: Vec<ExpectedRow>,
    /// MSG:4047 indices beyond the prediction, sorted.
    pub unknown_removed: Vec<i64>,
    pub got: Vec<FileRow>,
    pub verdicts: Verdicts,
    pub numbers: Numbers,
    pub video_tracks: usize,
    /// Any file track with a tag_ property.
    pub statistics_tags: bool,
    /// 0 when every verdict is yes, else 1.
    pub exit_code: i32,
}

/// Everything compare_tracks.main computes for `title` of `scan` against `ident`, with the rip log when one was
/// found. Err (exit 2) where the Python crashed, checked in the Python's order: expected_rows, file_rows, a kept
/// stream without a type (the table's `:<9` raised on None), a title the scan does not list (KeyError), checks.
/// Py: compare_tracks.main.
pub fn reconcile(
    scan: &Scan,
    title: u32,
    ident: &MkvmergeIdent,
    selection: Selection,
    rip: Option<RipLog<'_>>,
) -> Result<Report, String> {
    let rows = expected_rows(scan, title, selection)?;
    let got = file_rows(ident)?;
    let (rip_log, removed) = match rip {
        // The Python read the rip log with read_text (universal newlines); parse translates too (idempotent).
        Some(log) => (Some(log.shown), removed_streams(&text_mode(log.text))),
        None => (None, BTreeSet::new()),
    };
    let (want, dropped) = without_removed(&rows, &removed);
    if let Some(row) = want.iter().find(|row| row.r#type.is_none()) {
        return Err(format!("scan stream {title},{} has no type (attribute 1)", row.stream));
    }
    let attrs = scan.titles.get(&title).ok_or_else(|| format!("the scan lists no title {title}"))?;
    let (verdicts, numbers) = checks(&want, &got, attrs, ident)?;
    // sorted(i for i in removed if i >= len(want) + len(dropped)): the BTreeSet is sorted already.
    let unknown_removed = removed.iter().copied().filter(|&index| index >= rows.len() as i64).collect();
    let video_tracks = got.iter().filter(|g| g.r#type == "video").count();
    let statistics_tags = got.iter().any(|g| !g.tags.is_empty());
    let exit_code = if verdicts.all() { 0 } else { 1 };
    Ok(Report {
        title,
        selection,
        rip_log,
        want,
        dropped,
        unknown_removed,
        got,
        verdicts,
        numbers,
        video_tracks,
        statistics_tags,
        exit_code,
    })
}

/// str() of a row's type, as the REMOVED line prints it: "None" for a stream without attribute 1. (reconcile never
/// lets such a row into `want`, whose table line the Python could not print.)
fn type_shown(kind: &Option<String>) -> &str {
    kind.as_deref().unwrap_or("None")
}

/// The lines compare_tracks.main prints, byte for byte: the table (scan streams left, padded to 54 characters; file
/// tracks right), an empty line, RIP LOG, the REMOVED lines, TRACK COUNT to VERDICT, and the codecs line.
pub fn report_lines(report: &Report) -> Vec<String> {
    let mut lines = vec![format!("{:<54} | file", "scan")];
    for i in 0..report.want.len().max(report.got.len()) {
        let left = report.want.get(i).map_or_else(String::new, |w| {
            format!(
                "{:>2} {:<9} {:<4} {:<12} out={:<8} ch={:<2} f={}",
                w.stream,
                type_shown(&w.r#type),
                w.lang_raw,
                w.codec,
                w.out,
                w.ch,
                w.flags
            )
        });
        let right = report.got.get(i).map_or_else(String::new, |g| {
            format!("{:>2} {:<9} {:<4} {} ch={}", g.id, g.r#type, g.lang_raw, g.codec, py_str(&g.ch))
        });
        lines.push(format!("{left:<54} | {right}"));
    }
    lines.push(String::new());
    lines.push(format!("{RIP_LOG}{}", report.rip_log.as_deref().unwrap_or(NO_RIP_LOG)));
    for r in &report.dropped {
        lines.push(format!(
            "{REMOVED}scan stream {} ({} {} {} f={}): {REMOVED_NOT_EXPECTED}",
            r.stream,
            type_shown(&r.r#type),
            r.lang_raw,
            r.codec,
            r.flags
        ));
    }
    for index in &report.unknown_removed {
        lines.push(format!("{REMOVED}MSG:4047 names output track {index}{REMOVED_UNKNOWN}"));
    }
    lines.push(format!("{TRACK_COUNT}scan={} file={}", report.want.len(), report.got.len()));
    lines.push(format!("{ORDER_MATCH}{}{ORDER_MEANS}", yes_no(report.verdicts.order)));
    lines.push(format!("{CHAPTERS}scan={} file={}", report.numbers.scan_chapters, report.numbers.file_chapters));
    lines.push(format!(
        "{DURATION}scan={}s file={}s",
        report.numbers.scan_seconds,
        py_fixed(report.numbers.file_seconds, 1)
    ));
    lines.push(format!("{VIDEO_TRACKS}{}", report.video_tracks));
    lines.push(format!("{STATISTICS_TAGS}{}", if report.statistics_tags { "present" } else { "absent" }));
    let named: Vec<String> =
        report.verdicts.named().iter().map(|&(name, ok)| format!("{name}={}", yes_no(ok))).collect();
    lines.push(format!("{VERDICT}{}", named.join(" ")));
    lines.push(CODECS_NOT_COMPARED.to_string());
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// One title: video, then two audio tracks of the same language with 8 and 2 channels. Py: TRACK_SCAN.
    const TRACK_SCAN: &str = r#"TINFO:0,8,0,"2"
TINFO:0,9,0,"0:10:00"
SINFO:0,0,1,6201,"Video"
SINFO:0,0,6,0,"MpegH"
SINFO:0,1,1,6202,"Audio"
SINFO:0,1,3,0,"{lang}"
SINFO:0,1,6,0,"TrueHD"
SINFO:0,1,14,0,"8"
SINFO:0,2,1,6202,"Audio"
SINFO:0,2,3,0,"{lang}"
SINFO:0,2,6,0,"DD"
SINFO:0,2,14,0,"2"
"#;

    /// Two MSG:4047 lines: an empty forced-only subtitle track MakeMKV removed (output track 2), then an audio gap,
    /// which is not a removal. Py: the rip of test_a_stream_makemkv_removed_as_empty_is_read_from_the_rip_log.
    const RIP_4047_PAIR: &str = r#"MSG:4047,16,3,"AV sync issue in stream 2 at 0:00:00 : {removed}","AV sync issue in stream %1 at %2 : %3","2","0:00:00","{removed}"
MSG:4047,16,3,"AV sync issue in stream 1 at 0:01:00 : audio gap","AV sync issue in stream %1 at %2 : %3","1","0:01:00","audio gap"
"#;

    /// Title 3 of a synthetic DVD-like scan: video, a 3D video stream (MVC), English audio, a derived English audio
    /// stream (flag 256), an audio stream without a language and with flag 4, Spanish audio, French and English
    /// subtitles. New in the port.
    const SCAN_B: &str = r#"TINFO:3,8,0,"12"
TINFO:3,9,0,"1:30:05"
SINFO:3,0,1,6201,"Video"
SINFO:3,0,6,0,"Mpeg2"
SINFO:3,0,41,0,"V_MPEG2"
SINFO:3,1,1,6201,"Video"
SINFO:3,1,6,0,"Mpeg4 MVC"
SINFO:3,2,1,6202,"Audio"
SINFO:3,2,3,0,"eng"
SINFO:3,2,6,0,"DD"
SINFO:3,2,14,0,"6"
SINFO:3,2,41,0,"A_AC3"
SINFO:3,3,1,6202,"Audio"
SINFO:3,3,3,0,"eng"
SINFO:3,3,6,0,"DD"
SINFO:3,3,14,0,"2"
SINFO:3,3,22,0,"256"
SINFO:3,4,1,6202,"Audio"
SINFO:3,4,6,0,"DD"
SINFO:3,4,14,0,"2"
SINFO:3,4,22,0,"4"
SINFO:3,5,1,6202,"Audio"
SINFO:3,5,3,0,"spa"
SINFO:3,5,6,0,"DD"
SINFO:3,6,1,6203,"Subtitles"
SINFO:3,6,3,0,"fre"
SINFO:3,6,6,0,"VobSub"
SINFO:3,7,1,6203,"Subtitles"
SINFO:3,7,3,0,"eng"
SINFO:3,7,6,0,"VobSub"
SINFO:3,7,41,0,"S_VOBSUB"
"#;

    fn track_scan(lang: &str) -> Scan {
        parse(&TRACK_SCAN.replace("{lang}", lang))
    }

    /// A stand-in for `mkvmerge -J`: tracks are (type, language, audio channels or None); no chapter entry when
    /// `chapters` is 0. Py: ident(tracks, chapters=2, seconds=600) (callers pass the Python's defaults explicitly).
    fn ident(tracks: &[(&str, &str, Option<i64>)], chapters: i64, seconds: i64) -> MkvmergeIdent {
        let tracks: Vec<serde_json::Value> = tracks
            .iter()
            .enumerate()
            .map(|(i, &(kind, lang, ch))| {
                let mut properties = serde_json::Map::new();
                properties.insert("language".to_string(), json!(lang));
                if let Some(ch) = ch.filter(|&ch| ch != 0) {
                    properties.insert("audio_channels".to_string(), json!(ch));
                }
                json!({"id": i, "type": kind, "codec": "x", "properties": properties})
            })
            .collect();
        let chapters = if chapters != 0 { json!([{"num_entries": chapters}]) } else { json!([]) };
        let duration = seconds * 1_000_000_000;
        serde_json::from_value(json!({
            "tracks": tracks,
            "chapters": chapters,
            "container": {"properties": {"duration": duration}}
        }))
        .unwrap()
    }

    /// The verdicts of TRACK_SCAN in `lang` against a file of `tracks`. Py: CompareTracks.verdicts.
    fn verdicts(lang: &str, tracks: &[(&str, &str, Option<i64>)], chapters: i64, seconds: i64) -> Verdicts {
        let scan = track_scan(lang);
        let file = ident(tracks, chapters, seconds);
        let want = expected_rows(&scan, 0, Selection::All).unwrap();
        checks(&want, &file_rows(&file).unwrap(), &scan.titles[&0], &file).unwrap().0
    }

    fn streams(rows: &[ExpectedRow]) -> Vec<u32> {
        rows.iter().map(|row| row.stream).collect()
    }

    #[test]
    fn a_matching_file_passes_every_check() {
        let v = verdicts("eng", &[("video", "und", None), ("audio", "eng", Some(8)), ("audio", "eng", Some(2))], 2, 600);
        assert_eq!(v, Verdicts { count: true, order: true, video: true, chapters: true, duration: true });
    }

    #[test]
    fn same_language_audio_tracks_in_the_wrong_order_fail() {
        let v = verdicts("eng", &[("video", "und", None), ("audio", "eng", Some(2)), ("audio", "eng", Some(8))], 2, 600);
        assert!(v.count);
        assert!(!v.order);
    }

    #[test]
    fn both_iso_639_2_forms_name_the_same_language() {
        let v = verdicts("fra", &[("video", "und", None), ("audio", "fre", Some(8)), ("audio", "fre", Some(2))], 2, 600);
        assert!(v.order);
    }

    #[test]
    fn the_language_of_a_video_track_is_not_compared() {
        let v = verdicts("eng", &[("video", "eng", None), ("audio", "eng", Some(8)), ("audio", "eng", Some(2))], 2, 600);
        assert!(v.order);
    }

    #[test]
    fn a_stream_makemkv_removed_as_empty_is_read_from_the_rip_log() {
        let removed = "Forced subtitles track #2 turned out to be empty and was removed from output file";
        let rip = RIP_4047_PAIR.replace("{removed}", removed);
        assert_eq!(removed_streams(&rip), BTreeSet::from([2_i64]));
    }

    #[test]
    fn a_removed_stream_is_not_expected_in_the_file() {
        let scan = track_scan("eng");
        let file = ident(&[("video", "und", None), ("audio", "eng", Some(8))], 2, 600);
        let (want, dropped) = without_removed(&expected_rows(&scan, 0, Selection::All).unwrap(), &BTreeSet::from([2_i64]));
        assert_eq!(streams(&dropped), [2_u32]);
        // MSG:4047 counts the selected streams, not the scan's: with the first stream deselected, output track 1 is
        // scan stream 2.
        let rows = expected_rows(&scan, 0, Selection::All).unwrap()[1..].to_vec();
        let (want2, dropped2) = without_removed(&rows, &BTreeSet::from([1_i64]));
        assert_eq!(streams(&dropped2), [2_u32]);
        let others: Vec<u32> = rows.iter().filter(|row| row.stream != 2).map(|row| row.stream).collect();
        assert_eq!(streams(&want2), others);
        let (v, _) = checks(&want, &file_rows(&file).unwrap(), &scan.titles[&0], &file).unwrap();
        assert!(v.count);
        assert!(v.order);
    }

    #[test]
    fn chapter_count_and_duration_are_checked() {
        let v = verdicts("eng", &[("video", "und", None), ("audio", "eng", Some(8)), ("audio", "eng", Some(2))], 1, 500);
        assert!(v.order);
        assert!(!v.chapters);
        assert!(!v.duration);
    }

    #[test]
    fn a_stream_without_a_language_code_is_und_not_the_metadata_language() {
        // Attribute 28 is the disc's metadata language ("eng" even on Chinese subtitles); only attribute 3 names a
        // stream's language, and a stream without one has no language (spec 8.6).
        let scan = parse(
            r#"TINFO:0,8,0,"2"
TINFO:0,9,0,"0:10:00"
SINFO:0,0,1,6201,"Video"
SINFO:0,0,6,0,"MpegH"
SINFO:0,0,28,0,"eng"
SINFO:0,1,1,6202,"Audio"
SINFO:0,1,6,0,"DD"
SINFO:0,1,14,0,"2"
SINFO:0,1,28,0,"eng"
SINFO:0,2,1,6203,"Subtitles"
SINFO:0,2,3,0,"zho"
SINFO:0,2,6,0,"PGS"
SINFO:0,2,28,0,"eng"
SINFO:0,3,1,6203,"Subtitles"
SINFO:0,3,6,0,"PGS"
SINFO:0,3,28,0,"eng"
"#,
        );
        let want = expected_rows(&scan, 0, Selection::All).unwrap();
        assert_eq!(want.iter().map(|row| row.lang.as_str()).collect::<Vec<_>>(), ["eng", "und", "zho", "und"]);
        let eng = expected_rows(&scan, 0, Selection::Eng).unwrap();
        assert_eq!(streams(&eng), [0_u32, 1]);
        let file = ident(&[("video", "und", None), ("audio", "und", Some(2))], 2, 600);
        let (v, _) = checks(&eng, &file_rows(&file).unwrap(), &scan.titles[&0], &file).unwrap();
        assert!(v.order);
    }

    #[test]
    fn an_empty_file_against_an_empty_title_fails() {
        let scan = parse("TINFO:0,9,0,\"0:10:00\"\n");
        let file = ident(&[], 0, 0);
        let (v, _) = checks(&[], &file_rows(&file).unwrap(), &scan.titles[&0], &file).unwrap();
        assert_eq!(v, Verdicts { count: false, order: false, video: false, chapters: true, duration: false });
    }

    // New in the port.

    #[test]
    fn the_eng_selection_keeps_what_the_python_keeps() {
        // expected_rows(scan, 3, "eng") and (scan, 3, "all") as the Python gave them on SCAN_B: 3D video is never
        // expected; eng drops audio outside eng/und or with flag 256 or 1|2|4, and subtitles outside eng.
        let scan = parse(SCAN_B);
        assert_eq!(streams(&expected_rows(&scan, 3, Selection::Eng).unwrap()), [0_u32, 2, 7]);
        let all = expected_rows(&scan, 3, Selection::All).unwrap();
        assert_eq!(streams(&all), [0_u32, 2, 3, 4, 5, 6, 7]);
        assert_eq!(
            all.iter().map(|row| row.lang.as_str()).collect::<Vec<_>>(),
            ["und", "eng", "eng", "und", "spa", "fra", "eng"]
        );
        assert_eq!(all.iter().map(|row| row.flags).collect::<Vec<_>>(), [0_i64, 0, 256, 4, 0, 0, 0]);
        assert!(expected_rows(&scan, 0, Selection::All).unwrap().is_empty());
    }

    #[test]
    fn flags_that_are_not_a_number_are_an_error() {
        // int(a.get(22, "0") or 0) raised ValueError in the Python; exit 2 in the Rust.
        let scan = parse(&format!("{}SINFO:0,2,22,0,\"x\"\n", TRACK_SCAN.replace("{lang}", "eng")));
        assert_eq!(
            expected_rows(&scan, 0, Selection::All).unwrap_err(),
            "scan stream 0,2: attribute 22: invalid literal for int() with base 10: 'x'"
        );
    }

    #[test]
    fn an_unreadable_chapter_count_or_duration_is_an_error() {
        // int() and duration_seconds raised ValueError in the Python's checks; exit 2 in the Rust.
        let file = ident(&[("video", "und", None)], 2, 600);
        let scan = parse("TINFO:0,8,0,\"x\"\nTINFO:0,9,0,\"1:00\"\n");
        assert_eq!(
            checks(&[], &[], &scan.titles[&0], &file).unwrap_err(),
            "the title's chapter count (attribute 8): invalid literal for int() with base 10: 'x'"
        );
        let scan = parse("TINFO:0,8,0,\"2\"\nTINFO:0,9,0,\"1:00\"\n");
        assert_eq!(
            checks(&[], &[], &scan.titles[&0], &file).unwrap_err(),
            "the title's duration (attribute 9) is not H:MM:SS: '1:00'"
        );
    }

    #[test]
    fn a_language_property_that_is_not_a_string_is_an_error() {
        // The Python crashed on these (`.lower()` of an int; the table's `:<4` of None); exit 2 in the Rust.
        for (language, shown) in [(json!(5), "5"), (json!(null), "None")] {
            let file: MkvmergeIdent = serde_json::from_value(json!({
                "tracks": [{"id": 0, "type": "audio", "codec": "x", "properties": {"language": language}}]
            }))
            .unwrap();
            assert_eq!(
                file_rows(&file).unwrap_err(),
                format!("track 0: its language property is not a string: {shown}")
            );
        }
    }

    /// The file of SCAN_B's rip: video, English 5.1 audio, two English subtitle tracks. New in the port.
    const IDENT_B: &str = r#"{"tracks": [{"id": 0, "type": "video", "codec": "MPEG-1/2", "properties": {"language": "und"}}, {"id": 1, "type": "audio", "codec": "AC-3", "properties": {"language": "eng", "audio_channels": 6}}, {"id": 2, "type": "subtitles", "codec": "VobSub", "properties": {"language": "eng"}}, {"id": 3, "type": "subtitles", "codec": "VobSub", "properties": {"language": "eng"}}], "chapters": [{"num_entries": 12}], "container": {"properties": {"duration": 5404060000000}}}"#;

    /// What compare_tracks.py printed (exit 1) for `scan.txt 3 mkvmerge.json --selection eng` on SCAN_B and IDENT_B
    /// with no rip log beside them, run once while the port was planned.
    const REPORT_B: [&str; 15] = [
        "scan                                                   | file",
        " 0 video     und  Mpeg2        out=V_MPEG2  ch=   f=0  |  0 video     und  MPEG-1/2 ch=",
        " 2 audio     eng  DD           out=A_AC3    ch=6  f=0  |  1 audio     eng  AC-3 ch=6",
        " 7 subtitles eng  VobSub       out=S_VOBSUB ch=   f=0  |  2 subtitles eng  VobSub ch=",
        "                                                       |  3 subtitles eng  VobSub ch=",
        "",
        "RIP LOG          none found: tracks MakeMKV removed (MSG:4047) are not accounted for",
        "TRACK COUNT      scan=3 file=4",
        "ORDER MATCH      no (type, language except on video, audio channels; position by position)",
        "CHAPTERS         scan=12 file=12",
        "DURATION         scan=5405s file=5404.1s",
        "VIDEO TRACKS     1",
        "STATISTICS TAGS  absent",
        "VERDICT          count=no order=no video=yes chapters=yes duration=yes",
        "                 codecs are not compared: read the scan's out= column against the file's codec column",
    ];

    /// A video stream, then a stream without attribute 1 (no type). New in the port.
    const SCAN_E: &str = r#"TINFO:0,9,0,"0:10:00"
SINFO:0,0,1,6201,"Video"
SINFO:0,0,6,0,"MpegH"
SINFO:0,1,3,0,"eng"
SINFO:0,1,6,0,"PGS"
"#;

    /// MakeMKV's MSG:4047 for an empty forced-only subtitle track it removed as output track {n}. Synthetic.
    const RIP_REMOVING: &str = r#"MSG:4047,16,3,"AV sync issue in stream {n} at 0:00:00 : Forced subtitles track #{n} turned out to be empty and was removed from output file","AV sync issue in stream %1 at %2 : %3","{n}","0:00:00","Forced subtitles track #{n} turned out to be empty and was removed from output file"
"#;

    /// What compare_tracks.py printed (exit 0) for SCAN_E title 0 against one video track (no chapters, 600 s), with
    /// RIP_REMOVING (n = 1) as rip.txt beside mkvmerge.json, run once while the port was planned.
    const REPORT_E: [&str; 13] = [
        "scan                                                   | file",
        " 0 video     und  MpegH        out=         ch=   f=0  |  0 video     und  x ch=",
        "",
        "RIP LOG          rip.txt",
        "REMOVED          scan stream 1 (None eng PGS f=0): MakeMKV left it out of the file (MSG:4047), so it is not expected",
        "TRACK COUNT      scan=1 file=1",
        "ORDER MATCH      yes (type, language except on video, audio channels; position by position)",
        "CHAPTERS         scan=0 file=0",
        "DURATION         scan=600s file=600.0s",
        "VIDEO TRACKS     1",
        "STATISTICS TAGS  absent",
        "VERDICT          count=yes order=yes video=yes chapters=yes duration=yes",
        "                 codecs are not compared: read the scan's out= column against the file's codec column",
    ];

    #[test]
    fn the_report_lines_are_the_pythons() {
        let scan = parse(SCAN_B);
        let ident: MkvmergeIdent = serde_json::from_str(IDENT_B).unwrap();
        let report = reconcile(&scan, 3, &ident, Selection::Eng, None).unwrap();
        assert_eq!(report_lines(&report), REPORT_B);
        assert_eq!(report.exit_code, 1);
        assert_eq!(report.verdicts, Verdicts { count: false, order: false, video: true, chapters: true, duration: true });
    }

    #[test]
    fn a_kept_stream_without_a_type_is_an_error_and_a_removed_one_prints_none() {
        // The Python's table formats the type with `:<9`, which raises on None; its REMOVED line prints str(None).
        let scan = parse(SCAN_E);
        let file = ident(&[("video", "und", None)], 0, 600);
        assert_eq!(
            reconcile(&scan, 0, &file, Selection::All, None).unwrap_err(),
            "scan stream 0,1 has no type (attribute 1)"
        );
        let rip = RIP_REMOVING.replace("{n}", "1");
        let log = RipLog { shown: "rip.txt".to_string(), text: &rip };
        let report = reconcile(&scan, 0, &file, Selection::All, Some(log)).unwrap();
        assert_eq!(report_lines(&report), REPORT_E);
        assert_eq!(report.exit_code, 0);
    }

    #[test]
    fn a_lone_cr_in_the_rip_log_ends_a_line() {
        // Two removals (output tracks 5 and 1) separated and ended by a lone CR. Pinned to d3a754d's
        // compare_tracks.py run on SCAN_E, this rip.txt and one video track (exit 0): both lines are read, so scan
        // stream 1 is REMOVED and track 5 is beyond the prediction.
        let scan = parse(SCAN_E);
        let file = ident(&[("video", "und", None)], 0, 600);
        let line = |n: &str| RIP_REMOVING.replace("{n}", n).trim_end().to_string();
        let rip = format!("{}\r{}\r", line("5"), line("1"));
        let log = RipLog { shown: "rip.txt".to_string(), text: &rip };
        let report = reconcile(&scan, 0, &file, Selection::All, Some(log)).unwrap();
        let mut expected: Vec<&str> = REPORT_E.to_vec();
        expected.insert(5, "REMOVED          MSG:4047 names output track 5, which the prediction does not have");
        assert_eq!(report_lines(&report), expected);
        assert_eq!(report.unknown_removed, [5]);
        assert_eq!(report.exit_code, 0);
    }

    #[test]
    fn a_title_the_scan_does_not_list_is_an_error() {
        // scan.titles[args.title] raised KeyError in the Python after it had printed the table; exit 2 in the Rust.
        let file = ident(&[("video", "und", None)], 2, 600);
        assert_eq!(
            reconcile(&track_scan("eng"), 5, &file, Selection::All, None).unwrap_err(),
            "the scan lists no title 5"
        );
    }

    #[test]
    fn the_report_serialises_with_the_pythons_keys_in_order() {
        // reconcile.json (Task 14) and decide's "reconcile" (Task 13) are this serde form.
        let scan = parse(SCAN_B);
        let ident: MkvmergeIdent = serde_json::from_str(IDENT_B).unwrap();
        let report = reconcile(&scan, 3, &ident, Selection::Eng, None).unwrap();
        let value = serde_json::to_value(&report).unwrap();
        let keys = |v: &serde_json::Value| v.as_object().unwrap().keys().cloned().collect::<Vec<String>>();
        assert_eq!(
            keys(&value),
            [
                "title", "selection", "rip_log", "want", "dropped", "unknown_removed", "got", "verdicts", "numbers",
                "video_tracks", "statistics_tags", "exit_code"
            ]
        );
        assert_eq!(keys(&value["want"][0]), ["stream", "type", "lang", "lang_raw", "codec", "out", "ch", "flags"]);
        assert_eq!(keys(&value["got"][0]), ["id", "type", "lang", "lang_raw", "codec", "ch", "tags"]);
        assert_eq!(keys(&value["verdicts"]), ["count", "order", "video", "chapters", "duration"]);
        assert_eq!(keys(&value["numbers"]), ["scan_chapters", "file_chapters", "scan_seconds", "file_seconds"]);
        assert_eq!(value["selection"], "eng");
        assert_eq!(value["rip_log"], serde_json::Value::Null);
        assert_eq!(value["numbers"]["file_seconds"], 5404.06);
        assert_eq!(serde_json::from_value::<Report>(value).unwrap(), report);
    }
}
