//! ocrs trial (lazymkv phase 0, decisions D17, D19 and D22): reads every *.png of a folder, in name order, with the
//! ocrs library and the two models given on the command line. Nothing is downloaded: the library never fetches models.
//! With --crop <pad>, only the box of the dark pixels (the glyphs; the frames are dark text on white), grown by <pad>
//! pixels, is read. Writes one JSON line per frame and a last {"summary": {...}} line to --out (see README.md).
//! Standard output gets progress and timing only, never recognised text.

use std::error::Error;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use image::{imageops, GrayImage, RgbImage};
use ocrs::{ImageSource, OcrEngine, OcrEngineParams};
use rten::Model;
use serde_json::json;
use sha1::{Digest, Sha1};

/// The ocrs version pinned in Cargo.toml (`=0.13.1`); the two must change together.
const OCRS_VERSION: &str = "0.13.1";
/// A grey value below this is a glyph pixel (--crop).
const DARK: u8 = 128;

const USAGE: &str = "usage: ocrs-trial --detection <model file> --recognition <model file> --frames <dir> \
                     --out <jsonl> [--limit N] [--crop <pad>]";

struct Args {
    detection: PathBuf,
    recognition: PathBuf,
    frames: PathBuf,
    out: PathBuf,
    limit: Option<usize>,
    crop: Option<u32>,
}

fn parse_args(raw: &[String]) -> Result<Args, String> {
    let (mut detection, mut recognition, mut frames, mut out) = (None, None, None, None);
    let (mut limit, mut crop) = (None, None);
    let mut it = raw.iter();
    while let Some(flag) = it.next() {
        let value = it.next().ok_or_else(|| format!("{flag} needs a value\n{USAGE}"))?;
        let number = || format!("{flag} takes a whole number, not '{value}'");
        match flag.as_str() {
            "--detection" => detection = Some(PathBuf::from(value)),
            "--recognition" => recognition = Some(PathBuf::from(value)),
            "--frames" => frames = Some(PathBuf::from(value)),
            "--out" => out = Some(PathBuf::from(value)),
            "--limit" => limit = Some(value.parse::<usize>().map_err(|_| number())?),
            "--crop" => crop = Some(value.parse::<u32>().map_err(|_| number())?),
            _ => return Err(format!("unknown argument '{flag}'\n{USAGE}")),
        }
    }
    let need = |v: Option<PathBuf>, name: &str| v.ok_or_else(|| format!("missing {name}\n{USAGE}"));
    Ok(Args {
        detection: need(detection, "--detection")?,
        recognition: need(recognition, "--recognition")?,
        frames: need(frames, "--frames")?,
        out: need(out, "--out")?,
        limit,
        crop,
    })
}

/// Lower-case hex SHA-1 of a file, and its size in bytes.
fn sha1_file(path: &Path) -> Result<(String, usize), Box<dyn Error>> {
    let bytes = fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let hex: String = Sha1::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
    Ok((hex, bytes.len()))
}

fn load_model(path: &Path, what: &str) -> Result<Model, Box<dyn Error>> {
    Ok(Model::load_file(path).map_err(|e| format!("cannot load the {what} model {}: {e}", path.display()))?)
}

/// The *.png files of `dir` (any case of the extension), sorted by file name.
fn frame_files(dir: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).map_err(|e| format!("cannot list {}: {e}", dir.display()))? {
        let path = entry?.path();
        if path.is_file() && path.extension().map_or(false, |x| x.eq_ignore_ascii_case("png")) {
            files.push(path);
        }
    }
    files.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    Ok(files)
}

fn ms_since(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

/// [x, y, w, h] of the box around every pixel darker than DARK, grown by `pad` on each side and clamped to the
/// image; None when no pixel is that dark.
fn dark_box(grey: &GrayImage, pad: u32) -> Option<[u32; 4]> {
    let (width, height) = grey.dimensions();
    if width == 0 || height == 0 {
        return None;
    }
    let mut found: Option<(u32, u32, u32, u32)> = None;
    for (y, row) in grey.as_raw().chunks_exact(width as usize).enumerate() {
        if let Some(first) = row.iter().position(|&v| v < DARK) {
            let last = row.iter().rposition(|&v| v < DARK).unwrap_or(first);
            let (x, y, last) = (first as u32, y as u32, last as u32);
            found = Some(match found {
                None => (x, y, last, y),
                Some((x0, y0, x1, _)) => (x0.min(x), y0, x1.max(last), y),
            });
        }
    }
    let (x0, y0, x1, y1) = found?;
    let (left, top) = (x0.saturating_sub(pad), y0.saturating_sub(pad));
    let (right, bottom) = (x1.saturating_add(pad).min(width - 1), y1.saturating_add(pad).min(height - 1));
    Some([left, top, right - left + 1, bottom - top + 1])
}

/// One frame read, and where its time went: decode (PNG to pixels), crop (box scan and copy), detect (ocrs's
/// preparation, word detection and line grouping) and rec (line recognition). ms = detect + rec, the OCR alone.
struct Frame {
    lines: Vec<String>,
    crop: Option<[u32; 4]>,
    decode_ms: f64,
    crop_ms: f64,
    detect_ms: f64,
    rec_ms: f64,
}

/// The recognised lines of one image, top to bottom, each trimmed; empty lines are left out. Adds the detection and
/// recognition times to `frame`.
fn ocr(engine: &OcrEngine, img: &RgbImage, frame: &mut Frame) -> Result<Vec<String>, Box<dyn Error>> {
    let t = Instant::now();
    let source = ImageSource::from_bytes(img.as_raw(), img.dimensions())?;
    let input = engine.prepare_input(source)?;
    let words = engine.detect_words(&input)?;
    let line_rects = engine.find_text_lines(&input, &words);
    frame.detect_ms = ms_since(t);
    let t = Instant::now();
    let lines: Vec<String> = engine
        .recognize_text(&input, &line_rects)?
        .into_iter()
        .flatten()
        .map(|line| line.to_string().trim().to_string())
        .filter(|line| !line.is_empty())
        .collect();
    frame.rec_ms = ms_since(t);
    Ok(lines)
}

fn read_frame(engine: &OcrEngine, path: &Path, pad: Option<u32>) -> Result<Frame, Box<dyn Error>> {
    let mut frame = Frame { lines: Vec::new(), crop: None, decode_ms: 0.0, crop_ms: 0.0, detect_ms: 0.0, rec_ms: 0.0 };
    // The frames are 8-bit grey; ocrs takes HWC bytes, and RGB is the form its own example and CLI hand it.
    let t = Instant::now();
    let decoded = image::open(path)?;
    let grey = pad.map(|_| decoded.to_luma8());
    let rgb = decoded.into_rgb8();
    frame.decode_ms = ms_since(t);
    let Some(pad) = pad else {
        frame.lines = ocr(engine, &rgb, &mut frame)?;
        return Ok(frame);
    };
    let t = Instant::now();
    frame.crop = grey.as_ref().and_then(|g| dark_box(g, pad));
    let Some([x, y, w, h]) = frame.crop else {
        // No dark pixel: nothing to read, so no OCR at all.
        frame.crop_ms = ms_since(t);
        return Ok(frame);
    };
    let sub = imageops::crop_imm(&rgb, x, y, w, h).to_image();
    frame.crop_ms = ms_since(t);
    frame.lines = ocr(engine, &sub, &mut frame)?;
    Ok(frame)
}

fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

/// Mean of `xs` to 0.1, or null when there is none.
fn mean1(xs: &[f64]) -> serde_json::Value {
    if xs.is_empty() {
        serde_json::Value::Null
    } else {
        json!(round1(xs.iter().sum::<f64>() / xs.len() as f64))
    }
}

fn run(args: &Args) -> Result<(), Box<dyn Error>> {
    let (det_sha1, det_bytes) = sha1_file(&args.detection)?;
    let (rec_sha1, rec_bytes) = sha1_file(&args.recognition)?;
    println!("models: detection {det_sha1} ({det_bytes} bytes), recognition {rec_sha1} ({rec_bytes} bytes)");
    let t_load = Instant::now();
    let engine = OcrEngine::new(OcrEngineParams {
        detection_model: Some(load_model(&args.detection, "detection")?),
        recognition_model: Some(load_model(&args.recognition, "recognition")?),
        ..Default::default()
    })?;
    let load_ms = round1(ms_since(t_load));

    let mut files = frame_files(&args.frames)?;
    let found = files.len();
    if let Some(n) = args.limit {
        files.truncate(n);
    }
    if files.is_empty() {
        let dir = args.frames.display();
        return Err(format!("no *.png to read in {dir} ({found} found, --limit {:?})", args.limit).into());
    }
    let (count, pad) = (files.len(), args.crop);
    println!("ocrs {OCRS_VERSION}: models loaded in {load_ms} ms; reading {count} of {found} frames, crop pad {pad:?}");

    let file = File::create(&args.out).map_err(|e| format!("cannot write {}: {e}", args.out.display()))?;
    let mut out = BufWriter::new(file);
    let mut ms_total = 0.0;
    // Per-frame times; read_ms, detect and rec only of the frames that went through OCR.
    let (mut read_ms, mut decode, mut crop_ms, mut detect, mut rec) = (vec![], vec![], vec![], vec![], vec![]);
    let mut boxes: Vec<[u32; 4]> = Vec::new();
    for (k, path) in files.iter().enumerate() {
        let f = read_frame(&engine, path, args.crop).map_err(|e| format!("frame {k} ({}): {e}", path.display()))?;
        let ms = f.detect_ms + f.rec_ms;
        ms_total += ms;
        decode.push(f.decode_ms);
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let mut record = json!({"frame": k, "file": name, "lines": f.lines, "ms": round1(ms),
                                "decode_ms": round1(f.decode_ms), "detect_ms": round1(f.detect_ms),
                                "rec_ms": round1(f.rec_ms)});
        if args.crop.is_some() {
            record["crop"] = json!(f.crop);
            record["crop_ms"] = json!(round1(f.crop_ms));
            crop_ms.push(f.crop_ms);
        }
        // A frame is read (goes through OCR) unless --crop found no dark pixel in it.
        if args.crop.is_none() || f.crop.is_some() {
            read_ms.push(ms);
            detect.push(f.detect_ms);
            rec.push(f.rec_ms);
        }
        boxes.extend(f.crop);
        writeln!(out, "{record}")?;
        if (k + 1) % 50 == 0 {
            println!("{} of {} frames, {:.1} ms OCR per frame so far", k + 1, files.len(), ms_total / (k + 1) as f64);
        }
    }
    let n = files.len();
    let per_frame = round1(ms_total / n as f64);
    let column = |i: usize| boxes.iter().map(|b| b[i] as f64).collect::<Vec<f64>>();
    let summary = json!({"summary": {
        "frames": n, "ms_total": round1(ms_total), "ms_per_frame": per_frame, "load_ms": load_ms,
        "ocrs": OCRS_VERSION, "models": {"detection": det_sha1, "recognition": rec_sha1},
        "crop_pad": args.crop, "read_frames": read_ms.len(), "ms_per_read_frame": mean1(&read_ms),
        "decode_ms_per_frame": mean1(&decode), "crop_ms_per_frame": mean1(&crop_ms),
        "detect_ms_per_read_frame": mean1(&detect), "rec_ms_per_read_frame": mean1(&rec),
        "crop_boxes": boxes.len(), "crop_mean_w": mean1(&column(2)), "crop_mean_h": mean1(&column(3))
    }});
    writeln!(out, "{summary}")?;
    out.flush()?;
    let seconds = ms_total / 1000.0;
    println!("done: {n} frames, {seconds:.1} s of OCR, {per_frame} ms per frame; written {}", args.out.display());
    Ok(())
}

fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let args = match parse_args(&raw) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    if let Err(error) = run(&args) {
        eprintln!("ocrs-trial: {error}");
        std::process::exit(1);
    }
}
