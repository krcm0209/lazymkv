//! `lazymkv-dev ocr-compare <tess> <ocrs> [--rec DIR] [--tag CODE]`: Tesseract's and ocrs's text of the same subtitle
//! frames, side by side (the ocrs trial; decisions D17, D19, D22). Py: ocr_compare, its lines verbatim. It prints
//! numbers only, never subtitle text: the frames' lines are held in memory, folded, compared and measured. Exit 0 when
//! frames were compared; 2 when an input cannot be read, no frame is in both files, or an argument is wrong.

use crate::evidence::{os_error_text, Ctx};
use crate::{usage_error, Args};
use lazymkv_core::recording::files::{read_jsonl, FrameEntry, OcrSummary};
use lazymkv_core::recording::{
    decode_text, pathlib_str, py_fixed, py_float_value, py_int_value, py_repr, py_split, py_str, py_strip, py_truthy, Encoding, Errors,
    Ordered,
};
use lazymkv_core::subtitle::text::{correct, fold, measure, split_joined, Event, Measures, Rule, Track};
use serde_json::{Map, Value};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::Path;

/// The command's name as typed.
const NAME: &str = "ocr-compare";
/// argparse's usage line, under the command's name.
const USAGE: &str = "ocr-compare [-h] [--rec REC] [--tag TAG] tesseract ocrs";
/// The positional arguments, as argparse names them.
const POSITIONALS: [&str; 2] = ["tesseract", "ocrs"];
/// What -h prints: argparse's help of ocr_compare at 80 columns.
const HELP: &str = "usage: lazymkv-dev ocr-compare [-h] [--rec REC] [--tag TAG] tesseract ocrs

Tesseract's and ocrs's text of the same subtitle frames, side by side (the
ocrs trial; decisions D17, D19, D22).

positional arguments:
  tesseract   ocr-subs.ps1's sub-<n>.ocr.jsonl
  ocrs        ocrs-trial.ps1's ocrs-s<n>.jsonl for the same stream

optional arguments:
  -h, --help  show this help message and exit
  --rec REC   the recording folder of that reading
              (corpus/owner/<slug>/<label>/subs-ocr)
  --tag TAG   the stream's language tag (default eng: Tesseract's corrections
              apply)
";

/// How many of the most different frames are listed. Py: SHOW_WORST.
pub const SHOW_WORST: usize = 10;
/// A character error rate above this counts as high. Py: CER_HIGH.
pub const CER_HIGH: f64 = 0.20;
/// Per-frame fields of ocrs-trial's output that read_frames keeps (times in ms; crop is [x, y, w, h] or null).
/// Py: OCRS_FIELDS.
pub const OCRS_FIELDS: [&str; 6] = ["ms", "decode_ms", "detect_ms", "rec_ms", "crop", "crop_ms"];

/// The report's fixed lines.
const MARKERS_HEAD: &str = "markers (subtitle_text.measure, each frame with text one event):";
const WORST_HEAD: &str = "largest differences (frame, file, characters tesseract/ocrs, lines tesseract/ocrs, CER):";
const NO_DIFFERENCE: &str = "  none: every frame with text reads the same";
/// The speed line's parts without a time.
const TESSERACT_NONE: &str = "tesseract n/a (no --rec summary.json entry)";
const OCRS_NONE: &str = "ocrs n/a (no summary)";
/// What ocrs's per-frame time covered, with and without decode_ms in the records.
const OCR_ONLY: &str = "OCR";
const DECODE_AND_OCR: &str = "decode + OCR (an output from before --crop)";

/// One frame record as read_frames keeps it. `lines` holds subtitle text: in memory only, never printed.
/// Py: read_frames' dicts.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// r.get('frame').
    pub frame: Option<i64>,
    /// r.get('file').
    pub file: Option<String>,
    /// The frame's lines (none when it has no text).
    pub lines: Vec<String>,
    /// The OCRS_FIELDS the record has, with their values; presence is what ocrs_frames_line asks.
    pub fields: Map<String, Value>,
}

/// Edit distance (insert, delete, substitute; each costs 1) between two strings, over characters.
/// Py: ocr_compare.levenshtein.
pub fn levenshtein(a: &str, b: &str) -> usize {
    let (mut a, mut b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    if a.len() < b.len() {
        std::mem::swap(&mut a, &mut b);
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let substitute = prev[j] + usize::from(ca != cb);
            cur.push((prev[j + 1] + 1).min(cur[j] + 1).min(substitute));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// Levenshtein distance over the longer text's length: 0.0 for two equal texts, 1.0 when one is empty.
/// Py: ocr_compare.cer.
pub fn cer(a: &str, b: &str) -> f64 {
    let longer = a.chars().count().max(b.chars().count());
    if longer == 0 {
        0.0
    } else {
        levenshtein(a, b) as f64 / longer as f64
    }
}

/// The lines as subtitle_text reads them: folded to ASCII, then corrected for English (Tesseract's '|' for I) or with
/// '|' taken as a space otherwise; spaces collapsed, empty lines left out. Py: ocr_compare.norm_lines.
pub fn norm_lines(lines: &[String], english: bool) -> Vec<String> {
    let mut counts: Ordered<usize> = Ordered::new();
    let mut out = Vec::new();
    for line in lines {
        let folded = fold(line);
        let fixed = if english { correct(&folded, &mut counts) } else { folded.replace('|', " ") };
        let joined = py_split(&fixed).join(" ");
        if !joined.is_empty() {
            out.push(joined);
        }
    }
    out
}

/// One string per frame: the normalised lines joined with a space, so that a different line split costs nothing.
/// Py: ocr_compare.norm_text.
pub fn norm_text(lines: &[String], english: bool) -> String {
    norm_lines(lines, english).join(" ")
}

/// A file's text as Path.read_text(encoding="utf-8-sig") gives it; Err is the OSError or UnicodeDecodeError text,
/// the path shown as `shown`.
fn read_file_text(path: &Path, shown: &str) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|error| os_error_text(&error, shown))?;
    decode_text(&bytes, Encoding::Utf8Sig, Errors::Strict)
}

/// (frames, summary) of a JSON-lines text: the frames in file order, with any OCRS_FIELDS the record has, and the
/// last `{"summary": {...}}` line of ocrs-trial, or None. A record with a "lines" array is taken as it is (each item
/// str()-ed and stripped, blanks dropped); an older Tesseract record with only joined "text" is split with
/// split_joined and the frame's line count from `line_counts`, if known. Py: ocr_compare.read_frames, after the read.
pub fn frames_from_text(text: &str, line_counts: &BTreeMap<i64, Option<i64>>) -> Result<(Vec<Frame>, Option<Map<String, Value>>), String> {
    let mut frames = Vec::new();
    let mut summary = None;
    for record in read_jsonl(text)? {
        if let Some(found) = record.summary {
            summary = Some(found);
            continue;
        }
        let lines: Vec<String> = match &record.lines {
            Some(Value::Array(items)) => {
                items.iter().map(|item| py_strip(&py_str(item)).to_string()).filter(|line| !line.is_empty()).collect()
            }
            _ => {
                let joined = py_strip(record.text.as_deref().unwrap_or(""));
                if joined.is_empty() {
                    Vec::new()
                } else {
                    let nlines = record.frame.and_then(|frame| line_counts.get(&frame).copied().flatten());
                    split_joined(joined, nlines)
                }
            }
        };
        let mut fields = Map::new();
        for key in OCRS_FIELDS {
            if let Some(value) = record.rest.get(key) {
                fields.insert(key.to_string(), value.clone());
            }
        }
        frames.push(Frame { frame: record.frame, file: record.file, lines, fields });
    }
    Ok((frames, summary))
}

/// frames_from_text of the file at `path` (read as "utf-8-sig"), its errors naming the path as `shown`.
/// Py: ocr_compare.read_frames.
pub fn read_frames(path: &Path, shown: &str, line_counts: &BTreeMap<i64, Option<i64>>) -> Result<(Vec<Frame>, Option<Map<String, Value>>), String> {
    frames_from_text(&read_file_text(path, shown)?, line_counts)
}

/// What two records are paired by: the file name, or the frame number when a record lacks a name.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum PairKey {
    File(Option<String>),
    Frame(Option<i64>),
}

/// r.get(key) for the pairing method.
fn pair_key(record: &Frame, by_file: bool) -> PairKey {
    if by_file {
        PairKey::File(record.file.clone())
    } else {
        PairKey::Frame(record.frame)
    }
}

/// (pairs, method, only_a, only_b): pairs of records of the same frame, in a's order (b's last record of a key wins).
/// By file name when every record has one, else by frame number. Py: ocr_compare.pair_frames.
pub fn pair_frames<'f>(a: &'f [Frame], b: &'f [Frame]) -> (Vec<(&'f Frame, &'f Frame)>, &'static str, usize, usize) {
    let by_file = a.iter().chain(b.iter()).all(|record| record.file.as_deref().map_or(false, |file| !file.is_empty()));
    let method = if by_file { "file name" } else { "frame number" };
    let mut by_key: BTreeMap<PairKey, &'f Frame> = BTreeMap::new();
    for record in b {
        by_key.insert(pair_key(record, by_file), record);
    }
    let pairs: Vec<(&'f Frame, &'f Frame)> =
        a.iter().filter_map(|record| by_key.get(&pair_key(record, by_file)).map(|other| (record, *other))).collect();
    let keys_a: BTreeSet<PairKey> = a.iter().map(|record| pair_key(record, by_file)).collect();
    let only_b = b.iter().filter(|record| !keys_a.contains(&pair_key(record, by_file))).count();
    let only_a = a.len() - pairs.len();
    (pairs, method, only_a, only_b)
}

/// One compared frame: lengths, line counts and the error rate of the normalised texts. Py: compare's row dicts.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub frame: Option<i64>,
    /// The file name, "-" without one.
    pub file: String,
    /// Characters of each engine's normalised text.
    pub a: usize,
    pub b: usize,
    /// Lines of each engine's normalised text.
    pub la: usize,
    pub lb: usize,
    pub equal: bool,
    pub cer: f64,
}

/// Every number the report prints. Py: compare's dict.
#[derive(Clone, Debug, PartialEq)]
pub struct Compared {
    pub pairs: usize,
    pub method: &'static str,
    pub only_tess: usize,
    pub only_ocrs: usize,
    pub text_tess: usize,
    pub text_ocrs: usize,
    pub both: usize,
    pub either: usize,
    pub neither: usize,
    pub exact: usize,
    pub cer_mean: f64,
    pub cer_median: f64,
    pub cer_high: usize,
    pub same_lines: usize,
    pub worst: Vec<Row>,
    pub m_tess: Measures,
    pub m_ocrs: Measures,
}

/// Every number the report prints, from two lists of frame records. Py: ocr_compare.compare.
pub fn compare(tess: &[Frame], ocrs: &[Frame], tag: &str) -> Compared {
    let english = tag == "eng" || tag == "en";
    let (pairs, method, only_tess, only_ocrs) = pair_frames(tess, ocrs);
    let mut rows = Vec::new();
    for (t, o) in &pairs {
        let (a, b) = (norm_text(&t.lines, english), norm_text(&o.lines, english));
        rows.push(Row {
            frame: t.frame,
            file: t.file.clone().filter(|file| !file.is_empty()).unwrap_or_else(|| "-".to_string()),
            a: a.chars().count(),
            b: b.chars().count(),
            la: norm_lines(&t.lines, english).len(),
            lb: norm_lines(&o.lines, english).len(),
            equal: a == b,
            cer: cer(&a, &b),
        });
    }
    let either: Vec<&Row> = rows.iter().filter(|r| r.a > 0 || r.b > 0).collect();
    let both: Vec<&Row> = rows.iter().filter(|r| r.a > 0 && r.b > 0).collect();
    let cers: Vec<f64> = either.iter().map(|r| r.cer).collect();
    let mut worst: Vec<Row> = either.iter().filter(|r| r.cer > 0.0).map(|r| Row::clone(r)).collect();
    // sorted(key=(-cer, -|a - b|)) is stable: the largest error first, then the largest length difference.
    worst.sort_by(|x, y| {
        y.cer.partial_cmp(&x.cer).unwrap_or(Ordering::Equal).then_with(|| y.a.abs_diff(y.b).cmp(&x.a.abs_diff(x.b)))
    });
    worst.truncate(SHOW_WORST);
    let tess_frames: Vec<&Frame> = pairs.iter().map(|(t, _)| *t).collect();
    let ocrs_frames: Vec<&Frame> = pairs.iter().map(|(_, o)| *o).collect();
    Compared {
        pairs: pairs.len(),
        method,
        only_tess,
        only_ocrs,
        text_tess: rows.iter().filter(|r| r.a > 0).count(),
        text_ocrs: rows.iter().filter(|r| r.b > 0).count(),
        both: both.len(),
        either: either.len(),
        neither: rows.len() - either.len(),
        exact: either.iter().filter(|r| r.equal).count(),
        cer_mean: exact_mean(&cers).unwrap_or(0.0),
        cer_median: median(&cers).unwrap_or(0.0),
        cer_high: cers.iter().filter(|c| **c > CER_HIGH).count(),
        same_lines: both.iter().filter(|r| r.la == r.lb).count(),
        worst,
        m_tess: measure_engine(&tess_frames, tag),
        m_ocrs: measure_engine(&ocrs_frames, tag),
    }
}

/// subtitle_text's measure over one engine's frames with text, the k-th paired frame being an event at k seconds.
/// Py: ocr_compare.measure_engine.
pub fn measure_engine(frames: &[&Frame], tag: &str) -> Measures {
    let events: Vec<Event> = frames
        .iter()
        .enumerate()
        .filter(|(_, frame)| !frame.lines.is_empty())
        .map(|(k, frame)| Event { start: k as f64, dur: 0.0, lines: frame.lines.clone() })
        .collect();
    let mut track = Track::new(0, tag, None, false, events);
    measure(&mut track, &Rule::default()).clone()
}

/// '%.3f' % (k / n), or "n/a" for n == 0. Py: ocr_compare.rate.
fn rate(k: usize, n: usize) -> String {
    if n == 0 {
        "n/a".to_string()
    } else {
        py_fixed(k as f64 / n as f64, 3)
    }
}

/// One engine's marker counts and readability, numbers only. Py: ocr_compare.marker_line.
pub fn marker_line(m: &Measures) -> String {
    let shares: Vec<String> = m.shares.iter().map(|(language, share)| format!("{language} {}", py_fixed(*share, 3))).collect();
    format!(
        "events={} CUE={} LABEL={} CHEV={} NOTE={} ODD={} | M={} SO={} CS={} readable={} lang={} ({}) class={}",
        m.events,
        m.CUE,
        m.LABEL,
        m.CHEV,
        m.NOTE,
        m.ODD,
        m.M,
        m.SO,
        m.CS,
        if m.readable { "yes" } else { "no" },
        m.lang,
        shares.join(" "),
        m.class.as_str()
    )
}

/// The report's lines. Py: ocr_compare.report.
pub fn report_lines(c: &Compared) -> Vec<String> {
    let mut lines = vec![
        format!("paired by {}: {} frames; only in tesseract {}, only in ocrs {}", c.method, c.pairs, c.only_tess, c.only_ocrs),
        format!(
            "frames with text: tesseract {}, ocrs {}, both {}, either {}, neither {}",
            c.text_tess, c.text_ocrs, c.both, c.either, c.neither
        ),
        format!("exact match after folding: {} of {} frames with text in either ({})", c.exact, c.either, rate(c.exact, c.either)),
        format!(
            "character error rate (Levenshtein / longer text, folded): mean {}, median {} over {} frames; CER > {} on {} frames",
            py_fixed(c.cer_mean, 3),
            py_fixed(c.cer_median, 3),
            c.either,
            py_fixed(CER_HIGH, 2),
            c.cer_high
        ),
        format!("same line count: {} of {} frames with text in both ({})", c.same_lines, c.both, rate(c.same_lines, c.both)),
        MARKERS_HEAD.to_string(),
        format!("  tesseract: {}", marker_line(&c.m_tess)),
        format!("  ocrs:      {}", marker_line(&c.m_ocrs)),
        WORST_HEAD.to_string(),
    ];
    for r in &c.worst {
        let frame = r.frame.map_or_else(|| "None".to_string(), |frame| frame.to_string());
        lines.push(format!("  frame {frame} {} chars {}/{} lines {}/{} CER {}", r.file, r.a, r.b, r.la, r.lb, py_fixed(r.cer, 3)));
    }
    if c.worst.is_empty() {
        lines.push(NO_DIFFERENCE.to_string());
    }
    lines
}

/// The last component of a path as pathlib names it (empty and "." components dropped; on Windows "\\" separates
/// too). Py: Path(p).name.
fn path_name(text: &str) -> &str {
    text.split(|c: char| c == '/' || (cfg!(windows) && c == '\\')).filter(|part| !part.is_empty() && *part != ".").last().unwrap_or("")
}

/// The ASCII digits at the start of `text` (at least one) and what follows them.
fn leading_digits(text: &str) -> Option<(&str, &str)> {
    let end = text.find(|c: char| !c.is_ascii_digit()).unwrap_or(text.len());
    if end == 0 {
        None
    } else {
        Some(text.split_at(end))
    }
}

/// The stream number in sub-<n>.ocr.jsonl, ocrs-s<n>.jsonl or ocrs-s<n>-crop<pad>.jsonl, from the first name that
/// has one, or None. Py: ocr_compare.stream_number, re.match(r'(?:sub-|ocrs-s)(\d+)(?:-crop\d+)?\.', name).
pub fn stream_number(paths: &[&str]) -> Option<i64> {
    for path in paths {
        let name = path_name(path);
        let Some(rest) = name.strip_prefix("sub-").or_else(|| name.strip_prefix("ocrs-s")) else {
            continue;
        };
        let Some((digits, after)) = leading_digits(rest) else {
            continue;
        };
        let after = match after.strip_prefix("-crop").and_then(leading_digits) {
            Some((_, tail)) if tail.starts_with('.') => tail,
            _ => after,
        };
        if after.starts_with('.') {
            if let Ok(number) = digits.parse() {
                return Some(number);
            }
        }
    }
    None
}

/// The padding in ocrs-s<n>-crop<pad>.jsonl; 0 (the full frame) for any other name.
/// Py: ocr_compare.crop_number, re.match(r'ocrs-s\d+-crop(\d+)\.', name).
pub fn crop_number(path: &str) -> i64 {
    path_name(path)
        .strip_prefix("ocrs-s")
        .and_then(leading_digits)
        .and_then(|(_, rest)| rest.strip_prefix("-crop"))
        .and_then(leading_digits)
        .filter(|(_, tail)| tail.starts_with('.'))
        .and_then(|(digits, _)| digits.parse().ok())
        .unwrap_or(0)
}

/// The entry of stream `n` (and, when `crop` is given, of that crop; an entry without one is crop 0) in a summary
/// file's "streams" list (summary.json, ocrs-summary.json), or None; Err where the Python's int() or the read raised.
/// Py: ocr_compare.stream_entry.
pub fn stream_entry(path: &Path, n: Option<i64>, crop: Option<i64>) -> Result<Option<Map<String, Value>>, String> {
    let Some(n) = n else {
        return Ok(None);
    };
    if !path.is_file() {
        return Ok(None);
    }
    let summary: OcrSummary =
        serde_json::from_str(&read_file_text(path, &path.to_string_lossy())?).map_err(|error| error.to_string())?;
    for entry in summary.streams {
        let entry_n = match entry.get("n") {
            Some(value) => py_int_value(value)?,
            None => -1,
        };
        if entry_n != n {
            continue;
        }
        let crop_matches = match crop {
            None => true,
            Some(crop) => {
                let entry_crop = match entry.get("crop").filter(|value| py_truthy(value)) {
                    Some(value) => py_int_value(value)?,
                    None => 0,
                };
                entry_crop == crop
            }
        };
        if crop_matches {
            return Ok(Some(entry));
        }
    }
    Ok(None)
}

/// 'n/a' for None, else '%.<digits>f' % x. Py: ocr_compare.num.
fn num(x: Option<f64>, digits: usize) -> String {
    x.map_or_else(|| "n/a".to_string(), |x| py_fixed(x, digits))
}

/// bool() of an optional value (absent is false).
fn truthy(value: Option<&Value>) -> bool {
    value.map_or(false, py_truthy)
}

/// A value that is there and not null: Python's `x.get(k) is not None`.
fn present(value: Option<&Value>) -> bool {
    value.map_or(false, |value| !value.is_null())
}

/// statistics.mean of float() of the values that are not None; None when there is none. Py: ocr_compare.mean_of.
fn mean_of<'v>(values: impl Iterator<Item = Option<&'v Value>>) -> Result<Option<f64>, String> {
    let mut xs = Vec::new();
    for value in values.flatten() {
        if !value.is_null() {
            xs.push(py_float_value(value)?);
        }
    }
    Ok(exact_mean(&xs))
}

/// b[i] of a crop box ([x, y, w, h]); Err where the Python's indexing raised.
fn box_side(value: &Value, at: usize) -> Result<&Value, String> {
    match value {
        Value::Array(items) => items.get(at).ok_or_else(|| "list index out of range".to_string()),
        other => Err(format!("a crop box is not a list: {}", py_repr(other))),
    }
}

/// Where ocrs's time went, from its per-frame records: the OCR time per frame read, split into detection and
/// recognition, and for a cropped reading the boxes and the crop time; None when the records carry no time.
/// Py: ocr_compare.ocrs_frames_line.
pub fn ocrs_frames_line(frames: &[Frame]) -> Result<Option<String>, String> {
    if !frames.iter().any(|frame| frame.fields.contains_key("ms")) {
        return Ok(None);
    }
    let cropped = frames.iter().any(|frame| frame.fields.contains_key("crop"));
    let read: Vec<&Frame> =
        frames.iter().filter(|frame| frame.fields.contains_key("ms") && (truthy(frame.fields.get("crop")) || !cropped)).collect();
    let what = if frames.iter().any(|frame| frame.fields.contains_key("decode_ms")) { OCR_ONLY } else { DECODE_AND_OCR };
    let ms = num(mean_of(read.iter().map(|frame| frame.fields.get("ms")))?, 1);
    let mut parts = vec![format!("{what} {ms} ms mean over {} frames read", read.len())];
    if read.iter().any(|frame| frame.fields.contains_key("detect_ms")) {
        let detection = num(mean_of(read.iter().map(|frame| frame.fields.get("detect_ms")))?, 1);
        let recognition = num(mean_of(read.iter().map(|frame| frame.fields.get("rec_ms")))?, 1);
        parts.push(format!("detection {detection} ms, recognition {recognition} ms"));
    }
    if cropped {
        let boxes: Vec<&Value> = frames.iter().filter_map(|frame| frame.fields.get("crop")).filter(|value| py_truthy(value)).collect();
        let blank = frames.iter().filter(|frame| frame.fields.get("crop").is_some_and(|value| !py_truthy(value))).count();
        let widths = boxes.iter().map(|value| box_side(*value, 2)).collect::<Result<Vec<&Value>, String>>()?;
        let heights = boxes.iter().map(|value| box_side(*value, 3)).collect::<Result<Vec<&Value>, String>>()?;
        let width = num(mean_of(widths.into_iter().map(Some))?, 0);
        let height = num(mean_of(heights.into_iter().map(Some))?, 0);
        let crop_ms = num(mean_of(frames.iter().map(|frame| frame.fields.get("crop_ms")))?, 2);
        parts.push(format!(
            "crop: {} boxes, mean {width} x {height} px, {blank} frames without a dark pixel (not read), crop_ms mean {crop_ms}",
            boxes.len()
        ));
    }
    Ok(Some(format!("ocrs per frame: {}", parts.join("; "))))
}

/// Each engine's time per frame: Tesseract from summary.json (one call for every frame of the stream), ocrs from the
/// summary line of its output or, without one, from its entry (stream and crop) in ocrs-summary.json.
/// Py: ocr_compare.speed_line.
pub fn speed_line(rec: Option<&Path>, n: Option<i64>, ocrs_summary: Option<&Map<String, Value>>, crop: i64) -> Result<String, String> {
    let mut parts = Vec::new();
    let t = match rec {
        Some(rec) => stream_entry(&rec.join("summary.json"), n, None)?,
        None => None,
    };
    match t.filter(|t| !t.is_empty() && truthy(t.get("frames")) && present(t.get("tesseract_s"))) {
        Some(t) => {
            let seconds = py_float_value(&t["tesseract_s"])?;
            let frames = py_int_value(&t["frames"])?;
            if frames == 0 {
                // int() of a truthy value such as 0.5 is 0: the Python's ZeroDivisionError.
                return Err("float division by zero".to_string());
            }
            parts.push(format!(
                "tesseract {} ms/frame (summary.json: {} s for {} frames, one call)",
                py_fixed(1000.0 * seconds / frames as f64, 1),
                py_fixed(seconds, 1),
                frames
            ));
        }
        None => parts.push(TESSERACT_NONE.to_string()),
    }
    let o = match ocrs_summary.filter(|summary| !summary.is_empty()) {
        Some(summary) => Some(summary.clone()),
        None => match rec {
            Some(rec) => stream_entry(&rec.join("ocrs-summary.json"), n, Some(crop))?,
            None => None,
        },
    };
    match o.filter(|o| !o.is_empty() && present(o.get("ms_per_frame"))) {
        Some(o) => {
            let mut read = String::new();
            if present(o.get("ms_per_read_frame")) {
                let per_read = py_float_value(&o["ms_per_read_frame"])?;
                let read_frames = match o.get("read_frames") {
                    Some(value) => py_int_value(value)?,
                    None => 0,
                };
                read = format!(", {} ms per frame read ({read_frames} read)", py_fixed(per_read, 1));
            }
            let per_frame = py_float_value(&o["ms_per_frame"])?;
            let frames = match o.get("frames") {
                Some(value) => py_int_value(value)?,
                None => 0,
            };
            let pad = match o.get("crop_pad").filter(|value| py_truthy(value)) {
                Some(value) => format!(", crop pad {}", py_int_value(value)?),
                None => String::new(),
            };
            let load = match o.get("load_ms").filter(|value| py_truthy(value)) {
                Some(value) => py_float_value(value)?,
                None => 0.0,
            };
            parts.push(format!(
                "ocrs {} ms/frame over {frames} frames{read}{pad} (+ {} ms model load)",
                py_fixed(per_frame, 1),
                py_fixed(load, 1)
            ));
        }
        None => parts.push(OCRS_NONE.to_string()),
    }
    Ok(format!("speed: {}", parts.join("; ")))
}

/// What main computed inside its try before printing. Py: main's locals.
struct Outcome {
    tess_records: usize,
    ocrs_records: usize,
    summary_line: bool,
    compared: Compared,
    speed: String,
    ocrs_line: Option<String>,
}

/// main's try block: the line counts of sub-<n>.frames.json, both readings, the comparison, the speed line and the
/// ocrs per-frame line; Err is what "cannot read the input: " is followed by.
fn read_and_compare(ctx: &Ctx, tess_arg: &str, ocrs_arg: &str, rec: Option<&Path>, tag: &str) -> Result<Outcome, String> {
    let n = stream_number(&[tess_arg, ocrs_arg]);
    let mut counts: BTreeMap<i64, Option<i64>> = BTreeMap::new();
    if let (Some(rec), Some(n)) = (rec, n) {
        let frames_json = rec.join(format!("sub-{n}.frames.json"));
        if frames_json.is_file() {
            let text = read_file_text(&frames_json, &frames_json.to_string_lossy())?;
            let entries: Vec<FrameEntry> = serde_json::from_str(&text).map_err(|error| error.to_string())?;
            for entry in entries {
                counts.insert(entry.frame, entry.lines);
            }
        }
    }
    let (tess, _) = read_frames(&ctx.abs(tess_arg), &pathlib_str(tess_arg), &counts)?;
    let (ocrs, summary) = read_frames(&ctx.abs(ocrs_arg), &pathlib_str(ocrs_arg), &BTreeMap::new())?;
    let compared = compare(&tess, &ocrs, tag);
    let speed = speed_line(rec, n, summary.as_ref(), crop_number(ocrs_arg))?;
    let ocrs_line = ocrs_frames_line(&ocrs)?;
    Ok(Outcome {
        tess_records: tess.len(),
        ocrs_records: ocrs.len(),
        summary_line: summary.as_ref().is_some_and(|summary| !summary.is_empty()),
        compared,
        speed,
        ocrs_line,
    })
}

/// `lazymkv-dev ocr-compare <tess> <ocrs> [--rec DIR] [--tag CODE]`. Py: ocr_compare.main.
pub fn run(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    if args.iter().take_while(|arg| arg.as_str() != "--").any(|arg| arg == "-h" || arg == "--help") {
        let _ = write!(out, "{HELP}");
        return 0;
    }
    let mut parsed = Args::new(args, &["--rec", "--tag"]);
    let rec = match parsed.flag("--rec") {
        Ok(value) => value,
        Err(message) => return usage_error(err, NAME, USAGE, &message),
    };
    let tag = match parsed.flag("--tag") {
        Ok(value) => value.unwrap_or_else(|| "eng".to_string()),
        Err(message) => return usage_error(err, NAME, USAGE, &message),
    };
    let words = match parsed.positionals() {
        Ok(words) => words,
        Err(message) => return usage_error(err, NAME, USAGE, &message),
    };
    if words.len() < POSITIONALS.len() {
        let message = format!("the following arguments are required: {}", POSITIONALS[words.len()..].join(", "));
        return usage_error(err, NAME, USAGE, &message);
    }
    if words.len() > POSITIONALS.len() {
        let message = format!("unrecognized arguments: {}", words[POSITIONALS.len()..].join(" "));
        return usage_error(err, NAME, USAGE, &message);
    }
    let (tess_arg, ocrs_arg) = (words[0].as_str(), words[1].as_str());
    // `if a.rec`: an empty --rec names no recording folder.
    let rec_dir = rec.filter(|rec| !rec.is_empty()).map(|rec| ctx.abs(&rec));
    let outcome = match read_and_compare(ctx, tess_arg, ocrs_arg, rec_dir.as_deref(), &tag) {
        Ok(outcome) => outcome,
        Err(message) => {
            let _ = writeln!(err, "cannot read the input: {message}");
            return 2;
        }
    };
    if outcome.compared.pairs == 0 {
        let _ = writeln!(
            err,
            "no frame is in both files ({} tesseract records, {} ocrs records)",
            outcome.tess_records, outcome.ocrs_records
        );
        return 2;
    }
    let _ = writeln!(
        out,
        "tesseract: {tess_arg}: {} frames; ocrs: {ocrs_arg}: {} frames (summary line: {})",
        outcome.tess_records,
        outcome.ocrs_records,
        if outcome.summary_line { "yes" } else { "no" }
    );
    for line in report_lines(&outcome.compared) {
        let _ = writeln!(out, "{line}");
    }
    let _ = writeln!(out, "{}", outcome.speed);
    if let Some(line) = &outcome.ocrs_line {
        let _ = writeln!(out, "{line}");
    }
    0
}

/// A non-negative integer of any size, for statistics.mean's exact sum: 64-bit limbs, least significant first, no
/// zero limb at the top.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Big(Vec<u64>);

impl Big {
    fn zero() -> Big {
        Big(Vec::new())
    }

    /// `m * 2^shift`.
    fn shifted(m: u64, shift: u32) -> Big {
        let mut limbs = vec![0u64; (shift / 64) as usize];
        let bits = shift % 64;
        if bits == 0 {
            limbs.push(m);
        } else {
            limbs.push(m << bits);
            limbs.push(m >> (64 - bits));
        }
        Big(limbs).trimmed()
    }

    /// Without zero limbs at the top.
    fn trimmed(mut self) -> Big {
        while self.0.last() == Some(&0) {
            self.0.pop();
        }
        self
    }

    /// `self += other`.
    fn add(&mut self, other: &Big) {
        if self.0.len() < other.0.len() {
            self.0.resize(other.0.len(), 0);
        }
        let mut carry = false;
        for (at, limb) in self.0.iter_mut().enumerate() {
            let (sum, first) = limb.overflowing_add(other.0.get(at).copied().unwrap_or(0));
            let (sum, second) = sum.overflowing_add(u64::from(carry));
            *limb = sum;
            carry = first || second;
        }
        if carry {
            self.0.push(1);
        }
    }

    /// `self - other`, for `self >= other`.
    fn minus(&self, other: &Big) -> Big {
        let mut limbs = self.0.clone();
        let mut borrow = false;
        for (at, limb) in limbs.iter_mut().enumerate() {
            let (difference, first) = limb.overflowing_sub(other.0.get(at).copied().unwrap_or(0));
            let (difference, second) = difference.overflowing_sub(u64::from(borrow));
            *limb = difference;
            borrow = first || second;
        }
        Big(limbs).trimmed()
    }

    /// The order of two values.
    fn compare(&self, other: &Big) -> Ordering {
        self.0.len().cmp(&other.0.len()).then_with(|| self.0.iter().rev().cmp(other.0.iter().rev()))
    }

    /// The number of bits (0 for zero).
    fn bits(&self) -> u64 {
        match self.0.last() {
            None => 0,
            Some(top) => (self.0.len() as u64 - 1) * 64 + u64::from(64 - top.leading_zeros()),
        }
    }

    /// `self * 2^shift`.
    fn shl(&self, shift: u64) -> Big {
        let mut limbs = vec![0u64; (shift / 64) as usize];
        let bits = (shift % 64) as u32;
        if bits == 0 {
            limbs.extend_from_slice(&self.0);
        } else {
            let mut carry = 0u64;
            for &limb in &self.0 {
                limbs.push((limb << bits) | carry);
                carry = limb >> (64 - bits);
            }
            limbs.push(carry);
        }
        Big(limbs).trimmed()
    }

    /// `(self / n, self % n)`, for `n > 0`.
    fn div_rem(&self, n: u64) -> (Big, u64) {
        let mut quotient = vec![0u64; self.0.len()];
        let mut rest: u128 = 0;
        for at in (0..self.0.len()).rev() {
            let current = (rest << 64) | u128::from(self.0[at]);
            quotient[at] = (current / u128::from(n)) as u64;
            rest = current % u128::from(n);
        }
        (Big(quotient).trimmed(), rest as u64)
    }

    /// Bit `at`, 0 being the least significant.
    fn bit(&self, at: u64) -> bool {
        let word = (at / 64) as usize;
        word < self.0.len() && (self.0[word] >> (at % 64)) & 1 == 1
    }

    /// Whether a bit below bit `at` is set.
    fn any_below(&self, at: u64) -> bool {
        let word = (at / 64) as usize;
        let bits = at % 64;
        self.0.iter().take(word).any(|limb| *limb != 0) || (word < self.0.len() && bits > 0 && self.0[word] & ((1u64 << bits) - 1) != 0)
    }

    /// The low 64 bits of `self >> shift`.
    fn low_after_shift(&self, shift: u64) -> u64 {
        let word = (shift / 64) as usize;
        let bits = (shift % 64) as u32;
        let low = self.0.get(word).copied().unwrap_or(0);
        if bits == 0 {
            return low;
        }
        let high = self.0.get(word + 1).copied().unwrap_or(0);
        (low >> bits) | (high << (64 - bits))
    }
}

/// (negative, m, e) with x = ±m * 2^e exactly, m below 2^53.
fn float_parts(x: f64) -> (bool, u64, i64) {
    let bits = x.to_bits();
    let negative = bits >> 63 == 1;
    let exponent = ((bits >> 52) & 0x7ff) as i64;
    let fraction = bits & ((1u64 << 52) - 1);
    if exponent == 0 {
        (negative, fraction, -1074)
    } else {
        (negative, fraction | (1u64 << 52), exponent - 1075)
    }
}

/// 2^k for k from -1022 to 1023, built from its bits.
fn power_of_two(k: i64) -> f64 {
    f64::from_bits(((k + 1023) as u64) << 52)
}

/// m * 2^exp for an m of at most 54 bits, in steps of at most 2^1000: exact whenever the result is a float.
fn ldexp(m: u64, mut exp: i64) -> f64 {
    let mut x = m as f64;
    while exp > 0 {
        let step = exp.min(1000);
        x *= power_of_two(step);
        exp -= step;
    }
    while exp < 0 {
        let step = (-exp).min(1000);
        x *= power_of_two(-step);
        exp += step;
    }
    x
}

/// statistics.mean of floats as Python 3.9 computes it: every float is an integer times a power of two, so the sum is
/// formed exactly, divided by the count and rounded once to the nearest float, ties to even (float(Fraction(total,
/// n))). A NaN or an infinity makes the result the plain sum of the non-finite values over the count, as
/// statistics._sum keeps only those. None for no value (the Python's StatisticsError; its callers check first).
pub fn exact_mean(xs: &[f64]) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    let count = xs.len() as u64;
    if xs.iter().any(|x| !x.is_finite()) {
        let total = xs.iter().filter(|x| !x.is_finite()).fold(0.0, |sum, x| sum + x);
        return Some(total / count as f64);
    }
    let parts: Vec<(bool, u64, i64)> = xs.iter().map(|x| float_parts(*x)).filter(|(_, m, _)| *m != 0).collect();
    let Some(low) = parts.iter().map(|(_, _, e)| *e).min() else {
        return Some(0.0);
    };
    let (mut plus, mut minus) = (Big::zero(), Big::zero());
    for (negative, m, e) in &parts {
        let term = Big::shifted(*m, (e - low) as u32);
        if *negative {
            minus.add(&term);
        } else {
            plus.add(&term);
        }
    }
    let (negative, sum) = match plus.compare(&minus) {
        Ordering::Equal => return Some(0.0),
        Ordering::Greater => (false, plus.minus(&minus)),
        Ordering::Less => (true, minus.minus(&plus)),
    };
    // At least 66 bits of quotient: 53 for the float, the rounding bit, and more below it.
    let shift = 130u64.saturating_sub(sum.bits());
    let (quotient, rest) = sum.shl(shift).div_rem(count);
    // The mean is quotient * 2^exponent, plus rest / count of the quotient's last unit.
    let exponent = low - shift as i64;
    let length = quotient.bits() as i64;
    // Keep 53 bits, or fewer where the mean is subnormal (no bit below 2^-1074); at least 13 are dropped.
    let drop = (length - 53).max(-1074 - exponent) as u64;
    let mut kept = quotient.low_after_shift(drop);
    let half = quotient.bit(drop - 1);
    let below = quotient.any_below(drop - 1) || rest != 0;
    if half && (below || kept & 1 == 1) {
        kept += 1;
    }
    let value = ldexp(kept, exponent + drop as i64);
    Some(if negative { -value } else { value })
}

/// statistics.median: the middle value of the sorted values, or the mean of the two middle ones ((a + b) / 2 in
/// floats, as the Python). None for no value.
pub fn median(xs: &[f64]) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    let mut sorted = xs.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    let middle = sorted.len() / 2;
    Some(if sorted.len() % 2 == 1 { sorted[middle] } else { (sorted[middle - 1] + sorted[middle]) / 2.0 })
}

#[cfg(test)]
mod tests {
    use super::{
        cer, compare, crop_number, exact_mean, levenshtein, median, norm_lines, norm_text, pair_frames, read_frames, report_lines, run,
        stream_entry, stream_number, Frame,
    };
    use crate::evidence::Ctx;
    use crate::testutil::Scratch as TempDir;
    use serde_json::{json, Map, Value};
    use std::collections::BTreeMap;
    use std::path::Path;

    /// Py: NOTE and E_ACUTE.
    const NOTE: &str = "\u{266a}";
    const E_ACUTE: &str = "\u{e9}";

    /// Frames with these lines and no ocrs fields: frame k, file f-<k in 8 digits>.png. Py: frames.
    fn frames(texts: &[&[&str]]) -> Vec<Frame> {
        texts
            .iter()
            .enumerate()
            .map(|(k, lines)| Frame {
                frame: Some(k as i64),
                file: Some(format!("f-{k:08}.png")),
                lines: lines.iter().map(|line| line.to_string()).collect(),
                fields: Map::new(),
            })
            .collect()
    }

    /// The same frames as JSON records, for writing to a file. Py: frames.
    fn frame_values(texts: &[&[&str]]) -> Vec<Value> {
        texts.iter().enumerate().map(|(k, lines)| json!({"frame": k, "file": format!("f-{k:08}.png"), "lines": lines})).collect()
    }

    /// The record with the keys of `extra` added. Py: dict(f, **extra).
    fn with(mut record: Value, extra: Value) -> Value {
        if let (Value::Object(map), Value::Object(more)) = (&mut record, extra) {
            map.extend(more);
        }
        record
    }

    /// One JSON document per line. Py: write_jsonl.
    fn write_jsonl(path: &Path, rows: &[Value]) {
        let text: String = rows.iter().map(|row| format!("{}\n", serde_json::to_string(row).unwrap())).collect();
        std::fs::write(path, text).unwrap();
    }

    /// A path as an argument.
    fn path_arg(path: &Path) -> String {
        path.to_string_lossy().into_owned()
    }

    /// Python's assertAlmostEqual: equal when rounded to 7 places.
    fn assert_almost_equal(a: f64, b: f64) {
        assert!((a - b).abs() < 5e-8, "{a} != {b}");
    }

    /// The command with `cwd` as the current directory: (exit code, stdout, stderr). Py: Main.run_main.
    fn run_main(cwd: &Path, argv: &[String]) -> (i32, String, String) {
        let ctx = Ctx::for_test(cwd);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run(&ctx, argv, &mut out, &mut err);
        (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
    }

    /// Tesseract's records (with t, pts and the joined text) and ocrs's (900 ms each, then a summary line) of three
    /// frames, as sub-2.ocr.jsonl and ocrs-s2.jsonl in `d`. Py: Main.write_pair.
    fn write_pair(d: &Path) -> Vec<String> {
        let texts: [&[&str]; 3] = [&["Vorpal zindle quax."], &["[ZORBLE HUMS]"], &[]];
        let tess: Vec<Value> = frame_values(&texts)
            .into_iter()
            .enumerate()
            .map(|(k, record)| {
                let t = k as f64 * 0.25;
                with(record, json!({"t": t, "pts": t, "text": texts[k].join(" | ")}))
            })
            .collect();
        let ocr_texts: [&[&str]; 3] = [&["Vorpa1 zindle quax."], &["[ZORBLE HUMS]"], &[]];
        let mut ocrs: Vec<Value> = frame_values(&ocr_texts).into_iter().map(|record| with(record, json!({"ms": 900.0}))).collect();
        ocrs.push(json!({"summary": {"frames": 3, "ms_total": 2700.0, "ms_per_frame": 900.0, "load_ms": 800.0, "ocrs": "0.13.1",
            "models": {"detection": "aa", "recognition": "bb"}}}));
        write_jsonl(&d.join("sub-2.ocr.jsonl"), &tess);
        write_jsonl(&d.join("ocrs-s2.jsonl"), &ocrs);
        vec![path_arg(&d.join("sub-2.ocr.jsonl")), path_arg(&d.join("ocrs-s2.jsonl"))]
    }

    /// summary.json and ocrs-summary.json of Main.test_numbers_and_speed_from_the_recording, in `rec`.
    fn write_rec(rec: &Path) {
        std::fs::create_dir_all(rec).unwrap();
        let streams = json!([{"n": 1, "frames": 9, "tesseract_s": 9.0}, {"n": 2, "frames": 3, "tesseract_s": 1.5}]);
        std::fs::write(rec.join("summary.json"), json!({ "streams": streams }).to_string()).unwrap();
        let ocrs = json!([{"n": 2, "frames": 3, "ms_per_frame": 910.0, "load_ms": 750.0}]);
        std::fs::write(rec.join("ocrs-summary.json"), json!({ "streams": ocrs }).to_string()).unwrap();
    }

    /// A cropped reading: Tesseract's three frames and ocrs-s2-crop16.jsonl with per-frame times and crop boxes (the
    /// third frame without a dark pixel), with or without its summary line. Py: Main.write_cropped.
    fn write_cropped(d: &Path, summary: bool) -> Vec<String> {
        let texts: [&[&str]; 3] = [&["Vorpal zindle quax."], &["[ZORBLE HUMS]"], &[]];
        let tess = frame_values(&texts);
        let timings = [
            (300.0, 250.0, json!([500, 900, 900, 120]), 4.0),
            (200.0, 150.0, json!([600, 950, 700, 60]), 3.0),
            (0.0, 0.0, Value::Null, 2.0),
        ];
        let mut ocrs: Vec<Value> = frame_values(&texts)
            .into_iter()
            .zip(timings)
            .map(|(record, (ms, det, crop, crop_ms))| {
                with(record, json!({"ms": ms, "decode_ms": 20.0, "detect_ms": det, "rec_ms": ms - det, "crop": crop, "crop_ms": crop_ms}))
            })
            .collect();
        if summary {
            ocrs.push(json!({"summary": {"frames": 3, "ms_total": 500.0, "ms_per_frame": 166.7, "load_ms": 800.0, "crop_pad": 16,
                "read_frames": 2, "ms_per_read_frame": 250.0}}));
        }
        write_jsonl(&d.join("sub-2.ocr.jsonl"), &tess);
        write_jsonl(&d.join("ocrs-s2-crop16.jsonl"), &ocrs);
        vec![path_arg(&d.join("sub-2.ocr.jsonl")), path_arg(&d.join("ocrs-s2-crop16.jsonl"))]
    }

    #[test]
    fn distance_levenshtein() {
        assert_eq!(levenshtein("", ""), 0);
        assert_eq!(levenshtein("abc", ""), 3);
        assert_eq!(levenshtein("", "abc"), 3);
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(levenshtein("sitting", "kitten"), 3);
        assert_eq!(levenshtein("flaw", "lawn"), 2);
    }

    #[test]
    fn distance_cer_is_over_the_longer_text() {
        assert_eq!(cer("", ""), 0.0);
        assert_eq!(cer("abcd", ""), 1.0);
        assert_eq!(cer("", "abcd"), 1.0);
        assert_eq!(cer("abcd", "abcd"), 0.0);
        assert_almost_equal(cer("abcd", "abxd"), 0.25);
        assert_almost_equal(cer("ab", "abcd"), 0.5);
    }

    #[test]
    fn normalise_fold_and_english_corrections() {
        // An accent and the music notes fold as subtitle_text reads them; Tesseract's '|' for I is corrected.
        let lines = [format!("caf{E_ACUTE}  bleu"), "  ".to_string(), format!("{NOTE} la la {NOTE}"), "| wonder".to_string()];
        assert_eq!(norm_lines(&lines, true), ["cafe bleu", "# la la #", "I wonder"]);
    }

    #[test]
    fn normalise_other_language_takes_the_pipe_as_a_space() {
        let line = ["a|b".to_string()];
        assert_eq!(norm_lines(&line, false), ["a b"]);
        assert_eq!(norm_lines(&line, true), ["alb"]);
    }

    #[test]
    fn normalise_a_different_line_split_costs_nothing() {
        let one = ["one two".to_string(), "three".to_string()];
        let two = ["one".to_string(), "two three".to_string()];
        assert_eq!(norm_text(&one, true), norm_text(&two, true));
    }

    #[test]
    fn reading_lines_and_the_summary_line() {
        let tmp = TempDir::new("summary-line");
        let path = tmp.0.join("ocrs-s0.jsonl");
        write_jsonl(
            &path,
            &[
                json!({"frame": 0, "file": "f-1.png", "lines": [" Zabo kelu ", ""], "ms": 5.0}),
                json!({"summary": {"frames": 1, "ms_per_frame": 5.0}}),
            ],
        );
        let (got, summary) = read_frames(&path, &path_arg(&path), &BTreeMap::new()).unwrap();
        let mut fields = Map::new();
        fields.insert("ms".to_string(), json!(5.0));
        assert_eq!(got, vec![Frame { frame: Some(0), file: Some("f-1.png".to_string()), lines: vec!["Zabo kelu".to_string()], fields }]);
        assert_eq!(summary.unwrap()["frames"], json!(1));
    }

    #[test]
    fn reading_joined_text_is_split_with_the_line_count() {
        let tmp = TempDir::new("joined");
        let path = tmp.0.join("sub-0.ocr.jsonl");
        write_jsonl(
            &path,
            &[
                json!({"frame": 0, "file": "f-1.png", "text": "Zabo kelu | Mira tosa"}),
                json!({"frame": 1, "file": "f-2.png", "text": ""}),
            ],
        );
        let counts: BTreeMap<i64, Option<i64>> = BTreeMap::from([(0, Some(2))]);
        let (got, summary) = read_frames(&path, &path_arg(&path), &counts).unwrap();
        let lines: Vec<Vec<String>> = got.into_iter().map(|frame| frame.lines).collect();
        assert_eq!(lines, vec![vec!["Zabo kelu".to_string(), "Mira tosa".to_string()], vec![]]);
        assert!(summary.is_none());
    }

    #[test]
    fn reading_stream_number_from_either_name() {
        assert_eq!(stream_number(&["a/sub-12.ocr.jsonl", "b/ocrs-s3.jsonl"]), Some(12));
        assert_eq!(stream_number(&["a/copy.jsonl", "b/ocrs-s3.jsonl"]), Some(3));
        assert_eq!(stream_number(&["a/copy.jsonl", "b/ocrs-s4-crop16.jsonl"]), Some(4));
        assert_eq!(stream_number(&["a/copy.jsonl", "b/other.jsonl"]), None);
    }

    #[test]
    fn reading_crop_number() {
        assert_eq!(crop_number("b/ocrs-s4-crop16.jsonl"), 16);
        assert_eq!(crop_number("b/ocrs-s4.jsonl"), 0);
    }

    #[test]
    fn reading_stream_entry_by_crop() {
        let tmp = TempDir::new("entry");
        let path = tmp.0.join("ocrs-summary.json");
        // The first entry predates -Crop: it has no crop and counts as the full frame.
        let streams = json!([{"n": 0, "ms_per_frame": 1.0}, {"n": 0, "crop": 16, "ms_per_frame": 2.0}, {"n": 1, "crop": 0, "ms_per_frame": 3.0}]);
        std::fs::write(&path, json!({ "streams": streams }).to_string()).unwrap();
        let ms = |entry: Option<Map<String, Value>>| entry.map(|entry| entry["ms_per_frame"].clone());
        assert_eq!(ms(stream_entry(&path, Some(0), Some(0)).unwrap()), Some(json!(1.0)));
        assert_eq!(ms(stream_entry(&path, Some(0), Some(16)).unwrap()), Some(json!(2.0)));
        assert_eq!(ms(stream_entry(&path, Some(0), None).unwrap()), Some(json!(1.0)));
        assert_eq!(stream_entry(&path, Some(1), Some(16)).unwrap(), None);
    }

    #[test]
    fn pairing_by_file_name_with_a_limited_ocrs_run() {
        let empty: &[&str] = &[];
        let (tess, ocrs) = (frames(&[empty; 5]), frames(&[empty; 3]));
        let (pairs, method, only_t, only_o) = pair_frames(&tess, &ocrs);
        assert_eq!((pairs.len(), method, only_t, only_o), (3, "file name", 2, 0));
    }

    #[test]
    fn pairing_by_file_name_not_by_position() {
        let record = |frame: i64, file: &str, line: &str| Frame {
            frame: Some(frame),
            file: Some(file.to_string()),
            lines: vec![line.to_string()],
            fields: Map::new(),
        };
        let tess = vec![record(0, "f-a.png", "x"), record(1, "f-b.png", "y")];
        let ocrs = vec![record(0, "f-b.png", "y")];
        let (pairs, ..) = pair_frames(&tess, &ocrs);
        let numbers: Vec<(Option<i64>, Option<i64>)> = pairs.iter().map(|(t, o)| (t.frame, o.frame)).collect();
        assert_eq!(numbers, vec![(Some(1), Some(0))]);
    }

    #[test]
    fn pairing_by_frame_number_when_a_name_is_missing() {
        let record = |frame: i64, file: Option<&str>| Frame { frame: Some(frame), file: file.map(str::to_string), lines: Vec::new(), fields: Map::new() };
        let tess = vec![record(0, None), record(1, None)];
        let ocrs = vec![record(1, Some("f-b.png")), record(7, Some("f-z.png"))];
        let (pairs, method, only_t, only_o) = pair_frames(&tess, &ocrs);
        assert_eq!((pairs.len(), method, only_t, only_o), (1, "frame number", 1, 1));
    }

    /// Py: the frames of Compare.test_counts_rates_and_markers.
    fn compare_frames() -> (Vec<Frame>, Vec<Frame>) {
        let cafe = format!("caf{E_ACUTE} tosa");
        let tess = frames(&[&["[DOOR KNOCKS]"], &["BOLA: Zabo kelu mira."], &[], &[cafe.as_str()], &["| wonder"]]);
        let ocrs = frames(&[&["[DOOR KNOCKS]"], &["BOLA: Zabo kelu mlra."], &[], &["cafe tosa"], &[]]);
        (tess, ocrs)
    }

    #[test]
    fn compare_counts_rates_and_markers() {
        let (tess, ocrs) = compare_frames();
        let c = compare(&tess, &ocrs, "eng");
        assert_eq!((c.pairs, c.method, c.only_tess, c.only_ocrs), (5, "file name", 0, 0));
        assert_eq!((c.text_tess, c.text_ocrs, c.both, c.either, c.neither), (4, 3, 3, 4, 1));
        assert_eq!(c.exact, 2); // the cue, and the accent once folded
        assert_almost_equal(c.cer_mean, (0.0 + 1.0 / 21.0 + 0.0 + 1.0) / 4.0);
        assert_eq!(c.cer_high, 1); // the frame ocrs left empty
        assert_eq!(c.same_lines, 3);
        assert_eq!(c.worst.iter().map(|r| r.frame).collect::<Vec<_>>(), vec![Some(4), Some(1)]);
        for m in [&c.m_tess, &c.m_ocrs] {
            assert_eq!((m.CUE, m.LABEL, m.CHEV, m.NOTE), (1, 1, 0, 0));
        }
        assert_eq!((c.m_tess.events, c.m_ocrs.events), (4, 3));
    }

    #[test]
    fn compare_report_shows_no_text() {
        let c = compare(&frames(&[&["Quorbel fen."]]), &frames(&[&["Quorbe1 fen."]]), "eng");
        let text = report_lines(&c).join("\n");
        assert!(text.contains("frame 0 f-00000000.png chars 12/12 lines 1/1 CER 0.083"), "{text}");
        assert!(!text.contains("Quorbe"), "{text}");
    }

    #[test]
    fn compare_report_prints_the_pythons_lines() {
        let (tess, ocrs) = compare_frames();
        let c = compare(&tess, &ocrs, "eng");
        assert_eq!(
            report_lines(&c),
            [
                "paired by file name: 5 frames; only in tesseract 0, only in ocrs 0",
                "frames with text: tesseract 4, ocrs 3, both 3, either 4, neither 1",
                "exact match after folding: 2 of 4 frames with text in either (0.500)",
                "character error rate (Levenshtein / longer text, folded): mean 0.262, median 0.024 over 4 frames; CER > 0.20 on 1 frames",
                "same line count: 3 of 3 frames with text in both (1.000)",
                "markers (subtitle_text.measure, each frame with text one event):",
                "  tesseract: events=4 CUE=1 LABEL=1 CHEV=0 NOTE=0 ODD=0 | M=2 SO=1 CS=1 readable=no lang=short (en 0.143 es 0.000 fr 0.000 de 0.000) class=unreadable",
                "  ocrs:      events=3 CUE=1 LABEL=1 CHEV=0 NOTE=0 ODD=0 | M=2 SO=1 CS=1 readable=no lang=short (en 0.000 es 0.000 fr 0.000 de 0.000) class=unreadable",
                "largest differences (frame, file, characters tesseract/ocrs, lines tesseract/ocrs, CER):",
                "  frame 4 f-00000004.png chars 8/0 lines 1/0 CER 1.000",
                "  frame 1 f-00000001.png chars 21/21 lines 1/1 CER 0.048",
            ]
        );
        assert_eq!((c.cer_mean, c.cer_median), (0.2619047619047619, 0.023809523809523808));
    }

    #[test]
    fn main_numbers_and_speed_from_the_recording() {
        let tmp = TempDir::new("numbers");
        let rec = tmp.0.join("rec");
        write_rec(&rec);
        // The ocrs file's own summary line is the run compared, so it wins over ocrs-summary.json.
        let mut argv = write_pair(&tmp.0);
        argv.extend(["--rec".to_string(), path_arg(&rec)]);
        let (code, out, err) = run_main(&tmp.0, &argv);
        assert_eq!(code, 0, "{err}");
        assert!(out.contains("paired by file name: 3 frames; only in tesseract 0, only in ocrs 0"), "{out}");
        assert!(out.contains("frames with text: tesseract 2, ocrs 2, both 2, either 2, neither 1"), "{out}");
        assert!(out.contains("exact match after folding: 1 of 2 frames with text in either (0.500)"), "{out}");
        assert_eq!(out.matches("CUE=1 ").count(), 2, "{out}");
        assert!(
            out.contains(
                "speed: tesseract 500.0 ms/frame (summary.json: 1.5 s for 3 frames, one call); \
                 ocrs 900.0 ms/frame over 3 frames (+ 800.0 ms model load)"
            ),
            "{out}"
        );
        // An output from before --crop: its ms was decode + OCR, and it says so.
        assert!(out.contains("ocrs per frame: decode + OCR (an output from before --crop) 900.0 ms mean over 3 frames read"), "{out}");
        for word in ["Vorpa", "zindle", "quax", "ZORBLE"] {
            assert!(!out.contains(word) && !err.contains(word), "{word}");
        }
    }

    #[test]
    fn main_speed_from_the_summary_line_without_a_recording() {
        let tmp = TempDir::new("speed");
        let argv = write_pair(&tmp.0);
        let (code, out, err) = run_main(&tmp.0, &argv);
        assert_eq!(code, 0, "{err}");
        assert!(
            out.contains(
                "speed: tesseract n/a (no --rec summary.json entry); \
                 ocrs 900.0 ms/frame over 3 frames (+ 800.0 ms model load)"
            ),
            "{out}"
        );
    }

    #[test]
    fn main_a_cropped_reading() {
        let tmp = TempDir::new("cropped");
        let argv = write_cropped(&tmp.0, true);
        let (code, out, err) = run_main(&tmp.0, &argv);
        assert_eq!(code, 0, "{err}");
        assert!(out.contains("paired by file name: 3 frames; only in tesseract 0, only in ocrs 0"), "{out}");
        assert!(out.contains("exact match after folding: 2 of 2 frames with text in either (1.000)"), "{out}");
        assert!(
            out.contains("ocrs 166.7 ms/frame over 3 frames, 250.0 ms per frame read (2 read), crop pad 16 (+ 800.0 ms model load)"),
            "{out}"
        );
        assert!(
            out.contains(
                "ocrs per frame: OCR 250.0 ms mean over 2 frames read; detection 200.0 ms, recognition 50.0 ms; \
                 crop: 2 boxes, mean 800 x 90 px, 1 frames without a dark pixel (not read), crop_ms mean 3.00"
            ),
            "{out}"
        );
        for word in ["Vorpa", "zindle", "ZORBLE"] {
            assert!(!out.contains(word) && !err.contains(word), "{word}");
        }
    }

    #[test]
    fn main_a_cropped_reading_without_a_summary_line_takes_its_own_entry() {
        let tmp = TempDir::new("own-entry");
        let rec = tmp.0.join("rec");
        std::fs::create_dir_all(&rec).unwrap();
        let streams = json!([
            {"n": 2, "frames": 3, "ms_per_frame": 910.0, "load_ms": 750.0},
            {"n": 2, "crop": 16, "frames": 3, "ms_per_frame": 170.0, "load_ms": 700.0}
        ]);
        std::fs::write(rec.join("ocrs-summary.json"), json!({ "streams": streams }).to_string()).unwrap();
        let mut argv = write_cropped(&tmp.0, false);
        argv.extend(["--rec".to_string(), path_arg(&rec)]);
        let (code, out, err) = run_main(&tmp.0, &argv);
        assert_eq!(code, 0, "{err}");
        assert!(out.contains("ocrs 170.0 ms/frame over 3 frames (+ 700.0 ms model load)"), "{out}");
    }

    #[test]
    fn main_unreadable_or_disjoint_input_exits_2() {
        let tmp = TempDir::new("unreadable");
        let argv = write_pair(&tmp.0);
        assert_eq!(run_main(&tmp.0, &[argv[0].clone(), path_arg(&tmp.0.join("missing.jsonl"))]).0, 2);
        std::fs::write(tmp.0.join("bad.jsonl"), "{not json\n").unwrap();
        assert_eq!(run_main(&tmp.0, &[argv[0].clone(), path_arg(&tmp.0.join("bad.jsonl"))]).0, 2);
        write_jsonl(&tmp.0.join("other.jsonl"), &[json!({"frame": 0, "file": "f-elsewhere.png", "lines": []})]);
        let (code, _, err) = run_main(&tmp.0, &[argv[0].clone(), path_arg(&tmp.0.join("other.jsonl"))]);
        assert_eq!(code, 2);
        assert!(err.contains("no frame is in both files"), "{err}");
    }

    #[test]
    fn exact_mean_is_statistics_mean() {
        // Python 3.9.6's statistics.mean; the comment gives the plain sum over the count where it differs.
        assert_eq!(exact_mean(&[0.1, 0.2, 0.3]), Some(0.2)); // 0.20000000000000004
        assert_eq!(exact_mean(&[1e16, 1.0, -1e16]), Some(0.3333333333333333)); // 0.0
        assert_eq!(exact_mean(&[0.1; 10]), Some(0.1)); // 0.09999999999999999
        assert_eq!(exact_mean(&[1e308, 1e308]), Some(1e308)); // inf
        assert_eq!(exact_mean(&[5e-324, 0.0]), Some(0.0));
        assert_eq!(exact_mean(&[5e-324, 5e-324, 5e-324, 0.0]), Some(5e-324));
        assert_eq!(exact_mean(&[-1.5, -2.5]), Some(-2.0));
        assert_eq!(exact_mean(&[0.0, 1.0 / 21.0, 0.0, 1.0]), Some(0.2619047619047619));
        assert!(exact_mean(&[-0.0]).unwrap().is_sign_positive());
        assert_eq!(exact_mean(&[]), None);
        assert!(exact_mean(&[1.0, f64::NAN]).unwrap().is_nan());
        assert_eq!(exact_mean(&[1.0, f64::INFINITY]), Some(f64::INFINITY));
    }

    #[test]
    fn median_is_statistics_median() {
        assert_eq!(median(&[0.0, 1.0 / 21.0, 0.0, 1.0]), Some(0.023809523809523808));
        assert_eq!(median(&[3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&[]), None);
    }

    #[test]
    fn reading_stream_and_crop_numbers_need_the_whole_pattern() {
        // re.match(r'(?:sub-|ocrs-s)(\d+)(?:-crop\d+)?\.', name): the digits must be followed by "." or by
        // "-crop<digits>.", and the name is the path's last component (pathlib drops a trailing "/" and ".").
        assert_eq!(stream_number(&["sub-7", "ocrs-s8-crop.jsonl", "ocrs-s9-cropx.jsonl", "x/ocrs-s10-crop2.json"]), Some(10));
        assert_eq!(stream_number(&["./sub-3.ocr.jsonl/."]), Some(3));
        assert_eq!(crop_number("ocrs-s4-crop16"), 0);
        assert_eq!(crop_number("dir/ocrs-s4-crop16.jsonl/"), 16);
    }

    #[test]
    fn main_prints_the_pythons_lines() {
        let tmp = TempDir::new("lines");
        let pair = tmp.0.join("pair");
        std::fs::create_dir_all(&pair).unwrap();
        write_pair(&pair);
        write_rec(&pair.join("rec"));
        let argv: Vec<String> = ["pair/sub-2.ocr.jsonl", "pair/ocrs-s2.jsonl", "--rec", "pair/rec"].iter().map(|word| word.to_string()).collect();
        let (code, out, err) = run_main(&tmp.0, &argv);
        assert_eq!((code, err.as_str()), (0, ""));
        let want: String = [
            "tesseract: pair/sub-2.ocr.jsonl: 3 frames; ocrs: pair/ocrs-s2.jsonl: 3 frames (summary line: yes)",
            "paired by file name: 3 frames; only in tesseract 0, only in ocrs 0",
            "frames with text: tesseract 2, ocrs 2, both 2, either 2, neither 1",
            "exact match after folding: 1 of 2 frames with text in either (0.500)",
            "character error rate (Levenshtein / longer text, folded): mean 0.026, median 0.026 over 2 frames; CER > 0.20 on 0 frames",
            "same line count: 2 of 2 frames with text in both (1.000)",
            "markers (subtitle_text.measure, each frame with text one event):",
            "  tesseract: events=2 CUE=1 LABEL=0 CHEV=0 NOTE=0 ODD=0 | M=1 SO=1 CS=1 readable=no lang=short (en 0.000 es 0.000 fr 0.000 de 0.000) class=unreadable",
            "  ocrs:      events=2 CUE=1 LABEL=0 CHEV=0 NOTE=0 ODD=0 | M=1 SO=1 CS=1 readable=no lang=short (en 0.000 es 0.000 fr 0.000 de 0.000) class=unreadable",
            "largest differences (frame, file, characters tesseract/ocrs, lines tesseract/ocrs, CER):",
            "  frame 0 f-00000000.png chars 19/19 lines 1/1 CER 0.053",
            "speed: tesseract 500.0 ms/frame (summary.json: 1.5 s for 3 frames, one call); ocrs 900.0 ms/frame over 3 frames (+ 800.0 ms model load)",
            "ocrs per frame: decode + OCR (an output from before --crop) 900.0 ms mean over 3 frames read",
        ]
        .iter()
        .map(|line| format!("{line}\n"))
        .collect();
        assert_eq!(out, want);
    }

    #[test]
    fn main_names_a_missing_file_and_reads_arguments_as_argparse_did() {
        let tmp = TempDir::new("arguments");
        let pair = tmp.0.join("pair");
        std::fs::create_dir_all(&pair).unwrap();
        write_pair(&pair);
        // FileNotFoundError names str(Path(arg)): "./pair//missing.jsonl" is shown as "pair/missing.jsonl".
        for missing in ["pair/missing.jsonl", "./pair//missing.jsonl"] {
            let argv = vec!["pair/sub-2.ocr.jsonl".to_string(), missing.to_string()];
            let (code, out, err) = run_main(&tmp.0, &argv);
            assert_eq!((code, out.as_str()), (2, ""));
            if cfg!(unix) {
                assert_eq!(err, "cannot read the input: [Errno 2] No such file or directory: 'pair/missing.jsonl'\n");
            } else {
                assert!(err.starts_with("cannot read the input: [Errno 2] ") && err.ends_with(": 'pair/missing.jsonl'\n"), "{err}");
            }
        }
        let usage = "usage: lazymkv-dev ocr-compare [-h] [--rec REC] [--tag TAG] tesseract ocrs\n";
        let cases: [(&[&str], String); 3] = [
            (&[], format!("{usage}lazymkv-dev ocr-compare: error: the following arguments are required: tesseract, ocrs\n")),
            (&["a", "b", "--rec"], format!("{usage}lazymkv-dev ocr-compare: error: argument --rec: expected one argument\n")),
            (&["a", "b", "c"], format!("{usage}lazymkv-dev ocr-compare: error: unrecognized arguments: c\n")),
        ];
        for (args, stderr) in cases {
            let argv: Vec<String> = args.iter().map(|word| word.to_string()).collect();
            assert_eq!(run_main(&tmp.0, &argv), (2, String::new(), stderr), "{args:?}");
        }
        let (code, out, err) = run_main(&tmp.0, &["-h".to_string()]);
        assert_eq!((code, err.as_str()), (0, ""));
        assert!(out.starts_with(usage) && out.contains("\npositional arguments:\n  tesseract   ocr-subs.ps1's sub-<n>.ocr.jsonl\n"), "{out}");
    }
}
