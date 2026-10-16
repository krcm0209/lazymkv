//! `audio-proxy.json` names, in `encoding.read`, the program that reads the proxy back: `audio-levels.ps1` wrote
//! "(proxy_levels.py)" until the Rust port and writes "(lazymkv-dev levels-from-proxy)" since, while the files already
//! in the tuning dataset and in the evidence keep the old text. Port spec section 6: "the Rust reader accepts both".
//! The reader never checks the field; this test pins that, so a later check of it has to keep both spellings.

use lazymkv_core::audio::levels::{parse_proxy, PROXY_FORMAT};

/// The `read` text audio-levels.ps1 wrote before the port (its proxy headroom is 18 dB).
const READ_BEFORE: &str =
    "add 18 dB in float, convert to the source stream's sample_fmt, then measure (proxy_levels.py)";
/// The same text since the port.
const READ_SINCE: &str =
    "add 18 dB in float, convert to the source stream's sample_fmt, then measure (lazymkv-dev levels-from-proxy)";

/// A made-up audio-proxy.json of one stereo AC-3 stream (its mixdown and its side signal), with the fields the reader
/// keeps.
fn proxy_json(read: &str) -> String {
    format!(
        concat!(
            "{{\"format\": \"{format}\", \"slug\": \"some-film\", \"label\": \"all-tracks\",\n",
            " \"source\": {{\"file\": \"title_t00.mkv\", \"bytes\": 1000, \"audio_streams\": 1}},\n",
            " \"encoding\": {{\"codec\": \"flac\", \"gain_db\": -18, \"read\": \"{read}\"}},\n",
            " \"tracks\": [{{\"track\": 0, \"source_stream\": 1, \"series\": \"mix\",\n",
            "   \"source\": {{\"codec\": \"ac3\", \"channels\": 2, \"language\": \"eng\"}}}},\n",
            "  {{\"track\": 1, \"source_stream\": 1, \"series\": \"side\",\n",
            "   \"source\": {{\"codec\": \"ac3\", \"channels\": 2, \"language\": \"eng\"}}}}]}}\n"
        ),
        format = PROXY_FORMAT,
        read = read
    )
}

#[test]
fn both_spellings_of_the_reader_parse_alike() {
    let before = parse_proxy(&proxy_json(READ_BEFORE)).expect("the spelling before the port parses");
    let since = parse_proxy(&proxy_json(READ_SINCE)).expect("the spelling since the port parses");
    assert_eq!(before.encoding.get("read").and_then(|v| v.as_str()), Some(READ_BEFORE));
    assert_eq!(since.encoding.get("read").and_then(|v| v.as_str()), Some(READ_SINCE));
    for proxy in [&before, &since] {
        assert_eq!(proxy.format.as_ref().and_then(|v| v.as_str()), Some(PROXY_FORMAT));
        assert_eq!(proxy.encoding.get("gain_db").and_then(|v| v.as_i64()), Some(-18));
        assert_eq!(proxy.source.get("audio_streams").and_then(|v| v.as_i64()), Some(1));
        let series: Vec<Option<&str>> = proxy.tracks.iter().map(|t| t.series.as_deref()).collect();
        assert_eq!(series, vec![Some("mix"), Some("side")]);
        assert_eq!(proxy.tracks[0].source_stream.as_i64(), Some(1));
    }
}
