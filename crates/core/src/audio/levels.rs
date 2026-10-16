//! A level folder written by audio-levels.ps1 (experiment 13), read as audio_similarity.py reads it: manifest.txt, the
//! level files (one speech-band RMS level in dB per 20 ms frame), streams.json and audio-proxy.json, and the refusal
//! of a folder the analysis must not measure. Text in, values out: the caller's Tree supplies the bytes.

use super::measures::py_sorted;
use crate::recording::files::FfprobeStreams;
use crate::recording::{
    py_float, py_g, py_repr_str, py_split, py_splitlines, py_strip, read_text, universal_lines, Encoding, Errors,
    Ordered, PathKey, Tree,
};

/// dB; digital silence. Py: FLOOR.
pub const FLOOR: f64 = -100.0;
/// One level per 20 ms. Py: FRAME_MS.
pub const FRAME_MS: i64 = 20;
/// The same, in seconds. Py: FRAME_S.
pub const FRAME_S: f64 = 0.02;
/// The first line of manifest.txt starts with this. Py: MANIFEST_LINE.
pub const MANIFEST_LINE: &str = "# lazymkv audio-levels v2";
/// audio-proxy.json's format. Py: proxy_levels.PROXY_FORMAT.
pub const PROXY_FORMAT: &str = "lazymkv audio proxy v1";

/// The key of a level line in ffmpeg's raw ametadata output. Py: read_levels.
const RMS_KEY: &str = "lavfi.astats.Overall.RMS_level=";
/// The end of every refusal, verbatim. Py: refusal's `again`.
const AGAIN: &str = "this folder cannot be analysed: measure the title again with the new audio-levels.ps1, which writes audio-levels-20ms-<main>-<candidates>";

/// One manifest line's key=value pairs, in line order (dict semantics). Py: read_manifest's `fields`.
pub type Fields = Ordered<String>;

/// manifest.txt: the main stream's line, the candidates' lines in file order, and every other line's pairs merged
/// (info.update). Py: read_manifest's dict {"main", "cands", ...}.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Manifest {
    pub main: Option<Fields>,
    pub cands: Vec<Fields>,
    pub info: Fields,
}

/// A level series: one dB level per 20 ms frame, floored at FLOOR.
pub type Levels = Vec<f64>;
/// streams.json (a level folder's ffprobe stream list).
pub type Streams = FfprobeStreams;

/// audio-proxy.json, the parts proxy_levels reads; the objects are kept as JSON because the Python tests their keys'
/// presence. `encoding.read` may name proxy_levels.py or lazymkv-dev levels-from-proxy: it is never checked.
/// Py: load_proxy_json, build, proxy_streams.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct AudioProxy {
    pub format: Option<serde_json::Value>,
    /// file, bytes, audio_streams, ...
    #[serde(default)]
    pub source: serde_json::Map<String, serde_json::Value>,
    /// gain_db, ...
    #[serde(default)]
    pub encoding: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub tracks: Vec<ProxyTrack>,
}

/// One entry of audio-proxy.json's "tracks".
#[derive(Clone, Debug, serde::Deserialize)]
pub struct ProxyTrack {
    pub track: Option<i64>,
    /// An int in a good file (proxy_streams checks isinstance(..., int)); Null when the key is absent.
    #[serde(default)]
    pub source_stream: serde_json::Value,
    pub series: Option<String>,
    /// codec, profile, channels, layout, layout_guessed, language, title, sample_fmt, md5, ...
    pub source: Option<serde_json::Map<String, serde_json::Value>>,
}

/// Parses manifest.txt's text: lines by py_splitlines, '#' lines and lines without '=' skipped, pairs from the
/// py_split words holding '=' (split at the first '='); a line with a "main" key is the main stream, one with a
/// "cand" key a candidate, any other line updates `info`. Py: read_manifest.
pub fn parse_manifest(text: &str) -> Manifest {
    let mut man = Manifest::default();
    for line in py_splitlines(text) {
        if line.starts_with('#') || !line.contains('=') {
            continue;
        }
        let mut fields: Fields = Ordered::new();
        for word in py_split(line) {
            if let Some((key, value)) = word.split_once('=') {
                fields.insert(key.to_string(), value.to_string());
            }
        }
        if fields.contains_key("main") {
            man.main = Some(fields);
        } else if fields.contains_key("cand") {
            man.cands.push(fields);
        } else {
            for (key, value) in fields.0 {
                man.info.insert(key, value);
            }
        }
    }
    man
}

/// `<folder>/manifest.txt` read as utf-8-sig with replacement and parsed; Manifest::default() when it does not exist
/// (the Python returns its empty dict). Py: read_manifest.
pub fn read_manifest(tree: &dyn Tree, folder: &PathKey) -> Result<Manifest, String> {
    let path = folder.join("manifest.txt");
    if !tree.exists(&path) {
        return Ok(Manifest::default());
    }
    Ok(parse_manifest(&read_text(tree, &path, Encoding::Utf8Sig, Errors::Replace)?))
}

/// Levels in dB, one per frame: compact files (blank, '#' and "frame:" lines skipped) and raw ametadata output (only
/// "lavfi.astats.Overall.RMS_level=" lines among the lines holding '='); a value holding "inf" or "nan" is FLOOR,
/// any other is max(FLOOR, float(value)). Lines by py_splitlines, each py_strip'ed. Err is Python's float() text.
/// Py: read_levels.
pub fn parse_levels(text: &str) -> Result<Levels, String> {
    let mut out: Levels = Vec::new();
    for raw in py_splitlines(text) {
        let mut line = py_strip(raw);
        if line.is_empty() || line.starts_with('#') || line.starts_with("frame:") {
            continue;
        }
        if line.contains('=') {
            if !line.starts_with(RMS_KEY) {
                continue;
            }
            line = line.split_once('=').map_or("", |(_, value)| value);
        }
        if line.contains("inf") || line.contains("nan") {
            out.push(FLOOR);
            continue;
        }
        let value = py_float(line)
            .ok_or_else(|| format!("could not convert string to float: {}", py_repr_str(line)))?;
        // Python's max(FLOOR, value): FLOOR unless value is greater
        out.push(if value > FLOOR { value } else { FLOOR });
    }
    Ok(out)
}

/// A level file read as utf-8-sig with replacement and parsed. Py: read_levels(path).
pub fn read_levels(tree: &dyn Tree, path: &PathKey) -> Result<Levels, String> {
    parse_levels(&read_text(tree, path, Encoding::Utf8Sig, Errors::Replace)?)
}

/// The frame length a level file states, in ms: float() of "frame_ms=" in a first line starting with '#' (None when
/// that word does not parse), else the median spacing of the first 50 "pts_time:" values (lines from universal_lines,
/// as Python iterates a text file; at most 201 lines read) times 1000, rounded half to even; None when neither is
/// there. Py: frame_ms_of.
pub fn frame_ms_of(text: &str) -> Option<f64> {
    let mut times: Vec<f64> = Vec::new();
    for (k, line) in universal_lines(text).into_iter().enumerate() {
        if k == 0 && line.starts_with('#') {
            for word in py_split(line) {
                if let Some(rest) = word.strip_prefix("frame_ms=") {
                    return py_float(rest);
                }
            }
        }
        if let Some((_, after)) = line.split_once("pts_time:") {
            if let Some(first) = py_split(after).first() {
                if let Some(t) = py_float(first) {
                    times.push(t);
                }
            }
        }
        if times.len() >= 50 || k >= 200 {
            break;
        }
    }
    let steps: Vec<f64> = times.windows(2).map(|pair| pair[1] - pair[0]).collect();
    if steps.is_empty() {
        return None;
    }
    let steps = py_sorted(&steps);
    Some((steps[steps.len() / 2] * 1000.0).round_ties_even())
}

/// "" when `folder` was written by the experiment-13 audio-levels.ps1 (manifest.txt beginning with MANIFEST_LINE,
/// frame_ms=20, a main stream, 20 ms frames in main-mix.txt, in main-fc.txt when it exists and in every cand-<n>.txt),
/// else the Python's one-line reason, beginning with tree.display(folder). Err only when a present file cannot be
/// read. Py: refusal.
pub fn refusal(tree: &dyn Tree, folder: &PathKey) -> Result<String, String> {
    let d = tree.display(folder);
    let manifest = folder.join("manifest.txt");
    if !tree.is_file(&manifest) {
        return Ok(format!("{d}: no manifest.txt, so it was not written by the new audio-levels.ps1 (the first measurement's folders, audio-levels-<main>-<cand>, hold 100 ms full-band levels); {AGAIN}"));
    }
    let text = read_text(tree, &manifest, Encoding::Utf8Sig, Errors::Replace)?;
    let lines = py_splitlines(&text);
    if lines.first().map_or(true, |first| !first.starts_with(MANIFEST_LINE)) {
        return Ok(format!("{d}: manifest.txt does not begin with '{MANIFEST_LINE}'; {AGAIN}"));
    }
    let man = parse_manifest(&text);
    let frame_ms = man.info.get("frame_ms").map(String::as_str);
    if frame_ms != Some("20") {
        // str(FRAME_MS) is "20"
        let shown = frame_ms.unwrap_or("(none)");
        return Ok(format!("{d}: manifest.txt gives frame_ms={shown}, not {FRAME_MS}; {AGAIN}"));
    }
    if man.main.as_ref().map_or(true, |main| main.is_empty()) {
        return Ok(format!("{d}: manifest.txt names no main stream; {AGAIN}"));
    }
    let mut names: Vec<String> = vec!["main-mix.txt".to_string()];
    if tree.exists(&folder.join("main-fc.txt")) {
        names.push("main-fc.txt".to_string());
    }
    for cand in &man.cands {
        names.push(format!("cand-{}.txt", cand.get("cand").map_or("None", String::as_str)));
    }
    for name in &names {
        let path = folder.join(name);
        if !tree.is_file(&path) {
            return Ok(format!("{d}: {name} is missing; {AGAIN}"));
        }
        let ms = frame_ms_of(&read_text(tree, &path, Encoding::Utf8Sig, Errors::Replace)?);
        if ms != Some(FRAME_MS as f64) {
            let held = match ms {
                None => "frames of unknown length".to_string(),
                Some(v) => format!("{} ms frames", py_g(v)),
            };
            return Ok(format!("{d}: {name} holds {held}, not {FRAME_MS} ms; {AGAIN}"));
        }
    }
    Ok(String::new())
}

/// streams.json's text. Py: read_streams' json.loads(...).get("streams", []).
pub fn parse_streams(text: &str) -> Result<Streams, String> {
    serde_json::from_str(text).map_err(|e| e.to_string())
}

/// audio-proxy.json's text (the caller reads it with read_text, utf-8-sig). Py: load_proxy_json's json.loads.
pub fn parse_proxy(text: &str) -> Result<AudioProxy, String> {
    serde_json::from_str(text).map_err(|e| e.to_string())
}

#[cfg(test)]
mod reading {
    //! Ported from test_audio_similarity.py::Reading (2 tests).
    use super::*;
    use crate::recording::MemTree;

    #[test]
    fn compact_and_raw_level_files_are_read_and_silence_is_the_floor() {
        let mut tree = MemTree::new("tmp", "tmp");
        tree.insert_file("main-mix.txt", "# lazymkv audio levels v2\n# stream=1 kind=main-mix\n-45.3\n-100.0\n-12.0\n");
        assert_eq!(read_levels(&tree, &PathKey::new("main-mix.txt")).unwrap(), vec![-45.3, -100.0, -12.0]);
        tree.insert_file(
            "raw.txt",
            "frame:0    pts:0       pts_time:0\nlavfi.astats.Overall.RMS_level=-inf\n\
             frame:1    pts:160     pts_time:0.02\nlavfi.astats.Overall.RMS_level=-123.5\n\
             frame:2    pts:320     pts_time:0.04\nlavfi.astats.Overall.RMS_level=-23.5\n",
        );
        assert_eq!(read_levels(&tree, &PathKey::new("raw.txt")).unwrap(), vec![FLOOR, FLOOR, -23.5]);
    }

    #[test]
    fn manifest_lists_the_main_and_the_candidates() {
        let mut tree = MemTree::new("tmp", "tmp");
        tree.insert_file(
            "manifest.txt",
            "# lazymkv audio-levels v2\nfile=x.mkv\n\
             main=1 codec=dts channels=6 layout=5.1(side) language=eng fc=yes md5=aa\n\
             cand=3 codec=ac3 channels=2 layout=stereo language=eng md5=bb\ncand=4 codec=dts channels=6 md5=aa\n",
        );
        let man = read_manifest(&tree, &PathKey::root()).unwrap();
        assert_eq!(man.main.as_ref().unwrap().get("main").map(String::as_str), Some("1"));
        let cands: Vec<&str> = man.cands.iter().map(|c| c.get("cand").unwrap().as_str()).collect();
        assert_eq!(cands, vec!["3", "4"]);
        assert_eq!(man.info.get("file").map(String::as_str), Some("x.mkv"));
    }
}

#[cfg(test)]
mod tests {
    //! New tests: the refusal's reasons, frame_ms_of, the level and manifest lines, and the two JSON readers, each
    //! against what Python 3.9.6 gave for the same input.
    use super::*;
    use crate::recording::MemTree;
    use crate::testsupport::raw_file_text;

    /// The end of every refusal, verbatim. Py: refusal's `again`.
    const TAIL: &str = "; this folder cannot be analysed: measure the title again with the new audio-levels.ps1, which writes audio-levels-20ms-<main>-<candidates>";
    /// A compact level file's header line, as audio-levels.ps1 writes it.
    const HEAD: &str = "# lazymkv audio levels v2: frame_ms=20 band_hz=300-3400 unit=dB floor=-100 series=x file=x.mkv\n";
    /// A manifest the refusal accepts: main stream 1, candidate 3.
    const GOOD: &str = "# lazymkv audio-levels v2 (experiment 13)\nfile=x.mkv frame_ms=20\nmain=1 md5=aa\ncand=3 md5=bb\n";

    fn kv(pairs: &[(&str, &str)]) -> Fields {
        Ordered(pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect())
    }

    #[test]
    fn refusal_gives_the_pythons_reasons() {
        let lv = format!("{HEAD}-20.0\n-30.0\n");
        let lv = lv.as_str();
        let mut tree = MemTree::new("rec", "rec");
        tree.insert_dir("a");
        tree.insert_file("b/manifest.txt", "# lazymkv audio-levels v1\nframe_ms=20\n");
        tree.insert_file("c/manifest.txt", "# lazymkv audio-levels v2 (experiment 13)\nfile=x.mkv\nmain=1 md5=aa\n");
        tree.insert_file(
            "d/manifest.txt",
            "# lazymkv audio-levels v2 (experiment 13)\nfile=x.mkv frame_ms=40\nmain=1 md5=aa\n",
        );
        tree.insert_file("e/manifest.txt", "# lazymkv audio-levels v2 (experiment 13)\nframe_ms=20\ncand=3 md5=bb\n");
        tree.insert_file("f/manifest.txt", GOOD);
        tree.insert_file("f/cand-3.txt", lv);
        tree.insert_file("g/manifest.txt", GOOD);
        tree.insert_file("g/main-mix.txt", lv);
        tree.insert_file("g/cand-3.txt", raw_file_text(&[-20.0; 5], 0.1));
        tree.insert_file("h/manifest.txt", GOOD);
        tree.insert_file("h/main-mix.txt", "-20.0\n-30.0\n");
        tree.insert_file("h/cand-3.txt", lv);
        tree.insert_file("i/manifest.txt", GOOD);
        tree.insert_file("i/main-mix.txt", lv);
        tree.insert_file("i/main-fc.txt", "# x frame_ms=abc\n-20.0\n");
        tree.insert_file("i/cand-3.txt", lv);
        tree.insert_file("j/manifest.txt", GOOD);
        for name in ["main-mix.txt", "main-fc.txt", "cand-3.txt"] {
            tree.insert_file(&format!("j/{name}"), lv);
        }
        tree.insert_file("k/manifest.txt", GOOD);
        tree.insert_file("k/main-mix.txt", "# h frame_ms=20.0\n-1\n");
        tree.insert_file("k/cand-3.txt", "# h frame_ms=25\n-1\n");
        tree.insert_file("l/manifest.txt", format!("\u{feff}{}", GOOD.replace('\n', "\r\n")));
        tree.insert_file("l/main-mix.txt", lv);
        tree.insert_file("l/cand-3.txt", lv);
        tree.insert_file("m/manifest.txt", "");
        let why = |name: &str| refusal(&tree, &PathKey::new(name)).expect("every present file is readable");
        assert_eq!(
            why("a"),
            format!("rec/a: no manifest.txt, so it was not written by the new audio-levels.ps1 (the first measurement's folders, audio-levels-<main>-<cand>, hold 100 ms full-band levels){TAIL}")
        );
        assert_eq!(why("b"), format!("rec/b: manifest.txt does not begin with '# lazymkv audio-levels v2'{TAIL}"));
        assert_eq!(why("c"), format!("rec/c: manifest.txt gives frame_ms=(none), not 20{TAIL}"));
        assert_eq!(why("d"), format!("rec/d: manifest.txt gives frame_ms=40, not 20{TAIL}"));
        assert_eq!(why("e"), format!("rec/e: manifest.txt names no main stream{TAIL}"));
        assert_eq!(why("f"), format!("rec/f: main-mix.txt is missing{TAIL}"));
        assert_eq!(why("g"), format!("rec/g: cand-3.txt holds 100 ms frames, not 20 ms{TAIL}"));
        assert_eq!(why("h"), format!("rec/h: main-mix.txt holds frames of unknown length, not 20 ms{TAIL}"));
        assert_eq!(why("i"), format!("rec/i: main-fc.txt holds frames of unknown length, not 20 ms{TAIL}"));
        assert_eq!(why("j"), "");
        assert_eq!(why("k"), format!("rec/k: cand-3.txt holds 25 ms frames, not 20 ms{TAIL}"));
        assert_eq!(why("l"), "", "a byte-order mark and CRLF line ends are accepted");
        assert_eq!(why("m"), format!("rec/m: manifest.txt does not begin with '# lazymkv audio-levels v2'{TAIL}"));
    }

    #[test]
    fn frame_ms_of_reads_the_header_or_the_pts_spacing() {
        assert_eq!(frame_ms_of("#frame_ms=20\n"), None);
        assert_eq!(
            frame_ms_of("# a b\nframe:0 pts_time:0\nx pts_time:0.02\ny pts_time:\nz pts_time:0.04 q\n"),
            Some(20.0)
        );
        assert!(frame_ms_of("# frame_ms=nan\n").unwrap().is_nan());
        assert_eq!(frame_ms_of("# frame_ms=abc\n"), None);
        assert_eq!(frame_ms_of("abc\n"), None);
        assert_eq!(frame_ms_of("pts_time:0\rpts_time:0.025\rpts_time:0.05\r"), Some(25.0));
        assert_eq!(frame_ms_of("pts_time:1\npts_time:1.03\npts_time:1.06\npts_time:1.09\n"), Some(30.0));
        // at most 201 lines are read ...
        assert_eq!(frame_ms_of(&format!("{}pts_time:0\npts_time:0.02\n", "x\n".repeat(201))), None);
        assert_eq!(frame_ms_of(&format!("{}pts_time:0\npts_time:0.02\n", "x\n".repeat(199))), Some(20.0));
        // ... and at most 50 times are kept
        let first: String = (0..50).map(|k| format!("pts_time:{}\n", py_g(k as f64 * 0.04))).collect();
        let later: String = (0..60).map(|k| format!("pts_time:{}\n", py_g(2.0 + k as f64 * 0.02))).collect();
        assert_eq!(frame_ms_of(&(first + &later)), Some(40.0));
    }

    #[test]
    fn level_lines_are_read_as_the_python_reads_them() {
        let text = "x=1\nlavfi.astats.Overall.RMS_level= -12.25 \nframe:3\n  \n-nan\nInfinity\n1_0.5\n\x0c-7.5\n";
        assert_eq!(parse_levels(text).unwrap(), vec![-12.25, FLOOR, f64::INFINITY, 10.5, -7.5]);
        assert_eq!(parse_levels("# h\n-20.0\nabc\n"), Err("could not convert string to float: 'abc'".to_string()));
        let mut tree = MemTree::new("rec", "rec");
        tree.insert_file("bom.txt", "\u{feff}# h\r\n-20.5\r\n-100.0\r\n");
        assert_eq!(read_levels(&tree, &PathKey::new("bom.txt")).unwrap(), vec![-20.5, FLOOR]);
        assert_eq!(
            read_levels(&tree, &PathKey::new("none.txt")),
            Err("[Errno 2] No such file or directory: 'rec/none.txt'".to_string())
        );
    }

    #[test]
    fn manifest_lines_are_python_dicts() {
        let man = parse_manifest(
            "# lazymkv audio-levels v2\nfile=a.mkv seconds=3\nmain=1 codec=dts main=2 md5=aa x\ncand=3 codec=ac3\n\
             seconds=9 file=b.mkv extra=1\n# cand=9\nnovalue\ncand=4 a=b=c\n",
        );
        // a repeated key keeps its first place and takes the last value; words without '=' are left out
        assert_eq!(man.main, Some(kv(&[("main", "2"), ("codec", "dts"), ("md5", "aa")])));
        assert_eq!(man.cands, vec![kv(&[("cand", "3"), ("codec", "ac3")]), kv(&[("cand", "4"), ("a", "b=c")])]);
        // every other line updates one dict: file keeps its place and takes b.mkv
        assert_eq!(man.info, kv(&[("file", "b.mkv"), ("seconds", "9"), ("extra", "1")]));
    }

    #[test]
    fn a_folder_without_manifest_txt_reads_as_an_empty_manifest() {
        let mut tree = MemTree::new("rec", "rec");
        tree.insert_dir("x");
        assert_eq!(read_manifest(&tree, &PathKey::new("x")).unwrap(), Manifest::default());
    }

    #[test]
    fn streams_json_and_audio_proxy_json_parse() {
        let s = parse_streams(
            r#"{"streams": [{"index": 0, "codec_type": "video"}, {"index": 1, "codec_type": "audio", "codec_name": "ac3", "channels": 6, "tags": {"language": "eng", "title": "x"}}]}"#,
        )
        .unwrap();
        assert_eq!(s.streams.len(), 2);
        assert_eq!(s.streams[1].channels, Some(6));
        assert_eq!(s.streams[1].tags.as_ref().unwrap().get("language").map(String::as_str), Some("eng"));
        assert!(parse_streams("{}").unwrap().streams.is_empty());
        assert!(parse_streams("[").is_err());
        let p = parse_proxy(
            r#"{"format": "lazymkv audio proxy v1", "source": {"file": "x.mkv", "bytes": 1, "audio_streams": 2}, "encoding": {"gain_db": -6.0, "read": "lazymkv-dev levels-from-proxy"}, "tracks": [{"track": 0, "source_stream": 1, "series": "mix", "source": {"codec": "dts", "channels": 6, "language": "eng"}}, {"track": 1, "series": "fc"}]}"#,
        )
        .unwrap();
        assert_eq!(p.format, Some(serde_json::Value::from(PROXY_FORMAT)));
        assert_eq!(p.source.get("audio_streams"), Some(&serde_json::Value::from(2)));
        assert_eq!(p.encoding.get("gain_db"), Some(&serde_json::Value::from(-6.0)));
        assert_eq!(p.tracks.len(), 2);
        assert_eq!(p.tracks[0].source_stream, serde_json::Value::from(1));
        assert_eq!(p.tracks[1].source_stream, serde_json::Value::Null);
        assert_eq!(p.tracks[1].series.as_deref(), Some("fc"));
        assert!(p.tracks[1].source.is_none());
    }
}
