//! The mkvmerge and ffprobe drivers against the stand-in (design parts 5.3, 5.7, 5.11; Ruling S12), step 10's
//! extraction and packet listing, and call 24's size cross-check against the runner's own ffprobe.

use lazymkv_core::ipc::OutputStream;
use lazymkv_core::job::Tool;
use lazymkv_core::recording::files::PacketsFile;
use lazymkv_core::subtitle::bitmap::mkv::read_stream;
use lazymkv_core::subtitle::bitmap::CODEC_PGS;
use lazymkv_core::testsupport::mks::{mks, pgs_display_set, MksBlock, MksEncoding, MksShape, MksTrack};
use lazymkv_core::testsupport::{Scratch, ScratchExt};
use lazymkv_platform::fakes::FakeHost;
use lazymkv_engine::testkit::BusyRetry;
use lazymkv_platform::StdProcessGuard;
use lazymkv_tools::{exe_name, extract, extract_args, ident, packets, packets_args, probe, remux, CancelToken, Limits, ToolCtx};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const EXE: &str = std::env::consts::EXE_SUFFIX;
const IDENT: &str = r#"{"container":{"properties":{"duration":7150000000000}},"tracks":[{"id":0,"type":"video","codec":"MPEG-H/HEVC/H.265","properties":{"language":"und"}},{"id":1,"type":"audio","codec":"DTS-HD Master Audio","properties":{"language":"eng","audio_channels":6}}],"chapters":[{"num_entries":12}]}
"#;

fn stand_in(scratch: &Scratch, stem: &str, scenario: &str, files: &[(&str, &[u8])]) -> PathBuf {
    let program = scratch.0.join("tools").join(format!("{stem}{EXE}"));
    scratch.write(&format!("tools/{stem}.json"), scenario);
    for (name, bytes) in files {
        scratch.write(&format!("tools/{name}"), bytes);
    }
    fs::copy(env!("CARGO_BIN_EXE_fake-tool"), &program).unwrap();
    program
}

struct Env { host: FakeHost, limits: Limits, cancel: CancelToken, progress: Mutex<Vec<f64>> }

impl Env {
    fn new() -> Env {
        Env {
            host: FakeHost::new("2026-10-23", 1_800_000_000),
            limits: Limits { header: Duration::from_secs(120), scan: Duration::from_secs(60),
                exit_wait: Duration::from_secs(60), after_kill: Duration::from_millis(10) },
            cancel: CancelToken::default(),
            progress: Mutex::new(Vec::new()),
        }
    }

    /// Cancels after 30 s, so that a broken kill fails the test instead of hanging it (as `makemkv_driver`'s does).
    fn watchdog(&self) {
        let cancel = self.cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(30));
            cancel.cancel();
        });
    }

    fn run<R>(&self, f: impl FnOnce(&ToolCtx) -> R) -> R {
        let on_output = |_: OutputStream, _: &str| {};
        let on_progress = |fraction: f64| self.progress.lock().unwrap().push(fraction);
        let guard = BusyRetry(StdProcessGuard);
        let ctx = ToolCtx { guard: &guard, host: &self.host, limits: &self.limits, cancel: self.cancel.clone(),
            on_output: &on_output, on_progress: &on_progress };
        f(&ctx)
    }
}

#[test]
fn ident_accepts_exit_0_and_exit_1() {
    let scratch = Scratch::new("ident-0-1");
    let env = Env::new();
    let program = stand_in(&scratch, "mkvmerge", r#"{"responses":[
        {"args_contain":["-J","a.mkv"],"stdout":"ident.json"},
        {"args_contain":["-J","b.mkv"],"stdout":"ident.json","exit_code":1}]}"#, &[("ident.json", IDENT.as_bytes())]);
    for name in ["a.mkv", "b.mkv"] {
        let out = scratch.0.join(format!("job/{name}.json"));
        fs::create_dir_all(out.parent().unwrap()).unwrap();
        let read = env.run(|ctx| ident(ctx, &program, &scratch.0.join(name), &out)).unwrap().unwrap();
        assert_eq!(read.tracks.len(), 2, "{name}");
        assert_eq!(read.container.properties.duration, 7_150_000_000_000);
        assert_eq!(fs::read_to_string(&out).unwrap(), IDENT);
    }
}

#[test]
fn ident_refuses_another_exit_code() {
    let scratch = Scratch::new("ident-2");
    let env = Env::new();
    let program = stand_in(&scratch, "mkvmerge",
        r#"{"responses":[{"args_contain":["-J"],"stdout":"ident.json","exit_code":2}]}"#, &[("ident.json", IDENT.as_bytes())]);
    let out = scratch.0.join("job/mkvmerge.json");
    fs::create_dir_all(out.parent().unwrap()).unwrap();
    assert_eq!(env.run(|ctx| ident(ctx, &program, &scratch.0.join("t.mkv"), &out)).unwrap().unwrap_err(),
        "mkvmerge -J exited with code 2");
}

#[test]
fn an_ident_past_the_header_limit_fails_saying_so() {
    let scratch = Scratch::new("ident-limit");
    let mut env = Env::new();
    env.limits.header = Duration::from_millis(300);
    env.limits.after_kill = Duration::from_millis(300);
    env.watchdog();
    let program = stand_in(&scratch, "mkvmerge",
        r#"{"responses":[{"args_contain":["-J"],"stdout":"ident.json","hang_after_line":0}]}"#, &[("ident.json", IDENT.as_bytes())]);
    let out = scratch.0.join("job/mkvmerge.json");
    fs::create_dir_all(out.parent().unwrap()).unwrap();
    let started = Instant::now();
    let result = env.run(|ctx| ident(ctx, &program, &scratch.0.join("t.mkv"), &out)).unwrap();
    assert!(result.unwrap_err().starts_with("mkvmerge -J did not finish within"));
    assert!(started.elapsed() >= Duration::from_millis(600), "the driver returned before limits.after_kill (S56)");
}

#[test]
fn remux_progress_lines_reach_the_progress_callback() {
    let scratch = Scratch::new("remux");
    let env = Env::new();
    let program = stand_in(&scratch, "mkvmerge", r#"{"responses":[{"args_contain":["--gui-mode","-o"],"stdout":"remux.txt",
        "creates":[{"arg_after":"-o","copy_from":"placeholder.mkv"}]}]}"#,
        &[("remux.txt", b"#GUI#progress 45%\n#GUI#warning synthetic\n#GUI#progress 100%\n"), ("placeholder.mkv", b"placeholder bytes")]);
    let partial = scratch.0.join("lib/.lazymkv-partial/j-1-t00-00800.mkv.partial");
    fs::create_dir_all(partial.parent().unwrap()).unwrap();
    let args = vec!["--gui-mode".to_string(), "-o".to_string(), partial.display().to_string(), "raw.mkv".to_string()];
    let out = scratch.0.join("job/t00-00800/remux-mkvmerge.txt");
    fs::create_dir_all(out.parent().unwrap()).unwrap();
    assert_eq!(env.run(|ctx| remux(ctx, &program, &args, &out)).unwrap(), Ok(()));
    assert_eq!(*env.progress.lock().unwrap(), vec![0.45, 1.0]);
    assert_eq!(fs::read(&partial).unwrap(), b"placeholder bytes");
}

#[test]
fn a_remux_accepts_exit_1_and_refuses_exit_2() {
    let scratch = Scratch::new("remux-exit");
    let env = Env::new();
    let program = stand_in(&scratch, "mkvmerge", r#"{"responses":[
        {"args_contain":["--gui-mode","one.partial"],"exit_code":1},
        {"args_contain":["--gui-mode","two.partial"],"exit_code":2}]}"#, &[]);
    let out = scratch.0.join("job/remux-mkvmerge.txt");
    fs::create_dir_all(out.parent().unwrap()).unwrap();
    let args = |name: &str| vec!["--gui-mode".to_string(), "-o".to_string(), name.to_string(), "raw.mkv".to_string()];
    assert_eq!(env.run(|ctx| remux(ctx, &program, &args("one.partial"), &out)).unwrap(), Ok(()));
    assert_eq!(env.run(|ctx| remux(ctx, &program, &args("two.partial"), &out)).unwrap(),
        Err("mkvmerge exited with code 2".to_string()));
}

#[test]
fn a_probe_parses_the_streams() {
    let scratch = Scratch::new("probe");
    let env = Env::new();
    let streams = br#"{"streams":[{"index":0,"codec_type":"video","codec_name":"hevc"},{"index":1,"codec_type":"audio","codec_name":"dts","profile":"DTS-HD MA"}],"format":{"duration":"7150.000000"}}
"#;
    let program = stand_in(&scratch, "ffprobe", r#"{"responses":[{"args_contain":["-show_streams"],"stdout":"streams.json"}]}"#,
        &[("streams.json", streams)]);
    let out = scratch.0.join("job/t00-00800/ffprobe-streams.json");
    fs::create_dir_all(out.parent().unwrap()).unwrap();
    let read = env.run(|ctx| probe(ctx, &program, &scratch.0.join("raw.mkv"), &out)).unwrap().unwrap();
    assert_eq!(read.streams.len(), 2);
    assert_eq!(read.streams[1].codec_name.as_deref(), Some("dts"));
    assert_eq!(read.streams[1].profile.as_deref(), Some("DTS-HD MA"));
    assert_eq!(fs::read(&out).unwrap(), streams);
}

const PACKETS: &str = r#"{"packets":[{"pts_time":"1.000000","duration_time":"2.000000","size":"2029"}]}
"#;

#[test]
fn extraction_progress_lines_reach_the_progress_callback() {
    let scratch = Scratch::new("extract");
    let env = Env::new();
    let program = stand_in(&scratch, "mkvmerge", r#"{"responses":[{"args_contain":["--gui-mode","-D","-A","-s"],
        "stdout":"extract.txt","creates":[{"arg_after":"-o","copy_from":"evidence.mks"}]}]}"#,
        &[("extract.txt", b"#GUI#progress 50%\n#GUI#progress 100%\n"), ("evidence.mks", b"made-up evidence bytes")]);
    let evidence = scratch.0.join("staging/j/evidence/t00.mks");
    fs::create_dir_all(evidence.parent().unwrap()).unwrap();
    let args = extract_args(&evidence, &[3, 4], &scratch.0.join("raw.mkv"));
    let out = scratch.0.join("job/t00/evidence-mkvmerge.txt");
    fs::create_dir_all(out.parent().unwrap()).unwrap();
    assert_eq!(env.run(|ctx| extract(ctx, &program, &args, &out)).unwrap(), Ok(()));
    assert_eq!(*env.progress.lock().unwrap(), vec![0.5, 1.0]);
    assert_eq!(fs::read(&evidence).unwrap(), b"made-up evidence bytes");
}

#[test]
fn an_extraction_accepts_exit_1_and_refuses_exit_2() {
    let scratch = Scratch::new("extract-exit");
    let env = Env::new();
    let program = stand_in(&scratch, "mkvmerge", r#"{"responses":[
        {"args_contain":["-s","one.mks"],"exit_code":1},
        {"args_contain":["-s","two.mks"],"exit_code":2}]}"#, &[]);
    let out = scratch.0.join("job/t00/evidence-mkvmerge.txt");
    fs::create_dir_all(out.parent().unwrap()).unwrap();
    let args = |name: &str| extract_args(std::path::Path::new(name), &[3], std::path::Path::new("raw.mkv"));
    assert_eq!(env.run(|ctx| extract(ctx, &program, &args("one.mks"), &out)).unwrap(), Ok(()));
    assert_eq!(env.run(|ctx| extract(ctx, &program, &args("two.mks"), &out)).unwrap(),
        Err("mkvmerge exited with code 2".to_string()));
}

#[test]
fn a_packet_listing_keeps_ffprobes_json_and_refuses_another_exit_code() {
    let scratch = Scratch::new("packets");
    let env = Env::new();
    let program = stand_in(&scratch, "ffprobe", r#"{"responses":[
        {"args_contain":["s:0","packet=pts_time"],"stdout":"packets.json"},
        {"args_contain":["s:1","packet=pts_time"],"exit_code":1}]}"#, &[("packets.json", PACKETS.as_bytes())]);
    let evidence = scratch.0.join("t00.mks");
    let out = |n: i64| scratch.0.join(format!("job/t00/subs/sub-{n}.packets.json"));
    fs::create_dir_all(out(0).parent().unwrap()).unwrap();
    assert_eq!(env.run(|ctx| packets(ctx, &program, &packets_args(0, &evidence), &out(0))).unwrap(), Ok(()));
    assert_eq!(fs::read_to_string(out(0)).unwrap(), PACKETS);
    assert_eq!(env.run(|ctx| packets(ctx, &program, &packets_args(1, &evidence), &out(1))).unwrap(),
        Err("ffprobe exited with code 1".to_string()));
}

/// The ffprobe call 24's cross-check runs: `LAZYMKV_TEST_FFPROBE`, else the first `ffprobe` on `PATH` (K11's lookup
/// for layer 2, with no version floor here).
fn installed_ffprobe() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("LAZYMKV_TEST_FFPROBE") {
        return Some(PathBuf::from(path));
    }
    let name = exe_name(Tool::Ffprobe);
    std::env::split_paths(&std::env::var_os("PATH")?).map(|dir| dir.join(&name)).find(|path| path.is_file())
}

/// A JSON value of ffprobe's listing as text: its string, else its JSON form.
fn text(value: &Option<serde_json::Value>) -> String {
    match value {
        Some(serde_json::Value::String(text)) => text.clone(),
        Some(other) => other.to_string(),
        None => String::new(),
    }
}

#[test]
fn the_runners_ffprobe_lists_a_zlib_pgs_tracks_inflated_sizes_as_the_reader_does() {
    let Some(ffprobe) = installed_ffprobe() else {
        assert!(std::env::var_os("CI").is_none(), "CI has no ffprobe, so call 24's cross-check cannot run on this runner");
        println!("skipped: no ffprobe (set LAZYMKV_TEST_FFPROBE or put ffprobe on PATH), so call 24's cross-check did not run");
        return;
    };
    let scratch = Scratch::new("cross-check");
    let env = Env::new();
    let palette = [(1u8, [235u8, 128, 128, 255])];
    let payloads: Vec<Vec<u8>> = (0..4u32)
        .map(|k| pgs_display_set((1920, 1080), &[(0, 10, 10, 1900 - 100 * k, 200 + 20 * k, 1)], &palette))
        .collect();
    let sizes: Vec<u64> = payloads.iter().map(|p| p.len() as u64).collect();
    assert!(sizes.windows(2).all(|w| w[0] != w[1]), "the four made-up events need four sizes: {sizes:?}");
    let track = MksTrack { number: 1, kind: 17, codec_id: CODEC_PGS.to_string(), codec_private: Vec::new(),
        language: "eng".to_string(), encoding: MksEncoding::Zlib };
    let times = [1_000i64, 2_500, 4_000, 61_103];
    let blocks: Vec<MksBlock> = times.iter().zip(&payloads).map(|(t, p)| MksBlock::new(1, *t, p.clone())).collect();
    let bytes = mks(&[track], &blocks, &MksShape::default());
    let inflated: u64 = sizes.iter().sum();
    assert!((bytes.len() as u64) < inflated, "the file is {} bytes for {inflated} inflated, so zlib did not apply", bytes.len());
    let evidence = scratch.write_path("staging/j/evidence/t00.mks", &bytes);
    let out = scratch.0.join("job/t00/subs/sub-0.packets.json");
    fs::create_dir_all(out.parent().unwrap()).unwrap();
    assert_eq!(env.run(|ctx| packets(ctx, &ffprobe, &packets_args(0, &evidence), &out)).unwrap(), Ok(()));
    let listed: PacketsFile = serde_json::from_slice(&fs::read(&out).unwrap()).unwrap();
    let by_ffprobe: Vec<(i64, u64)> = listed
        .packets()
        .iter()
        .map(|p| ((text(&p.pts_time).parse::<f64>().unwrap() * 1000.0).round() as i64, text(&p.size).parse().unwrap()))
        .collect();
    let stream = read_stream(&bytes, 0).unwrap();
    let by_reader: Vec<(i64, u64)> = stream.blocks.iter().map(|b| ((b.pts * 1000.0).round() as i64, b.size)).collect();
    assert_eq!(by_reader.iter().map(|r| r.1).collect::<Vec<_>>(), sizes);
    assert_eq!(by_ffprobe, by_reader,
        "call 24: this runner's ffprobe lists other (pts ms, size) pairs than the reader; stop and give the owner both lists");
    println!("cross-check: {} packets, sizes {sizes:?}, equal in number, order and size", by_reader.len());
}
