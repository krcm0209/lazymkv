//! The ffmpeg driver against the stand-in (design parts 5.3, 5.7 step 10a; R5-code-27; Ruling S12).

use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use lazymkv_core::ipc::{OutputStream, CANCELLED};
use lazymkv_core::testsupport::{Scratch, ScratchExt};
use lazymkv_engine::testkit::{ci_factor, BusyRetry};
use lazymkv_platform::fakes::FakeHost;
use lazymkv_platform::{GuardedChild, ProcessGuard, Spawn, StdProcessGuard};
use lazymkv_tools::{layouts_args, proxy_check_args, run_ffmpeg, streamhash_args, with_suffix, CancelToken, Limits, ToolCtx};

const EXE: &str = std::env::consts::EXE_SUFFIX;
const HASHES: &str = "0,a,MD5=0123456789abcdef0123456789abcdef\n1,a,MD5=fedcba9876543210fedcba9876543210\n";
const LAYOUTS: &str = "Standard channel layouts:\nNAME           DECOMPOSITION\nmono           FC\nstereo         FL+FR\n5.1            FL+FR+FC+LFE+BL+BR\n";

fn stand_in(scratch: &Scratch, stem: &str, scenario: &str, files: &[(&str, &[u8])]) -> PathBuf {
    let program = scratch.0.join("tools").join(format!("{stem}{EXE}"));
    scratch.write(&format!("tools/{stem}.json"), scenario);
    for (name, bytes) in files {
        scratch.write(&format!("tools/{name}"), bytes);
    }
    fs::copy(env!("CARGO_BIN_EXE_fake-tool"), &program).unwrap();
    program
}

/// The stand-in's runner, keeping every `Spawn` it is given.
#[derive(Default)]
struct Recording {
    spawns: Mutex<Vec<Spawn>>,
}

impl ProcessGuard for Recording {
    fn spawn(&self, spawn: &Spawn) -> io::Result<Box<dyn GuardedChild>> {
        self.spawns.lock().unwrap().push(spawn.clone());
        BusyRetry(StdProcessGuard).spawn(spawn)
    }
}

/// Runs `f` with a context over `guard`; a kill pauses 300 ms (S56).
fn run<R>(guard: &dyn ProcessGuard, cancel: &CancelToken, f: impl FnOnce(&ToolCtx) -> R) -> R {
    let host = FakeHost::new("2026-10-23", 1_800_000_000);
    let limits = Limits { header: Duration::from_secs(120), scan: Duration::from_secs(60),
        exit_wait: Duration::from_secs(60), after_kill: Duration::from_millis(300) };
    let on_output = |_: OutputStream, _: &str| {};
    let on_progress = |_: f64| {};
    let ctx = ToolCtx { guard, host: &host, limits: &limits, cancel: cancel.clone(), on_output: &on_output,
        on_progress: &on_progress };
    f(&ctx)
}

#[test]
fn run_ffmpeg_runs_below_normal_in_its_working_folder() {
    let scratch = Scratch::new("ffmpeg-cwd");
    // arg_after "md5" names the relative file streamhash.txt, which the stand-in creates in its own working folder.
    let program = stand_in(&scratch, "ffmpeg", r#"{"responses":[{"args_contain":["-f","streamhash"],
        "creates":[{"arg_after":"md5","copy_from":"hashes.txt"}]}]}"#, &[("hashes.txt", HASHES.as_bytes())]);
    let work = scratch.0.join("work").join("levels-j-t00");
    fs::create_dir_all(&work).unwrap();
    let out = scratch.0.join("job").join("t00").join("streamhash-ffmpeg.txt");
    fs::create_dir_all(out.parent().unwrap()).unwrap();
    let args = streamhash_args(&scratch.0.join("staging").join("raw.mkv"), "streamhash.txt");
    let guard = Recording::default();
    let ran = run(&guard, &CancelToken::default(), |ctx| run_ffmpeg(ctx, &program, &args, &out, Some(&work)));
    assert_eq!(ran.unwrap().unwrap().exit_code, Some(0));
    assert_eq!(fs::read_to_string(work.join("streamhash.txt")).unwrap(), HASHES);
    let spawns = guard.spawns.lock().unwrap();
    assert_eq!(spawns.len(), 1);
    assert_eq!((spawns[0].program.as_path(), spawns[0].args.as_slice()), (program.as_path(), args.as_slice()));
    assert!(spawns[0].below_normal, "step 10a's ffmpeg runs below normal (design part 5.3)");
    assert_eq!(spawns[0].cwd.as_deref(), Some(work.as_path()));
    assert!(spawns[0].env.is_empty(), "{:?}", spawns[0].env);
    for suffix in ["", ".stderr", ".timing.jsonl", ".exit.json"] {
        assert!(with_suffix(&out, suffix).is_file(), "no capture file {}{suffix}", out.display());
    }
}

#[test]
fn run_ffmpeg_returns_the_layouts_table_on_standard_output() {
    let scratch = Scratch::new("ffmpeg-layouts");
    let program = stand_in(&scratch, "ffmpeg", r#"{"responses":[{"args_contain":["-layouts"],"stdout":"layouts.txt"}]}"#,
        &[("layouts.txt", LAYOUTS.as_bytes())]);
    let out = scratch.0.join("work").join("levels-j-t00").join("layouts.txt");
    fs::create_dir_all(out.parent().unwrap()).unwrap();
    let guard = Recording::default();
    let ran = run(&guard, &CancelToken::default(), |ctx| run_ffmpeg(ctx, &program, &layouts_args(), &out, None));
    assert_eq!(ran.unwrap().unwrap().stdout, LAYOUTS.as_bytes());
    assert_eq!(fs::read_to_string(&out).unwrap(), LAYOUTS);
    assert_eq!(guard.spawns.lock().unwrap()[0].cwd, None);
}

#[test]
fn a_failed_run_names_the_program_and_its_exit_code() {
    let scratch = Scratch::new("ffmpeg-exit");
    let ffmpeg = stand_in(&scratch, "ffmpeg", r#"{"responses":[{"args_contain":["-layouts"],"exit_code":1}]}"#, &[]);
    let ffprobe = stand_in(&scratch, "ffprobe", r#"{"responses":[{"args_contain":["proxy-check.json"],"exit_code":1}]}"#, &[]);
    let work = scratch.0.join("work").join("levels-j-t00");
    fs::create_dir_all(&work).unwrap();
    let guard = Recording::default();
    let cancel = CancelToken::default();
    let ffmpeg_ran = run(&guard, &cancel, |ctx| run_ffmpeg(ctx, &ffmpeg, &layouts_args(), &work.join("layouts.txt"), None));
    assert_eq!(ffmpeg_ran.unwrap().unwrap_err(), "ffmpeg exited with code 1");
    let check = proxy_check_args("audio-proxy.mka.partial");
    let ffprobe_ran = run(&guard, &cancel, |ctx| run_ffmpeg(ctx, &ffprobe, &check, &work.join("proxy-check.txt"), Some(&work)));
    assert_eq!(ffprobe_ran.unwrap().unwrap_err(), "ffprobe exited with code 1");
}

#[test]
fn a_cancelled_run_is_cancelled_and_pauses_after_the_kill() {
    let scratch = Scratch::new("ffmpeg-cancel");
    let program = stand_in(&scratch, "ffmpeg",
        r#"{"responses":[{"args_contain":["-f","streamhash"],"stdout":"hashes.txt","hang_after_line":0}]}"#,
        &[("hashes.txt", HASHES.as_bytes())]);
    let work = scratch.0.join("work").join("levels-j-t00");
    fs::create_dir_all(&work).unwrap();
    let cancel = CancelToken::default();
    let delay = Duration::from_millis(200) * ci_factor();
    let later = cancel.clone();
    std::thread::spawn(move || {
        std::thread::sleep(delay);
        later.cancel();
    });
    let started = Instant::now();
    let args = streamhash_args(&scratch.0.join("raw.mkv"), "streamhash.txt");
    let ran = run(&Recording::default(), &cancel, |ctx| run_ffmpeg(ctx, &program, &args, &work.join("ffmpeg.txt"), Some(&work)));
    assert_eq!(ran.unwrap().unwrap_err(), CANCELLED);
    assert!(started.elapsed() >= delay + Duration::from_millis(300), "the driver returned before limits.after_kill (S56)");
}
