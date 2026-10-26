//! The makemkvcon drivers against the stand-in (design parts 5.3, 5.4, 5.6; Ruling S12).

use lazymkv_core::disc::TitleSource;
use lazymkv_core::ipc::OutputStream;
use lazymkv_core::job::{Failure, FailureClass, OutputCause};
use lazymkv_core::testsupport::{Scratch, ScratchExt};
use lazymkv_platform::fakes::FakeHost;
use lazymkv_engine::testkit::BusyRetry;
use lazymkv_platform::StdProcessGuard;
use lazymkv_tools::{
    list_drives, rip, scan, CancelToken, Limits, MakemkvFlags, RipOutcome, RipRun, ScanOutcome, StagingHealth, ToolCtx,
};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const EXE: &str = std::env::consts::EXE_SUFFIX;
const VERSION: &str = "MSG:1005,0,1,\"MakeMKV v2.0.0 win(x64-release) started\",\"%1 started\",\"MakeMKV v2.0.0 win(x64-release)\"\n";
const ADDED_0: &str = "MSG:3307,0,2,\"File 00800.mpls was added as title #0\",\"File %1 was added as title #%2\",\"00800.mpls\",\"0\"\n";
const ADDED_1: &str = "MSG:3307,0,2,\"File 00800.mpls was added as title #1\",\"File %1 was added as title #%2\",\"00800.mpls\",\"1\"\n";
const TITLE: &str = "TINFO:0,9,0,\"1:59:10\"\nTINFO:0,16,0,\"00800.mpls\"\n";
const COMPLETE: &str = "MSG:5011,0,0,\"Operation successfully completed\",\"Operation successfully completed\"\nTCOUNT:1\n";
const SAVING: &str = "MSG:5015,0,0,\"Saving to MKV file\",\"Saving to MKV file\"\n";
const PROGRESS: &str = "PRGV:32768,32768,65536\n";
const SAVED: &str = "MSG:5036,0,1,\"Copy complete. 1 titles saved.\",\"Copy complete. %1 titles saved.\",\"1\"\n";
const FAILED: &str = "MSG:5037,0,2,\"Copy complete. 0 titles saved, 1 failed.\",\"Copy complete. %1 titles saved, %2 failed.\",\"0\",\"1\"\n";
const FILLER: &str = "MSG:1004,0,0,\"Synthetic debug line\",\"Synthetic debug line\"\n";
const READ_ERROR: &str = "MSG:2003,0,3,\"Error 'Scsi error' occurred while reading '/BDMV/STREAM/00001.m2ts' at offset '6144'\",\"Error '%1' occurred while reading '%2' at offset '%3'\",\"Scsi error\",\"/BDMV/STREAM/00001.m2ts\",\"6144\"\n";
const LICENCE: &str = "MSG:5050,0,0,\"Synthetic licence message\",\"Synthetic licence message\"\n";
const REJECTED: &str = "MSG:1009,0,1,\"Profile parsing error: synthetic\",\"Profile parsing error: %1\",\"synthetic\"\n";
const DRIVES: &str = "DRV:0,2,999,12,\"BD-RE SYNTHETIC DRIVE 1.00 SERIAL\",\"SYNTHETIC_MOVIE\",\"tok0\"\nDRV:1,256,999,0,\"\",\"\",\"\"\nMSG:5010,0,0,\"Failed to open disc\",\"Failed to open disc\"\n";
const HEALTHY: StagingHealth = StagingHealth { share_healthy: true, out_of_space: false };

/// A stand-in makemkvcon64 in `<scratch>/tools/` with its scenario and replay files beside it (S15).
fn stand_in(scratch: &Scratch, scenario: &str, files: &[(&str, &[u8])]) -> PathBuf {
    let program = scratch.0.join("tools").join(format!("makemkvcon64{EXE}"));
    scratch.write("tools/makemkvcon64.json", scenario);
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
            limits: Limits {
                header: Duration::from_secs(120),
                scan: Duration::from_secs(60),
                exit_wait: Duration::from_secs(60),
                after_kill: Duration::from_millis(10),
            },
            cancel: CancelToken::default(),
            progress: Mutex::new(Vec::new()),
        }
    }

    fn run<R>(&self, f: impl FnOnce(&ToolCtx) -> R) -> R {
        let on_output = |_: OutputStream, _: &str| {};
        let on_progress = |fraction: f64| self.progress.lock().unwrap().push(fraction);
        let ctx = ToolCtx {
            guard: &BusyRetry(StdProcessGuard),
            host: &self.host,
            limits: &self.limits,
            cancel: self.cancel.clone(),
            on_output: &on_output,
            on_progress: &on_progress,
        };
        f(&ctx)
    }

    /// Cancels after 30 s, so that a broken kill fails the test instead of hanging it.
    fn watchdog(&self) {
        let cancel = self.cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(30));
            cancel.cancel();
        });
    }
}

fn flags(scratch: &Scratch) -> MakemkvFlags {
    MakemkvFlags { minlength_s: 120, cache_mb: 1024, profile: scratch.0.join("job.mmcp.xml"), debug: false }
}

fn scan_failure(outcome: &ScanOutcome) -> (Failure, String) {
    match outcome {
        ScanOutcome::Failed { failure, message } => (*failure, message.clone()),
        other => panic!("expected Failed, got {other:?}"),
    }
}

fn rip_failure(outcome: &RipOutcome) -> (Failure, String) {
    match outcome {
        RipOutcome::Failed { failure, message } => (*failure, message.clone()),
        other => panic!("expected Failed, got {other:?}"),
    }
}

const UNKNOWN: Failure = Failure { class: FailureClass::Unknown, output: None };

/// Runs one scan of `stdout` (exit `exit_code`) and returns the outcome and the recorded scan.txt path.
fn scan_once(scratch: &Scratch, env: &Env, stdout: &[u8], exit_code: i32) -> (ScanOutcome, PathBuf) {
    let scenario = format!(r#"{{"responses":[{{"args_contain":["info","disc:0"],"stdout":"scan.txt","exit_code":{exit_code}}}]}}"#);
    let program = stand_in(scratch, &scenario, &[("scan.txt", stdout)]);
    let out = scratch.0.join("job/scan.txt");
    fs::create_dir_all(out.parent().unwrap()).unwrap();
    let outcome = env.run(|ctx| scan(ctx, &program, &flags(scratch), 0, &out)).unwrap();
    (outcome, out)
}

/// Runs one rip of title index 0 (source 00800.mpls) under `scenario`, with `rip.txt` holding `stdout`.
fn rip_once(scratch: &Scratch, env: &Env, scenario: &str, stdout: &str, staging: StagingHealth) -> (RipRun, PathBuf, PathBuf) {
    let program = stand_in(scratch, scenario, &[("rip.txt", stdout.as_bytes()), ("placeholder.mkv", b"placeholder bytes")]);
    let dir = scratch.0.join("staging/j-1/rip/t00-00800");
    let out = scratch.0.join("job/t00-00800/rip.txt");
    fs::create_dir_all(&dir).unwrap();
    fs::create_dir_all(out.parent().unwrap()).unwrap();
    let source = TitleSource::File { name: "00800.mpls".to_string() };
    let ran = env.run(|ctx| rip(ctx, &program, &flags(scratch), 0, 0, source, &dir, &out, &|| staging)).unwrap();
    (ran, dir, out)
}

const ONE_MKV: &str = r#"{"responses":[{"args_contain":["mkv","disc:0"],"stdout":"rip.txt",
    "creates":[{"dir_arg_from_end":1,"name":"title_t00.mkv","copy_from":"placeholder.mkv"}]}]}"#;

#[test]
fn a_drive_listing_finds_the_index_and_ignores_msg_5010() {
    let scratch = Scratch::new("drive-list");
    let env = Env::new();
    let program = stand_in(&scratch, r#"{"responses":[{"args_contain":["info","disc:9999"],"stdout":"drive-list.txt"}]}"#,
        &[("drive-list.txt", DRIVES.as_bytes())]);
    let out = scratch.0.join("job/drive-list.txt");
    fs::create_dir_all(out.parent().unwrap()).unwrap();
    let listing = env.run(|ctx| list_drives(ctx, &program, false, "tok0", &out)).unwrap();
    assert_eq!(listing.index, Some(0));
    assert_eq!(listing.summary.drives.len(), 2);
    assert_eq!(listing.run.exit_code, Some(0));
    assert_eq!(fs::read(&out).unwrap(), DRIVES.as_bytes());
    let other = env.run(|ctx| list_drives(ctx, &program, false, "tok1", &out)).unwrap();
    assert_eq!(other.index, None);
}

#[test]
fn a_scan_without_5011_and_tcount_fails_as_unknown_whatever_its_exit_code() {
    let scratch = Scratch::new("scan-incomplete");
    let env = Env::new();
    let text = format!("{VERSION}{ADDED_0}{TITLE}");
    let (outcome, _) = scan_once(&scratch, &env, text.as_bytes(), 0);
    let (failure, message) = scan_failure(&outcome);
    assert_eq!(failure, UNKNOWN);
    assert_eq!(message, "the scan ended without MSG:5011 and a TCOUNT line (exit code 0)");
}

#[test]
fn a_complete_scan_succeeds_whatever_its_exit_code() {
    let scratch = Scratch::new("scan-exit-1");
    let env = Env::new();
    let text = format!("{VERSION}{ADDED_0}{TITLE}{COMPLETE}");
    let (outcome, out) = scan_once(&scratch, &env, text.as_bytes(), 1);
    match outcome {
        ScanOutcome::Done { text: read, summary } => {
            assert_eq!(read, text);
            assert!(summary.scan_complete);
        }
        other => panic!("expected Done, got {other:?}"),
    }
    assert_eq!(fs::read(&out).unwrap(), text.as_bytes());
    for suffix in [".stderr", ".timing.jsonl", ".exit.json"] {
        assert!(PathBuf::from(format!("{}{suffix}", out.display())).is_file(), "{suffix}");
    }
}

#[test]
fn a_scan_licence_message_fails_as_licence() {
    let scratch = Scratch::new("scan-licence");
    let env = Env::new();
    let text = format!("{VERSION}{LICENCE}");
    let (outcome, _) = scan_once(&scratch, &env, text.as_bytes(), 0);
    assert_eq!(
        outcome,
        ScanOutcome::Failed {
            failure: Failure { class: FailureClass::Licence, output: None },
            message: "Synthetic licence message".to_string()
        }
    );
}

#[test]
fn a_1009_on_the_scan_is_reported_as_a_rejected_profile() {
    let scratch = Scratch::new("scan-1009");
    let env = Env::new();
    let text = format!("{VERSION}{REJECTED}{ADDED_0}{TITLE}{COMPLETE}");
    let (outcome, _) = scan_once(&scratch, &env, text.as_bytes(), 0);
    assert_eq!(outcome, ScanOutcome::ProfileRejected { text: "Profile parsing error: synthetic".to_string() });
}

/// RP-5: the driver kills a scan past its limit and fails at once; the engine's retry is Task 23's.
#[test]
fn a_scan_past_its_limit_is_killed_and_fails_as_unknown() {
    let scratch = Scratch::new("scan-limit");
    let mut env = Env::new();
    // Long enough for a freshly copied stand-in to start and write its call counter while the other tests run: with
    // 300 ms it was sometimes killed before the counter was written.
    env.limits.scan = Duration::from_secs(5);
    env.watchdog();
    let text = format!("{VERSION}{ADDED_0}");
    let program = stand_in(&scratch,
        r#"{"responses":[{"args_contain":["info","disc:0"],"stdout":"scan.txt","hang_after_line":0}]}"#,
        &[("scan.txt", text.as_bytes())]);
    let out = scratch.0.join("job/scan.txt");
    fs::create_dir_all(out.parent().unwrap()).unwrap();
    let started = Instant::now();
    let outcome = env.run(|ctx| scan(ctx, &program, &flags(&scratch), 0, &out)).unwrap();
    assert!(started.elapsed() < Duration::from_secs(20));
    let (failure, message) = scan_failure(&outcome);
    assert_eq!(failure, UNKNOWN);
    assert_eq!(message, "the scan did not finish within scan_timeout_min and was killed");
    // Task 10's counter: the stand-in ran once, so the driver did not retry.
    assert_eq!(fs::read_to_string(scratch.0.join("tools/makemkvcon64.calls.json")).unwrap(), r#"{"0":1}"#);
}

#[test]
fn a_scan_line_with_invalid_utf8_is_recorded_byte_for_byte_and_read_with_replacement() {
    let scratch = Scratch::new("scan-1252");
    let env = Env::new();
    let mut bytes = VERSION.as_bytes().to_vec();
    bytes.extend_from_slice(b"CINFO:2,0,\"Caf\xe9 Disc\"\n");
    bytes.extend_from_slice(format!("{ADDED_0}{TITLE}{COMPLETE}").as_bytes());
    let (outcome, out) = scan_once(&scratch, &env, &bytes, 0);
    assert_eq!(fs::read(&out).unwrap(), bytes);
    match outcome {
        ScanOutcome::Done { text, .. } => assert!(text.contains("CINFO:2,0,\"Caf\u{fffd} Disc\"")),
        other => panic!("expected Done, got {other:?}"),
    }
}

#[test]
fn a_rip_succeeds_with_one_saved_title_and_one_mkv() {
    let scratch = Scratch::new("rip-ok");
    let env = Env::new();
    let text = format!("{VERSION}{ADDED_0}{SAVING}{PROGRESS}{SAVED}");
    let (ran, dir, out) = rip_once(&scratch, &env, ONE_MKV, &text, HEALTHY);
    assert_eq!(ran.outcome, RipOutcome::Ripped { file: dir.join("title_t00.mkv") });
    assert_eq!(ran.run.exit_code, Some(0));
    assert!(ran.summary.title_saved);
    assert_eq!(ran.summary.index_guard, Some(true));
    assert_eq!(*env.progress.lock().unwrap(), vec![0.5]);
    assert_eq!(fs::read(&out).unwrap(), text.as_bytes());
}

#[test]
fn a_rip_with_two_mkv_files_fails() {
    let scratch = Scratch::new("rip-two");
    let env = Env::new();
    let scenario = r#"{"responses":[{"args_contain":["mkv","disc:0"],"stdout":"rip.txt","creates":[
        {"dir_arg_from_end":1,"name":"title_t00.mkv","copy_from":"placeholder.mkv"},
        {"dir_arg_from_end":1,"name":"title_t01.mkv","copy_from":"placeholder.mkv"}]}]}"#;
    let (ran, _, _) = rip_once(&scratch, &env, scenario, &format!("{VERSION}{ADDED_0}{SAVING}{SAVED}"), HEALTHY);
    let (failure, message) = rip_failure(&ran.outcome);
    assert_eq!(failure, UNKNOWN);
    assert_eq!(message, "the title folder holds 2 .mkv files, not one");
}

#[test]
fn an_index_mismatch_kills_the_rip() {
    let scratch = Scratch::new("rip-drift");
    let mut env = Env::new();
    env.limits.after_kill = Duration::from_millis(300);
    env.watchdog();
    let scenario = r#"{"responses":[{"args_contain":["mkv","disc:0"],"stdout":"rip.txt","hang_after_line":3}]}"#;
    let started = Instant::now();
    let (ran, _, _) = rip_once(&scratch, &env, scenario, &format!("{VERSION}{ADDED_1}{SAVING}{PROGRESS}{SAVED}"), HEALTHY);
    assert!(started.elapsed() >= Duration::from_millis(300), "the driver returned before limits.after_kill (S56)");
    assert!(started.elapsed() < Duration::from_secs(20));
    assert_eq!(ran.outcome, RipOutcome::IndexMismatch);
    assert_eq!(ran.summary.index_guard, Some(false));
    assert_ne!(ran.run.exit_code, Some(0));
}

#[test]
fn a_licence_message_during_a_rip_is_rip_outcome_licence() {
    let scratch = Scratch::new("rip-licence");
    let env = Env::new();
    let (ran, _, _) = rip_once(&scratch, &env, ONE_MKV, &format!("{VERSION}{ADDED_0}{SAVING}{LICENCE}{FAILED}"), HEALTHY);
    assert_eq!(ran.outcome, RipOutcome::Licence { message: "Synthetic licence message".to_string() });
}

#[test]
fn a_1009_first_seen_on_a_rip_fails_the_title_as_unknown() {
    let scratch = Scratch::new("rip-1009");
    let env = Env::new();
    let (ran, _, _) = rip_once(&scratch, &env, ONE_MKV, &format!("{VERSION}{REJECTED}{ADDED_0}{SAVING}{SAVED}"), HEALTHY);
    let (failure, message) = rip_failure(&ran.outcome);
    assert_eq!(failure, UNKNOWN);
    assert_eq!(message, "MakeMKV rejected the profile on the rip: Profile parsing error: synthetic");
}

#[test]
fn a_rip_with_2003_and_a_healthy_share_fails_as_media_read_error() {
    let scratch = Scratch::new("rip-2003");
    let env = Env::new();
    let (ran, _, _) = rip_once(&scratch, &env, ONE_MKV, &format!("{VERSION}{ADDED_0}{SAVING}{READ_ERROR}{FAILED}"), HEALTHY);
    let (failure, message) = rip_failure(&ran.outcome);
    assert_eq!(failure, Failure { class: FailureClass::MediaReadError, output: None });
    assert_eq!(message, "MakeMKV reported the title failed (exit code 0)");
}

#[test]
fn a_rip_with_the_share_unreachable_fails_as_output_share_unreachable() {
    let scratch = Scratch::new("rip-share");
    let env = Env::new();
    let lost = StagingHealth { share_healthy: false, out_of_space: false };
    let (ran, _, _) = rip_once(&scratch, &env, ONE_MKV, &format!("{VERSION}{ADDED_0}{SAVING}{FAILED}"), lost);
    let (failure, _) = rip_failure(&ran.outcome);
    assert_eq!(failure, Failure { class: FailureClass::Output, output: Some(OutputCause::ShareUnreachable) });
}

#[test]
fn a_rip_that_does_not_exit_within_the_exit_wait_is_killed_and_fails_saying_so() {
    let scratch = Scratch::new("rip-exit-wait");
    let mut env = Env::new();
    env.limits.exit_wait = Duration::from_millis(300);
    env.watchdog();
    let scenario = r#"{"responses":[{"args_contain":["mkv","disc:0"],"stdout":"rip.txt","hang_after_line":4}]}"#;
    let started = Instant::now();
    let (ran, _, _) = rip_once(&scratch, &env, scenario, &format!("{VERSION}{ADDED_0}{SAVING}{SAVED}{FILLER}"), HEALTHY);
    assert!(started.elapsed() < Duration::from_secs(20));
    let (failure, message) = rip_failure(&ran.outcome);
    assert_eq!(failure, UNKNOWN);
    assert_eq!(message, "makemkvcon did not exit within exit_wait_min of its final message and was killed");
    assert!(ran.run.timed_out);
    assert!(!ran.run.cancelled);
}

#[test]
fn another_makemkvcon_running_fails_the_step_naming_it() {
    let scratch = Scratch::new("busy");
    let env = Env::new();
    env.host.set_running(vec!["makemkvcon64.exe".to_string()]);
    let (outcome, out) = scan_once(&scratch, &env, format!("{VERSION}{COMPLETE}").as_bytes(), 0);
    assert_eq!(scan_failure(&outcome), (UNKNOWN, "another MakeMKV process is running: makemkvcon64.exe".to_string()));
    assert!(!out.exists());
    let listed = env.run(|ctx| list_drives(ctx, &scratch.0.join(format!("tools/makemkvcon64{EXE}")), false, "tok0",
        &scratch.0.join("job/drive-list.txt")));
    let error = listed.unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::ResourceBusy);
    assert_eq!(error.to_string(), "another MakeMKV process is running: makemkvcon64.exe");
    env.host.set_running(vec!["MakeMKV.exe".to_string()]);
    let (ran, _, _) = rip_once(&scratch, &env, ONE_MKV, &format!("{VERSION}{ADDED_0}{SAVING}{SAVED}"), HEALTHY);
    assert_eq!(rip_failure(&ran.outcome), (UNKNOWN, "another MakeMKV process is running: MakeMKV.exe".to_string()));
}
