//! The recorder's files as the PowerShell jobs and crates/capture write them (port spec 3.3, `core::recording`), and
//! what makes a folder a rip label. Unknown keys are ignored; every field the Python reads with .get() is an Option.
//! A file whose field has another type fails to parse, and its reader reports it unreadable (02-interfaces.md, open
//! point 10: every recorded rip-summary.json has `ok` a bool, `title` and `bytes` ints).

use super::{py_split, py_splitlines, py_strip, py_truthy, read_json, Ordered, PathKey, Tree};
use serde_json::Value;

/// The rip's track selection, from the profile's file name. Py: compare_tracks --selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Selection {
    All,
    Eng,
}

impl Selection {
    /// "all" or "eng".
    pub fn as_str(self) -> &'static str {
        match self {
            Selection::All => "all",
            Selection::Eng => "eng",
        }
    }

    /// The selection a --selection value names, spelt exactly; None for anything else.
    pub fn parse(text: &str) -> Option<Selection> {
        match text {
            "all" => Some(Selection::All),
            "eng" => Some(Selection::Eng),
            _ => None,
        }
    }
}

/// meta.json (crates/capture): the disc's volume label, format ("bluray" or "dvd"), fingerprint and size.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct Meta {
    pub volume_label: Option<String>,
    pub format: Option<String>,
    pub fingerprint: Option<String>,
    pub disc_id: Option<String>,
    pub file_count: Option<i64>,
    pub total_bytes: Option<i64>,
    pub captured_unix: Option<f64>,
}

/// One entry of files.json (a list of these).
#[derive(Clone, Debug, serde::Deserialize)]
pub struct FilesEntry {
    pub path: String,
    pub size: i64,
}

/// rip-summary.json (rip-title.ps1) and extract-summary.json (extract-from-copy.ps1): the fields the Python reads.
/// Py: titles_report, ripped, same_rip, _scan_flags, exp11 match.
#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize)]
pub struct Summary {
    pub ok: Option<bool>,
    pub result: Option<String>,
    pub slug: Option<String>,
    pub label: Option<String>,
    pub title: Option<i64>,
    pub profile: Option<String>,
    pub bytes: Option<i64>,
}

/// probe-summary.json (probe-title.ps1).
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct ProbeSummary {
    pub ok: Option<bool>,
    pub result: Option<String>,
    pub file: Option<String>,
    pub file_bytes: Option<i64>,
    pub subtitle_tracks: Option<i64>,
}

/// mkvmerge -J output, the parts read. Py: compare_tracks.file_rows/checks, exp11, levels_args.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct MkvmergeIdent {
    #[serde(default)]
    pub tracks: Vec<MkvTrack>,
    #[serde(default)]
    pub chapters: Vec<MkvChapters>,
    #[serde(default)]
    pub container: MkvContainer,
}

/// One track of mkvmerge -J.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct MkvTrack {
    pub id: i64,
    #[serde(rename = "type")]
    pub r#type: String,
    #[serde(default)]
    pub codec: String,
    /// Every property as mkvmerge wrote it (language, audio_channels, tag_*, forced_track, track_name, ...).
    #[serde(default)]
    pub properties: serde_json::Map<String, Value>,
}

/// One chapter edition of mkvmerge -J.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct MkvChapters {
    #[serde(default)]
    pub num_entries: i64,
}

/// mkvmerge -J's container.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct MkvContainer {
    #[serde(default)]
    pub properties: MkvContainerProperties,
}

/// duration in nanoseconds (0 when absent, as `.get("duration", 0)`).
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct MkvContainerProperties {
    #[serde(default)]
    pub duration: i64,
}

/// ffprobe-streams.json and a level folder's streams.json. Py: ffprobe_audio, read_streams, _scan_flags,
/// audio_positions, no_audio, levels_args.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct FfprobeStreams {
    #[serde(default)]
    pub streams: Vec<FfprobeStream>,
}

/// One ffprobe stream. An absent key stays absent when the stream is written again, as ffprobe and proxy_streams
/// leave it out.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FfprobeStream {
    pub index: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codec_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codec_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channels: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_layout: Option<String>,
    /// ffprobe's tags (language, title, ...), in file order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Ordered<String>>,
}

/// One entry of subs/index.json (a list). Py: subtitle_alignment.index_streams, subtitle_text.load_title.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct SubsIndexEntry {
    pub n: i64,
    pub track_id: Option<i64>,
    pub language: Option<String>,
    pub codec: Option<String>,
    pub track_name: Option<String>,
    pub forced: Option<bool>,
    pub default: Option<bool>,
}

/// One packet of sub-<n>.packets.json; ffprobe writes the three as strings. Py: events, packet_events, buckets.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct Packet {
    pub pts_time: Option<Value>,
    pub duration_time: Option<Value>,
    pub size: Option<Value>,
}

/// sub-<n>.packets.json: `{"packets": [...]}`, or a bare list (packet_events accepts both).
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(untagged)]
pub enum PacketsFile {
    Object {
        #[serde(default)]
        packets: Vec<Packet>,
    },
    List(Vec<Packet>),
}

impl PacketsFile {
    /// The packets, whichever form the file has.
    pub fn packets(&self) -> &[Packet] {
        match self {
            PacketsFile::Object { packets } => packets,
            PacketsFile::List(packets) => packets,
        }
    }
}

/// subs-ocr/summary.json (ocr-subs.ps1) and ocrs-summary.json (ocrs-trial.ps1): their stream entries.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct OcrSummary {
    #[serde(default)]
    pub streams: Vec<serde_json::Map<String, Value>>,
}

/// One entry of subs-ocr/sub-<n>.frames.json (a list): the frame number and its OCR line count.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct FrameEntry {
    pub frame: i64,
    pub lines: Option<i64>,
}

/// One record of sub-<n>.ocr.jsonl or ocrs-s<n>[-crop<pad>].jsonl. Holds subtitle text: in memory only, never
/// written, printed or hashed field by field. Py: ocr_records, ocr_compare.read_frames.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct OcrLine {
    pub frame: Option<i64>,
    pub file: Option<String>,
    pub t: Option<f64>,
    pub pts: Option<f64>,
    /// A list in the final format; anything else means "use `text`" (isinstance(r.get('lines'), list)).
    pub lines: Option<Value>,
    pub text: Option<String>,
    /// ocrs-trial's `{"summary": {...}}` line.
    pub summary: Option<serde_json::Map<String, Value>>,
    /// The other keys (ms, decode_ms, detect_ms, rec_ms, crop, crop_ms), presence kept.
    #[serde(flatten)]
    pub rest: serde_json::Map<String, Value>,
}

/// Every non-blank line (py_strip) of a JSON-lines text, parsed; lines split by py_splitlines. Err is serde_json's
/// text for the first line that is not a record. Py: the two jsonl readers.
pub fn read_jsonl(text: &str) -> Result<Vec<OcrLine>, String> {
    let mut out = Vec::new();
    for line in py_splitlines(text) {
        if py_strip(line).is_empty() {
            continue;
        }
        out.push(serde_json::from_str(line).map_err(|error| error.to_string())?);
    }
    Ok(out)
}

/// tmdb.json, slimmed (original_language, fetched_at).
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct Tmdb {
    pub original_language: Option<String>,
    pub fetched_at: Option<String>,
}

/// What thediscdb-hash.json / thediscdb-discid.json says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiscDbAnswer {
    Hit,
    Miss,
    NotADisc,
}

/// A non-empty list or an object with a truthy "titles" is a hit, an empty list a miss, anything else not a disc
/// record. Py: hit_rate.lookup.
pub fn thediscdb_answer(value: &Value) -> DiscDbAnswer {
    match value {
        Value::Array(items) if items.is_empty() => DiscDbAnswer::Miss,
        Value::Array(_) => DiscDbAnswer::Hit,
        Value::Object(map) if map.get("titles").map_or(false, py_truthy) => DiscDbAnswer::Hit,
        _ => DiscDbAnswer::NotADisc,
    }
}

/// lookup-status.txt: `{parts[0]: parts[-1]}` for every line (py_splitlines) with two or more py_split parts.
/// Py: hit_rate.statuses.
pub fn lookup_statuses(text: &str) -> Ordered<String> {
    let mut out = Ordered::new();
    for line in py_splitlines(text) {
        let parts = py_split(line);
        if parts.len() >= 2 {
            out.insert(parts[0].to_string(), parts[parts.len() - 1].to_string());
        }
    }
    out
}

/// A rip label of a recording.
#[derive(Clone, Debug, PartialEq)]
pub struct RipLabel {
    pub label: String,
    /// "rip-summary.json" or "extract-summary.json", whichever says ok (rip-summary.json first, as same_rip).
    pub summary_file: &'static str,
    pub summary: Summary,
    pub title: i64,
    pub selection: Selection,
}

/// The summaries that make a rip label, in the order same_rip reads them.
const SUMMARY_FILES: [&str; 2] = ["rip-summary.json", "extract-summary.json"];

/// Every rip label of the recording at `slug_dir`, in PathKey order: a folder (never `golden`) with an ok-saying
/// summary and an mkvmerge.json.
pub fn rip_labels(tree: &dyn Tree, slug_dir: &PathKey) -> Vec<RipLabel> {
    tree.list(slug_dir)
        .iter()
        .filter(|path| tree.is_dir(path))
        .filter_map(|path| rip_label(tree, slug_dir, path.name()).ok())
        .collect()
}

/// The rip label `label` of `slug_dir`, or why it is not one: "<label>: no rip-summary.json or extract-summary.json
/// saying ok" (a summary that cannot be read counts as not saying ok, as same_rip skips it), "<label>: no
/// mkvmerge.json", "<label>: the summary names no title", or, for a folder named golden, "golden: the folder name
/// golden is reserved for goldens; it is not a rip label".
pub fn rip_label(tree: &dyn Tree, slug_dir: &PathKey, label: &str) -> Result<RipLabel, String> {
    if label == "golden" {
        return Err(format!("{}: the folder name golden is reserved for goldens; it is not a rip label", label));
    }
    let dir = slug_dir.join(label);
    let mut found = None;
    for name in SUMMARY_FILES {
        if let Ok(summary) = read_json::<Summary>(tree, &dir.join(name)) {
            if summary.ok == Some(true) {
                found = Some((name, summary));
                break;
            }
        }
    }
    let (summary_file, summary) =
        found.ok_or_else(|| format!("{}: no rip-summary.json or extract-summary.json saying ok", label))?;
    if !tree.is_file(&dir.join("mkvmerge.json")) {
        return Err(format!("{}: no mkvmerge.json", label));
    }
    let title = title_of(&summary).ok_or_else(|| format!("{}: the summary names no title", label))?;
    let selection = selection_of(summary.profile.as_deref());
    Ok(RipLabel { label: label.to_string(), summary_file, summary, title, selection })
}

/// The title number a summary names.
pub fn title_of(summary: &Summary) -> Option<i64> {
    summary.title
}

/// `eng` when the profile's file name (the part after the last '\\', never Path::file_name: it is a Windows path)
/// is `eng-only.mmcp.xml`, else `all` (also for no profile: extract-summary.json names none).
pub fn selection_of(profile_path: Option<&str>) -> Selection {
    match profile_path {
        Some(path) if path.rsplit('\\').next() == Some("eng-only.mmcp.xml") => Selection::Eng,
        _ => Selection::All,
    }
}

/// The disc format meta.json states, as crates/capture writes it ("bluray" or "dvd").
pub fn disc_format(meta: &Meta) -> Option<&str> {
    meta.format.as_deref()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recording::MemTree;
    use serde_json::json;

    /// A recording with three rip labels and five folders that are not: a failed rip, a rip without mkvmerge.json,
    /// a summary without a title, a golden folder, and a missing one.
    fn recording() -> MemTree {
        let mut tree = MemTree::new("corpus/owner", "owner");
        tree.insert_file(
            "disc-a/all-tracks/rip-summary.json",
            r#"{"ok": true, "result": "ok", "slug": "disc-a", "label": "all-tracks", "title": 0, "profile": "C:\\lazymkv\\profiles\\all-tracks.mmcp.xml", "bytes": 1000}"#,
        );
        tree.insert_file("disc-a/all-tracks/mkvmerge.json", "{}");
        tree.insert_file(
            "disc-a/eng-only/rip-summary.json",
            r#"{"ok": true, "title": 0, "profile": "C:\\lazymkv\\profiles\\eng-only.mmcp.xml"}"#,
        );
        tree.insert_file("disc-a/eng-only/mkvmerge.json", "{}");
        tree.insert_file("disc-a/extract/extract-summary.json", "\u{feff}{\"ok\": true, \"title\": 3, \"bytes\": null}");
        tree.insert_file("disc-a/extract/mkvmerge.json", "{}");
        tree.insert_file("disc-a/failed/rip-summary.json", r#"{"ok": false, "title": 0}"#);
        tree.insert_file("disc-a/failed/mkvmerge.json", "{}");
        tree.insert_file("disc-a/no-file/rip-summary.json", r#"{"ok": true, "title": 1}"#);
        tree.insert_file("disc-a/no-title/rip-summary.json", r#"{"ok": true}"#);
        tree.insert_file("disc-a/no-title/mkvmerge.json", "{}");
        tree.insert_file("disc-a/golden/rip-summary.json", r#"{"ok": true, "title": 0}"#);
        tree.insert_file("disc-a/golden/mkvmerge.json", "{}");
        tree.insert_file("disc-a/scan.txt", "");
        tree
    }

    #[test]
    fn rip_labels_are_folders_with_an_ok_summary_and_mkvmerge_json() {
        let tree = recording();
        let labels = rip_labels(&tree, &PathKey::new("disc-a"));
        let names: Vec<&str> = labels.iter().map(|label| label.label.as_str()).collect();
        assert_eq!(names, vec!["all-tracks", "eng-only", "extract"]);
        assert_eq!((labels[0].title, labels[0].selection, labels[0].summary_file), (0, Selection::All, "rip-summary.json"));
        assert_eq!((labels[1].title, labels[1].selection), (0, Selection::Eng));
        assert_eq!(
            (labels[2].title, labels[2].selection, labels[2].summary_file),
            (3, Selection::All, "extract-summary.json")
        );
        assert_eq!(labels[0].summary.bytes, Some(1000));
        assert_eq!(labels[2].summary.bytes, None);
    }

    #[test]
    fn rip_label_says_why_a_folder_is_not_a_rip_label() {
        let tree = recording();
        let slug = PathKey::new("disc-a");
        assert_eq!(
            rip_label(&tree, &slug, "failed").unwrap_err(),
            "failed: no rip-summary.json or extract-summary.json saying ok"
        );
        assert_eq!(rip_label(&tree, &slug, "no-file").unwrap_err(), "no-file: no mkvmerge.json");
        assert_eq!(rip_label(&tree, &slug, "no-title").unwrap_err(), "no-title: the summary names no title");
        assert_eq!(
            rip_label(&tree, &slug, "missing").unwrap_err(),
            "missing: no rip-summary.json or extract-summary.json saying ok"
        );
        assert_eq!(
            rip_label(&tree, &slug, "golden").unwrap_err(),
            "golden: the folder name golden is reserved for goldens; it is not a rip label"
        );
        assert_eq!(rip_label(&tree, &slug, "all-tracks").unwrap().label, "all-tracks");
    }

    #[test]
    fn selection_comes_from_the_windows_profile_name() {
        assert_eq!(selection_of(Some("C:\\lazymkv\\profiles\\eng-only.mmcp.xml")), Selection::Eng);
        assert_eq!(selection_of(Some("C:\\lazymkv\\profiles\\all-tracks.mmcp.xml")), Selection::All);
        assert_eq!(selection_of(Some("eng-only.mmcp.xml")), Selection::Eng);
        assert_eq!(selection_of(Some("C:/profiles/eng-only.mmcp.xml")), Selection::All);
        assert_eq!(selection_of(None), Selection::All);
        assert_eq!(Selection::Eng.as_str(), "eng");
        assert_eq!(Selection::parse("all"), Some(Selection::All));
        assert_eq!(Selection::parse("ENG"), None);
        assert_eq!(serde_json::to_string(&Selection::Eng).unwrap(), "\"eng\"");
        assert_eq!(serde_json::from_str::<Selection>("\"all\"").unwrap(), Selection::All);
    }

    #[test]
    fn recorder_files_parse_with_their_defaults() {
        let streams: FfprobeStreams = serde_json::from_str(
            r#"{"streams": [{"index": 1, "codec_type": "audio", "codec_name": "ac3", "channels": 6, "tags": {"title": "Main", "language": "eng"}}]}"#,
        )
        .unwrap();
        let stream = &streams.streams[0];
        assert_eq!((stream.index, stream.channels, stream.profile.clone()), (1, Some(6), None));
        let tags: Vec<(&str, &String)> = stream.tags.as_ref().unwrap().iter().collect();
        assert_eq!(tags, vec![("title", &"Main".to_string()), ("language", &"eng".to_string())]);
        assert_eq!(
            serde_json::to_string(stream).unwrap(),
            r#"{"index":1,"codec_type":"audio","codec_name":"ac3","channels":6,"tags":{"title":"Main","language":"eng"}}"#
        );
        let ident: MkvmergeIdent =
            serde_json::from_str(r#"{"tracks": [{"id": 0, "type": "video", "properties": {"language": "und"}}]}"#).unwrap();
        assert_eq!((ident.tracks[0].id, ident.tracks[0].r#type.as_str(), ident.tracks[0].codec.as_str()), (0, "video", ""));
        assert_eq!(ident.tracks[0].properties.get("language"), Some(&json!("und")));
        assert_eq!((ident.chapters.len(), ident.container.properties.duration), (0, 0));
        let meta: Meta = serde_json::from_str(r#"{"format": "dvd", "file_count": 2, "captured_unix": 1700000000}"#).unwrap();
        assert_eq!(disc_format(&meta), Some("dvd"));
        assert_eq!(meta.captured_unix, Some(1700000000.0));
        let tmdb: Tmdb = serde_json::from_str(r#"{"original_language": "es", "fetched_at": "2026-10-01"}"#).unwrap();
        assert_eq!(tmdb.original_language.as_deref(), Some("es"));
        let summary: Summary = serde_json::from_str(r#"{"ok": true, "title": 2, "unknown": [1]}"#).unwrap();
        assert_eq!(title_of(&summary), Some(2));
        let frames: Vec<FrameEntry> = serde_json::from_str(r#"[{"frame": 4, "lines": 2}, {"frame": 5}]"#).unwrap();
        assert_eq!((frames[0].frame, frames[0].lines, frames[1].lines), (4, Some(2), None));
    }

    #[test]
    fn packets_jsonl_discdb_and_lookup_status_read_as_the_python() {
        let object: PacketsFile =
            serde_json::from_str(r#"{"packets": [{"pts_time": "1.5", "duration_time": "2.0", "size": "120"}]}"#).unwrap();
        let list: PacketsFile = serde_json::from_str(r#"[{"pts_time": "1.5"}]"#).unwrap();
        let empty: PacketsFile = serde_json::from_str("{}").unwrap();
        assert_eq!(object.packets()[0].size, Some(json!("120")));
        assert_eq!(list.packets()[0].duration_time, None);
        assert!(empty.packets().is_empty());
        let lines =
            read_jsonl("{\"frame\": 1, \"lines\": [\"LINE ONE\"], \"ms\": 12}\n\n  \n{\"summary\": {\"frames\": 1}}\r\n").unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!((lines[0].frame, lines[0].rest.get("ms")), (Some(1), Some(&json!(12))));
        assert!(lines[0].lines.as_ref().map_or(false, |value| value.is_array()));
        assert!(lines[1].summary.is_some());
        assert!(read_jsonl("{\"frame\": 1}\nnot json\n").is_err());
        assert_eq!(thediscdb_answer(&json!([{"id": 1}])), DiscDbAnswer::Hit);
        assert_eq!(thediscdb_answer(&json!([])), DiscDbAnswer::Miss);
        assert_eq!(thediscdb_answer(&json!({"titles": [1]})), DiscDbAnswer::Hit);
        assert_eq!(thediscdb_answer(&json!({"titles": []})), DiscDbAnswer::NotADisc);
        assert_eq!(thediscdb_answer(&json!("error")), DiscDbAnswer::NotADisc);
        let statuses = lookup_statuses("dischash  GET 200\ndiscid 404\nlonely\ndischash x 500\n");
        assert_eq!(
            statuses.0,
            vec![("dischash".to_string(), "500".to_string()), ("discid".to_string(), "404".to_string())]
        );
    }
}
