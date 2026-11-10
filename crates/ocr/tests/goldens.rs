//! The decoder goldens of design part 10.4 on phase 0's six trial tracks (design part 10.9's table; notes calls 7 and
//! 24; T5-R6, T5-R22, T5-R23, T5-R45). They read the recordings through `LAZYMKV_EVIDENCE`, the tuning dataset through
//! `LAZYMKV_OCR_TEXT` and the six tracks through `LAZYMKV_OCR_TRACKS` (one `<slug>/<label>/s<n>` line per row, the
//! file `C:\lazymkv\phase0\trial-tracks.txt` outside the repository), with `LAZYMKV_OCR_STAGING` as the last place an
//! evidence file is looked for (R6-phase0-20); without any of the first three they print `skipped: <reason>` and pass.
//! They run in the owner's desktop session on the mini PC through `scripts/phase0/ocr-goldens.ps1`, the only session
//! that reaches the share, and print row numbers and counts only: never a slug, a label, a path, a file name or any
//! subtitle text (RP21, the notes' rule on titles). Errors name the row and the kind of file, never its path.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use lazymkv_core::recording::files::{read_jsonl, FrameEntry, Packet, SubsIndexEntry};
use lazymkv_core::subtitle::align::load_packets;
use lazymkv_core::subtitle::bitmap::evidence::check_events;
use lazymkv_core::subtitle::bitmap::mkv::{read_stream, SubStream};
use lazymkv_core::subtitle::bitmap::pgs::PgsDecoder;
use lazymkv_core::subtitle::bitmap::prepare::{crop, prepare, Crop, Grey, INK};
use lazymkv_core::subtitle::bitmap::vobsub::decode_vobsub;
use lazymkv_core::subtitle::bitmap::{Bitmap, SubCodec};
use lazymkv_core::subtitle::text::{ocr_records, packet_events, pair_text, Rule};
use lazymkv_ocr::read_png_grey;
use serde_json::Value;

/// The largest grey difference a paired event may show, for ffmpeg's rounding through RGB (design part 10.4, notes
/// call 7); a larger one is a finding for the owner, never a reason to widen it (T5-R23).
const TOLERANCE: u8 = 2;

/// The rows of design part 10.9's table, one line of the tracks file each.
const ROWS: usize = 6;

/// One row of design part 10.9's table as the tracks file names it; `slug` and `label` only build paths.
struct Track {
    row: usize,
    slug: String,
    label: String,
    n: usize,
}

struct Inputs {
    recordings: PathBuf,
    dataset: PathBuf,
    staging: Option<PathBuf>,
    tracks: Vec<Track>,
}

impl Inputs {
    /// `<recordings>/<slug>/<label>`, the track's recording.
    fn recording(&self, track: &Track) -> PathBuf {
        self.recordings.join(&track.slug).join(&track.label)
    }

    /// `<dataset>/<slug>/<label>/subs-ocr`, phase 0's reading in the tuning dataset.
    fn reading(&self, track: &Track) -> PathBuf {
        self.dataset.join(&track.slug).join(&track.label).join("subs-ocr")
    }
}

/// `skipped: <reason>` on standard error, which the test harness does not capture; the notes count it as not run.
fn skip(reason: &str) {
    let _ = std::io::stderr().write_all(format!("skipped: {reason}\n").as_bytes());
}

/// The folder a variable names, when it is set and is a folder.
fn folder(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).map(PathBuf::from).filter(|path| path.is_dir())
}

/// The inputs, or None after the skip line of the first one missing. A tracks file that is not six
/// `<slug>/<label>/s<n>` lines panics naming the line number only.
fn inputs() -> Option<Inputs> {
    let Some(recordings) = folder("LAZYMKV_EVIDENCE") else {
        skip("no recordings: give LAZYMKV_EVIDENCE, the phase 0 recordings folder (scripts/phase0/ocr-goldens.ps1 sets it in the desktop session)");
        return None;
    };
    let Some(dataset) = folder("LAZYMKV_OCR_TEXT") else {
        skip("no tuning dataset: give LAZYMKV_OCR_TEXT, the folder holding <slug>/<label>/subs-ocr/ (scripts/phase0/ocr-goldens.ps1 sets it)");
        return None;
    };
    let Some(list) = std::env::var_os("LAZYMKV_OCR_TRACKS").map(PathBuf::from).filter(|path| path.is_file()) else {
        skip("no trial tracks: give LAZYMKV_OCR_TRACKS, a file of one <slug>/<label>/s<n> line per row of design part 10.9's table");
        return None;
    };
    let text = std::fs::read_to_string(&list)
        .unwrap_or_else(|error| panic!("the tracks file could not be read ({:?})", error.kind()));
    let mut tracks = Vec::new();
    for (i, line) in text.lines().map(str::trim).filter(|line| !line.is_empty()).enumerate() {
        let parts: Vec<&str> = line.split('/').collect();
        let n = match parts.as_slice() {
            [slug, label, stream] if !slug.is_empty() && !label.is_empty() => {
                stream.strip_prefix('s').and_then(|n| n.parse::<usize>().ok())
            }
            _ => None,
        };
        let n = n.unwrap_or_else(|| panic!("line {} of the tracks file is not <slug>/<label>/s<n>", i + 1));
        tracks.push(Track { row: i + 1, slug: parts[0].to_string(), label: parts[1].to_string(), n });
    }
    assert_eq!(tracks.len(), ROWS, "the tracks file holds {} rows, not design part 10.9's {ROWS}", tracks.len());
    Some(Inputs { recordings, dataset, staging: folder("LAZYMKV_OCR_STAGING"), tracks })
}

/// A file's bytes; the error names the row and what the file is, never its path (RP21).
fn bytes(row: usize, what: &str, path: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|error| format!("row {row}: {what} could not be read ({:?})", error.kind()))
}

/// A UTF-8 text without its byte order mark, which Windows PowerShell 5.1's `Set-Content -Encoding UTF8` writes.
fn text(row: usize, what: &str, path: &Path) -> Result<String, String> {
    let text = String::from_utf8(bytes(row, what, path)?).map_err(|_| format!("row {row}: {what} is not UTF-8"))?;
    Ok(text.trim_start_matches('\u{feff}').to_string())
}

/// A JSON (JavaScript Object Notation) value; the error gives the line only, since serde_json's text can quote the
/// file.
fn json(row: usize, what: &str, path: &Path) -> Result<Value, String> {
    serde_json::from_str(&text(row, what, path)?)
        .map_err(|error| format!("row {row}: {what} does not parse at line {}", error.line()))
}

/// ffprobe's number, written as a string or a number.
fn number(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::String(text) => text.parse().ok(),
        other => other.as_f64(),
    }
}

/// The track's evidence file and where it was found: the dataset's copy, else the path the recording's
/// `subs-ocr/summary.json` names under `evidence`, else `<staging>/<slug>/<label>/evidence.mks` (B2, R6-phase0-18,
/// R6-phase0-20).
fn evidence(inputs: &Inputs, track: &Track) -> Result<(Vec<u8>, &'static str), String> {
    let row = track.row;
    let copy = inputs.reading(track).join("evidence.mks");
    if copy.is_file() {
        return Ok((bytes(row, "the dataset's evidence.mks", &copy)?, "dataset"));
    }
    let summary = inputs.recording(track).join("subs-ocr").join("summary.json");
    if summary.is_file() {
        let named = json(row, "summary.json", &summary)?.get("evidence").and_then(Value::as_str).map(PathBuf::from);
        if let Some(named) = named.filter(|path| path.is_file()) {
            return Ok((bytes(row, "the evidence.mks summary.json names", &named)?, "summary"));
        }
    }
    let staged = inputs.staging.as_ref().map(|staging| staging.join(&track.slug).join(&track.label).join("evidence.mks"));
    if let Some(staged) = staged.filter(|path| path.is_file()) {
        return Ok((bytes(row, "the staged evidence.mks", &staged)?, "staging"));
    }
    Err(format!("row {row}: no evidence.mks in the dataset, at the path summary.json names or in the staging folder"))
}

/// The recording's `subs/sub-<n>.packets.json`, read as the text rule reads it.
fn packets(inputs: &Inputs, track: &Track, n: usize) -> Result<Vec<Packet>, String> {
    let name = format!("sub-{n}.packets.json");
    let path = inputs.recording(track).join("subs").join(&name);
    load_packets(&text(track.row, &name, &path)?, &name).map_err(|error| format!("row {}: {error}", track.row))
}

/// The stream's codec as printed; another codec is an error naming its codec ID.
fn codec_word(row: usize, stream: &SubStream) -> Result<&'static str, String> {
    match stream.codec {
        Some(SubCodec::Pgs) => Ok("PGS"),
        Some(SubCodec::VobSub) => Ok("VobSub"),
        None => Err(format!("row {row}: the stream is {}, not PGS or VobSub", stream.codec_id)),
    }
}

/// One row of the size golden: its printed line and whether it passes (design part 10.3, notes call 24, K1).
fn size_row(inputs: &Inputs, track: &Track) -> Result<(String, bool), String> {
    let row = track.row;
    let (file, source) = evidence(inputs, track)?;
    let stream = read_stream(&file, track.n).map_err(|error| format!("row {row}: {error}"))?;
    let codec = codec_word(row, &stream)?;
    let listed = packets(inputs, track, track.n)?;
    let (mut times, mut sizes) = (0, 0);
    for (block, packet) in stream.blocks.iter().zip(&listed) {
        if number(packet.pts_time.as_ref()).map_or(true, |pts| (pts - block.pts).abs() > 0.0005) {
            times += 1;
        }
        if number(packet.size.as_ref()).map(|size| size as u64) != Some(block.size) {
            sizes += 1;
        }
    }
    let clear_max = Rule::default().CLEAR_MAX_BYTES;
    let events: Vec<(f64, u64)> =
        stream.blocks.iter().filter(|block| block.size as i64 > clear_max).map(|block| (block.pts, block.size)).collect();
    let check = check_events(&events, &listed, clear_max).err();
    // The whole file: every subtitle stream the recording's subs/index.json lists, for K1's two sums.
    let index: Vec<SubsIndexEntry> = serde_json::from_value(json(row, "index.json", &inputs.recording(track).join("subs").join("index.json"))?)
        .map_err(|_| format!("row {row}: index.json is not a list of streams"))?;
    let (mut block_bytes, mut packet_bytes) = (0u64, 0u64);
    for entry in &index {
        let n = usize::try_from(entry.n).map_err(|_| format!("row {row}: index.json lists stream {}", entry.n))?;
        let other = read_stream(&file, n).map_err(|error| format!("row {row}: stream s{n}: {error}"))?;
        block_bytes += other.blocks.iter().map(|block| block.size).sum::<u64>();
        packet_bytes += packets(inputs, track, n)?.iter().filter_map(|packet| number(packet.size.as_ref())).map(|size| size as u64).sum::<u64>();
    }
    let passes = stream.blocks.len() == listed.len() && times == 0 && sizes == 0 && check.is_none();
    let line = format!(
        "row {row}: s{} {codec}, evidence from the {source}, {} bytes; {} blocks, {} packets listed, {times} time and {sizes} size differences, {} events, cross-check {}; all {} subtitle streams: {block_bytes} block bytes, {packet_bytes} packet bytes",
        track.n,
        file.len(),
        stream.blocks.len(),
        listed.len(),
        events.len(),
        check.unwrap_or_else(|| "ok".to_string()),
        index.len(),
    );
    Ok((line, passes))
}

/// The canvas phase 0 drew the stream on: summary.json's `canvas` for the stream (`ocr-subs.ps1` line 230), else what
/// `ocr-subs.ps1` line 162 computes from `subs-ocr/streams.json` (the stream's width x height, else 1920x1080).
fn canvas(inputs: &Inputs, track: &Track) -> Result<((u32, u32), &'static str), String> {
    let row = track.row;
    let folder = inputs.recording(track).join("subs-ocr");
    let summary = json(row, "summary.json", &folder.join("summary.json"))?;
    let named = summary
        .get("streams")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|entry| entry.get("n").and_then(Value::as_u64) == Some(track.n as u64))
        .and_then(|entry| entry.get("canvas"))
        .and_then(Value::as_str);
    if let Some(size) = named {
        let parsed = size.split_once('x').and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)));
        return parsed.map(|size| (size, "summary")).ok_or_else(|| format!("row {row}: summary.json's canvas is not <width>x<height>"));
    }
    let probe = json(row, "streams.json", &folder.join("streams.json"))?;
    let stream = probe
        .get("streams")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|entry| entry.get("codec_type").and_then(Value::as_str) == Some("subtitle"))
        .nth(track.n)
        .ok_or_else(|| format!("row {row}: streams.json has no subtitle stream s{}", track.n))?;
    let side = |key: &str| stream.get(key).and_then(Value::as_u64).filter(|side| *side > 0).map(|side| side as u32);
    Ok(match (side("width"), side("height")) {
        (Some(width), Some(height)) => ((width, height), "streams"),
        _ => ((1920, 1080), "default"),
    })
}

/// The prepared frame on phase 0's canvas: equal sizes give the frame unchanged; otherwise a white canvas with the
/// frame's pixels from its top-left corner, and the row's line prints both sizes.
fn on_canvas(frame: &Grey, (width, height): (u32, u32)) -> Grey {
    if (frame.width, frame.height) == (width, height) {
        return frame.clone();
    }
    let mut pixels = vec![255u8; (width * height) as usize];
    for y in 0..height.min(frame.height) {
        for x in 0..width.min(frame.width) {
            pixels[(y * width + x) as usize] = frame.pixels[(y * frame.width + x) as usize];
        }
    }
    Grey { width, height, pixels }
}

/// (left, top, right, bottom), inclusive, of the pixels at grey 127 or darker (design part 10.5); None without one.
fn dark_box(image: &Grey) -> Option<(u32, u32, u32, u32)> {
    let mut found: Option<(u32, u32, u32, u32)> = None;
    for (i, &grey) in image.pixels.iter().enumerate() {
        if grey > INK {
            continue;
        }
        let (x, y) = (i as u32 % image.width, i as u32 / image.width);
        found = Some(match found {
            None => (x, y, x, y),
            Some((left, top, right, bottom)) => (left.min(x), top.min(y), right.max(x), bottom.max(y)),
        });
    }
    found
}

/// A crop's place and size on its frame (T5-R6).
fn crop_box(crop: &Crop) -> (u32, u32, u32, u32) {
    (crop.left, crop.top, crop.image.width, crop.image.height)
}

/// The largest grey difference between two frames of one size.
fn largest_difference(a: &Grey, b: &Grey) -> u8 {
    a.pixels.iter().zip(&b.pixels).map(|(x, y)| x.abs_diff(*y)).max().unwrap_or(0)
}

#[derive(Default)]
struct Tally {
    paired: usize,
    unpaired_events: usize,
    extra_frames: usize,
    box_mismatches: usize,
    frame_size: usize,
    crop_mismatches: usize,
    largest: u8,
}

/// One row of the decoder golden: its printed line and whether it passes (design part 10.4, notes call 7).
fn decode_row(inputs: &Inputs, track: &Track) -> Result<(String, bool), String> {
    let row = track.row;
    let (file, _) = evidence(inputs, track)?;
    let stream = read_stream(&file, track.n).map_err(|error| format!("row {row}: {error}"))?;
    let codec = codec_word(row, &stream)?;
    let clear_max = Rule::default().CLEAR_MAX_BYTES;
    let blocks: Vec<_> = stream.blocks.iter().filter(|block| block.size as i64 > clear_max).collect();
    let mut events = packet_events(&packets(inputs, track, track.n)?, clear_max).map_err(|error| format!("row {row}: {error}"))?;
    if blocks.len() != events.len() {
        return Err(format!("row {row}: the reader gives {} events and the packet list {}; the size golden names the difference", blocks.len(), events.len()));
    }
    let (canvas, canvas_from) = canvas(inputs, track)?;
    // Phase 0's text frames as pair_text sees them, each carrying its frame file in place of its lines, so that
    // pair_text's own pairing (order on equal counts, else time within 0.30 s) names each event's frames.
    let reading = inputs.reading(track);
    let records_name = format!("sub-{}.ocr.jsonl", track.n);
    let records = read_jsonl(&text(row, &records_name, &reading.join(&records_name))?)
        .map_err(|_| format!("row {row}: {records_name} does not parse"))?;
    let frames_name = format!("sub-{}.frames.json", track.n);
    let frames: Vec<FrameEntry> = serde_json::from_value(json(row, &frames_name, &inputs.recording(track).join("subs-ocr").join(&frames_name))?)
        .map_err(|_| format!("row {row}: {frames_name} is not a list of frames"))?;
    let counts: BTreeMap<i64, Option<i64>> = frames.iter().map(|frame| (frame.frame, frame.lines)).collect();
    let mut text_frames = Vec::new();
    for record in &records {
        let kept = ocr_records(std::slice::from_ref(record), &counts).map_err(|_| format!("row {row}: {records_name} does not read"))?;
        if let Some(mut kept) = kept.into_iter().next() {
            kept.lines = vec![record.file.clone().ok_or_else(|| format!("row {row}: a record of {records_name} names no file"))?];
            text_frames.push(kept);
        }
    }
    let (method, unpaired_frames) = pair_text(&mut events, &text_frames);
    let frame_dir = reading.join(format!("s{}", track.n));
    let mut pgs = PgsDecoder::new();
    let mut tally = Tally::default();
    let mut video = (0, 0);
    for (i, (block, event)) in blocks.iter().zip(&events).enumerate() {
        let decoded: Result<Bitmap, String> = match (stream.codec, stream.idx.as_ref()) {
            (Some(SubCodec::Pgs), _) => pgs.decode(&block.payload),
            (_, Some(idx)) => decode_vobsub(&block.payload, idx),
            (_, None) => Err("the VobSub track has no idx header".to_string()),
        };
        let bitmap = decoded.map_err(|error| format!("row {row}: event {i}: {error}"))?;
        video = (bitmap.video_width, bitmap.video_height);
        if event.lines.is_empty() {
            tally.unpaired_events += 1;
            continue;
        }
        tally.paired += 1;
        let prepared = prepare(&bitmap);
        let drawn = on_canvas(&prepared, canvas);
        let worker = crop(&prepared).map(|crop| crop_box(&crop));
        for (j, name) in event.lines.iter().enumerate() {
            tally.extra_frames += usize::from(j > 0);
            let frame = read_png_grey(&bytes(row, "a phase 0 frame", &frame_dir.join(name))?)
                .map_err(|_| format!("row {row}: event {i}: its frame is not a grey PNG"))?;
            if (frame.width, frame.height) != canvas {
                tally.frame_size += 1;
                tally.box_mismatches += 1;
                continue;
            }
            if dark_box(&drawn) != dark_box(&frame) {
                tally.box_mismatches += 1;
            }
            if worker != crop(&frame).map(|crop| crop_box(&crop)) {
                tally.crop_mismatches += 1;
            }
            tally.largest = tally.largest.max(largest_difference(&drawn, &frame));
        }
    }
    let passes = tally.box_mismatches == 0 && tally.crop_mismatches == 0 && tally.largest <= TOLERANCE;
    let line = format!(
        "row {row}: {codec}, {} events, {} frames, {} text frames paired by {}: {} paired events, {} unpaired events, {unpaired_frames} unpaired frames, {} extra frames; {} box mismatches ({} of frame size), {} crop mismatches, largest difference {}; canvas {}x{} from {canvas_from}, video {}x{}",
        events.len(),
        records.len(),
        text_frames.len(),
        method.as_str(),
        tally.paired,
        tally.unpaired_events,
        tally.extra_frames,
        tally.box_mismatches,
        tally.frame_size,
        tally.crop_mismatches,
        tally.largest,
        canvas.0,
        canvas.1,
        video.0,
        video.1,
    );
    Ok((line, passes))
}

/// Design part 10.3 and notes call 24 (K1): on each trial track the reader's blocks equal the recording's packet
/// list in number, order (each pts within 0.0005 s of the listed `pts_time`) and size once the content encoding is
/// undone, and Task 6's `check_events` accepts its events; never weakened to count and order alone.
#[test]
fn the_reader_sizes_equal_the_recorded_packet_lists_on_the_trial_tracks() {
    let Some(inputs) = inputs() else { return };
    let mut failed = Vec::new();
    for track in &inputs.tracks {
        let (line, passes) = size_row(&inputs, track).unwrap_or_else(|error| (error, false));
        println!("{line}");
        if !passes {
            failed.push(track.row);
        }
    }
    assert!(failed.is_empty(), "rows {failed:?} differ from their packet lists in number, order or size (notes call 24: a stop)");
}

/// Design part 10.4 and notes call 7 (T5-R5, T5-R6, T5-R22, T5-R23): every event of each trial track is decoded and
/// prepared; each event paired with a phase 0 text frame has the frame's dark-pixel box exactly, the crop the worker
/// would read equals the crop of the frame, and no pixel differs by more than 2 grey levels. Unpaired events and
/// frames are counted and printed, not failed (T5-R22).
#[test]
fn the_decoders_match_phase_0s_frames_on_the_trial_tracks() {
    let Some(inputs) = inputs() else { return };
    let mut failed = Vec::new();
    for track in &inputs.tracks {
        let (line, passes) = decode_row(&inputs, track).unwrap_or_else(|error| (error, false));
        println!("{line}");
        if !passes {
            failed.push(track.row);
        }
    }
    assert!(
        failed.is_empty(),
        "rows {failed:?} have a box or crop mismatch, a difference above {TOLERANCE} grey levels or an error (design part 10.4, notes call 7)"
    );
}
