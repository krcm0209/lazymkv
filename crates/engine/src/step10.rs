//! Step 10 of spec 6.4 in the engine (design parts 5.7, 10.2): the subtitle evidence file, each subtitle stream's packet list and probe-summary.json; Task 15 adds the OCR worker's runs.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use lazymkv_core::recording::files::{MkvmergeIdent, SubsIndexEntry};
use lazymkv_platform::write_atomic;
use lazymkv_tools::{ffprobe, mkvmerge, with_suffix, ToolCtx};
use serde_json::{json, Value};

use crate::staged::UnitError;
use crate::steps::{ended_line, not_run_line, run_line};

/// What step 10's extraction made (design part 10.2 steps 1 and 2): the evidence file and `subs\index.json`'s entries,
/// one per subtitle track of the title in track order, `n` counted from 0 (T5-R3). Def: T5-14. Use: T5-15.
#[allow(dead_code)] // read_streams reads it from Task 15 on
#[derive(Clone, Debug)]
pub(crate) struct Extracted {
    /// The subtitle-only evidence file on the staging share.
    pub(crate) evidence: PathBuf,
    /// `subs\index.json`'s entries, in track order.
    pub(crate) index: Vec<SubsIndexEntry>,
}

/// What `probe-summary.json` holds; None is a value not measured yet, written as null.
#[derive(Default)]
struct Summary {
    file_bytes: Option<u64>,
    subtitle_tracks: Option<usize>,
    evidence_bytes: Option<u64>,
    evidence_ms: Option<u64>,
}

/// `<path>` and the error, the sentence of a file step 10 could not read or write.
fn shown(path: &Path, error: impl std::fmt::Display) -> String {
    format!("{}: {error}", path.display())
}

/// Step 10's extraction and packet lists for one title (design part 10.2 steps 1 and 2; T5-R56). Def: T5-14.
/// Use: T5-14, T5-18.
#[allow(clippy::too_many_arguments)]
pub(crate) fn extract(
    ctx: &ToolCtx,
    mkvmerge: &Path,
    ffprobe: &Path,
    raw: &Path,
    ident: &MkvmergeIdent,
    evidence: &Path,
    label_dir: &Path,
    log: &dyn Fn(&str),
) -> Result<Extracted, UnitError> {
    let mut summary = Summary::default();
    let made = extract_into(ctx, mkvmerge, ffprobe, raw, ident, evidence, label_dir, log, &mut summary);
    let path = label_dir.join("probe-summary.json");
    match made {
        Ok(extracted) => {
            write_summary(&path, raw, &summary, None).map_err(|error| UnitError::Failed(shown(&path, error)))?;
            Ok(extracted)
        }
        Err(error) => {
            let sentence = match &error {
                UnitError::Failed(sentence) | UnitError::Staging(sentence) => sentence.clone(),
            };
            // Phase 0 writes its summary on a failure too (probe-title.ps1 lines 131 to 137); a failure to write it
            // leaves the first error.
            let _ = write_summary(&path, raw, &summary, Some(&sentence));
            Err(error)
        }
    }
}

/// `probe-summary.json` (R5-code-29: phase 0's `evidence_s` and remux-test keys are left out): `ok` and `result` are
/// `true` and `ok`, or `false` and `FAILED: <failure>`; pretty JSON and `\n` through `write_atomic`.
fn write_summary(path: &Path, raw: &Path, summary: &Summary, failure: Option<&str>) -> io::Result<()> {
    let doc = json!({
        "ok": failure.is_none(),
        "result": failure.map_or_else(|| "ok".to_string(), |failure| format!("FAILED: {failure}")),
        "file": raw.display().to_string(),
        "file_bytes": summary.file_bytes,
        "subtitle_tracks": summary.subtitle_tracks,
        "evidence_bytes": summary.evidence_bytes,
        "evidence_ms": summary.evidence_ms,
    });
    write_atomic(path, &pretty(&doc))
}

/// `value` as pretty JSON and a final newline.
fn pretty(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(value).expect("a JSON value serializes");
    bytes.push(b'\n');
    bytes
}

/// `extract`'s steps 1 to 8, filling `summary` as each value is measured.
#[allow(clippy::too_many_arguments)]
fn extract_into(
    ctx: &ToolCtx,
    mkvmerge: &Path,
    ffprobe: &Path,
    raw: &Path,
    ident: &MkvmergeIdent,
    evidence: &Path,
    label_dir: &Path,
    log: &dyn Fn(&str),
    summary: &mut Summary,
) -> Result<Extracted, UnitError> {
    let key = label_dir.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    let file_bytes = fs::metadata(raw).map_err(|error| UnitError::Staging(shown(raw, error)))?.len();
    summary.file_bytes = Some(file_bytes);

    let tracks: Vec<_> = ident.tracks.iter().filter(|track| track.r#type == "subtitles").collect();
    if let Some(track) = tracks.iter().find(|track| track.id < 0) {
        return Err(UnitError::Failed(format!("{key}/mkvmerge.json: track id {} is not a track number", track.id)));
    }
    if tracks.is_empty() {
        return Err(UnitError::Failed(format!("{key}/mkvmerge.json lists no subtitle track")));
    }
    let ids: Vec<i64> = tracks.iter().map(|track| track.id).collect();
    summary.subtitle_tracks = Some(tracks.len());

    let subs = label_dir.join("subs");
    fs::create_dir_all(&subs).map_err(|error| UnitError::Failed(shown(&subs, error)))?;
    if let Some(folder) = evidence.parent() {
        fs::create_dir_all(folder).map_err(|error| UnitError::Staging(shown(folder, error)))?;
    }
    match fs::remove_file(evidence) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(UnitError::Staging(shown(evidence, error))),
        _ => {}
    }

    let out = label_dir.join("evidence-mkvmerge.txt");
    let name = format!("{key}/evidence-mkvmerge.txt");
    let args = mkvmerge::extract_args(evidence, &ids, raw);
    let started = ctx.host.now_ms();
    let ran = recorded(&name, mkvmerge, &args, &out, log, || mkvmerge::extract(ctx, mkvmerge, &args, &out));
    summary.evidence_ms = Some(ctx.host.now_ms().saturating_sub(started));
    match ran {
        Err(error) => return Err(UnitError::Failed(format!("mkvmerge could not run: {error}"))),
        Ok(Err(message)) => return Err(UnitError::Failed(message)),
        Ok(Ok(())) => {}
    }

    let evidence_bytes = match fs::metadata(evidence) {
        Ok(metadata) => metadata.len(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let wrote = format!("mkvmerge wrote no {}; see {key}/evidence-mkvmerge.txt", evidence.display());
            return Err(UnitError::Failed(wrote));
        }
        Err(error) => return Err(UnitError::Staging(shown(evidence, error))),
    };
    if evidence_bytes == 0 {
        return Err(UnitError::Failed(format!("{} is empty", evidence.display())));
    }
    summary.evidence_bytes = Some(evidence_bytes);

    for n in 0..tracks.len() {
        let stream = n as i64;
        let out = subs.join(format!("sub-{n}.packets.json"));
        let name = format!("{key}/subs/sub-{n}.packets.json");
        let args = ffprobe::packets_args(stream, evidence);
        match recorded(&name, ffprobe, &args, &out, log, || ffprobe::packets(ctx, ffprobe, &args, &out)) {
            Err(error) => return Err(UnitError::Failed(format!("ffprobe could not run: {error}"))),
            Ok(Err(message)) => return Err(UnitError::Failed(format!("subtitle stream s:{n}: {message}"))),
            Ok(Ok(())) => {}
        }
    }

    // subs\index.json (scripts/phase0/probe-title.ps1 lines 102 to 110): an absent property is null.
    let index: Vec<SubsIndexEntry> = tracks
        .iter()
        .enumerate()
        .map(|(n, track)| {
            let property = |name: &str| track.properties.get(name);
            SubsIndexEntry {
                n: n as i64,
                track_id: Some(track.id),
                language: property("language").and_then(Value::as_str).map(String::from),
                codec: Some(track.codec.clone()),
                track_name: property("track_name").and_then(Value::as_str).map(String::from),
                forced: property("forced_track").and_then(Value::as_bool),
                default: property("default_track").and_then(Value::as_bool),
            }
        })
        .collect();
    let doc: Vec<Value> = index
        .iter()
        .map(|entry| {
            json!({
                "n": entry.n,
                "track_id": entry.track_id,
                "language": entry.language,
                "codec": entry.codec,
                "track_name": entry.track_name,
                "forced": entry.forced,
                "default": entry.default,
            })
        })
        .collect();
    let path = subs.join("index.json");
    write_atomic(&path, &pretty(&Value::Array(doc))).map_err(|error| UnitError::Failed(shown(&path, error)))?;
    Ok(Extracted { evidence: evidence.to_path_buf(), index })
}

/// One run recorded into `out`: its old `<out>.exit.json` removed, the `run` line, the run, then the `ended` line from
/// the exit file, or the error of a run that could not be recorded.
fn recorded(
    name: &str,
    program: &Path,
    args: &[String],
    out: &Path,
    log: &dyn Fn(&str),
    run: impl FnOnce() -> io::Result<Result<(), String>>,
) -> io::Result<Result<(), String>> {
    let _ = fs::remove_file(with_suffix(out, ".exit.json"));
    log(&run_line(name, program, args));
    let ran = run();
    match &ran {
        Ok(_) => log(&ended_line(name, out)),
        Err(error) => log(&not_run_line(name, error)),
    }
    ran
}
