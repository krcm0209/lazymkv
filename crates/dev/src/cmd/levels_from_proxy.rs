//! `lazymkv-dev levels-from-proxy`: rebuilds an audio-levels folder from a title's compact audio copy, the proxy
//! that audio-levels.ps1 writes to the tuning dataset (`<slug>\<label>\audio-proxy.mka`, with audio-proxy.json
//! beside it), without the disc or the rip. The proxy holds every audio stream of the ripped title as one mono FLAC
//! track (its mixdown, series "mix"), a second track with the centre channel ("fc") for a layout with one, and a
//! side track ("side", not read here), stored gain_db below the signal the level chain measures; every track carries
//! the tags LAZYMKV_SOURCE_STREAM (ffprobe's stream number in the rip) and LAZYMKV_SERIES. Each track is read back in
//! float, the stored gain is undone, the samples are converted to the sample format the source stream decodes to
//! (sample_fmt in audio-proxy.json), and the chain of audio-levels.ps1 runs over every track in one ffmpeg pass. The
//! files written are those audio-levels.ps1 writes (manifest.txt, main-mix.txt, main-fc.txt, cand-<n>.txt), plus
//! streams.json when audio-proxy.json records every audio stream of the rip, ffmpeg.txt and proxy.txt.
//! Py: proxy_levels.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use lazymkv_core::audio::levels::{parse_proxy, AudioProxy, MANIFEST_LINE, PROXY_FORMAT};
use lazymkv_core::recording::{
    decode_text, py_float_value, py_g, py_int, py_int_value, py_is_space, py_repr, py_repr_str, py_str, py_strip,
    py_truthy, Encoding, Errors, Ordered,
};
use serde_json::{Map, Value};

use crate::evidence::{os_error_text, pathlib_str as py_path_str, Ctx};
use crate::pyjson::{write_level_file, write_manifest, write_streams_json};
use crate::runner::Output;

/// The level chain of audio-levels.ps1 after a track's reader: 8 kHz, the 300-3400 Hz band, 160-sample (20 ms)
/// frames, each frame's RMS level, printed by ametadata into the file named next. Py: TAIL.
const TAIL: &str = concat!(
    "aresample=8000,highpass=f=300,lowpass=f=3400,asetnsamples=n=160:p=0,",
    "astats=metadata=1:reset=1:measure_perchannel=none:measure_overall=RMS_level,",
    "ametadata=print:key=lavfi.astats.Overall.RMS_level:file=",
);
/// The file beside the proxy that describes it. Py: load_proxy_json.
const PROXY_JSON: &str = "audio-proxy.json";
/// After the path of a missing audio-proxy.json. Py: load_proxy_json.
const MISSING: &str = "is missing: without it the proxy is incomplete (audio-levels.ps1 writes it last)";
/// Py: build.
const TWICE: &str = "the main stream is also a candidate, or a candidate is listed twice";
/// A level file's header up to its series name. Py: build's header.
const LEVEL_HEADER: &str = "# lazymkv audio levels v2: frame_ms=20 band_hz=300-3400 unit=dB floor=-100 series=";
/// streams.json's first value. Py: build.
const FROM_PROXY: &str = "audio-proxy.json (audio streams only)";
/// The usage line (exit 1, as the Python's sys.exit). Py: main, with the command's new name.
const USAGE: &str =
    "usage: lazymkv-dev levels-from-proxy <audio-proxy.mka> <new folder> --main <stream> [--cand <stream>[,<stream>...]]";
/// Printed after the frames when no streams.json was written. Py: main, with lazymkv-dev retune for the harness's
/// old name.
const NO_STREAMS_NOTE: &str = concat!(
    "note: audio-proxy.json does not record every audio stream of the rip, so no streams.json was written: ",
    "lazymkv-dev retune counts none of this folder's candidates (copy the rip's ffprobe-streams.json there as ",
    "streams.json)",
);

/// Writes a level folder for `main` and `cands` from the proxy `mka`: ffprobe for the proxy's tracks, one ffmpeg
/// pass (cwd `out`) through ctx.runner, level files by pyjson::level_file_text, manifest.txt, streams.json when
/// proxy_streams gives one, proxy.txt as "levels rebuilt from <mka_shown>" (the path as given, not resolved:
/// spec section 4). `info` None reads audio-proxy.json beside `mka`. Returns frames per series in write order
/// (main-mix, main-fc, cand-<n> ...). Err with the Python's SystemExit texts. Relative `mka` and `out` are taken
/// against ctx.cwd. Py: proxy_levels.build.
pub fn build(
    ctx: &Ctx,
    mka: &Path,
    mka_shown: &str,
    out: &Path,
    main: i64,
    cands: &[i64],
    info: Option<AudioProxy>,
) -> Result<Ordered<usize>, String> {
    let mka = resolve_like_python(&ctx.cwd.join(mka));
    let out_shown = py_path_str(&out.to_string_lossy());
    let out = ctx.cwd.join(out);
    let info = match info {
        Some(info) => info,
        None => load_proxy_json(&mka)?,
    };
    let gain = match info.encoding.get("gain_db") {
        Some(value) => py_float_value(value)?,
        None => 0.0,
    };
    let tracks = proxy_tracks(ctx, &mka)?;
    if out.exists() {
        return Err(format!("{out_shown} already exists"));
    }
    let measured: Vec<i64> = std::iter::once(main).chain(cands.iter().copied()).collect();
    let mix = |n: i64| (n, "mix".to_string());
    for &n in &measured {
        if !tracks.contains_key(&mix(n)) {
            let held: BTreeSet<i64> = tracks.keys().map(|(stream, _)| *stream).collect();
            let held: Vec<String> = held.iter().map(|stream| stream.to_string()).collect();
            return Err(format!("the proxy has no track of stream {n} (it holds [{}])", held.join(", ")));
        }
    }
    let distinct: BTreeSet<i64> = cands.iter().copied().collect();
    if cands.contains(&main) || distinct.len() != cands.len() {
        return Err(TWICE.to_string());
    }
    std::fs::create_dir_all(&out).map_err(|e| format!("{out_shown}: {e}"))?;
    let fc = (main, "fc".to_string());
    let has_fc = tracks.contains_key(&fc);
    // (series, source stream, proxy track) in the Python's order: main-mix, main-fc, cand-<n> ...
    let mut series: Vec<(String, i64, &str)> = vec![("main-mix".to_string(), main, tracks[&mix(main)].as_str())];
    if has_fc {
        series.push(("main-fc".to_string(), main, tracks[&fc].as_str()));
    }
    for &n in cands {
        series.push((format!("cand-{n}"), n, tracks[&mix(n)].as_str()));
    }
    let mut chains = Vec::new();
    for (i, (name, stream, track)) in series.iter().enumerate() {
        chains.push(format!("[0:{track}]{}{TAIL}raw-{name}.txt[o{i}]", reader(&info, *stream, gain)?));
    }
    let mut args: Vec<String> = ["-v", "error", "-nostdin", "-i"].iter().map(|arg| arg.to_string()).collect();
    args.push(mka.to_string_lossy().into_owned());
    args.push("-filter_complex".to_string());
    args.push(chains.join(";"));
    for i in 0..series.len() {
        args.extend(["-map".to_string(), format!("[o{i}]"), "-f".to_string(), "null".to_string(), format!("o{i}")]);
    }
    let run = run_tool(ctx, "ffmpeg", &args, Some(out.as_path()))?;
    write_file(&out.join("ffmpeg.txt"), &format!("{}{}", run.stdout, run.stderr))?;
    if run.status != 0 {
        return Err(format!("ffmpeg failed (exit {}): {}", run.status, first_chars(py_strip(&run.stderr), 300)));
    }
    let proxy_name = word_str(&mka.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default());
    let mut frames: Ordered<usize> = Ordered::new();
    for (name, _, _) in &series {
        let raw_path = out.join(format!("raw-{name}.txt"));
        let bytes = std::fs::read(&raw_path).map_err(|e| format!("{}: {e}", raw_path.display()))?;
        let raw = decode_text(&bytes, Encoding::Utf8, Errors::Strict)?;
        let header = format!("{LEVEL_HEADER}{name} file={proxy_name}");
        let count = write_level_file(&out.join(format!("{name}.txt")), &raw, &header)?;
        frames.insert(name.clone(), count);
        std::fs::remove_file(&raw_path).map_err(|e| format!("{}: {e}", raw_path.display()))?;
    }
    let src = &info.source;
    let mut lines = vec![
        format!("{MANIFEST_LINE} (experiment 13)"),
        format!(
            "file={} bytes={} seconds=0 frame_ms=20 band_hz=300-3400 from_proxy={proxy_name}",
            word(src.get("file")),
            src.get("bytes").map_or_else(|| "0".to_string(), py_str)
        ),
    ];
    for &n in &measured {
        let s = facts_of(&info, n);
        let (role, fc_word, key) = if n == main {
            (format!("main={n}"), format!(" fc={}", if has_fc { "yes" } else { "no" }), "main-mix".to_string())
        } else {
            (format!("cand={n}"), String::new(), format!("cand-{n}"))
        };
        let count = frames.get(&key).copied().unwrap_or(0);
        let guess = match s.get("layout_guessed") {
            Some(value) if py_truthy(value) => format!(" layout_guessed={}", py_str(value)),
            _ => String::new(),
        };
        let md5 = match s.get("md5") {
            Some(value) if py_truthy(value) => py_str(value),
            _ => format!("unknown-{n}"),
        };
        lines.push(format!(
            "{role} codec={} profile={} channels={} layout={}{guess} language={}{fc_word} frames={count} md5={md5}",
            word(s.get("codec")),
            word(s.get("profile")),
            s.get("channels").map_or_else(|| "0".to_string(), py_str),
            word(s.get("layout")),
            word(s.get("language")),
        ));
    }
    write_manifest(&out.join("manifest.txt"), &lines).map_err(|e| format!("manifest.txt: {e}"))?;
    if let Some(streams) = proxy_streams(&info) {
        let mut doc = Map::new();
        doc.insert("from_proxy".to_string(), Value::from(FROM_PROXY));
        doc.insert("streams".to_string(), Value::Array(streams));
        write_streams_json(&out.join("streams.json"), &Value::Object(doc)).map_err(|e| format!("streams.json: {e}"))?;
    }
    write_file(&out.join("proxy.txt"), &format!("levels rebuilt from {mka_shown}\n"))?;
    Ok(frames)
}

/// streams.json's "streams" from audio-proxy.json, None unless it records every audio stream with codec, channels
/// and language: the fields ffprobe_audio and position_veto read (index, codec_type, codec_name, channels, profile,
/// channel_layout, the language and title tags), in stream order; an empty profile, layout or tag is left out, as
/// ffprobe leaves it out. A stream's facts are those of its first track (setdefault). Py: proxy_streams.
pub fn proxy_streams(info: &AudioProxy) -> Option<Vec<Value>> {
    let empty = Map::new();
    let mut facts: Vec<(i64, &Map<String, Value>)> = Vec::new();
    for track in &info.tracks {
        let source = track.source.as_ref().unwrap_or(&empty);
        let n = track.source_stream.as_i64()?;
        if !["codec", "channels", "language"].iter().all(|key| source.contains_key(*key)) {
            return None;
        }
        if !facts.iter().any(|(m, _)| *m == n) {
            facts.push((n, source));
        }
    }
    let count = info.source.get("audio_streams");
    if facts.is_empty() || !count.is_some_and(|value| py_eq_int(value, facts.len() as i64)) {
        return None;
    }
    facts.sort_by_key(|(n, _)| *n);
    let mut out = Vec::new();
    for (n, s) in facts {
        let mut stream = Map::new();
        stream.insert("index".to_string(), Value::from(n));
        stream.insert("codec_type".to_string(), Value::from("audio"));
        stream.insert("codec_name".to_string(), s["codec"].clone());
        let channels = if py_truthy(&s["channels"]) { py_int_value(&s["channels"]).ok()? } else { 0 };
        stream.insert("channels".to_string(), Value::from(channels));
        if let Some(profile) = s.get("profile").filter(|value| py_truthy(value)) {
            stream.insert("profile".to_string(), profile.clone());
        }
        if let Some(layout) = s.get("layout").filter(|value| py_truthy(value)) {
            stream.insert("channel_layout".to_string(), layout.clone());
        }
        let mut tags = Map::new();
        for key in ["language", "title"] {
            if let Some(value) = s.get(key).filter(|value| py_truthy(value)) {
                tags.insert(key.to_string(), value.clone());
            }
        }
        if !tags.is_empty() {
            stream.insert("tags".to_string(), Value::Object(tags));
        }
        out.push(Value::Object(stream));
    }
    Some(out)
}

/// The command, registered in COMMANDS as "levels-from-proxy". The words are read exactly as the Python's main
/// reads them: `<mka> <out> --main N`, then `--cand LIST` only as the fifth and sixth words; any other word after
/// them is ignored. Every failure prints one line to `err` and exits 1. Py: proxy_levels.main.
pub fn run(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    if args.len() < 4 || args[2] != "--main" {
        let _ = writeln!(err, "{USAGE}");
        return 1;
    }
    let Some(main) = py_int(&args[3]) else {
        let _ = writeln!(err, "{}", not_an_int(&args[3]));
        return 1;
    };
    let mut cands = Vec::new();
    if args.len() >= 6 && args[4] == "--cand" {
        for piece in args[5].split(',').filter(|piece| !piece.is_empty()) {
            let Some(n) = py_int(piece) else {
                let _ = writeln!(err, "{}", not_an_int(piece));
                return 1;
            };
            cands.push(n);
        }
    }
    match build(ctx, Path::new(&args[0]), &args[0], Path::new(&args[1]), main, &cands, None) {
        Ok(frames) => {
            let done: Vec<String> = frames.iter().map(|(name, count)| format!("{name} {count} frames")).collect();
            let _ = writeln!(out, "levels from {}: {} -> {}", args[0], done.join(", "), args[1]);
            if !ctx.cwd.join(&args[1]).join("streams.json").is_file() {
                let _ = writeln!(out, "{NO_STREAMS_NOTE}");
            }
            0
        }
        Err(message) => {
            let _ = writeln!(err, "{message}");
            1
        }
    }
}

/// int()'s ValueError text, printed where the Python crashed on a stream number.
fn not_an_int(text: &str) -> String {
    format!("invalid literal for int() with base 10: {}", py_repr_str(text))
}

/// audio-proxy.json beside the proxy, read as utf-8-sig and checked for its format. Py: load_proxy_json.
fn load_proxy_json(mka: &Path) -> Result<AudioProxy, String> {
    let path = mka.with_file_name(PROXY_JSON);
    let shown = path.display().to_string();
    if !path.is_file() {
        return Err(format!("{shown} {MISSING}"));
    }
    let bytes = std::fs::read(&path).map_err(|e| format!("{shown}: {e}"))?;
    let text = decode_text(&bytes, Encoding::Utf8Sig, Errors::Strict)?;
    let data = parse_proxy(&text)?;
    let format = data.format.clone().unwrap_or(Value::Null);
    if format.as_str() != Some(PROXY_FORMAT) {
        return Err(format!("{shown}: format is {}, not {}", py_repr(&format), py_repr_str(PROXY_FORMAT)));
    }
    Ok(data)
}

/// {(source stream, series): the proxy's track number as a filter graph names it} from the proxy's own tags (tag
/// names upper-cased, the series "mix" when not tagged), read by ffprobe. Py: proxy_tracks.
fn proxy_tracks(ctx: &Ctx, mka: &Path) -> Result<BTreeMap<(i64, String), String>, String> {
    let shown = mka.to_string_lossy().into_owned();
    let mut args: Vec<String> = ["-v", "error", "-show_entries", "stream=index,codec_type:stream_tags", "-of", "json"]
        .iter()
        .map(|arg| arg.to_string())
        .collect();
    args.push(shown.clone());
    let probe = run_tool(ctx, "ffprobe", &args, None)?;
    if probe.status != 0 {
        return Err(format!("ffprobe cannot read {shown}: {}", first_chars(py_strip(&probe.stderr), 300)));
    }
    let doc: Value = serde_json::from_str(&probe.stdout).map_err(|e| format!("ffprobe's output: {e}"))?;
    let Value::Object(doc) = doc else {
        return Err("ffprobe's output is not a JSON object".to_string());
    };
    let mut out = BTreeMap::new();
    for stream in doc.get("streams").and_then(Value::as_array).into_iter().flatten() {
        let Some(stream) = stream.as_object() else { continue };
        let mut tags: BTreeMap<String, &Value> = BTreeMap::new();
        if let Some(Value::Object(given)) = stream.get("tags") {
            for (key, value) in given {
                tags.insert(key.to_uppercase(), value);
            }
        }
        if stream.get("codec_type").and_then(Value::as_str) != Some("audio") {
            continue;
        }
        let Some(source) = tags.get("LAZYMKV_SOURCE_STREAM") else { continue };
        let source = py_int_value(source)?;
        let series = tags.get("LAZYMKV_SERIES").map_or_else(|| "mix".to_string(), |value| py_str(value));
        let index = stream.get("index").ok_or_else(|| "KeyError: 'index'".to_string())?;
        out.insert((source, series), py_str(index));
    }
    Ok(out)
}

/// The start of a track's chain: float mono, the stored gain undone, then the sample format the source stream
/// decodes to (sample_fmt in audio-proxy.json, "fltp" when it gives none), because ffmpeg's filters compute in that
/// format and 16-bit arithmetic reads quiet frames lower. Py: build's reader.
fn reader(info: &AudioProxy, stream: i64, gain: f64) -> Result<String, String> {
    let facts = facts_of(info, stream);
    let fmt = match facts.get("sample_fmt") {
        Some(value) if py_truthy(value) => match value.as_str() {
            Some(text) => text.to_string(),
            None => return Err("TypeError: expected string or bytes-like object".to_string()),
        },
        _ => "fltp".to_string(),
    };
    if fmt.is_empty() || !fmt.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()) {
        return Err(format!("audio-proxy.json gives stream {stream} the sample format {}", py_repr_str(&fmt)));
    }
    Ok(format!("aformat=sample_fmts=fltp:channel_layouts=mono{},aformat=sample_fmts={fmt},", volume_undo(gain)))
}

/// ",volume=<-gain>dB" with -gain spelt by format(x, "g"), or nothing for a gain of 0. Py: `undo` in build's reader.
fn volume_undo(gain: f64) -> String {
    if gain != 0.0 {
        format!(",volume={}dB", py_g(-gain))
    } else {
        String::new()
    }
}

/// What audio-proxy.json says about stream `n`: the `source` of the last track of that stream (build's dict
/// comprehension keeps the last), {} when there is none. Py: build's `source.get(n, {})`.
fn facts_of(info: &AudioProxy, n: i64) -> Map<String, Value> {
    info.tracks
        .iter()
        .rev()
        .find(|track| py_eq_int(&track.source_stream, n))
        .and_then(|track| track.source.clone())
        .unwrap_or_default()
}

/// Python's `==` between a JSON value and an int: numbers by value, true and false as 1 and 0, nothing else.
fn py_eq_int(value: &Value, n: i64) -> bool {
    match value {
        Value::Number(number) => {
            if let Some(i) = number.as_i64() {
                i == n
            } else if let Some(u) = number.as_u64() {
                i64::try_from(u).map_or(false, |u| u == n)
            } else {
                number.as_f64().map_or(false, |f| f == n as f64)
            }
        }
        Value::Bool(flag) => i64::from(*flag) == n,
        _ => false,
    }
}

/// A manifest value: "-" for None, null or "", otherwise str(value) with every run of whitespace as one "_".
/// Py: word.
fn word(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => word_str(""),
        Some(value) => word_str(&py_str(value)),
    }
}

/// word() of a str: re.sub(r"\s+", "_", text), "-" for "" (re's \s on str is str.isspace).
fn word_str(text: &str) -> String {
    if text.is_empty() {
        return "-".to_string();
    }
    let mut out = String::new();
    let mut in_space = false;
    for c in text.chars() {
        if py_is_space(c) {
            if !in_space {
                out.push('_');
            }
            in_space = true;
        } else {
            out.push(c);
            in_space = false;
        }
    }
    out
}

/// The first `n` characters of `text`. Py: text[:n].
fn first_chars(text: &str, n: usize) -> String {
    text.chars().take(n).collect()
}

/// ctx.runner.run, with Python's OSError text when the program cannot be run (the Python crashed there).
fn run_tool(ctx: &Ctx, program: &str, args: &[String], cwd: Option<&Path>) -> Result<Output, String> {
    ctx.runner.run(program, args, cwd).map_err(|e| os_error_text(&e, program))
}

/// Writes a text file as Path.write_text(text, encoding="utf-8") does on the Mac: UTF-8, no newline translation.
fn write_file(path: &Path, text: &str) -> Result<(), String> {
    std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Python's Path.resolve() (strict=False) of an absolute path: the longest part that exists, resolved through its
/// symbolic links, then the rest with "." dropped and ".." taking a name off. Windows' verbatim prefix (\\?\C:\) is
/// dropped so ffmpeg gets the path Python would have given it.
fn resolve_like_python(path: &Path) -> PathBuf {
    let parts: Vec<Component> = path.components().collect();
    for cut in (1..=parts.len()).rev() {
        let head: PathBuf = parts[..cut].iter().collect();
        if let Ok(mut resolved) = std::fs::canonicalize(&head) {
            for part in &parts[cut..] {
                match part {
                    Component::CurDir => {}
                    Component::ParentDir => {
                        resolved.pop();
                    }
                    other => resolved.push(other.as_os_str()),
                }
            }
            return strip_verbatim(resolved);
        }
    }
    path.to_path_buf()
}

/// `\\?\C:\x` as `C:\x`; any other path unchanged.
fn strip_verbatim(path: PathBuf) -> PathBuf {
    let plain = path
        .to_str()
        .and_then(|text| text.strip_prefix(r"\\?\"))
        .filter(|rest| rest.as_bytes().get(1) == Some(&b':'))
        .map(PathBuf::from);
    plain.unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::path::Path;
    use std::rc::Rc;

    use lazymkv_core::audio::levels::{AudioProxy, PROXY_FORMAT};
    use lazymkv_core::audio::reference::{ffprobe_audio, read_streams, AudioStream};
    use lazymkv_core::audio::rule::position_veto;
    use lazymkv_core::recording::files::FfprobeStream;
    use lazymkv_core::recording::{py_fixed, Ordered, PathKey};
    use lazymkv_core::testsupport::programme;
    use serde_json::{json, Value};

    use super::*;
    use crate::evidence::{skip, Ctx, DiskTree};
    use crate::pyjson::{dumps, Style, KEY};
    use crate::testutil::Scratch as TempDir;
    use crate::runner::{Call, FakeRunner, Output, RealRunner, Runner};

    /// A file's text.
    fn read(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    /// Owned words of a command line.
    fn words(list: &[&str]) -> Vec<String> {
        list.iter().map(|word| word.to_string()).collect()
    }

    /// Frames per series, in order.
    fn ordered(pairs: &[(&str, usize)]) -> Ordered<usize> {
        Ordered(pairs.iter().map(|(name, n)| (name.to_string(), *n)).collect())
    }

    /// A refusal as build returns it.
    fn refused(text: &str) -> Result<Ordered<usize>, String> {
        Err(text.to_string())
    }

    /// Every call the fake programs received, in order.
    type Log = Rc<RefCell<Vec<Call>>>;

    /// A program's answer: exit 0, nothing printed.
    fn ok() -> Output {
        Output { status: 0, stdout: String::new(), stderr: String::new() }
    }

    /// What `ffprobe -v error -show_entries stream=index,codec_type:stream_tags -of json` prints for audio tracks
    /// given as (track number, source stream, series).
    fn probe_output(tracks: &[(i64, i64, &str)]) -> Output {
        let streams: Vec<Value> = tracks
            .iter()
            .map(|&(index, n, series)| {
                json!({"index": index, "codec_type": "audio",
                       "tags": {"LAZYMKV_SOURCE_STREAM": n.to_string(), "LAZYMKV_SERIES": series}})
            })
            .collect();
        Output { status: 0, stdout: json!({ "streams": streams }).to_string(), stderr: String::new() }
    }

    /// The names a filter graph's ametadata writes to, raw-<name>.txt.
    /// Py: re.findall(r"file=raw-([a-z0-9-]+)\.txt", graph).
    fn raw_names(graph: &str) -> Vec<String> {
        graph
            .split("file=raw-")
            .skip(1)
            .filter_map(|rest| {
                let name: String =
                    rest.chars().take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-').collect();
                (!name.is_empty() && rest[name.len()..].starts_with(".txt")).then_some(name)
            })
            .collect()
    }

    /// A FakeRunner for the two programs: ffprobe answers `probe`; ffmpeg writes `raw` into every raw-<name>.txt of
    /// its filter graph, in its working folder, and answers `ffmpeg`. Every call is logged in `log`.
    /// Py: ProxyStreams' fake ffmpeg and its patched proxy_tracks.
    fn fake(probe: Output, raw: String, ffmpeg: Output, log: &Log) -> FakeRunner {
        let log = Rc::clone(log);
        FakeRunner::new(Box::new(
            move |program: &str, args: &[String], cwd: Option<&Path>| -> std::io::Result<Output> {
                log.borrow_mut().push(Call {
                    program: program.to_string(),
                    args: args.to_vec(),
                    cwd: cwd.map(Path::to_path_buf),
                });
                if program == "ffprobe" {
                    return Ok(probe.clone());
                }
                let at = args.iter().position(|arg| arg == "-filter_complex").expect("ffmpeg is given a filter graph");
                let dir = cwd.expect("ffmpeg runs in the new folder");
                for name in raw_names(&args[at + 1]) {
                    std::fs::write(dir.join(format!("raw-{name}.txt")), &raw)?;
                }
                Ok(ffmpeg.clone())
            },
        ))
    }

    /// A test context in `dir` whose ffprobe and ffmpeg are `runner`.
    fn ctx_with(dir: &Path, runner: FakeRunner) -> Ctx {
        let mut ctx = Ctx::for_test(dir);
        ctx.runner = Box::new(runner);
        ctx
    }

    /// build from `ctx`'s folder with the relative proxy path "audio-proxy.mka" and `info` (None: read the file).
    fn build_with(ctx: &Ctx, out: &str, main: i64, cands: &[i64], info: Option<Value>) -> Result<Ordered<usize>, String> {
        let info: Option<AudioProxy> = info.map(|value| serde_json::from_value(value).unwrap());
        build(ctx, Path::new("audio-proxy.mka"), "audio-proxy.mka", Path::new(out), main, cands, info)
    }

    /// The command's exit code, stdout and stderr for `list`.
    fn call(ctx: &Ctx, list: &[&str]) -> (i32, String, String) {
        crate::testutil::call(run, ctx, list)
    }

    /// The same, as expected.
    fn printed(code: i32, out: &str, err: &str) -> (i32, String, String) {
        (code, out.to_string(), err.to_string())
    }

    /// A stream of the rip as ProxyStreams.info takes it: (index, codec, profile, channels, layout, language).
    type Facts = (i64, &'static str, &'static str, i64, &'static str, &'static str);

    /// Two DTS streams in English (the reference, stream 2, is the second) and an AC-3 stream without a language.
    /// Py: ProxyStreams.VETO.
    const VETO: [Facts; 3] = [
        (1, "dts", "DTS", 6, "5.1(side)", "eng"),
        (2, "dts", "DTS-HD MA", 6, "5.1(side)", "eng"),
        (3, "ac3", "", 2, "stereo", ""),
    ];

    /// audio-proxy.json for `streams`: a mix track each, and an fc track for more than two channels; `count`
    /// replaces source.audio_streams. Py: ProxyStreams.info.
    fn info_value(streams: &[Facts], count: Option<usize>) -> Value {
        let mut tracks: Vec<Value> = Vec::new();
        for &(n, codec, profile, ch, layout, lang) in streams {
            let source = json!({"codec": codec, "profile": profile, "channels": ch, "layout": layout,
                                "layout_guessed": "", "language": lang, "title": format!("t{n}"),
                                "sample_fmt": "fltp", "md5": format!("m{n}")});
            let series: &[&str] = if ch > 2 { &["mix", "fc"] } else { &["mix"] };
            for s in series {
                let track = tracks.len();
                tracks.push(json!({"track": track, "source_stream": n, "series": s, "source": source.clone()}));
            }
        }
        json!({"format": PROXY_FORMAT,
               "source": {"file": "x.mkv", "bytes": 1, "audio_streams": count.unwrap_or(streams.len())},
               "encoding": {"gain_db": -18}, "tracks": tracks})
    }

    /// The same, parsed. Py: ProxyStreams.info.
    fn info(streams: &[Facts], count: Option<usize>) -> AudioProxy {
        serde_json::from_value(info_value(streams, count)).unwrap()
    }

    /// streams.json as the Python wrote it for VETO.
    const VETO_STREAMS_JSON: &str = concat!(
        "{\n",
        " \"from_proxy\": \"audio-proxy.json (audio streams only)\",\n",
        " \"streams\": [\n",
        "  {\n",
        "   \"index\": 1,\n",
        "   \"codec_type\": \"audio\",\n",
        "   \"codec_name\": \"dts\",\n",
        "   \"channels\": 6,\n",
        "   \"profile\": \"DTS\",\n",
        "   \"channel_layout\": \"5.1(side)\",\n",
        "   \"tags\": {\n",
        "    \"language\": \"eng\",\n",
        "    \"title\": \"t1\"\n",
        "   }\n",
        "  },\n",
        "  {\n",
        "   \"index\": 2,\n",
        "   \"codec_type\": \"audio\",\n",
        "   \"codec_name\": \"dts\",\n",
        "   \"channels\": 6,\n",
        "   \"profile\": \"DTS-HD MA\",\n",
        "   \"channel_layout\": \"5.1(side)\",\n",
        "   \"tags\": {\n",
        "    \"language\": \"eng\",\n",
        "    \"title\": \"t2\"\n",
        "   }\n",
        "  },\n",
        "  {\n",
        "   \"index\": 3,\n",
        "   \"codec_type\": \"audio\",\n",
        "   \"codec_name\": \"ac3\",\n",
        "   \"channels\": 2,\n",
        "   \"channel_layout\": \"stereo\",\n",
        "   \"tags\": {\n",
        "    \"title\": \"t3\"\n",
        "   }\n",
        "  }\n",
        " ]\n",
        "}\n",
    );

    /// manifest.txt as the Python wrote it for VETO, main 2, candidate 3.
    const VETO_MANIFEST: &str = concat!(
        "# lazymkv audio-levels v2 (experiment 13)\n",
        "file=x.mkv bytes=1 seconds=0 frame_ms=20 band_hz=300-3400 from_proxy=audio-proxy.mka\n",
        "main=2 codec=dts profile=DTS-HD_MA channels=6 layout=5.1(side) language=eng fc=yes frames=200 md5=m2\n",
        "cand=3 codec=ac3 profile=- channels=2 layout=stereo language=- frames=200 md5=m3\n",
    );

    /// ffprobe's answer in the mixed run: a video stream, a lower-case tag name, a track without a series (so
    /// "mix"), a side track, an audio stream without tags, a subtitle stream, and stream 5 as track 7.
    const MIXED_PROBE: &str = r#"{"streams": [
        {"index": 0, "codec_type": "video"},
        {"index": 1, "codec_type": "audio", "tags": {"lazymkv_source_stream": "1", "LAZYMKV_SERIES": "mix"}},
        {"index": 2, "codec_type": "audio", "tags": {"LAZYMKV_SOURCE_STREAM": "1", "LAZYMKV_SERIES": "fc"}},
        {"index": 3, "codec_type": "audio", "tags": {"LAZYMKV_SOURCE_STREAM": "2"}},
        {"index": 4, "codec_type": "audio", "tags": {"LAZYMKV_SOURCE_STREAM": "3", "LAZYMKV_SERIES": "side"}},
        {"index": 5, "codec_type": "audio"},
        {"index": 6, "codec_type": "subtitle", "tags": {"LAZYMKV_SOURCE_STREAM": "4"}},
        {"index": 7, "codec_type": "audio", "tags": {"LAZYMKV_SOURCE_STREAM": "5"}}
    ]}"#;

    fn mixed_probe() -> Output {
        Output { status: 0, stdout: MIXED_PROBE.to_string(), stderr: String::new() }
    }

    /// audio-proxy.json of the mixed run: a lossless main stream with a centre track (its sample format on both
    /// tracks), a candidate whose layout was guessed and whose sample format and md5 are empty, and a candidate
    /// without channels, layout, language or md5, whose profile holds two spaces.
    const MIXED_INFO: &str = r#"{"format": "lazymkv audio proxy v1",
        "source": {"file": "a film.mkv", "bytes": 98765, "audio_streams": 3},
        "encoding": {"gain_db": -18, "read": "lazymkv-dev levels-from-proxy"},
        "tracks": [
         {"track": 0, "source_stream": 1, "series": "mix", "source": {"codec": "truehd", "profile": "", "channels": 8,
          "layout": "7.1", "layout_guessed": "", "language": "eng", "title": "Main", "sample_fmt": "s32p", "md5": "m1"}},
         {"track": 1, "source_stream": 1, "series": "fc", "source": {"codec": "truehd", "profile": "", "channels": 8,
          "layout": "7.1", "layout_guessed": "", "language": "eng", "title": "Main", "sample_fmt": "s32p", "md5": "m1"}},
         {"track": 2, "source_stream": 2, "series": "mix", "source": {"codec": "ac3", "channels": 2, "layout": "",
          "layout_guessed": "stereo", "language": "fre", "title": "", "sample_fmt": "", "md5": ""}},
         {"track": 3, "source_stream": 5, "series": "mix", "source": {"codec": "pcm_s16le", "profile": "x  y",
          "language": "", "sample_fmt": "s16"}}
        ]}"#;

    /// ffmpeg's raw ametadata output in the mixed run: a frame line, CRLF, a lone CR, -inf, nan, values below the
    /// floor, a tie and a near-tie of the 0.1 rounding, a key that does not start the line, no final line end.
    const RAW: &str = concat!(
        "frame:0    pts:0       pts_time:0\r\n",
        "lavfi.astats.Overall.RMS_level=-23.450000\r\n",
        "lavfi.astats.Overall.RMS_level=-inf\r\n",
        "frame:1\r",
        "lavfi.astats.Overall.RMS_level=nan\r",
        "lavfi.astats.Overall.RMS_level=-120.000000\n",
        "lavfi.astats.Overall.RMS_level=-0.040000\n",
        "lavfi.astats.Overall.RMS_level=-0.050000\n",
        "lavfi.astats.Overall.RMS_level=-100.000000\n",
        "lavfi.astats.Overall.RMS_level=-9.950000\n",
        "lavfi.astats.Overall.RMS_level=-10.049999\n",
        "  lavfi.astats.Overall.RMS_level=-5.0\n",
        "lavfi.astats.Overall.RMS_level=-7.25",
    );

    /// Every level file of the mixed run after its header line.
    const MIXED_LEVELS: &str = "-23.5\r\n-100.0\r\n-100.0\r\n-100.0\r\n-0.0\r\n-0.1\r\n-100.0\r\n-10.0\r\n-10.0\r\n-7.3\r\n";

    /// manifest.txt of the mixed run.
    const MIXED_MANIFEST: &str = concat!(
        "# lazymkv audio-levels v2 (experiment 13)\n",
        "file=a_film.mkv bytes=98765 seconds=0 frame_ms=20 band_hz=300-3400 from_proxy=audio-proxy.mka\n",
        "main=1 codec=truehd profile=- channels=8 layout=7.1 language=eng fc=yes frames=10 md5=m1\n",
        "cand=2 codec=ac3 profile=- channels=2 layout=- layout_guessed=stereo language=fre frames=10 md5=unknown-2\n",
        "cand=5 codec=pcm_s16le profile=x_y channels=0 layout=- language=- frames=10 md5=unknown-5\n",
    );

    /// The filter graph the Python gave ffmpeg in the mixed run, verbatim.
    const PYTHON_GRAPH: &str = concat!(
        "[0:1]aformat=sample_fmts=fltp:channel_layouts=mono,volume=18dB,aformat=sample_fmts=s32p,aresample=8000,",
        "highpass=f=300,lowpass=f=3400,asetnsamples=n=160:p=0,astats=metadata=1:reset=1:measure_perchannel=none:",
        "measure_overall=RMS_level,ametadata=print:key=lavfi.astats.Overall.RMS_level:file=raw-main-mix.txt[o0];",
        "[0:2]aformat=sample_fmts=fltp:channel_layouts=mono,volume=18dB,aformat=sample_fmts=s32p,aresample=8000,",
        "highpass=f=300,lowpass=f=3400,asetnsamples=n=160:p=0,astats=metadata=1:reset=1:measure_perchannel=none:",
        "measure_overall=RMS_level,ametadata=print:key=lavfi.astats.Overall.RMS_level:file=raw-main-fc.txt[o1];",
        "[0:3]aformat=sample_fmts=fltp:channel_layouts=mono,volume=18dB,aformat=sample_fmts=fltp,aresample=8000,",
        "highpass=f=300,lowpass=f=3400,asetnsamples=n=160:p=0,astats=metadata=1:reset=1:measure_perchannel=none:",
        "measure_overall=RMS_level,ametadata=print:key=lavfi.astats.Overall.RMS_level:file=raw-cand-2.txt[o2];",
        "[0:7]aformat=sample_fmts=fltp:channel_layouts=mono,volume=18dB,aformat=sample_fmts=s16,aresample=8000,",
        "highpass=f=300,lowpass=f=3400,asetnsamples=n=160:p=0,astats=metadata=1:reset=1:measure_perchannel=none:",
        "measure_overall=RMS_level,ametadata=print:key=lavfi.astats.Overall.RMS_level:file=raw-cand-5.txt[o3]",
    );

    /// The mixed run: from the folder `dir`, relative paths, main stream 1 with its centre track, candidates 2 and 5.
    fn mixed_run(dir: &Path) -> (Result<Ordered<usize>, String>, Vec<Call>) {
        let log = Log::default();
        let ffmpeg = Output { status: 0, stdout: "o\n".to_string(), stderr: String::new() };
        let ctx = ctx_with(dir, fake(mixed_probe(), RAW.to_string(), ffmpeg, &log));
        let info: AudioProxy = serde_json::from_str(MIXED_INFO).unwrap();
        let result = build(&ctx, Path::new("audio-proxy.mka"), "audio-proxy.mka", Path::new("out1"), 1, &[2, 5], Some(info));
        let calls = log.borrow().clone();
        (result, calls)
    }

    /// audio-proxy.json of the refusals and of the command's tests: stream 1 (six channels, a centre track) and
    /// stream 2.
    const SMALL_INFO: &str = r#"{"format": "lazymkv audio proxy v1",
        "source": {"file": "x.mkv", "bytes": 1, "audio_streams": 2},
        "encoding": {"gain_db": -18},
        "tracks": [
         {"track": 0, "source_stream": 1, "series": "mix", "source": {"codec": "ac3", "profile": "", "channels": 6,
          "layout": "stereo", "layout_guessed": "", "language": "eng", "title": "t1", "sample_fmt": "s32p", "md5": "m1"}},
         {"track": 1, "source_stream": 1, "series": "fc", "source": {"codec": "ac3", "profile": "", "channels": 6,
          "layout": "stereo", "layout_guessed": "", "language": "eng", "title": "t1", "sample_fmt": "s32p", "md5": "m1"}},
         {"track": 2, "source_stream": 2, "series": "mix", "source": {"codec": "ac3", "profile": "", "channels": 2,
          "layout": "stereo", "layout_guessed": "", "language": "eng", "title": "t2", "sample_fmt": "fltp", "md5": "m2"}}
        ]}"#;

    fn small_info() -> Value {
        serde_json::from_str(SMALL_INFO).unwrap()
    }

    /// manifest.txt for SMALL_INFO, main 1, candidate 2, three frames each.
    const SMALL_MANIFEST: &str = concat!(
        "# lazymkv audio-levels v2 (experiment 13)\n",
        "file=x.mkv bytes=1 seconds=0 frame_ms=20 band_hz=300-3400 from_proxy=audio-proxy.mka\n",
        "main=1 codec=ac3 profile=- channels=6 layout=stereo language=eng fc=yes frames=3 md5=m1\n",
        "cand=2 codec=ac3 profile=- channels=2 layout=stereo language=eng frames=3 md5=m2\n",
    );

    /// audio-proxy.json for the proxy the real test makes: stream 1 with a centre track, stream 2, 18 dB stored below
    /// the signal.
    const REAL_PROXY_JSON: &str = r#"{"format": "lazymkv audio proxy v1", "source": {"file": "made up.mkv", "bytes": 1234, "audio_streams": 2}, "encoding": {"gain_db": -18, "read": "lazymkv-dev levels-from-proxy"}, "tracks": [
{"track": 0, "source_stream": 1, "series": "mix", "source": {"codec": "dts", "profile": "DTS-HD MA", "channels": 6, "layout": "5.1(side)", "layout_guessed": "", "language": "eng", "title": "Main", "sample_fmt": "s32p", "md5": "aaaa"}},
{"track": 1, "source_stream": 1, "series": "fc", "source": {"codec": "dts", "profile": "DTS-HD MA", "channels": 6, "layout": "5.1(side)", "layout_guessed": "", "language": "eng", "title": "Main", "sample_fmt": "s32p", "md5": "aaaa"}},
{"track": 2, "source_stream": 2, "series": "mix", "source": {"codec": "ac3", "profile": "", "channels": 2, "layout": "stereo", "layout_guessed": "", "language": "eng", "title": "Commentary", "sample_fmt": "fltp", "md5": ""}}
]}"#;

    /// manifest.txt the Python wrote for that proxy (ffmpeg 8.1.2).
    const REAL_MANIFEST: &str = concat!(
        "# lazymkv audio-levels v2 (experiment 13)\n",
        "file=made_up.mkv bytes=1234 seconds=0 frame_ms=20 band_hz=300-3400 from_proxy=audio-proxy.mka\n",
        "main=1 codec=dts profile=DTS-HD_MA channels=6 layout=5.1(side) language=eng fc=yes frames=100 md5=aaaa\n",
        "cand=2 codec=ac3 profile=- channels=2 layout=stereo language=eng frames=100 md5=unknown-2\n",
    );

    /// streams.json the Python wrote for that proxy.
    const REAL_STREAMS_JSON: &str = concat!(
        "{\n",
        " \"from_proxy\": \"audio-proxy.json (audio streams only)\",\n",
        " \"streams\": [\n",
        "  {\n",
        "   \"index\": 1,\n",
        "   \"codec_type\": \"audio\",\n",
        "   \"codec_name\": \"dts\",\n",
        "   \"channels\": 6,\n",
        "   \"profile\": \"DTS-HD MA\",\n",
        "   \"channel_layout\": \"5.1(side)\",\n",
        "   \"tags\": {\n",
        "    \"language\": \"eng\",\n",
        "    \"title\": \"Main\"\n",
        "   }\n",
        "  },\n",
        "  {\n",
        "   \"index\": 2,\n",
        "   \"codec_type\": \"audio\",\n",
        "   \"codec_name\": \"ac3\",\n",
        "   \"channels\": 2,\n",
        "   \"channel_layout\": \"stereo\",\n",
        "   \"tags\": {\n",
        "    \"language\": \"eng\",\n",
        "    \"title\": \"Commentary\"\n",
        "   }\n",
        "  }\n",
        " ]\n",
        "}\n",
    );

    /// Py: test_audio_dataset ProxyStreams. proxy_levels writes streams.json from audio-proxy.json, so a rebuilt
    /// folder keeps its position veto.
    mod proxy_streams {
        use super::*;

        #[test]
        fn the_streams_are_those_ffprobe_lists() {
            let streams = proxy_streams(&info(&VETO, None)).unwrap();
            assert_eq!(
                streams[2],
                json!({"index": 3, "codec_type": "audio", "codec_name": "ac3", "channels": 2,
                       "channel_layout": "stereo", "tags": {"title": "t3"}})
            );
            // json.dumps writes the keys in this order (the Python's dict comparison did not look at it).
            assert_eq!(
                dumps(&streams[2], Style::PY_DEFAULT),
                r#"{"index": 3, "codec_type": "audio", "codec_name": "ac3", "channels": 2, "channel_layout": "stereo", "tags": {"title": "t3"}}"#
            );
            let parsed: Vec<FfprobeStream> = serde_json::from_value(Value::Array(streams)).unwrap();
            let stream = |n: i64, channels: i64, lossless: bool, language: &str, description: &str| AudioStream {
                stream: n,
                channels,
                lossless,
                language: language.to_string(),
                description: description.to_string(),
            };
            assert_eq!(
                ffprobe_audio(&parsed),
                vec![
                    stream(1, 6, false, "eng", "dts DTS 6 ch 5.1(side) t1"),
                    stream(2, 6, true, "eng", "dts DTS-HD MA 6 ch 5.1(side) t2"),
                    stream(3, 2, false, "und", "ac3 2 ch stereo t3"),
                ]
            );
            assert_eq!(position_veto(Some(parsed.as_slice()), 2).holds, Some(true));
            assert_eq!(proxy_streams(&info(&VETO, Some(4))), None); // a stream of the rip is missing
            let mut partial = info_value(&VETO, None);
            partial["tracks"][0]["source"].as_object_mut().unwrap().remove("language");
            assert_eq!(proxy_streams(&serde_json::from_value(partial).unwrap()), None);
        }

        #[test]
        fn build_writes_streams_json_beside_the_levels() {
            let tmp = TempDir::new("veto");
            let root = tmp.0.clone();
            let (mix, _) = programme(420, 1);
            let raw: String = mix[..200].iter().map(|v| format!("{KEY}{}\n", py_fixed(*v, 6))).collect();
            let probe = probe_output(&[(0, 1, "mix"), (1, 1, "fc"), (2, 2, "mix"), (3, 2, "fc"), (4, 3, "mix")]);
            let ctx = ctx_with(&root, fake(probe, raw, ok(), &Log::default()));
            let mka = root.join("audio-proxy.mka");
            let shown = mka.display().to_string();
            let frames = build(&ctx, &mka, &shown, &root.join("veto"), 2, &[3], Some(info(&VETO, None))).unwrap();
            assert_eq!(frames, ordered(&[("main-mix", 200), ("main-fc", 200), ("cand-3", 200)]));
            let tree = DiskTree::open(&root, &root.display().to_string()).unwrap();
            let written = read_streams(&tree, &PathKey::new("veto")).unwrap();
            let expected: Vec<FfprobeStream> =
                serde_json::from_value(Value::Array(proxy_streams(&info(&VETO, None)).unwrap())).unwrap();
            assert_eq!(written.as_deref(), Some(expected.as_slice()));
            assert_eq!(position_veto(written.as_deref(), 2).holds, Some(true));
            // What the Python wrote for exactly these inputs, byte for byte.
            assert_eq!(read(&root.join("veto/streams.json")), VETO_STREAMS_JSON);
            assert_eq!(read(&root.join("veto/manifest.txt")), VETO_MANIFEST);
            for (series, bytes) in [("main-mix", 1513), ("main-fc", 1512), ("cand-3", 1511)] {
                let text = read(&root.join(format!("veto/{series}.txt")));
                assert_eq!(text.len(), bytes, "{series}");
                assert_eq!(text.matches("\r\n").count(), 201, "{series}");
                let head = format!(
                    "# lazymkv audio levels v2: frame_ms=20 band_hz=300-3400 unit=dB floor=-100 series={series} file=audio-proxy.mka\r\n-19.9\r\n-19.9\r\n-19.9\r\n-19.9\r\n-30.5\r\n"
                );
                assert!(text.starts_with(&head), "{series}: {text:.200}");
                assert!(!root.join(format!("veto/raw-{series}.txt")).exists());
            }
            // Deliberate (spec section 4): the path as given; the Python wrote the resolved one.
            assert_eq!(read(&root.join("veto/proxy.txt")), format!("levels rebuilt from {shown}\n"));
            build(&ctx, &mka, &shown, &root.join("short"), 2, &[3], Some(info(&VETO, Some(4)))).unwrap();
            assert!(!root.join("short/streams.json").exists());
            assert!(root.join("short/manifest.txt").is_file());
        }
    }

    #[test]
    fn the_proxys_tracks_come_from_its_own_tags() {
        let tmp = TempDir::new("tracks");
        let mka = tmp.0.join("audio-proxy.mka");
        let log = Log::default();
        let tracks = proxy_tracks(&ctx_with(&tmp.0, fake(mixed_probe(), String::new(), ok(), &log)), &mka).unwrap();
        let expected: BTreeMap<(i64, String), String> =
            [(1, "mix", "1"), (1, "fc", "2"), (2, "mix", "3"), (3, "side", "4"), (5, "mix", "7")]
                .iter()
                .map(|&(n, series, track)| ((n, series.to_string()), track.to_string()))
                .collect();
        assert_eq!(tracks, expected);
        let shown = mka.to_string_lossy().into_owned();
        assert_eq!(
            log.borrow()[0].args,
            words(&["-v", "error", "-show_entries", "stream=index,codec_type:stream_tags", "-of", "json", &shown])
        );
        let absent = ctx_with(&tmp.0, FakeRunner::absent());
        assert_eq!(proxy_tracks(&absent, &mka), Err("[Errno 2] No such file or directory: 'ffprobe'".to_string()));
        let named = Output {
            status: 0,
            stdout: r#"{"streams": [{"index": 1, "codec_type": "audio", "tags": {"LAZYMKV_SOURCE_STREAM": "one"}}]}"#
                .to_string(),
            stderr: String::new(),
        };
        let bad = ctx_with(&tmp.0, fake(named, String::new(), ok(), &Log::default()));
        assert_eq!(proxy_tracks(&bad, &mka), Err("invalid literal for int() with base 10: 'one'".to_string()));
    }

    #[test]
    fn build_runs_ffprobe_then_one_ffmpeg_pass_as_the_python_did() {
        let tmp = TempDir::new("args");
        let (result, calls) = mixed_run(&tmp.0);
        assert_eq!(result, Ok(ordered(&[("main-mix", 10), ("main-fc", 10), ("cand-2", 10), ("cand-5", 10)])));
        let mka = resolve_like_python(&tmp.0.join("audio-proxy.mka")).to_string_lossy().into_owned();
        assert_eq!(calls.len(), 2);
        assert_eq!(
            calls[0],
            Call {
                program: "ffprobe".to_string(),
                args: words(&["-v", "error", "-show_entries", "stream=index,codec_type:stream_tags", "-of", "json", &mka]),
                cwd: None,
            }
        );
        let mut ffmpeg = words(&["-v", "error", "-nostdin", "-i", &mka, "-filter_complex", PYTHON_GRAPH]);
        for i in 0..4 {
            ffmpeg.extend(words(&["-map", &format!("[o{i}]"), "-f", "null", &format!("o{i}")]));
        }
        assert_eq!(calls[1], Call { program: "ffmpeg".to_string(), args: ffmpeg, cwd: Some(tmp.0.join("out1")) });
    }

    #[test]
    fn build_writes_the_files_as_the_python_did() {
        let tmp = TempDir::new("files");
        mixed_run(&tmp.0).0.unwrap();
        let out = tmp.0.join("out1");
        let mut names: Vec<String> = std::fs::read_dir(&out)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        // No raw-<series>.txt left, and no streams.json: stream 5's facts have no channels.
        assert_eq!(names, ["cand-2.txt", "cand-5.txt", "ffmpeg.txt", "main-fc.txt", "main-mix.txt", "manifest.txt", "proxy.txt"]);
        for series in ["main-mix", "main-fc", "cand-2", "cand-5"] {
            assert_eq!(
                read(&out.join(format!("{series}.txt"))),
                format!(
                    "# lazymkv audio levels v2: frame_ms=20 band_hz=300-3400 unit=dB floor=-100 series={series} file=audio-proxy.mka\r\n{MIXED_LEVELS}"
                ),
                "{series}"
            );
        }
        assert_eq!(read(&out.join("manifest.txt")), MIXED_MANIFEST);
        assert_eq!(read(&out.join("ffmpeg.txt")), "o\n");
        // Deliberate (spec section 4): the path as given; the Python wrote the resolved one.
        assert_eq!(read(&out.join("proxy.txt")), "levels rebuilt from audio-proxy.mka\n");
    }

    #[test]
    fn build_refuses_as_the_python_does() {
        let tmp = TempDir::new("refusals");
        let root = tmp.0.as_path();
        let three = format!("{KEY}-20.000000\n").repeat(3);
        let ctx = ctx_with(root, fake(mixed_probe(), three.clone(), ok(), &Log::default()));
        std::fs::create_dir(root.join("good")).unwrap();
        assert_eq!(build_with(&ctx, "good", 1, &[2], Some(small_info())), refused("good already exists"));
        assert_eq!(build_with(&ctx, "./good/", 1, &[2], Some(small_info())), refused("good already exists"));
        assert_eq!(
            build_with(&ctx, "e1", 9, &[2], Some(small_info())),
            refused("the proxy has no track of stream 9 (it holds [1, 2, 3, 5])")
        );
        for cands in [[2, 1], [2, 2]] {
            assert_eq!(
                build_with(&ctx, "e2", 1, &cands, Some(small_info())),
                refused("the main stream is also a candidate, or a candidate is listed twice")
            );
        }
        assert!(!root.join("e1").exists() && !root.join("e2").exists());
        let mut upper = small_info();
        upper["tracks"][2]["source"]["sample_fmt"] = json!("S32P");
        assert_eq!(
            build_with(&ctx, "e4", 1, &[2], Some(upper)),
            refused("audio-proxy.json gives stream 2 the sample format 'S32P'")
        );
        assert_eq!(std::fs::read_dir(root.join("e4")).unwrap().count(), 0, "made, then left empty, as the Python leaves it");
        // A stream's facts are its last track's: track 1's "s32p" stands and track 0's "s32 p" is never read.
        let mut last = small_info();
        last["tracks"][0]["source"]["sample_fmt"] = json!("s32 p");
        assert!(build_with(&ctx, "e4b", 1, &[2], Some(last)).is_ok());

        let failing = Output { status: 3, stdout: "partial\n".to_string(), stderr: format!("\n  {}zzzz\n", "y".repeat(299)) };
        let log = Log::default();
        let loud = ctx_with(root, fake(mixed_probe(), three.clone(), failing, &log));
        let mut gain = small_info();
        gain["encoding"] = json!({"gain_db": 6.5});
        assert_eq!(build_with(&loud, "e5", 1, &[], Some(gain)), Err(format!("ffmpeg failed (exit 3): {}z", "y".repeat(299))));
        assert_eq!(read(&root.join("e5/ffmpeg.txt")), format!("partial\n\n  {}zzzz\n", "y".repeat(299)));
        assert!(log.borrow()[1].args[6].contains(",volume=-6.5dB,"));

        let mka_shown = resolve_like_python(&root.join("audio-proxy.mka")).display().to_string();
        let unreadable =
            Output { status: 1, stdout: String::new(), stderr: " Invalid data found when processing input\n".to_string() };
        let probe_fails = ctx_with(root, fake(unreadable, three.clone(), ok(), &Log::default()));
        assert_eq!(
            build_with(&probe_fails, "e6", 1, &[2], Some(small_info())),
            Err(format!("ffprobe cannot read {mka_shown}: Invalid data found when processing input"))
        );
        assert!(!root.join("e6").exists());
        let absent = ctx_with(root, FakeRunner::absent());
        assert_eq!(
            build_with(&absent, "e6", 1, &[2], Some(small_info())),
            refused("[Errno 2] No such file or directory: 'ffprobe'")
        );

        // audio-proxy.json read from beside the proxy (info None).
        let json_shown = resolve_like_python(&root.join("audio-proxy.json")).display().to_string();
        assert_eq!(
            build_with(&ctx, "e10", 1, &[2], None),
            Err(format!("{json_shown} is missing: without it the proxy is incomplete (audio-levels.ps1 writes it last)"))
        );
        let mut v0 = small_info();
        v0["format"] = json!("lazymkv audio proxy v0");
        std::fs::write(root.join("audio-proxy.json"), v0.to_string()).unwrap();
        assert_eq!(
            build_with(&ctx, "e7", 1, &[2], None),
            Err(format!("{json_shown}: format is 'lazymkv audio proxy v0', not 'lazymkv audio proxy v1'"))
        );
        let mut unnamed = small_info();
        unnamed.as_object_mut().unwrap().remove("format");
        std::fs::write(root.join("audio-proxy.json"), unnamed.to_string()).unwrap();
        assert_eq!(
            build_with(&ctx, "e8", 1, &[2], None),
            Err(format!("{json_shown}: format is None, not 'lazymkv audio proxy v1'"))
        );
        let mut marked = b"\xef\xbb\xbf".to_vec();
        marked.extend_from_slice(SMALL_INFO.as_bytes());
        std::fs::write(root.join("audio-proxy.json"), marked).unwrap();
        assert_eq!(build_with(&ctx, "e9", 1, &[2], None), Ok(ordered(&[("main-mix", 3), ("main-fc", 3), ("cand-2", 3)])));
        assert_eq!(read(&root.join("e9/manifest.txt")), SMALL_MANIFEST);
    }

    #[test]
    fn run_reads_its_arguments_and_prints_as_the_python_did() {
        let tmp = TempDir::new("run");
        let root = tmp.0.as_path();
        std::fs::write(root.join("audio-proxy.json"), SMALL_INFO).unwrap();
        let ctx = ctx_with(root, fake(mixed_probe(), format!("{KEY}-20.000000\n").repeat(3), ok(), &Log::default()));
        let usage = "usage: lazymkv-dev levels-from-proxy <audio-proxy.mka> <new folder> --main <stream> [--cand <stream>[,<stream>...]]\n";
        let done = |folder: &str, series: &str| format!("levels from audio-proxy.mka: {series} -> {folder}\n");
        let both = "main-mix 3 frames, main-fc 3 frames, cand-2 3 frames";
        let alone = "main-mix 3 frames, main-fc 3 frames";
        let not_int = "invalid literal for int() with base 10: 'x'\n";
        assert_eq!(call(&ctx, &["audio-proxy.mka", "good", "--main", "1", "--cand", "2"]), printed(0, &done("good", both), ""));
        assert_eq!(call(&ctx, &["audio-proxy.mka", "good", "--main", "1", "--cand", "2"]), printed(1, "", "good already exists\n"));
        assert_eq!(call(&ctx, &["audio-proxy.mka", "good"]), printed(1, "", usage));
        assert_eq!(call(&ctx, &["audio-proxy.mka", "good2", "--cand", "1"]), printed(1, "", usage));
        assert_eq!(call(&ctx, &["audio-proxy.mka", "good3", "--main", "1", "--cand"]), printed(0, &done("good3", alone), ""));
        assert_eq!(call(&ctx, &["audio-proxy.mka", "good4", "--main", "x"]), printed(1, "", not_int));
        assert_eq!(call(&ctx, &["audio-proxy.mka", "good5", "--main", "1", "--cands", "2"]), printed(0, &done("good5", alone), ""));
        assert_eq!(
            call(&ctx, &["audio-proxy.mka", "good6", "--main", " +1 ", "--cand", ",2,", "extra"]),
            printed(0, &done("good6", both), "")
        );
        assert_eq!(call(&ctx, &["audio-proxy.mka", "good8", "--main", "1", "--cand", "2,x"]), printed(1, "", not_int));
        let missing = resolve_like_python(&root.join("missing")).join("audio-proxy.json");
        assert_eq!(
            call(&ctx, &["missing/audio-proxy.mka", "good7", "--main", "1"]),
            printed(
                1,
                "",
                &format!("{} is missing: without it the proxy is incomplete (audio-levels.ps1 writes it last)\n", missing.display())
            )
        );
        // audio-proxy.json without every audio stream of the rip: no streams.json, and the note (its command renamed).
        std::fs::write(root.join("audio-proxy.json"), SMALL_INFO.replace("\"audio_streams\": 2", "\"audio_streams\": 3"))
            .unwrap();
        let note = "note: audio-proxy.json does not record every audio stream of the rip, so no streams.json was written: lazymkv-dev retune counts none of this folder's candidates (copy the rip's ffprobe-streams.json there as streams.json)\n";
        assert_eq!(
            call(&ctx, &["audio-proxy.mka", "noted", "--main", "1", "--cand", "2"]),
            printed(0, &format!("{}{note}", done("noted", both)), "")
        );
    }

    #[test]
    fn levels_from_proxy_is_a_command() {
        let tmp = TempDir::new("dispatch");
        let ctx = Ctx::for_test(&tmp.0);
        let (code, out, err) = crate::testutil::call(crate::dispatch, &ctx, &["levels-from-proxy", "audio-proxy.mka", "out"]);
        assert_eq!(code, 1);
        assert_eq!(
            err,
            "usage: lazymkv-dev levels-from-proxy <audio-proxy.mka> <new folder> --main <stream> [--cand <stream>[,<stream>...]]\n"
        );
        assert!(out.is_empty());
    }

    #[test]
    fn manifest_words_and_the_volume_are_spelt_as_python_spells_them() {
        assert_eq!(word(None), "-");
        assert_eq!(word(Some(&Value::Null)), "-");
        assert_eq!(word(Some(&json!(""))), "-");
        assert_eq!(word(Some(&json!("a b\t c"))), "a_b_c");
        assert_eq!(word(Some(&json!("x\u{1c}y"))), "x_y");
        assert_eq!(word(Some(&json!("\u{a0}lead"))), "_lead");
        assert_eq!(word(Some(&json!(0))), "0");
        assert_eq!(word(Some(&json!(false))), "False");
        assert_eq!(word(Some(&json!(1.5))), "1.5");
        for (gain, undo) in [
            (-18.0, ",volume=18dB"),
            (6.5, ",volume=-6.5dB"),
            (0.1, ",volume=-0.1dB"),
            (-1e-7, ",volume=1e-07dB"),
            (123456789.0, ",volume=-1.23457e+08dB"),
            (0.0, ""),
            (-0.0, ""),
        ] {
            assert_eq!(volume_undo(gain), undo, "gain {gain}");
        }
    }

    #[test]
    fn paths_are_resolved_and_shown_as_pathlib_does() {
        for (given, shown) in [
            ("good", "good"),
            ("good/", "good"),
            ("./a//b/./", "a/b"),
            ("", "."),
            ("./", "."),
            ("/", "/"),
            ("//", "//"),
            ("//x", "//x"),
            ("///x/", "/x"),
            ("a/../b", "a/../b"),
        ] {
            assert_eq!(py_path_str(given), shown, "{given:?}");
        }
        let tmp = TempDir::new("resolve");
        let real = resolve_like_python(&tmp.0);
        assert!(real.is_absolute() && real.is_dir());
        assert!(!real.to_string_lossy().starts_with(r"\\?\"));
        assert_eq!(resolve_like_python(&tmp.0.join("nowhere/../audio-proxy.mka")), real.join("audio-proxy.mka"));
        assert_eq!(resolve_like_python(&tmp.0.join("nowhere/x.mka")), real.join("nowhere").join("x.mka"));
    }

    #[test]
    fn real_ffmpeg_rebuilds_the_levels_of_a_made_proxy() {
        for tool in ["ffmpeg", "ffprobe"] {
            match RealRunner.run(tool, &words(&["-version"]), None) {
                Ok(answer) if answer.status == 0 => {}
                Ok(answer) => return skip(&format!("{tool} -version exited {}", answer.status)),
                Err(_) => return skip(&format!("{tool} is not on PATH")),
            }
        }
        let tmp = TempDir::new("real");
        // Three mono FLAC tracks of 2 s at 8 kHz, tagged as audio-levels.ps1 tags them: stream 1's mix (1 kHz) and
        // centre (1 kHz, 6 dB lower), stream 2's mix (500 Hz), each stored 18 dB below what the chain measures.
        let make = words(&[
            "-v", "error", "-nostdin", "-y",
            "-f", "lavfi", "-i", "aevalsrc=0.0630957*sin(2*PI*1000*t):s=8000:d=2",
            "-f", "lavfi", "-i", "aevalsrc=0.0315479*sin(2*PI*1000*t):s=8000:d=2",
            "-f", "lavfi", "-i", "aevalsrc=0.0630957*sin(2*PI*500*t):s=8000:d=2",
            "-map", "0:a", "-map", "1:a", "-map", "2:a", "-c:a", "flac",
            "-metadata:s:a:0", "LAZYMKV_SOURCE_STREAM=1", "-metadata:s:a:0", "LAZYMKV_SERIES=mix",
            "-metadata:s:a:1", "LAZYMKV_SOURCE_STREAM=1", "-metadata:s:a:1", "LAZYMKV_SERIES=fc",
            "-metadata:s:a:2", "LAZYMKV_SOURCE_STREAM=2", "-metadata:s:a:2", "LAZYMKV_SERIES=mix",
            "audio-proxy.mka",
        ]);
        let made = RealRunner.run("ffmpeg", &make, Some(tmp.0.as_path())).unwrap();
        if made.status != 0 {
            return skip(&format!("this ffmpeg cannot make the test proxy (lavfi, flac, matroska): {}", made.stderr.trim()));
        }
        std::fs::write(tmp.0.join("audio-proxy.json"), REAL_PROXY_JSON).unwrap();
        let mut ctx = Ctx::for_test(&tmp.0);
        ctx.runner = Box::new(RealRunner);
        let frames = build(&ctx, Path::new("audio-proxy.mka"), "audio-proxy.mka", Path::new("out"), 1, &[2], None).unwrap();
        assert_eq!(frames, ordered(&[("main-mix", 100), ("main-fc", 100), ("cand-2", 100)]));
        let out = tmp.0.join("out");
        assert_eq!(read(&out.join("manifest.txt")), REAL_MANIFEST);
        assert_eq!(read(&out.join("streams.json")), REAL_STREAMS_JSON);
        assert_eq!(read(&out.join("proxy.txt")), "levels rebuilt from audio-proxy.mka\n");
        assert!(out.join("ffmpeg.txt").is_file());
        // The Python on ffmpeg 8.1.2 gave -9.0 (one frame -9.1), -15.1, and -9.5 (one frame -9.7) for these tracks.
        for (series, level) in [("main-mix", -9.0), ("main-fc", -15.1), ("cand-2", -9.5)] {
            assert!(!out.join(format!("raw-{series}.txt")).exists());
            let text = read(&out.join(format!("{series}.txt")));
            let mut lines = text.split("\r\n");
            let header = format!(
                "# lazymkv audio levels v2: frame_ms=20 band_hz=300-3400 unit=dB floor=-100 series={series} file=audio-proxy.mka"
            );
            assert_eq!(lines.next(), Some(header.as_str()));
            let values: Vec<f64> = lines.filter(|line| !line.is_empty()).map(|line| line.parse().unwrap()).collect();
            assert_eq!(values.len(), 100, "{series}");
            assert!(values.iter().all(|v| (v - level).abs() <= 0.3), "{series}: {values:?}");
        }
    }
}
