//! The streams the audio rule reads and the stream it judges against: spec 8.6's ranking (step 3) with its guards G1
//! and G2 (step 4) as far as ffprobe's facts go, the programme reference of EXP-13 addendum 9.1, and the readers of a
//! level folder's stream facts (its manifest lines and its streams.json).
//! Ported from scripts/phase0/audio_similarity.py (is_lossless, rank, placed_first, ffprobe_audio, read_streams,
//! manifest_stream) and scripts/phase0/audio_dataset.py (original_language, programme_reference, wanted,
//! first_of_language).

use crate::audio::levels::{parse_streams, Fields};
use crate::audio::rule::first_of_language as rule_first_of_language;
use crate::lang::{iso639, NO_LANGUAGE, TMDB_NO_LANGUAGE};
use crate::recording::files::FfprobeStream;
use crate::recording::{
    py_int, py_repr, py_repr_str, py_truthy, read_json, read_text, Encoding, Errors, PathKey, Tree,
};

/// A stream as placed_first takes it: the Python's tuple (stream, channels, lossless, language, description).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioStream {
    pub stream: i64,
    pub channels: i64,
    pub lossless: bool,
    pub language: String,
    pub description: String,
}

/// Lossless audio as spec 8.6 ranks it: truehd, mlp, flac, pcm*, or a profile holding "DTS-HD MA" once '_' is read
/// as a space (ffprobe names DTS:X tracks "DTS-HD MA + DTS:X"; a level folder's manifest writes "DTS-HD_MA").
/// Py: is_lossless.
pub fn is_lossless(codec: Option<&str>, profile: Option<&str>) -> bool {
    let c = codec.unwrap_or("");
    matches!(c, "truehd" | "mlp" | "flac")
        || c.starts_with("pcm")
        || profile.unwrap_or("").replace('_', " ").contains("DTS-HD MA")
}

/// Spec 8.6 step 3's sort key: most channels, then lossless, then the earlier stream. Py: rank.
pub fn rank(a: &AudioStream) -> (i64, bool, i64) {
    (-a.channels, !a.lossless, a.stream)
}

/// The stream spec 8.6 places first among the given streams of one language: rank (step 3), then its guards (step
/// 4) as far as ffprobe's facts go. G1: a lossy stream of two channels or fewer (the spec says 256 kb/s or less;
/// streams.json carries no bit rate) never comes before a lossless one. G2: a lossy stream of two channels or fewer
/// that is not the language's first stream and has more channels than that first stream gives way to it.
/// `streams` is never empty: the Python indexes [0], and this panics on an empty slice where the Python raised
/// IndexError. Py: placed_first.
pub fn placed_first(streams: &[AudioStream]) -> AudioStream {
    let mut ranked: Vec<&AudioStream> = streams.iter().collect();
    ranked.sort_by_key(|a| rank(a)); // stable, as sorted()
    let top = *ranked.first().expect("placed_first takes at least one stream");
    if !top.lossless && top.channels <= 2 {
        if let Some(lossless) = ranked.iter().find(|a| a.lossless) {
            return AudioStream::clone(lossless);
        }
        // min() keeps the first of equal stream numbers, and so does min_by_key.
        if let Some(first) = streams.iter().min_by_key(|a| a.stream) {
            if first.stream != top.stream && first.channels < top.channels {
                return first.clone();
            }
        }
    }
    top.clone()
}

/// The audio streams of an ffprobe stream list (a level folder's streams.json, a rip's ffprobe-streams.json) in file
/// order, in the form placed_first takes: the index, the channels (0 when absent), is_lossless of codec and profile,
/// the language tag ("und" when absent or empty), and a description joining the codec, the profile, "<channels> ch"
/// ("None ch" when ffprobe gives no channels), the layout and the title tag, each left out when absent or empty.
/// A codec_name that is absent or null is "" (ffprobe always writes it for an audio stream). Py: ffprobe_audio.
pub fn ffprobe_audio(streams: &[FfprobeStream]) -> Vec<AudioStream> {
    let mut out = Vec::new();
    for s in streams {
        if s.codec_type.as_deref() != Some("audio") {
            continue;
        }
        let tag = |key: &str| s.tags.as_ref().and_then(|tags| tags.get(key)).map(String::as_str);
        let codec = s.codec_name.as_deref().unwrap_or("");
        let channels = format!("{} ch", s.channels.map_or_else(|| "None".to_string(), |n| n.to_string()));
        let what = [Some(codec), s.profile.as_deref(), Some(channels.as_str()), s.channel_layout.as_deref(), tag("title")];
        let description = what.iter().flatten().filter(|w| !w.is_empty()).copied().collect::<Vec<&str>>().join(" ");
        out.push(AudioStream {
            stream: s.index,
            channels: s.channels.unwrap_or(0),
            lossless: is_lossless(Some(codec), s.profile.as_deref()),
            language: tag("language").filter(|l| !l.is_empty()).unwrap_or("und").to_string(),
            description,
        });
    }
    out
}

/// The stream list of a level folder's streams.json (ffprobe's list of the file's streams), read as utf-8-sig; None
/// when the folder has no such file; Err with the read or parse error. Py: read_streams.
pub fn read_streams(tree: &dyn Tree, folder: &PathKey) -> Result<Option<Vec<FfprobeStream>>, String> {
    let path = folder.join("streams.json");
    if !tree.is_file(&path) {
        return Ok(None);
    }
    let text = read_text(tree, &path, Encoding::Utf8Sig, Errors::Strict)?;
    Ok(Some(parse_streams(&text)?.streams))
}

/// A manifest line's stream in the form placed_first takes: the cand= value, else the main= value (`cand or main`),
/// as an int; channels= as an int, 0 when it is not one; is_lossless of codec= and profile=; language=, "und" when
/// absent or empty; the description "<codec, ? when absent> <channels> ch". Err with the Python's int() text when the
/// stream number is not an int or absent. Py: manifest_stream.
pub fn manifest_stream(info: &Fields) -> Result<AudioStream, String> {
    let n = match info.get("cand") {
        Some(cand) if !cand.is_empty() => Some(cand.as_str()),
        _ => info.get("main").map(String::as_str),
    };
    // int(info.get("channels") or 0), 0 on ValueError
    let channels = info.get("channels").and_then(|text| py_int(text)).unwrap_or(0);
    let codec = info.get("codec").map(String::as_str);
    Ok(AudioStream {
        stream: int_text(n)?,
        channels,
        lossless: is_lossless(codec, info.get("profile").map(String::as_str)),
        language: info.get("language").filter(|l| !l.is_empty()).cloned().unwrap_or_else(|| "und".to_string()),
        description: format!("{} {channels} ch", codec.unwrap_or("?")),
    })
}

/// (codes, what): the three-letter codes of the work's original language from <slug_dir>/tmdb.json (read as
/// utf-8-sig), and a phrase saying how it was found. codes is empty when the original language is English, none or
/// unknown. Py: audio_dataset.original_language; its phrases verbatim (the "audio_similarity.ISO639" in one of them
/// included: levels-args prints it).
pub fn original_language(tree: &dyn Tree, slug_dir: &PathKey) -> (&'static [&'static str], String) {
    const UNREADABLE: &str = "original language unknown (no readable tmdb.json)";
    // except (OSError, ValueError, AttributeError): no file, not UTF-8, not JSON, not a JSON object
    let Ok(data) = read_json::<serde_json::Value>(tree, &slug_dir.join("tmdb.json")) else {
        return (&[], UNREADABLE.to_string());
    };
    let Some(object) = data.as_object() else {
        return (&[], UNREADABLE.to_string());
    };
    let code = match object.get("original_language") {
        Some(code) if py_truthy(code) => code,
        _ => return (&[], "original language unknown (tmdb.json gives none)".to_string()),
    };
    let Some(code) = code.as_str() else {
        // A number or true: the Python's `code not in ISO639` holds and {code!r} is its repr. (A list or an object
        // made the Python raise TypeError; it gets the same sentence here.)
        return (&[], format!("original language unknown (TMDb {} is not in audio_similarity.ISO639)", py_repr(code)));
    };
    if code == "en" {
        return (&[], "original language English (TMDb en)".to_string());
    }
    if code == TMDB_NO_LANGUAGE {
        return (
            &[],
            format!("original language none (TMDb {code}: a work without speech; English is the reference language)"),
        );
    }
    match iso639(code) {
        None => (
            &[],
            format!("original language unknown (TMDb {} is not in audio_similarity.ISO639)", py_repr_str(code)),
        ),
        Some(codes) => (codes, format!("original language {} (TMDb {code})", codes.join("/"))),
    }
}

/// (stream, why): the title's programme reference (EXP-13 addendum 9.1): the stream spec 8.6 places first in the
/// work's original language when the title has one in it ("the original language"), else in English ("English"),
/// else among all streams ("any language (no stream is in a wanted language)": measured for the dataset only).
/// `audio` is never empty (placed_first). Py: audio_dataset.programme_reference.
pub fn programme_reference(audio: &[AudioStream], original: &[&str]) -> (AudioStream, &'static str) {
    let english: &[&str] = &["eng"];
    for (codes, why) in [(original, "the original language"), (english, "English")] {
        let mine: Vec<AudioStream> =
            audio.iter().filter(|a| codes.iter().any(|code| *code == a.language)).cloned().collect();
        if !mine.is_empty() {
            return (placed_first(&mine), why);
        }
    }
    (placed_first(audio), "any language (no stream is in a wanted language)")
}

/// A stream measured as a candidate: in English or the original language (R4), or with no language code (the app
/// does not judge those; phase 0 measures them for the dataset). Py: audio_dataset.wanted.
pub fn wanted(a: &AudioStream, original: &[&str]) -> bool {
    a.language == "eng"
        || original.iter().any(|code| *code == a.language)
        || NO_LANGUAGE.iter().any(|code| *code == a.language)
}

/// rule::first_of_language over a streams.json, as (stream, description); None without streams.json (`streams or
/// []`). It is the condition of the position veto (row 0, D21). Py: audio_dataset.first_of_language.
pub fn first_of_language(streams: Option<&[FfprobeStream]>, main: i64) -> Option<(i64, String)> {
    let audio = ffprobe_audio(streams.unwrap_or(&[]));
    rule_first_of_language(&audio, main).map(|first| (first.stream, first.description))
}

/// int() of a manifest value as the Python calls it: its ValueError text for a value that is not an int, its
/// TypeError text for a key that is absent (None).
fn int_text(text: Option<&str>) -> Result<i64, String> {
    match text {
        None => Err("int() argument must be a string, a bytes-like object or a number, not 'NoneType'".to_string()),
        Some(text) => {
            py_int(text).ok_or_else(|| format!("invalid literal for int() with base 10: {}", py_repr_str(text)))
        }
    }
}

#[cfg(test)]
mod tests;
