//! Event counts and start-time alignment between subtitle streams (design spec 8.7, its timing bands): the port of
//! the phase 0 Python `subtitle_alignment`. Every input is the JSON that
//! `ffprobe -v error -select_streams s:N -show_entries packet=pts_time,duration_time,size -of json FILE` prints. The
//! caller reads the files (`align_subs_dir` through a `Tree`, `lazymkv-dev align` from the file system for its
//! `name=path` inputs); nothing here touches a disk. `aligned` here is unrounded: `subtitle::text::aligned`
//! (Task 10) is a different function, which rounds to 3 decimals.

use crate::recording::files::{Packet, SubsIndexEntry};
use crate::recording::{
    py_fixed, py_float_value, py_int_value, py_repr_str, read_text, Encoding, Errors, Ordered, PathKey, Tree,
};

/// Words in mkvmerge's codec names of text subtitles: "SubRip/SRT", "SubStationAlpha", "WebVTT", "HDMV TextST".
/// Py: subtitle_alignment.TEXT_CODECS.
pub const TEXT_CODECS: [&str; 5] = ["SubRip", "SRT", "SubStation", "WebVTT", "Text"];

/// The size limits of the packet-size line, in the Python's order. Py: buckets' `(32, 64, 128, 256)`.
const BUCKET_LIMITS: [i64; 4] = [32, 64, 128, 256];

/// The line above the pair list, verbatim. Py: main.
const PAIRS_HEADER: &str = "pairs, with the larger of aligned(A, B) and aligned(B, A):";

/// An English or no-language stream of a subs folder: "s<n>-<lang>-<text|image>[-forced]", its packets file,
/// whether it is text, its index entry. Py: index_streams' tuples.
#[derive(Clone, Debug)]
pub struct IndexStream {
    pub name: String,
    pub packets: PathKey,
    pub text: bool,
    pub entry: SubsIndexEntry,
}

/// One run of the `align` table: the lines printed, the exit code (0, or 2 with `error` holding the argparse
/// message the Python gave: no index.json, no English or no-language stream), and align.json. A run the Python
/// ended with a traceback (a file missing or unreadable, a size or a time that int() or float() refused) is exit 2
/// too (spec 3.4): `error` holds the exception's text and `lines` what was printed before it.
#[derive(Clone, Debug, PartialEq)]
pub struct AlignRun {
    pub lines: Vec<String>,
    pub exit_code: i32,
    pub error: Option<String>,
    pub json: AlignJson,
}

/// align.json (port spec section 5): the table over subs/ as numbers.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AlignJson {
    pub clear_max_bytes: i64,
    pub tolerance_ms: i64,
    pub exit_code: i32,
    pub error: Option<String>,
    /// The streams in table order; on an error, those fully read before it.
    pub streams: Vec<AlignStream>,
    /// aligned(row, column), unrounded, rows and columns in `streams` order; empty on an error.
    pub matrix: Vec<Vec<f64>>,
    /// Every pair, A before B in `streams` order, with the larger of aligned(A, B) and aligned(B, A); empty on an
    /// error.
    pub pairs: Vec<AlignPair>,
}

/// One stream of the table: the numbers of its packet line and its event count.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AlignStream {
    pub name: String,
    /// None for name=path inputs.
    pub codec: Option<String>,
    /// None for name=path inputs.
    pub track_name: Option<String>,
    pub packets: usize,
    pub buckets: [usize; 4],
    pub events: usize,
}

/// One line of the pair list, unrounded.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AlignPair {
    pub a: String,
    pub b: String,
    pub larger: f64,
}

/// `int(p.get("size", 0))`: a packet without a size is 0.
pub(crate) fn size_of(packet: &Packet) -> Result<i64, String> {
    match &packet.size {
        None => Ok(0),
        Some(size) => py_int_value(size),
    }
}

/// Start times, in seconds, of packets large enough to show something: packets with a pts_time and a size above
/// `clear_max_bytes` (a missing size is 0), sorted. Err with the Python's int() or float() text where it raised.
/// Py: events.
pub fn events(packets: &[Packet], clear_max_bytes: i64) -> Result<Vec<f64>, String> {
    let mut starts = Vec::new();
    for packet in packets {
        // `"pts_time" in p and int(p.get("size", 0)) > clear_max_bytes`, then float(p["pts_time"]): the size is
        // converted only for a packet with a start time, and before that time.
        let Some(pts_time) = &packet.pts_time else {
            continue;
        };
        if size_of(packet)? > clear_max_bytes {
            starts.push(py_float_value(pts_time)?);
        }
    }
    // sorted(): ascending and stable; the comparison is total on the finite times ffprobe writes.
    starts.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
    Ok(starts)
}

/// bisect.bisect_left: the first index whose start is not below `x`, by CPython's halving loop.
pub(crate) fn bisect_left(starts: &[f64], x: f64) -> usize {
    let (mut lo, mut hi) = (0, starts.len());
    while lo < hi {
        let mid = (lo + hi) / 2;
        if starts[mid] < x {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

/// Share of `a`'s starts within `tolerance_s` of some start in sorted `b`, unrounded; 0.0 for an empty `a`.
/// Py: subtitle_alignment.aligned.
pub fn aligned(a: &[f64], b: &[f64], tolerance_s: f64) -> f64 {
    if a.is_empty() {
        return 0.0;
    }
    let mut hits = 0usize;
    for &start in a {
        let i = bisect_left(b, start - tolerance_s);
        if i < b.len() && b[i] <= start + tolerance_s {
            hits += 1;
        }
    }
    // hits / len(a): Python's int division is correctly rounded, as this one is.
    hits as f64 / a.len() as f64
}

/// A text subtitle codec name of mkvmerge's; PGS and VobSub are images. Py: is_text.
pub fn is_text(codec: Option<&str>) -> bool {
    let codec = codec.unwrap_or("");
    TEXT_CODECS.iter().any(|word| codec.contains(word))
}

/// Packets of at most 32, 64, 128 and 256 bytes. Py: buckets.
pub fn buckets(packets: &[Packet]) -> Result<[usize; 4], String> {
    // Every size is converted before any is counted, as the Python's list comprehension does.
    let mut sizes = Vec::with_capacity(packets.len());
    for packet in packets {
        sizes.push(size_of(packet)?);
    }
    let mut counts = [0usize; 4];
    for (count, limit) in counts.iter_mut().zip(BUCKET_LIMITS) {
        *count = sizes.iter().filter(|&&size| size <= limit).count();
    }
    Ok(counts)
}

/// The English and no-language streams of a subs folder, in index order; index.json read as "utf-8-sig" with
/// replacement. Py: index_streams.
pub fn index_streams(tree: &dyn Tree, subs_dir: &PathKey) -> Result<Vec<IndexStream>, String> {
    let index = subs_dir.join("index.json");
    let text = read_text(tree, &index, Encoding::Utf8Sig, Errors::Replace)?;
    let entries: Vec<SubsIndexEntry> =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", tree.display(&index)))?;
    let mut streams = Vec::new();
    for entry in entries {
        // `entry.get("language") or "und"`: no language and "" are both "und".
        let language = match entry.language.as_deref() {
            None | Some("") => "und".to_string(),
            Some(code) => code.to_string(),
        };
        if language != "eng" && language != "und" {
            continue;
        }
        let text = is_text(entry.codec.as_deref());
        let kind = if text { "text" } else { "image" };
        let forced = if entry.forced == Some(true) { "-forced" } else { "" };
        let name = format!("s{}-{language}-{kind}{forced}", entry.n);
        let packets = subs_dir.join(&format!("sub-{}.packets.json", entry.n));
        streams.push(IndexStream { name, packets, text, entry });
    }
    Ok(streams)
}

/// AttributeError's text for `.get` on a JSON value that is not an object.
fn no_get(value: &serde_json::Value) -> String {
    let kind = match value {
        serde_json::Value::Null => "NoneType",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Number(number) if number.is_f64() => "float",
        serde_json::Value::Number(_) => "int",
        serde_json::Value::String(_) => "str",
        serde_json::Value::Array(_) => "list",
        serde_json::Value::Object(_) => "dict",
    };
    format!("'{kind}' object has no attribute 'get'")
}

/// The packets of a packets file's decoded text: `json.loads(text).get("packets", [])` of the Python's load_packets
/// ([] when the key is absent). A top level that is not an object is refused with the AttributeError text `.get`
/// raised (subtitle_text accepts a bare list; this script never did, so `PacketsFile` is not used). `shown`
/// prefixes serde's syntax errors.
pub fn load_packets(text: &str, shown: &str) -> Result<Vec<Packet>, String> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("{shown}: {e}"))?;
    let document = match value {
        serde_json::Value::Object(document) => document,
        other => return Err(no_get(&other)),
    };
    match document.get("packets") {
        None => Ok(Vec::new()),
        Some(packets) => serde_json::from_value(packets.clone()).map_err(|e| format!("{shown}: {e}")),
    }
}

/// argparse's error when `<subs_dir>/index.json` does not exist, `subs_dir` as typed. Py: main's first ap.error.
pub fn no_index_error(subs_dir: &str) -> String {
    format!("no {subs_dir}/index.json: the title has no subtitle track, or the recording was not pulled")
}

/// argparse's error when index.json lists no English or no-language stream. Py: main's second ap.error.
pub fn no_stream_error(subs_dir: &str) -> String {
    format!("{subs_dir}/index.json lists no English or no-language subtitle stream")
}

/// The table header's rule for `--subs-dir`, verbatim. Py: main.
fn subs_dir_rule(clear_max_bytes: i64) -> String {
    format!("image streams ignore packets of {clear_max_bytes} bytes or less, text streams count every packet")
}

/// The table header's rule for `name=path` inputs, verbatim. Py: main.
fn named_rule(clear_max_bytes: i64) -> String {
    format!("packets of {clear_max_bytes} bytes or less are ignored")
}

/// A list of four ints as print() shows it: "[a, b, c, d]".
fn bucket_list(sizes: &[usize; 4]) -> String {
    format!("[{}, {}, {}, {}]", sizes[0], sizes[1], sizes[2], sizes[3])
}

/// What a run has printed and read so far. Py: main's `tracks` dict and its print calls.
struct Table {
    clear_max_bytes: i64,
    tolerance_ms: i64,
    lines: Vec<String>,
    /// name -> (its align.json entry, its start times); a repeated name keeps its first place and takes the last
    /// value, as `tracks[name] = ...` does.
    tracks: Ordered<(AlignStream, Vec<f64>)>,
}

impl Table {
    fn new(clear_max_bytes: i64, tolerance_ms: i64) -> Table {
        Table { clear_max_bytes, tolerance_ms, lines: Vec::new(), tracks: Ordered::new() }
    }

    fn add(&mut self, stream: AlignStream, starts: Vec<f64>) {
        self.tracks.insert(stream.name.clone(), (stream, starts));
    }

    /// The run stops: the Python's ap.error (exit 2) or a crash (a traceback there, exit 2 here). The lines
    /// printed so far stay; align.json lists the streams read so far.
    fn fail(self, message: String) -> AlignRun {
        let streams = self.tracks.iter().map(|(_, (stream, _))| stream.clone()).collect();
        let json = AlignJson {
            clear_max_bytes: self.clear_max_bytes,
            tolerance_ms: self.tolerance_ms,
            exit_code: 2,
            error: Some(message.clone()),
            streams,
            matrix: Vec::new(),
            pairs: Vec::new(),
        };
        AlignRun { lines: self.lines, exit_code: 2, error: Some(message), json }
    }

    /// The rest of main after the packet lines: the event counts, the table and the pairs.
    fn finish(self, rule: &str) -> AlignRun {
        let Table { clear_max_bytes, tolerance_ms, mut lines, tracks } = self;
        let rows: Vec<(&str, &[f64])> =
            tracks.iter().map(|(name, (_, starts))| (name, starts.as_slice())).collect();
        lines.push(String::new());
        for (name, starts) in &rows {
            lines.push(format!("{name}: events={}", starts.len()));
        }
        lines.push(String::new());
        // args.tolerance_ms / 1000: true division of two ints, correctly rounded as this one is.
        let tolerance = tolerance_ms as f64 / 1000.0;
        // max([8] + [len(n) + 2 for n in names]); len() counts characters, as Rust's padding does.
        let width = rows.iter().map(|(name, _)| name.chars().count() + 2).fold(8, usize::max);
        lines.push(format!("aligned(row, column); {rule}; tolerance {tolerance_ms} ms:"));
        let mut header = " ".repeat(width);
        for (name, _) in &rows {
            header.push_str(&format!("{name:>width$}"));
        }
        lines.push(header);
        let mut matrix = Vec::new();
        for (name, row_starts) in &rows {
            let mut row = format!("{name:<width$}");
            let mut values = Vec::new();
            for (_, column_starts) in &rows {
                let value = aligned(row_starts, column_starts, tolerance);
                row.push_str(&format!("{:>width$}", py_fixed(value, 2)));
                values.push(value);
            }
            lines.push(row);
            matrix.push(values);
        }
        lines.push(String::new());
        lines.push(PAIRS_HEADER.to_string());
        let mut pairs = Vec::new();
        for (i, (a, a_starts)) in rows.iter().enumerate() {
            for (b, b_starts) in &rows[i + 1..] {
                let forward = aligned(a_starts, b_starts, tolerance);
                let backward = aligned(b_starts, a_starts, tolerance);
                // max(x, y) keeps x unless y is larger.
                let larger = if backward > forward { backward } else { forward };
                lines.push(format!("{a:<width$}{b:<width$}{}", py_fixed(larger, 2)));
                pairs.push(AlignPair { a: a.to_string(), b: b.to_string(), larger });
            }
        }
        let streams = tracks.iter().map(|(_, (stream, _))| stream.clone()).collect();
        let json = AlignJson { clear_max_bytes, tolerance_ms, exit_code: 0, error: None, streams, matrix, pairs };
        AlignRun { lines, exit_code: 0, error: None, json }
    }
}

/// `--subs-dir DIR`: image streams drop packets of `clear_max_bytes` or less, text streams count every packet. The
/// two argparse errors name the folder as `tree.display` prints it (the command checks them first, with the folder
/// as typed). Py: main's subs-dir branch.
pub fn align_subs_dir(tree: &dyn Tree, subs_dir: &PathKey, clear_max_bytes: i64, tolerance_ms: i64) -> AlignRun {
    let mut table = Table::new(clear_max_bytes, tolerance_ms);
    let shown = tree.display(subs_dir);
    // (Path(DIR) / "index.json").exists(): a file or a folder of that name.
    if !tree.exists(&subs_dir.join("index.json")) {
        return table.fail(no_index_error(&shown));
    }
    let streams = match index_streams(tree, subs_dir) {
        Ok(streams) => streams,
        Err(message) => return table.fail(message),
    };
    if streams.is_empty() {
        return table.fail(no_stream_error(&shown));
    }
    for stream in streams {
        let packets = match read_text(tree, &stream.packets, Encoding::Utf8Sig, Errors::Strict)
            .and_then(|contents| load_packets(&contents, &tree.display(&stream.packets)))
        {
            Ok(packets) => packets,
            Err(message) => return table.fail(message),
        };
        let sizes = match buckets(&packets) {
            Ok(sizes) => sizes,
            Err(message) => return table.fail(message),
        };
        let IndexStream { name, text, entry, .. } = stream;
        // {entry.get('codec')} is str(): None prints as None; {...!r} is repr().
        let codec_shown = entry.codec.as_deref().unwrap_or("None").to_string();
        let track_name_shown = entry.track_name.as_deref().map_or_else(|| "None".to_string(), py_repr_str);
        table.lines.push(format!(
            "{name}: codec={codec_shown} track_name={track_name_shown} packets={} sized<=32/64/128/256: {}",
            packets.len(),
            bucket_list(&sizes)
        ));
        // events(packets, 0 if text else args.clear_max_bytes)
        let limit = if text { 0 } else { clear_max_bytes };
        let starts = match events(&packets, limit) {
            Ok(starts) => starts,
            Err(message) => return table.fail(message),
        };
        let stream = AlignStream {
            name,
            codec: entry.codec,
            track_name: entry.track_name,
            packets: packets.len(),
            buckets: sizes,
            events: starts.len(),
        };
        table.add(stream, starts);
    }
    table.finish(&subs_dir_rule(clear_max_bytes))
}

/// `name=path ...`: every stream drops packets of `clear_max_bytes` or less. `inputs` are the names and the packets
/// the caller read, in command-line order. Py: main's inputs branch.
pub fn align_named(inputs: &[(String, Vec<Packet>)], clear_max_bytes: i64, tolerance_ms: i64) -> AlignRun {
    let mut table = Table::new(clear_max_bytes, tolerance_ms);
    for (name, packets) in inputs {
        let sizes = match buckets(packets) {
            Ok(sizes) => sizes,
            Err(message) => return table.fail(message),
        };
        table.lines.push(format!("{name}: packets={} sized<=32/64/128/256: {}", packets.len(), bucket_list(&sizes)));
        let starts = match events(packets, clear_max_bytes) {
            Ok(starts) => starts,
            Err(message) => return table.fail(message),
        };
        let stream = AlignStream {
            name: name.clone(),
            codec: None,
            track_name: None,
            packets: packets.len(),
            buckets: sizes,
            events: starts.len(),
        };
        table.add(stream, starts);
    }
    table.finish(&named_rule(clear_max_bytes))
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;
    use crate::recording::MemTree;

    /// The byte-order mark probe-title.ps1 writes before index.json.
    const BOM: &str = "\u{feff}";

    /// Fixture S's index.json as json.dumps wrote it (made up): English image streams 0 and 2 (2 forced, its track
    /// name holding a quote), French stream 1 (it has no packets file: it must not be read), text stream 3 without
    /// a language, stream 4 with "" for a language and no codec.
    const INDEX_S: &str = r#"[{"n": 0, "track_id": 3, "language": "eng", "codec": "HDMV PGS", "track_name": null, "forced": false, "default": true}, {"n": 1, "track_id": 4, "language": "fre", "codec": "HDMV PGS", "track_name": null, "forced": false, "default": false}, {"n": 2, "track_id": 5, "language": "eng", "codec": "HDMV PGS", "track_name": "Forced 'only'", "forced": true, "default": false}, {"n": 3, "track_id": 6, "language": null, "codec": "SubRip/SRT", "track_name": "CC", "forced": null, "default": false}, {"n": 4, "track_id": 7, "language": "", "codec": null, "track_name": "x", "forced": false, "default": false}]"#;

    /// What the Python printed (exit 0) for fixture N given as
    /// `main=main.json other-long-name=other.json c=c.json d=d.json empty=empty.json` (64 bytes, 300 ms).
    const OUTPUT_N: [&str; 31] = [
        "main: packets=18 sized<=32/64/128/256: [5, 9, 9, 9]",
        "other-long-name: packets=4 sized<=32/64/128/256: [0, 0, 0, 0]",
        "c: packets=3 sized<=32/64/128/256: [0, 0, 0, 0]",
        "d: packets=5 sized<=32/64/128/256: [0, 0, 0, 0]",
        "empty: packets=2 sized<=32/64/128/256: [2, 2, 2, 2]",
        "",
        "main: events=8",
        "other-long-name: events=4",
        "c: events=3",
        "d: events=5",
        "empty: events=0",
        "",
        "aligned(row, column); packets of 64 bytes or less are ignored; tolerance 300 ms:",
        "                              main  other-long-name                c                d            empty",
        "main                          1.00             0.12             0.38             0.62             0.00",
        "other-long-name               0.25             1.00             0.00             0.25             0.00",
        "c                             1.00             0.00             1.00             1.00             0.00",
        "d                             1.00             0.20             0.60             1.00             0.00",
        "empty                         0.00             0.00             0.00             0.00             0.00",
        "",
        "pairs, with the larger of aligned(A, B) and aligned(B, A):",
        "main             other-long-name  0.25",
        "main             c                1.00",
        "main             d                1.00",
        "main             empty            0.00",
        "other-long-name  c                0.00",
        "other-long-name  d                0.25",
        "other-long-name  empty            0.00",
        "c                d                1.00",
        "c                empty            0.00",
        "d                empty            0.00",
    ];

    /// What the Python printed (exit 0) for `--subs-dir subs` over fixture S (64 bytes, 300 ms).
    const OUTPUT_S: [&str; 24] = [
        "s0-eng-image: codec=HDMV PGS track_name=None packets=20 sized<=32/64/128/256: [10, 10, 10, 10]",
        "s2-eng-image-forced: codec=HDMV PGS track_name=\"Forced 'only'\" packets=4 sized<=32/64/128/256: [2, 2, 2, 2]",
        "s3-und-text: codec=SubRip/SRT track_name='CC' packets=11 sized<=32/64/128/256: [5, 11, 11, 11]",
        "s4-und-image: codec=None track_name='x' packets=0 sized<=32/64/128/256: [0, 0, 0, 0]",
        "",
        "s0-eng-image: events=10",
        "s2-eng-image-forced: events=2",
        "s3-und-text: events=10",
        "s4-und-image: events=0",
        "",
        "aligned(row, column); image streams ignore packets of 64 bytes or less, text streams count every packet; tolerance 300 ms:",
        "                              s0-eng-image  s2-eng-image-forced          s3-und-text         s4-und-image",
        "s0-eng-image                          1.00                 0.20                 1.00                 0.00",
        "s2-eng-image-forced                   1.00                 1.00                 1.00                 0.00",
        "s3-und-text                           1.00                 0.20                 1.00                 0.00",
        "s4-und-image                          0.00                 0.00                 0.00                 0.00",
        "",
        "pairs, with the larger of aligned(A, B) and aligned(B, A):",
        "s0-eng-image         s2-eng-image-forced  1.00",
        "s0-eng-image         s3-und-text          1.00",
        "s0-eng-image         s4-und-image         0.00",
        "s2-eng-image-forced  s3-und-text          1.00",
        "s2-eng-image-forced  s4-und-image         0.00",
        "s3-und-text          s4-und-image         0.00",
    ];

    /// What the Python printed (exit 0) for `--subs-dir subs --clear-max-bytes 31 --tolerance-ms 100`.
    const OUTPUT_S2: [&str; 24] = [
        "s0-eng-image: codec=HDMV PGS track_name=None packets=20 sized<=32/64/128/256: [10, 10, 10, 10]",
        "s2-eng-image-forced: codec=HDMV PGS track_name=\"Forced 'only'\" packets=4 sized<=32/64/128/256: [2, 2, 2, 2]",
        "s3-und-text: codec=SubRip/SRT track_name='CC' packets=11 sized<=32/64/128/256: [5, 11, 11, 11]",
        "s4-und-image: codec=None track_name='x' packets=0 sized<=32/64/128/256: [0, 0, 0, 0]",
        "",
        "s0-eng-image: events=10",
        "s2-eng-image-forced: events=4",
        "s3-und-text: events=10",
        "s4-und-image: events=0",
        "",
        "aligned(row, column); image streams ignore packets of 31 bytes or less, text streams count every packet; tolerance 100 ms:",
        "                              s0-eng-image  s2-eng-image-forced          s3-und-text         s4-und-image",
        "s0-eng-image                          1.00                 0.20                 1.00                 0.00",
        "s2-eng-image-forced                   0.50                 1.00                 0.50                 0.00",
        "s3-und-text                           1.00                 0.20                 1.00                 0.00",
        "s4-und-image                          0.00                 0.00                 0.00                 0.00",
        "",
        "pairs, with the larger of aligned(A, B) and aligned(B, A):",
        "s0-eng-image         s2-eng-image-forced  0.50",
        "s0-eng-image         s3-und-text          1.00",
        "s0-eng-image         s4-und-image         0.00",
        "s2-eng-image-forced  s3-und-text          0.50",
        "s2-eng-image-forced  s4-und-image         0.00",
        "s3-und-text          s4-und-image         0.00",
    ];

    /// What the Python printed (exit 0) for `a=first.json b=other.json a=last.json`: the name `a` given twice.
    const OUTPUT_REPEATED: [&str; 14] = [
        "a: packets=2 sized<=32/64/128/256: [0, 0, 0, 0]",
        "b: packets=1 sized<=32/64/128/256: [0, 0, 0, 0]",
        "a: packets=1 sized<=32/64/128/256: [0, 0, 0, 0]",
        "",
        "a: events=1",
        "b: events=1",
        "",
        "aligned(row, column); packets of 64 bytes or less are ignored; tolerance 300 ms:",
        "               a       b",
        "a           1.00    0.00",
        "b           0.00    1.00",
        "",
        "pairs, with the larger of aligned(A, B) and aligned(B, A):",
        "a       b       0.00",
    ];

    /// Packets from a JSON list.
    fn packets(json: &str) -> Vec<Packet> {
        serde_json::from_str(json).unwrap()
    }

    /// A JSON string, as ffprobe writes sizes and times.
    fn string(text: &str) -> Option<Value> {
        Some(Value::from(text))
    }

    /// A packet with the keys given (align never reads duration_time).
    fn packet(pts_time: Option<&str>, size: Option<Value>) -> Packet {
        Packet { pts_time: pts_time.map(Value::from), duration_time: None, size }
    }

    /// A packets file's text, {"packets": [...]}, each packet with the keys it has.
    fn packets_text(packets: &[Packet]) -> String {
        let items: Vec<Value> = packets
            .iter()
            .map(|packet| {
                let mut item = serde_json::Map::new();
                if let Some(pts_time) = &packet.pts_time {
                    item.insert("pts_time".to_string(), pts_time.clone());
                }
                if let Some(size) = &packet.size {
                    item.insert("size".to_string(), size.clone());
                }
                Value::Object(item)
            })
            .collect();
        serde_json::json!({ "packets": items }).to_string()
    }

    /// Fixture N: five `name=path` inputs (made up; the Python read files holding exactly these packets). main has
    /// eight 5000-byte starts at 1..8 s, eight clear packets of 30 and 60 bytes, a packet without a time and one
    /// without a size; c's sizes are JSON ints.
    fn named_inputs() -> Vec<(String, Vec<Packet>)> {
        let mut main = Vec::new();
        for s in 1..=8 {
            main.push(packet(Some(format!("{s}.000000").as_str()), string("5000")));
            let clear = if s % 2 == 1 { "30" } else { "60" };
            main.push(packet(Some(format!("{s}.500000").as_str()), string(clear)));
        }
        main.push(packet(None, string("4000")));
        main.push(packet(Some("20.000000"), None));
        let other: Vec<Packet> = ["1.250000", "10.000000", "20.000000", "30.000000"]
            .into_iter()
            .map(|t| packet(Some(t), string("3000")))
            .collect();
        let c: Vec<Packet> = ["2.000000", "3.000000", "5.000000"]
            .into_iter()
            .map(|t| packet(Some(t), Some(Value::from(1000))))
            .collect();
        let d: Vec<Packet> = (1..=5).map(|s| packet(Some(format!("{s}.000000").as_str()), string("700"))).collect();
        let empty = vec![packet(Some("4.000000"), string("10")), packet(Some("6.000000"), string("20"))];
        vec![
            ("main".to_string(), main),
            ("other-long-name".to_string(), other),
            ("c".to_string(), c),
            ("d".to_string(), d),
            ("empty".to_string(), empty),
        ]
    }

    /// Fixture S under "subs/" (display root ".", so its paths print as "subs/..."), without the files named in
    /// `without`. Made up; the Python read the same packets from disk.
    fn subs_tree(without: &[&str]) -> MemTree {
        let mut s0 = Vec::new();
        for i in 0..10 {
            s0.push(packet(Some(format!("{}.000000", 10 + 4 * i).as_str()), string("5000")));
            s0.push(packet(Some(format!("{}.000000", 12 + 4 * i).as_str()), string("30")));
        }
        let s2 = vec![
            packet(Some("14.000000"), string("3000")),
            packet(Some("16.000000"), string("32")),
            packet(Some("30.000000"), string("3000")),
            packet(Some("32.000000"), string("32")),
        ];
        let mut s3: Vec<Packet> = (0..10)
            .map(|i| {
                let size = (20 + (i % 3) * 20).to_string();
                packet(Some(format!("{}.100000", 10 + 4 * i).as_str()), string(&size))
            })
            .collect();
        s3.push(packet(Some("50.000000"), string("0")));
        let files = [
            ("subs/index.json", format!("{BOM}{INDEX_S}")),
            ("subs/sub-0.packets.json", packets_text(&s0)),
            ("subs/sub-2.packets.json", packets_text(&s2)),
            ("subs/sub-3.packets.json", packets_text(&s3)),
            ("subs/sub-4.packets.json", r#"{"format": {}}"#.to_string()),
        ];
        let mut tree = MemTree::new(".", "rec");
        for (path, text) in files {
            if !without.contains(&path) {
                tree.insert_file(path, text);
            }
        }
        tree
    }

    /// align.json's streams as tuples: name, codec, track name, packets, buckets, events.
    fn streams_of(json: &AlignJson) -> Vec<(&str, Option<&str>, Option<&str>, usize, [usize; 4], usize)> {
        json.streams
            .iter()
            .map(|s| (s.name.as_str(), s.codec.as_deref(), s.track_name.as_deref(), s.packets, s.buckets, s.events))
            .collect()
    }

    /// align.json's pairs as tuples.
    fn pairs_of(json: &AlignJson) -> Vec<(&str, &str, f64)> {
        json.pairs.iter().map(|p| (p.a.as_str(), p.b.as_str(), p.larger)).collect()
    }

    // Ported from test_analysis, class Alignment.

    #[test]
    fn events_drop_small_clear_packets() {
        let read = packets(
            r#"[{"pts_time": "1.000", "size": "5000"}, {"pts_time": "3.000", "size": "30"},
                {"pts_time": "9.500", "size": "4000"}, {"size": "4000"}]"#,
        );
        assert_eq!(events(&read, 64).unwrap(), vec![1.0, 9.5]);
    }

    #[test]
    fn aligned_is_the_share_of_events_with_a_near_start() {
        let a = [1.0, 2.0, 3.0];
        let b = [1.1, 2.5, 3.0];
        // assertAlmostEqual: equal once rounded to 7 places.
        assert!((aligned(&a, &b, 0.3) - 2.0 / 3.0).abs() < 0.5e-7);
        assert_eq!(aligned(&[], &b, 0.3), 0.0);
        assert_eq!(aligned(&a, &[], 0.3), 0.0);
    }

    // Ported from test_analysis, class SubsDir (the three of its five tests that call functions; the two that run
    // main() are in crates/dev/src/cmd/align.rs).

    #[test]
    fn text_codecs_are_told_from_image_codecs() {
        for codec in ["SubRip/SRT", "SubStationAlpha", "WebVTT", "HDMV TextST"] {
            assert!(is_text(Some(codec)), "{codec}");
        }
        for codec in [Some("HDMV PGS"), Some("VobSub"), Some(""), None] {
            assert!(!is_text(codec), "{codec:?}");
        }
    }

    #[test]
    fn only_english_and_no_language_streams_are_kept() {
        let index = r#"[{"n": 0, "language": "eng", "codec": "HDMV PGS", "track_name": null, "forced": false}, {"n": 1, "language": "fre", "codec": "HDMV PGS", "track_name": null, "forced": false}, {"n": 2, "language": "eng", "codec": "HDMV PGS", "track_name": null, "forced": true}, {"n": 3, "language": null, "codec": "SubRip/SRT", "track_name": null, "forced": null}]"#;
        let mut tree = MemTree::new(".", "rec");
        tree.insert_file("subs/index.json", format!("{BOM}{index}"));
        let names: Vec<String> =
            index_streams(&tree, &PathKey::new("subs")).unwrap().into_iter().map(|s| s.name).collect();
        assert_eq!(names, ["s0-eng-image", "s2-eng-image-forced", "s3-und-text"]);
    }

    #[test]
    fn an_index_in_another_encoding_is_still_read() {
        let text = r#"[{"n": 0, "language": "eng", "codec": "VobSub", "track_name": "Fran_ais", "forced": false}]"#;
        let bytes: Vec<u8> = text.bytes().map(|b| if b == b'_' { 0xe7 } else { b }).collect();
        let mut tree = MemTree::new(".", "rec");
        tree.insert_file("subs/index.json", bytes);
        let names: Vec<String> =
            index_streams(&tree, &PathKey::new("subs")).unwrap().into_iter().map(|s| s.name).collect();
        assert_eq!(names, ["s0-eng-image"]);
    }

    // New in the port.

    #[test]
    fn events_convert_sizes_and_times_as_int_and_float_do() {
        // A JSON int size, a float size int() truncates, a time with spaces around it, a packet without a size (0),
        // a float time with a size holding an underscore (int("1_000") is 1000); sorted on the way out. The Python
        // gave [0.5, 3.0, 7.5] with 64 and [0.5, 2.25, 3.0, 7.5] with 0 for these packets.
        let read = packets(
            r#"[{"pts_time": "7.5", "size": 65}, {"pts_time": " 2.25 ", "size": 64.9},
                {"pts_time": "3", "size": "65"}, {"pts_time": "1e0"}, {"pts_time": 0.5, "size": "1_000"}]"#,
        );
        assert_eq!(events(&read, 64), Ok(vec![0.5, 3.0, 7.5]));
        assert_eq!(events(&read, 0), Ok(vec![0.5, 2.25, 3.0, 7.5]));
        // The size is converted first, and only for a packet with a start time; then the time. The Python's
        // ValueError texts.
        let refused = [
            (r#"[{"pts_time": "1.000000", "size": "abc"}]"#, "invalid literal for int() with base 10: 'abc'"),
            (r#"[{"pts_time": "x", "size": "900"}]"#, "could not convert string to float: 'x'"),
            (r#"[{"pts_time": "x", "size": "abc"}]"#, "invalid literal for int() with base 10: 'abc'"),
        ];
        for (json, message) in refused {
            assert_eq!(events(&packets(json), 64), Err(message.to_string()), "{json}");
        }
        assert_eq!(events(&packets(r#"[{"pts_time": "x", "size": "10"}]"#), 64), Ok(vec![]));
        assert_eq!(events(&packets(r#"[{"size": "abc"}]"#), 64), Ok(vec![]));
    }

    #[test]
    fn aligned_counts_a_start_at_either_edge_of_the_band() {
        // bisect_left finds a start equal to start - tol, and `<=` keeps one equal to start + tol (values exact in
        // binary); the Python gave 1.0, 1.0, 0.0 and 1.0.
        assert_eq!(aligned(&[1.0], &[0.5], 0.5), 1.0);
        assert_eq!(aligned(&[1.0], &[1.5], 0.5), 1.0);
        assert_eq!(aligned(&[1.0], &[0.25, 1.75], 0.5), 0.0);
        // Each start of `a` is looked up on its own: two may match the same start of `b`.
        assert_eq!(aligned(&[1.0, 1.25], &[1.125], 0.5), 1.0);
        // 1 of 8 is a tie at two decimals: '%.2f' rounds it half to even, and so does py_fixed.
        let share = aligned(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0], &[1.25], 0.3);
        assert_eq!(share, 0.125);
        assert_eq!(py_fixed(share, 2), "0.12");
    }

    #[test]
    fn buckets_count_the_sizes_up_to_each_limit() {
        // Sizes 32, 33, 64, 65, 128, 256, 257 and a packet without one (0): the Python gave [2, 4, 6, 7].
        let read = packets(
            r#"[{"size": "32"}, {"size": "33"}, {"size": 64}, {"size": "65"}, {"size": "128"},
                {"size": "256"}, {"size": "257"}, {}]"#,
        );
        assert_eq!(buckets(&read), Ok([2, 4, 6, 7]));
        // Every size is converted before any is counted.
        let refused = packets(r#"[{"size": "10"}, {"size": "1.5"}]"#);
        assert_eq!(buckets(&refused), Err("invalid literal for int() with base 10: '1.5'".to_string()));
    }

    #[test]
    fn index_streams_name_the_packets_file_and_keep_the_entry() {
        let tree = subs_tree(&[]);
        let streams = index_streams(&tree, &PathKey::new("subs")).unwrap();
        let seen: Vec<(&str, String, bool, i64, Option<i64>)> = streams
            .iter()
            .map(|s| (s.name.as_str(), s.packets.to_string(), s.text, s.entry.n, s.entry.track_id))
            .collect();
        assert_eq!(
            seen,
            vec![
                ("s0-eng-image", "subs/sub-0.packets.json".to_string(), false, 0, Some(3)),
                ("s2-eng-image-forced", "subs/sub-2.packets.json".to_string(), false, 2, Some(5)),
                ("s3-und-text", "subs/sub-3.packets.json".to_string(), true, 3, Some(6)),
                ("s4-und-image", "subs/sub-4.packets.json".to_string(), false, 4, Some(7)),
            ]
        );
        // Without index.json: the tree's FileNotFoundError text, as the Python's read_text raised it.
        let mut bare = MemTree::new(".", "rec");
        bare.insert_dir("subs");
        assert_eq!(
            index_streams(&bare, &PathKey::new("subs")).unwrap_err(),
            "[Errno 2] No such file or directory: 'subs/index.json'"
        );
    }

    #[test]
    fn align_named_prints_what_the_python_printed() {
        // 1 of 8 (0.125) and 5 of 8 (0.625) print as 0.12 and 0.62: half to even, as '%.2f'.
        let run = align_named(&named_inputs(), 64, 300);
        assert_eq!(run.lines, OUTPUT_N);
        assert_eq!((run.exit_code, run.error.as_deref()), (0, None));
        let json = &run.json;
        assert_eq!((json.clear_max_bytes, json.tolerance_ms, json.exit_code, json.error.as_deref()), (64, 300, 0, None));
        assert_eq!(
            streams_of(json),
            vec![
                ("main", None, None, 18, [5, 9, 9, 9], 8),
                ("other-long-name", None, None, 4, [0, 0, 0, 0], 4),
                ("c", None, None, 3, [0, 0, 0, 0], 3),
                ("d", None, None, 5, [0, 0, 0, 0], 5),
                ("empty", None, None, 2, [2, 2, 2, 2], 0),
            ]
        );
        assert_eq!(
            json.matrix,
            vec![
                vec![1.0, 0.125, 0.375, 0.625, 0.0],
                vec![0.25, 1.0, 0.0, 0.25, 0.0],
                vec![1.0, 0.0, 1.0, 1.0, 0.0],
                vec![1.0, 0.2, 0.6, 1.0, 0.0],
                vec![0.0, 0.0, 0.0, 0.0, 0.0],
            ]
        );
        assert_eq!(
            pairs_of(json),
            vec![
                ("main", "other-long-name", 0.25),
                ("main", "c", 1.0),
                ("main", "d", 1.0),
                ("main", "empty", 0.0),
                ("other-long-name", "c", 0.0),
                ("other-long-name", "d", 0.25),
                ("other-long-name", "empty", 0.0),
                ("c", "d", 1.0),
                ("c", "empty", 0.0),
                ("d", "empty", 0.0),
            ]
        );
    }

    #[test]
    fn align_subs_dir_prints_what_the_python_printed() {
        let tree = subs_tree(&[]);
        let run = align_subs_dir(&tree, &PathKey::new("subs"), 64, 300);
        assert_eq!(run.lines, OUTPUT_S);
        assert_eq!((run.exit_code, run.error.as_deref()), (0, None));
        assert_eq!((run.json.clear_max_bytes, run.json.tolerance_ms, run.json.exit_code), (64, 300, 0));
        assert_eq!(
            streams_of(&run.json),
            vec![
                ("s0-eng-image", Some("HDMV PGS"), None, 20, [10, 10, 10, 10], 10),
                ("s2-eng-image-forced", Some("HDMV PGS"), Some("Forced 'only'"), 4, [2, 2, 2, 2], 2),
                ("s3-und-text", Some("SubRip/SRT"), Some("CC"), 11, [5, 11, 11, 11], 10),
                ("s4-und-image", None, Some("x"), 0, [0, 0, 0, 0], 0),
            ]
        );
        assert_eq!(
            run.json.matrix,
            vec![
                vec![1.0, 0.2, 1.0, 0.0],
                vec![1.0, 1.0, 1.0, 0.0],
                vec![1.0, 0.2, 1.0, 0.0],
                vec![0.0, 0.0, 0.0, 0.0],
            ]
        );
        assert_eq!(
            pairs_of(&run.json),
            vec![
                ("s0-eng-image", "s2-eng-image-forced", 1.0),
                ("s0-eng-image", "s3-und-text", 1.0),
                ("s0-eng-image", "s4-und-image", 0.0),
                ("s2-eng-image-forced", "s3-und-text", 1.0),
                ("s2-eng-image-forced", "s4-und-image", 0.0),
                ("s3-und-text", "s4-und-image", 0.0),
            ]
        );
        // 31 bytes: stream 2's 32-byte packets now count; 100 ms: a text start 0.1 s after an image start sits on
        // the band's edge, where only the Python's binary arithmetic decides.
        let narrow = align_subs_dir(&tree, &PathKey::new("subs"), 31, 100);
        assert_eq!(narrow.lines, OUTPUT_S2);
        assert_eq!(narrow.json.matrix[1], vec![0.5, 1.0, 0.5, 0.0]);
    }

    #[test]
    fn a_folder_without_index_or_english_stream_is_refused_as_argparse_refused_it() {
        let mut bare = MemTree::new(".", "rec");
        bare.insert_dir("subs");
        let run = align_subs_dir(&bare, &PathKey::new("subs"), 64, 300);
        let message = "no subs/index.json: the title has no subtitle track, or the recording was not pulled";
        assert!(run.lines.is_empty());
        assert_eq!((run.exit_code, run.error.as_deref()), (2, Some(message)));
        assert_eq!((run.json.exit_code, run.json.error.as_deref()), (2, Some(message)));
        assert!(run.json.streams.is_empty() && run.json.matrix.is_empty() && run.json.pairs.is_empty());

        let mut french = MemTree::new(".", "rec");
        let index = r#"[{"n": 0, "language": "fre", "codec": "VobSub", "track_name": null, "forced": false}]"#;
        french.insert_file("subs/index.json", format!("{BOM}{index}"));
        let run = align_subs_dir(&french, &PathKey::new("subs"), 64, 300);
        let message = "subs/index.json lists no English or no-language subtitle stream";
        assert!(run.lines.is_empty());
        assert_eq!((run.exit_code, run.error.as_deref()), (2, Some(message)));

        // The command prints both with the folder exactly as typed (the Python's f-strings used args.subs_dir).
        assert_eq!(
            no_index_error("nothere/"),
            "no nothere//index.json: the title has no subtitle track, or the recording was not pulled"
        );
        assert_eq!(no_stream_error("noeng"), "noeng/index.json lists no English or no-language subtitle stream");
    }

    #[test]
    fn a_packets_file_that_cannot_be_read_ends_the_run_after_the_lines_before_it() {
        // The Python printed the lines of streams 0 and 2, then raised FileNotFoundError for stream 3's file.
        let tree = subs_tree(&["subs/sub-3.packets.json"]);
        let run = align_subs_dir(&tree, &PathKey::new("subs"), 64, 300);
        assert_eq!(run.lines, OUTPUT_S[..2]);
        let message = "[Errno 2] No such file or directory: 'subs/sub-3.packets.json'";
        assert_eq!((run.exit_code, run.error.as_deref()), (2, Some(message)));
        let names: Vec<&str> = run.json.streams.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["s0-eng-image", "s2-eng-image-forced"]);
        assert!(run.json.matrix.is_empty() && run.json.pairs.is_empty());
    }

    #[test]
    fn load_packets_takes_the_packets_of_an_object_only() {
        let one = load_packets(r#"{"packets": [{"pts_time": "1.0", "size": "9"}]}"#, "x.json").unwrap();
        assert_eq!(one.len(), 1);
        // .get("packets", []): an object without the key has no packets.
        assert!(load_packets(r#"{"format": {}}"#, "x.json").unwrap().is_empty());
        // .get on anything but a dict raised AttributeError in the Python (a bare list too, empty or not).
        let refused = [
            ("[]", "list"),
            (r#"[{"pts_time": "1.0", "size": "900"}]"#, "list"),
            ("\"x\"", "str"),
            ("1", "int"),
            ("1.5", "float"),
            ("true", "bool"),
            ("null", "NoneType"),
        ];
        for (text, kind) in refused {
            let message = format!("'{kind}' object has no attribute 'get'");
            assert_eq!(load_packets(text, "x.json").unwrap_err(), message, "{text}");
        }
        // Not JSON: serde's message after the file's name (the Python's JSONDecodeError text is not reproduced).
        assert!(load_packets("{", "x.json").unwrap_err().starts_with("x.json: "));
    }

    #[test]
    fn a_repeated_name_keeps_its_first_place_and_takes_the_last_events() {
        // `tracks[name] = ...` on a dict: the second `a` replaces the first's events in the first's place, after
        // printing its own packet line.
        let first = vec![packet(Some("1.000000"), string("900")), packet(Some("2.000000"), string("900"))];
        let other = vec![packet(Some("1.000000"), string("900"))];
        let last = vec![packet(Some("5.000000"), string("900"))];
        let inputs = [("a".to_string(), first), ("b".to_string(), other), ("a".to_string(), last)];
        let run = align_named(&inputs, 64, 300);
        assert_eq!(run.lines, OUTPUT_REPEATED);
        let streams: Vec<(&str, usize, usize)> =
            run.json.streams.iter().map(|s| (s.name.as_str(), s.packets, s.events)).collect();
        assert_eq!(streams, vec![("a", 1, 1), ("b", 1, 1)]);
        assert_eq!(run.json.matrix, vec![vec![1.0, 0.0], vec![0.0, 1.0]]);
    }
}
