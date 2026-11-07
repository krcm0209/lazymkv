//! The OCR evidence files of design part 10.12 in phase 0's shape [EVID-5/OCR-11], and design part 10.3's event cross-check. Pure (RP19): the worker and the engine do the writing. Record lines hold subtitle text; every other output and every error holds counts, times and sizes only.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::{json, Map, Value};

use crate::recording::files::Packet;
use crate::recording::py_float_value;
use crate::subtitle::align::size_of;
use crate::subtitle::bitmap::lines::{LineBox, DECODER_VERSION, LINE_FINDER_VERSION};

/// How a record's lines were found: by the row line finder or by the detector (design parts 10.6 and 10.12; L6).
/// Written as "rows" or "detector". Def: T5-6. Use: T5-8, T5-10.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Method {
    Rows,
    Detector,
}

/// One event's milliseconds per stage (design part 10.12, L6); `detect` is None when the detector did not run.
/// Def: T5-6. Use: T5-8, T5-10.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
pub struct StageMs {
    pub decode: f64,
    pub prepare: f64,
    pub lines: f64,
    pub detect: Option<f64>,
    pub recognize: f64,
}

/// The engine key of design part 10.12: the ocrs version and the SHA-256 of the two models. Def: T5-6. Use: T5-8,
/// T5-10, T5-15.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EngineId {
    pub ocrs: String,
    pub detection_sha256: String,
    pub recognition_sha256: String,
}

/// One event's record (design parts 10.3 and 10.12; L6, K1): phase 0's six keys and the extra keys. Holds subtitle
/// text: written only by the worker (design part 10.12). Def: T5-6. Use: T5-8, T5-10.
#[derive(Clone, Debug, PartialEq)]
pub struct EventRecord {
    pub frame: usize,
    pub file: String,
    pub pts: f64,
    pub lines: Vec<String>,
    pub size: u64,
    pub boxes: Vec<LineBox>,
    pub method: Method,
    pub trigger: Option<u8>,
    pub engine: EngineId,
    pub ms: StageMs,
}

#[derive(serde::Serialize)]
struct BoxOut {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

#[derive(serde::Serialize)]
struct DecoderOut {
    bitmap: &'static str,
    rows: &'static str,
}

const DECODER: DecoderOut = DecoderOut { bitmap: DECODER_VERSION, rows: LINE_FINDER_VERSION };

#[derive(serde::Serialize)]
struct LineOut<'a> {
    frame: usize,
    file: &'a str,
    t: f64,
    pts: f64,
    text: String,
    lines: &'a [String],
    size: u64,
    boxes: Vec<BoxOut>,
    method: Method,
    trigger: Option<u8>,
    engine: &'a EngineId,
    decoder: DecoderOut,
    ms: &'a StageMs,
}

/// One record as one compact JSON (JavaScript Object Notation) object and "\n" (design part 10.12; R6-phase0-17):
/// phase 0's keys frame, file, t, pts, text and lines, then size, boxes, method, trigger, engine, decoder and ms.
/// Def: T5-6. Use: T5-8, T5-10.
pub fn record_line(record: &EventRecord) -> String {
    let out = LineOut {
        frame: record.frame,
        file: &record.file,
        t: record.pts,
        pts: record.pts,
        text: record.lines.join(" | "),
        lines: &record.lines,
        size: record.size,
        boxes: record.boxes.iter().map(|b| BoxOut { x: b.x, y: b.y, width: b.width, height: b.height }).collect(),
        method: record.method,
        trigger: record.trigger,
        engine: &record.engine,
        decoder: DECODER,
        ms: &record.ms,
    };
    let mut line = serde_json::to_string(&out).expect("a record line serializes");
    line.push('\n');
    line
}

/// The crop's file name, `f-<round(pts * 1000), 9 digits>.png`, which sorts in pts order (design part 10.12; T5-R21).
/// Def: T5-6. Use: T5-8, T5-10.
pub fn crop_name(pts: f64) -> String {
    format!("f-{:09}.png", (pts * 1000.0).round() as u64)
}

/// The five stage totals of one run, in milliseconds (design part 10.12). Def: T5-6. Use: T5-8, T5-15.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StageTotals {
    pub decode: f64,
    pub prepare: f64,
    pub lines: f64,
    pub detect: f64,
    pub recognize: f64,
}

/// run.json (design part 10.12; the layout is the builder's, notes section 6): counts and times only, no text.
/// `detector[i]` counts trigger i + 1 (L6); `ms.detect` is 0 when the detector never ran. Def: T5-6. Use: T5-8,
/// T5-10, T5-15.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RunTotals {
    pub events: usize,
    pub distinct: usize,
    pub rows: usize,
    pub detector: [usize; 4],
    pub ms: StageTotals,
    pub engine: EngineId,
    pub wall_ms: u64,
}

/// run.json as pretty JSON and "\n"; holds no text (design part 10.12). Def: T5-6. Use: T5-8.
pub fn run_json(totals: &RunTotals) -> String {
    let mut text = serde_json::to_string_pretty(totals).expect("run totals serialize");
    text.push('\n');
    text
}

/// Reads run.json back (design part 10.12); Err is `run.json does not parse: ` and serde's text, which names no
/// value. Def: T5-6. Use: T5-10, T5-15.
pub fn read_run_json(text: &str) -> Result<RunTotals, String> {
    serde_json::from_str(text).map_err(|error| format!("run.json does not parse: {error}"))
}

#[derive(serde::Deserialize)]
struct PtsSize {
    pts: f64,
    size: u64,
}

/// The `(pts, size)` of each whole record: a line ended by "\n" that parses; the text after the last newline (empty
/// or torn) and blank lines are left out (design part 10.8; T5-R9). Def: T5-6. Use: T5-8, T5-10, T5-15.
pub fn whole_records(text: &str) -> Vec<(f64, u64)> {
    let mut pieces: Vec<&str> = text.split('\n').collect();
    pieces.pop();
    pieces
        .into_iter()
        .filter(|piece| !piece.trim_matches(['\r', ' ']).is_empty())
        .filter_map(|piece| serde_json::from_str::<PtsSize>(piece).ok())
        .map(|record| (record.pts, record.size))
        .collect()
}

/// The cross-check of design part 10.3 (T5-R59): the records' number, order and sizes against the listed packets
/// larger than `clear_max` bytes. Err holds numbers only, or `size_of`'s or `py_float_value`'s text. Def: T5-6.
/// Use: T5-11, T5-15.
pub fn check_events(records: &[(f64, u64)], packets: &[Packet], clear_max: i64) -> Result<(), String> {
    let mut listed: Vec<(f64, i64)> = Vec::new();
    for packet in packets {
        let Some(pts_time) = &packet.pts_time else {
            continue;
        };
        let size = size_of(packet)?;
        if size <= clear_max {
            continue;
        }
        listed.push((py_float_value(pts_time)?, size));
    }
    listed.sort_by(|a, b| a.0.total_cmp(&b.0));
    if records.len() != listed.len() {
        return Err(format!("{} events recorded, {} listed", records.len(), listed.len()));
    }
    for (i, (record, list)) in records.iter().zip(&listed).enumerate() {
        if i > 0 && record.0 < records[i - 1].0 {
            return Err(format!("event {i}: pts {} recorded after {}", record.0, records[i - 1].0));
        }
        if i64::try_from(record.1).ok() != Some(list.1) {
            return Err(format!("event {i}: size {} recorded, {} listed", record.1, list.1));
        }
    }
    Ok(())
}

/// Phase 0's five text markers (`Count-Markers` of ocr-subs.ps1; T5-R20), counted per line. Def: T5-6. Use: T5-15.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Markers {
    pub bracket: usize,
    pub note: usize,
    pub hash: usize,
    pub speaker: usize,
    pub upper: usize,
}

static BRACKET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\[(].*[\])]").expect("bracket pattern"));
static HASH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*#|#\s*$").expect("hash pattern"));
static SPEAKER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*[A-Z][A-Z0-9 .'\-]{0,24}:(\s|$)").expect("speaker pattern"));
static THREE_UPPER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Z].*[A-Z].*[A-Z]").expect("upper pattern"));

fn count_markers(lines: &[String]) -> Markers {
    let mut markers = Markers::default();
    for line in lines {
        markers.bracket += usize::from(BRACKET.is_match(line));
        markers.note += usize::from(line.contains(['\u{266A}', '\u{266B}']));
        markers.hash += usize::from(HASH.is_match(line));
        markers.speaker += usize::from(SPEAKER.is_match(line));
        markers.upper += usize::from(!line.chars().any(|c| c.is_ascii_lowercase()) && THREE_UPPER.is_match(line));
    }
    markers
}

#[derive(serde::Deserialize)]
struct RecordIn {
    frame: usize,
    file: String,
    t: f64,
    pts: f64,
    lines: Vec<String>,
}

fn chars_of(lines: &[String]) -> usize {
    lines.join(" | ").replace(" | ", "").encode_utf16().count()
}

/// frames.json from the records (design part 10.12; T5-R20): a pretty JSON list and "\n", phase 0's seven keys per
/// event. Err names a line number and a column, never its text. Def: T5-6. Use: T5-15.
pub fn frames_json(records_text: &str) -> Result<String, String> {
    let mut frames = Vec::new();
    for (index, line) in records_text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let record: RecordIn = serde_json::from_str(line)
            .map_err(|error| format!("line {} of the records is not a record (column {})", index + 1, error.column()))?;
        let mut entry = Map::new();
        entry.insert("frame".to_string(), json!(record.frame));
        entry.insert("file".to_string(), json!(record.file));
        entry.insert("t".to_string(), json!(record.t));
        entry.insert("pts".to_string(), json!(record.pts));
        entry.insert("chars".to_string(), json!(chars_of(&record.lines)));
        entry.insert("lines".to_string(), json!(record.lines.len()));
        entry.insert("markers".to_string(), json!(count_markers(&record.lines)));
        frames.push(Value::Object(entry));
    }
    let mut text = serde_json::to_string_pretty(&frames).expect("frames serialize");
    text.push('\n');
    Ok(text)
}

/// `(events with text, lines, markers summed)` over the records that parse (design part 10.12). Def: T5-6.
/// Use: T5-10, T5-15.
pub fn text_counts(records_text: &str) -> (usize, usize, Markers) {
    let mut with_text = 0;
    let mut lines = 0;
    let mut total = Markers::default();
    for line in records_text.lines() {
        let Ok(record) = serde_json::from_str::<RecordIn>(line) else {
            continue;
        };
        if chars_of(&record.lines) > 0 {
            with_text += 1;
            lines += record.lines.len();
        }
        let markers = count_markers(&record.lines);
        total.bracket += markers.bracket;
        total.note += markers.note;
        total.hash += markers.hash;
        total.speaker += markers.speaker;
        total.upper += markers.upper;
    }
    (with_text, lines, total)
}

/// What became of one stream (design part 10.8): read, or unread with the reason, never text. Def: T5-6.
/// Use: T5-15.
#[derive(Clone, Debug, PartialEq)]
pub enum StreamOutcome {
    Read(RunTotals),
    Unread(String),
}

/// One stream's entry of summary.json (design parts 10.8 and 10.12; T5-R57): `exit` is None when the run ended
/// without one; `wall_ms` is the engine's clock, start to exit. Def: T5-6. Use: T5-10, T5-15.
#[derive(Clone, Debug, PartialEq)]
pub struct StreamSummary {
    pub n: i64,
    pub codec: String,
    pub outcome: StreamOutcome,
    pub exit: Option<i32>,
    pub wall_ms: u64,
    pub with_text: usize,
    pub lines: usize,
    pub markers: Markers,
}

/// summary.json, `{"streams": [...]}` pretty with "\n" (design part 10.12; the shape Task 2's `OcrSummary::unread`
/// reads): a read entry has run, decoder, with_text, lines and markers; an unread one has `why`. A stream number is
/// written once: if `streams` lists it twice, the last entry is written. Def: T5-6. Use: T5-10, T5-15.
pub fn summary_json(streams: &[StreamSummary]) -> String {
    let mut entries = Vec::new();
    for (index, stream) in streams.iter().enumerate() {
        if streams[index + 1..].iter().any(|later| later.n == stream.n) {
            continue;
        }
        let mut entry = Map::new();
        entry.insert("n".to_string(), json!(stream.n));
        entry.insert("codec".to_string(), json!(stream.codec));
        match &stream.outcome {
            StreamOutcome::Read(totals) => {
                entry.insert("read".to_string(), json!(true));
                entry.insert("exit".to_string(), json!(stream.exit));
                entry.insert("wall_ms".to_string(), json!(stream.wall_ms));
                entry.insert("run".to_string(), serde_json::to_value(totals).expect("run totals serialize"));
                entry.insert("decoder".to_string(), serde_json::to_value(DECODER).expect("decoder serializes"));
                entry.insert("with_text".to_string(), json!(stream.with_text));
                entry.insert("lines".to_string(), json!(stream.lines));
                entry.insert("markers".to_string(), json!(stream.markers));
            }
            StreamOutcome::Unread(why) => {
                entry.insert("read".to_string(), json!(false));
                entry.insert("why".to_string(), json!(why));
                entry.insert("exit".to_string(), json!(stream.exit));
                entry.insert("wall_ms".to_string(), json!(stream.wall_ms));
            }
        }
        entries.push(Value::Object(entry));
    }
    let mut text = serde_json::to_string_pretty(&json!({ "streams": entries })).expect("summary serializes");
    text.push('\n');
    text
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::*;
    use crate::recording::files::{read_jsonl, FrameEntry, OcrSummary, Packet, PacketsFile};
    use crate::subtitle::bitmap::cap::cap_reason;
    use crate::subtitle::bitmap::lines::LineBox;

    fn engine() -> EngineId {
        EngineId { ocrs: "0.13.1".to_string(), detection_sha256: "d".repeat(64), recognition_sha256: "r".repeat(64) }
    }

    fn record(frame: usize, pts: f64, lines: &[&str], size: u64) -> EventRecord {
        EventRecord {
            frame,
            file: crop_name(pts),
            pts,
            lines: lines.iter().map(|line| line.to_string()).collect(),
            size,
            boxes: vec![LineBox { x: 16, y: 4, width: 200, height: 40 }],
            method: Method::Rows,
            trigger: None,
            engine: engine(),
            ms: StageMs { decode: 1.5, prepare: 0.25, lines: 0.125, detect: None, recognize: 80.0 },
        }
    }

    fn totals() -> RunTotals {
        RunTotals {
            events: 3,
            distinct: 2,
            rows: 2,
            detector: [1, 0, 0, 0],
            ms: StageTotals { decode: 4.5, prepare: 0.75, lines: 0.5, detect: 12.0, recognize: 240.0 },
            engine: engine(),
            wall_ms: 5321,
        }
    }

    fn packets(text: &str) -> Vec<Packet> {
        serde_json::from_str::<PacketsFile>(text).unwrap().packets().to_vec()
    }

    fn keys(value: &Value) -> Vec<String> {
        value.as_object().unwrap().keys().cloned().collect()
    }

    const LISTED: &str = r#"{"packets": [{"pts_time": "20.000000", "size": "30"}, {"pts_time": "23.000000", "size": "3000"}, {"pts_time": "21.000000", "size": "1400"}, {"pts_time": "25.000000", "size": "64"}, {"size": "5000"}]}"#;

    const ALL_MARKERS: Markers = Markers { bracket: 1, note: 1, hash: 1, speaker: 1, upper: 1 };

    fn streams() -> [StreamSummary; 2] {
        [
            StreamSummary { n: 0, codec: "S_HDMV/PGS".to_string(), outcome: StreamOutcome::Read(totals()), exit: Some(0),
                wall_ms: 6012, with_text: 3, lines: 5, markers: ALL_MARKERS },
            StreamSummary { n: 3, codec: "S_VOBSUB".to_string(), outcome: StreamOutcome::Unread(cap_reason(60.0, 12, 20)),
                exit: None, wall_ms: 60004, with_text: 0, lines: 0, markers: Markers::default() },
        ]
    }

    fn marker_records() -> String {
        [
            record_line(&record(0, 10.0, &["[sentinel-a]", "\u{266A} sentinel \u{266A}"], 1400)),
            record_line(&record(1, 12.5, &["SENTINEL-B: sentinel", "# SENTINEL ABC #"], 900)),
            record_line(&EventRecord { file: crop_name(10.0), ..record(2, 14.0, &[], 1400) }),
            record_line(&record(3, 15.0, &["\u{1D11E} sentinel"], 500)),
        ]
        .concat()
    }

    #[test]
    fn a_record_line_has_phase_0s_keys_then_the_extra_keys_in_order() {
        let line = record_line(&record(2, 12.5, &["text-sentinel-3e8f", "text-sentinel-a1"], 1400));
        let want = [
            r#"{"frame":2,"file":"f-000012500.png","t":12.5,"pts":12.5,"text":"text-sentinel-3e8f | text-sentinel-a1","#,
            r#""lines":["text-sentinel-3e8f","text-sentinel-a1"],"size":1400,"boxes":[{"x":16,"y":4,"width":200,"height":40}],"#,
            r#""method":"rows","trigger":null,"engine":{"ocrs":"0.13.1","detection_sha256":"DDD","recognition_sha256":"RRR"},"#,
            r#""decoder":{"bitmap":"lazymkv-bitmap 1","rows":"lazymkv-rows 1"},"#,
            r#""ms":{"decode":1.5,"prepare":0.25,"lines":0.125,"detect":null,"recognize":80.0}}"#,
            "\n",
        ]
        .concat()
        .replace("DDD", &"d".repeat(64))
        .replace("RRR", &"r".repeat(64));
        assert_eq!(line, want);
        let value: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(keys(&value), ["frame", "file", "t", "pts", "text", "lines", "size", "boxes", "method", "trigger", "engine", "decoder", "ms"]);
    }

    #[test]
    fn text_joins_the_lines_with_space_bar_space_and_t_equals_pts() {
        let mut detector = record(0, 61.103, &["text-sentinel-3e8f", "text-sentinel-b2", "text-sentinel-c3"], 900);
        detector.method = Method::Detector;
        detector.trigger = Some(3);
        detector.ms.detect = Some(12.0);
        let value: Value = serde_json::from_str(&record_line(&detector)).unwrap();
        assert_eq!(value["text"], "text-sentinel-3e8f | text-sentinel-b2 | text-sentinel-c3");
        assert_eq!((value["t"].as_f64(), value["pts"].as_f64()), (Some(61.103), Some(61.103)));
        assert_eq!((&value["method"], &value["trigger"], &value["ms"]["detect"]), (&json!("detector"), &json!(3), &json!(12.0)));
        let blank: Value = serde_json::from_str(&record_line(&record(1, 62.0, &[], 300))).unwrap();
        assert_eq!((&blank["text"], &blank["lines"]), (&json!(""), &json!([])));
        let read = read_jsonl(&record_line(&detector)).unwrap();
        assert_eq!((read[0].frame, read[0].pts, read[0].t), (Some(0), Some(61.103), Some(61.103)));
        for key in ["size", "boxes", "method", "trigger", "engine", "decoder", "ms"] {
            assert!(read[0].rest.contains_key(key), "OcrLine keeps {key} in rest");
        }
    }

    #[test]
    fn crop_names_sort_in_pts_order() {
        assert_eq!(crop_name(0.0), "f-000000000.png");
        assert_eq!(crop_name(61.103), "f-000061103.png");
        assert_eq!(crop_name(5025.5), "f-005025500.png");
        let names: Vec<String> = [9.5, 10.0, 100.25, 999.999, 7384.04].iter().map(|pts| crop_name(*pts)).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    #[test]
    fn run_json_round_trips_and_holds_no_text() {
        let text = run_json(&totals());
        let want = [
            "{",
            r#"  "events": 3,"#,
            r#"  "distinct": 2,"#,
            r#"  "rows": 2,"#,
            r#"  "detector": ["#,
            "    1,",
            "    0,",
            "    0,",
            "    0",
            "  ],",
            r#"  "ms": {"#,
            r#"    "decode": 4.5,"#,
            r#"    "prepare": 0.75,"#,
            r#"    "lines": 0.5,"#,
            r#"    "detect": 12.0,"#,
            r#"    "recognize": 240.0"#,
            "  },",
            r#"  "engine": {"#,
            r#"    "ocrs": "0.13.1","#,
            r#"    "detection_sha256": "DDD","#,
            r#"    "recognition_sha256": "RRR""#,
            "  },",
            r#"  "wall_ms": 5321"#,
            "}",
            "",
        ]
        .join("\n")
        .replace("DDD", &"d".repeat(64))
        .replace("RRR", &"r".repeat(64));
        assert_eq!(text, want);
        assert_eq!(read_run_json(&text), Ok(totals()));
        let error = read_run_json(r#"{"events": 1}"#).unwrap_err();
        assert!(error.starts_with("run.json does not parse: "), "{error}");
    }

    #[test]
    fn whole_records_leave_out_a_torn_last_line() {
        let a = record_line(&record(0, 10.0, &["text-sentinel-3e8f"], 1400));
        let b = record_line(&record(1, 12.5, &["text-sentinel-3e8f"], 900));
        let c = record_line(&record(2, 14.0, &["text-sentinel-3e8f"], 700));
        assert_eq!(whole_records(&format!("{a}{b}{}", &c[..30])), vec![(10.0, 1400), (12.5, 900)]);
        assert_eq!(whole_records(&format!("{a}{b}{}", c.trim_end())), vec![(10.0, 1400), (12.5, 900)], "a line without its newline is not whole");
        assert_eq!(whole_records(&format!("{a}\n{b}{c}")), vec![(10.0, 1400), (12.5, 900), (14.0, 700)], "a blank line is skipped");
        assert_eq!(whole_records(""), Vec::<(f64, u64)>::new());
    }

    #[test]
    fn check_events_passes_equal_number_order_and_size() {
        assert_eq!(check_events(&[(21.0, 1400), (23.0, 3000)], &packets(LISTED), 64), Ok(()), "listed out of file order and sorted by pts");
        assert_eq!(check_events(&[], &packets(r#"{"packets": []}"#), 64), Ok(()));
    }

    #[test]
    fn check_events_names_a_count_or_size_difference_with_numbers_only() {
        let listed = packets(LISTED);
        assert_eq!(check_events(&[(21.0, 1400)], &listed, 64), Err("1 events recorded, 2 listed".to_string()));
        assert_eq!(check_events(&[(21.0, 1400), (23.0, 3000), (24.0, 80)], &listed, 64), Err("3 events recorded, 2 listed".to_string()));
        assert_eq!(check_events(&[(21.0, 1400), (23.0, 2999)], &listed, 64), Err("event 1: size 2999 recorded, 3000 listed".to_string()));
        assert!(check_events(&[], &packets(r#"{"packets": [{"pts_time": "1.0", "size": "big"}]}"#), 64).is_err());
    }

    #[test]
    fn check_events_refuses_records_out_of_pts_order() {
        let listed = packets(r#"{"packets": [{"pts_time": "21.0", "size": "1400"}, {"pts_time": "23.0", "size": "1400"}]}"#);
        assert_eq!(check_events(&[(23.0, 1400), (21.0, 1400)], &listed, 64), Err("event 1: pts 21 recorded after 23".to_string()));
    }

    #[test]
    fn a_packet_of_exactly_clear_max_bytes_is_no_event() {
        let listed = packets(r#"{"packets": [{"pts_time": "1.0", "size": "64"}, {"pts_time": "2.0", "size": "65"}, {"pts_time": "3.0", "size": "40"}]}"#);
        assert_eq!(check_events(&[(2.0, 65)], &listed, 64), Ok(()));
        assert_eq!(check_events(&[(1.0, 64), (2.0, 65)], &listed, 40), Ok(()), "T5-R59: the job's own sub_clear_max_bytes");
        assert_eq!(check_events(&[(2.0, 65)], &listed, 40), Err("1 events recorded, 2 listed".to_string()));
    }

    #[test]
    fn frames_json_counts_chars_without_separators_and_phase_0s_five_markers() {
        let text = frames_json(&marker_records()).unwrap();
        assert!(text.ends_with("]\n"));
        let none = json!({"bracket": 0, "note": 0, "hash": 0, "speaker": 0, "upper": 0});
        let frames: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(frames, json!([
            {"frame": 0, "file": "f-000010000.png", "t": 10.0, "pts": 10.0, "chars": 24, "lines": 2,
             "markers": {"bracket": 1, "note": 1, "hash": 0, "speaker": 0, "upper": 0}},
            {"frame": 1, "file": "f-000012500.png", "t": 12.5, "pts": 12.5, "chars": 36, "lines": 2,
             "markers": {"bracket": 0, "note": 0, "hash": 1, "speaker": 1, "upper": 1}},
            {"frame": 2, "file": "f-000010000.png", "t": 14.0, "pts": 14.0, "chars": 0, "lines": 0, "markers": none.clone()},
            {"frame": 3, "file": "f-000015000.png", "t": 15.0, "pts": 15.0, "chars": 11, "lines": 1, "markers": none},
        ]), "chars counts UTF-16 units, as phase 0's String.Length did");
        assert_eq!(keys(&frames[0]), ["frame", "file", "t", "pts", "chars", "lines", "markers"]);
        assert_eq!(keys(&frames[0]["markers"]), ["bracket", "note", "hash", "speaker", "upper"]);
        let entries: Vec<FrameEntry> = serde_json::from_str(&text).unwrap();
        assert_eq!((entries[1].frame, entries[1].lines), (1, Some(2)));
    }

    #[test]
    fn frames_json_refuses_a_line_that_is_not_a_record_without_quoting_it() {
        let text = format!("{}{}", record_line(&record(0, 10.0, &["text-sentinel-3e8f"], 1400)), "{\"frame\": \"text-sentinel-3e8f\"}\n");
        let error = frames_json(&text).unwrap_err();
        assert!(error.starts_with("line 2 of the records is not a record (column "), "{error}");
        assert!(!error.contains("sentinel"), "{error}");
    }

    #[test]
    fn text_counts_give_events_with_text_lines_and_the_markers_summed() {
        assert_eq!(text_counts(&marker_records()), (3, 5, ALL_MARKERS));
        assert_eq!(text_counts(""), (0, 0, Markers::default()));
    }

    #[test]
    fn summary_json_says_why_an_unread_stream_was_not_read_and_task_2s_reader_reads_it() {
        let text = summary_json(&streams());
        assert!(text.ends_with("}\n"));
        let summary: OcrSummary = serde_json::from_str(&text).unwrap();
        assert_eq!(summary.unread(), vec![(3, "time cap of 60.0 s reached after 12 of 20 events".to_string())]);
        let unread = Value::Object(summary.streams[1].clone());
        assert_eq!(unread, json!({"n": 3, "codec": "S_VOBSUB", "read": false,
            "why": "time cap of 60.0 s reached after 12 of 20 events", "exit": null, "wall_ms": 60004}));
        assert_eq!(keys(&unread), ["n", "codec", "read", "why", "exit", "wall_ms"]);
    }

    #[test]
    fn summary_json_gives_a_read_streams_run_totals_text_counts_and_versions() {
        let value: Value = serde_json::from_str(&summary_json(&streams()[..1])).unwrap();
        let entry = &value["streams"][0];
        assert_eq!(keys(entry), ["n", "codec", "read", "exit", "wall_ms", "run", "decoder", "with_text", "lines", "markers"]);
        assert_eq!(entry["run"], serde_json::from_str::<Value>(&run_json(&totals())).unwrap());
        assert_eq!(entry["decoder"], json!({"bitmap": "lazymkv-bitmap 1", "rows": "lazymkv-rows 1"}));
        assert_eq!((&entry["n"], &entry["codec"], &entry["read"], &entry["exit"], &entry["wall_ms"]),
            (&json!(0), &json!("S_HDMV/PGS"), &json!(true), &json!(0), &json!(6012)));
        assert_eq!((&entry["with_text"], &entry["lines"]), (&json!(3), &json!(5)));
        assert_eq!(entry["markers"], json!({"bracket": 1, "note": 1, "hash": 1, "speaker": 1, "upper": 1}));
        assert!(serde_json::from_value::<OcrSummary>(value).unwrap().unread().is_empty());
    }
}
