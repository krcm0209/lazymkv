//! Test helpers the command tests share: a scratch folder, a file writer and a call of a command that collects its output.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::evidence::Ctx;
use crate::Command;

/// A fresh folder under the system's temporary directory, removed when dropped.
pub(crate) struct Scratch(pub(crate) PathBuf);

impl Scratch {
    pub(crate) fn new(tag: &str) -> Scratch {
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        let n = COUNT.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("lazymkv-test-{tag}-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Runs `run`, a command's entry point; (exit code, stdout, stderr).
pub(crate) fn call(run: Command, ctx: &Ctx, args: &[&str]) -> (i32, String, String) {
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run(ctx, &args, &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

/// Writes `text` to `path`, making its folders first.
pub(crate) fn write(path: &Path, text: impl AsRef<[u8]>) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// The lines as print() writes them.
pub(crate) fn printed(lines: &[&str]) -> String {
    lines.iter().map(|line| format!("{line}\n")).collect()
}

/// The SHA-1 of `bytes` as lowercase hex.
pub(crate) fn sha1_hex(bytes: &[u8]) -> String {
    use sha1::{Digest, Sha1};
    Sha1::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

/// A file written under a scratch folder by a path relative to it, its folders made first.
pub(crate) trait ScratchExt {
    fn write(&self, rel: &str, data: impl AsRef<[u8]>);
}

impl ScratchExt for Scratch {
    fn write(&self, rel: &str, data: impl AsRef<[u8]>) {
        write(&self.0.join(rel), data);
    }
}

/// The bytes the evidence tests hash for a subs folder: index.json, then every `sub-<n>.packets.json` in name order.
pub(crate) fn evidence_inputs(folder: &Path) -> Vec<u8> {
    let mut bytes = fs::read(folder.join("index.json")).unwrap();
    let mut names: Vec<String> = fs::read_dir(folder)
        .unwrap()
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| name.starts_with("sub-") && name.ends_with(".packets.json"))
        .collect();
    names.sort();
    for name in names {
        bytes.extend(fs::read(folder.join(name)).unwrap());
    }
    bytes
}
