//! The engine binary (design parts 3.1, 4.2; S28; Rulings W1, W2, M7). Outside Windows it creates nothing and exits 1
//! with S28's sentence. On Windows only its refusals before a start run here, its usage errors and an argument that is
//! not Unicode: a start would take the engine's mutex and pipe, so Task T3-23 tests the start through
//! `native::run_with` with names of its own (`engine_native.rs`).

use std::ffi::OsString;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[cfg(not(windows))]
fn run_engine(data: &std::path::Path) -> (Option<i32>, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_lazymkv-engine")).arg("--data").arg(data).output().unwrap();
    (output.status.code(), String::from_utf8(output.stderr).unwrap())
}

#[cfg(not(windows))]
fn absent_data() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("lazymkv-engine-binary-{}", std::process::id()))
}

#[cfg(not(windows))]
#[test]
fn the_binary_refuses_to_run_outside_windows_in_this_version() {
    let data = absent_data();
    assert_eq!(run_engine(&data), (Some(1), "lazymkv-engine runs only on Windows in this version\n".to_string()));
    assert!(!data.exists());
}

/// A word that is not Unicode: a byte that is not UTF-8 on Unix, an unpaired surrogate on Windows.
#[cfg(unix)]
fn not_unicode() -> OsString {
    use std::os::unix::ffi::OsStringExt;
    OsString::from_vec(b"lazymkv-engine-\xff".to_vec())
}

#[cfg(windows)]
fn not_unicode() -> OsString {
    use std::os::windows::ffi::OsStringExt;
    let mut wide: Vec<u16> = "lazymkv-engine-".encode_utf16().collect();
    wide.push(0xD800);
    OsString::from_wide(&wide)
}

/// M7, amended at review round 1 of pull request 3 (R5), on every system: `--data` with a folder whose name is not
/// Unicode is refused before anything is read, created or locked, exit 1, and neither that folder nor its lossy
/// reading, which an engine that read the word lossily would create, exists. The child is polled against a 30 s
/// deadline and killed there, since on Windows a binary that read the word lossily starts an engine with the
/// production mutex and pipe instead.
#[test]
fn an_argument_that_is_not_unicode_is_refused() {
    let data = std::env::temp_dir().join(not_unicode());
    let lossy = PathBuf::from(data.to_string_lossy().into_owned());
    let mut child = Command::new(env!("CARGO_BIN_EXE_lazymkv-engine"))
        .arg("--data")
        .arg(&data)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            if lossy.exists() {
                let _ = std::fs::remove_dir_all(&lossy);
            }
            panic!("the engine started instead of refusing the argument (M7)");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let mut err = String::new();
    child.stderr.take().unwrap().read_to_string(&mut err).unwrap();
    assert_eq!(status.code(), Some(1), "{err}");
    assert_eq!(err, format!("lazymkv-engine: an argument is not Unicode: {}\n", data.to_string_lossy()));
    assert!(!data.exists(), "the folder whose name is not Unicode was created");
    assert!(!lossy.exists(), "the folder of the lossy name was created");
}

#[cfg(windows)]
#[test]
fn an_unknown_argument_is_a_usage_error_exit_2() {
    let output = Command::new(env!("CARGO_BIN_EXE_lazymkv-engine")).arg("extra").output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(String::from_utf8(output.stderr).unwrap(),
               "usage: lazymkv-engine [--data DIR]\nlazymkv-engine: error: unrecognized arguments: extra\n");
}
