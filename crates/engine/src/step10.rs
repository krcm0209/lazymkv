//! Step 10 of spec 6.4 in the engine (design parts 5.7, 10.2, 10.7, 10.8): the subtitle evidence file, each subtitle stream's packet list and probe-summary.json, then the OCR worker's run of each English or no-language bitmap stream, its output moved into the staged tuning folder and its counts written into the job folder.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use lazymkv_core::ipc::CANCELLED;
use lazymkv_core::recording::files::{MkvmergeIdent, Packet, PacketsFile, SubsIndexEntry};
use lazymkv_core::subtitle::bitmap::cap::{cap_reason, time_cap_s};
use lazymkv_core::subtitle::bitmap::evidence::{
    check_events, frames_json, read_run_json, summary_json, text_counts, whole_records, Markers, RunTotals,
    StreamOutcome, StreamSummary,
};
use lazymkv_core::subtitle::text::{is_text_codec, packet_events, READING};
use lazymkv_platform::write_atomic;
use lazymkv_tools::{
    end_reason, ffprobe, mkvmerge, ocr_args, run_worker_limited, with_suffix, ToolCtx, WorkerEnd, WORKER_MEMORY_LIMIT,
};
use serde_json::{json, Value};

use crate::staged::{copy_checked, copy_tree_checked, move_checked, UnitError};
use crate::steps::{code_text, ended_line, exit_file, not_run_line, run_line, shown};

/// A title that needs step 10 when the engine has no worker beside it (design part 10.7; T5-R8). Def: T5-15.
/// Use: T5-15.
pub(crate) const OCR_MISSING: &str = "lazymkv-ocr is not beside the engine, so step 10 cannot read subtitles";

/// How one stream's reading ended (design parts 10.2, 10.8): read with its whole records, or unread with the note's
/// reason, which holds no subtitle text. Def: T5-15. Use: T5-15, T5-18.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum StreamRead {
    /// Read: the number of whole records the worker wrote.
    Read { records: usize },
    /// Unread, with the note's reason.
    Unread { reason: String },
}

/// What step 10's extraction made (design part 10.2 steps 1 and 2): the evidence file and `subs\index.json`'s entries,
/// one per subtitle track of the title in track order, `n` counted from 0 (T5-R3). Def: T5-14. Use: T5-15.
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

/// The index entries the worker reads (design part 10.2 step 3; T5-R11; R2-decoders-47): language `eng`, `en`, `und`,
/// empty or absent, and a codec that `is_text_codec` does not name, in index order. Def: T5-15. Use: T5-15.
pub(crate) fn ocr_streams(index: &[SubsIndexEntry]) -> Vec<&SubsIndexEntry> {
    index
        .iter()
        .filter(|entry| matches!(entry.language.as_deref(), None | Some("eng" | "en" | "und" | "")))
        .filter(|entry| !is_text_codec(entry.codec.as_deref()))
        .collect()
}

/// A stream's cap (design part 10.8, spec 14; T5-R10, T5-R58): `over` when the harness set one, else
/// `time_cap_s(events)` seconds. Def: T5-15. Use: T5-15.
pub(crate) fn stream_cap(over: Option<Duration>, events: usize) -> Duration {
    over.unwrap_or_else(|| Duration::from_secs_f64(time_cap_s(events)))
}

/// Step 10's reading of one title (design part 10.2 steps 3 to 5, 10.7, 10.8, 10.12; B1 (the worker's folder); T5-R4,
/// T5-R8, T5-R9, T5-R12, T5-R55, T5-R57, T5-R59): each `ocr_streams` entry of `extracted.index` read by `worker` in
/// `title_work`, one at a time, its cap `cap(events)`; a read stream's output moved into `<tuning>\subs-ocr\` and its
/// `sub-<n>.frames.json` written into `<label_dir>\subs-ocr\`; `summary.json` written there and then copied into the
/// staged folder; `title_work` deleted. `log` gets each run's job.log lines. Err: `Failed` for a local or argument
/// failure (and `CANCELLED` once the cancel is raised), `Staging` for a staging write that failed. Def: T5-15.
/// Use: T5-15, T5-18.
#[allow(clippy::too_many_arguments)]
pub(crate) fn read_streams(
    ctx: &ToolCtx,
    worker: &Path,
    extracted: &Extracted,
    label_dir: &Path,
    tuning: &Path,
    title_work: &Path,
    clear_max: i64,
    cap: &dyn Fn(usize) -> Duration,
    log: &dyn Fn(&str),
) -> Result<BTreeMap<i64, StreamRead>, UnitError> {
    let read = read_into(ctx, worker, extracted, label_dir, tuning, title_work, clear_max, cap, log);
    // Every end deletes the work folder: the text and crops of an unread or interrupted run go with it (P19).
    match fs::remove_dir_all(title_work) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => {
            log(&format!("the work folder {} could not be removed: {error}", title_work.display()));
        }
        _ => {}
    }
    read
}

/// `dir` emptied and made again: an old folder removed (NotFound is no error), then `create_dir_all`.
fn fresh(dir: &Path) -> io::Result<()> {
    match fs::remove_dir_all(dir) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    fs::create_dir_all(dir)
}

/// What an exit-0 run wrote into `out`, checked: the records' text, run.json's totals and frames.json's text, or the
/// unread reason, which names files and numbers only.
fn read_output(out: &Path, n: i64, packets: &[Packet], clear_max: i64) -> Result<(String, RunTotals, String), String> {
    let name = format!("sub-{n}.ocr.jsonl");
    let records = match fs::read_to_string(out.join(&name)) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Err(format!("lazymkv-ocr wrote no {name}")),
        Err(error) => return Err(format!("{name} could not be read: {error}")),
    };
    let run_name = format!("sub-{n}.run.json");
    let totals = match fs::read_to_string(out.join(&run_name)) {
        Ok(text) => read_run_json(&text)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Err(format!("lazymkv-ocr wrote no {run_name}")),
        Err(error) => return Err(format!("{run_name} could not be read: {error}")),
    };
    check_events(&whole_records(&records), packets, clear_max)?;
    let frames = frames_json(&records)?;
    Ok((records, totals, frames))
}

/// `read_streams` before its work folder is deleted.
#[allow(clippy::too_many_arguments)]
fn read_into(
    ctx: &ToolCtx,
    worker: &Path,
    extracted: &Extracted,
    label_dir: &Path,
    tuning: &Path,
    title_work: &Path,
    clear_max: i64,
    cap: &dyn Fn(usize) -> Duration,
    log: &dyn Fn(&str),
) -> Result<BTreeMap<i64, StreamRead>, UnitError> {
    let failed = |path: &Path, error: &dyn std::fmt::Display| UnitError::Failed(shown(path, error));
    let staging = |path: &Path, error: &dyn std::fmt::Display| UnitError::Staging(shown(path, error));
    // A re-run reads every stream again (until Task 18's C3 skip), so what an earlier run left goes first.
    let subs_ocr = tuning.join(READING);
    fresh(&subs_ocr).map_err(|error| staging(&subs_ocr, &error))?;
    let counts = label_dir.join(READING);
    fresh(&counts).map_err(|error| failed(&counts, &error))?;
    fresh(title_work).map_err(|error| failed(title_work, &error))?;

    let evidence = title_work.join("evidence.mks");
    let mut summaries = Vec::new();
    let mut reads = BTreeMap::new();
    for (at, entry) in ocr_streams(&extracted.index).into_iter().enumerate() {
        let n = entry.n;
        let path = label_dir.join("subs").join(format!("sub-{n}.packets.json"));
        let bytes = fs::read(&path).map_err(|error| failed(&path, &error))?;
        let packets: PacketsFile = serde_json::from_slice(&bytes).map_err(|error| failed(&path, &error))?;
        let events = packet_events(packets.packets(), clear_max).map_err(UnitError::Failed)?.len();
        if at == 0 {
            copy_checked(&extracted.evidence, &evidence).map_err(|error| staging(&extracted.evidence, &error))?;
        }

        let out = title_work.join(format!("s{n}"));
        fs::create_dir_all(&out).map_err(|error| failed(&out, &error))?;
        let capture = title_work.join(format!("s{n}.capture.txt"));
        let args = ocr_args(&evidence, n, clear_max, &out);
        log(&format!("run lazymkv-ocr s{n}: {} {}", worker.display(), args.join(" ")));
        let limit = cap(events);
        let end = run_worker_limited(ctx, worker, &args, &capture, limit)
            .map_err(|error| UnitError::Failed(format!("lazymkv-ocr could not run: {error}")))?;
        let (exit, wall_ms) = exit_file(&capture).unwrap_or((None, 0));
        log(&format!("ended lazymkv-ocr s{n}: exit {} after {wall_ms} ms", code_text(exit)));
        if matches!(end, WorkerEnd::Exited(_)) {
            // T6-R3: what the engine asked for, true on every system; only Windows enforces it.
            let started = format!("lazymkv-ocr s{n} was started with a memory limit of {WORKER_MEMORY_LIMIT} bytes");
            log(&format!("{started} (design part 5.3)"));
        }

        let outcome = match &end {
            WorkerEnd::Cancelled => return Err(UnitError::Failed(CANCELLED.to_string())),
            WorkerEnd::ArgumentError(_) => return Err(UnitError::Failed(end_reason(&end).unwrap_or_default())),
            WorkerEnd::TimedOut => {
                let records = fs::read(out.join(format!("sub-{n}.ocr.jsonl")))
                    .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
                    .unwrap_or_default();
                Err(cap_reason(limit.as_secs_f64(), whole_records(&records).len(), events))
            }
            WorkerEnd::Refused(_) | WorkerEnd::Exited(_) => Err(end_reason(&end).unwrap_or_default()),
            WorkerEnd::Read => read_output(&out, n, packets.packets(), clear_max),
        };
        let codec = entry.codec.clone().unwrap_or_default();
        match outcome {
            Err(reason) => {
                // P19: the partial text and crops go now; the work folder's deletion is the backstop.
                let _ = fs::remove_dir_all(&out);
                summaries.push(StreamSummary {
                    n,
                    codec,
                    outcome: StreamOutcome::Unread(reason.clone()),
                    exit,
                    wall_ms,
                    with_text: 0,
                    lines: 0,
                    markers: Markers::default(),
                });
                reads.insert(n, StreamRead::Unread { reason });
            }
            Ok((records, totals, frames)) => {
                for name in [format!("sub-{n}.ocr.jsonl"), format!("sub-{n}.run.json")] {
                    let to = subs_ocr.join(&name);
                    move_checked(&out.join(&name), &to).map_err(|error| staging(&to, &error))?;
                }
                let crops = out.join(format!("s{n}"));
                if crops.is_dir() {
                    let to = subs_ocr.join(format!("s{n}"));
                    copy_tree_checked(&crops, &to).map_err(|error| staging(&to, &error))?;
                }
                let path = counts.join(format!("sub-{n}.frames.json"));
                write_atomic(&path, frames.as_bytes()).map_err(|error| failed(&path, &error))?;
                let (with_text, lines, markers) = text_counts(&records);
                summaries.push(StreamSummary {
                    n,
                    codec,
                    outcome: StreamOutcome::Read(totals),
                    exit,
                    wall_ms,
                    with_text,
                    lines,
                    markers,
                });
                reads.insert(n, StreamRead::Read { records: whole_records(&records).len() });
            }
        }
    }

    // T5-R12: the job folder's summary.json first, the staged copy last, since C3 reads it as the folder's marker.
    let summary = counts.join("summary.json");
    write_atomic(&summary, summary_json(&summaries).as_bytes()).map_err(|error| failed(&summary, &error))?;
    let staged = subs_ocr.join("summary.json");
    copy_checked(&summary, &staged).map_err(|error| staging(&staged, &error))?;
    Ok(reads)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(n: i64, language: Option<&str>, codec: &str) -> SubsIndexEntry {
        SubsIndexEntry {
            n,
            track_id: Some(n + 2),
            language: language.map(str::to_string),
            codec: Some(codec.to_string()),
            track_name: None,
            forced: Some(false),
            default: Some(false),
        }
    }

    /// T5-R11, R2-decoders-47, R6-phase0-14: a text stream (the closed captions MakeMKV converts to SubRip) is never
    /// given to the worker, so summary.json names it nowhere and it gets no note; every English or no-language stream
    /// of any other codec goes to the worker, which refuses a codec other than PGS and VobSub with exit 3.
    #[test]
    fn a_text_stream_is_set_aside_and_gets_no_note() {
        let index = [
            entry(0, Some("eng"), "SubRip/SRT"),
            entry(1, Some("eng"), "HDMV PGS"),
            entry(2, Some("en"), "VobSub"),
            entry(3, Some("und"), "HDMV PGS"),
            entry(4, None, "VobSub"),
            entry(5, Some(""), "HDMV PGS"),
            entry(6, Some("fre"), "HDMV PGS"),
            entry(7, Some("eng"), "DVBSUB"),
            entry(8, Some("eng"), "SubStationAlpha"),
        ];
        let read: Vec<i64> = ocr_streams(&index).iter().map(|entry| entry.n).collect();
        assert_eq!(read, [1, 2, 3, 4, 5, 7]);
    }

    /// T5-R10, T5-R58: the cap is spec 14's fixed value; only the harness's override replaces it.
    #[test]
    fn the_cap_is_time_cap_s_unless_the_harness_overrides_it() {
        assert_eq!(stream_cap(None, 887), Duration::from_secs_f64(time_cap_s(887)));
        assert_eq!(stream_cap(None, 10), Duration::from_secs(60));
        assert_eq!(stream_cap(None, 0), Duration::from_secs(60));
        assert_eq!(stream_cap(Some(Duration::from_secs(2)), 887), Duration::from_secs(2));
    }
}
