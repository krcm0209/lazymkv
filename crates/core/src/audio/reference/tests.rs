//! Tests of core::audio::reference, all new: the ported Python tests reach these functions only through results
//! (rule/tests.rs) and levels_args (Task 8). Every expected value is what the Python function returns for the same
//! input (audio_similarity.py and audio_dataset.py at the port's commit).

use crate::audio::levels::Fields;
use crate::audio::reference::{
    ffprobe_audio, first_of_language, is_lossless, manifest_stream, original_language, placed_first,
    programme_reference, rank, read_streams, wanted, AudioStream,
};
use crate::recording::files::FfprobeStream;
use crate::recording::{MemTree, Ordered, PathKey};

/// A stream as placed_first takes it: (stream, channels, lossless, language, description).
fn s(stream: i64, channels: i64, lossless: bool, language: &str, description: &str) -> AudioStream {
    AudioStream { stream, channels, lossless, language: language.to_string(), description: description.to_string() }
}

/// A manifest line's fields, in order.
fn fields(pairs: &[(&str, &str)]) -> Fields {
    Ordered(pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect())
}

/// One entry of an ffprobe stream list; no tags at all when `tags` is empty.
fn probe(
    index: i64,
    codec_type: &str,
    codec: Option<&str>,
    profile: Option<&str>,
    channels: Option<i64>,
    layout: Option<&str>,
    tags: &[(&str, &str)],
) -> FfprobeStream {
    FfprobeStream {
        index,
        codec_type: Some(codec_type.to_string()),
        codec_name: codec.map(str::to_string),
        profile: profile.map(str::to_string),
        channels,
        channel_layout: layout.map(str::to_string),
        tags: if tags.is_empty() {
            None
        } else {
            Some(Ordered(tags.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()))
        },
    }
}

#[test]
fn is_lossless_names_the_lossless_codecs_and_dts_hd_ma() {
    let cases = [
        (Some("truehd"), None),
        (Some("mlp"), None),
        (Some("flac"), None),
        (Some("pcm_s24le"), None),
        (Some("dts"), Some("DTS-HD MA + DTS:X")),
        (Some("dts"), Some("DTS-HD_MA")), // a manifest's spelling
        (Some("eac3"), Some("DTS-HD MA")),
        (Some("dts"), Some("DTS")),
        (Some("ac3"), None),
        (None, None),
    ];
    assert_eq!(
        cases.map(|(codec, profile)| is_lossless(codec, profile)),
        [true, true, true, true, true, true, true, false, false, false]
    );
}

#[test]
fn rank_and_placed_first_follow_8_6_step_3() {
    assert_eq!(rank(&s(2, 6, false, "eng", "x")), (-6, true, 2));
    // most channels, then lossless, then the earlier stream
    let streams = [s(1, 2, false, "eng", "x"), s(2, 6, false, "eng", "x"), s(3, 6, true, "eng", "x")];
    assert_eq!(placed_first(&streams).stream, 3);
}

#[test]
fn placed_first_applies_the_guards_g1_and_g2() {
    // G1: a lossy stream of two channels or fewer never comes before a lossless one
    assert_eq!(placed_first(&[s(1, 2, false, "eng", "x"), s(2, 1, true, "eng", "x")]).stream, 2);
    // G2: a lossy stereo top that is not the first stream and has more channels than it gives way to it
    assert_eq!(placed_first(&[s(1, 1, false, "eng", "x"), s(2, 2, false, "eng", "x")]).stream, 1);
    assert_eq!(placed_first(&[s(1, 2, false, "eng", "x"), s(2, 2, false, "eng", "x")]).stream, 1);
}

#[test]
fn ffprobe_audio_lists_the_audio_streams_in_file_order_with_their_description() {
    let streams = [
        probe(0, "video", Some("hevc"), None, None, None, &[]),
        probe(1, "audio", Some("truehd"), None, Some(8), Some("7.1"), &[("language", "eng"), ("title", "Main")]),
        probe(2, "audio", Some("ac3"), None, None, None, &[]),
        probe(3, "audio", Some("dts"), Some("DTS-HD MA"), Some(6), None, &[("language", "")]),
        probe(4, "subtitle", Some("hdmv_pgs_subtitle"), None, None, None, &[]),
    ];
    assert_eq!(
        ffprobe_audio(&streams),
        [
            s(1, 8, true, "eng", "truehd 8 ch 7.1 Main"),
            s(2, 0, false, "und", "ac3 None ch"),
            s(3, 6, true, "und", "dts DTS-HD MA 6 ch"),
        ]
    );
}

#[test]
fn read_streams_is_none_without_a_streams_json_file() {
    let mut tree = MemTree::new(".", "lazymkv-test");
    tree.insert_dir("a");
    assert_eq!(read_streams(&tree, &PathKey::new("a")), Ok(None));
    // read as utf-8-sig: a byte-order mark is allowed
    tree.insert_file(
        "b/streams.json",
        "\u{feff}{\"streams\": [{\"index\": 0, \"codec_type\": \"video\"}, {\"index\": 1, \"codec_type\": \"audio\", \"codec_name\": \"ac3\", \"channels\": 2}]}",
    );
    let list = read_streams(&tree, &PathKey::new("b")).expect("it parses").expect("it is there");
    assert_eq!(list.iter().map(|stream| stream.index).collect::<Vec<_>>(), [0, 1]);
    tree.insert_file("c/streams.json", "not json");
    assert!(read_streams(&tree, &PathKey::new("c")).is_err());
    // a folder named streams.json is not the file
    tree.insert_dir("d/streams.json");
    assert_eq!(read_streams(&tree, &PathKey::new("d")), Ok(None));
}

#[test]
fn manifest_stream_reads_a_manifest_line() {
    assert_eq!(
        manifest_stream(&fields(&[("cand", "3"), ("codec", "dts"), ("profile", "DTS-HD_MA"), ("channels", "6"), ("language", "eng")])),
        Ok(s(3, 6, true, "eng", "dts 6 ch"))
    );
    // channels that are not an int count 0; no codec is "?"; no language is "und"
    assert_eq!(manifest_stream(&fields(&[("cand", "4"), ("channels", "x")])), Ok(s(4, 0, false, "und", "? 0 ch")));
    assert_eq!(
        manifest_stream(&fields(&[("main", "1"), ("codec", "ac3"), ("channels", "2"), ("language", "fra")])),
        Ok(s(1, 2, false, "fra", "ac3 2 ch"))
    );
    // info.get("cand") or info.get("main"): an empty cand= falls back to main=
    assert_eq!(manifest_stream(&fields(&[("cand", ""), ("main", "2")])), Ok(s(2, 0, false, "und", "? 0 ch")));
    assert_eq!(
        manifest_stream(&fields(&[("cand", "x")])),
        Err("invalid literal for int() with base 10: 'x'".to_string())
    );
    assert_eq!(
        manifest_stream(&fields(&[])),
        Err("int() argument must be a string, a bytes-like object or a number, not 'NoneType'".to_string())
    );
}

#[test]
fn original_language_says_how_it_was_found() {
    let unreadable = "original language unknown (no readable tmdb.json)";
    let gives_none = "original language unknown (tmdb.json gives none)";
    let cases: [(&str, Option<&str>, &[&str], &str); 12] = [
        ("no-file", None, &[], unreadable),
        ("not-json", Some("not json"), &[], unreadable),
        ("a-list", Some("[]"), &[], unreadable),
        ("no-key", Some("{}"), &[], gives_none),
        ("empty", Some(r#"{"original_language": ""}"#), &[], gives_none),
        ("en", Some(r#"{"original_language": "en"}"#), &[], "original language English (TMDb en)"),
        (
            "xx",
            Some(r#"{"original_language": "xx"}"#),
            &[],
            "original language none (TMDb xx: a work without speech; English is the reference language)",
        ),
        (
            "qq",
            Some(r#"{"original_language": "qq"}"#),
            &[],
            "original language unknown (TMDb 'qq' is not in audio_similarity.ISO639)",
        ),
        ("es", Some(r#"{"original_language": "es"}"#), &["spa"], "original language spa (TMDb es)"),
        ("de", Some(r#"{"original_language": "de"}"#), &["deu", "ger"], "original language deu/ger (TMDb de)"),
        (
            "a-number",
            Some(r#"{"original_language": 5}"#),
            &[],
            "original language unknown (TMDb 5 is not in audio_similarity.ISO639)",
        ),
        ("bom", Some("\u{feff}{\"original_language\": \"ja\"}"), &["jpn"], "original language jpn (TMDb ja)"),
    ];
    let mut tree = MemTree::new(".", "lazymkv-test");
    for (slug, text, codes, what) in cases {
        tree.insert_dir(slug);
        if let Some(text) = text {
            tree.insert_file(&format!("{slug}/tmdb.json"), text);
        }
        let (got_codes, got_what) = original_language(&tree, &PathKey::new(slug));
        assert_eq!((got_codes, got_what.as_str()), (codes, what), "{slug}");
    }
}

#[test]
fn programme_reference_takes_the_original_language_then_english_then_any_language() {
    let audio = [s(1, 6, false, "eng", "ac3 6 ch"), s(2, 2, false, "spa", "ac3 2 ch"), s(3, 6, true, "fra", "truehd 6 ch")];
    assert_eq!(programme_reference(&audio, &["spa"]), (audio[1].clone(), "the original language"));
    assert_eq!(programme_reference(&audio, &[]), (audio[0].clone(), "English"));
    assert_eq!(
        programme_reference(&audio[2..], &["jpn"]),
        (audio[2].clone(), "any language (no stream is in a wanted language)")
    );
}

#[test]
fn wanted_is_english_the_original_language_or_no_language_code() {
    let cases: [(&str, &[&str], bool); 7] = [
        ("eng", &[], true),
        ("spa", &["spa"], true),
        ("spa", &[], false),
        ("und", &[], true),
        ("", &[], true),
        ("zxx", &[], true),
        ("fra", &["spa"], false),
    ];
    for (language, original, expected) in cases {
        assert_eq!(wanted(&s(1, 2, false, language, "x"), original), expected, "{language:?}");
    }
}

#[test]
fn the_datasets_first_of_language_gives_the_stream_and_its_description() {
    let streams = [
        probe(0, "video", None, None, None, None, &[]),
        probe(1, "audio", Some("dts"), Some("DTS"), Some(6), None, &[("language", "eng")]),
        probe(2, "audio", Some("dts"), Some("DTS-HD MA"), Some(6), None, &[("language", "eng")]),
    ];
    assert_eq!(first_of_language(None, 1), None);
    assert_eq!(first_of_language(Some(&streams[..]), 2), Some((1, "dts DTS 6 ch".to_string())));
    assert_eq!(first_of_language(Some(&streams[..]), 1), None);
}
