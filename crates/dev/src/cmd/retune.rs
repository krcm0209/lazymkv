//! `lazymkv-dev retune [--evidence DIR] [--set NAME=VALUE ...] [--snapshot FILE] [--compare FILE] [--spec FILE]
//! [--allow-unmeasured]`: the tuning harness of the audio rule over the evidence clone (spec 3.4; spec 15's
//! `retune` until lazymkvctl exists). Exit 0; 1 a FAIL; 2 an argument, the spec's row 13, a --set value, a labels or
//! fitted file, or a --compare file that is not a snapshot; 3 no evidence clone, or something could not be checked
//! and --allow-unmeasured was not given. Py: audio_dataset.main (its positional corpus path is --evidence).
//!
//! `lazymkv-dev retune --subtitles [--evidence DIR] [--spec FILE] [--text DIR]` (RP15, phase 1 Task 18) runs the
//! subtitle harness instead (`lazymkv_core::subtitle::labels::harness`, spec 15 "Rules against the stored evidence"):
//! every labelled English or no-language subtitle track of the evidence, judged by 8.7 and by the text rule over the
//! OCR text of --text DIR (else LAZYMKV_OCR_TEXT), counted against spec section 17 row 14 read from the spec, with
//! spec 14's default settings. Exit 0; 1 a costly verdict; 2 an argument, the spec's row 14, a subtitle labels file
//! that does not validate, or an OCR text folder that is not there; 3 no evidence clone, no subtitle label, or a
//! labelled track that could not be judged.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::Value;

use lazymkv_core::audio::dataset::{
    destination, harness, parse_sets, read_snapshot, result_line, snapshot_line, why_phrase, Destination,
    HarnessError, HarnessInput, PreviousSnapshot, BASELINE_FILE, SNAPSHOT_FORMAT, SNAPSHOT_FORMAT_V2, TRIAL_FILE,
    UNCHECKED_FILE,
};
use lazymkv_core::audio::labels::exp13_requirements;
use lazymkv_core::recording::{decode_text, Encoding, Errors, Tree};
use lazymkv_core::settings::Settings;
use lazymkv_core::subtitle::labels::{exp14_requirements, harness as subtitle_harness};

use crate::evidence::{
    default_spec, ocr_text_dir, os_error_text, resolve_evidence, Ctx, DiskTree, Refused, EVIDENCE_SHOWN,
};
use crate::pyjson::{dumps, write_snapshot, Style};
use crate::{usage_error, Args};

/// The synopsis after "usage: lazymkv-dev ".
const USAGE: &str = "retune [--evidence DIR] [--set NAME=VALUE] [--snapshot FILE] [--compare FILE] [--spec FILE] [--allow-unmeasured] [--subtitles] [--text DIR]";
const FLAGS: [&str; 8] =
    ["--evidence", "--set", "--snapshot", "--compare", "--spec", "--allow-unmeasured", "--subtitles", "--text"];
/// --subtitles runs the subtitle harness, which takes none of the audio harness's flags (RP15).
const SUBTITLES_ALONE: &str =
    "argument --subtitles: not allowed with --set, --snapshot, --compare or --allow-unmeasured";
/// --text names the OCR text, which only the subtitle harness reads.
const TEXT_NEEDS_SUBTITLES: &str = "argument --text: only with --subtitles";
/// How to give the OCR text, at the end of the refusal of a folder that is not there.
const TEXT_HINT: &str = "(give --text DIR or LAZYMKV_OCR_TEXT)";

pub fn run(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let mut parsed = Args::new(args, &FLAGS);
    let allow_unmeasured = parsed.switch("--allow-unmeasured");
    let subtitles = parsed.switch("--subtitles");
    let text_flag = match parsed.flag("--text") {
        Ok(value) => value,
        Err(message) => return usage_error(err, "retune", USAGE, &message),
    };
    let sets = match parsed.flags("--set") {
        Ok(values) => values,
        Err(message) => return usage_error(err, "retune", USAGE, &message),
    };
    let evidence_flag = match parsed.flag("--evidence") {
        Ok(value) => value,
        Err(message) => return usage_error(err, "retune", USAGE, &message),
    };
    let snapshot_flag = match parsed.flag("--snapshot") {
        Ok(value) => value,
        Err(message) => return usage_error(err, "retune", USAGE, &message),
    };
    let compare_flag = match parsed.flag("--compare") {
        Ok(value) => value,
        Err(message) => return usage_error(err, "retune", USAGE, &message),
    };
    let spec_flag = match parsed.flag("--spec") {
        Ok(value) => value,
        Err(message) => return usage_error(err, "retune", USAGE, &message),
    };
    let words = match parsed.positionals() {
        Ok(words) => words,
        Err(message) => return usage_error(err, "retune", USAGE, &message),
    };
    if !words.is_empty() {
        return usage_error(err, "retune", USAGE, &format!("unrecognized arguments: {}", words.join(" ")));
    }
    let (spec_path, spec_shown) = match spec_file(ctx, spec_flag) {
        Some(found) => found,
        None => return usage_error(err, "retune", USAGE, "the following arguments are required: --spec"),
    };
    if subtitles {
        if !sets.is_empty() || snapshot_flag.is_some() || compare_flag.is_some() || allow_unmeasured {
            return usage_error(err, "retune", USAGE, SUBTITLES_ALONE);
        }
        return run_subtitles(ctx, evidence_flag.as_deref(), &spec_path, &spec_shown, text_flag.as_deref(), out, err);
    }
    if text_flag.is_some() {
        return usage_error(err, "retune", USAGE, TEXT_NEEDS_SUBTITLES);
    }
    let req = match read_text_file(&spec_path, &spec_shown, Encoding::Utf8Sig).and_then(|text| exp13_requirements(&text)) {
        Ok(req) => req,
        Err(message) => return fail(err, &message, 2),
    };
    let overrides = match parse_sets(&sets) {
        Ok(overrides) => overrides,
        Err(message) => return fail(err, &message, 2),
    };
    let evidence = match resolve_evidence(ctx, evidence_flag.as_deref()) {
        Ok(dir) => dir,
        Err(Refused { code, line }) => return fail(err, &line, code),
    };
    let compare = match &compare_flag {
        None => None,
        Some(given) => match read_snapshot_file(&ctx.abs(given)) {
            Some(snapshot) => Some((given.clone(), snapshot)),
            None => {
                let line = format!(
                    "{given}: not a snapshot written by lazymkv-dev retune ({SNAPSHOT_FORMAT}) or by audio_dataset.py ({SNAPSHOT_FORMAT_V2})"
                );
                return fail(err, &line, 2);
            }
        },
    };
    let tree = match DiskTree::open(&evidence, EVIDENCE_SHOWN) {
        Ok(tree) => tree,
        Err(e) => return fail(err, &os_error_text(&e, &evidence.display().to_string()), 3),
    };
    let previous = read_snapshot_file(&evidence.join(BASELINE_FILE));
    let created = ctx.now();
    let input = HarnessInput {
        tree: &tree,
        req: &req,
        overrides: &overrides,
        previous: previous.as_ref(),
        compare: compare.as_ref().map(|(given, snapshot)| (given.as_str(), snapshot)),
        use_cache: true,
        created: &created,
    };
    let h = match harness(&input) {
        Ok(h) => h,
        Err(HarnessError::Labels(lines)) => {
            for line in lines {
                let _ = writeln!(out, "{line}");
            }
            return 2;
        }
    };
    let baseline_shown = format!("{EVIDENCE_SHOWN}/{BASELINE_FILE}");
    let dest = destination(snapshot_flag.is_some(), !overrides.is_empty(), &h.unchecked);
    let (path, shown) = match (dest, &snapshot_flag) {
        (Destination::Given, Some(given)) => (ctx.abs(given), given.clone()),
        (Destination::Trial, _) => {
            let path = ctx.temp_dir().join(TRIAL_FILE);
            let shown = path.display().to_string();
            (path, shown)
        }
        (Destination::Unchecked, _) => {
            let path = ctx.temp_dir().join(UNCHECKED_FILE);
            let shown = path.display().to_string();
            (path, shown)
        }
        _ => (evidence.join(BASELINE_FILE), baseline_shown.clone()),
    };
    let why = why_phrase(dest, &baseline_shown, &h.unchecked);
    let value = match serde_json::to_value(&h.snapshot) {
        Ok(value) => value,
        Err(e) => return fail(err, &e.to_string(), 2),
    };
    let written = match write_unless_unchanged(&path, &value) {
        Ok(written) => written,
        Err(e) => return fail(err, &os_error_text(&e, &shown), 2),
    };
    let (result, code) = result_line(&h.failures, &h.unchecked, allow_unmeasured);
    for line in &h.lines {
        let _ = writeln!(out, "{line}");
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "{}", snapshot_line(&shown, &why, written));
    let _ = writeln!(out, "{result}");
    code
}

/// Prints `line` to stderr and returns `code`.
fn fail(err: &mut dyn Write, line: &str, code: i32) -> i32 {
    let _ = writeln!(err, "{line}");
    code
}

/// The spec file and how it is shown: --spec as given (taken against cwd), else the design spec under the repository
/// root; None when neither names one.
fn spec_file(ctx: &Ctx, flag: Option<String>) -> Option<(PathBuf, String)> {
    match flag {
        Some(given) => Some((ctx.abs(&given), given)),
        None => default_spec(ctx).map(|path| {
            let shown = path.display().to_string();
            (path, shown)
        }),
    }
}

/// `retune --subtitles`: row 14 from the spec (exit 2 when it cannot be read), the evidence (resolve_evidence's code),
/// the OCR text (--text DIR, else LAZYMKV_OCR_TEXT, else none; exit 2 for a folder that is not there), then the
/// subtitle harness with spec 14's default settings; its report on stdout and its exit code. RP15, spec 15.
fn run_subtitles(
    ctx: &Ctx,
    evidence_flag: Option<&str>,
    spec_path: &Path,
    spec_shown: &str,
    text_flag: Option<&str>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let row14 = match read_text_file(spec_path, spec_shown, Encoding::Utf8Sig).and_then(|text| exp14_requirements(&text)) {
        Ok(row14) => row14,
        Err(message) => return fail(err, &message, 2),
    };
    let evidence = match resolve_evidence(ctx, evidence_flag) {
        Ok(dir) => dir,
        Err(Refused { code, line }) => return fail(err, &line, code),
    };
    let tree = match DiskTree::open(&evidence, EVIDENCE_SHOWN) {
        Ok(tree) => tree,
        Err(e) => return fail(err, &os_error_text(&e, &evidence.display().to_string()), 3),
    };
    let text = match ocr_text_dir(ctx, text_flag) {
        None => None,
        Some(dir) => {
            let shown = dir.display().to_string();
            if !dir.is_dir() {
                return fail(err, &format!("the OCR text folder {shown} does not exist {TEXT_HINT}"), 2);
            }
            match DiskTree::open(&dir, &shown) {
                Ok(tree) => Some(tree),
                Err(e) => return fail(err, &os_error_text(&e, &shown), 2),
            }
        }
    };
    let h = subtitle_harness(&tree, text.as_ref().map(|tree| tree as &dyn Tree), &row14, &Settings::default());
    for line in &h.lines {
        let _ = writeln!(out, "{line}");
    }
    h.exit_code
}

/// Path.read_text(encoding) with strict errors; Err with the OSError or UnicodeDecodeError text.
fn read_text_file(path: &Path, shown: &str, encoding: Encoding) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| os_error_text(&e, shown))?;
    decode_text(&bytes, encoding, Errors::Strict)
}

/// load_snapshot(path): the file read as plain "utf-8"; None when it is missing, not text, not JSON or not a
/// snapshot.
fn read_snapshot_file(path: &Path) -> Option<PreviousSnapshot> {
    read_snapshot(&read_text_file(path, "", Encoding::Utf8).ok()?)
}

/// The document as text with "created" set to null in place (`dict(d, created=None)`), so NaN-free documents compare
/// as the Python's `timeless` did.
fn timeless(value: &Value) -> String {
    let mut v = value.clone();
    if let Some(object) = v.as_object_mut() {
        object.insert("created".to_string(), Value::Null);
    }
    dumps(&v, Style::INDENT1_ASCII)
}

/// Writes the snapshot unless `path` already holds it with only another "created"; true when written.
/// Py: write_snapshot's comparison and write.
fn write_unless_unchanged(path: &Path, value: &Value) -> std::io::Result<bool> {
    if let Some(old) = read_snapshot_file(path) {
        if timeless(&old.raw) == timeless(value) {
            return Ok(false);
        }
    }
    write_snapshot(path, value)?;
    Ok(true)
}

#[cfg(test)]
mod tests;
