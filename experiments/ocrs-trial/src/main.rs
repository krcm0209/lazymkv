//! ocrs trial (lazymkv phase 0, decisions D17 and D19): reads every *.png of a folder, in name order, with the ocrs
//! library and the two models given on the command line. Nothing is downloaded: the library never fetches models.
//! Writes one JSON line per frame ({"frame", "file", "lines", "ms"}) and a last {"summary": {...}} line to --out.
//! Standard output gets progress and timing only, never recognised text. See README.md.

use std::error::Error;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use ocrs::{ImageSource, OcrEngine, OcrEngineParams};
use rten::Model;
use serde_json::json;
use sha1::{Digest, Sha1};

/// The ocrs version pinned in Cargo.toml (`=0.13.1`); the two must change together.
const OCRS_VERSION: &str = "0.13.1";

const USAGE: &str = "usage: ocrs-trial --detection <model file> --recognition <model file> --frames <dir> \
                     --out <jsonl> [--limit N]";

struct Args {
    detection: PathBuf,
    recognition: PathBuf,
    frames: PathBuf,
    out: PathBuf,
    limit: Option<usize>,
}

fn parse_args(raw: &[String]) -> Result<Args, String> {
    let (mut detection, mut recognition, mut frames, mut out, mut limit) = (None, None, None, None, None);
    let mut it = raw.iter();
    while let Some(flag) = it.next() {
        let value = it.next().ok_or_else(|| format!("{flag} needs a value\n{USAGE}"))?;
        match flag.as_str() {
            "--detection" => detection = Some(PathBuf::from(value)),
            "--recognition" => recognition = Some(PathBuf::from(value)),
            "--frames" => frames = Some(PathBuf::from(value)),
            "--out" => out = Some(PathBuf::from(value)),
            "--limit" => {
                let n: usize = value.parse().map_err(|_| format!("--limit takes a whole number, not '{value}'"))?;
                limit = Some(n);
            }
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

/// The recognised lines of one frame, top to bottom, each trimmed; empty lines are left out.
fn read_frame(engine: &OcrEngine, path: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    // The frames are 8-bit grey; ocrs takes HWC bytes, and RGB is the form its own example and CLI hand it.
    let img = image::open(path)?.into_rgb8();
    let source = ImageSource::from_bytes(img.as_raw(), img.dimensions())?;
    let input = engine.prepare_input(source)?;
    let words = engine.detect_words(&input)?;
    let line_rects = engine.find_text_lines(&input, &words);
    let lines: Vec<String> = engine
        .recognize_text(&input, &line_rects)?
        .into_iter()
        .flatten()
        .map(|line| line.to_string().trim().to_string())
        .filter(|line| !line.is_empty())
        .collect();
    Ok(lines)
}

fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
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
    let load_ms = round1(t_load.elapsed().as_secs_f64() * 1000.0);

    let mut files = frame_files(&args.frames)?;
    let found = files.len();
    if let Some(n) = args.limit {
        files.truncate(n);
    }
    if files.is_empty() {
        let dir = args.frames.display();
        return Err(format!("no *.png to read in {dir} ({found} found, --limit {:?})", args.limit).into());
    }
    println!("ocrs {OCRS_VERSION}: models loaded in {load_ms} ms; reading {} of {found} frames", files.len());

    let file = File::create(&args.out).map_err(|e| format!("cannot write {}: {e}", args.out.display()))?;
    let mut out = BufWriter::new(file);
    let mut ms_total = 0.0;
    for (k, path) in files.iter().enumerate() {
        let t0 = Instant::now();
        let lines = read_frame(&engine, path).map_err(|e| format!("frame {k} ({}): {e}", path.display()))?;
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        ms_total += ms;
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        writeln!(out, "{}", json!({"frame": k, "file": name, "lines": lines, "ms": round1(ms)}))?;
        if (k + 1) % 50 == 0 {
            println!("{} of {} frames, {:.1} ms per frame so far", k + 1, files.len(), ms_total / (k + 1) as f64);
        }
    }
    let n = files.len();
    let per_frame = round1(ms_total / n as f64);
    let summary = json!({"summary": {
        "frames": n, "ms_total": round1(ms_total), "ms_per_frame": per_frame, "load_ms": load_ms,
        "ocrs": OCRS_VERSION, "models": {"detection": det_sha1, "recognition": rec_sha1}
    }});
    writeln!(out, "{summary}")?;
    out.flush()?;
    let seconds = ms_total / 1000.0;
    println!("done: {n} frames in {seconds:.1} s, {per_frame} ms per frame; written {}", args.out.display());
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
