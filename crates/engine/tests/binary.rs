//! The PR 2 engine binary (design part 4.2, S28): it creates nothing and exits 1 with its OS's sentence.

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
