//! The worker's one run: its arguments, one stream of `evidence.mks` read into records, crops and `run.json`, and its
//! exit code (design parts 10.2 steps 3 and 4, 10.7, 10.12; call 5).

use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use lazymkv_core::subtitle::bitmap::evidence::{
    crop_name, record_line, run_json, EventRecord, Method, RunTotals, StageMs, StageTotals,
};
use lazymkv_core::subtitle::bitmap::lines::{line_boxes, median_line_height, rows, trigger, LineBox, Rows};
use lazymkv_core::subtitle::bitmap::mkv::{self, SubStream};
use lazymkv_core::subtitle::bitmap::pgs::PgsDecoder;
use lazymkv_core::subtitle::bitmap::prepare::{crop, crop_key, prepare, Crop};
use lazymkv_core::subtitle::bitmap::vobsub::decode_vobsub;
use lazymkv_core::subtitle::bitmap::{Bitmap, SubCodec};

use crate::{engine_id, grey_png, Ocr};

/// The worker's command line (notes call 5). Def: T5-8. Use: T5-8.
pub const USAGE: &str = "usage: lazymkv-ocr <evidence.mks> --stream N --clear-max BYTES --out DIR";

/// The worker's four arguments: the evidence file, the subtitle-relative stream number (T5-R3), the job's
/// `sub_clear_max_bytes` (T5-R59) and the output folder (design part 10.2 step 3, B1 (the worker's folder)).
/// Def: T5-8. Use: T5-8.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkerArgs {
    /// The evidence.mks to read; only read, never written.
    pub evidence: PathBuf,
    /// ffprobe's subtitle-relative index of the stream to read (T5-R3).
    pub stream: usize,
    /// A packet of this many bytes or fewer is a clear, not an event (spec 8.7).
    pub clear_max: i64,
    /// The folder every output goes into, and the only one written.
    pub out: PathBuf,
}

/// Stores `value` into `slot`, refusing a flag given a second time.
fn once<T>(slot: &mut Option<T>, flag: &str, value: T) -> Result<(), String> {
    if slot.is_some() {
        return Err(format!("{flag} is given twice"));
    }
    *slot = Some(value);
    Ok(())
}

/// Reads the worker's arguments in any order (notes call 5). Def: T5-8. Use: T5-8.
pub fn parse_args(args: &[String]) -> Result<WorkerArgs, String> {
    let (mut evidence, mut stream, mut clear_max, mut out) = (None, None, None, None);
    let mut at = 0;
    while at < args.len() {
        let argument = args[at].as_str();
        at += 1;
        let unknown = || format!("unknown argument '{argument}'");
        if !argument.starts_with("--") {
            if evidence.is_some() {
                return Err(unknown());
            }
            evidence = Some(PathBuf::from(argument));
            continue;
        }
        if !matches!(argument, "--stream" | "--clear-max" | "--out") {
            return Err(unknown());
        }
        let value = args.get(at).ok_or_else(|| format!("{argument} needs a value"))?.as_str();
        at += 1;
        match argument {
            "--stream" => {
                let n = value.parse::<usize>().map_err(|_| format!("--stream takes a whole number, not '{value}'"))?;
                once(&mut stream, argument, n)?;
            }
            "--clear-max" => {
                let bytes = value
                    .parse::<i64>()
                    .ok()
                    .filter(|bytes| *bytes >= 0)
                    .ok_or_else(|| format!("--clear-max takes a whole number of bytes, not '{value}'"))?;
                once(&mut clear_max, argument, bytes)?;
            }
            _ => once(&mut out, argument, PathBuf::from(value))?,
        }
    }
    Ok(WorkerArgs {
        evidence: evidence.ok_or("missing <evidence.mks>")?,
        stream: stream.ok_or("missing --stream")?,
        clear_max: clear_max.ok_or("missing --clear-max")?,
        out: out.ok_or("missing --out")?,
    })
}

/// The refusal of a stream that is not PGS or VobSub (T5-R11).
fn not_bitmap(n: usize, codec_id: &str) -> String {
    format!("subtitle stream {n} has codec {codec_id}, which is not PGS or VobSub")
}

/// Reads subtitle stream `n` of the evidence file and refuses a codec other than PGS and VobSub, before any model is
/// loaded (design parts 10.3, 10.8; T5-R11). Def: T5-8. Use: T5-8.
pub fn open_stream(bytes: &[u8], n: usize) -> Result<SubStream, String> {
    let stream = mkv::read_stream(bytes, n)?;
    if stream.codec.is_none() {
        return Err(not_bitmap(n, &stream.codec_id));
    }
    Ok(stream)
}

/// The milliseconds since `start`.
fn ms_since(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

/// An event's crop after the first pass: its rows, the first event with its key, and the crop itself only for that
/// first event, until it is read.
struct Found {
    first: usize,
    rows: Rows,
    crop: Option<Crop>,
}

/// One event after the first pass (design part 10.5's memory figure: one image per distinct crop).
struct Event {
    pts: f64,
    size: u64,
    video_height: u32,
    decode: f64,
    prepare: f64,
    found: Option<Found>,
}

/// What a distinct crop's first event found, which its repeats take.
#[derive(Clone)]
struct Reading {
    file: String,
    lines: Vec<String>,
    boxes: Vec<LineBox>,
    method: Method,
    trigger: Option<u8>,
}

/// The first pass: every event decoded and prepared, its crop keyed and its rows found.
fn decode_all(stream: &SubStream, n: usize, clear_max: i64) -> Result<Vec<Event>, String> {
    let mut pgs = PgsDecoder::new();
    let idx = match (stream.codec, stream.idx.as_ref()) {
        (Some(SubCodec::Pgs), _) => None,
        (Some(SubCodec::VobSub), Some(idx)) => Some(idx),
        (Some(SubCodec::VobSub), None) => return Err(format!("subtitle stream {n} has no idx header")),
        (None, _) => return Err(not_bitmap(n, &stream.codec_id)),
    };
    let mut firsts: HashMap<[u8; 20], usize> = HashMap::new();
    let mut events = Vec::new();
    for block in stream.blocks.iter().filter(|block| block.size as i64 > clear_max) {
        let (i, pts) = (events.len(), block.pts);
        let start = Instant::now();
        let decoded: Result<Bitmap, String> = match idx {
            None => pgs.decode(&block.payload),
            Some(idx) => decode_vobsub(&block.payload, idx),
        };
        let bitmap = decoded.map_err(|error| format!("event {i} at {pts:.3} s does not decode: {error}"))?;
        let decode = ms_since(start);
        let start = Instant::now();
        let cropped = crop(&prepare(&bitmap)).map(|crop| (crop_key(&crop.image), crop));
        let prepare = ms_since(start);
        let found = cropped.map(|(key, crop)| {
            let rows = rows(&crop.image);
            let first = *firsts.entry(key).or_insert(i);
            Found { first, rows, crop: (first == i).then_some(crop) }
        });
        events.push(Event { pts, size: block.size, video_height: bitmap.video_height, decode, prepare, found });
    }
    Ok(events)
}

/// Reads one stream's events into `sub-<n>.ocr.jsonl`, `s<n>\f-<ms>.png` and, last, `sub-<n>.run.json` in
/// `args.out` (design parts 10.2 step 4, 10.5 to 10.7, 10.12). Every event is decoded and each distinct crop is read
/// once; the stream's median line height is taken over every event before any event is read. Def: T5-8. Use: T5-8.
pub fn read_stream(ocr: &Ocr, stream: &SubStream, args: &WorkerArgs, started: Instant) -> Result<RunTotals, String> {
    let n = args.stream;
    let mut events = decode_all(stream, n, args.clear_max)?;
    let all_rows: Vec<Rows> = events.iter().filter_map(|e| e.found.as_ref().map(|f| f.rows.clone())).collect();
    let median = median_line_height(&all_rows);
    drop(all_rows);

    let crops_name = format!("s{n}");
    let crops = args.out.join(&crops_name);
    std::fs::create_dir_all(&crops).map_err(|error| format!("could not write {crops_name}: {error}"))?;
    let records_name = format!("sub-{n}.ocr.jsonl");
    let wrote = |name: &str| {
        let name = name.to_string();
        move |error: std::io::Error| format!("could not write {name}: {error}")
    };
    let mut records = File::create(args.out.join(&records_name)).map_err(wrote(&records_name))?;
    let engine = engine_id();
    let mut readings: HashMap<usize, Reading> = HashMap::new();
    let mut totals = RunTotals {
        events: events.len(),
        distinct: 0,
        rows: 0,
        detector: [0; 4],
        ms: StageTotals::default(),
        engine: engine.clone(),
        wall_ms: 0,
    };
    for (i, event) in events.iter_mut().enumerate() {
        let pts = event.pts;
        let mut ms = StageMs { decode: event.decode, prepare: event.prepare, lines: 0.0, detect: None, recognize: 0.0 };
        let reading = match event.found.as_mut() {
            None => Reading {
                file: String::new(),
                lines: Vec::new(),
                boxes: Vec::new(),
                method: Method::Rows,
                trigger: None,
            },
            Some(found) => match found.crop.take() {
                None => readings[&found.first].clone(),
                Some(crop) => {
                    let failed = |error: String| format!("ocrs failed on event {i} at {pts:.3} s: {error}");
                    let start = Instant::now();
                    let fired = trigger(&found.rows, median, &crop, event.video_height);
                    let (boxes, lines, method) = match fired {
                        Some(_) => {
                            ms.lines = ms_since(start);
                            let detected = ocr.detect(&crop.image).map_err(failed)?;
                            ms.detect = Some(detected.detect_ms);
                            ms.recognize = detected.recognize_ms;
                            (detected.boxes, detected.lines, Method::Detector)
                        }
                        None => {
                            let boxes = line_boxes(&crop.image, &found.rows);
                            ms.lines = ms_since(start);
                            let start = Instant::now();
                            let lines = ocr.recognize(&crop.image, &boxes).map_err(failed)?;
                            ms.recognize = ms_since(start);
                            (boxes, lines, Method::Rows)
                        }
                    };
                    let file = crop_name(pts);
                    let png = grey_png(&crop.image).map_err(|error| format!("could not write {file}: {error}"))?;
                    std::fs::write(crops.join(&file), png).map_err(wrote(&file))?;
                    totals.distinct += 1;
                    let reading = Reading { file, lines, boxes, method, trigger: fired };
                    readings.insert(i, reading.clone());
                    reading
                }
            },
        };
        if reading.method == Method::Rows {
            totals.rows += 1;
        }
        if let Some(k) = reading.trigger {
            totals.detector[usize::from(k) - 1] += 1;
        }
        totals.ms.decode += ms.decode;
        totals.ms.prepare += ms.prepare;
        totals.ms.lines += ms.lines;
        totals.ms.detect += ms.detect.unwrap_or(0.0);
        totals.ms.recognize += ms.recognize;
        let record = EventRecord {
            frame: i,
            file: reading.file,
            pts,
            lines: reading.lines,
            size: event.size,
            boxes: reading.boxes,
            method: reading.method,
            trigger: reading.trigger,
            engine: engine.clone(),
            ms,
        };
        records.write_all(record_line(&record).as_bytes()).map_err(wrote(&records_name))?;
    }
    drop(records);
    totals.wall_ms = started.elapsed().as_millis() as u64;
    let run_name = format!("sub-{n}.run.json");
    std::fs::write(args.out.join(&run_name), run_json(&totals)).map_err(wrote(&run_name))?;
    Ok(totals)
}

/// The worker's run: exit 0 when the stream was read, 2 on an argument error with the message and the usage on
/// `err`, 3 when the stream is refused with the reason alone on `err` (notes call 5; T5-R8 reads exit 3's first line
/// as the note's reason). Def: T5-8. Use: T5-8.
pub fn main_with(args: &[String], err: &mut dyn Write) -> i32 {
    let started = Instant::now();
    let args = match parse_args(args) {
        Ok(args) => args,
        Err(message) => {
            let _ = write!(err, "{message}\n{USAGE}\n");
            return 2;
        }
    };
    let run = || -> Result<RunTotals, String> {
        let bytes =
            std::fs::read(&args.evidence).map_err(|error| format!("the evidence file could not be read: {error}"))?;
        let stream = open_stream(&bytes, args.stream)?;
        drop(bytes);
        let ocr = Ocr::load()?;
        std::fs::create_dir_all(&args.out).map_err(|error| format!("the output folder could not be made: {error}"))?;
        read_stream(&ocr, &stream, &args, started)
    };
    match run() {
        Ok(_) => 0,
        Err(reason) => {
            let _ = writeln!(err, "{reason}");
            3
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|arg| arg.to_string()).collect()
    }

    #[test]
    fn parse_args_reads_the_four_arguments_in_any_order() {
        let want = WorkerArgs {
            evidence: PathBuf::from("evidence.mks"),
            stream: 1,
            clear_max: 64,
            out: PathBuf::from("out"),
        };
        let forward = strings(&["evidence.mks", "--stream", "1", "--clear-max", "64", "--out", "out"]);
        let shuffled = strings(&["--out", "out", "--clear-max", "64", "evidence.mks", "--stream", "1"]);
        assert_eq!(parse_args(&forward), Ok(want.clone()));
        assert_eq!(parse_args(&shuffled), Ok(want));
    }

    #[test]
    fn parse_args_refuses_a_negative_clear_max_a_repeated_flag_and_a_second_path() {
        let cases = [
            (vec!["e.mks", "--stream", "0", "--clear-max", "-1", "--out", "o"], "--clear-max takes a whole number of bytes, not '-1'"),
            (vec!["e.mks", "--stream", "0", "--stream", "1", "--clear-max", "64", "--out", "o"], "--stream is given twice"),
            (vec!["e.mks", "f.mks", "--stream", "0", "--clear-max", "64", "--out", "o"], "unknown argument 'f.mks'"),
            (vec!["e.mks", "--stream", "0", "--clear-max", "64", "--out"], "--out needs a value"),
        ];
        for (args, message) in cases {
            assert_eq!(parse_args(&strings(&args)), Err(message.to_string()), "{args:?}");
        }
    }
}
