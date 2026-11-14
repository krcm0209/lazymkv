//! Layer 2 of the tests (design part 12.1, G1; calls 20 and 24; T5-R49): the real mkvmerge, ffprobe and ffmpeg on
//! short Matroska files made during the test, from ffmpeg's lavfi sources (libavfilter's virtual input) or from
//! `core::testsupport::mks`, through the drivers the engine uses. Each tool comes from LAZYMKV_TEST_MKVMERGE,
//! LAZYMKV_TEST_FFPROBE or LAZYMKV_TEST_FFMPEG, else the first match on PATH, and ffmpeg and ffprobe must be FFmpeg
//! 7.0 or later (K11). Without a tool, or with an older FFmpeg, a test prints `skipped: <reason>` and passes; on CI
//! (`CI` set), where every runner installs the three, it fails instead, so a green run proves that layer 2 ran. Every
//! file is made up: block shapes and sine tones, no byte of the evidence clone (RP21). The lavfi input files are made
//! by the FFmpeg LAZYMKV_TEST_FFMPEG_FULL names when it is set, else by the ffmpeg under test (T6-R8).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use lazymkv_core::audio::levels::parse_levels;
use lazymkv_core::audio::pass::{
    centre, ffmpeg_args, graph, last_peak, parse_layouts, parse_streamhash, pass_streams, peak_clipped, proxy_check,
    series_file, GAIN_FIRST_DB,
};
use lazymkv_core::disc::StreamKind;
use lazymkv_core::ipc::OutputStream;
use lazymkv_core::job::Tool;
use lazymkv_core::recording::files::PacketsFile;
use lazymkv_core::subtitle::bitmap::mkv::read_stream;
use lazymkv_core::subtitle::bitmap::CODEC_PGS;
use lazymkv_core::testsupport::mks::{mks, pgs_display_set, MksBlock, MksEncoding, MksShape, MksTrack};
use lazymkv_core::testsupport::{Scratch, ScratchExt};
use lazymkv_core::toolver::{standing, Standing, FFMPEG_MIN};
use lazymkv_core::tracks::layout::{Layout, OutputTrack};
use lazymkv_core::verify::VerifyReport;
use lazymkv_platform::fakes::FakeHost;
use lazymkv_platform::StdProcessGuard;
use lazymkv_tools::{
    exe_name, extract, extract_args, ident, layouts_args, packets, packets_args, probe, proxy_check_args, remux,
    remux_args, run_ffmpeg, streamhash_args, CancelToken, Limits, ToolCtx,
};
use serde_json::json;

/// Layer 2's three tools, each with the variable that names it (design part 12.1).
const MKVMERGE: (&str, Tool) = ("LAZYMKV_TEST_MKVMERGE", Tool::Mkvmerge);
const FFPROBE: (&str, Tool) = ("LAZYMKV_TEST_FFPROBE", Tool::Ffprobe);
const FFMPEG: (&str, Tool) = ("LAZYMKV_TEST_FFMPEG", Tool::Ffmpeg);

/// The lavfi sources, 3 s each. VIDEO is a black 64 x 36 picture at 25 frames a second. SURROUND is 5.1 in ffmpeg's
/// order FL FR FC LFE BL BR: 440 Hz at 0.1 on the fronts, 1 kHz at 0.2 on the centre (a root mean square level of -17.0 dB), silence on
/// the LFE (low-frequency effects) channel, 300 Hz at 0.05 on the backs. STEREO is two equal channels of 700 Hz at
/// 0.1, so its side (0.5 L - 0.5 R) is silent. MONO is 500 Hz at 0.1.
const VIDEO: &str = "color=c=black:s=64x36:r=25:d=3";
const SURROUND: &str = "aevalsrc=0.1*sin(2*PI*440*t)|0.1*sin(2*PI*440*t)|0.2*sin(2*PI*1000*t)|0|0.05*sin(2*PI*300*t)|0.05*sin(2*PI*300*t):c=5.1:s=48000:d=3";
const STEREO: &str = "aevalsrc=0.1*sin(2*PI*700*t)|0.1*sin(2*PI*700*t):c=stereo:s=48000:d=3";
const MONO: &str = "aevalsrc=0.1*sin(2*PI*500*t):c=mono:s=48000:d=3";


/// What the tests hand a driver: a fake host, the give-up bounds and a cancel token, as
/// `mkvmerge_ffprobe_driver.rs` builds them. The guard is `StdProcessGuard` itself, since a real tool is not a freshly
/// copied stand-in and has nothing to retry, and the progress fraction is ignored. The 120 s header bound is a
/// give-up bound, not a wait, so it does not scale by `ci_factor()`.
struct Env { host: FakeHost, limits: Limits, cancel: CancelToken }

impl Env {
    fn new() -> Env {
        Env {
            host: FakeHost::new("2026-10-23", 1_800_000_000),
            limits: Limits { header: Duration::from_secs(120), scan: Duration::from_secs(60),
                exit_wait: Duration::from_secs(60), after_kill: Duration::from_millis(10) },
            cancel: CancelToken::default(),
        }
    }

    /// Runs `f` with a context over the real process guard and no output or progress listener.
    fn run<R>(&self, f: impl FnOnce(&ToolCtx) -> R) -> R {
        let on_output = |_: OutputStream, _: &str| {};
        let on_progress = |_: f64| {};
        let ctx = ToolCtx { guard: &StdProcessGuard, host: &self.host, limits: &self.limits,
            cancel: self.cancel.clone(), on_output: &on_output, on_progress: &on_progress };
        f(&ctx)
    }
}

/// The text of a file, panicking `<path>: <error>` when it cannot be read.
fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// A JSON value of ffprobe's listing as text: its string as written, another value as its JSON form, none as "".
/// ffprobe writes `pts_time` and `size` as strings.
fn text(value: &Option<serde_json::Value>) -> String {
    match value {
        Some(serde_json::Value::String(text)) => text.clone(),
        Some(other) => other.to_string(),
        None => String::new(),
    }
}

/// None when the first line of `-version` names FFmpeg 7.0 or later, else the reason it does not (K11). The release
/// word is the third when the second is `version`, none otherwise; `lazymkv_core::toolver::standing` judges it against
/// `FFMPEG_MIN`, the rule the self-test applies, so a dated git build has no release number here too (the review of
/// pull request #21 (X11)).
fn below_7_0(first_line: &str) -> Option<String> {
    let words: Vec<&str> = first_line.split_whitespace().collect();
    if words.is_empty() {
        return Some(
            "-version printed nothing, so layer 2 cannot tell that it is FFmpeg 7.0 or later (K11)".to_string(),
        );
    }
    let shown = words.iter().take(3).copied().collect::<Vec<_>>().join(" ");
    let release = if words.get(1) == Some(&"version") { words.get(2).copied() } else { None };
    let judged = match release {
        Some(word) => standing(word, FFMPEG_MIN),
        None => Standing::Unreadable,
    };
    match judged {
        Standing::AtLeast => None,
        Standing::Below => Some(format!("{shown} is below FFmpeg 7.0, layer 2's minimum (K11)")),
        Standing::Unreadable => Some(format!(
            "{shown} names no release number, so layer 2 cannot tell that it is FFmpeg 7.0 or later (K11)"
        )),
    }
}

/// One tool: the path in `var` when set and not empty (it must be a file), else the first match on `PATH`. ffmpeg and
/// ffprobe must also be FFmpeg 7.0 or later.
fn real_tool(var: &str, tool: Tool) -> Result<PathBuf, String> {
    let path = match std::env::var_os(var).filter(|value| !value.is_empty()) {
        Some(set) => {
            let path = PathBuf::from(set);
            if !path.is_file() {
                return Err(format!("{var} names {}, which is not a file", path.display()));
            }
            path
        }
        None => {
            let name = exe_name(tool);
            let found = std::env::var_os("PATH")
                .and_then(|paths| std::env::split_paths(&paths).map(|dir| dir.join(&name)).find(|path| path.is_file()));
            found.ok_or_else(|| format!("no {0} (set {var} or put {0} on PATH)", tool.as_str()))?
        }
    };
    if matches!(tool, Tool::Ffmpeg | Tool::Ffprobe) {
        let output = Command::new(&path)
            .arg("-version")
            .output()
            .map_err(|error| format!("{} -version could not run: {error}", path.display()))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some(reason) = below_7_0(stdout.lines().next().unwrap_or("")) {
            return Err(reason);
        }
    }
    Ok(path)
}

/// Prints `skipped: <reason>` and passes; on CI (`CI` set, as GitHub Actions sets it), where every runner installs
/// the three tools, fails instead, so a green run proves that layer 2 ran.
fn skip_or_fail(reason: &str) {
    if std::env::var_os("CI").is_some() {
        panic!("layer 2 must run on CI (call 20, T5-R49): {reason}");
    }
    lazymkv_dev::evidence::skip(reason);
}

/// The paths of the wanted tools in order, or None after a skip (or a CI failure) at the first one missing.
fn real_tools(wanted: &[(&str, Tool)]) -> Option<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for (var, tool) in wanted {
        match real_tool(var, *tool) {
            Ok(path) => paths.push(path),
            Err(reason) => {
                skip_or_fail(&reason);
                return None;
            }
        }
    }
    Some(paths)
}

/// The ffmpeg that makes the lavfi input files (T6-R8): the path LAZYMKV_TEST_FFMPEG_FULL names when it is set and
/// not empty, which must be a file and FFmpeg 7.0 or later, else `ffmpeg`, the tool under test. None after a skip (or
/// a CI failure) when the named one does not qualify. The release's minimal build has no lavfi, so its smoke job names
/// a full FFmpeg here and the minimal one as LAZYMKV_TEST_FFMPEG.
fn generator(ffmpeg: &Path) -> Option<PathBuf> {
    const FULL: &str = "LAZYMKV_TEST_FFMPEG_FULL";
    if std::env::var_os(FULL).filter(|value| !value.is_empty()).is_none() {
        return Some(ffmpeg.to_path_buf());
    }
    match real_tool(FULL, Tool::Ffmpeg) {
        Ok(path) => Some(path),
        Err(reason) => {
            skip_or_fail(&reason);
            None
        }
    }
}

/// Makes `dir/raw.mkv` with the real ffmpeg: one FFV1 video (stream 0) and one FLAC audio stream per `(source,
/// language)` (streams 1 to n), 3 s long, as a rip's streams are numbered. Both encoders are FFmpeg's own, so the file
/// needs no external library.
fn lavfi_mkv(ffmpeg: &Path, dir: &Path, audio: &[(&str, &str)]) -> PathBuf {
    let mut args: Vec<String> =
        ["-v", "error", "-nostdin", "-y", "-f", "lavfi", "-i", VIDEO].map(String::from).to_vec();
    for (source, _) in audio {
        args.extend(["-f", "lavfi", "-i", source].map(String::from));
    }
    args.extend(["-map", "0:v"].map(String::from));
    for k in 1..=audio.len() {
        args.extend(["-map".to_string(), format!("{k}:a")]);
    }
    args.extend(["-c:v", "ffv1", "-c:a", "flac"].map(String::from));
    for (k, (_, language)) in audio.iter().enumerate() {
        args.extend([format!("-metadata:s:a:{k}"), format!("language={language}")]);
    }
    args.extend(["-t", "3", "raw.mkv"].map(String::from));
    let output = Command::new(ffmpeg)
        .current_dir(dir)
        .args(&args)
        .output()
        .unwrap_or_else(|error| panic!("{} could not run: {error}", ffmpeg.display()));
    if !output.status.success() {
        panic!("ffmpeg could not make the test file (lavfi, ffv1, flac, matroska): {}",
            String::from_utf8_lossy(&output.stderr).trim());
    }
    dir.join("raw.mkv")
}

#[test]
fn real_mkvmerge_keeps_zlib_on_a_pgs_track_and_ffprobe_lists_its_inflated_sizes() {
    let Some(tools) = real_tools(&[MKVMERGE, FFPROBE]) else { return };
    let (mkvmerge, ffprobe) = (&tools[0], &tools[1]);
    let scratch = Scratch::new("layer2-zlib");
    let env = Env::new();
    let palette = [(1u8, [235u8, 128, 128, 255])];
    // Tall rectangles, so the four events inflate to about 22 KB: mkvmerge pads even a tiny file with about 5 KB of
    // EBML void elements, so events smaller than that could never show a file smaller than its inflated events.
    let payloads: Vec<Vec<u8>> = (0..4u32)
        .map(|k| pgs_display_set((1920, 1080), &[(0, 10, 10, 1900 - 100 * k, 800 + 80 * k, 1)], &palette))
        .collect();
    let sizes: Vec<u64> = payloads.iter().map(|p| p.len() as u64).collect();
    assert!(sizes.windows(2).all(|w| w[0] != w[1]), "the four made-up events need four sizes: {sizes:?}");
    // The raw file stores the PGS track plain, so any zlib in the evidence file is mkvmerge's own default (K1).
    let track = MksTrack { number: 1, kind: 17, codec_id: CODEC_PGS.to_string(), codec_private: Vec::new(),
        language: "eng".to_string(), encoding: MksEncoding::None };
    let times = [1_000i64, 2_500, 4_000, 61_103];
    let blocks: Vec<MksBlock> = times.iter().zip(&payloads).map(|(t, p)| MksBlock::new(1, *t, p.clone())).collect();
    let raw = scratch.write_path("raw.mks", mks(&[track], &blocks, &MksShape::default()));
    let job = scratch.0.join("job/t00");
    std::fs::create_dir_all(job.join("subs")).unwrap();
    let identified = env.run(|ctx| ident(ctx, mkvmerge, &raw, &job.join("mkvmerge.json"))).unwrap().unwrap();
    let kinds: Vec<(i64, &str, &str)> =
        identified.tracks.iter().map(|t| (t.id, t.r#type.as_str(), t.codec.as_str())).collect();
    assert_eq!(kinds, [(0, "subtitles", "HDMV PGS")]);
    let evidence = scratch.0.join("staging/j/evidence/t00.mks");
    std::fs::create_dir_all(evidence.parent().unwrap()).unwrap();
    let args = extract_args(&evidence, &[0], &raw);
    assert_eq!(env.run(|ctx| extract(ctx, mkvmerge, &args, &job.join("evidence-mkvmerge.txt"))).unwrap(), Ok(()));
    let bytes = std::fs::read(&evidence).unwrap();
    let inflated: u64 = sizes.iter().sum();
    assert!((bytes.len() as u64) < inflated,
        "mkvmerge wrote {} bytes for {inflated} bytes of events, so it did not compress the PGS track (K1)", bytes.len());
    let stream = read_stream(&bytes, 0).unwrap();
    let by_reader: Vec<(i64, u64)> = stream.blocks.iter().map(|b| ((b.pts * 1000.0).round() as i64, b.size)).collect();
    let made: Vec<(i64, u64)> = times.iter().copied().zip(sizes.iter().copied()).collect();
    assert_eq!(by_reader, made, "mkvmerge kept each event's time and bytes");
    let listing = job.join("subs/sub-0.packets.json");
    assert_eq!(env.run(|ctx| packets(ctx, ffprobe, &packets_args(0, &evidence), &listing)).unwrap(), Ok(()));
    let listed: PacketsFile = serde_json::from_slice(&std::fs::read(&listing).unwrap()).unwrap();
    let by_ffprobe: Vec<(i64, u64)> = listed
        .packets()
        .iter()
        .map(|p| ((text(&p.pts_time).parse::<f64>().unwrap() * 1000.0).round() as i64, text(&p.size).parse().unwrap()))
        .collect();
    assert_eq!(by_ffprobe, by_reader,
        "call 24: this runner's ffprobe lists other (pts ms, size) pairs than the reader over mkvmerge's file; stop and give the owner both lists");
    println!("cross-check over mkvmerge's file: {} packets, sizes {sizes:?}, equal in number, order and size", by_reader.len());
}

#[test]
fn real_ffmpeg_step_10a_pass_writes_phase_0s_level_files_over_a_lavfi_file() {
    let Some(tools) = real_tools(&[FFMPEG, FFPROBE]) else { return };
    let (ffmpeg, ffprobe) = (&tools[0], &tools[1]);
    let scratch = Scratch::new("layer2-pass");
    let env = Env::new();
    let Some(source) = generator(ffmpeg) else { return };
    let raw = lavfi_mkv(&source, &scratch.0, &[(SURROUND, "eng"), (STEREO, "spa")]);
    let label = scratch.0.join("job/t00");
    let work = scratch.0.join("work/levels-j-t00");
    std::fs::create_dir_all(&label).unwrap();
    std::fs::create_dir_all(&work).unwrap();
    let probed = env.run(|ctx| probe(ctx, ffprobe, &raw, &label.join("ffprobe-streams.json"))).unwrap().unwrap();
    let audio = pass_streams(&probed.streams);
    assert_eq!(audio.iter().map(|s| (s.index, s.channels)).collect::<Vec<_>>(), [(1, 6), (2, 2)]);
    let listed = env.run(|ctx| run_ffmpeg(ctx, ffmpeg, &layouts_args(), &work.join("ffmpeg-layouts.txt"), Some(&work)));
    let layouts = parse_layouts(&String::from_utf8_lossy(&listed.unwrap().unwrap().stdout)).unwrap();
    let main_centre = centre(&audio[0], &layouts);
    assert!(main_centre.has_fc && main_centre.guessed.is_none(), "{main_centre:?}");
    let graph = graph(&audio, 1, &[2], &layouts, true, GAIN_FIRST_DB);
    let series: Vec<(&str, &str)> = graph.series.iter().map(|s| (s.name.as_str(), s.raw.as_str())).collect();
    assert_eq!(series, [("main-mix", "raw-main-mix.txt"), ("main-fc", "raw-main-fc.txt"), ("cand-2", "raw-cand-2.txt")]);
    let proxy: Vec<(i64, &str)> = graph.proxy.iter().map(|t| (t.stream, t.series.as_str())).collect();
    assert_eq!(proxy, [(1, "mix"), (1, "fc"), (2, "mix"), (2, "side")]);
    std::fs::write(work.join("graph.txt"), &graph.text).unwrap();
    let args = ffmpeg_args(&raw.display().to_string(), &graph, true, Some("audio-proxy.mka.partial"));
    assert!(args.iter().any(|a| a == "-/filter_complex"), "K11: the pass hands its graph over as -/filter_complex graph.txt");
    let pass = env.run(|ctx| run_ffmpeg(ctx, ffmpeg, &args, &work.join("pass.txt"), Some(&work))).unwrap();
    pass.unwrap_or_else(|why| panic!("{why}: {}", read(&work.join("pass.txt.stderr"))));
    let hashes = parse_streamhash(&read(&work.join("streamhash.txt")), audio.len()).unwrap();
    assert_eq!(hashes.iter().map(|(k, _)| *k).collect::<Vec<_>>(), [0, 1]);
    for s in &graph.series {
        let file = series_file(&read(&work.join(&s.raw)), &s.name, "raw.mkv").unwrap();
        let header = format!(
            "# lazymkv audio levels v2: frame_ms=20 band_hz=300-3400 unit=dB floor=-100 series={} file=raw.mkv\r\n",
            s.name
        );
        assert!(file.starts_with(&header), "{}: {:?}", s.name, file.lines().next());
        assert!(file.ends_with("\r\n") && !file.replace("\r\n", "").contains('\n'), "{}: every line ends with CRLF", s.name);
        let values = parse_levels(&file).unwrap();
        assert!((149..=151).contains(&values.len()), "{}: {} frames of 20 ms for 3 s", s.name, values.len());
        assert!(values.iter().all(|v| (-100.0..=0.0).contains(v)), "{}: {values:?}", s.name);
        if s.name == "main-fc" {
            let mut sorted = values.clone();
            sorted.sort_by(f64::total_cmp);
            let median = sorted[sorted.len() / 2];
            assert!((median + 17.0).abs() <= 0.5, "main-fc's median is {median} dB; the centre's 1 kHz tone at 0.2 is -17.0 dB");
        }
    }
    assert!(work.join("audio-proxy.mka.partial").is_file());
    let check = env.run(|ctx| {
        run_ffmpeg(ctx, ffprobe, &proxy_check_args("audio-proxy.mka.partial"), &work.join("proxy-check.txt"), Some(&work))
    });
    check.unwrap().unwrap();
    let durations = proxy_check(&read(&work.join("proxy-check.json")), &graph.proxy).unwrap();
    assert_eq!(durations.len(), 4);
    assert!(durations.iter().all(|d| (2.9..=3.1).contains(d)), "{durations:?}");
    for (k, track) in graph.proxy.iter().enumerate() {
        assert_eq!(peak_clipped(&read(&work.join(&track.peak))), Ok(false), "proxy track {k} ({} {})", track.stream, track.series);
    }
    let side = last_peak(&read(&work.join(&graph.proxy[3].peak))).unwrap();
    assert!(side.contains("inf"), "the side of two equal channels is silent: {side}");
}

#[test]
fn real_ffmpeg_streamhash_pass_lists_every_audio_stream() {
    let Some(tools) = real_tools(&[FFMPEG]) else { return };
    let ffmpeg = &tools[0];
    let scratch = Scratch::new("layer2-streamhash");
    let env = Env::new();
    let Some(source) = generator(ffmpeg) else { return };
    let raw = lavfi_mkv(&source, &scratch.0, &[(SURROUND, "eng"), (STEREO, "spa"), (MONO, "eng")]);
    let work = scratch.0.join("work/levels-j-t00");
    std::fs::create_dir_all(&work).unwrap();
    let args = streamhash_args(&raw, "streamhash.txt");
    let run = env.run(|ctx| run_ffmpeg(ctx, ffmpeg, &args, &work.join("streamhash-ffmpeg.txt"), Some(&work))).unwrap();
    run.unwrap_or_else(|why| panic!("{why}: {}", read(&work.join("streamhash-ffmpeg.txt.stderr"))));
    let text = read(&work.join("streamhash.txt"));
    let hashes = parse_streamhash(&text, 3).unwrap();
    assert_eq!(hashes.iter().map(|(k, _)| *k).collect::<Vec<_>>(), [0, 1, 2]);
    assert!(hashes.iter().all(|(_, h)| h.len() == 32 && h.bytes().all(|b| b.is_ascii_hexdigit())), "{hashes:?}");
    assert_eq!(hashes.iter().map(|(_, h)| h).collect::<BTreeSet<_>>().len(), 3, "three streams, three digests");
    assert_eq!(parse_streamhash(&text, 4), Err("streamhash.txt holds 3 of 4 stream hashes".to_string()));
    assert!(!text.contains(",v,"), "only the audio streams are hashed: {text}");
}

#[test]
fn real_mkvmerge_identifies_and_remuxes_a_lavfi_file() {
    let Some(tools) = real_tools(&[MKVMERGE, FFMPEG]) else { return };
    let (mkvmerge, ffmpeg) = (&tools[0], &tools[1]);
    let scratch = Scratch::new("layer2-remux");
    let env = Env::new();
    let Some(source) = generator(ffmpeg) else { return };
    let raw = lavfi_mkv(&source, &scratch.0, &[(SURROUND, "spa"), (STEREO, "eng")]);
    let job = scratch.0.join("job/t00");
    std::fs::create_dir_all(&job).unwrap();
    let identified = env.run(|ctx| ident(ctx, mkvmerge, &raw, &job.join("mkvmerge.json"))).unwrap().unwrap();
    let tracks: Vec<serde_json::Value> = identified
        .tracks
        .iter()
        .map(|t| json!({"id": t.id, "type": t.r#type, "language": t.properties.get("language"),
            "channels": t.properties.get("audio_channels")}))
        .collect();
    assert_eq!(tracks, [
        json!({"id": 0, "type": "video", "language": "und", "channels": null}),
        json!({"id": 1, "type": "audio", "language": "spa", "channels": 6}),
        json!({"id": 2, "type": "audio", "language": "eng", "channels": 2}),
    ]);
    let seconds = identified.container.properties.duration as f64 / 1e9;
    assert!((2.9..=3.1).contains(&seconds), "{seconds} s");
    // The layout puts the English stereo first and the Spanish 5.1 second, so the remux reorders the tracks.
    let track = |stream: u32, kind: StreamKind, language: Option<&str>, name: Option<&str>, default: bool, original: bool| {
        OutputTrack { stream, kind, language: language.map(String::from), name: name.map(String::from), default,
            forced: false, hearing_impaired: false, original }
    };
    let layout = Layout {
        tracks: vec![
            track(0, StreamKind::Video, None, None, true, false),
            track(2, StreamKind::Audio, Some("eng"), Some("Stereo"), true, false),
            track(1, StreamKind::Audio, Some("spa"), Some("Surround 5.1"), false, true),
        ],
        chapters_kept: true,
    };
    let staged = VerifyReport { duration_ok: true, expected_seconds: seconds, file_seconds: seconds, chapters_ok: true,
        scan_chapters: 0, file_chapters: 0, tracks_ok: true, pairs: vec![(0, Some(0)), (1, Some(1)), (2, Some(2))],
        extra_tracks: Vec::new(), unknown_removed: Vec::new(), mismatches: Vec::new(), unchecked: Vec::new(),
        read_errors: Vec::new(), ok: true };
    let partial = scratch.0.join("lib/.lazymkv-partial/t00.mkv.partial");
    std::fs::create_dir_all(partial.parent().unwrap()).unwrap();
    let args = remux_args(&partial, &layout, &staged, &raw).unwrap();
    assert_eq!(env.run(|ctx| remux(ctx, mkvmerge, &args, &job.join("remux-mkvmerge.txt"))).unwrap(), Ok(()));
    let out = env.run(|ctx| ident(ctx, mkvmerge, &partial, &job.join("final-mkvmerge.json"))).unwrap().unwrap();
    let facts: Vec<serde_json::Value> = out
        .tracks
        .iter()
        .map(|t| {
            let p = |key: &str| t.properties.get(key).cloned();
            json!({"id": t.id, "type": t.r#type, "language": p("language"), "name": p("track_name"),
                "default": p("default_track"), "forced": p("forced_track"),
                "hearing_impaired": p("flag_hearing_impaired"), "original": p("flag_original"),
                "channels": p("audio_channels")})
        })
        .collect();
    assert_eq!(facts, [
        json!({"id": 0, "type": "video", "language": "und", "name": null, "default": true, "forced": false,
            "hearing_impaired": false, "original": false, "channels": null}),
        json!({"id": 1, "type": "audio", "language": "eng", "name": "Stereo", "default": true, "forced": false,
            "hearing_impaired": false, "original": false, "channels": 2}),
        json!({"id": 2, "type": "audio", "language": "spa", "name": "Surround 5.1", "default": false, "forced": false,
            "hearing_impaired": false, "original": true, "channels": 6}),
    ]);
    let final_seconds = out.container.properties.duration as f64 / 1e9;
    assert!((2.9..=3.1).contains(&final_seconds), "{final_seconds} s");
}

#[test]
fn the_layer_2_skip_reads_ffmpegs_version_and_skips_below_7_0() {
    assert_eq!(below_7_0("ffmpeg version 6.1.1-3ubuntu5 Copyright (c) 2000-2023 the FFmpeg developers"),
        Some("ffmpeg version 6.1.1-3ubuntu5 is below FFmpeg 7.0, layer 2's minimum (K11)".to_string()));
    assert_eq!(below_7_0("ffprobe version n6.0 Copyright (c) 2007-2023 the FFmpeg developers"),
        Some("ffprobe version n6.0 is below FFmpeg 7.0, layer 2's minimum (K11)".to_string()));
    for line in [
        "ffmpeg version 7.0 Copyright (c) 2000-2024 the FFmpeg developers",
        "ffmpeg version n7.1.2-12-g0123abcd-20251020 Copyright (c) 2000-2025 the FFmpeg developers",
        "ffprobe version 9.0.2-full_build-www.gyan.dev Copyright (c) 2007-2026 the FFmpeg developers",
        "ffmpeg version 10.0 Copyright (c) 2000-2027 the FFmpeg developers",
    ] {
        assert_eq!(below_7_0(line), None, "{line}");
    }
    assert_eq!(below_7_0("ffmpeg version N-117000-g0123abcd Copyright (c) 2000-2025 the FFmpeg developers"),
        Some("ffmpeg version N-117000-g0123abcd names no release number, so layer 2 cannot tell that it is FFmpeg 7.0 or later (K11)".to_string()));
    let no_number = "names no release number, so layer 2 cannot tell that it is FFmpeg 7.0 or later (K11)";
    assert_eq!(below_7_0("ffmpeg version 2026-10-01-git-0a1b2c3d4e Copyright (c) 2000-2026 the FFmpeg developers"),
        Some(format!("ffmpeg version 2026-10-01-git-0a1b2c3d4e {no_number}")));
    assert_eq!(below_7_0("   "),
        Some("-version printed nothing, so layer 2 cannot tell that it is FFmpeg 7.0 or later (K11)".to_string()));
}
