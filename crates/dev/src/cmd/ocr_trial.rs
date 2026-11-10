//! `lazymkv-dev ocr-trial`, the OCR (optical character recognition) trial of design part 10.9 (P17, P26, P34; the
//! notes' A2, B2, B3, calls 8 and 19; Rulings T5-R24, T5-R44, T5-R62): the inventory, arm R through the worker as the
//! engine starts it, arm R' in process, the per-frame records of arms F and R', the numbers-only summary of
//! `subs-ocr-engine`, and the parity of R against F. Compiled only with the cargo feature `ocr-trial`, so rten reaches
//! no other `lazymkv-dev` build (A2). It prints counts, times and file base names only: subtitle text stays in the
//! files it reads and writes.

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use lazymkv_core::ipc::OutputStream;
use lazymkv_core::recording::files::{read_jsonl, FrameEntry, OcrLine, PacketsFile, SubsIndexEntry};
use lazymkv_core::recording::{decode_text, py_fixed, py_int, py_repr_str, Encoding, Errors, MemTree, PathKey};
use lazymkv_core::settings::Settings;
use lazymkv_core::subtitle::bitmap::cap::{cap_reason, time_cap_s};
use lazymkv_core::subtitle::bitmap::evidence::{
    read_run_json, summary_json, text_counts, whole_records, RunTotals, StreamOutcome, StreamSummary,
};
use lazymkv_core::subtitle::bitmap::lines::{line_boxes, median_line_height, rows, trigger, Rows};
use lazymkv_core::subtitle::bitmap::prepare::{crop, Grey};
use lazymkv_core::subtitle::text::{packet_events, subs_run, Decision, Measures, PairRow, Rule, SubsRun, Track};
use lazymkv_ocr::{read_png_grey, Ocr};
use lazymkv_platform::{native_guard, write_atomic, Host, ProcessGuard};
use lazymkv_tools::ocr::{end_reason, ocr_args, run_worker, WorkerEnd};
use lazymkv_tools::{CancelToken, Limits, ToolCtx};
use serde_json::Value;

use crate::cmd::ocr_compare::{cer, exact_mean, norm_text};
use crate::cmd::subs;
use crate::evidence::{pathlib_split, Ctx};
use crate::{usage_error, Args};

/// The command table's synopsis (design 10.9). Def: T5-10. Use: T5-10.
pub const SYNOPSIS: &str = "inventory|read|rows|f-records|summary|parity ...";

/// The six subcommands' usage lines, after "usage: lazymkv-dev ", in SYNOPSIS's order. Def: T5-10. Use: T5-10.
pub const USAGE: [&str; 6] = [
    "ocr-trial inventory --rec DIR --text DIR --stream N --staging DIR [--where FILE]",
    "ocr-trial read <evidence.mks> --stream N --clear-max BYTES --packets FILE --out DIR",
    "ocr-trial rows <frames dir> --out FILE",
    "ocr-trial f-records --rec DIR --stream N --in FILE --out FILE",
    "ocr-trial summary --dir DIR --rec DIR",
    "ocr-trial parity --rec DIR --f DIR --r DIR",
];

/// `lazymkv-dev ocr-trial`, registered in COMMANDS (design 10.9 steps 1, 3 and 4). `read` prints `guard native` and
/// calls `read_with` with `lazymkv_platform::native_guard()` and the system's host (T5-R62). Def: T5-10. Use: T5-12.
pub fn run(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    match args.first().map(String::as_str) {
        Some("-h") | Some("--help") => {
            for line in USAGE {
                let _ = writeln!(out, "usage: lazymkv-dev {line}");
            }
            0
        }
        Some("inventory") => run_inventory(ctx, &args[1..], out, err),
        Some("read") => {
            let _ = writeln!(out, "guard native");
            read_with(native_guard().as_ref(), system_host().as_ref(), &args[1..], out, err)
        }
        Some("rows") => run_rows(ctx, &args[1..], out, err),
        Some("f-records") => run_f_records(ctx, &args[1..], out, err),
        Some("summary") => run_summary(ctx, &args[1..], out, err),
        Some("parity") => run_parity(ctx, &args[1..], out, err),
        _ => usage_error(
            err,
            "ocr-trial",
            "ocr-trial inventory|read|rows|f-records|summary|parity ...",
            "the first argument is one of inventory, read, rows, f-records, summary, parity",
        ),
    }
}

// ------------------------------------------------------------------ shared helpers

/// A clock-only host for the systems without `WinHost` and for the tests: the time from `SystemTime`, local time as
/// UTC, and no MakeMKV, volume, install path or process.
#[cfg(any(test, not(windows)))]
struct ClockHost;

#[cfg(any(test, not(windows)))]
impl Host for ClockHost {
    fn now_unix(&self) -> u64 {
        self.now_ms() / 1000
    }

    fn now_ms(&self) -> u64 {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
        now.map(|elapsed| elapsed.as_millis() as u64).unwrap_or(0)
    }

    fn local_date(&self) -> String {
        lazymkv_platform::localtime::format_local_time(self.now_ms(), 0).chars().take(10).collect()
    }

    fn local_time(&self, unix_ms: u64) -> String {
        lazymkv_platform::localtime::format_local_time(unix_ms, 0)
    }

    fn makemkv_app_key(&self) -> Option<String> {
        None
    }

    fn makemkv_debug_log(&self) -> Option<PathBuf> {
        None
    }

    fn running_processes(&self) -> Vec<String> {
        Vec::new()
    }

    fn volume_label(&self, _root: &Path) -> Option<String> {
        None
    }

    fn default_install_path(&self, _tool: lazymkv_core::job::Tool) -> Option<PathBuf> {
        None
    }

    fn user_writable(&self, _path: &Path) -> bool {
        true
    }
}

/// The system's host: `WinHost` on Windows, `ClockHost` elsewhere.
fn system_host() -> Box<dyn Host> {
    #[cfg(windows)]
    {
        Box::new(lazymkv_platform::win::WinHost::new())
    }
    #[cfg(not(windows))]
    {
        Box::new(ClockHost)
    }
}

/// The worker beside this program, the sibling the engine starts too (design 10.7).
fn worker_path() -> io::Result<PathBuf> {
    Ok(std::env::current_exe()?.with_file_name(format!("lazymkv-ocr{}", std::env::consts::EXE_SUFFIX)))
}

/// A path's base name, for messages that name no folder.
fn base(path: &Path) -> String {
    path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default()
}

/// The file's text, read UTF-8 with or without a byte order mark; Err is the system's or the decoder's text alone.
fn read_text(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    decode_text(&bytes, Encoding::Utf8Sig, Errors::Strict)
}

/// `read_text` with the base name before its error.
fn read_named(path: &Path) -> Result<String, String> {
    read_text(path).map_err(|error| format!("{}: {error}", base(path)))
}

/// The JSON-lines records of a file; Err names the file's base name.
fn read_records(path: &Path) -> Result<Vec<OcrLine>, String> {
    read_jsonl(&read_named(path)?).map_err(|error| format!("{}: {error}", base(path)))
}

/// A run.json read back; Err names the file's base name.
fn read_run(path: &Path) -> Result<RunTotals, String> {
    read_run_json(&read_named(path)?).map_err(|error| format!("{}: {error}", base(path)))
}

/// Writes `bytes` whole to `path` through `write_atomic`, its folder made first; Err names the base name only.
fn write_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|error| format!("the folder of {}: {error}", base(path)))?;
    }
    write_atomic(path, bytes).map_err(|error| format!("{}: {error}", base(path)))
}

/// The subcommands, in USAGE's order.
const SUBCOMMANDS: [&str; 6] = ["inventory", "read", "rows", "f-records", "summary", "parity"];

/// An argument error of subcommand `at`: argparse's two lines on `err`, exit 2.
fn usage(err: &mut dyn Write, at: usize, message: &str) -> i32 {
    usage_error(err, &format!("ocr-trial {}", SUBCOMMANDS[at]), USAGE[at], message)
}

/// `end not started: <message>` on `out`, exit 1: the worker was not started.
fn not_started(out: &mut dyn Write, message: &str) -> i32 {
    let _ = writeln!(out, "end not started: {message}");
    1
}

/// A subcommand's refusal: `lazymkv-dev ocr-trial <sub>: <message>` on `err`, exit 1.
fn refuse(err: &mut dyn Write, sub: &str, message: &str) -> i32 {
    let _ = writeln!(err, "lazymkv-dev ocr-trial {sub}: {message}");
    1
}

/// A subcommand's flags and positionals.
struct Parsed {
    flags: BTreeMap<&'static str, String>,
    words: Vec<String>,
}

impl Parsed {
    /// A flag `parse` found (every required flag is there).
    fn get(&self, flag: &str) -> &str {
        self.flags.get(flag).map(String::as_str).unwrap_or("")
    }

    /// A whole number of 0 or more, argparse's type=int.
    fn whole(&self, flag: &str) -> Result<i64, String> {
        let text = self.get(flag);
        py_int(text)
            .filter(|number| *number >= 0)
            .ok_or_else(|| format!("argument {flag}: invalid int value: {}", py_repr_str(text)))
    }
}

/// Parses `args` with exactly the flags of a usage line: `required` must each be given, `optional` may be, and the
/// words left must be the `positional` names, in order.
fn parse(
    args: &[String],
    required: &[&'static str],
    optional: &[&'static str],
    positional: &[&str],
) -> Result<Parsed, String> {
    let declared: Vec<&'static str> = required.iter().chain(optional).copied().collect();
    let mut parsed = Args::new(args, &declared);
    let mut flags = BTreeMap::new();
    for flag in &declared {
        if let Some(value) = parsed.flag(flag)? {
            flags.insert(*flag, value);
        }
    }
    let words = parsed.positionals()?;
    if words.len() > positional.len() {
        return Err(format!("unrecognized arguments: {}", words[positional.len()..].join(" ")));
    }
    let mut missing: Vec<&str> = positional[words.len()..].to_vec();
    missing.extend(required.iter().filter(|flag| !flags.contains_key(*flag)));
    if !missing.is_empty() {
        return Err(format!("the following arguments are required: {}", missing.join(", ")));
    }
    Ok(Parsed { flags, words })
}

/// The displayed events of a packet list, as the rule counts them; Err names the file's base name.
fn events_of(packets: &Path, clear_max: i64) -> Result<usize, String> {
    let name = base(packets);
    let text = read_named(packets)?;
    let file: PacketsFile = serde_json::from_str(&text).map_err(|error| format!("{name}: {error}"))?;
    packet_events(file.packets(), clear_max).map(|events| events.len()).map_err(|error| format!("{name}: {error}"))
}

// ------------------------------------------------------------------ inventory (design 10.9 step 1, B2, T5-R24)

fn run_inventory(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    const AT: usize = 0;
    let parsed = match parse(args, &["--rec", "--text", "--stream", "--staging"], &["--where"], &[]) {
        Ok(parsed) => parsed,
        Err(message) => return usage(err, AT, &message),
    };
    let n = match parsed.whole("--stream") {
        Ok(n) => n,
        Err(message) => return usage(err, AT, &message),
    };
    let rec = ctx.abs(parsed.get("--rec"));
    let text = ctx.abs(parsed.get("--text"));
    let (Some(label), Some(slug)) = (rec.file_name(), rec.parent().and_then(Path::file_name)) else {
        return refuse(err, "inventory", "--rec names no <slug>/<label> folder");
    };
    let (slug, label) = (slug.to_string_lossy().into_owned(), label.to_string_lossy().into_owned());
    let mut missing = Vec::new();

    let frames = match fs::read_dir(text.join(format!("s{n}"))) {
        Ok(entries) => entries
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().is_file())
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|name| name.starts_with("f-") && name.ends_with(".png"))
            .count(),
        Err(_) => 0,
    };
    let tesseract_file = text.join(format!("sub-{n}.ocr.jsonl"));
    let tesseract = if tesseract_file.is_file() {
        match read_records(&tesseract_file) {
            Ok(records) => records.len(),
            Err(message) => return refuse(err, "inventory", &message),
        }
    } else {
        missing.push(format!("missing: sub-{n}.ocr.jsonl (the Tesseract reading)"));
        0
    };
    let f_file = text.join(format!("ocrs-s{n}.jsonl"));
    let f_records = if f_file.is_file() {
        match read_records(&f_file) {
            Ok(records) => Some(records.iter().filter(|record| record.frame.is_some()).count()),
            Err(message) => return refuse(err, "inventory", &message),
        }
    } else {
        None
    };

    let mut candidates: Vec<(PathBuf, &str)> = vec![(text.join("evidence.mks"), "the dataset copy")];
    let summary_file = rec.join("subs-ocr").join("summary.json");
    if summary_file.is_file() {
        let named = read_named(&summary_file)
            .and_then(|body| serde_json::from_str::<Value>(&body).map_err(|e| format!("summary.json: {e}")));
        match named {
            Ok(summary) => {
                if let Some(path) = summary.get("evidence").and_then(Value::as_str) {
                    candidates.push((ctx.abs(path), "the staging path summary.json names"));
                }
            }
            Err(message) => return refuse(err, "inventory", &message),
        }
    }
    // Joined with '/', as the share's layout is written; Windows reads either separator.
    let staging = format!("{}/{slug}/{label}/evidence.mks", parsed.get("--staging"));
    candidates.push((ctx.abs(&staging), "the phase 0 staging layout"));
    let evidence = candidates.into_iter().find(|(path, _)| path.is_file());
    if evidence.is_none() {
        missing.push(
            "missing: evidence.mks (no dataset copy, no path in summary.json, none in the phase 0 staging layout)"
                .to_string(),
        );
    }

    let packets = rec.join("subs").join(format!("sub-{n}.packets.json"));
    let events = if packets.is_file() {
        match events_of(&packets, Rule::default().CLEAR_MAX_BYTES) {
            Ok(events) => events,
            Err(message) => return refuse(err, "inventory", &message),
        }
    } else {
        missing.push(format!("missing: subs/sub-{n}.packets.json (the packet list)"));
        0
    };
    let f_count = f_records.unwrap_or(0);
    if frames == 0 && f_count == 0 {
        missing.push(format!("missing: s{n}/f-*.png and ocrs-s{n}.jsonl (parity cannot be judged on this track)"));
    }
    if !missing.is_empty() {
        for line in &missing {
            let _ = writeln!(out, "{line}");
        }
        return 1;
    }
    let Some((evidence, source)) = evidence else {
        return 1;
    };
    let bytes = match fs::metadata(&evidence) {
        Ok(meta) => meta.len(),
        Err(error) => return refuse(err, "inventory", &format!("evidence.mks: {error}")),
    };
    if let Some(where_file) = parsed.flags.get("--where") {
        let line = format!("{}\n", evidence.display());
        if let Err(message) = write_file(&ctx.abs(where_file), line.as_bytes()) {
            return refuse(err, "inventory", &message);
        }
    }
    let f_reading = match f_records {
        Some(k) => format!("{k} records"),
        None => "none".to_string(),
    };
    let _ = writeln!(
        out,
        "frames {frames}, Tesseract records {tesseract}, F reading {f_reading}, evidence.mks {bytes} bytes from {source}, \
         events {events}; R' {}; F {}",
        if frames > 0 { "run" } else { "not run" },
        if f_count > 0 { "exists" } else { "to run" },
    );
    0
}

// ------------------------------------------------------------------ read, arm R (design 10.9 step 3, 10.8, T5-R44)

/// Arm R (design 10.9 step 3, K2; T5-R44, T5-R62): the sibling `lazymkv-ocr` through `run_worker` over a `ToolCtx`
/// holding `guard` and `host`, with the engine's fixed cap `time_cap_s(events)`. Def: T5-10. Use: T5-10, T5-12.
pub fn read_with(guard: &dyn ProcessGuard, host: &dyn Host, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    const AT: usize = 1;
    let parsed = match parse(args, &["--stream", "--clear-max", "--packets", "--out"], &[], &["evidence.mks"]) {
        Ok(parsed) => parsed,
        Err(message) => return usage(err, AT, &message),
    };
    let (stream, clear_max) = match (parsed.whole("--stream"), parsed.whole("--clear-max")) {
        (Ok(stream), Ok(clear_max)) => (stream, clear_max),
        (Err(message), _) | (_, Err(message)) => return usage(err, AT, &message),
    };
    let evidence = PathBuf::from(&parsed.words[0]);
    let packets = PathBuf::from(parsed.get("--packets"));
    let out_dir = PathBuf::from(parsed.get("--out"));
    let Some(out_name) = out_dir.file_name().map(|name| name.to_string_lossy().into_owned()) else {
        return not_started(out, "--out names no folder");
    };
    if let Err(error) = fs::create_dir_all(&out_dir) {
        return not_started(out, &format!("--out: {error}"));
    }
    match fs::read_dir(&out_dir) {
        Ok(mut entries) => {
            if entries.next().is_some() {
                let refusal = "--out already holds files; the trial reads each track once, into an empty folder";
                return not_started(out, refusal);
            }
        }
        Err(error) => return not_started(out, &format!("--out: {error}")),
    }
    let events = match events_of(&packets, clear_max) {
        Ok(events) => events,
        Err(message) => return not_started(out, &message),
    };
    let cap_s = time_cap_s(events);
    let _ = writeln!(out, "events {events} cap {cap_s:.1} s");
    // Beside the folder, never in it.
    let capture = out_dir.parent().unwrap_or(Path::new("")).join(format!("{out_name}.capture.txt"));
    let limits = Limits::from_settings(&Settings::default());
    let ctx = ToolCtx {
        guard,
        host,
        limits: &limits,
        cancel: CancelToken::default(),
        on_output: &|_: OutputStream, _: &str| {},
        on_progress: &|_: f64| {},
    };
    let program = match worker_path() {
        Ok(program) => program,
        Err(error) => return not_started(out, &error.to_string()),
    };
    let started = host.now_ms();
    let args = ocr_args(&evidence, stream, clear_max, &out_dir);
    let end = match run_worker(&ctx, &program, &args, &capture, Duration::from_secs_f64(cap_s)) {
        Ok(end) => end,
        Err(error) => return not_started(out, &error.to_string()),
    };
    let wall_s = host.now_ms().saturating_sub(started) as f64 / 1000.0;
    let _ = writeln!(out, "wall {wall_s:.1} s from the worker's start to its exit");
    let line = match &end {
        WorkerEnd::Read => {
            let run_file = out_dir.join(format!("sub-{stream}.run.json"));
            match read_run(&run_file) {
                Ok(t) => {
                    let _ = writeln!(
                        out,
                        "run.json wall_ms {} events {} distinct {} rows {} detector {}/{}/{}/{} ms decode {:.1} prepare {:.1} \
                         lines {:.1} detect {:.1} recognize {:.1}",
                        t.wall_ms,
                        t.events,
                        t.distinct,
                        t.rows,
                        t.detector[0],
                        t.detector[1],
                        t.detector[2],
                        t.detector[3],
                        t.ms.decode,
                        t.ms.prepare,
                        t.ms.lines,
                        t.ms.detect,
                        t.ms.recognize,
                    );
                    let _ = writeln!(out, "end Read");
                    return 0;
                }
                Err(message) => format!("end Read, but {message}"),
            }
        }
        WorkerEnd::TimedOut => {
            let records = fs::read(out_dir.join(format!("sub-{stream}.ocr.jsonl"))).unwrap_or_default();
            let read = whole_records(&String::from_utf8_lossy(&records)).len();
            format!("end TimedOut: {}", cap_reason(cap_s, read, events))
        }
        WorkerEnd::Refused(_) => format!("end Refused: {}", end_reason(&end).unwrap_or_default()),
        WorkerEnd::ArgumentError(_) => format!("end ArgumentError: {}", end_reason(&end).unwrap_or_default()),
        WorkerEnd::Exited(_) => format!("end Exited: {}", end_reason(&end).unwrap_or_default()),
        WorkerEnd::Cancelled => "end Cancelled".to_string(),
    };
    let _ = writeln!(out, "{line}");
    1
}

// ------------------------------------------------------------------ rows, arm R' (design 10.9)

/// One frame's stage times in a record of `rows`.
#[derive(serde::Serialize)]
struct RowMs {
    prepare: f64,
    lines: f64,
    detect: Option<f64>,
    recognize: f64,
}

/// One frame's record of `rows`; holds subtitle text in `lines`, written to the records file only.
#[derive(serde::Serialize)]
struct RowRecord<'a> {
    frame: usize,
    file: &'a str,
    lines: Vec<String>,
    method: Option<&'static str>,
    trigger: Option<u8>,
    ms: RowMs,
}

/// The last line of `rows`' records file: numbers only.
#[derive(serde::Serialize)]
struct RowSummary {
    frames: usize,
    blank: usize,
    rows: usize,
    detector: [usize; 4],
    load_ms: u64,
    ms_total: f64,
    wall_ms: u64,
}

#[derive(serde::Serialize)]
struct RowSummaryLine {
    summary: RowSummary,
}

/// The milliseconds since `start`.
fn ms_since(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

/// A frame of the folder as a grey image; Err names the file's base name.
fn frame_at(dir: &Path, name: &str) -> Result<Grey, String> {
    let bytes = fs::read(dir.join(name)).map_err(|error| format!("{name}: {error}"))?;
    read_png_grey(&bytes).map_err(|error| format!("{name}: {error}"))
}

/// The records text and the summary of arm R' over the frames folder `dir`.
fn rows_records(dir: &Path) -> Result<(String, RowSummary), String> {
    let wall = Instant::now();
    let mut names: Vec<String> = fs::read_dir(dir)
        .map_err(|error| format!("the frames folder cannot be listed: {error}"))?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_file())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.starts_with("f-") && name.ends_with(".png"))
        .collect();
    names.sort();
    if names.is_empty() {
        return Err("no f-*.png in the frames folder".to_string());
    }
    let load = Instant::now();
    let ocr = Ocr::load()?;
    let load_ms = ms_since(load).round() as u64;
    // Pass 1: each frame's rows only, one frame in memory at a time.
    let mut found: Vec<Rows> = Vec::new();
    for name in &names {
        if let Some(c) = crop(&frame_at(dir, name)?) {
            found.push(rows(&c.image));
        }
    }
    let median = median_line_height(&found);
    drop(found);
    // Pass 2: each frame read by rows or by the detector, as the worker reads an event.
    let mut text = String::new();
    let mut summary =
        RowSummary { frames: names.len(), blank: 0, rows: 0, detector: [0; 4], load_ms, ms_total: 0.0, wall_ms: 0 };
    for (k, name) in names.iter().enumerate() {
        let frame = frame_at(dir, name)?;
        let start = Instant::now();
        let cropped = crop(&frame);
        let prepare = ms_since(start);
        let record = match cropped {
            None => {
                summary.blank += 1;
                RowRecord {
                    frame: k,
                    file: name,
                    lines: Vec::new(),
                    method: None,
                    trigger: None,
                    ms: RowMs { prepare, lines: 0.0, detect: None, recognize: 0.0 },
                }
            }
            Some(c) => {
                let start = Instant::now();
                let found = rows(&c.image);
                let fired = trigger(&found, median, &c, frame.height);
                match fired {
                    Some(number) => {
                        let lines = ms_since(start);
                        let detected = ocr.detect(&c.image).map_err(|error| format!("{name}: {error}"))?;
                        if let Some(count) = summary.detector.get_mut(usize::from(number).saturating_sub(1)) {
                            *count += 1;
                        }
                        RowRecord {
                            frame: k,
                            file: name,
                            lines: detected.lines,
                            method: Some("detector"),
                            trigger: Some(number),
                            ms: RowMs { prepare, lines, detect: Some(detected.detect_ms), recognize: detected.recognize_ms },
                        }
                    }
                    None => {
                        let boxes = line_boxes(&c.image, &found);
                        let lines = ms_since(start);
                        let start = Instant::now();
                        let read = ocr.recognize(&c.image, &boxes).map_err(|error| format!("{name}: {error}"))?;
                        let recognize = ms_since(start);
                        summary.rows += 1;
                        RowRecord {
                            frame: k,
                            file: name,
                            lines: read,
                            method: Some("rows"),
                            trigger: None,
                            ms: RowMs { prepare, lines, detect: None, recognize },
                        }
                    }
                }
            }
        };
        summary.ms_total += record.ms.prepare + record.ms.lines + record.ms.detect.unwrap_or(0.0) + record.ms.recognize;
        text.push_str(&serde_json::to_string(&record).map_err(|error| error.to_string())?);
        text.push('\n');
    }
    summary.wall_ms = ms_since(wall).round() as u64;
    Ok((text, summary))
}

fn run_rows(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let parsed = match parse(args, &["--out"], &[], &["frames dir"]) {
        Ok(parsed) => parsed,
        Err(message) => return usage(err, 2, &message),
    };
    let dir = ctx.abs(&parsed.words[0]);
    let out_file = ctx.abs(parsed.get("--out"));
    let (mut text, summary) = match rows_records(&dir) {
        Ok(read) => read,
        Err(message) => return refuse(err, "rows", &message),
    };
    let line = format!(
        "rows: frames {}, blank {}, rows {}, detector {}/{}/{}/{}, load_ms {}, ms_total {:.1}, wall_ms {}",
        summary.frames,
        summary.blank,
        summary.rows,
        summary.detector[0],
        summary.detector[1],
        summary.detector[2],
        summary.detector[3],
        summary.load_ms,
        summary.ms_total,
        summary.wall_ms,
    );
    match serde_json::to_string(&RowSummaryLine { summary }) {
        Ok(last) => {
            text.push_str(&last);
            text.push('\n');
        }
        Err(error) => return refuse(err, "rows", &error.to_string()),
    }
    if let Err(message) = write_file(&out_file, text.as_bytes()) {
        return refuse(err, "rows", &message);
    }
    let _ = writeln!(out, "{line}");
    0
}

// ------------------------------------------------------------------ f-records (design 10.9 step 4, T5-R24)

/// One record of `f-records`' output, in `sub-<n>.ocr.jsonl` form; holds subtitle text in `lines`.
#[derive(serde::Serialize)]
struct TimedRecord<'a> {
    frame: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    t: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pts: Option<f64>,
    lines: Vec<Value>,
}

/// The records of `input` given the times of `frames_file` by frame number: (the text, records, with text, with
/// pts).
fn timed_records(frames_file: &Path, input: &Path) -> Result<(String, usize, usize, usize), String> {
    let frames_name = base(frames_file);
    let entries: Vec<FrameEntry> = serde_json::from_str(&read_named(frames_file)?)
        .map_err(|error| format!("{frames_name}: {error}"))?;
    let mut by_frame: BTreeMap<i64, &FrameEntry> = BTreeMap::new();
    for entry in &entries {
        by_frame.entry(entry.frame).or_insert(entry);
    }
    let records = read_records(input)?;
    let (mut text, mut count, mut with_text, mut with_pts) = (String::new(), 0, 0, 0);
    for record in &records {
        let Some(k) = record.frame else {
            continue;
        };
        let Some(entry) = by_frame.get(&k) else {
            return Err(format!("frame {k} is not in {frames_name}"));
        };
        if entry.t.is_none() && entry.pts.is_none() {
            return Err(format!("frame {k} has neither t nor pts in {frames_name}"));
        }
        let lines = match &record.lines {
            Some(Value::Array(items)) => items.clone(),
            _ => Vec::new(),
        };
        count += 1;
        with_text += usize::from(!lines.is_empty());
        with_pts += usize::from(entry.pts.is_some());
        let timed = TimedRecord { frame: k, file: record.file.as_deref(), t: entry.t, pts: entry.pts, lines };
        text.push_str(&serde_json::to_string(&timed).map_err(|error| error.to_string())?);
        text.push('\n');
    }
    Ok((text, count, with_text, with_pts))
}

fn run_f_records(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    const AT: usize = 3;
    let parsed = match parse(args, &["--rec", "--stream", "--in", "--out"], &[], &[]) {
        Ok(parsed) => parsed,
        Err(message) => return usage(err, AT, &message),
    };
    let n = match parsed.whole("--stream") {
        Ok(n) => n,
        Err(message) => return usage(err, AT, &message),
    };
    let frames_file = ctx.abs(parsed.get("--rec")).join("subs-ocr").join(format!("sub-{n}.frames.json"));
    let (text, count, with_text, with_pts) = match timed_records(&frames_file, &ctx.abs(parsed.get("--in"))) {
        Ok(read) => read,
        Err(message) => return refuse(err, "f-records", &message),
    };
    if let Err(message) = write_file(&ctx.abs(parsed.get("--out")), text.as_bytes()) {
        return refuse(err, "f-records", &message);
    }
    let _ = writeln!(out, "records {count}, with text {with_text}, with pts {with_pts}");
    0
}

// ------------------------------------------------------------------ summary (design 10.9 step 3)

/// The stream summaries of the run.json files in `dir`, stream number ascending.
fn stream_summaries(dir: &Path, rec: &Path) -> Result<Vec<StreamSummary>, String> {
    let mut numbers: Vec<i64> = fs::read_dir(dir)
        .map_err(|error| format!("--dir cannot be listed: {error}"))?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_file())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter_map(|name| {
            let digits = name.strip_prefix("sub-")?.strip_suffix(".run.json")?;
            if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            digits.parse().ok()
        })
        .collect();
    numbers.sort_unstable();
    if numbers.is_empty() {
        return Err("no sub-<n>.run.json in --dir".to_string());
    }
    let index: Vec<SubsIndexEntry> = read_text(&rec.join("subs").join("index.json"))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    let mut streams = Vec::new();
    for m in numbers {
        let run_file = dir.join(format!("sub-{m}.run.json"));
        let totals = read_run(&run_file)?;
        let records_file = dir.join(format!("sub-{m}.ocr.jsonl"));
        if !records_file.is_file() {
            return Err(format!("sub-{m}.ocr.jsonl is missing beside sub-{m}.run.json"));
        }
        let (with_text, lines, markers) = text_counts(&read_named(&records_file)?);
        let codec = index
            .iter()
            .find(|entry| entry.n == m)
            .and_then(|entry| entry.codec.clone())
            .unwrap_or_else(|| "unknown".to_string());
        let wall_ms = totals.wall_ms;
        streams.push(StreamSummary {
            n: m,
            codec,
            outcome: StreamOutcome::Read(totals),
            exit: Some(0),
            wall_ms,
            with_text,
            lines,
            markers,
        });
    }
    Ok(streams)
}

fn run_summary(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let parsed = match parse(args, &["--dir", "--rec"], &[], &[]) {
        Ok(parsed) => parsed,
        Err(message) => return usage(err, 4, &message),
    };
    let dir = ctx.abs(parsed.get("--dir"));
    let streams = match stream_summaries(&dir, &ctx.abs(parsed.get("--rec"))) {
        Ok(streams) => streams,
        Err(message) => return refuse(err, "summary", &message),
    };
    if let Err(message) = write_file(&dir.join("summary.json"), summary_json(&streams).as_bytes()) {
        return refuse(err, "summary", &message);
    }
    let _ = writeln!(out, "summary: {} streams", streams.len());
    0
}

// ------------------------------------------------------------------ parity (design 10.9 "Parity passes", P34)

/// What `parity` printed and found (design 10.9 "Parity passes", P34): report lines and difference lines, numbers
/// only. Def: T5-10. Use: T5-10.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parity {
    pub lines: Vec<String>,
    pub differences: Vec<String>,
}

/// One arm's run of the subtitle text rule over the recording `rec` and the records folder `text`, as `subs` reads
/// them.
fn arm(ctx: &Ctx, rec: &str, text: &str) -> SubsRun {
    let (rec_tree, rec_key) = subs::rec_folder_tree(ctx, rec);
    let mut tree = MemTree::new(&pathlib_split(text).0, "");
    subs::add_folder_files(&mut tree, &PathKey::root(), &ctx.abs(text));
    let rule = Rule::default();
    subs_run(rec_tree.as_ref(), &rec_key, &tree, &PathKey::root(), None, rule.CLEAR_MAX_BYTES, &rule, &[])
}

fn yes(flag: bool) -> &'static str {
    if flag {
        "yes"
    } else {
        "no"
    }
}

fn fixed(x: f64) -> String {
    py_fixed(x, 3)
}

fn fixed_or_none(x: Option<f64>) -> String {
    x.map_or_else(|| "none".to_string(), fixed)
}

/// The counts of design 10.9's parity, in its order.
fn counts(m: &Measures) -> [(&'static str, usize); 9] {
    [
        ("CUE", m.CUE),
        ("LABEL", m.LABEL),
        ("CHEV", m.CHEV),
        ("M", m.M),
        ("SO", m.SO),
        ("CS", m.CS),
        ("ODD", m.ODD),
        ("NOTE", m.NOTE),
        ("CAPS_CUE", m.CAPS_CUE),
    ]
}

/// A stream's measures on one report line.
fn measures_text(m: Option<&Measures>) -> String {
    let Some(m) = m else {
        return "not read".to_string();
    };
    let mut text: Vec<String> = counts(m).iter().map(|(key, value)| format!("{key} {value}")).collect();
    text.push(format!("caps_style {}", yes(m.caps_style)));
    text.push(format!("class {}", m.class.as_str()));
    text.push(format!("readable {}", yes(m.readable)));
    text.push(format!("letter_events {}", fixed(m.letter_events)));
    text.push(format!("wordlike {}", fixed(m.wordlike)));
    text.push(format!("lang {}", m.lang));
    text.push("shares".to_string());
    for (name, share) in m.shares.iter() {
        text.push(format!("{name} {}", fixed(*share)));
    }
    text.join(" ")
}

/// How a value is compared with a threshold.
#[derive(Clone, Copy)]
enum Side {
    AtLeast,
    AtMost,
    Below,
}

impl Side {
    fn holds(self, x: f64, t: f64) -> bool {
        match self {
            Side::AtLeast => x >= t,
            Side::AtMost => x <= t,
            Side::Below => x < t,
        }
    }
}

/// The first threshold that separates `a` and `b`.
fn separating(a: f64, b: f64, thresholds: &[(f64, Side)]) -> Option<f64> {
    thresholds.iter().find(|(t, side)| side.holds(a, *t) != side.holds(b, *t)).map(|(t, _)| *t)
}

/// The differences of one stream read in both arms, in design 10.9's order; a count differs when the arms are more than
/// one event apart (the owner's amendment of 2026-11-08), the rest when unequal.
fn stream_differences(n: i64, a: &Measures, b: &Measures, rule: &Rule, out: &mut Vec<String>) {
    for ((key, x), (_, y)) in counts(a).iter().zip(counts(b).iter()) {
        if x.abs_diff(*y) > 1 {
            out.push(format!("s{n}: {key} F {x} R {y}"));
        }
    }
    if a.caps_style != b.caps_style {
        out.push(format!("s{n}: caps_style F {} R {}", yes(a.caps_style), yes(b.caps_style)));
    }
    if a.class != b.class {
        out.push(format!("s{n}: class F {} R {}", a.class.as_str(), b.class.as_str()));
    }
    if a.readable != b.readable {
        out.push(format!("s{n}: readable F {} R {}", yes(a.readable), yes(b.readable)));
    }
    if a.lang != b.lang {
        out.push(format!("s{n}: lang F {} R {}", a.lang, b.lang));
    }
    let mut values = vec![
        ("letter_events".to_string(), a.letter_events, b.letter_events, vec![(rule.READ_LETTER_EVENTS, Side::AtLeast)]),
        ("wordlike".to_string(), a.wordlike, b.wordlike, vec![(rule.READ_WORDLIKE, Side::AtLeast)]),
    ];
    for (name, x) in a.shares.iter() {
        let Some(y) = b.shares.get(name) else {
            continue;
        };
        let thresholds = if name == "en" {
            vec![(rule.EN_MIN, Side::AtLeast), (rule.EN_MAX_OTHER, Side::AtMost)]
        } else {
            vec![(rule.OTHER_MIN, Side::AtLeast)]
        };
        values.push((format!("share {name}"), *x, *y, thresholds));
    }
    for (key, x, y, thresholds) in values {
        if let Some(t) = separating(x, y, &thresholds) {
            out.push(format!("s{n}: {key} F {} R {} on two sides of {}", fixed(x), fixed(y), fixed(t)));
        }
    }
}

/// The report line comparing the two arms' event texts of one stream (reported only).
fn text_line(n: i64, f: Option<&Track>, r: Option<&Track>) -> String {
    let tag = f.or(r).map_or("und", |t| t.tag.as_str());
    let english = tag == "eng" || tag == "en";
    let (mut events, mut exact, mut rates) = (0, 0, Vec::new());
    if let (Some(f), Some(r)) = (f, r) {
        for (a, b) in f.events.iter().zip(&r.events) {
            if a.lines.is_empty() && b.lines.is_empty() {
                continue;
            }
            let (x, y) = (norm_text(&a.lines, english), norm_text(&b.lines, english));
            events += 1;
            exact += usize::from(x == y);
            rates.push(cer(&x, &y));
        }
    }
    format!(
        "s{n}: F against R, events {events}, exact {exact}, cer mean {} (reported only)",
        fixed_or_none(exact_mean(&rates))
    )
}

/// Stream `n`'s track in `run`.
fn track_of(run: &SubsRun, n: i64) -> Option<&Track> {
    run.tracks.iter().find(|t| t.n == n)
}

/// A track's measures, when it was read.
fn measured_of(track: Option<&Track>) -> Option<&Measures> {
    track.and_then(|t| t.measured.as_ref()).map(|m| &m.m)
}

/// A decision as `<role> <review | -> <note | ->`, or `none` when the arm has none.
fn decision_text(d: Option<&Decision>) -> String {
    let Some(d) = d else {
        return "none".to_string();
    };
    let or_dash = |text: &Option<String>| text.as_deref().filter(|text| !text.is_empty()).unwrap_or("-").to_string();
    format!("{} {} {}", d.role.as_str(), or_dash(&d.review), or_dash(&d.note))
}

/// Design 10.9's parity of arm R against arm F over one title, with counts within one event; Err when either run has
/// no analysis. Def: T5-10.
/// Use: T5-10, T5-12.
pub fn parity(f: &SubsRun, r: &SubsRun, rule: &Rule) -> Result<Parity, String> {
    let Some(fa) = &f.analysis else {
        return Err("arm F's run has no analysis".to_string());
    };
    let Some(ra) = &r.analysis else {
        return Err("arm R's run has no analysis".to_string());
    };
    let mut streams: Vec<i64> = f.tracks.iter().map(|t| t.n).collect();
    streams.extend(r.tracks.iter().map(|t| t.n).filter(|n| !f.tracks.iter().any(|t| t.n == *n)));

    let (mut lines, mut differences) = (Vec::new(), Vec::new());
    for &n in &streams {
        let (ft, rt) = (track_of(f, n), track_of(r, n));
        let tag = ft.or(rt).map_or("und", |t| t.tag.as_str());
        lines.push(format!("s{n} {tag} F {}", measures_text(measured_of(ft))));
        lines.push(format!("s{n} {tag} R {}", measures_text(measured_of(rt))));
    }
    for &n in &streams {
        let (ft, rt) = (track_of(f, n), track_of(r, n));
        match (measured_of(ft), measured_of(rt)) {
            (Some(a), Some(b)) => stream_differences(n, a, b, rule, &mut differences),
            (None, None) => {}
            (a, b) => differences.push(format!("s{n}: read F {} R {}", yes(a.is_some()), yes(b.is_some()))),
        }
    }
    for &n in &streams {
        let (ft, rt) = (track_of(f, n), track_of(r, n));
        if measured_of(ft).is_some() || measured_of(rt).is_some() {
            lines.push(text_line(n, ft, rt));
        }
    }

    let cb = |pair: Option<&PairRow>| match pair {
        Some(row) => format!("CB {}/{}", fixed_or_none(row.pair.cb.0), fixed_or_none(row.pair.cb.1)),
        None => "-".to_string(),
    };
    let bands = [
        (rule.CB_DIFF, Side::AtMost),
        (rule.CB_SAME, Side::AtLeast),
        (rule.CB_SUBSET_BACK, Side::Below),
        (rule.CB_SUBSET_IN, Side::AtLeast),
    ];
    let mut keys: Vec<(i64, i64)> = fa.pairs.iter().map(|p| (p.a, p.b)).collect();
    keys.extend(ra.pairs.iter().map(|p| (p.a, p.b)).filter(|key| !fa.pairs.iter().any(|p| (p.a, p.b) == *key)));
    for (a, b) in keys {
        let fp = fa.pairs.iter().find(|p| (p.a, p.b) == (a, b));
        let rp = ra.pairs.iter().find(|p| (p.a, p.b) == (a, b));
        lines.push(format!("pair s{a} s{b} F {} R {}", cb(fp), cb(rp)));
        let (Some(fp), Some(rp)) = (fp, rp) else {
            differences.push(format!("pair s{a} s{b}: in {} only", if fp.is_some() { "F" } else { "R" }));
            continue;
        };
        let directions = [
            (format!("s{a} in s{b}"), fp.pair.cb.0, rp.pair.cb.0),
            (format!("s{b} in s{a}"), fp.pair.cb.1, rp.pair.cb.1),
        ];
        for (dir, x, y) in directions {
            match (x, y) {
                (Some(x), Some(y)) => {
                    if let Some(t) = separating(x, y, &bands) {
                        differences.push(format!(
                            "pair s{a} s{b}: CB {dir} F {} R {} on two sides of {}",
                            fixed(x),
                            fixed(y),
                            fixed(t)
                        ));
                    }
                }
                (None, None) => {}
                (x, y) => {
                    differences.push(format!("pair s{a} s{b}: CB {dir} F {} R {}", fixed_or_none(x), fixed_or_none(y)));
                }
            }
        }
    }

    let mut decided: Vec<i64> = fa.decide.keys().chain(ra.decide.keys()).copied().collect();
    decided.sort_unstable();
    decided.dedup();
    for n in decided {
        let text = format!("F {} R {}", decision_text(fa.decide.get(&n)), decision_text(ra.decide.get(&n)));
        lines.push(format!("s{n} decision {text}"));
        if decision_text(fa.decide.get(&n)) != decision_text(ra.decide.get(&n)) {
            differences.push(format!("s{n}: decision {text}"));
        }
    }
    Ok(Parity { lines, differences })
}

fn run_parity(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let parsed = match parse(args, &["--rec", "--f", "--r"], &[], &[]) {
        Ok(parsed) => parsed,
        Err(message) => return usage(err, 5, &message),
    };
    let rec = parsed.get("--rec");
    let f = arm(ctx, rec, parsed.get("--f"));
    let r = arm(ctx, rec, parsed.get("--r"));
    if f.exit_code == 2 || r.exit_code == 2 {
        for (name, run) in [("F", &f), ("R", &r)] {
            if run.exit_code == 2 {
                for line in &run.stderr {
                    let _ = writeln!(err, "{name}: {line}");
                }
            }
        }
        return 2;
    }
    let found = match parity(&f, &r, &Rule::default()) {
        Ok(found) => found,
        Err(message) => return refuse(err, "parity", &message),
    };
    for line in found.lines.iter().chain(&found.differences) {
        let _ = writeln!(out, "{line}");
    }
    if found.differences.is_empty() {
        let _ = writeln!(out, "PARITY PASS");
        0
    } else {
        let _ = writeln!(out, "PARITY FAIL: {} differences", found.differences.len());
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{write, Scratch, ScratchExt};
    use lazymkv_core::recording::files::read_jsonl;
    use lazymkv_core::recording::py_fixed;
    use lazymkv_core::subtitle::bitmap::evidence::{run_json, EngineId, RunTotals, StageTotals};
    use lazymkv_core::subtitle::bitmap::prepare::Grey;
    use lazymkv_core::subtitle::text::{ocr_records, OcrRecord};
    use lazymkv_platform::{GuardedChild, Spawn};
    use lazymkv_tools::ocr::ocr_args;
    use serde_json::{json, Value};
    use std::collections::BTreeMap;
    use std::fs;
    use std::io;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    /// Made-up words: no real subtitle text (RP21).
    const WORDS: [&str; 11] = ["babin", "dagon", "gikol", "kubal", "lodin", "bekan", "dulog", "gamin", "kodel", "labun", "bodig"];

    /// Event k's lines: a made-up sentence, and a sound cue as event 5 (as `subs`' tests).
    fn lines_of(k: usize) -> Vec<String> {
        if k == 5 {
            return vec!["(DOOR SLAMS)".to_string()];
        }
        vec![format!("We {} the {} and the {}.", WORDS[k % 11], WORDS[(3 * k + 1) % 11], WORDS[(7 * k + 2) % 11])]
    }

    fn path(p: &Path) -> String {
        p.display().to_string()
    }

    fn strings(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| word.to_string()).collect()
    }

    /// (exit code, stdout, stderr) of `lazymkv-dev ocr-trial <args>` through the command table.
    fn trial(s: &Scratch, args: &[String]) -> (i32, String, String) {
        let ctx = Ctx::for_test(&s.0);
        let mut argv = vec!["ocr-trial".to_string()];
        argv.extend(args.iter().cloned());
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = crate::dispatch(&ctx, &argv, &mut out, &mut err);
        (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
    }

    /// sub-<n>.packets.json of `events` displayed events, 4 s apart from 10 s, each followed by a 30-byte clear.
    fn packets(events: usize) -> String {
        let mut list = Vec::new();
        for k in 0..events {
            let start = 10.0 + 4.0 * k as f64;
            list.push(json!({"pts_time": py_fixed(start, 3), "duration_time": "2.500", "size": "4000"}));
            list.push(json!({"pts_time": py_fixed(start + 2.5, 3), "duration_time": "0.021", "size": "30"}));
        }
        json!({ "packets": list }).to_string()
    }

    /// The recording rec/disc-a/all-tracks: subs/index.json with one "HDMV PGS" stream per (n, language), each with
    /// 30 events in its packet list.
    fn recording(s: &Scratch, streams: &[(i64, &str)]) -> PathBuf {
        let rec = s.0.join("rec/disc-a/all-tracks");
        let index: Vec<Value> = streams
            .iter()
            .map(|(n, language)| json!({"n": n, "track_id": 3 + n, "language": language, "codec": "HDMV PGS",
                "track_name": null, "forced": false, "default": false}))
            .collect();
        write(&rec.join("subs/index.json"), Value::Array(index).to_string());
        for (n, _) in streams {
            write(&rec.join(format!("subs/sub-{n}.packets.json")), packets(30));
        }
        rec
    }

    /// `dir`/sub-<n>.ocr.jsonl for `recording`'s 30 events, event k read as `lines_of(k)` or as `edit`'s lines for
    /// its event, each record at its packet's pts.
    fn records(dir: &Path, n: i64, edit: Option<(usize, &[&str])>) {
        records_edited(dir, n, edit.as_slice());
    }

    /// As `records`, with any number of events read as the given lines in place of `lines_of(k)`.
    fn records_edited(dir: &Path, n: i64, edits: &[(usize, &[&str])]) {
        let mut text = String::new();
        for k in 0..30usize {
            let pts = 10.0 + 4.0 * k as f64;
            let lines: Vec<String> = match edits.iter().find(|(at, _)| *at == k) {
                Some((_, lines)) => strings(lines),
                None => lines_of(k),
            };
            let file = format!("f-{:09}.png", (pts * 1000.0) as u64);
            text.push_str(&json!({"frame": k, "file": file, "t": pts, "pts": pts, "lines": lines}).to_string());
            text.push('\n');
        }
        write(&dir.join(format!("sub-{n}.ocr.jsonl")), text);
    }

    #[derive(Clone, Copy)]
    struct Inputs {
        frames: usize,
        tesseract: bool,
        f: bool,
        dataset_evidence: bool,
        packets: bool,
    }

    const FULL: Inputs = Inputs { frames: 3, tesseract: true, f: false, dataset_evidence: true, packets: true };

    /// The trial's made-up inputs for stream 0: `recording`'s rec/disc-a/all-tracks, and in the dataset folder
    /// data/disc-a/all-tracks/subs-ocr/ `frames` files s0/f-<k:08>.png, the Tesseract reading sub-0.ocr.jsonl (3
    /// records), arm F's ocrs-s0.jsonl (3 records and a summary line) and a 100-byte evidence.mks, as `i` asks.
    /// Returns (rec, text).
    fn inputs(s: &Scratch, i: Inputs) -> (PathBuf, PathBuf) {
        let rec = recording(s, &[(0, "eng")]);
        if !i.packets {
            fs::remove_file(rec.join("subs/sub-0.packets.json")).unwrap();
        }
        let text = s.0.join("data/disc-a/all-tracks/subs-ocr");
        fs::create_dir_all(&text).unwrap();
        for k in 0..i.frames {
            write(&text.join(format!("s0/f-{k:08}.png")), "png");
        }
        let record = |k: usize| format!("{{\"frame\": {k}, \"file\": \"f-{k:08}.png\", \"lines\": []}}\n");
        if i.tesseract {
            write(&text.join("sub-0.ocr.jsonl"), (0..3).map(record).collect::<String>());
        }
        if i.f {
            write(&text.join("ocrs-s0.jsonl"), (0..3).map(record).collect::<String>() + "{\"summary\": {\"frames\": 3}}\n");
        }
        if i.dataset_evidence {
            write(&text.join("evidence.mks"), [0u8; 100]);
        }
        (rec, text)
    }

    fn inventory(s: &Scratch, rec: &Path, text: &Path, more: &[&str]) -> (i32, String, String) {
        let staging = path(&s.0.join("staging"));
        let mut args = strings(&["inventory", "--rec", &path(rec), "--text", &path(text), "--stream", "0", "--staging", &staging]);
        args.extend(strings(more));
        trial(s, &args)
    }

    #[test]
    fn inventory_stops_on_a_missing_tesseract_reading_evidence_or_packet_list() {
        for (case, want) in [
            (Inputs { tesseract: false, ..FULL }, "missing: sub-0.ocr.jsonl (the Tesseract reading)"),
            (
                Inputs { dataset_evidence: false, ..FULL },
                "missing: evidence.mks (no dataset copy, no path in summary.json, none in the phase 0 staging layout)",
            ),
            (Inputs { packets: false, ..FULL }, "missing: subs/sub-0.packets.json (the packet list)"),
        ] {
            let s = Scratch::new("trial-inventory-missing");
            let (rec, text) = inputs(&s, case);
            let (code, out, err) = inventory(&s, &rec, &text, &[]);
            assert_eq!(code, 1, "{out}{err}");
            assert!(out.lines().any(|line| line == want), "{out}");
            assert!(!out.contains(&path(&s.0)), "the inventory prints no folder path: {out}");
        }
    }

    #[test]
    fn inventory_reports_r_prime_not_run_without_frames() {
        let s = Scratch::new("trial-inventory-no-frames");
        let (rec, text) = inputs(&s, Inputs { frames: 0, f: true, ..FULL });
        let (code, out, err) = inventory(&s, &rec, &text, &[]);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(
            out,
            "frames 0, Tesseract records 3, F reading 3 records, evidence.mks 100 bytes from the dataset copy, \
             events 30; R' not run; F exists\n"
        );
    }

    #[test]
    fn inventory_stops_when_neither_frames_nor_an_f_reading_exist() {
        let s = Scratch::new("trial-inventory-no-arm-f");
        let (rec, text) = inputs(&s, Inputs { frames: 0, f: false, ..FULL });
        let (code, out, _) = inventory(&s, &rec, &text, &[]);
        assert_eq!(code, 1);
        assert!(out.lines().any(|line| line == "missing: s0/f-*.png and ocrs-s0.jsonl (parity cannot be judged on this track)"), "{out}");
    }

    #[test]
    fn inventory_takes_evidence_from_the_staging_folder_when_summary_json_names_none() {
        let s = Scratch::new("trial-inventory-staging");
        let (rec, text) = inputs(&s, Inputs { dataset_evidence: false, ..FULL });
        write(&rec.join("subs-ocr/summary.json"), "\u{feff}{\"slug\": \"disc-a\", \"streams\": []}\r\n");
        let staged = s.0.join("staging/disc-a/all-tracks/evidence.mks");
        write(&staged, [0u8; 250]);
        let where_file = path(&s.0.join("where.txt"));
        let (code, out, _) = inventory(&s, &rec, &text, &["--where", &where_file]);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains(", evidence.mks 250 bytes from the phase 0 staging layout, "), "{out}");
        assert_eq!(fs::read_to_string(&where_file).unwrap(), format!("{}\n", path(&staged)));
    }

    #[test]
    fn inventory_prefers_the_dataset_copy_then_the_path_summary_json_names() {
        let s = Scratch::new("trial-inventory-order");
        let (rec, text) = inputs(&s, FULL);
        let named = s.0.join("elsewhere/evidence.mks");
        write(&named, [0u8; 180]);
        write(&s.0.join("staging/disc-a/all-tracks/evidence.mks"), [0u8; 250]);
        write(&rec.join("subs-ocr/summary.json"), json!({"evidence": path(&named), "streams": []}).to_string());
        let (_, out, _) = inventory(&s, &rec, &text, &[]);
        assert!(out.contains(", evidence.mks 100 bytes from the dataset copy, "), "{out}");
        fs::remove_file(text.join("evidence.mks")).unwrap();
        let (_, out, _) = inventory(&s, &rec, &text, &[]);
        assert!(out.contains(", evidence.mks 180 bytes from the staging path summary.json names, "), "{out}");
    }

    /// Keeps each Spawn and starts nothing, so a test sees what arm R would start.
    struct Recorder(Mutex<Vec<Spawn>>);

    impl ProcessGuard for Recorder {
        fn spawn(&self, spawn: &Spawn) -> io::Result<Box<dyn GuardedChild>> {
            self.0.lock().unwrap().push(spawn.clone());
            Err(io::Error::other("recorded, not started"))
        }
    }

    fn read_args(s: &Scratch) -> Vec<String> {
        strings(&[
            &path(&s.0.join("evidence.mks")), "--stream", "0", "--clear-max", "64",
            "--packets", &path(&s.0.join("sub-0.packets.json")), "--out", &path(&s.0.join("r")),
        ])
    }

    #[test]
    fn read_starts_the_sibling_worker_below_normal_with_rten_num_threads_3_and_the_fixed_cap() {
        let s = Scratch::new("trial-read-spawn");
        s.write("evidence.mks", [0u8; 16]);
        s.write("sub-0.packets.json", packets(887));
        let guard = Recorder(Mutex::new(Vec::new()));
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = read_with(&guard, &ClockHost, &read_args(&s), &mut out, &mut err);
        let out = String::from_utf8(out).unwrap();
        assert_eq!(code, 1, "{out}");
        assert!(out.starts_with("events 887 cap 1800.6 s\n"), "{out}");
        let last = out.lines().last().unwrap();
        assert!(last.starts_with("end not started: ") && last.contains("recorded, not started"), "{out}");
        let spawned = guard.0.lock().unwrap();
        assert_eq!(spawned.len(), 1);
        let worker = std::env::current_exe().unwrap().with_file_name(format!("lazymkv-ocr{}", std::env::consts::EXE_SUFFIX));
        assert_eq!(spawned[0].program, worker);
        assert_eq!(spawned[0].args, ocr_args(&s.0.join("evidence.mks"), 0, 64, &s.0.join("r")));
        assert_eq!(spawned[0].env, vec![("RTEN_NUM_THREADS".to_string(), "3".to_string())]);
        assert!(spawned[0].below_normal);
        assert!(s.0.join("r").is_dir(), "read makes --out before the start");
    }

    #[test]
    fn read_refuses_an_out_folder_that_already_holds_a_file() {
        let s = Scratch::new("trial-read-not-empty");
        s.write("evidence.mks", [0u8; 16]);
        s.write("sub-0.packets.json", packets(3));
        s.write("r/sub-0.ocr.jsonl", "");
        let guard = Recorder(Mutex::new(Vec::new()));
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(read_with(&guard, &ClockHost, &read_args(&s), &mut out, &mut err), 1);
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "end not started: --out already holds files; the trial reads each track once, into an empty folder\n"
        );
        assert!(guard.0.lock().unwrap().is_empty());
    }

    #[test]
    fn rows_reads_made_up_block_frames_into_records() {
        let s = Scratch::new("trial-rows");
        let white = Grey { width: 1280, height: 720, pixels: vec![255; 1280 * 720] };
        let mut block = white.clone();
        for y in 600..640usize {
            for x in 440..840usize {
                block.pixels[y * 1280 + x] = 0;
            }
        }
        for (k, frame) in [&white, &block, &block].into_iter().enumerate() {
            write(&s.0.join(format!("frames/f-{k:08}.png")), lazymkv_ocr::grey_png(frame).unwrap());
        }
        let out_file = s.0.join("rows-s0.jsonl");
        let (code, out, err) = trial(&s, &strings(&["rows", &path(&s.0.join("frames")), "--out", &path(&out_file)]));
        assert_eq!(code, 0, "{err}");
        assert!(out.starts_with("rows: frames 3, blank 1, "), "{out}");
        let lines: Vec<Value> =
            fs::read_to_string(&out_file).unwrap().lines().map(|line| serde_json::from_str(line).unwrap()).collect();
        assert_eq!(lines.len(), 4);
        assert_eq!((&lines[0]["frame"], &lines[0]["file"]), (&json!(0), &json!("f-00000000.png")));
        assert_eq!((&lines[0]["lines"], &lines[0]["method"]), (&json!([]), &Value::Null));
        for k in 1..3 {
            assert_eq!(lines[k]["frame"], json!(k));
            assert!(lines[k]["lines"].is_array());
            assert!(matches!(lines[k]["method"].as_str(), Some("rows") | Some("detector")), "{}", lines[k]["method"]);
            assert!(lines[k]["ms"]["recognize"].is_number());
        }
        let summary = &lines[3]["summary"];
        assert_eq!((summary["frames"].as_u64(), summary["blank"].as_u64()), (Some(3), Some(1)));
    }

    /// sub-0.frames.json as ocr-subs.ps1 writes it in Windows PowerShell 5.1: a byte order mark and CRLF line ends.
    fn frames_json(rec: &Path, entries: &[Value]) {
        let body: Vec<String> = entries.iter().map(Value::to_string).collect();
        write(&rec.join("subs-ocr/sub-0.frames.json"), format!("\u{feff}[\r\n{}\r\n]\r\n", body.join(",\r\n")));
    }

    /// Arm F's per-frame records as ocrs-trial writes them: no time, a last summary line.
    const F_INPUT: &str = "{\"frame\": 0, \"file\": \"f-00000000.png\", \"lines\": [], \"ms\": 10}\n\
        {\"frame\": 1, \"file\": \"f-00000001.png\", \"lines\": [\"We babin the dagon.\"], \"ms\": 12}\n\
        {\"summary\": {\"frames\": 2}}\n";

    fn f_records(s: &Scratch, rec: &Path) -> (i32, String, String) {
        s.write("ocrs-s0.jsonl", F_INPUT);
        let (input, output) = (path(&s.0.join("ocrs-s0.jsonl")), path(&s.0.join("f/sub-0.ocr.jsonl")));
        trial(s, &strings(&["f-records", "--rec", &path(rec), "--stream", "0", "--in", &input, "--out", &output]))
    }

    #[test]
    fn f_records_take_pts_and_t_from_frames_json_by_frame_number() {
        let s = Scratch::new("trial-f-records");
        let rec = s.0.join("rec/disc-a/all-tracks");
        frames_json(&rec, &[
            json!({"frame": 1, "file": "f-00000001.png", "t": 0.5, "pts": 61.603, "chars": 16, "lines": 1}),
            json!({"frame": 0, "file": "f-00000000.png", "t": 0.25, "pts": 61.353, "chars": 0, "lines": 0}),
        ]);
        let (code, out, err) = f_records(&s, &rec);
        assert_eq!((code, out.as_str()), (0, "records 2, with text 1, with pts 2\n"), "{err}");
        assert_eq!(
            fs::read_to_string(s.0.join("f/sub-0.ocr.jsonl")).unwrap(),
            [
                r#"{"frame":0,"file":"f-00000000.png","t":0.25,"pts":61.353,"lines":[]}"#,
                r#"{"frame":1,"file":"f-00000001.png","t":0.5,"pts":61.603,"lines":["We babin the dagon."]}"#,
                "",
            ]
            .join("\n")
        );
    }

    #[test]
    fn f_records_from_a_frames_json_without_pts_keep_t_for_the_proof_of_concept_pairing() {
        let s = Scratch::new("trial-f-records-poc");
        let rec = s.0.join("rec/disc-a/all-tracks");
        frames_json(&rec, &[json!({"frame": 0, "t": 0.25, "lines": 0}), json!({"frame": 1, "t": 0.5, "lines": 1})]);
        let (code, out, _) = f_records(&s, &rec);
        assert_eq!((code, out.as_str()), (0, "records 2, with text 1, with pts 0\n"));
        let text = fs::read_to_string(s.0.join("f/sub-0.ocr.jsonl")).unwrap();
        assert!(!text.contains("\"pts\""), "{text}");
        assert_eq!(
            ocr_records(&read_jsonl(&text).unwrap(), &BTreeMap::new()).unwrap(),
            vec![OcrRecord { t: 0.5, pts: false, lines: vec!["We babin the dagon.".to_string()] }]
        );
    }

    #[test]
    fn f_records_stop_on_a_frame_with_neither_t_nor_pts_or_one_frames_json_lacks() {
        let s = Scratch::new("trial-f-records-stop");
        let rec = s.0.join("rec/disc-a/all-tracks");
        frames_json(&rec, &[json!({"frame": 0, "t": 0.25, "lines": 0}), json!({"frame": 1, "lines": 1})]);
        let (code, _, err) = f_records(&s, &rec);
        assert_eq!((code, err.as_str()), (1, "lazymkv-dev ocr-trial f-records: frame 1 has neither t nor pts in sub-0.frames.json\n"));
        assert!(!s.0.join("f/sub-0.ocr.jsonl").exists());
        frames_json(&rec, &[json!({"frame": 0, "t": 0.25, "lines": 0})]);
        let (code, _, err) = f_records(&s, &rec);
        assert_eq!((code, err.as_str()), (1, "lazymkv-dev ocr-trial f-records: frame 1 is not in sub-0.frames.json\n"));
    }

    #[test]
    fn summary_lists_each_read_stream_with_counts_and_no_text() {
        let s = Scratch::new("trial-summary");
        let rec = recording(&s, &[(0, "eng")]);
        let totals = RunTotals {
            events: 2,
            distinct: 2,
            rows: 2,
            detector: [0; 4],
            ms: StageTotals { decode: 1.0, prepare: 1.0, lines: 1.0, detect: 0.0, recognize: 5.0 },
            engine: EngineId { ocrs: "0.13.1".to_string(), detection_sha256: "d".repeat(64), recognition_sha256: "e".repeat(64) },
            wall_ms: 900,
        };
        s.write("engine/sub-0.run.json", run_json(&totals));
        s.write(
            "engine/sub-0.ocr.jsonl",
            "{\"frame\":0,\"file\":\"f-000010000.png\",\"t\":10.0,\"pts\":10.0,\"text\":\"text-sentinel-3e8f\",\"lines\":[\"text-sentinel-3e8f\"],\"size\":4000}\n\
             {\"frame\":1,\"file\":\"f-000014000.png\",\"t\":14.0,\"pts\":14.0,\"text\":\"\",\"lines\":[],\"size\":4000}\n",
        );
        let (code, out, err) = trial(&s, &strings(&["summary", "--dir", &path(&s.0.join("engine")), "--rec", &path(&rec)]));
        assert_eq!((code, out.as_str()), (0, "summary: 1 streams\n"), "{err}");
        let text = fs::read_to_string(s.0.join("engine/summary.json")).unwrap();
        assert!(!text.contains("text-sentinel-3e8f"), "{text}");
        assert!(text.ends_with("}\n"));
        let summary: Value = serde_json::from_str(&text).unwrap();
        let stream = &summary["streams"][0];
        assert_eq!((&stream["n"], &stream["codec"], &stream["read"]), (&json!(0), &json!("HDMV PGS"), &json!(true)));
        assert_eq!((&stream["with_text"], &stream["exit"], &stream["wall_ms"]), (&json!(1), &json!(0), &json!(900)));
    }

    fn measures(run: &SubsRun, at: usize) -> &lazymkv_core::subtitle::text::Measures {
        &run.tracks[at].measured.as_ref().expect("the stream is measured").m
    }

    #[test]
    fn parity_passes_within_one_event_per_count_and_fails_when_a_count_differs_by_two() {
        let s = Scratch::new("trial-parity-count");
        let rec = recording(&s, &[(0, "eng")]);
        let (f_dir, r_dir) = (s.0.join("f"), s.0.join("r"));
        // Arm F reads eight sound cues (event 5 and seven more), an SDH track whose decision note holds no count, so
        // a count that moves by one event leaves the decision alone.
        let cue: &[&str] = &["(PHONE RINGS)"];
        let dialogue: &[&str] = &["We babin the dagon."];
        let mut f_edits: Vec<(usize, &[&str])> = [9, 11, 13, 17, 19, 23, 29].iter().map(|&k| (k, cue)).collect();
        records_edited(&f_dir, 0, &f_edits);
        records_edited(&r_dir, 0, &f_edits);
        let args = strings(&["parity", "--rec", &path(&rec), "--f", &path(&f_dir), "--r", &path(&r_dir)]);
        let (code, out, err) = trial(&s, &args);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.ends_with("PARITY PASS\n"), "{out}");
        let ctx = Ctx::for_test(&s.0);
        let f = arm(&ctx, &path(&rec), &path(&f_dir));
        // Arm R reads event 5's sound cue as dialogue: SO is one event apart, which passes.
        f_edits.push((5, dialogue));
        records_edited(&r_dir, 0, &f_edits);
        let r = arm(&ctx, &path(&rec), &path(&r_dir));
        let (fm, rm) = (measures(&f, 0), measures(&r, 0));
        assert_eq!(fm.SO.abs_diff(rm.SO), 1);
        let found = parity(&f, &r, &Rule::default()).unwrap();
        assert!(found.differences.is_empty(), "{:?}", found.differences);
        let (code, out, err) = trial(&s, &args);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.ends_with("PARITY PASS\n"), "{out}");
        // Arm R reads a second sound cue as dialogue too: SO is two events apart, which fails.
        f_edits.retain(|(k, _)| *k != 9);
        f_edits.push((9, dialogue));
        records_edited(&r_dir, 0, &f_edits);
        let r = arm(&ctx, &path(&rec), &path(&r_dir));
        let (fm, rm) = (measures(&f, 0), measures(&r, 0));
        assert_eq!(fm.SO.abs_diff(rm.SO), 2);
        let found = parity(&f, &r, &Rule::default()).unwrap();
        assert!(found.differences.contains(&format!("s0: SO F {} R {}", fm.SO, rm.SO)), "{:?}", found.differences);
        let (code, out, _) = trial(&s, &args);
        assert_eq!(code, 1);
        assert!(out.ends_with(&format!("PARITY FAIL: {} differences\n", found.differences.len())), "{out}");
        assert!(!out.contains("babin") && !out.contains(&path(&s.0)), "parity prints numbers only: {out}");
    }

    #[test]
    fn parity_fails_when_a_stopword_share_crosses_its_threshold() {
        let s = Scratch::new("trial-parity-share");
        let rec = recording(&s, &[(0, "eng")]);
        records(&s.0.join("f"), 0, None);
        records(&s.0.join("r"), 0, None);
        let ctx = Ctx::for_test(&s.0);
        let mut f = arm(&ctx, &path(&rec), &path(&s.0.join("f")));
        let mut r = arm(&ctx, &path(&rec), &path(&s.0.join("r")));
        let share = |run: &mut SubsRun, x: f64| run.tracks[0].measured.as_mut().unwrap().m.shares.insert("en".to_string(), x);
        share(&mut f, 0.21);
        share(&mut r, 0.19);
        assert_eq!(
            parity(&f, &r, &Rule::default()).unwrap().differences,
            vec!["s0: share en F 0.210 R 0.190 on two sides of 0.200".to_string()]
        );
        share(&mut r, 0.25);
        assert!(parity(&f, &r, &Rule::default()).unwrap().differences.is_empty());
    }

    #[test]
    fn parity_fails_when_a_cb_direction_changes_band() {
        let s = Scratch::new("trial-parity-cb");
        let rec = recording(&s, &[(0, "eng"), (1, "spa")]);
        for (dir, n) in [("f", 0), ("f", 1), ("r", 0), ("r", 1)] {
            records(&s.0.join(dir), n, None);
        }
        let ctx = Ctx::for_test(&s.0);
        let mut f = arm(&ctx, &path(&rec), &path(&s.0.join("f")));
        let mut r = arm(&ctx, &path(&rec), &path(&s.0.join("r")));
        assert_eq!(f.analysis.as_ref().unwrap().pairs.len(), 1);
        f.analysis.as_mut().unwrap().pairs[0].pair.cb = (Some(0.65), Some(0.55));
        r.analysis.as_mut().unwrap().pairs[0].pair.cb = (Some(0.55), Some(0.55));
        assert_eq!(
            parity(&f, &r, &Rule::default()).unwrap().differences,
            vec!["pair s0 s1: CB s0 in s1 F 0.650 R 0.550 on two sides of 0.600".to_string()]
        );
    }
}
