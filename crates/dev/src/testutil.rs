//! Test helpers the command tests share: a scratch folder, a file writer and a call of a command that collects its output.

use std::fs;
use std::path::Path;

use crate::evidence::Ctx;
use crate::Command;

/// The scratch folder, its file writer and the plain writer live once in lazymkv_core::testsupport, so that the
/// integration tests under crates/dev/tests share them.
pub(crate) use lazymkv_core::testsupport::{write, Scratch, ScratchExt};

/// Runs `run`, a command's entry point; (exit code, stdout, stderr).
pub(crate) fn call(run: Command, ctx: &Ctx, args: &[&str]) -> (i32, String, String) {
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run(ctx, &args, &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

/// The lines as print() writes them.
pub(crate) fn printed(lines: &[&str]) -> String {
    lines.iter().map(|line| format!("{line}\n")).collect()
}

pub(crate) use lazymkv_core::testsupport::sha1_hex;

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
