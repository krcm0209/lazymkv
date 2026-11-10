//! Step 10a's pass, ported from scripts/phase0/audio-levels.ps1 (design parts 5.7 and 7.6; call 9, C1, call 22): the
//! filter graph and command, the series files, the stream hashes, the centre channel, the manifest, the proxy checks
//! and audio-proxy.json. Text in, text out (RP19): engine::step10a runs ffmpeg and writes the files.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use regex::Regex;
use serde_json::{json, Value};

use crate::audio::levels::{MANIFEST_LINE, PROXY_FORMAT};
use crate::recording::files::FfprobeStream;
use crate::recording::universal_lines;

/// The proxy's first gain in dB (design 7.6, spec 7, C1). Def: T5-16. Use: T5-17.
pub const GAIN_FIRST_DB: i32 = -18;
/// The gain of C1's redo in dB (design 7.6, C1). Def: T5-16. Use: T5-17.
pub const GAIN_REDO_DB: i32 = -30;
/// A running peak at or above this, in dBFS, after the gain is a possible clip (audio-levels.ps1 line 73, C1).
/// Def: T5-16. Use: T5-17.
pub const CLIP_DBFS: f64 = -0.1;

/// The key of a level line in ffmpeg's ametadata output (audio-levels.ps1 `$Key`).
const RMS_KEY: &str = "lavfi.astats.Overall.RMS_level=";
/// The key of a running-peak line (audio-levels.ps1 `$PeakKey`).
const PEAK_KEY: &str = "lavfi.astats.Overall.Peak_level=";
/// The level chain up to its file name (audio-levels.ps1 lines 265 to 267).
const TAIL: &str = "aresample=8000,highpass=f=300,lowpass=f=3400,asetnsamples=n=160:p=0,astats=metadata=1:reset=1:measure_perchannel=none:measure_overall=RMS_level,ametadata=print:key=lavfi.astats.Overall.RMS_level:file=";
/// ffmpeg's guess for a stream whose layout it cannot name, by channel count (audio-levels.ps1 line 77).
const GUESSED: &[(i64, &str)] =
    &[(1, "mono"), (2, "stereo"), (3, "2.1"), (4, "4.0"), (5, "5.0"), (6, "5.1"), (7, "6.1"), (8, "7.1")];

/// ffprobe's spelling of an unnamed layout, "<n> channels (FL+FR+...)" (audio-levels.ps1 line 136, `-match`).
static SPELLED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^[0-9]+ channels \(([^()]+)\)$").expect("the layout pattern is valid"));
/// One line of streamhash.txt (audio-levels.ps1 line 345, `-match`).
static HASH_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^([0-9]+),a,MD5=([0-9a-f]{32})$").expect("the streamhash pattern is valid"));
/// A Matroska DURATION tag (audio-levels.ps1 line 152).
static DURATION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([0-9]+):([0-9]{2}):([0-9]{2}(\.[0-9]+)?)$").expect("the duration pattern is valid"));

/// One audio stream of the file as the pass reads it (audio-levels.ps1 lines 249 to 259). Def: T5-16. Use: T5-17.
#[derive(Clone, Debug, PartialEq)]
pub struct PassStream {
    /// ffprobe's stream index.
    pub index: i64,
    /// codec_name, "" when absent.
    pub codec: String,
    /// ffprobe's profile.
    pub profile: Option<String>,
    /// The channel count, 0 when absent.
    pub channels: i64,
    /// ffprobe's channel_layout.
    pub layout: Option<String>,
    /// The `language` tag.
    pub language: Option<String>,
}

/// The audio streams of `streams` (codec_type "audio"), by ascending index (ffprobe's file order); codec "" when
/// absent, channels 0 when absent, language from the `language` tag. Def: T5-16. Use: T5-17.
pub fn pass_streams(streams: &[FfprobeStream]) -> Vec<PassStream> {
    let mut out: Vec<PassStream> = streams
        .iter()
        .filter(|s| s.codec_type.as_deref() == Some("audio"))
        .map(|s| PassStream {
            index: s.index,
            codec: s.codec_name.clone().unwrap_or_default(),
            profile: s.profile.clone(),
            channels: s.channels.unwrap_or(0),
            layout: s.channel_layout.clone(),
            language: s.tags.as_ref().and_then(|t| t.get("language")).cloned(),
        })
        .collect();
    out.sort_by_key(|s| s.index);
    out
}

/// Phase 0's four refusals of a main stream and candidates (audio-levels.ps1 lines 177, 178, 246, 247), checked in
/// that order over the main then each candidate: "stream {main} is the main stream and cannot also be a candidate",
/// "a candidate stream is listed twice: {cands joined with ,}", "the file has no stream {n}",
/// "stream {n} is {codec_type or empty}, not audio". Def: T5-16. Use: T5-17.
pub fn check_main_and_candidates(streams: &[FfprobeStream], main: i64, cands: &[i64]) -> Result<(), String> {
    if cands.contains(&main) {
        return Err(format!("stream {main} is the main stream and cannot also be a candidate"));
    }
    if cands.iter().collect::<BTreeSet<_>>().len() != cands.len() {
        let joined: Vec<String> = cands.iter().map(i64::to_string).collect();
        return Err(format!("a candidate stream is listed twice: {}", joined.join(",")));
    }
    for n in std::iter::once(&main).chain(cands) {
        let Some(s) = streams.iter().find(|s| s.index == *n) else {
            return Err(format!("the file has no stream {n}"));
        };
        if s.codec_type.as_deref() != Some("audio") {
            return Err(format!("stream {n} is {}, not audio", s.codec_type.as_deref().unwrap_or("")));
        }
    }
    Ok(())
}

/// The main stream and candidates of one `levels_args` job line, the first of which step 10a runs (T5-R38): the
/// integer after the word `-Main` and the comma list after `-Cand` (none when absent); None for a line that does not
/// start with `scripts/phase0/job.sh` or has no integer `-Main`. Def: T5-16. Use: T5-17.
pub fn main_and_candidates(job_line: &str) -> Option<(i64, Vec<i64>)> {
    if !job_line.starts_with("scripts/phase0/job.sh") {
        return None;
    }
    let words: Vec<&str> = job_line.split_ascii_whitespace().collect();
    let after = |flag: &str| words.iter().position(|w| *w == flag).map(|k| words.get(k + 1).copied());
    let main = after("-Main")??.parse::<i64>().ok()?;
    let cands = match after("-Cand") {
        None => Vec::new(),
        Some(list) => list?.split(',').map(|c| c.parse::<i64>().ok()).collect::<Option<Vec<i64>>>()?,
    };
    Some((main, cands))
}

/// ffmpeg's own table of named channel layouts (`ffmpeg -hide_banner -layouts`, section NAME DECOMPOSITION).
/// Def: T5-16. Use: T5-17.
#[derive(Clone, Debug, PartialEq)]
pub struct Layouts {
    /// Layout name, ASCII-lowercased as a PowerShell hashtable matches it, to its channels.
    table: BTreeMap<String, Vec<String>>,
}

/// The table of `ffmpeg -hide_banner -layouts`' output (audio-levels.ps1 lines 119 to 130); Err "ffmpeg -layouts printed
/// no table of channel layouts (no NAME DECOMPOSITION section)" when it holds no entry. Def: T5-16. Use: T5-17.
pub fn parse_layouts(text: &str) -> Result<Layouts, String> {
    let mut table = BTreeMap::new();
    let mut in_table = false;
    for line in universal_lines(text) {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() != 2 {
            continue;
        }
        if cols[0].eq_ignore_ascii_case("NAME") {
            in_table = cols[1].eq_ignore_ascii_case("DECOMPOSITION");
            continue;
        }
        if in_table {
            table.insert(cols[0].to_ascii_lowercase(), cols[1].split('+').map(String::from).collect());
        }
    }
    if table.is_empty() {
        return Err("ffmpeg -layouts printed no table of channel layouts (no NAME DECOMPOSITION section)".to_string());
    }
    Ok(Layouts { table })
}

/// Whether a stream has a centre channel besides others, and the layout ffmpeg guesses when it names none
/// (audio-levels.ps1 lines 74 to 77, 132 to 149, 252 to 259; R6-phase0-58). Def: T5-16. Use: T5-17.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Centre {
    /// The channels hold FC and more than one channel.
    pub has_fc: bool,
    /// The layout name ffmpeg guesses from the channel count, when the stream's own layout names no channels.
    pub guessed: Option<String>,
}

/// R6-phase0-58's rule over `layouts`. Def: T5-16. Use: T5-16, T5-17.
pub fn centre(stream: &PassStream, layouts: &Layouts) -> Centre {
    let layout = stream.layout.as_deref().unwrap_or("");
    let named: Vec<String> = if let Some(channels) = layouts.table.get(&layout.to_ascii_lowercase()) {
        channels.clone()
    } else if let Some(caps) = SPELLED.captures(layout) {
        caps[1].split('+').map(|part| part.split('@').next().unwrap_or("").to_string()).collect()
    } else {
        Vec::new()
    };
    let (channels, guessed) = if !named.is_empty() {
        (named, None)
    } else {
        let guess = GUESSED.iter().find(|(count, _)| *count == stream.channels).map(|(_, name)| *name);
        match guess.and_then(|name| layouts.table.get(&name.to_ascii_lowercase()).map(|c| (name, c))) {
            Some((name, c)) => (c.clone(), Some(name.to_string())),
            None => (Vec::new(), None),
        }
    };
    let has_fc = channels.iter().any(|c| c.eq_ignore_ascii_case("FC")) && channels.len() > 1;
    Centre { has_fc, guessed }
}

/// The name prefix of a level folder, `audio-levels-20ms-<main>[-<cand>...]` (design part 7.5): the one definition,
/// which `level_folder` writes and the engine's staged-folder reader and dataset copy read (T5-R68). Def: T5-16.
/// Use: T5-17, T5-18, T5-19.
pub const LEVEL_PREFIX: &str = "audio-levels-20ms-";

/// "audio-levels-20ms-<main>[-<cand>...]", the candidates in the order given (R6-phase0-57). Def: T5-16. Use: T5-17.
pub fn level_folder(main: i64, cands: &[i64]) -> String {
    let mut name = format!("{LEVEL_PREFIX}{main}");
    for c in cands {
        name.push_str(&format!("-{c}"));
    }
    name
}

/// One level series of the pass: its name (`main-mix`, `main-fc`, `cand-<n>`) and ffmpeg's raw file `raw-<name>.txt`.
/// Def: T5-16. Use: T5-17.
#[derive(Clone, Debug, PartialEq)]
pub struct SeriesOut {
    /// `main-mix`, `main-fc` or `cand-<n>`.
    pub name: String,
    /// `raw-<name>.txt`.
    pub raw: String,
}

/// One proxy track k: its source stream, series (`mix`, `fc`, `side`), language tag (`proxy_language`) and ffmpeg's
/// running-peak file `peak-<k>.txt`. Def: T5-16. Use: T5-17.
#[derive(Clone, Debug, PartialEq)]
pub struct ProxyTrack {
    /// The source stream's index.
    pub stream: i64,
    /// `mix`, `fc` or `side`.
    pub series: String,
    /// The track's language tag, from `proxy_language`.
    pub language: String,
    /// `peak-<k>.txt`.
    pub peak: String,
}

/// The pass's filter graph (`graph.txt`'s whole content, no BOM, no final newline), its series and its proxy tracks.
/// Def: T5-16. Use: T5-17.
#[derive(Clone, Debug, PartialEq)]
pub struct Graph {
    /// graph.txt's content.
    pub text: String,
    /// The level series, `o<k>` in order.
    pub series: Vec<SeriesOut>,
    /// The proxy tracks, `p<k>` in order.
    pub proxy: Vec<ProxyTrack>,
}

/// Phase 0's graph (audio-levels.ps1 lines 262 to 313; R6-phase0-48 to 50), with the level branches only when
/// `levels` (false is C1's redo) and the proxy chain at `gain_db`. Def: T5-16. Use: T5-17.
pub fn graph(streams: &[PassStream], main: i64, cands: &[i64], layouts: &Layouts, levels: bool, gain_db: i32) -> Graph {
    let to_proxy = format!("aresample=8000,volume={gain_db}dB,asetnsamples=n=8000:p=0,astats=metadata=1:reset=0:measure_perchannel=none:measure_overall=Peak_level,ametadata=print:key=lavfi.astats.Overall.Peak_level:file=");
    let mut series: Vec<SeriesOut> = Vec::new();
    let mut proxy: Vec<ProxyTrack> = Vec::new();
    let mut parts: Vec<String> = Vec::new();
    for s in streams {
        let n = s.index;
        let has_fc = centre(s, layouts).has_fc;
        let mut branches: Vec<String> = Vec::new();
        let mut level = |branches: &mut Vec<String>, chain: &str, name: String| {
            let raw = format!("raw-{name}.txt");
            branches.push(format!("{chain},{TAIL}{raw}[o{}]", series.len()));
            series.push(SeriesOut { name, raw });
        };
        if levels {
            if n == main {
                level(&mut branches, "aformat=channel_layouts=mono", "main-mix".to_string());
                if has_fc {
                    level(&mut branches, "pan=mono|c0=FC", "main-fc".to_string());
                }
            } else if cands.contains(&n) {
                level(&mut branches, "aformat=channel_layouts=mono", format!("cand-{n}"));
            }
        }
        let language = proxy_language(s.language.as_deref());
        let mut track = |branches: &mut Vec<String>, chain: &str, name: &str| {
            let k = proxy.len();
            let peak = format!("peak-{k}.txt");
            branches.push(format!("{chain},{to_proxy}{peak},aformat=sample_fmts=s32[p{k}]"));
            proxy.push(ProxyTrack { stream: n, series: name.to_string(), language: language.clone(), peak });
        };
        track(&mut branches, "aformat=channel_layouts=mono", "mix");
        if has_fc {
            track(&mut branches, "pan=mono|c0=FC", "fc");
        }
        if s.channels == 2 {
            track(&mut branches, "pan=mono|c0=0.5*c0-0.5*c1", "side");
        }
        if branches.len() == 1 {
            parts.push(format!("[0:{n}]{}", branches[0]));
        } else if branches.len() > 1 {
            let labels: String = (0..branches.len()).map(|b| format!("[s{n}b{b}]")).collect();
            parts.push(format!("[0:{n}]asplit={}{labels}", branches.len()));
            for (b, branch) in branches.iter().enumerate() {
                parts.push(format!("[s{n}b{b}]{branch}"));
            }
        }
    }
    Graph { text: parts.join(";"), series, proxy }
}

/// ffmpeg's arguments after the program, run in the level folder (audio-levels.ps1 lines 314 to 328; R6-phase0-51).
/// Def: T5-16. Use: T5-17.
pub fn ffmpeg_args(file: &str, graph: &Graph, levels: bool, proxy_partial: Option<&str>) -> Vec<String> {
    let mut args: Vec<String> =
        ["-v", "warning", "-nostats", "-nostdin", "-i", file, "-/filter_complex", "graph.txt"].map(String::from).to_vec();
    if levels {
        for k in 0..graph.series.len() {
            args.extend(["-map".to_string(), format!("[o{k}]"), "-f".to_string(), "null".to_string(), format!("o{k}")]);
        }
        for t in graph.proxy.iter().filter(|t| t.series == "mix") {
            args.extend(["-map".to_string(), format!("0:{}", t.stream)]);
        }
        args.extend(["-c", "copy", "-f", "streamhash", "-hash", "md5", "streamhash.txt"].map(String::from));
    }
    if let Some(partial) = proxy_partial {
        for k in 0..graph.proxy.len() {
            args.extend(["-map".to_string(), format!("[p{k}]")]);
        }
        args.extend(["-c:a", "flac", "-compression_level", "5", "-bits_per_raw_sample", "24"].map(String::from));
        for (k, t) in graph.proxy.iter().enumerate() {
            let flag = format!("-metadata:s:a:{k}");
            for value in [
                format!("LAZYMKV_SOURCE_STREAM={}", t.stream),
                format!("LAZYMKV_SERIES={}", t.series),
                format!("language={}", t.language),
            ] {
                args.extend([flag.clone(), value]);
            }
        }
        args.extend(["-f".to_string(), "matroska".to_string(), partial.to_string()]);
    }
    args
}

/// A manifest value without spaces: `-` when empty, each whitespace run as `_` (audio-levels.ps1 Get-Word).
fn word(value: Option<&str>) -> String {
    let text = value.unwrap_or("");
    if text.is_empty() {
        return "-".to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut in_space = false;
    for c in text.chars() {
        if c.is_whitespace() {
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

/// One series file from ffmpeg's raw ametadata text (audio-levels.ps1 lines 79 to 98, 349 to 358; R6-phase0-55, 56;
/// T5-R19): the v2 header, one value per RMS (root mean square) line, every line ended with CRLF, no BOM. Def: T5-16. Use: T5-17, T5-25.
pub fn series_file(raw: &str, series: &str, file_name: &str) -> Result<String, String> {
    let mut out = format!(
        "# lazymkv audio levels v2: frame_ms=20 band_hz=300-3400 unit=dB floor=-100 series={series} file={}\r\n",
        word(Some(file_name))
    );
    let mut count = 0usize;
    for line in universal_lines(raw) {
        let Some(value) = line.strip_prefix(RMS_KEY) else { continue };
        let text = if value.contains("inf") || value.contains("nan") {
            "-100.0".to_string()
        } else {
            match value.trim().parse::<f64>() {
                Ok(v) if v.is_finite() => {
                    if v < -100.0 {
                        "-100.0".to_string()
                    } else {
                        net_fixed1(value)
                    }
                }
                _ => return Err(format!("raw-{series}.txt: {value} is not a number")),
            }
        };
        out.push_str(&text);
        out.push_str("\r\n");
        count += 1;
    }
    if count == 0 {
        return Err(format!("{series}.txt holds no level"));
    }
    Ok(out)
}

/// A number formatted as Windows PowerShell 5.1 formats `[double]::Parse(value, invariant).ToString('0.0', invariant)`
/// (.NET Framework: 15 significant digits, then half away from zero; T5-R19). The caller has checked that `value`
/// parses to a finite f64; any other text gives "". Def: T5-16. Use: T5-16, T5-25.
pub fn net_fixed1(value: &str) -> String {
    let v = match value.trim().parse::<f64>() {
        Ok(v) if v.is_finite() => v,
        _ => return String::new(),
    };
    // |v| to 15 significant digits: m (15 digits, no point) times 10^(e - 14).
    let sci = format!("{:.14e}", v.abs());
    let Some((mantissa, exponent)) = sci.split_once('e') else { return String::new() };
    let (Ok(m), Ok(e)) = (mantissa.replace('.', "").parse::<u128>(), exponent.parse::<i32>()) else {
        return String::new();
    };
    // The value in tenths, n = m * 10^(e - 13), its digits as text so that no power of ten can overflow.
    let tenths: String = if e >= 13 {
        format!("{m}{}", "0".repeat((e - 13) as usize))
    } else if 13 - e <= 15 {
        let d = 10u128.pow((13 - e) as u32);
        ((m + d / 2) / d).to_string()
    } else {
        "0".to_string()
    };
    let negative = v < 0.0 && tenths != "0";
    let (whole, tenth) = tenths.split_at(tenths.len() - 1);
    let whole = if whole.is_empty() { "0" } else { whole };
    format!("{}{whole}.{tenth}", if negative { "-" } else { "" })
}

/// `streamhash.txt`'s (k, MD5) pairs by ascending k, k the position in the `-map 0:<n>` order (audio-levels.ps1 lines
/// 342 to 347; R1-evidence-9, R6-phase0-53, 77). Def: T5-16. Use: T5-17.
pub fn parse_streamhash(text: &str, audio_streams: usize) -> Result<Vec<(i64, String)>, String> {
    let mut hashes: BTreeMap<i64, String> = BTreeMap::new();
    for line in universal_lines(text) {
        let Some(caps) = HASH_LINE.captures(line) else { continue };
        let Ok(k) = caps[1].parse::<i64>() else { continue };
        if k < audio_streams as i64 {
            hashes.insert(k, caps[2].to_string());
        }
    }
    if hashes.len() != audio_streams {
        return Err(format!("streamhash.txt holds {} of {audio_streams} stream hashes", hashes.len()));
    }
    Ok(hashes.into_iter().collect())
}

/// `manifest.txt` (audio-levels.ps1 lines 360 to 371; R6-phase0-57): CRLF line ends, no BOM. Def: T5-16. Use: T5-17.
#[allow(clippy::too_many_arguments)]
pub fn manifest_text(
    file: &str,
    bytes: u64,
    seconds: f64,
    main: &PassStream,
    cands: &[PassStream],
    layouts: &Layouts,
    frames: &[(i64, usize)],
    md5: &[(i64, String)],
) -> String {
    let mut lines = vec![
        format!("{MANIFEST_LINE} (experiment 13)"),
        format!(
            "file={} bytes={bytes} seconds={} frame_ms=20 band_hz=300-3400",
            word(Some(file)),
            seconds.round_ties_even() as i64
        ),
    ];
    let line = |role: &str, s: &PassStream, fc: Option<bool>| {
        let c = centre(s, layouts);
        let guess = c.guessed.as_ref().map(|g| format!(" layout_guessed={g}")).unwrap_or_default();
        let fc = match fc {
            Some(true) => " fc=yes",
            Some(false) => " fc=no",
            None => "",
        };
        let count = frames.iter().find(|(n, _)| *n == s.index).map(|(_, c)| c.to_string()).unwrap_or_default();
        let hash = md5.iter().find(|(n, _)| *n == s.index).map(|(_, h)| h.as_str()).unwrap_or("");
        format!(
            "{role}={} codec={} profile={} channels={} layout={}{guess} language={}{fc} frames={count} md5={hash}",
            s.index,
            word(Some(&s.codec)),
            word(s.profile.as_deref()),
            s.channels,
            word(s.layout.as_deref()),
            word(s.language.as_deref()),
        )
    };
    lines.push(line("main", main, Some(centre(main, layouts).has_fc)));
    for c in cands {
        lines.push(line("cand", c, None));
    }
    lines.iter().map(|l| format!("{l}\r\n")).collect()
}

/// The last running peak of a `peak-<k>.txt`, trimmed (audio-levels.ps1 lines 100 to 108); None when it holds none.
/// Def: T5-16. Use: T5-16, T5-17.
pub fn last_peak(peak_text: &str) -> Option<String> {
    let last = universal_lines(peak_text).into_iter().filter_map(|l| l.strip_prefix(PEAK_KEY)).last()?.trim();
    // Phase 0's `if (-not $peak)`: an empty last value is no peak.
    (!last.is_empty()).then(|| last.to_string())
}

/// Whether the last running peak is at or above CLIP_DBFS (lines 402 to 408; C1); a peak holding `inf` or `nan` is not.
/// Err "holds no peak" or "holds the peak {text}, which is not a number". Def: T5-16. Use: T5-17.
pub fn peak_clipped(peak_text: &str) -> Result<bool, String> {
    let Some(peak) = last_peak(peak_text) else { return Err("holds no peak".to_string()) };
    if peak.contains("inf") || peak.contains("nan") {
        return Ok(false);
    }
    match peak.parse::<f64>() {
        Ok(v) => Ok(v >= CLIP_DBFS),
        Err(_) => Err(format!("holds the peak {peak}, which is not a number")),
    }
}

/// A key of a JSON object, matched ASCII case-insensitively as PowerShell reads ConvertFrom-Json's properties.
fn field<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    value.as_object()?.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v)
}

/// A JSON value as PowerShell's `[string]` gives it here: a string as written, a number in its decimal form, absent
/// or null as "". Step 10a's proxy record reads ffprobe's answer with it too (T5-R69). Def: T5-16. Use: T5-16, T5-17.
pub fn text_of(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
    }
}

/// `proxy-check.json` checked against the tracks written (lines 384 to 399; R6-phase0-59); Ok gives each track's
/// DURATION tag in seconds, in track order. Def: T5-16. Use: T5-17.
pub fn proxy_check(json: &str, expected: &[ProxyTrack]) -> Result<Vec<f64>, String> {
    let v: Value = serde_json::from_str(json)
        .map_err(|e| format!("proxy check failed: proxy-check.json does not parse: {e}"))?;
    let got: Vec<Value> = field(&v, "streams").and_then(Value::as_array).cloned().unwrap_or_default();
    if got.len() != expected.len() {
        return Err(format!("proxy check failed: the proxy has {} tracks, {} were written", got.len(), expected.len()));
    }
    let mut durations = Vec::with_capacity(expected.len());
    for (k, (g, t)) in got.iter().zip(expected).enumerate() {
        let place = format!("proxy track {k} (stream {} {})", t.stream, t.series);
        let get = |key: &str| text_of(field(g, key));
        let same = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
        let (kind, codec, rate, channels, bits) =
            (get("codec_type"), get("codec_name"), get("sample_rate"), get("channels"), get("bits_per_raw_sample"));
        if !same(&kind, "audio") || !same(&codec, "flac") || !same(&rate, "8000") || !same(&channels, "1") || !same(&bits, "24")
        {
            return Err(format!(
                "proxy check failed: {place} is {kind} {codec} {rate} Hz {channels} ch {bits} bit, not flac 8000 Hz 1 ch 24 bit"
            ));
        }
        let tag = |key: &str| text_of(field(g, "tags").and_then(|tags| field(tags, key)));
        let (source, series) = (tag("LAZYMKV_SOURCE_STREAM"), tag("LAZYMKV_SERIES"));
        if !same(&source, &t.stream.to_string()) || !same(&series, &t.series) {
            return Err(format!("proxy check failed: {place} is tagged stream {source} {series}"));
        }
        let duration = tag_seconds(&tag("DURATION"));
        if duration.is_nan() || duration <= 0.0 {
            return Err(format!("proxy check failed: {place} has no duration (the file was not finished)"));
        }
        durations.push(duration);
    }
    Ok(durations)
}

/// A Matroska duration tag ("01:02:03.456000000") in seconds; -1 when there is none (audio-levels.ps1 Get-TagSeconds).
fn tag_seconds(text: &str) -> f64 {
    let Some(caps) = DURATION.captures(text) else { return -1.0 };
    let num = |k: usize| caps[k].parse::<f64>().unwrap_or(f64::NAN);
    num(1) * 3600.0 + num(2) * 60.0 + num(3)
}

/// The stream's language tag when it matches `^[a-z]{3}$` (case-sensitive), else "und" (R6-phase0-52). Def: T5-16. Use: T5-17.
pub fn proxy_language(tag: Option<&str>) -> String {
    match tag {
        Some(t) if t.len() == 3 && t.bytes().all(|b| b.is_ascii_lowercase()) => t.to_string(),
        _ => "und".to_string(),
    }
}

/// What `audio-proxy.json` records (audio-levels.ps1 lines 410 to 443; design 7.6; call 22; R6-phase0-60; T5-R17).
/// Def: T5-16. Use: T5-17.
#[derive(Clone, Debug, PartialEq)]
pub struct ProxyRecord {
    /// The job ID (design 7.6).
    pub slug: String,
    /// The title key.
    pub label: String,
    /// "yyyy-MM-dd HH:mm:ss", the pass's start.
    pub created: String,
    /// The source file.
    pub source: ProxySource,
    /// The pass.
    pub pass: ProxyPass,
    /// -18, or -30 after the redo.
    pub gain_db: i32,
    /// None: no proxy kept (call 22).
    pub proxy: Option<ProxyFile>,
    /// One entry per proxy track, in track order.
    pub tracks: Vec<ProxyTrackFacts>,
    /// [-18], or [-18, -30].
    pub gains_tried: Vec<i32>,
}

/// Part of ProxyRecord (audio-levels.ps1 lines 411 to 443). Def: T5-16. Use: T5-17.
#[derive(Clone, Debug, PartialEq)]
pub struct ProxySource {
    /// The file's name.
    pub file: String,
    /// Its size.
    pub bytes: u64,
    /// Its last write time, "yyyy-MM-dd HH:mm:ss".
    pub modified: String,
    /// ffprobe's format duration as written.
    pub duration_s: String,
    /// How many audio streams the file has.
    pub audio_streams: usize,
}

/// Part of ProxyRecord (audio-levels.ps1 lines 411 to 443). Def: T5-16. Use: T5-17.
#[derive(Clone, Debug, PartialEq)]
pub struct ProxyPass {
    /// The pass's whole seconds.
    pub seconds: i64,
    /// The first line of `ffmpeg -version`.
    pub ffmpeg: String,
    /// The level folder's name.
    pub levels_folder: String,
}

/// Part of ProxyRecord (audio-levels.ps1 lines 411 to 443). Def: T5-16. Use: T5-17.
#[derive(Clone, Debug, PartialEq)]
pub struct ProxyFile {
    /// audio-proxy.mka's size.
    pub bytes: u64,
    /// Its track count.
    pub tracks: usize,
}

/// Part of ProxyRecord (audio-levels.ps1 lines 411 to 443). Def: T5-16. Use: T5-17.
#[derive(Clone, Debug, PartialEq)]
pub struct ProxyTrackFacts {
    /// The track's position k.
    pub track: usize,
    /// Its source stream's index.
    pub source_stream: i64,
    /// `mix`, `fc` or `side`.
    pub series: String,
    /// The DURATION tag in seconds.
    pub duration_s: f64,
    /// The last running peak, as written.
    pub peak_dbfs: String,
    /// The source stream's facts.
    pub source: ProxySourceStream,
}

/// Part of ProxyRecord (audio-levels.ps1 lines 411 to 443). Def: T5-16. Use: T5-17.
#[derive(Clone, Debug, PartialEq)]
pub struct ProxySourceStream {
    /// codec_name.
    pub codec: String,
    /// profile.
    pub profile: String,
    /// The channel count.
    pub channels: i64,
    /// channel_layout.
    pub layout: String,
    /// The layout ffmpeg guesses, "" when none.
    pub layout_guessed: String,
    /// The language tag.
    pub language: String,
    /// The title tag.
    pub title: String,
    /// sample_rate as written.
    pub sample_rate: String,
    /// sample_fmt.
    pub sample_fmt: String,
    /// bits_per_raw_sample as written.
    pub bits_per_raw_sample: String,
    /// start_time as written.
    pub start_time: String,
    /// The stream's packet MD5.
    pub md5: String,
}

/// `audio-proxy.json`'s text: pretty JSON (two-space indent, `serde_json::to_string_pretty`) and a final "\n", keys in
/// phase 0's order then `gains_tried` last. Def: T5-16. Use: T5-17.
pub fn proxy_json(record: &ProxyRecord) -> String {
    let gain = record.gain_db;
    let proxy = match &record.proxy {
        None => Value::Null,
        Some(p) => json!({"file": "audio-proxy.mka", "bytes": p.bytes, "tracks": p.tracks}),
    };
    let tracks: Vec<Value> = record
        .tracks
        .iter()
        .map(|t| {
            let s = &t.source;
            json!({
                "track": t.track,
                "source_stream": t.source_stream,
                "series": t.series,
                "duration_s": (t.duration_s * 1000.0).round_ties_even() / 1000.0,
                "peak_dbfs": t.peak_dbfs,
                "source": {
                    "codec": s.codec, "profile": s.profile, "channels": s.channels, "layout": s.layout,
                    "layout_guessed": s.layout_guessed, "language": s.language, "title": s.title,
                    "sample_rate": s.sample_rate, "sample_fmt": s.sample_fmt,
                    "bits_per_raw_sample": s.bits_per_raw_sample, "start_time": s.start_time, "md5": s.md5
                }
            })
        })
        .collect();
    let value = json!({
        "format": PROXY_FORMAT,
        "slug": record.slug,
        "label": record.label,
        "created": record.created,
        "source": {
            "file": record.source.file, "bytes": record.source.bytes, "modified": record.source.modified,
            "duration_s": record.source.duration_s, "audio_streams": record.source.audio_streams
        },
        "pass": {"seconds": record.pass.seconds, "ffmpeg": record.pass.ffmpeg, "levels_folder": record.pass.levels_folder},
        "encoding": {
            "codec": "flac", "sample_rate": 8000, "channels": 1, "bits": 24, "compression_level": 5, "gain_db": gain,
            "mix": "aformat=channel_layouts=mono on the decoded stream, as in the level chain",
            "fc": "pan=mono|c0=FC, for every stream whose layout has FC besides other channels",
            "side": "pan=mono|c0=0.5*c0-0.5*c1, for every two-channel stream (the mix is 0.5*c0+0.5*c1)",
            "chain": format!("aresample=8000,volume={gain}dB,aformat=sample_fmts=s32"),
            "read": format!("add {} dB in float, convert to the source stream's sample_fmt, then measure (lazymkv-dev levels-from-proxy)", -gain)
        },
        "proxy": proxy,
        "not_kept": "video and subtitle streams; channels other than the mixdown, the centre and a stereo pair's side signal; sound above 4 kHz; object-audio metadata; the packets themselves (their MD5 is kept)",
        "tracks": tracks,
        "gains_tried": record.gains_tried
    });
    serde_json::to_string_pretty(&value).expect("a JSON value serializes") + "\n"
}

#[cfg(test)]
mod tests;
