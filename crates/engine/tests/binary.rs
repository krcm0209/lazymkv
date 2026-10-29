//! The engine binary (design parts 3.1, 4.2; S28; Rulings W1, W2, M7). Outside Windows it creates nothing and exits 1
//! with S28's sentence. On Windows only its usage errors run here: a start would take the engine's mutex and pipe, so
//! Task T3-23 tests the start through `native::run_with` with names of its own (`engine_native.rs`).

use std::ffi::OsString;
use std::process::Command;

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

/// The binary reads its arguments lossily, as lazymkv-dev's and lazymkvctl's mains do, so a word that is not Unicode
/// reaches `main_with` instead of panicking with exit 101 (M7): outside Windows as a data folder, exit 1.
#[cfg(not(windows))]
#[test]
fn an_argument_that_is_not_unicode_is_read_lossily() {
    let data = std::env::temp_dir().join(not_unicode());
    let output = Command::new(env!("CARGO_BIN_EXE_lazymkv-engine")).arg("--data").arg(&data).output().unwrap();
    assert_eq!(output.status.code(), Some(1), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(!data.exists());
}

/// M7 on Windows: the word reaches `run_with` as a word left over, a usage error that starts nothing.
#[cfg(windows)]
#[test]
fn an_argument_that_is_not_unicode_is_read_lossily() {
    let word = not_unicode();
    let output = Command::new(env!("CARGO_BIN_EXE_lazymkv-engine")).arg(&word).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(String::from_utf8(output.stderr).unwrap(),
               "usage: lazymkv-engine [--data DIR]\nlazymkv-engine: error: unrecognized arguments: lazymkv-engine-\u{FFFD}\n");
    assert!(!std::env::temp_dir().join(&word).exists());
}

#[cfg(windows)]
#[test]
fn an_unknown_argument_is_a_usage_error_exit_2() {
    let output = Command::new(env!("CARGO_BIN_EXE_lazymkv-engine")).arg("extra").output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(String::from_utf8(output.stderr).unwrap(),
               "usage: lazymkv-engine [--data DIR]\nlazymkv-engine: error: unrecognized arguments: extra\n");
}
