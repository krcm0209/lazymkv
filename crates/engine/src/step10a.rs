//! Step 10a, the packet MD5 and tuning evidence pass (design parts 5.7, 7.5, 7.6).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use lazymkv_core::audio::dataset::levels_args;
use lazymkv_core::audio::pass::{
    centre, check_main_and_candidates, ffmpeg_args, graph, last_peak, level_folder, main_and_candidates, manifest_text,
    parse_layouts, parse_streamhash, pass_streams, peak_clipped, proxy_check, proxy_json, series_file, Graph, Layouts,
    PassStream, ProxyFile, ProxyPass, ProxyRecord, ProxySource, ProxySourceStream, ProxyTrackFacts, GAIN_FIRST_DB,
    GAIN_REDO_DB,
};
use lazymkv_core::ipc::CANCELLED;
use lazymkv_core::recording::files::FfprobeStreams;
use lazymkv_core::recording::PathKey;
use lazymkv_dev::evidence::DiskTree;
use lazymkv_tools::{layouts_args, proxy_check_args, run_ffmpeg, streamhash_args, with_suffix, RunResult, ToolCtx};
use serde_json::Value;

use crate::staged::{copy_checked, copy_into_job, UnitError};
use crate::steps::{ended_line, not_run_line, run_line};

/// The kind of step 10a's work folder, `<data>\work\levels-<job id>-<title key>\` (T5-R39). Def: T5-17. Use: T5-17.
pub(crate) const WORK_KIND: &str = "levels";
/// The packet MD5 file of either pass, phase 0's name (design part 7.1). Def: T5-17. Use: T5-17, T5-18.
pub(crate) const STREAMHASH: &str = "streamhash.txt";
/// The setting-off pass's capture in the job folder's `<title key>\`, with its three siblings (T5-R57).
/// Def: T5-17. Use: T5-17, T5-18.
pub(crate) const STREAMHASH_RUN: &str = "streamhash-ffmpeg.txt";
/// The compact copy (design part 7.6). Def: T5-17. Use: T5-17, T5-19.
pub(crate) const PROXY: &str = "audio-proxy.mka";
/// The compact copy's record, written beside the proxy and, last, in the level folder (design part 7.5, T5-R13).
/// Def: T5-17. Use: T5-17, T5-18, T5-19.
pub(crate) const PROXY_JSON: &str = "audio-proxy.json";
/// The redo's working folder under the title's work folder, so the redo leaves the first pass's files as they are (C1).
/// Def: T5-17. Use: T5-17.
pub(crate) const REDO: &str = "redo";
/// The job.log sentence after `<key>: ` for a title whose probe lists no audio stream (T5-R66). Def: T5-17. Use: T5-17.
pub(crate) const NO_AUDIO: &str =
    "the file has no audio stream, so step 10a runs no ffmpeg pass and writes an empty streamhash.txt";

/// The proxy as ffmpeg writes it, in the pass's working folder, before the checks keep it.
const PROXY_PARTIAL: &str = "audio-proxy.mka.partial";
/// The proxy check's JSON, written by ffprobe in the pass's working folder.
const PROXY_CHECK: &str = "proxy-check.json";
/// The first line of every job line levels-args gives for an audio-levels run (T5-R38).
const JOB_LINE: &str = "scripts/phase0/job.sh audio-levels.ps1 ";

/// What one title's pass reads and where it writes (design part 5.7 step 10a; T5-R39): the staged file, the job ID
/// and title key, the job folder's `<title key>\`, the staged tuning folder, the work folder, the job's own
/// `keep_tuning_evidence` (ENG-19), and whether Probe's ffprobe-streams.json lists an audio stream (`has_audio`,
/// T5-R66). Def: T5-17. Use: T5-17.
pub(crate) struct PassInputs<'a> {
    pub(crate) raw: &'a Path,
    pub(crate) job_id: &'a str,
    pub(crate) key: &'a str,
    pub(crate) label_dir: &'a Path,
    pub(crate) tuning: &'a Path,
    pub(crate) title_work: &'a Path,
    pub(crate) keep: bool,
    pub(crate) audio: bool,
}

/// How a pass ended (design part 5.7 step 10a): the level folder's name and whether a proxy was kept (C1, call 22),
/// the setting-off pass's `streamhash.txt`, or `NoAudio`: a file whose probe lists no audio stream, for which no pass
/// ran and `streamhash.txt` is empty (T5-R66). Def: T5-17. Use: T5-17.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PassEnd {
    Levels { folder: String, proxy_kept: bool },
    Streamhash,
    NoAudio,
}

/// `<path>: <error>`, the sentence of a file step 10a could not read or write.
fn shown(path: &Path, error: impl std::fmt::Display) -> String {
    format!("{}: {error}", path.display())
}

/// A file of the work folder, written as is (`Failed` on an error).
fn write_work(path: &Path, bytes: impl AsRef<[u8]>) -> Result<(), UnitError> {
    fs::write(path, bytes).map_err(|error| UnitError::Failed(shown(path, error)))
}

/// A file of the work folder as text (`Failed` on an error).
fn read_work(path: &Path) -> Result<String, UnitError> {
    fs::read_to_string(path).map_err(|error| UnitError::Failed(shown(path, error)))
}

/// A file a tool should have written in the work folder: missing is `Failed("<key>: ffmpeg wrote no <name>")`.
fn read_written(key: &str, dir: &Path, name: &str) -> Result<String, UnitError> {
    let path = dir.join(name);
    match fs::read_to_string(&path) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            Err(UnitError::Failed(format!("{key}: ffmpeg wrote no {name}")))
        }
        Err(error) => Err(UnitError::Failed(shown(&path, error))),
    }
}

/// The level-args job lines (T5-R38): the first's main stream and candidates through `main_and_candidates`, and the
/// other job lines as they are, which phase 2 does not run. None when there is no job line or the first does not parse.
fn pass_lines(lines: &[String]) -> Option<((i64, Vec<i64>), Vec<String>)> {
    let mut jobs = lines.iter().filter(|line| line.starts_with(JOB_LINE));
    let first = main_and_candidates(jobs.next()?)?;
    Some((first, jobs.cloned().collect()))
}

/// Which tool a run is, for the sentence of a run that did not end with exit 0.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Ran {
    /// One of the ffmpeg runs.
    Ffmpeg,
    /// ffprobe's proxy check.
    ProxyCheck,
}

/// `<out>.exit.json`'s exit code as text, `none` for a run without one.
fn exit_code_text(out: &Path) -> String {
    let exit = fs::read(with_suffix(out, ".exit.json")).ok();
    let exit = exit.and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
    let code = exit.and_then(|exit| exit["exit_code"].as_i64()).filter(|code| *code != -1);
    code.map_or_else(|| "none".to_string(), |code| code.to_string())
}

/// The last non-empty line of `<out>.stderr`, or "" when there is none.
fn last_stderr_line(out: &Path) -> String {
    let bytes = fs::read(with_suffix(out, ".stderr")).unwrap_or_default();
    let text = String::from_utf8_lossy(&bytes);
    text.lines().map(str::trim).filter(|line| !line.is_empty()).last().unwrap_or("").to_string()
}

/// What every run of the pass shares: the tool context, the title key, the work folder and job.log.
struct Runner<'a, 'c> {
    ctx: &'a ToolCtx<'c>,
    key: &'a str,
    title_work: &'a Path,
    log: &'a dyn Fn(&str),
}

impl Runner<'_, '_> {
    /// One run recorded into `out` with `cwd` as its working folder: the `run <key>/<name>` line, the run through
    /// `run_ffmpeg` (below normal, no limit), then the `ended` line from `<out>.exit.json`, `<name>` being `out`'s
    /// path below the work folder with `/`. A run that could not start, a cancel (`CANCELLED`) and any end but exit 0
    /// are `Failed` with the pass's sentences.
    fn run(&self, ran: Ran, program: &Path, args: &[String], out: &Path, cwd: &Path) -> Result<RunResult, UnitError> {
        let below = out.strip_prefix(self.title_work).unwrap_or(out);
        let below: Vec<String> =
            below.components().map(|part| part.as_os_str().to_string_lossy().into_owned()).collect();
        let name = format!("{}/{}", self.key, below.join("/"));
        let _ = fs::remove_file(with_suffix(out, ".exit.json"));
        (self.log)(&run_line(&name, program, args));
        let result = run_ffmpeg(self.ctx, program, args, out, Some(cwd));
        match &result {
            Ok(_) => (self.log)(&ended_line(&name, out)),
            Err(error) => (self.log)(&not_run_line(&name, error)),
        }
        let key = self.key;
        match result {
            Err(error) => Err(UnitError::Failed(format!("{key}: step 10a's ffmpeg pass could not run: {error}"))),
            Ok(Ok(run)) => Ok(run),
            Ok(Err(why)) if why == CANCELLED => Err(UnitError::Failed(CANCELLED.to_string())),
            Ok(Err(_)) => {
                let code = exit_code_text(out);
                Err(UnitError::Failed(match ran {
                    Ran::ProxyCheck => format!("{key}: proxy check failed: ffprobe exited {code}"),
                    Ran::Ffmpeg => match last_stderr_line(out) {
                        line if line.is_empty() => format!("{key}: step 10a's ffmpeg pass failed (exit {code})"),
                        line => format!("{key}: step 10a's ffmpeg pass failed (exit {code}): {line}"),
                    },
                }))
            }
        }
    }
}

/// Step 10a for one title (design parts 5.7, 7.5, 7.6; T5-R13, T5-R38, T5-R39, T5-R55 to T5-R57): the setting-on pass
/// with its -30 dB redo, or the setting-off streamhash pass, or for a title with no audio stream `no_audio_pass` under
/// either setting (T5-R66), run in the work folder, its files copied into the job folder and then the staged folder
/// through `staged::copy_checked`, the work folder deleted. `log` takes job.log lines. Err: `Failed` for a tool, read
/// or decode failure, `Staging` for a write error on the staging share (T5-R56). Def: T5-17. Use: T5-17.
pub(crate) fn run_pass(
    ctx: &ToolCtx,
    ffmpeg: &Path,
    ffprobe: &Path,
    inputs: &PassInputs,
    log: &dyn Fn(&str),
) -> Result<PassEnd, UnitError> {
    let work = inputs.title_work;
    match fs::remove_dir_all(work) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(UnitError::Failed(shown(work, error))),
        _ => {}
    }
    fs::create_dir_all(work).map_err(|error| UnitError::Failed(shown(work, error)))?;
    let ended = fs::create_dir_all(inputs.tuning)
        .map_err(|error| UnitError::Staging(shown(inputs.tuning, error)))
        .and_then(|()| {
            let runner = Runner { ctx, key: inputs.key, title_work: work, log };
            if !inputs.audio {
                no_audio_pass(inputs)
            } else if inputs.keep {
                levels_pass(&runner, ffmpeg, ffprobe, inputs)
            } else {
                streamhash_pass(&runner, ffmpeg, inputs)
            }
        });
    if let Err(error) = fs::remove_dir_all(work) {
        if error.kind() != io::ErrorKind::NotFound {
            log(&format!("{}: the work folder could not be removed: {error}", work.display()));
        }
    }
    ended
}

/// `<label_dir>\ffprobe-streams.json`, Probe's, typed and as JSON (`Failed` when it cannot be read or parsed).
fn read_probe(label_dir: &Path) -> Result<(FfprobeStreams, Value), UnitError> {
    let path = label_dir.join("ffprobe-streams.json");
    let bytes = fs::read(&path).map_err(|error| UnitError::Failed(shown(&path, error)))?;
    let typed: FfprobeStreams = serde_json::from_slice(&bytes).map_err(|error| UnitError::Failed(shown(&path, error)))?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|error| UnitError::Failed(shown(&path, error)))?;
    Ok((typed, value))
}

/// Whether `<label_dir>\ffprobe-streams.json` lists an audio stream: `pass_streams`' codec_type "audio" test, as
/// levels-args reads it (T5-R66). Err: read_probe's `Failed`. Def: T5-17. Use: T5-17, T5-18.
pub(crate) fn has_audio(label_dir: &Path) -> Result<bool, UnitError> {
    Ok(!pass_streams(&read_probe(label_dir)?.0.streams).is_empty())
}

/// The k-th line of `streamhash.txt` in the working folder `dir` as the MD5 of `audio[k]`'s stream (R1-evidence-9).
fn read_md5(key: &str, dir: &Path, audio: &[PassStream]) -> Result<Vec<(i64, String)>, UnitError> {
    let text = read_written(key, dir, STREAMHASH)?;
    let pairs = parse_streamhash(&text, audio.len()).map_err(|why| UnitError::Failed(format!("{key}: {why}")))?;
    Ok(pairs.into_iter().map(|(k, md5)| (audio[k as usize].index, md5)).collect())
}

/// The setting-off pass (design part 5.7, R6-phase0-54, T5-R57): every audio stream's packets hashed, the capture and
/// `streamhash.txt` copied into the job folder, then `streamhash.txt` into the staged folder, C3's marker.
fn streamhash_pass(runner: &Runner, ffmpeg: &Path, inputs: &PassInputs) -> Result<PassEnd, UnitError> {
    let (key, work) = (inputs.key, inputs.title_work);
    let (probe, _) = read_probe(inputs.label_dir)?;
    let audio = pass_streams(&probe.streams);
    let out = work.join(STREAMHASH_RUN);
    runner.run(Ran::Ffmpeg, ffmpeg, &streamhash_args(inputs.raw, STREAMHASH), &out, work)?;
    read_md5(key, work, &audio)?;
    for suffix in ["", ".stderr", ".timing.jsonl", ".exit.json"] {
        let name = format!("{STREAMHASH_RUN}{suffix}");
        into_job(&work.join(&name), &inputs.label_dir.join(&name))?;
    }
    into_job(&work.join(STREAMHASH), &inputs.label_dir.join(STREAMHASH))?;
    into_staged(&work.join(STREAMHASH), &inputs.tuning.join(STREAMHASH))?;
    Ok(PassEnd::Streamhash)
}

/// A title whose probe lists no audio stream (spec 6.4 step 10a's "every audio stream" with none; T5-R66): no ffmpeg
/// pass, an empty `streamhash.txt` written in the work folder, copied into the job folder, then into the staged folder,
/// C3's marker whatever keep_tuning_evidence is.
fn no_audio_pass(inputs: &PassInputs) -> Result<PassEnd, UnitError> {
    let from = inputs.title_work.join(STREAMHASH);
    write_work(&from, b"")?;
    into_job(&from, &inputs.label_dir.join(STREAMHASH))?;
    into_staged(&from, &inputs.tuning.join(STREAMHASH))?;
    Ok(PassEnd::NoAudio)
}

/// `copy_into_job`, its error `Failed("<to>: <error>")`.
fn into_job(from: &Path, to: &Path) -> Result<(), UnitError> {
    copy_into_job(from, to).map_err(|error| UnitError::Failed(shown(to, error)))
}

/// `copy_checked`, its error `Staging("<to>: <error>")` (T5-R56).
fn into_staged(from: &Path, to: &Path) -> Result<(), UnitError> {
    copy_checked(from, to).map_err(|error| UnitError::Staging(shown(to, error)))
}

/// An argument of command.txt, in double quotes when it holds a space.
fn quoted(arg: &str) -> String {
    if arg.contains(' ') {
        format!("\"{arg}\"")
    } else {
        arg.to_string()
    }
}

/// A modification or start time as `yyyy-MM-dd HH:mm:ss`.
fn seconds_text(ctx: &ToolCtx, unix_ms: u64) -> String {
    ctx.host.local_time(unix_ms).chars().take(19).collect()
}

/// A JSON value as text: a string as written, a number in its decimal form, absent or null as "".
fn text_of(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(text)) => text.clone(),
        Some(other) => other.to_string(),
    }
}

/// How one proxy pass ended: its working folder, each track's last peak and DURATION, and the first clipped track.
struct ProxyRun {
    dir: PathBuf,
    peaks: Vec<String>,
    durations: Vec<f64>,
    clipped: Option<usize>,
}

/// C1's redo in its own working folder `dir`: its graph.txt, ffmpeg's run `pass.txt` with `args`, then the proxy
/// check and the running peaks of every proxy track.
fn redo_run(
    runner: &Runner,
    ffmpeg: &Path,
    ffprobe: &Path,
    dir: &Path,
    graph: &Graph,
    args: &[String],
) -> Result<ProxyRun, UnitError> {
    write_work(&dir.join("graph.txt"), graph.text.as_bytes())?;
    runner.run(Ran::Ffmpeg, ffmpeg, args, &dir.join("pass.txt"), dir)?;
    check_proxy(runner, ffprobe, dir, graph)
}

/// The proxy check in `dir` (R6-phase0-59, C1): ffprobe's stream entries checked against the tracks written, then
/// each track's running peak read and its file deleted.
fn check_proxy(runner: &Runner, ffprobe: &Path, dir: &Path, graph: &Graph) -> Result<ProxyRun, UnitError> {
    let key = runner.key;
    runner.run(Ran::ProxyCheck, ffprobe, &proxy_check_args(PROXY_PARTIAL), &dir.join("proxy-check.txt"), dir)?;
    let check = read_work(&dir.join(PROXY_CHECK))?;
    let durations = proxy_check(&check, &graph.proxy).map_err(|why| UnitError::Failed(format!("{key}: {why}")))?;
    let (mut peaks, mut clipped) = (Vec::new(), None);
    for (k, track) in graph.proxy.iter().enumerate() {
        let path = dir.join(&track.peak);
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(UnitError::Failed(format!(
                    "{key}: proxy check failed: ffmpeg wrote no {} for proxy track {k} (stream {} {})",
                    track.peak, track.stream, track.series
                )))
            }
            Err(error) => return Err(UnitError::Failed(shown(&path, error))),
        };
        let clips = peak_clipped(&text).map_err(|why| UnitError::Failed(format!("{key}: {} {why}", track.peak)))?;
        if clips && clipped.is_none() {
            clipped = Some(k);
        }
        peaks.push(last_peak(&text).unwrap_or_default());
        fs::remove_file(&path).map_err(|error| UnitError::Failed(shown(&path, error)))?;
    }
    Ok(ProxyRun { dir: dir.to_path_buf(), peaks, durations, clipped })
}

/// The setting-on pass (design parts 5.7, 7.6; call 9, C1, call 22; R6-phase0-49 to 61): phase 0's level folder and
/// the FLAC proxy at -18 dB, made again at -30 dB when it clips and kept proxy-less when it clips again; the level
/// files copied into the job folder, then the staged folder, then the proxy, `audio-proxy.json`, and last the level
/// folder's `audio-proxy.json`, C3's marker (T5-R13).
fn levels_pass(runner: &Runner, ffmpeg: &Path, ffprobe: &Path, inputs: &PassInputs) -> Result<PassEnd, UnitError> {
    let (key, work, label_dir) = (inputs.key, inputs.title_work, inputs.label_dir);
    let log = runner.log;
    let (probe, probe_json) = read_probe(label_dir)?;
    let audio = pass_streams(&probe.streams);

    // The streams to measure: levels-args' first job line (T5-R38).
    let job_dir = label_dir.parent().unwrap_or(Path::new(""));
    let tree = DiskTree::open(job_dir, &job_dir.display().to_string())
        .map_err(|error| UnitError::Failed(shown(job_dir, error)))?;
    let lines = levels_args(&tree, &PathKey::root(), key).map_err(|error| {
        let first = error.lines().next().unwrap_or("");
        UnitError::Failed(format!("{key}: step 10a cannot choose the streams to measure: {first}"))
    })?;
    let ((main, cands), rest) =
        pass_lines(&lines).ok_or_else(|| UnitError::Failed(format!("{key}: levels-args gave no audio-levels line")))?;
    for line in rest {
        log(&format!(
            "{key}: step 10a runs one pass (spec 6.4 step 10a); levels-args' dataset-only line is not run in phase 2: \
             {line}"
        ));
    }
    check_main_and_candidates(&probe.streams, main, &cands).map_err(|why| UnitError::Failed(format!("{key}: {why}")))?;

    // ffmpeg's layout table and version.
    let layouts_run = runner.run(Ran::Ffmpeg, ffmpeg, &layouts_args(), &work.join("ffmpeg-layouts.txt"), work)?;
    let layouts: Layouts = parse_layouts(&String::from_utf8_lossy(&layouts_run.stdout))
        .map_err(|why| UnitError::Failed(format!("{key}: {why}")))?;
    let version_args: Vec<String> = ["-hide_banner", "-version"].map(String::from).to_vec();
    let version_run = runner.run(Ran::Ffmpeg, ffmpeg, &version_args, &work.join("ffmpeg-version.txt"), work)?;
    let version = String::from_utf8_lossy(&version_run.stdout).lines().next().unwrap_or("").trim().to_string();

    // The first pass at -18 dB, its files in the work folder.
    let first = graph(&audio, main, &cands, &layouts, true, GAIN_FIRST_DB);
    let probe_path = label_dir.join("ffprobe-streams.json");
    let streams_bytes = fs::read(&probe_path).map_err(|error| UnitError::Failed(shown(&probe_path, error)))?;
    write_work(&work.join("streams.json"), &streams_bytes)?;
    write_work(&work.join("ffprobe.txt"), fs::read(with_suffix(&probe_path, ".stderr")).unwrap_or_default())?;
    let raw_text = inputs.raw.display().to_string();
    let args = ffmpeg_args(&raw_text, &first, true, Some(PROXY_PARTIAL));
    let words: Vec<String> = args.iter().map(|arg| quoted(arg)).collect();
    write_work(&work.join("command.txt"), format!("\"{}\" {}\r\n", ffmpeg.display(), words.join(" ")))?;
    write_work(&work.join("graph.txt"), first.text.as_bytes())?;
    let pass = runner.run(Ran::Ffmpeg, ffmpeg, &args, &work.join("pass.txt"), work)?;
    write_work(&work.join("ffmpeg.txt"), fs::read(work.join("pass.txt.stderr")).unwrap_or_default())?;
    let md5 = read_md5(key, work, &audio)?;

    // The level series and the manifest.
    let raw_name = inputs.raw.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    let mut frames: Vec<(i64, usize)> = Vec::new();
    for series in &first.series {
        let text = read_written(key, work, &series.raw)?;
        let file = series_file(&text, &series.name, &raw_name)
            .map_err(|why| UnitError::Failed(format!("{key}: {why}")))?;
        write_work(&work.join(format!("{}.txt", series.name)), file.as_bytes())?;
        let raw_path = work.join(&series.raw);
        fs::remove_file(&raw_path).map_err(|error| UnitError::Failed(shown(&raw_path, error)))?;
        let count = file.split("\r\n").filter(|line| !line.is_empty()).count().saturating_sub(1);
        if series.name == "main-mix" {
            frames.push((main, count));
        } else if let Some(n) = series.name.strip_prefix("cand-").and_then(|n| n.parse::<i64>().ok()) {
            frames.push((n, count));
        }
    }
    let raw_meta = fs::metadata(inputs.raw).map_err(|error| UnitError::Failed(shown(inputs.raw, error)))?;
    let seconds = (pass.elapsed_ms + 500) / 1000;
    let main_stream = audio.iter().find(|s| s.index == main).cloned().ok_or_else(|| {
        UnitError::Failed(format!("{key}: the file has no stream {main}"))
    })?;
    let cand_streams: Vec<PassStream> =
        cands.iter().filter_map(|n| audio.iter().find(|s| s.index == *n).cloned()).collect();
    let manifest =
        manifest_text(&raw_name, raw_meta.len(), seconds as f64, &main_stream, &cand_streams, &layouts, &frames, &md5);
    write_work(&work.join("manifest.txt"), manifest.as_bytes())?;

    // The proxy check, and C1's redo at -30 dB when a track clips.
    let mut checked = check_proxy(runner, ffprobe, work, &first)?;
    let mut gains_tried = vec![GAIN_FIRST_DB];
    let mut proxy_kept = true;
    if let Some(k) = checked.clipped {
        let track = &first.proxy[k];
        log(&format!(
            "{key}: the audio proxy of stream {} ({}) clips at -18 dB, so the title's proxy is made again at -30 dB",
            track.stream, track.series
        ));
        let redo_dir = work.join(REDO);
        fs::create_dir_all(&redo_dir).map_err(|error| UnitError::Failed(shown(&redo_dir, error)))?;
        let redo = graph(&audio, main, &cands, &layouts, false, GAIN_REDO_DB);
        let redo_args = ffmpeg_args(&raw_text, &redo, false, Some(PROXY_PARTIAL));
        let again = redo_run(runner, ffmpeg, ffprobe, &redo_dir, &redo, &redo_args)?;
        gains_tried.push(GAIN_REDO_DB);
        if let Some(k) = again.clipped {
            let track = &redo.proxy[k];
            log(&format!(
                "{key}: the audio proxy of stream {} ({}) clips at -30 dB too, so the dataset gets no proxy for this \
                 title",
                track.stream, track.series
            ));
            proxy_kept = false;
        }
        checked = again;
    }
    let gain_db = *gains_tried.last().unwrap_or(&GAIN_FIRST_DB);

    // audio-proxy.json (T5-R17).
    let folder = level_folder(main, &cands);
    let partial = checked.dir.join(PROXY_PARTIAL);
    let proxy = if proxy_kept {
        let bytes = fs::metadata(&partial).map_err(|error| UnitError::Failed(shown(&partial, error)))?.len();
        Some(ProxyFile { bytes, tracks: first.proxy.len() })
    } else {
        None
    };
    let stream_objects: Vec<Value> = probe_json["streams"].as_array().cloned().unwrap_or_default();
    let mut tracks = Vec::new();
    for (k, track) in first.proxy.iter().enumerate() {
        let object = stream_objects.iter().find(|s| s["index"].as_i64() == Some(track.stream));
        let object = object.cloned().unwrap_or(Value::Null);
        let tag = |name: &str| text_of(object.get("tags").and_then(|tags| tags.get(name)));
        let get = |name: &str| text_of(object.get(name));
        let layout_guessed = audio
            .iter()
            .find(|s| s.index == track.stream)
            .and_then(|s| centre(s, &layouts).guessed)
            .unwrap_or_default();
        let md5_of = md5.iter().find(|(n, _)| *n == track.stream).map(|(_, h)| h.clone()).unwrap_or_default();
        tracks.push(ProxyTrackFacts {
            track: k,
            source_stream: track.stream,
            series: track.series.clone(),
            duration_s: checked.durations.get(k).copied().unwrap_or(0.0),
            peak_dbfs: checked.peaks.get(k).cloned().unwrap_or_default(),
            source: ProxySourceStream {
                codec: get("codec_name"),
                profile: get("profile"),
                channels: object.get("channels").and_then(Value::as_i64).unwrap_or(0),
                layout: get("channel_layout"),
                layout_guessed,
                language: tag("language"),
                title: tag("title"),
                sample_rate: get("sample_rate"),
                sample_fmt: get("sample_fmt"),
                bits_per_raw_sample: get("bits_per_raw_sample"),
                start_time: get("start_time"),
                md5: md5_of,
            },
        });
    }
    let modified_ms = raw_meta
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |since| since.as_millis() as u64);
    let record = ProxyRecord {
        slug: inputs.job_id.to_string(),
        label: key.to_string(),
        created: seconds_text(runner.ctx, pass.started_unix * 1000),
        source: ProxySource {
            file: raw_name,
            bytes: raw_meta.len(),
            modified: seconds_text(runner.ctx, modified_ms),
            duration_s: text_of(probe_json.get("format").and_then(|format| format.get("duration"))),
            audio_streams: audio.len(),
        },
        pass: ProxyPass { seconds: seconds as i64, ffmpeg: version, levels_folder: folder.clone() },
        gain_db,
        proxy,
        tracks,
        gains_tried,
    };
    write_work(&work.join(PROXY_JSON), proxy_json(&record).as_bytes())?;

    // The level files, in name order: the job folder first, then the staged folder, C3's marker last (T5-R13).
    let mut level: Vec<(String, PathBuf)> = ["command.txt", "ffmpeg.txt", "ffprobe.txt", "graph.txt", "manifest.txt"]
        .iter()
        .map(|name| (name.to_string(), work.join(name)))
        .collect();
    level.push((PROXY_CHECK.to_string(), checked.dir.join(PROXY_CHECK)));
    for series in &first.series {
        let name = format!("{}.txt", series.name);
        level.push((name.clone(), work.join(name)));
    }
    level.push(("streams.json".to_string(), work.join("streams.json")));
    level.push((STREAMHASH.to_string(), work.join(STREAMHASH)));
    level.sort();
    let job_levels = label_dir.join(&folder);
    fs::create_dir_all(&job_levels).map_err(|error| UnitError::Failed(shown(&job_levels, error)))?;
    for (name, from) in &level {
        into_job(from, &job_levels.join(name))?;
    }
    let staged_levels = inputs.tuning.join(&folder);
    fs::create_dir_all(&staged_levels).map_err(|error| UnitError::Staging(shown(&staged_levels, error)))?;
    for (name, from) in &level {
        into_staged(from, &staged_levels.join(name))?;
    }
    if proxy_kept {
        into_staged(&partial, &inputs.tuning.join(PROXY))?;
    }
    into_staged(&work.join(PROXY_JSON), &inputs.tuning.join(PROXY_JSON))?;
    into_staged(&work.join(PROXY_JSON), &staged_levels.join(PROXY_JSON))?;
    Ok(PassEnd::Levels { folder, proxy_kept })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_first_levels_args_line_is_run_and_the_rest_are_named() {
        let lines: Vec<String> = [
            "job-1/t00-00800: 4 audio streams (from ffprobe-streams.json); the original language is eng",
            "  stream 1: DTS-HD MA 6 ch lang=eng",
            "scripts/phase0/job.sh audio-levels.ps1 audio-levels-job-1-t00-00800-1 -Slug job-1 -Label t00-00800 -Main 1 -Cand 2",
            "scripts/phase0/wait.sh audio-levels-job-1-t00-00800-1",
            "scripts/phase0/job.sh audio-levels.ps1 audio-levels-job-1-t00-00800-3 -Slug job-1 -Label t00-00800 -Main 3 -Cand 4",
            "scripts/phase0/wait.sh audio-levels-job-1-t00-00800-3",
        ]
        .map(String::from)
        .to_vec();
        assert_eq!(pass_lines(&lines), Some(((1, vec![2]), vec![lines[4].clone()])));
        assert_eq!(pass_lines(&lines[..2]), None);
    }
}
