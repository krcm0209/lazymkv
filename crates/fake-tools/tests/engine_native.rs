//! The engine binary's Windows start and the native whole job (design parts 3.1 to 3.8, 4.2; S28; Rulings W1 to W5,
//! W19, W26, W27, T3-23-a to T3-23-d). Every engine here uses names of its own (`test_name`), so none meets the desktop
//! task's engine or another test.
#![cfg(windows)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::{Duration, Instant};

use lazymkv_clients::OfflineTransport;
use lazymkv_core::ipc::CANCELLED;
use lazymkv_core::testsupport::Scratch;
use lazymkv_ctl::{run, Env};
use lazymkv_dev::evidence::{skip, Ctx};
use lazymkv_engine::native::{run_with, NativeOptions, ENGINE_RUNNING, NOT_ELEVATED};
use lazymkv_engine::testkit::{ci_factor, Harness};
use lazymkv_platform::security::{test_name, TokenFacts};
use lazymkv_platform::fakes::{MemorySecrets, RecordingEjector, ScriptedDrives};
use lazymkv_platform::win::{own_token, EngineMutex, PipeConnector, PipeServer, WinHost};
use lazymkv_platform::{ConnectError, Host, IpcConnector, StdProcessGuard};
use serde_json::{json, Value};

const KEY: &str = "t00-00800";
const FINAL: &str = "Synthetic Movie (2001) {tmdb-900001}/Synthetic Movie (2001) {tmdb-900001} - 1080p.mkv";
/// S55's names (crates/tools/src/makemkv.rs:353).
const MAKEMKV: [&str; 3] = ["makemkvcon64", "makemkvcon", "makemkv"];
/// A whole stand-in job's cap: 120 s, 480 s under CI (Ruling R29.c5).
const JOB_CAP: Duration = Duration::from_secs(120);

fn fake_tool() -> &'static Path { Path::new(env!("CARGO_BIN_EXE_fake-tool")) }

fn settings() -> Value { json!({"keep_tuning_evidence": false, "drive_settle_s": 0, "drive_ready_timeout_s": 1}) }

/// Ruling W27: S55 stops every stand-in `makemkvcon` call while a MakeMKV process runs, and the MakeMKV interface may
/// be open on the mini PC. True, after printing the skip line, when one runs and `CI` is unset; never true on CI.
fn makemkv_runs_here() -> bool {
    if std::env::var_os("CI").is_some() {
        return false;
    }
    let running = WinHost::new().running_processes().into_iter().any(|name| {
        let lower = name.to_lowercase();
        MAKEMKV.contains(&lower.strip_suffix(".exe").unwrap_or(&lower))
    });
    if running {
        skip("a MakeMKV process is running on this machine (S55)");
    }
    running
}

/// A test's own mutex, pipe and data folder; the folder is not created.
fn names(tag: &str) -> (String, String, PathBuf) {
    let name = test_name(tag);
    (format!(r"Global\{name}"), format!(r"\\.\pipe\{name}"), std::env::temp_dir().join(format!("{name}-data")))
}

/// Test parts for every service, so a start a refusal wrongly let through meets no drive, vault, console or real
/// `makemkvcon64.exe` (Global Constraints; design part 3.1). Only the token stays None: the real token is what the
/// refusal tests read.
fn options(mutex: &str, pipe: &str) -> NativeOptions {
    NativeOptions {
        mutex: mutex.to_string(),
        pipe: pipe.to_string(),
        drives: Some(Box::new(ScriptedDrives::new().0)),
        ejector: Some(Arc::new(RecordingEjector::new())),
        processes: Some(Arc::new(StdProcessGuard)),
        secrets: Some(Arc::new(MemorySecrets::new())),
        http: Some(Arc::new(OfflineTransport)),
        console: Some(Box::new(std::io::sink())),
        console_stop: Some(Box::new(|_| Ok(()))),
        token: None,
    }
}

/// `run_with(["--data", data], .., options)` on a thread: its exit code and standard error. Fails the test when it
/// has not returned within 30 s (120 s under CI), since it then started an engine instead of refusing.
fn refused(data: &Path, options: NativeOptions) -> (i32, String) {
    let args = vec!["--data".to_string(), data.display().to_string()];
    let (send, got) = channel();
    std::thread::spawn(move || {
        let mut err = Vec::new();
        let code = run_with(&args, &mut err, options);
        let _ = send.send((code, String::from_utf8(err).unwrap()));
    });
    let cap = Duration::from_secs(30) * ci_factor();
    got.recv_timeout(cap).unwrap_or_else(|_| panic!("run_with did not return within {cap:?}: it started an engine"))
}

/// Polls every 20 ms until `done` holds; fails naming `what` after `cap` times `ci_factor`.
fn wait_for(h: &Harness, what: &str, cap: Duration, done: impl Fn(&Harness) -> bool) {
    let deadline = Instant::now() + cap * ci_factor();
    while !done(h) {
        assert!(Instant::now() < deadline, "waited {:?} for {what}", cap * ci_factor());
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// lazymkvctl through the native harness's real pipe client.
fn ctl(h: &Harness, argv: &[&str]) -> (i32, String, String) {
    let scratch = Scratch::new("native-ctl");
    let ctx = Ctx::for_test(&scratch.0);
    let connector = h.pipe_client();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let argv: Vec<String> = argv.iter().map(|word| word.to_string()).collect();
    let code = run(&argv, Env { ctx: &ctx, connector: Some(&connector), stdin: &mut &b""[..], out: &mut out, err: &mut err });
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

#[test]
fn a_native_engine_files_a_stand_in_disc_through_the_real_pipe() {
    if makemkv_runs_here() {
        return;
    }
    let pipe = format!(r"\\.\pipe\{}", test_name("native-job"));
    let h = Harness::new_native(fake_tool(), &["movie-direct"], settings(), &pipe);
    let (code, out, err) = ctl(&h, &["status"]);
    assert_eq!((code, err.as_str()), (0, ""));
    assert_eq!(serde_json::from_str::<Value>(&out).unwrap()["jobs"], json!([]));
    h.insert("tok0", "movie-direct");
    wait_for(&h, "the job to be Filed", JOB_CAP, |h| h.job_ids().first().is_some_and(|id| h.state_of(id) == "Filed"));
    assert!(h.library().join(FINAL).is_file());
    let log = h.engine_log();
    assert!(log.contains("was not delivered: no notification delivery in this build; it arrives in pull request 4"), "{log}");
    assert!(!log.contains("refused a pipe client"), "{log}");
    assert_eq!(ctl(&h, &["stop"]), (0, String::new(), String::new()));
    assert_eq!(h.native_exit(), 0);
    assert!(h.engine_log().ends_with(" engine stopped\n"));
    let mutex = format!(r"Global\{}", pipe.strip_prefix(r"\\.\pipe\").unwrap());
    assert!(EngineMutex::take(&mutex).is_ok(), "the stopped engine released its mutex");
}

#[test]
fn a_second_native_engine_says_an_engine_is_running() {
    let (mutex, pipe, data) = names("native-second");
    let first = EngineMutex::take(&mutex).expect("the first engine's mutex");
    let (code, err) = refused(&data, options(&mutex, &pipe));
    drop(first);
    assert_eq!((code, err), (1, format!("{ENGINE_RUNNING}\n")));
    assert!(!data.exists(), "the mutex comes before the data folder (W3)");
}

#[test]
fn a_native_engine_refuses_a_data_folder_owned_by_the_user() {
    let (mutex, pipe, data) = names("native-owner");
    fs::create_dir_all(&data).unwrap();
    let sid = own_token().unwrap().user_sid;
    let set = Command::new("icacls").arg(&data).args(["/setowner", &format!("*{sid}")]).output().unwrap();
    assert!(set.status.success(), "icacls /setowner: {}", String::from_utf8_lossy(&set.stdout));
    let (code, err) = refused(&data, options(&mutex, &pipe));
    let _ = fs::remove_dir_all(&data);
    assert_eq!(code, 1, "{err}");
    assert!(err.starts_with(&format!("lazymkv-engine: {} is owned by S-1-5-", data.display())), "{err}");
    assert!(err.ends_with(", neither Administrators nor SYSTEM; the engine will not use it (design part 3.5)\n"), "{err}");
    assert!(EngineMutex::take(&mutex).is_ok(), "the refusal released the mutex");
    assert_eq!(PipeConnector::new(&pipe).connect().err(), Some(ConnectError::NotRunning), "the pipe comes after the folder");
}

#[test]
fn a_native_engine_refuses_a_held_pipe_name() {
    let (mutex, pipe, data) = names("native-pipe");
    let held = PipeServer::bind(&pipe).expect("another process's pipe");
    let (code, err) = refused(&data, options(&mutex, &pipe));
    drop(held);
    let _ = fs::remove_dir_all(&data);
    assert_eq!((code, err), (1, format!("lazymkv-engine: another process holds the pipe {pipe} (design part 3.3)\n")));
    assert!(EngineMutex::take(&mutex).is_ok(), "the refusal released the mutex");
}

/// Ruling PF3-S23-24-1: with an elevated token the test holds the mutex first, as another engine would, so an engine
/// that met the mutex before the elevation check would say `ENGINE_RUNNING` instead (W3; design parts 3.2, 3.3).
#[test]
fn a_native_engine_that_is_not_elevated_says_so_before_the_mutex() {
    let (mutex, pipe, data) = names("native-lua");
    let own = own_token().unwrap();
    let held = own.elevated.then(|| EngineMutex::take(&mutex).expect("another engine's mutex"));
    let token = TokenFacts { elevated: false, ..own };
    let (code, err) = refused(&data, NativeOptions { token: Some(token), ..options(&mutex, &pipe) });
    drop(held);
    assert_eq!((code, err), (1, format!("{NOT_ELEVATED}\n")));
    assert!(!data.exists(), "no data folder before the elevation check (W3)");
}

#[test]
fn the_native_engines_log_lines_reach_its_console_writer() {
    let pipe = format!(r"\\.\pipe\{}", test_name("native-log"));
    let h = Harness::new_native(fake_tool(), &["movie-direct"], settings(), &pipe);
    h.wait("the console writer to hold every engine.log line", |h| {
        h.engine_log().contains(" engine started (lazymkv ") && h.console_text() == h.engine_log()
    });
}

#[test]
fn a_forced_shutdown_from_the_console_handler_stops_running_steps() {
    if makemkv_runs_here() {
        return;
    }
    let pipe = format!(r"\\.\pipe\{}", test_name("native-stop"));
    let h = Harness::new_native(fake_tool(), &["movie-direct", "movie-direct-hang"], settings(), &pipe);
    h.insert("tok0", "movie-direct");
    wait_for(&h, "the rip to run", JOB_CAP, |h| {
        h.job_ids().first().is_some_and(|id| h.job_dir(id).join(KEY).join("rip.txt").is_file())
    });
    let id = h.job_ids().pop().unwrap();
    h.console_stop_event();
    assert_eq!(h.native_exit(), 0);
    assert_eq!(h.state_of(&id), "Failed");
    assert_eq!(h.record(&id).message.as_deref(), Some(CANCELLED));
    assert!(h.job_dir(&id).join(KEY).join("rip.txt.exit.json").is_file(), "the killed rip ended before the engine did");
    assert!(h.engine_log().ends_with(" engine stopped\n"));
}
