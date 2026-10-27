//! The PR 2 engine binary (design part 4.2, S28): it creates nothing and exits 1 with its OS's sentence.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

fn run_engine(data: &Path) -> (Option<i32>, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_lazymkv-engine")).arg("--data").arg(data).output().unwrap();
    (output.status.code(), String::from_utf8(output.stderr).unwrap())
}

fn absent_data() -> PathBuf {
    std::env::temp_dir().join(format!("lazymkv-engine-binary-{}", std::process::id()))
}

#[cfg(not(windows))]
#[test]
fn the_binary_refuses_to_run_outside_windows_in_this_version() {
    let data = absent_data();
    assert_eq!(run_engine(&data), (Some(1), "lazymkv-engine runs only on Windows in this version\n".to_string()));
    assert!(!data.exists());
}

#[cfg(windows)]
#[test]
fn the_binary_has_no_windows_implementations_in_this_build() {
    let data = absent_data();
    assert_eq!(run_engine(&data), (Some(1), "lazymkv-engine has no Windows implementations in this build\n".to_string()));
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

/// The binary reads its arguments lossily, as lazymkv-dev's and lazymkvctl's mains do, so a data folder whose name is
/// not Unicode reaches `main_with` and exits 1 instead of panicking with exit 101 (ruling at the final review).
#[test]
fn an_argument_that_is_not_unicode_is_read_lossily() {
    let data = std::env::temp_dir().join(not_unicode());
    let output = Command::new(env!("CARGO_BIN_EXE_lazymkv-engine")).arg("--data").arg(&data).output().unwrap();
    assert_eq!(output.status.code(), Some(1), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(!data.exists());
}
