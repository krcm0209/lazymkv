//! Tests of core::audio::pass against scripts/phase0/audio-levels.ps1: its graph, command, series files, stream hashes,
//! centre rule, manifest, proxy checks and audio-proxy.json, over made-up streams. The edge values of net_fixed1 are
//! what Windows PowerShell 5.1 printed on the mini PC (Task 16 Step 1).

use serde_json::json;

use super::*;
use crate::audio::dataset::levels_args;
use crate::audio::levels::{parse_proxy, parse_streams};
use crate::recording::{MemTree, PathKey};
use crate::testsupport::probe_streams_json;

/// Phase 0's level chain up to its file name (audio-levels.ps1 lines 265 to 267).
const TAIL: &str = "aresample=8000,highpass=f=300,lowpass=f=3400,asetnsamples=n=160:p=0,astats=metadata=1:reset=1:measure_perchannel=none:measure_overall=RMS_level,ametadata=print:key=lavfi.astats.Overall.RMS_level:file=";

/// Phase 0's proxy chain up to its file name, at `gain` dB (lines 270 to 272).
fn to_proxy(gain: i32) -> String {
    format!("aresample=8000,volume={gain}dB,asetnsamples=n=8000:p=0,astats=metadata=1:reset=0:measure_perchannel=none:measure_overall=Peak_level,ametadata=print:key=lavfi.astats.Overall.Peak_level:file=")
}

/// A made-up `ffmpeg -hide_banner -layouts` output: a DESCRIPTION section, then the NAME DECOMPOSITION table.
const LAYOUTS: &str = "Individual channels:\nNAME           DESCRIPTION\nFL             front left\nFR             front right\nFC             front center\nLFE            low frequency\nSL             side left\nSR             side right\nBL             back left\nBR             back right\n\nStandard channel layouts:\nNAME           DECOMPOSITION\nmono           FC\nstereo         FL+FR\n2.1            FL+FR+LFE\n5.1            FL+FR+FC+LFE+BL+BR\n5.1(side)      FL+FR+FC+LFE+SL+SR\n";

/// A made-up file: video, a 5.1 main, a stereo and a mono candidate, a subtitle stream.
const THREE: &str = r#"{"streams": [
  {"index": 0, "codec_type": "video", "codec_name": "hevc"},
  {"index": 1, "codec_type": "audio", "codec_name": "truehd", "channels": 6, "channel_layout": "5.1(side)", "tags": {"language": "eng"}},
  {"index": 2, "codec_type": "audio", "codec_name": "ac3", "channels": 2, "channel_layout": "stereo", "tags": {"language": "eng"}},
  {"index": 3, "codec_type": "audio", "codec_name": "ac3", "channels": 1, "channel_layout": "mono", "tags": {"language": "spa"}},
  {"index": 4, "codec_type": "subtitle", "codec_name": "hdmv_pgs_subtitle"}]}"#;

/// Two mono streams, the second with no language tag.
const MONO_PAIR: &str = r#"{"streams": [
  {"index": 1, "codec_type": "audio", "codec_name": "ac3", "channels": 1, "channel_layout": "mono", "tags": {"language": "eng"}},
  {"index": 2, "codec_type": "audio", "codec_name": "ac3", "channels": 1, "channel_layout": "mono"}]}"#;

/// Layouts ffprobe could not name, spelled out, unguessable, and a name in another case.
const ODD: &str = r#"{"streams": [
  {"index": 1, "codec_type": "audio", "codec_name": "pcm_s24le", "channels": 6, "channel_layout": "unknown", "tags": {"language": "eng"}},
  {"index": 2, "codec_type": "audio", "codec_name": "dts", "profile": "DTS-HD MA", "channels": 2, "channel_layout": "stereo", "tags": {"language": "eng"}},
  {"index": 3, "codec_type": "audio", "codec_name": "aac", "channels": 3, "channel_layout": "3 channels (FL+FR+LFE@2)"},
  {"index": 4, "codec_type": "audio", "codec_name": "aac", "channels": 3},
  {"index": 5, "codec_type": "audio", "codec_name": "aac", "channels": 9},
  {"index": 6, "codec_type": "audio", "codec_name": "eac3", "channels": 6, "channel_layout": "5.1(Side)"}]}"#;

const H0: &str = "0123456789abcdef0123456789abcdef";
const H1: &str = "fedcba9876543210fedcba9876543210";
const H2: &str = "00112233445566778899aabbccddeeff";

fn streams(json: &str) -> Vec<PassStream> {
    pass_streams(&parse_streams(json).unwrap().streams)
}

fn layouts() -> Layouts {
    parse_layouts(LAYOUTS).unwrap()
}

#[test]
fn the_graph_is_phase_0s_for_a_main_with_centre_and_two_candidates() {
    let s = streams(THREE);
    assert_eq!(s.iter().map(|x| x.index).collect::<Vec<i64>>(), vec![1, 2, 3]);
    let g = graph(&s, 1, &[2, 3], &layouts(), true, GAIN_FIRST_DB);
    let p = to_proxy(-18);
    let expected = [
        "[0:1]asplit=4[s1b0][s1b1][s1b2][s1b3]".to_string(),
        format!("[s1b0]aformat=channel_layouts=mono,{TAIL}raw-main-mix.txt[o0]"),
        format!("[s1b1]pan=mono|c0=FC,{TAIL}raw-main-fc.txt[o1]"),
        format!("[s1b2]aformat=channel_layouts=mono,{p}peak-0.txt,aformat=sample_fmts=s32[p0]"),
        format!("[s1b3]pan=mono|c0=FC,{p}peak-1.txt,aformat=sample_fmts=s32[p1]"),
        "[0:2]asplit=3[s2b0][s2b1][s2b2]".to_string(),
        format!("[s2b0]aformat=channel_layouts=mono,{TAIL}raw-cand-2.txt[o2]"),
        format!("[s2b1]aformat=channel_layouts=mono,{p}peak-2.txt,aformat=sample_fmts=s32[p2]"),
        format!("[s2b2]pan=mono|c0=0.5*c0-0.5*c1,{p}peak-3.txt,aformat=sample_fmts=s32[p3]"),
        "[0:3]asplit=2[s3b0][s3b1]".to_string(),
        format!("[s3b0]aformat=channel_layouts=mono,{TAIL}raw-cand-3.txt[o3]"),
        format!("[s3b1]aformat=channel_layouts=mono,{p}peak-4.txt,aformat=sample_fmts=s32[p4]"),
    ];
    assert_eq!(g.text, expected.join(";"));
    let series: Vec<(&str, &str)> = g.series.iter().map(|x| (x.name.as_str(), x.raw.as_str())).collect();
    assert_eq!(series, vec![("main-mix", "raw-main-mix.txt"), ("main-fc", "raw-main-fc.txt"),
                            ("cand-2", "raw-cand-2.txt"), ("cand-3", "raw-cand-3.txt")]);
    let tracks: Vec<(i64, &str, &str, &str)> =
        g.proxy.iter().map(|t| (t.stream, t.series.as_str(), t.language.as_str(), t.peak.as_str())).collect();
    assert_eq!(tracks, vec![(1, "mix", "eng", "peak-0.txt"), (1, "fc", "eng", "peak-1.txt"), (2, "mix", "eng", "peak-2.txt"),
                            (2, "side", "eng", "peak-3.txt"), (3, "mix", "spa", "peak-4.txt")]);
}

#[test]
fn asplit_is_used_only_for_a_stream_with_several_branches() {
    let g = graph(&streams(MONO_PAIR), 1, &[], &layouts(), true, GAIN_FIRST_DB);
    let p = to_proxy(-18);
    let expected = [
        "[0:1]asplit=2[s1b0][s1b1]".to_string(),
        format!("[s1b0]aformat=channel_layouts=mono,{TAIL}raw-main-mix.txt[o0]"),
        format!("[s1b1]aformat=channel_layouts=mono,{p}peak-0.txt,aformat=sample_fmts=s32[p0]"),
        format!("[0:2]aformat=channel_layouts=mono,{p}peak-1.txt,aformat=sample_fmts=s32[p1]"),
    ];
    assert_eq!(g.text, expected.join(";"));
    assert_eq!(g.proxy[1].language, "und");
}

#[test]
fn the_redo_graph_holds_only_the_proxy_branches_at_minus_30() {
    let g = graph(&streams(THREE), 1, &[2, 3], &layouts(), false, GAIN_REDO_DB);
    let p = to_proxy(-30);
    let expected = [
        "[0:1]asplit=2[s1b0][s1b1]".to_string(),
        format!("[s1b0]aformat=channel_layouts=mono,{p}peak-0.txt,aformat=sample_fmts=s32[p0]"),
        format!("[s1b1]pan=mono|c0=FC,{p}peak-1.txt,aformat=sample_fmts=s32[p1]"),
        "[0:2]asplit=2[s2b0][s2b1]".to_string(),
        format!("[s2b0]aformat=channel_layouts=mono,{p}peak-2.txt,aformat=sample_fmts=s32[p2]"),
        format!("[s2b1]pan=mono|c0=0.5*c0-0.5*c1,{p}peak-3.txt,aformat=sample_fmts=s32[p3]"),
        format!("[0:3]aformat=channel_layouts=mono,{p}peak-4.txt,aformat=sample_fmts=s32[p4]"),
    ];
    assert_eq!(g.text, expected.join(";"));
    assert!(g.series.is_empty());
    assert_eq!(g.proxy, graph(&streams(THREE), 1, &[2, 3], &layouts(), true, GAIN_FIRST_DB).proxy);
}

/// The proxy half of the command for THREE's five tracks, written to `audio-proxy.mka.partial`.
fn proxy_part() -> Vec<String> {
    let mut v: Vec<String> = (0..5).flat_map(|k| ["-map".to_string(), format!("[p{k}]")]).collect();
    v.extend(["-c:a", "flac", "-compression_level", "5", "-bits_per_raw_sample", "24"].map(String::from));
    let tracks = [(1, "mix", "eng"), (1, "fc", "eng"), (2, "mix", "eng"), (2, "side", "eng"), (3, "mix", "spa")];
    for (k, (n, series, lang)) in tracks.iter().enumerate() {
        for value in [format!("LAZYMKV_SOURCE_STREAM={n}"), format!("LAZYMKV_SERIES={series}"), format!("language={lang}")] {
            v.extend([format!("-metadata:s:a:{k}"), value]);
        }
    }
    v.extend(["-f", "matroska", "audio-proxy.mka.partial"].map(String::from));
    v
}

#[test]
fn the_command_has_phase_0s_parts_in_order() {
    let head: Vec<String> =
        ["-v", "warning", "-nostats", "-nostdin", "-i", "title_t00.mkv", "-/filter_complex", "graph.txt"].map(String::from).to_vec();
    let g = graph(&streams(THREE), 1, &[2, 3], &layouts(), true, GAIN_FIRST_DB);
    let mut want = head.clone();
    for k in 0..4 {
        want.extend(["-map".to_string(), format!("[o{k}]"), "-f".to_string(), "null".to_string(), format!("o{k}")]);
    }
    for n in 1..=3 {
        want.extend(["-map".to_string(), format!("0:{n}")]);
    }
    want.extend(["-c", "copy", "-f", "streamhash", "-hash", "md5", "streamhash.txt"].map(String::from));
    want.extend(proxy_part());
    assert_eq!(ffmpeg_args("title_t00.mkv", &g, true, Some("audio-proxy.mka.partial")), want);
    let redo = graph(&streams(THREE), 1, &[2, 3], &layouts(), false, GAIN_REDO_DB);
    let mut want = head;
    want.extend(proxy_part());
    assert_eq!(ffmpeg_args("title_t00.mkv", &redo, false, Some("audio-proxy.mka.partial")), want);
}

/// A made-up raw ametadata file: -inf, a half, nan, a value below the floor, an exact binary tie, a peak line and an
/// indented RMS line (neither is read).
const RAW: &[&str] = &[
    "frame:0    pts:0       pts_time:0",
    "lavfi.astats.Overall.RMS_level=-inf",
    "frame:1    pts:160     pts_time:0.02",
    "lavfi.astats.Overall.RMS_level=-23.450000",
    "frame:2    pts:320     pts_time:0.04",
    "lavfi.astats.Overall.RMS_level=nan",
    "frame:3    pts:480     pts_time:0.06",
    "lavfi.astats.Overall.RMS_level=-123.500000",
    "frame:4    pts:640     pts_time:0.08",
    "lavfi.astats.Overall.RMS_level=-12.250000",
    "lavfi.astats.Overall.Peak_level=-3.000000",
    " lavfi.astats.Overall.RMS_level=-5.000000",
];
const HEADER: &str = "# lazymkv audio levels v2: frame_ms=20 band_hz=300-3400 unit=dB floor=-100 series=main-mix file=My_Title_t00.mkv";

#[test]
fn a_series_file_has_the_v2_header_crlf_lines_and_minus_100_for_inf_and_nan() {
    let raw = RAW.join("\n") + "\n";
    let want = [HEADER, "-100.0", "-23.5", "-100.0", "-100.0", "-12.3"].join("\r\n") + "\r\n";
    assert_eq!(series_file(&raw, "main-mix", "My Title \t t00.mkv"), Ok(want));
    assert!(series_file(&raw, "main-mix", "").unwrap().starts_with(
        "# lazymkv audio levels v2: frame_ms=20 band_hz=300-3400 unit=dB floor=-100 series=main-mix file=-\r\n"));
    assert_eq!(series_file("frame:0\n", "cand-2", "x.mkv"), Err("cand-2.txt holds no level".to_string()));
    assert_eq!(series_file("lavfi.astats.Overall.RMS_level=abc\n", "main-fc", "x.mkv"),
               Err("raw-main-fc.txt: abc is not a number".to_string()));
    assert_eq!(series_file("lavfi.astats.Overall.RMS_level=Infinity\n", "main-fc", "x.mkv"),
               Err("raw-main-fc.txt: Infinity is not a number".to_string()));
}

#[test]
fn a_raw_file_with_crlf_and_a_trailing_blank_line_gives_the_same_series() {
    // Review Focus 5: .NET's File.ReadLines ends a line at CRLF, LF or CR, and a blank line holds no value.
    let want = series_file(&(RAW.join("\n") + "\n"), "main-mix", "My Title t00.mkv").unwrap();
    assert_eq!(series_file(&(RAW.join("\r\n") + "\r\n\r\n"), "main-mix", "My Title t00.mkv").unwrap(), want);
    assert_eq!(series_file(&(RAW.join("\r") + "\r"), "main-mix", "My Title t00.mkv").unwrap(), want);
    assert_eq!(series_file(&RAW.join("\n"), "main-mix", "My Title t00.mkv").unwrap(), want, "no newline after the last line");
    assert!(want.ends_with("-12.3\r\n") && !want.contains("\r\n\r\n") && !want.starts_with('\u{feff}'));
}

/// (ffmpeg's text, what Windows PowerShell 5.1 printed for it on the mini PC in Step 1).
const EDGES: &[(&str, &str)] = &[
    ("-12.25", "-12.3"), ("-12.250000", "-12.3"), ("-0.25", "-0.3"), ("-23.45", "-23.5"), ("-1.15", "-1.2"),
    ("-7.650000", "-7.7"), ("-64.349999", "-64.3"), ("-0.05", "-0.1"), ("-0.04", "0.0"), ("-0.000000", "0.0"),
    ("0.000000", "0.0"), ("-99.95", "-100.0"), ("-99.94", "-99.9"), ("-100", "-100.0"), ("-3", "-3.0"),
    ("-20.0499999", "-20.0"), ("-20.04999999999999", "-20.1"),
];

#[test]
fn net_fixed1_matches_dotnet_framework_on_the_edge_values() {
    // Review Focus 5, T5-R19: ties on the binary value, 15-digit inputs and values that round to zero.
    for (text, printed) in EDGES {
        assert_eq!(net_fixed1(text), *printed, "{text}");
    }
    // Rust's {:.1} rounds the binary value half to even, so it is not the port's formatter (R6-phase0-56).
    assert_eq!(format!("{:.1}", -12.25_f64), "-12.2");
    assert_eq!(format!("{:.1}", -1.15_f64), "-1.1");
}

#[test]
fn streamhash_with_a_missing_stream_is_refused_with_both_counts() {
    let two = format!("0,a,MD5={H0}\n1,a,MD5={H1}\n");
    assert_eq!(parse_streamhash(&two, 3), Err("streamhash.txt holds 2 of 3 stream hashes".to_string()));
    let three = format!("0,a,MD5={H0}\r\n1,a,MD5={H1}\r\n2,a,MD5={H2}\r\n");
    assert_eq!(parse_streamhash(&three, 3), Ok(vec![(0, H0.to_string()), (1, H1.to_string()), (2, H2.to_string())]));
    // A repeated k counts once; a short hash, an indented line and a k past the count are not counted.
    let noisy = format!("0,a,MD5={H0}\n0,a,MD5={H0}\n1,a,MD5=abc\n 2,a,MD5={H2}\n5,a,MD5={H2}\n");
    assert_eq!(parse_streamhash(&noisy, 3), Err("streamhash.txt holds 1 of 3 stream hashes".to_string()));
    assert_eq!(parse_streamhash(&format!("0,A,md5={H0}\n"), 1), Ok(vec![(0, H0.to_string())]), "-match ignores case");
}

/// A made-up peak file of astats with reset=0: one running peak per second.
fn peaks(values: &[&str]) -> String {
    values
        .iter()
        .enumerate()
        .map(|(k, v)| format!("frame:{k}    pts:{}     pts_time:{k}\nlavfi.astats.Overall.Peak_level={v}\n", k * 8000))
        .collect()
}

#[test]
fn a_peak_at_minus_0_1_is_clipped_and_minus_0_11_is_not() {
    assert_eq!(peak_clipped(&peaks(&["-20.000000", "-0.100000"])), Ok(true));
    assert_eq!(peak_clipped(&peaks(&["-20.000000", "-0.110000"])), Ok(false));
    assert_eq!(peak_clipped(&peaks(&["0.000000"])), Ok(true));
    assert_eq!(peak_clipped(&peaks(&["-inf"])), Ok(false));
    assert_eq!(last_peak(&peaks(&["-30.000000", " -0.110000 "])), Some("-0.110000".to_string()));
    assert_eq!(last_peak("frame:0\n"), None);
    assert_eq!(peak_clipped("frame:0\n"), Err("holds no peak".to_string()));
    assert_eq!(peak_clipped(&peaks(&["x"])), Err("holds the peak x, which is not a number".to_string()));
}

#[test]
fn a_guessed_layout_is_recorded_and_finds_its_centre() {
    let s = streams(ODD);
    let l = layouts();
    let got: Vec<Centre> = s.iter().map(|x| centre(x, &l)).collect();
    let guess = |name: &str| Some(name.to_string());
    assert_eq!(got, vec![
        Centre { has_fc: true, guessed: guess("5.1") },
        Centre { has_fc: false, guessed: None },
        Centre { has_fc: false, guessed: None },
        Centre { has_fc: false, guessed: guess("2.1") },
        Centre { has_fc: false, guessed: None },
        Centre { has_fc: true, guessed: None },
    ]);
    let text = manifest_text("My Title t00.mkv", 1000, 42.5, &s[0], &s[1..2], &l, &[(1, 250), (2, 249)],
                             &[(1, H0.to_string()), (2, H1.to_string())]);
    let want: Vec<String> = vec![
        "# lazymkv audio-levels v2 (experiment 13)".to_string(),
        "file=My_Title_t00.mkv bytes=1000 seconds=42 frame_ms=20 band_hz=300-3400".to_string(),
        format!("main=1 codec=pcm_s24le profile=- channels=6 layout=unknown layout_guessed=5.1 language=eng fc=yes frames=250 md5={H0}"),
        format!("cand=2 codec=dts profile=DTS-HD_MA channels=2 layout=stereo language=eng frames=249 md5={H1}"),
    ];
    assert_eq!(text, want.join("\r\n") + "\r\n");
    let g = graph(&s, 1, &[2], &l, true, GAIN_FIRST_DB);
    assert!(g.text.contains(&format!("[s1b1]pan=mono|c0=FC,{TAIL}raw-main-fc.txt[o1]")), "{}", g.text);
}

#[test]
fn the_layout_table_is_read_from_the_name_decomposition_section_only() {
    let none = "ffmpeg -layouts printed no table of channel layouts (no NAME DECOMPOSITION section)";
    assert_eq!(parse_layouts("").err().as_deref(), Some(none));
    assert_eq!(parse_layouts("NAME           DESCRIPTION\nquad           FL+FR+FC+LFE\n").err().as_deref(), Some(none));
    let four = streams(r#"{"streams": [{"index": 1, "codec_type": "audio", "channels": 4, "channel_layout": "quad"},
                                         {"index": 2, "codec_type": "audio", "channels": 4}]}"#);
    let with_quad = parse_layouts(&format!("{LAYOUTS}quad           FL+FR+FC+BC\n")).unwrap();
    assert_eq!(centre(&four[0], &with_quad), Centre { has_fc: true, guessed: None });
    assert_eq!(centre(&four[0], &layouts()), Centre { has_fc: false, guessed: None }, "quad is not in that table");
    assert_eq!(centre(&four[1], &layouts()), Centre { has_fc: false, guessed: None }, "4.0 is not in that table");
}

#[test]
fn the_proxy_language_is_a_three_letter_tag_else_und() {
    let cases = [(Some("eng"), "eng"), (Some("spa"), "spa"), (None, "und"), (Some(""), "und"), (Some("Eng"), "und"),
                 (Some("en"), "und"), (Some("engl"), "und"), (Some("e1g"), "und")];
    for (tag, want) in cases {
        assert_eq!(proxy_language(tag), want, "{tag:?}");
    }
}

#[test]
fn the_level_folder_is_named_main_then_candidates() {
    assert_eq!(level_folder(1, &[2, 3]), "audio-levels-20ms-1-2-3");
    assert_eq!(level_folder(3, &[1]), "audio-levels-20ms-3-1");
    assert_eq!(level_folder(4, &[]), "audio-levels-20ms-4");
}

#[test]
fn main_and_candidates_read_levels_args_first_line() {
    let mut tree = MemTree::new(".", "tmp");
    tree.insert_file("some-film/dub-first/ffprobe-streams.json", probe_streams_json(&[
        ("dts", Some("DTS-HD MA"), 6, "spa"), ("truehd", None, 8, "eng"), ("ac3", None, 2, "eng"),
        ("ac3", None, 6, "fra"), ("ac3", None, 2, "fra")]));
    tree.insert_file("some-film/tmdb.json", r#"{"original_language": "es"}"#);
    let lines = levels_args(&tree, &PathKey::new("some-film"), "dub-first").unwrap();
    let jobs: Vec<&String> = lines.iter().filter(|l| l.starts_with("scripts/phase0/job.sh")).collect();
    assert_eq!(jobs.len(), 2);
    assert_eq!(main_and_candidates(jobs[0]), Some((1, vec![2, 3])), "T5-R38: the first job line is the pass");
    assert_eq!(main_and_candidates(jobs[1]), Some((4, vec![5])));
    assert_eq!(main_and_candidates("scripts/phase0/job.sh audio-levels.ps1 l -Slug s -Label t -Main 3"), Some((3, vec![])));
    assert_eq!(main_and_candidates("scripts/phase0/wait.sh audio-levels-some-film-dub-first"), None);
    assert_eq!(main_and_candidates("  stream 1: dts DTS-HD MA 6 ch lang=spa"), None);
    assert_eq!(main_and_candidates("scripts/phase0/job.sh audio-levels.ps1 l -Slug s -Label t -Main x"), None);
}

#[test]
fn the_main_and_candidates_are_checked_with_phase_0s_refusals() {
    let all = parse_streams(THREE).unwrap().streams;
    assert_eq!(check_main_and_candidates(&all, 1, &[2, 3]), Ok(()));
    let refused = |main: i64, cands: &[i64]| check_main_and_candidates(&all, main, cands).unwrap_err();
    assert_eq!(refused(1, &[1]), "stream 1 is the main stream and cannot also be a candidate");
    assert_eq!(refused(1, &[2, 2]), "a candidate stream is listed twice: 2,2");
    assert_eq!(refused(1, &[7]), "the file has no stream 7");
    assert_eq!(refused(0, &[]), "stream 0 is video, not audio");
    assert_eq!(refused(1, &[4]), "stream 4 is subtitle, not audio");
}

/// proxy-check.json as ffprobe writes it for two tracks of stream 2 (mix, side), edited by `edit`.
fn check_json(edit: impl Fn(&mut Vec<serde_json::Value>)) -> String {
    let track = |index: i64, series: &str, duration: &str| json!({"index": index, "codec_name": "flac", "codec_type": "audio",
        "sample_fmt": "s32", "sample_rate": "8000", "channels": 1, "bits_per_raw_sample": "24",
        "tags": {"LAZYMKV_SOURCE_STREAM": "2", "LAZYMKV_SERIES": series, "language": "eng", "DURATION": duration}});
    let mut streams = vec![track(0, "mix", "01:02:03.500000000"), track(1, "side", "00:00:01.000000000")];
    edit(&mut streams);
    serde_json::to_string_pretty(&json!({"programs": [], "streams": streams})).unwrap()
}

fn mix_and_side() -> Vec<ProxyTrack> {
    let t = |series: &str, k: usize| ProxyTrack { stream: 2, series: series.to_string(), language: "eng".to_string(), peak: format!("peak-{k}.txt") };
    vec![t("mix", 0), t("side", 1)]
}

#[test]
fn the_proxy_check_wants_one_flac_8000_hz_mono_24_bit_track_per_series_with_its_tags_and_a_duration() {
    let want = mix_and_side();
    assert_eq!(proxy_check(&check_json(|_| {}), &want), Ok(vec![3723.5, 1.0]));
    assert_eq!(proxy_check(&check_json(|s| { s.pop(); }), &want),
               Err("proxy check failed: the proxy has 1 tracks, 2 were written".to_string()));
    assert_eq!(proxy_check(&check_json(|s| s[1]["sample_rate"] = json!("48000")), &want),
               Err("proxy check failed: proxy track 1 (stream 2 side) is audio flac 48000 Hz 1 ch 24 bit, not flac 8000 Hz 1 ch 24 bit".to_string()));
    assert_eq!(proxy_check(&check_json(|s| s[0]["tags"]["LAZYMKV_SERIES"] = json!("fc")), &want),
               Err("proxy check failed: proxy track 0 (stream 2 mix) is tagged stream 2 fc".to_string()));
    assert_eq!(proxy_check(&check_json(|s| { s[1]["tags"].as_object_mut().unwrap().remove("DURATION"); }), &want),
               Err("proxy check failed: proxy track 1 (stream 2 side) has no duration (the file was not finished)".to_string()));
    let lower = check_json(|s| s[0]["tags"] = json!({"lazymkv_source_stream": "2", "lazymkv_series": "mix", "duration": "00:00:02.5"}));
    assert_eq!(proxy_check(&lower, &want), Ok(vec![2.5, 1.0]), "tag names are read as PowerShell reads them, ignoring case");
}

fn source(md5: &str) -> ProxySourceStream {
    ProxySourceStream { codec: "ac3".to_string(), profile: String::new(), channels: 2, layout: "stereo".to_string(),
        layout_guessed: String::new(), language: "eng".to_string(), title: String::new(), sample_rate: "48000".to_string(),
        sample_fmt: "fltp".to_string(), bits_per_raw_sample: String::new(), start_time: "0.000000".to_string(), md5: md5.to_string() }
}

/// A made-up record of one stereo stream (its mix and side tracks).
fn record(proxy: Option<ProxyFile>, gain_db: i32, gains_tried: Vec<i32>) -> ProxyRecord {
    let track = |k: usize, series: &str, peak: &str| ProxyTrackFacts { track: k, source_stream: 1, series: series.to_string(),
        duration_s: 12.3456, peak_dbfs: peak.to_string(), source: source(H0) };
    ProxyRecord {
        slug: "job-a".to_string(), label: "t00".to_string(), created: "2026-11-06 10:00:00".to_string(),
        source: ProxySource { file: "title_t00.mkv".to_string(), bytes: 1000, modified: "2026-11-06 09:00:00".to_string(),
                              duration_s: "12.345000".to_string(), audio_streams: 1 },
        pass: ProxyPass { seconds: 42, ffmpeg: "ffmpeg version 9.0.2".to_string(), levels_folder: "audio-levels-20ms-1".to_string() },
        gain_db, proxy,
        tracks: vec![track(0, "mix", "-0.050000"), track(1, "side", "-6.000000")],
        gains_tried,
    }
}

#[test]
fn audio_proxy_json_without_a_proxy_records_both_gains_and_parses_with_dev() {
    let text = proxy_json(&record(None, GAIN_REDO_DB, vec![GAIN_FIRST_DB, GAIN_REDO_DB]));
    assert!(text.starts_with("{\n  \"format\": \"lazymkv audio proxy v1\",\n"), "{text}");
    assert!(text.ends_with("}\n") && !text.ends_with("}\n\n"));
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(keys, ["format", "slug", "label", "created", "source", "pass", "encoding", "proxy", "not_kept", "tracks", "gains_tried"]);
    let enc: Vec<&str> = v["encoding"].as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(enc, ["codec", "sample_rate", "channels", "bits", "compression_level", "gain_db", "mix", "fc", "side", "chain", "read"]);
    let track_keys: Vec<&str> = v["tracks"][0].as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(track_keys, ["track", "source_stream", "series", "duration_s", "peak_dbfs", "source"]);
    let src = json!({"codec": "ac3", "profile": "", "channels": 2, "layout": "stereo", "layout_guessed": "", "language": "eng",
        "title": "", "sample_rate": "48000", "sample_fmt": "fltp", "bits_per_raw_sample": "", "start_time": "0.000000", "md5": H0});
    let src_keys: Vec<&str> = v["tracks"][0]["source"].as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(src_keys, ["codec", "profile", "channels", "layout", "layout_guessed", "language", "title", "sample_rate",
                          "sample_fmt", "bits_per_raw_sample", "start_time", "md5"]);
    assert_eq!(v, json!({
        "format": "lazymkv audio proxy v1", "slug": "job-a", "label": "t00", "created": "2026-11-06 10:00:00",
        "source": {"file": "title_t00.mkv", "bytes": 1000, "modified": "2026-11-06 09:00:00", "duration_s": "12.345000", "audio_streams": 1},
        "pass": {"seconds": 42, "ffmpeg": "ffmpeg version 9.0.2", "levels_folder": "audio-levels-20ms-1"},
        "encoding": {"codec": "flac", "sample_rate": 8000, "channels": 1, "bits": 24, "compression_level": 5, "gain_db": -30,
            "mix": "aformat=channel_layouts=mono on the decoded stream, as in the level chain",
            "fc": "pan=mono|c0=FC, for every stream whose layout has FC besides other channels",
            "side": "pan=mono|c0=0.5*c0-0.5*c1, for every two-channel stream (the mix is 0.5*c0+0.5*c1)",
            "chain": "aresample=8000,volume=-30dB,aformat=sample_fmts=s32",
            "read": "add 30 dB in float, convert to the source stream's sample_fmt, then measure (lazymkv-dev levels-from-proxy)"},
        "proxy": null,
        "not_kept": "video and subtitle streams; channels other than the mixdown, the centre and a stereo pair's side signal; sound above 4 kHz; object-audio metadata; the packets themselves (their MD5 is kept)",
        "tracks": [
            {"track": 0, "source_stream": 1, "series": "mix", "duration_s": 12.346, "peak_dbfs": "-0.050000", "source": src.clone()},
            {"track": 1, "source_stream": 1, "series": "side", "duration_s": 12.346, "peak_dbfs": "-6.000000", "source": src}],
        "gains_tried": [-18, -30]
    }));
    // dev's reader (core::audio::levels, which decide's read_level_md5 and levels-from-proxy use) reads it.
    let p = parse_proxy(&text).unwrap();
    assert!(p.kept_no_proxy());
    assert_eq!(p.gains_tried, Some(vec![-18, -30]));
    assert_eq!(p.encoding.get("gain_db").and_then(|g| g.as_i64()), Some(-30));
    let tracks: Vec<(Option<i64>, Option<&str>)> = p.tracks.iter().map(|t| (t.source_stream.as_i64(), t.series.as_deref())).collect();
    assert_eq!(tracks, vec![(Some(1), Some("mix")), (Some(1), Some("side"))]);
    assert_eq!(p.tracks[0].source.as_ref().unwrap().get("md5").and_then(|m| m.as_str()), Some(H0));
    let kept = parse_proxy(&proxy_json(&record(Some(ProxyFile { bytes: 5000, tracks: 2 }), GAIN_FIRST_DB, vec![GAIN_FIRST_DB]))).unwrap();
    assert!(!kept.kept_no_proxy());
    assert_eq!(kept.proxy, Some(json!({"file": "audio-proxy.mka", "bytes": 5000, "tracks": 2})));
    assert_eq!(kept.gains_tried, Some(vec![-18]));
    assert_eq!(kept.encoding.get("read").and_then(|r| r.as_str()),
               Some("add 18 dB in float, convert to the source stream's sample_fmt, then measure (lazymkv-dev levels-from-proxy)"));
}
