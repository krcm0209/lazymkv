//! The mkvmerge and ffprobe drivers against the stand-in (design parts 5.3, 5.7, 5.11; Ruling S12).

use lazymkv_core::ipc::OutputStream;
use lazymkv_core::testsupport::{Scratch, ScratchExt};
use lazymkv_platform::fakes::FakeHost;
use lazymkv_platform::StdProcessGuard;
use lazymkv_tools::{ident, probe, remux, CancelToken, Limits, ToolCtx};
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

struct Env { host: FakeHost, limits: Limits, progress: Mutex<Vec<f64>> }

impl Env {
    fn new() -> Env {
        Env {
            host: FakeHost::new("2026-10-23", 1_800_000_000),
            limits: Limits { header: Duration::from_secs(120), scan: Duration::from_secs(60),
                exit_wait: Duration::from_secs(60), after_kill: Duration::from_millis(10) },
            progress: Mutex::new(Vec::new()),
        }
    }

    fn run<R>(&self, f: impl FnOnce(&ToolCtx) -> R) -> R {
        let on_output = |_: OutputStream, _: &str| {};
        let on_progress = |fraction: f64| self.progress.lock().unwrap().push(fraction);
        let ctx = ToolCtx { guard: &StdProcessGuard, host: &self.host, limits: &self.limits, cancel: CancelToken::default(),
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
