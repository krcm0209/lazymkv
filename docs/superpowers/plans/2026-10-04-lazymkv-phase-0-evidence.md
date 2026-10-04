# lazymkv Phase 0 (Evidence) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Produce the facts, recordings and small tools that the rest of lazymkv is built on: a verified disc fingerprint, a capture tool, stand-in executables, a corpus of real MakeMKV output, and the answers to the twelve experiments in the spec.

**Architecture:** A Rust workspace is started with three crates: `lazymkv-core` (pure fingerprint and job-profile functions), `lazymkv-capture` (a command-line tool that snapshots a disc, records a command's output with timing, writes a job profile and ejects a drive) and `fake-tools` (one stand-in executable that replays recordings). PowerShell scripts drive MakeMKV, MKVToolNix and FFmpeg on the Windows mini PC; Python scripts on the Mac analyse what comes back. Every experiment ends in a written report, and the last task folds the results into the spec.

**Tech Stack:** Rust 1.99.0 (edition 2021; crates `md-5`, `sha1`, `serde`, `serde_json`), PowerShell 5.1 on the mini PC, Python 3 (standard library only) and bash on the Mac, MakeMKV 2.0.0, MKVToolNix, FFmpeg.

**Spec:** `docs/superpowers/specs/2026-10-04-lazymkv-design.md`. Read sections 3, 5, 6, 8.6, 8.7, 9, 15, 16, 17 and 19 before starting.

## Global Constraints

- The code lives in this repository on the Mac. It is built and run only on the mini PC (`USER@makemkv-box`, Windows 11, Rust 1.99.0 MSVC) by syncing with `scripts/box.sh`. The Mac has no Rust toolchain.
- Rust edition 2021. No dependency beyond `md-5 0.10`, `sha1 0.10`, `serde 1`, `serde_json 1`. No argument-parsing or Windows API crates.
- Paths on the mini PC: sources `C:\lazymkv\src`, Cargo target `C:\lazymkv\target`, working files `C:\lazymkv\phase0`.
- MakeMKV is `C:\Program Files (x86)\MakeMKV\makemkvcon64.exe`, always run with `-r`. Scan and rip use identical `--minlength=120 --cache=1024 --profile=<job profile>`.
- The all-tracks selection string is exactly `+sel:all,-sel:mvcvideo,=100:all`.
- Minimum versions: MakeMKV 2.0.0, MKVToolNix 54.0, FFmpeg 6.1.
- Nothing is installed on the mini PC or the Mac without the owner's explicit yes in chat, naming the package.
- The agent never types a password or token. Share credentials, the TMDb token and the firewall rule in Task 9 are set by the owner.
- Long jobs (rips, disc copies, remuxes) are started with `scripts/phase0/run-interactive.ps1` so they run in the owner's logged-on desktop session, where the saved share credentials work. They are polled, never waited on in one SSH call. The owner must be logged in at the mini PC's console.
- A desktop-session job is started with `scripts/phase0/job.sh <script.ps1> <log name> [arguments]` and awaited with `scripts/phase0/wait.sh <log name>` (both written in Task 7). `wait.sh` gives up after about nine minutes with exit code 3; run it in the background and run it again until it prints the log ending in `PHASE0-DONE`. Script arguments must not contain quotes.
- Text printed by MakeMKV is never used to build a path. Output files are found by listing a directory that was empty before the call.
- Secrets never enter the repository: no TMDb token, no share password, no `Unit_Key_RO.inf` contents (only its SHA-1).
- The agent must not fetch pages from dvdcompare.net (its robots.txt blocks AI agents).
- Nothing is written to the owner's media library roots. Experiment 9 uses a separate test library folder.
- Every experiment report states what was run, what was observed, and "yes" or "no" to the experiment's question, using `docs/phase0/experiments/TEMPLATE.md`. A step that could not be run is reported as not run, with the reason.
- Commit after every task. End commit messages with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.

## Owner inputs needed during execution

| When | What the owner does |
|---|---|
| Task 7 | Approves the installs; gives the staging path on the NAS and connects to it once from the mini PC with "remember my credentials"; sets `TMDB_TOKEN` (a TMDb API read access token) as a user environment variable on the mini PC |
| Tasks 9 to 15 | Inserts discs when asked. Disc A: a UHD film never scanned on this machine, ideally with Java menus, Atmos or DTS:X and a commentary track |
| Task 9 | Adds, then removes, a Windows Firewall outbound block rule for `makemkvcon64.exe` |
| Task 11 | Disconnects the NAS from the network for one minute during a test rip |
| Task 14 | Swaps to the second drive |
| Task 16 | Adds a test library folder to Plex and Jellyfin and reports what each shows |

## File Structure

| Path | Responsibility |
|---|---|
| `Cargo.toml`, `rust-toolchain.toml` | Workspace and toolchain pin |
| `crates/core/src/fingerprint.rs` | Content hash, disc format, AACS disc ID |
| `crates/core/src/profile.rs` | Build a job profile from MakeMKV's default profile |
| `crates/core/tests/thediscdb_corpus.rs` | Fingerprint checked against every pinned public log |
| `crates/capture/src/main.rs` | Argument handling for the four subcommands |
| `crates/capture/src/disc.rs` | Disc snapshot: file listing, fingerprint, navigation-file copies |
| `crates/capture/src/record.rs` | Run a command, tee its output, record line timing |
| `crates/capture/src/eject.rs` | Eject through `DeviceIoControl` |
| `crates/fake-tools/src/main.rs`, `scenario.rs` | Stand-in executable replaying a recording |
| `scripts/box.sh` | Sync the repository to the mini PC and run a command there |
| `scripts/phase0/*.ps1` | Procedures run on the mini PC |
| `scripts/phase0/*.py` | Analysis run on the Mac |
| `corpus/thediscdb/` | Pinned public scan logs (MIT) |
| `corpus/owner/<slug>/` | Recordings of the owner's discs |
| `docs/phase0/environment.md` | Installed versions and machine facts |
| `docs/phase0/experiments/EXP-NN-*.md` | One report per experiment |
| `docs/phase0/REPORT.md` | Summary and the spec changes made |

---

### Task 1: Workspace, sync script and disc fingerprint

**Files:**
- Create: `Cargo.toml`, `rust-toolchain.toml`, `scripts/box.sh`
- Create: `crates/core/Cargo.toml`, `crates/core/src/lib.rs`, `crates/core/src/fingerprint.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `lazymkv_core::fingerprint::{DiscFile { path: String, size: u64 }, DiscFormat { BluRay, Dvd }, disc_format(&[DiscFile]) -> Option<DiscFormat>, content_hash(&[DiscFile]) -> Option<String>, aacs_disc_id(&[u8]) -> String}`. `scripts/box.sh [command...]` syncs the working tree to `C:\lazymkv\src` and runs the command there with `CARGO_TARGET_DIR=C:\lazymkv\target`.

- [ ] **Step 1: Write the workspace files**

`Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = ["crates/core"]

[workspace.package]
edition = "2021"
version = "0.0.0"
publish = false

[workspace.dependencies]
md-5 = "0.10"
sha1 = "0.10"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

`rust-toolchain.toml`:

```toml
[toolchain]
channel = "1.99.0"
```

`crates/core/Cargo.toml`:

```toml
[package]
name = "lazymkv-core"
edition.workspace = true
version.workspace = true
publish.workspace = true

[dependencies]
md-5.workspace = true
sha1.workspace = true

[dev-dependencies]
serde_json.workspace = true
```

`crates/core/src/lib.rs`:

```rust
pub mod fingerprint;
```

- [ ] **Step 2: Write the sync script**

`scripts/box.sh`:

```bash
#!/usr/bin/env bash
# Mirror the working tree to the mini PC and optionally run a command there.
# Usage: scripts/box.sh                 (sync only)
#        scripts/box.sh cargo test -p lazymkv-core
set -euo pipefail
HOST="${LAZYMKV_BOX:-USER@makemkv-box}"
cd "$(git rev-parse --show-toplevel)"
ARCHIVE="$(mktemp -t lazymkv-src)"
trap 'rm -f "$ARCHIVE"' EXIT
git ls-files -co --exclude-standard -z | COPYFILE_DISABLE=1 tar --null -T - -czf "$ARCHIVE"
ssh -o BatchMode=yes "$HOST" '(if exist C:\lazymkv\src rmdir /s /q C:\lazymkv\src) & mkdir C:\lazymkv\src'
scp -q -o BatchMode=yes "$ARCHIVE" "$HOST:C:/lazymkv/src.tgz"
ssh -o BatchMode=yes "$HOST" 'tar -xzf C:\lazymkv\src.tgz -C C:\lazymkv\src & del C:\lazymkv\src.tgz'
if [ "$#" -gt 0 ]; then
  ssh -o BatchMode=yes "$HOST" "cd /d C:\\lazymkv\\src && set CARGO_TARGET_DIR=C:\\lazymkv\\target&& set PATH=%USERPROFILE%\\.cargo\\bin;%PATH%&& $*"
fi
```

The remote shell is `cmd.exe`. Arguments are joined with spaces, so quote any argument that contains a space twice (once for bash, once for `cmd`).

Run: `chmod +x scripts/box.sh`

- [ ] **Step 3: Write the failing tests**

`crates/core/src/fingerprint.rs` (tests only for now, so the build fails on the missing items):

```rust
//! Disc fingerprints computed from a file listing and file bytes handed in.

#[cfg(test)]
mod tests {
    use super::*;

    fn bd(name: &str, size: u64) -> DiscFile {
        DiscFile { path: format!("BDMV/STREAM/{name}"), size }
    }

    // File sizes from TheDiscDB's public log for John Wick (2014), 4K disc.
    // Its recorded ContentHash is the expected value.
    fn john_wick() -> Vec<DiscFile> {
        vec![
            bd("00009.m2ts", 2_979_840),
            bd("00007.m2ts", 1_420_118_016),
            bd("00006.m2ts", 1_522_556_928),
            bd("00005.m2ts", 1_256_460_288),
            bd("00004.m2ts", 1_498_521_600),
            bd("00002.m2ts", 3_618_170_880),
            bd("00003.m2ts", 2_834_472_960),
            bd("00001.m2ts", 67_757_930_496),
            bd("00000.m2ts", 38_842_368),
            bd("00011.m2ts", 135_168),
            bd("00010.m2ts", 2_961_408),
            bd("00008.m2ts", 269_912_064),
        ]
    }

    #[test]
    fn content_hash_matches_thediscdb_for_a_uhd() {
        assert_eq!(
            content_hash(&john_wick()).as_deref(),
            Some("76E63EBB5B66F73B0C37BFAB59B8ABA5")
        );
    }

    #[test]
    fn content_hash_ignores_files_outside_the_stream_folder_and_listing_order() {
        let mut files = john_wick();
        files.reverse();
        files.push(DiscFile { path: "BDMV/PLAYLIST/00001.mpls".into(), size: 1234 });
        files.push(DiscFile { path: "BDMV\\STREAM\\SSIF\\00001.ssif".into(), size: 99 });
        assert_eq!(
            content_hash(&files).as_deref(),
            Some("76E63EBB5B66F73B0C37BFAB59B8ABA5")
        );
    }

    #[test]
    fn format_is_detected_from_the_folder_layout() {
        assert_eq!(disc_format(&john_wick()), Some(DiscFormat::BluRay));
        let dvd = vec![
            DiscFile { path: "VIDEO_TS/VIDEO_TS.IFO".into(), size: 10 },
            DiscFile { path: "VIDEO_TS/VTS_01_1.VOB".into(), size: 20 },
        ];
        assert_eq!(disc_format(&dvd), Some(DiscFormat::Dvd));
        assert_eq!(disc_format(&[]), None);
        assert_eq!(content_hash(&[]), None);
    }

    #[test]
    fn dvd_hash_covers_every_file_in_video_ts() {
        let a = vec![
            DiscFile { path: "VIDEO_TS/VIDEO_TS.IFO".into(), size: 10 },
            DiscFile { path: "VIDEO_TS/VTS_01_1.VOB".into(), size: 20 },
        ];
        let mut b = a.clone();
        b.push(DiscFile { path: "VIDEO_TS/VIDEO_TS.BUP".into(), size: 10 });
        assert_ne!(content_hash(&a), content_hash(&b));
    }

    #[test]
    fn aacs_disc_id_is_the_uppercase_sha1_of_the_file() {
        assert_eq!(aacs_disc_id(b"abc"), "A9993E364706816ABA3E25717850C26C9CD0D89D");
    }
}
```

- [ ] **Step 4: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-core`
Expected: compilation fails with "cannot find type `DiscFile` in this scope".

- [ ] **Step 5: Write the implementation**

Insert above the `tests` module in `crates/core/src/fingerprint.rs`:

```rust
use md5::{Digest, Md5};
use sha1::Sha1;

/// One file on a disc: its path relative to the disc root, and its size in bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscFile {
    pub path: String,
    pub size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscFormat {
    BluRay,
    Dvd,
}

fn upper_parts(path: &str) -> Vec<String> {
    path.replace('\\', "/")
        .split('/')
        .filter(|part| !part.is_empty())
        .map(|part| part.to_ascii_uppercase())
        .collect()
}

/// Blu-ray and UHD discs have files in `BDMV/STREAM`; DVDs have `VIDEO_TS`.
pub fn disc_format(files: &[DiscFile]) -> Option<DiscFormat> {
    let mut dvd = false;
    for file in files {
        let parts = upper_parts(&file.path);
        if parts.len() == 3 && parts[0] == "BDMV" && parts[1] == "STREAM" {
            return Some(DiscFormat::BluRay);
        }
        if parts.len() == 2 && parts[0] == "VIDEO_TS" {
            dvd = true;
        }
    }
    dvd.then_some(DiscFormat::Dvd)
}

/// TheDiscDB's content hash: the MD5 of the file sizes, each as a little-endian
/// 64-bit integer, of `BDMV/STREAM/*.m2ts` (Blu-ray, UHD) or of every file in
/// `VIDEO_TS` (DVD), sorted by file name. Returned as uppercase hex.
pub fn content_hash(files: &[DiscFile]) -> Option<String> {
    let format = disc_format(files)?;
    let mut hashed: Vec<(String, u64)> = files
        .iter()
        .filter_map(|file| {
            let parts = upper_parts(&file.path);
            match format {
                DiscFormat::BluRay
                    if parts.len() == 3
                        && parts[0] == "BDMV"
                        && parts[1] == "STREAM"
                        && parts[2].ends_with(".M2TS") =>
                {
                    Some((parts[2].clone(), file.size))
                }
                DiscFormat::Dvd if parts.len() == 2 && parts[0] == "VIDEO_TS" => {
                    Some((parts[1].clone(), file.size))
                }
                _ => None,
            }
        })
        .collect();
    if hashed.is_empty() {
        return None;
    }
    hashed.sort();
    let mut md5 = Md5::new();
    for (_, size) in &hashed {
        md5.update((*size as i64).to_le_bytes());
    }
    Some(hex_upper(md5.finalize().as_slice()))
}

/// The AACS disc ID: the SHA-1 of the bytes of `AACS/Unit_Key_RO.inf`, as uppercase hex.
pub fn aacs_disc_id(unit_key_ro: &[u8]) -> String {
    hex_upper(Sha1::digest(unit_key_ro).as_slice())
}

fn hex_upper(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02X}")).collect()
}
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-core`
Expected: `test result: ok. 5 passed; 0 failed`.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml rust-toolchain.toml scripts/box.sh crates/core
git commit -m "Add workspace, mini PC sync script and disc fingerprint"
```

---

### Task 2: Job profile generator

**Files:**
- Create: `crates/core/src/profile.rs`
- Modify: `crates/core/src/lib.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `lazymkv_core::profile::{ALL_TRACKS_SELECTION: &str, ProfileError { NoNameElement, NoDefaultSelection }, make_job_profile(default_profile_xml: &str, name: &str, selection: &str) -> Result<String, ProfileError>}`.

Background: MakeMKV ships its default conversion profile as `default.mmcp.xml` inside `appdata.tar` in its install folder. The profile's name is a `<name ...>...</name>` element, and each track class carries `defaultSelection="$app_DefaultSelectionString"`. A job profile is that file with a new name and every `defaultSelection` value replaced by a literal selection string (spec 9.3).

- [ ] **Step 1: Write the failing tests**

`crates/core/src/profile.rs`:

```rust
//! Builds a per-job MakeMKV conversion profile from MakeMKV's default profile.

#[cfg(test)]
mod tests {
    use super::*;

    // Same shape as MakeMKV's default profile; the text is ours.
    const DEFAULT: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<profile>
    <name lang="mogz">:5086</name>
    <mkvSettings insertFirstChapter00IfMissing="true" />
    <outputSettings name="copy" outputFormat="directCopy"></outputSettings>
    <trackSettings input="default">
        <output outputSettingsName="copy"
                defaultSelection="$app_DefaultSelectionString">
        </output>
    </trackSettings>
    <trackSettings input="LPCM-stereo">
        <output outputSettingsName="lpcm" defaultSelection="$app_DefaultSelectionString"></output>
    </trackSettings>
</profile>
"#;

    #[test]
    fn replaces_the_name_and_every_selection() {
        let out = make_job_profile(DEFAULT, "lazymkv all tracks", ALL_TRACKS_SELECTION).unwrap();
        assert!(out.contains(r#"<name lang="eng">lazymkv all tracks</name>"#));
        assert!(!out.contains(":5086"));
        assert!(!out.contains("$app_DefaultSelectionString"));
        assert_eq!(
            out.matches(r#"defaultSelection="+sel:all,-sel:mvcvideo,=100:all""#).count(),
            2
        );
        assert!(out.contains(r#"<mkvSettings insertFirstChapter00IfMissing="true" />"#));
        assert!(out.contains(r#"<outputSettings name="copy" outputFormat="directCopy">"#));
    }

    #[test]
    fn escapes_xml_special_characters() {
        let out = make_job_profile(DEFAULT, "a & b", "+sel:(eng|\"x\")&<y>").unwrap();
        assert!(out.contains("<name lang=\"eng\">a &amp; b</name>"));
        assert!(out.contains("defaultSelection=\"+sel:(eng|&quot;x&quot;)&amp;&lt;y&gt;\""));
    }

    #[test]
    fn reports_a_profile_it_cannot_edit() {
        assert_eq!(
            make_job_profile("<profile></profile>", "n", "s"),
            Err(ProfileError::NoNameElement)
        );
        assert_eq!(
            make_job_profile("<profile><name>x</name></profile>", "n", "s"),
            Err(ProfileError::NoDefaultSelection)
        );
    }
}
```

Add to `crates/core/src/lib.rs`:

```rust
pub mod profile;
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-core profile`
Expected: compilation fails with "cannot find function `make_job_profile` in this scope".

- [ ] **Step 3: Write the implementation**

Insert above the `tests` module in `crates/core/src/profile.rs`:

```rust
/// Selects every stream except 3D video, with equal weights so the output
/// track order follows the scan order (spec 9.3).
pub const ALL_TRACKS_SELECTION: &str = "+sel:all,-sel:mvcvideo,=100:all";

#[derive(Debug, PartialEq, Eq)]
pub enum ProfileError {
    NoNameElement,
    NoDefaultSelection,
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Returns `default_profile_xml` with its profile name set to `name` and every
/// `defaultSelection` attribute set to the literal `selection`.
pub fn make_job_profile(
    default_profile_xml: &str,
    name: &str,
    selection: &str,
) -> Result<String, ProfileError> {
    const NAME_END: &str = "</name>";
    const KEY: &str = "defaultSelection=\"";

    let name_start = default_profile_xml
        .find("<name")
        .ok_or(ProfileError::NoNameElement)?;
    let name_end = default_profile_xml[name_start..]
        .find(NAME_END)
        .ok_or(ProfileError::NoNameElement)?
        + name_start
        + NAME_END.len();

    let mut out = String::with_capacity(default_profile_xml.len() + 256);
    out.push_str(&default_profile_xml[..name_start]);
    out.push_str(&format!("<name lang=\"eng\">{}</name>", xml_escape(name)));

    let mut rest = &default_profile_xml[name_end..];
    let mut replaced = 0;
    while let Some(position) = rest.find(KEY) {
        let value_start = position + KEY.len();
        let value_len = rest[value_start..]
            .find('"')
            .ok_or(ProfileError::NoDefaultSelection)?;
        out.push_str(&rest[..value_start]);
        out.push_str(&xml_escape(selection));
        rest = &rest[value_start + value_len..];
        replaced += 1;
    }
    out.push_str(rest);

    if replaced == 0 {
        return Err(ProfileError::NoDefaultSelection);
    }
    Ok(out)
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-core`
Expected: `test result: ok. 8 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
git add crates/core
git commit -m "Add MakeMKV job profile generator"
```

---

### Task 3: Capture tool

**Files:**
- Modify: `Cargo.toml` (add the member)
- Create: `crates/capture/Cargo.toml`, `crates/capture/src/main.rs`, `crates/capture/src/disc.rs`, `crates/capture/src/record.rs`, `crates/capture/src/eject.rs`

**Interfaces:**
- Consumes: `lazymkv_core::fingerprint::{aacs_disc_id, content_hash, disc_format, DiscFile, DiscFormat}`, `lazymkv_core::profile::make_job_profile`.
- Produces: the executable `lazymkv-capture` with four subcommands, used by every later task:
  - `disc --root <path> --label <text> --out <dir>` writes `<dir>\files.json` (array of `{path, size}`), `<dir>\meta.json` (`volume_label`, `format` = `"bluray"`, `"dvd"` or null, `fingerprint`, `disc_id`, `file_count`, `total_bytes`, `captured_unix`) and copies navigation files under `<dir>\nav\`.
  - `record --out <file> -- <command> [args...]` writes the command's standard output to `<file>`, line arrival times to `<file>.timing.jsonl` (`{"n":0,"t_ms":12}` per line), standard error to `<file>.stderr`, and `{"exit_code":..,"elapsed_ms":..,"lines":..}` to `<file>.exit.json`. It echoes the output and exits with the command's exit code.
  - `profile --default <xml file> --name <text> --selection <text> --out <file>`.
  - `eject --drive <letter>` prints the result of each step and exits 0 only if the eject call succeeded.

The DVD disc ID (spec section 5) is not computed in phase 0; DVDs are looked up by fingerprint only. Record this in the final report (Task 17).

- [ ] **Step 1: Add the crate to the workspace**

In `Cargo.toml` change the members line to:

```toml
members = ["crates/core", "crates/capture"]
```

`crates/capture/Cargo.toml`:

```toml
[package]
name = "lazymkv-capture"
edition.workspace = true
version.workspace = true
publish.workspace = true

[[bin]]
name = "lazymkv-capture"
path = "src/main.rs"

[dependencies]
lazymkv-core = { path = "../core" }
serde.workspace = true
serde_json.workspace = true
```

- [ ] **Step 2: Write the failing tests**

`crates/capture/src/main.rs`:

```rust
mod disc;
mod eject;
mod record;

fn main() {}

#[cfg(test)]
pub(crate) mod testutil {
    use std::path::PathBuf;

    /// A fresh, empty directory under the system temporary folder.
    pub fn temp_dir(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("lazymkv-{tag}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flag_returns_the_value_after_its_name() {
        let args: Vec<String> = ["disc", "--root", "D:\\", "--out", "x"].iter().map(|s| s.to_string()).collect();
        assert_eq!(flag(&args, "--root"), Ok("D:\\"));
        assert_eq!(flag(&args, "--out"), Ok("x"));
        assert!(flag(&args, "--label").unwrap_err().contains("missing --label"));
    }
}
```

`crates/capture/src/eject.rs`:

```rust
//! Ejects an optical drive's tray (spec 4.3, Ejector). Exercised on real drives in Task 14.
```

`crates/capture/src/disc.rs`:

```rust
//! Snapshot of a disc's file system: listing, fingerprint, navigation-file copies.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::temp_dir;
    use lazymkv_core::fingerprint::{content_hash, DiscFile};
    use std::fs;
    use std::path::Path;

    fn write(root: &Path, rel: &str, bytes: &[u8]) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn navigation_files_are_recognised() {
        assert!(is_nav_file("BDMV/PLAYLIST/00800.mpls"));
        assert!(is_nav_file("BDMV/index.bdmv"));
        assert!(is_nav_file("BDMV/MovieObject.bdmv"));
        assert!(is_nav_file("BDMV/META/DL/bdmt_eng.xml"));
        assert!(is_nav_file("VIDEO_TS/VTS_01_0.IFO"));
        assert!(!is_nav_file("BDMV/STREAM/00001.m2ts"));
        assert!(!is_nav_file("VIDEO_TS/VTS_01_1.VOB"));
        assert!(!is_nav_file("AACS/Unit_Key_RO.inf"));
    }

    #[test]
    fn snapshot_lists_fingerprints_and_copies_navigation_files() {
        let root = temp_dir("disc");
        let out = temp_dir("out");
        write(&root, "BDMV/STREAM/00001.m2ts", &[0u8; 300]);
        write(&root, "BDMV/STREAM/00000.m2ts", &[0u8; 100]);
        write(&root, "BDMV/PLAYLIST/00001.mpls", b"mpls");
        write(&root, "BDMV/index.bdmv", b"index");
        write(&root, "AACS/Unit_Key_RO.inf", b"abc");

        let meta = snapshot(&root, "MY_DISC", &out).unwrap();

        assert_eq!(meta.volume_label, "MY_DISC");
        assert_eq!(meta.format.as_deref(), Some("bluray"));
        assert_eq!(meta.file_count, 5);
        assert_eq!(meta.total_bytes, 300 + 100 + 4 + 5 + 3);
        let expected = content_hash(&[
            DiscFile { path: "BDMV/STREAM/00000.m2ts".into(), size: 100 },
            DiscFile { path: "BDMV/STREAM/00001.m2ts".into(), size: 300 },
        ]);
        assert_eq!(meta.fingerprint, expected);
        assert_eq!(meta.disc_id.as_deref(), Some("A9993E364706816ABA3E25717850C26C9CD0D89D"));

        assert!(out.join("nav/BDMV/PLAYLIST/00001.mpls").exists());
        assert!(out.join("nav/BDMV/index.bdmv").exists());
        assert!(!out.join("nav/BDMV/STREAM/00001.m2ts").exists());
        assert!(!out.join("nav/AACS/Unit_Key_RO.inf").exists());
        assert!(out.join("meta.json").exists());

        let listed: serde_json::Value =
            serde_json::from_slice(&fs::read(out.join("files.json")).unwrap()).unwrap();
        assert_eq!(listed.as_array().unwrap().len(), 5);
        assert_eq!(listed[0]["path"], "AACS/Unit_Key_RO.inf");
        assert_eq!(listed[0]["size"], 3);
    }
}
```

`crates/capture/src/record.rs`:

```rust
//! Runs a command, tees its standard output to a file, and records when each line arrived.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::temp_dir;
    use std::cell::Cell;
    use std::io::Cursor;

    struct StepClock(Cell<u64>);

    impl Clock for StepClock {
        fn elapsed_ms(&self) -> u64 {
            let now = self.0.get();
            self.0.set(now + 10);
            now
        }
    }

    #[test]
    fn tee_copies_lines_and_records_their_timing() {
        let input = b"MSG:1005,0,1,\"a\"\nTCOUNT:2\nlast line without newline".to_vec();
        let mut reader = Cursor::new(input.clone());
        let (mut out, mut timing, mut echo) = (Vec::new(), Vec::new(), Vec::new());

        let lines = tee_lines(&mut reader, &mut out, &mut timing, &mut echo, &StepClock(Cell::new(5))).unwrap();

        assert_eq!(lines, 3);
        assert_eq!(out, input);
        assert_eq!(echo, input);
        assert_eq!(
            String::from_utf8(timing).unwrap(),
            "{\"n\":0,\"t_ms\":5}\n{\"n\":1,\"t_ms\":15}\n{\"n\":2,\"t_ms\":25}\n"
        );
    }

    #[test]
    fn run_recorded_captures_output_and_the_exit_code() {
        let out = temp_dir("rec").join("scan.txt");
        let command: Vec<String> = if cfg!(windows) {
            vec!["cmd".into(), "/c".into(), "echo hello& exit /b 3".into()]
        } else {
            vec!["sh".into(), "-c".into(), "echo hello; exit 3".into()]
        };

        let code = run_recorded(&command, &out).unwrap();

        assert_eq!(code, 3);
        assert_eq!(std::fs::read_to_string(&out).unwrap().trim(), "hello");
        let exit = std::fs::read_to_string(with_suffix(&out, ".exit.json")).unwrap();
        assert!(exit.contains("\"exit_code\":3"), "{exit}");
        assert!(exit.contains("\"lines\":1"), "{exit}");
        assert!(with_suffix(&out, ".timing.jsonl").exists());
        assert!(with_suffix(&out, ".stderr").exists());
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p lazymkv-capture`
Expected: compilation fails with "cannot find function `flag`", "cannot find function `snapshot`" and "cannot find function `tee_lines`".

- [ ] **Step 4: Implement the disc snapshot**

Insert above the `tests` module in `crates/capture/src/disc.rs`:

```rust
use lazymkv_core::fingerprint::{aacs_disc_id, content_hash, disc_format, DiscFile, DiscFormat};
use serde::Serialize;
use std::fs;
use std::io;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize)]
struct FileEntry<'a> {
    path: &'a str,
    size: u64,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Meta {
    pub volume_label: String,
    pub format: Option<String>,
    pub fingerprint: Option<String>,
    pub disc_id: Option<String>,
    pub file_count: usize,
    pub total_bytes: u64,
    pub captured_unix: u64,
}

/// Every file under `root`, with `/`-separated paths relative to it, sorted by path.
pub fn list_files(root: &Path) -> io::Result<Vec<DiscFile>> {
    let mut files = Vec::new();
    walk(root, root, &mut files)?;
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

fn walk(root: &Path, dir: &Path, files: &mut Vec<DiscFile>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        let path = entry.path();
        if metadata.is_dir() {
            walk(root, &path, files)?;
        } else {
            let relative = path
                .strip_prefix(root)
                .expect("walked paths are under the root")
                .components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            files.push(DiscFile { path: relative, size: metadata.len() });
        }
    }
    Ok(())
}

/// True for the small unencrypted navigation files kept with a recording.
pub fn is_nav_file(relative_path: &str) -> bool {
    let path = relative_path.to_ascii_uppercase();
    (path.starts_with("BDMV/PLAYLIST/") && path.ends_with(".MPLS"))
        || path == "BDMV/INDEX.BDMV"
        || path == "BDMV/MOVIEOBJECT.BDMV"
        || (path.starts_with("BDMV/META/DL/") && path.ends_with(".XML"))
        || (path.starts_with("VIDEO_TS/") && path.ends_with(".IFO"))
}

fn find_unit_key(files: &[DiscFile]) -> Option<&DiscFile> {
    ["AACS/UNIT_KEY_RO.INF", "AACS/DUPLICATE/UNIT_KEY_RO.INF"]
        .iter()
        .find_map(|wanted| files.iter().find(|file| file.path.to_ascii_uppercase() == *wanted))
}

/// Writes `files.json`, `meta.json` and `nav/` under `out` for the disc mounted at `root`.
pub fn snapshot(root: &Path, volume_label: &str, out: &Path) -> io::Result<Meta> {
    let files = list_files(root)?;
    fs::create_dir_all(out)?;

    let entries: Vec<FileEntry> = files
        .iter()
        .map(|file| FileEntry { path: &file.path, size: file.size })
        .collect();
    fs::write(out.join("files.json"), serde_json::to_vec_pretty(&entries)?)?;

    for file in files.iter().filter(|file| is_nav_file(&file.path)) {
        let destination = out.join("nav").join(&file.path);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(root.join(&file.path), destination)?;
    }

    let disc_id = find_unit_key(&files)
        .and_then(|file| fs::read(root.join(&file.path)).ok())
        .map(|bytes| aacs_disc_id(&bytes));

    let meta = Meta {
        volume_label: volume_label.to_string(),
        format: disc_format(&files).map(|format| match format {
            DiscFormat::BluRay => "bluray".to_string(),
            DiscFormat::Dvd => "dvd".to_string(),
        }),
        fingerprint: content_hash(&files),
        disc_id,
        file_count: files.len(),
        total_bytes: files.iter().map(|file| file.size).sum(),
        captured_unix: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs())
            .unwrap_or(0),
    };
    fs::write(out.join("meta.json"), serde_json::to_vec_pretty(&meta)?)?;
    Ok(meta)
}
```

- [ ] **Step 5: Implement the recorder**

Insert above the `tests` module in `crates/capture/src/record.rs`:

```rust
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

pub trait Clock {
    fn elapsed_ms(&self) -> u64;
}

pub struct RealClock(Instant);

impl RealClock {
    pub fn start() -> Self {
        RealClock(Instant::now())
    }
}

impl Clock for RealClock {
    fn elapsed_ms(&self) -> u64 {
        self.0.elapsed().as_millis() as u64
    }
}

/// Copies `reader` line by line to `out` and `echo`, and writes one timing
/// record per line to `timing`. Returns the number of lines.
pub fn tee_lines(
    reader: &mut dyn BufRead,
    out: &mut dyn Write,
    timing: &mut dyn Write,
    echo: &mut dyn Write,
    clock: &dyn Clock,
) -> io::Result<u64> {
    let mut line = Vec::new();
    let mut count = 0u64;
    loop {
        line.clear();
        if reader.read_until(b'\n', &mut line)? == 0 {
            break;
        }
        out.write_all(&line)?;
        echo.write_all(&line)?;
        echo.flush()?;
        writeln!(timing, "{{\"n\":{},\"t_ms\":{}}}", count, clock.elapsed_ms())?;
        count += 1;
    }
    out.flush()?;
    timing.flush()?;
    Ok(count)
}

pub fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

/// Runs `command`. Standard output goes to `out_path`, line timing to
/// `<out_path>.timing.jsonl`, standard error to `<out_path>.stderr`, and the
/// exit code to `<out_path>.exit.json`. Returns the exit code.
pub fn run_recorded(command: &[String], out_path: &Path) -> io::Result<i32> {
    let (program, args) = command
        .split_first()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no command given"))?;
    if let Some(parent) = out_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let stderr_file = File::create(with_suffix(out_path, ".stderr"))?;
    let clock = RealClock::start();
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::from(stderr_file))
        .spawn()?;
    let mut reader = BufReader::new(child.stdout.take().expect("stdout was piped"));
    let mut out = File::create(out_path)?;
    let mut timing = File::create(with_suffix(out_path, ".timing.jsonl"))?;
    let mut echo = io::stdout();
    let lines = tee_lines(&mut reader, &mut out, &mut timing, &mut echo, &clock)?;
    let code = child.wait()?.code().unwrap_or(-1);
    std::fs::write(
        with_suffix(out_path, ".exit.json"),
        format!(
            "{{\"exit_code\":{},\"elapsed_ms\":{},\"lines\":{}}}\n",
            code,
            clock.elapsed_ms(),
            lines
        ),
    )?;
    Ok(code)
}
```

- [ ] **Step 6: Implement eject**

Append to `crates/capture/src/eject.rs`:

```rust
#[cfg(windows)]
pub fn eject(drive_letter: char) -> Result<(), String> {
    use std::ffi::c_void;
    use std::ptr::{null, null_mut};

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateFileW(
            name: *const u16,
            access: u32,
            share: u32,
            security: *mut c_void,
            disposition: u32,
            flags: u32,
            template: *mut c_void,
        ) -> *mut c_void;
        fn DeviceIoControl(
            handle: *mut c_void,
            code: u32,
            in_buffer: *const c_void,
            in_size: u32,
            out_buffer: *mut c_void,
            out_size: u32,
            returned: *mut u32,
            overlapped: *mut c_void,
        ) -> i32;
        fn CloseHandle(handle: *mut c_void) -> i32;
        fn GetLastError() -> u32;
    }

    const GENERIC_READ: u32 = 0x8000_0000;
    const GENERIC_WRITE: u32 = 0x4000_0000;
    const FILE_SHARE_READ: u32 = 1;
    const FILE_SHARE_WRITE: u32 = 2;
    const OPEN_EXISTING: u32 = 3;
    const FSCTL_LOCK_VOLUME: u32 = 0x0009_0018;
    const FSCTL_DISMOUNT_VOLUME: u32 = 0x0009_0020;
    const IOCTL_STORAGE_MEDIA_REMOVAL: u32 = 0x002D_4804;
    const IOCTL_STORAGE_EJECT_MEDIA: u32 = 0x002D_4808;

    let device: Vec<u16> = format!(r"\\.\{drive_letter}:")
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let handle = CreateFileW(
            device.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            null_mut(),
            OPEN_EXISTING,
            0,
            null_mut(),
        );
        if handle as isize == -1 {
            return Err(format!("cannot open drive {drive_letter}: (error {})", GetLastError()));
        }

        let mut steps: Vec<String> = Vec::new();
        let mut control = |name: &str, code: u32, in_buffer: *const c_void, in_size: u32| -> bool {
            let mut returned = 0u32;
            let ok = DeviceIoControl(handle, code, in_buffer, in_size, null_mut(), 0, &mut returned, null_mut()) != 0;
            steps.push(if ok {
                format!("{name}=ok")
            } else {
                format!("{name}=error {}", GetLastError())
            });
            ok
        };

        control("lock", FSCTL_LOCK_VOLUME, null(), 0);
        control("dismount", FSCTL_DISMOUNT_VOLUME, null(), 0);
        let allow_removal: u8 = 0;
        control(
            "allow-removal",
            IOCTL_STORAGE_MEDIA_REMOVAL,
            &allow_removal as *const u8 as *const c_void,
            1,
        );
        let ejected = control("eject", IOCTL_STORAGE_EJECT_MEDIA, null(), 0);
        CloseHandle(handle);

        let report = steps.join(", ");
        if ejected {
            println!("{report}");
            Ok(())
        } else {
            Err(report)
        }
    }
}

#[cfg(not(windows))]
pub fn eject(_drive_letter: char) -> Result<(), String> {
    Err("eject is only implemented on Windows".to_string())
}
```

- [ ] **Step 7: Implement the command line**

In `crates/capture/src/main.rs` replace `fn main() {}` with:

```rust
use lazymkv_core::profile::make_job_profile;
use std::path::Path;
use std::process::ExitCode;

const USAGE: &str = "usage:
  lazymkv-capture disc --root <path> --label <text> --out <dir>
  lazymkv-capture record --out <file> -- <command> [args...]
  lazymkv-capture profile --default <xml file> --name <text> --selection <text> --out <file>
  lazymkv-capture eject --drive <letter>";

/// The value that follows `name` in `args`.
fn flag<'a>(args: &'a [String], name: &str) -> Result<&'a str, String> {
    args.iter()
        .position(|arg| arg == name)
        .and_then(|index| args.get(index + 1))
        .map(String::as_str)
        .ok_or_else(|| format!("missing {name}\n{USAGE}"))
}

fn run(args: &[String]) -> Result<i32, String> {
    let io_error = |error: std::io::Error| error.to_string();
    match args.first().map(String::as_str) {
        Some("disc") => {
            let meta = disc::snapshot(
                Path::new(flag(args, "--root")?),
                flag(args, "--label")?,
                Path::new(flag(args, "--out")?),
            )
            .map_err(io_error)?;
            println!("{}", serde_json::to_string_pretty(&meta).map_err(|error| error.to_string())?);
            Ok(0)
        }
        Some("record") => {
            let split = args
                .iter()
                .position(|arg| arg == "--")
                .ok_or_else(|| format!("missing --\n{USAGE}"))?;
            record::run_recorded(&args[split + 1..], Path::new(flag(&args[..split], "--out")?))
                .map_err(io_error)
        }
        Some("profile") => {
            let default_xml = std::fs::read_to_string(flag(args, "--default")?).map_err(io_error)?;
            let xml = make_job_profile(&default_xml, flag(args, "--name")?, flag(args, "--selection")?)
                .map_err(|error| format!("{error:?}"))?;
            std::fs::write(flag(args, "--out")?, xml).map_err(io_error)?;
            Ok(0)
        }
        Some("eject") => {
            let letter = flag(args, "--drive")?
                .chars()
                .next()
                .ok_or_else(|| "empty drive letter".to_string())?;
            eject::eject(letter)?;
            Ok(0)
        }
        _ => Err(USAGE.to_string()),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => ExitCode::from(code.clamp(0, 255) as u8),
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
    }
}
```

- [ ] **Step 8: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p lazymkv-capture`
Expected: `test result: ok. 5 passed; 0 failed`.

- [ ] **Step 9: Build the release executable and try it**

Run: `scripts/box.sh cargo build --release -p lazymkv-capture`
Expected: `Finished` and the file `C:\lazymkv\target\release\lazymkv-capture.exe`.

Run: `ssh USER@makemkv-box "C:\lazymkv\target\release\lazymkv-capture.exe record --out C:\lazymkv\phase0\smoke\drives.txt -- \"C:\Program Files (x86)\MakeMKV\makemkvcon64.exe\" -r --cache=1 info disc:9999"`
Expected: `MSG:1005` and sixteen `DRV:` lines are printed, and `C:\lazymkv\phase0\smoke\drives.txt.exit.json` exists.

- [ ] **Step 10: Commit**

```bash
git add Cargo.toml crates/capture
git commit -m "Add capture tool: disc snapshot, recorded command, job profile, eject"
```

---

### Task 4: Stand-in executable

**Files:**
- Modify: `Cargo.toml` (add the member)
- Create: `crates/fake-tools/Cargo.toml`, `crates/fake-tools/src/main.rs`, `crates/fake-tools/src/scenario.rs`, `crates/fake-tools/tests/replay.rs`

**Interfaces:**
- Consumes: the `.timing.jsonl` format written by `lazymkv-capture record`.
- Produces: the executable `fake-tool`. Copied under another name (`makemkvcon64.exe`, `mkvmerge.exe`, `ffprobe.exe`), it reads `%LAZYMKV_FAKE_DIR%\<that name without extension, lowercased>.json`:

```json
{"responses": [
  {"args_contain": ["info", "disc:0"], "stdout": "scan.txt", "timing": "scan.txt.timing.jsonl", "exit_code": 0,
   "creates": [{"dir_arg": 4, "name": "title_t00.mkv", "copy_from": "sample.mkv"}]}
]}
```

The first response whose every `args_contain` string is a substring of some argument is used. `stdout` and `timing` name files in the scenario folder; the lines are printed, delayed to their recorded times multiplied by `LAZYMKV_FAKE_SPEED` (default 0, no delay). Each `creates` entry copies `copy_from` into the directory named by argument number `dir_arg` (0-based, not counting the program). The process exits with `exit_code`; with no matching response it prints a message and exits 2.

- [ ] **Step 1: Add the crate to the workspace**

In `Cargo.toml` change the members line to:

```toml
members = ["crates/core", "crates/capture", "crates/fake-tools"]
```

`crates/fake-tools/Cargo.toml`:

```toml
[package]
name = "fake-tools"
edition.workspace = true
version.workspace = true
publish.workspace = true

[[bin]]
name = "fake-tool"
path = "src/main.rs"

[dependencies]
serde.workspace = true
serde_json.workspace = true
```

- [ ] **Step 2: Write the failing tests**

`crates/fake-tools/src/main.rs`:

```rust
mod scenario;

fn main() {}
```

`crates/fake-tools/src/scenario.rs`:

```rust
//! The scenario file a stand-in executable replays.

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|arg| arg.to_string()).collect()
    }

    #[test]
    fn the_first_response_whose_needles_all_appear_is_selected() {
        let scenario: Scenario = serde_json::from_str(
            r#"{"responses":[
                {"args_contain":["info","disc:9999"],"stdout":"drives.txt"},
                {"args_contain":["info"],"stdout":"scan.txt","exit_code":1},
                {"args_contain":["mkv"],"creates":[{"dir_arg":4,"name":"t.mkv","copy_from":"s.mkv"}]}
            ]}"#,
        )
        .unwrap();

        let drives = select(&scenario, &args(&["-r", "--cache=1", "info", "disc:9999"])).unwrap();
        assert_eq!(drives.stdout.as_deref(), Some("drives.txt"));
        assert_eq!(drives.exit_code, 0);

        let scan = select(&scenario, &args(&["-r", "info", "disc:0"])).unwrap();
        assert_eq!(scan.stdout.as_deref(), Some("scan.txt"));
        assert_eq!(scan.exit_code, 1);

        let rip = select(&scenario, &args(&["-r", "mkv", "disc:0", "0", "out"])).unwrap();
        assert_eq!(rip.creates[0].dir_arg, 4);
        assert!(rip.stdout.is_none());

        assert!(select(&scenario, &args(&["backup"])).is_none());
    }

    #[test]
    fn timing_lines_give_each_line_its_arrival_time() {
        assert_eq!(
            parse_timing("{\"n\":0,\"t_ms\":5}\n{\"n\":1,\"t_ms\":250}\nnot json\n"),
            vec![5, 250]
        );
    }
}
```

`crates/fake-tools/tests/replay.rs`:

```rust
use std::fs;
use std::process::Command;

#[test]
fn replays_output_creates_files_and_returns_the_exit_code() {
    let dir = std::env::temp_dir().join(format!("lazymkv-fake-{}", std::process::id()));
    let out_dir = dir.join("out");
    fs::create_dir_all(&dir).unwrap();
    let tool = dir.join(if cfg!(windows) { "makemkvcon64.exe" } else { "makemkvcon64" });
    fs::copy(env!("CARGO_BIN_EXE_fake-tool"), &tool).unwrap();

    let recorded = "TCOUNT:1\nMSG:5036,0,1,\"Copy complete. 1 titles saved.\"\n";
    fs::write(dir.join("rip.txt"), recorded).unwrap();
    fs::write(dir.join("sample.mkv"), b"not really an mkv").unwrap();
    fs::write(
        dir.join("makemkvcon64.json"),
        r#"{"responses":[
            {"args_contain":["mkv","disc:0"],"stdout":"rip.txt",
             "creates":[{"dir_arg":4,"name":"title_t00.mkv","copy_from":"sample.mkv"}]},
            {"args_contain":["backup"],"exit_code":7}
        ]}"#,
    )
    .unwrap();

    let run = |args: &[&str]| {
        Command::new(&tool)
            .args(args)
            .env("LAZYMKV_FAKE_DIR", &dir)
            .output()
            .unwrap()
    };

    let rip = run(&["-r", "mkv", "disc:0", "0", out_dir.to_str().unwrap()]);
    assert_eq!(rip.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&rip.stdout), recorded);
    assert_eq!(fs::read(out_dir.join("title_t00.mkv")).unwrap(), b"not really an mkv");

    assert_eq!(run(&["-r", "backup", "disc:0", "x"]).status.code(), Some(7));
    assert_eq!(run(&["-r", "info", "disc:0"]).status.code(), Some(2));
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `scripts/box.sh cargo test -p fake-tools`
Expected: compilation fails with "cannot find type `Scenario` in this scope".

- [ ] **Step 4: Implement the scenario types**

Insert above the `tests` module in `crates/fake-tools/src/scenario.rs`:

```rust
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Scenario {
    pub responses: Vec<Response>,
}

#[derive(Debug, Deserialize)]
pub struct Response {
    /// Each string must be a substring of some argument.
    pub args_contain: Vec<String>,
    /// File in the scenario folder whose content is printed.
    #[serde(default)]
    pub stdout: Option<String>,
    /// File in the scenario folder with one `{"n":..,"t_ms":..}` record per printed line.
    #[serde(default)]
    pub timing: Option<String>,
    #[serde(default)]
    pub exit_code: i32,
    #[serde(default)]
    pub creates: Vec<Create>,
}

#[derive(Debug, Deserialize)]
pub struct Create {
    /// Index of the argument that names the output directory.
    pub dir_arg: usize,
    pub name: String,
    pub copy_from: String,
}

pub fn select<'a>(scenario: &'a Scenario, args: &[String]) -> Option<&'a Response> {
    scenario.responses.iter().find(|response| {
        response
            .args_contain
            .iter()
            .all(|needle| args.iter().any(|arg| arg.contains(needle.as_str())))
    })
}

#[derive(Deserialize)]
struct TimingRecord {
    t_ms: u64,
}

/// Arrival time in milliseconds of each line, from a `.timing.jsonl` file.
pub fn parse_timing(text: &str) -> Vec<u64> {
    text.lines()
        .filter_map(|line| serde_json::from_str::<TimingRecord>(line).ok())
        .map(|record| record.t_ms)
        .collect()
}
```

- [ ] **Step 5: Implement the executable**

Replace `crates/fake-tools/src/main.rs` with:

```rust
mod scenario;

use scenario::{parse_timing, select, Scenario};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;
use std::{env, fs, thread};

fn run() -> Result<i32, String> {
    let exe = env::current_exe().map_err(|error| error.to_string())?;
    let tool = exe
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let dir = PathBuf::from(
        env::var("LAZYMKV_FAKE_DIR").map_err(|_| "LAZYMKV_FAKE_DIR is not set".to_string())?,
    );
    let speed: f64 = env::var("LAZYMKV_FAKE_SPEED")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0.0);

    let scenario_path = dir.join(format!("{tool}.json"));
    let scenario_text = fs::read_to_string(&scenario_path)
        .map_err(|error| format!("{}: {error}", scenario_path.display()))?;
    let scenario: Scenario = serde_json::from_str(&scenario_text)
        .map_err(|error| format!("{}: {error}", scenario_path.display()))?;

    let args: Vec<String> = env::args().skip(1).collect();
    let response =
        select(&scenario, &args).ok_or_else(|| format!("{tool}: no response matches {args:?}"))?;

    if let Some(stdout_file) = &response.stdout {
        let text = fs::read(dir.join(stdout_file)).map_err(|error| format!("{stdout_file}: {error}"))?;
        let timing = match &response.timing {
            Some(timing_file) => parse_timing(
                &fs::read_to_string(dir.join(timing_file))
                    .map_err(|error| format!("{timing_file}: {error}"))?,
            ),
            None => Vec::new(),
        };
        let mut out = std::io::stdout();
        let mut waited_ms = 0u64;
        for (index, line) in text.split_inclusive(|byte| *byte == b'\n').enumerate() {
            if speed > 0.0 {
                if let Some(t_ms) = timing.get(index) {
                    let due_ms = (*t_ms as f64 * speed) as u64;
                    if due_ms > waited_ms {
                        thread::sleep(Duration::from_millis(due_ms - waited_ms));
                        waited_ms = due_ms;
                    }
                }
            }
            out.write_all(line).map_err(|error| error.to_string())?;
            out.flush().map_err(|error| error.to_string())?;
        }
    }

    for create in &response.creates {
        let target_dir = args
            .get(create.dir_arg)
            .ok_or_else(|| format!("creates: there is no argument {}", create.dir_arg))?;
        fs::create_dir_all(target_dir).map_err(|error| format!("{target_dir}: {error}"))?;
        fs::copy(dir.join(&create.copy_from), Path::new(target_dir).join(&create.name))
            .map_err(|error| format!("{}: {error}", create.copy_from))?;
    }

    Ok(response.exit_code)
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code.clamp(0, 255) as u8),
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
    }
}
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `scripts/box.sh cargo test -p fake-tools`
Expected: `test result: ok. 2 passed` for the unit tests and `test result: ok. 1 passed` for `replay`.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml crates/fake-tools
git commit -m "Add stand-in executable that replays recordings"
```

---

### Task 5: Public scan-log corpus

**Files:**
- Create: `scripts/phase0/fetch-thediscdb-fixtures.sh`, `corpus/thediscdb/README.md`
- Create (downloaded): `corpus/thediscdb/LICENSE`, `corpus/thediscdb/movie/...`, `corpus/thediscdb/series/...`
- Create: `crates/core/tests/thediscdb_corpus.rs`

**Interfaces:**
- Consumes: `lazymkv_core::fingerprint::{content_hash, DiscFile}`.
- Produces: saved TheDiscDB API responses for the nine pinned discs under `corpus/thediscdb-api/` (`dischash-<hash>.json` and `discid-<id>.json`), and nine pinned disc logs under `corpus/thediscdb/`, each as `discNN.txt` (raw `makemkvcon` robot output, including `HSH:index,name,time,size` lines for every hashed file), `discNN.json` (TheDiscDB's parsed form, with `ContentHash` and per-title `Item.Type`) and `discNN-summary.txt`.

Background: TheDiscDB publishes its data at `github.com/TheDiscDb/data` under the MIT licence. The files are pinned to commit `b54d1922080fb1947912500db126cf95dcdfaa45`.

- [ ] **Step 1: Write the fetch script**

`scripts/phase0/fetch-thediscdb-fixtures.sh`:

```bash
#!/usr/bin/env bash
# Download a pinned set of TheDiscDB scan logs (MIT licence) into corpus/thediscdb.
set -euo pipefail
COMMIT=b54d1922080fb1947912500db126cf95dcdfaa45
BASE="https://raw.githubusercontent.com/TheDiscDb/data/$COMMIT"
DEST="$(git rev-parse --show-toplevel)/corpus/thediscdb"

urlencode() { python3 -c 'import sys, urllib.parse; print(urllib.parse.quote(sys.argv[1]))' "$1"; }

# fetch <release folder under data/> <disc name>...
fetch() {
  local dir="$1"; shift
  mkdir -p "$DEST/$dir"
  for disc in "$@"; do
    for suffix in .txt .json -summary.txt; do
      curl -fsS -m 60 "$BASE/data/$(urlencode "$dir")/$disc$suffix" -o "$DEST/$dir/$disc$suffix"
    done
  done
}

fetch "movie/John Wick (2014)/4k-ultra-hd--blu-ray" disc01 disc02
fetch "movie/The Expendables 3 (2014)/the-expendables-3-film-collection" disc01
fetch "movie/Smile (2022)/2022-4k" disc01
fetch "movie/Knowing (2009)/2010-dvd" disc01
fetch "series/Chernobyl (2019)/2020-4k-US" disc01 disc02
fetch "series/Firefly (2002)/2003-dvd-the-complete-series" disc01 disc02
curl -fsS -m 60 "$BASE/LICENSE" -o "$DEST/LICENSE"

# Save the live API's answer for each pinned disc, by content hash and by disc ID (spec 8.1).
API="$(git rev-parse --show-toplevel)/corpus/thediscdb-api"
mkdir -p "$API"
find "$DEST" -name 'disc*.json' | while read -r json; do
  hash="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1], encoding="utf-8-sig")).get("ContentHash") or "")' "$json")"
  id="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1], encoding="utf-8-sig")).get("GlobalDiscId") or "")' "$json")"
  [ -n "$hash" ] && curl -sS -m 60 "https://thediscdb.com/api/dischash/$hash" -o "$API/dischash-$hash.json"
  [ -n "$id" ] && curl -sS -m 60 "https://thediscdb.com/api/discid/$id" -o "$API/discid-$id.json"
  sleep 1
done
echo "saved $(ls "$API" | wc -l | tr -d ' ') API responses"
echo "fetched $(find "$DEST" -name 'disc*.txt' ! -name '*-summary.txt' | wc -l | tr -d ' ') logs"
```

- [ ] **Step 2: Run it**

Run: `chmod +x scripts/phase0/fetch-thediscdb-fixtures.sh && scripts/phase0/fetch-thediscdb-fixtures.sh`
Expected: `saved 18 API responses` (fewer if a disc has no disc ID) and `fetched 9 logs`.

- [ ] **Step 3: Write the corpus README**

`corpus/thediscdb/README.md`:

```markdown
# TheDiscDB scan logs

Real `makemkvcon` robot-mode scan logs with TheDiscDB's parsed form and
human-labelled title types, copied unchanged from
https://github.com/TheDiscDb/data at commit
b54d1922080fb1947912500db126cf95dcdfaa45. MIT licence, see `LICENSE`.
Fetched by `scripts/phase0/fetch-thediscdb-fixtures.sh`.

| Folder | Why it is here |
|---|---|
| `movie/John Wick (2014)/4k-ultra-hd--blu-ray` | UHD with Dolby Vision (disc01) and its Blu-ray (disc02) |
| `movie/The Expendables 3 (2014)/the-expendables-3-film-collection` | Two cuts plus decoy playlists |
| `movie/Smile (2022)/2022-4k` | UHD with Java menu labels (`FPL_MainFeature`) and many forced-only subtitle streams |
| `movie/Knowing (2009)/2010-dvd` | DVD with flagged commentary audio and subtitles |
| `series/Chernobyl (2019)/2020-4k-US` | UHD miniseries |
| `series/Firefly (2002)/2003-dvd-the-complete-series` | DVD television season |
```

- [ ] **Step 4: Write the failing test**

`crates/core/tests/thediscdb_corpus.rs`:

```rust
use lazymkv_core::fingerprint::{content_hash, DiscFile};
use std::fs;
use std::path::{Path, PathBuf};

fn scan_logs(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if path.is_dir() {
            scan_logs(&path, found);
        } else if name.starts_with("disc") && name.ends_with(".txt") && !name.ends_with("-summary.txt") {
            found.push(path);
        }
    }
}

/// `HSH:index,name,time,size` lines list every file TheDiscDB hashed.
fn hashed_files(log: &str) -> Vec<DiscFile> {
    log.lines()
        .filter(|line| line.starts_with("HSH:"))
        .map(|line| {
            let fields: Vec<&str> = line.split(',').collect();
            let name = fields[1];
            let size: u64 = fields[fields.len() - 1].trim().parse().unwrap();
            let folder = if name.to_ascii_lowercase().ends_with(".m2ts") {
                "BDMV/STREAM"
            } else {
                "VIDEO_TS"
            };
            DiscFile { path: format!("{folder}/{name}"), size }
        })
        .collect()
}

#[test]
fn content_hash_matches_every_pinned_thediscdb_log() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/thediscdb");
    let mut logs = Vec::new();
    scan_logs(&root, &mut logs);
    assert_eq!(logs.len(), 9, "expected the nine pinned logs under {}", root.display());

    for log_path in logs {
        let log = String::from_utf8_lossy(&fs::read(&log_path).unwrap()).into_owned();
        let parsed: serde_json::Value =
            serde_json::from_slice(&fs::read(log_path.with_extension("json")).unwrap()).unwrap();
        let expected = parsed["ContentHash"].as_str().unwrap();
        assert_eq!(
            content_hash(&hashed_files(&log)).as_deref(),
            Some(expected),
            "{}",
            log_path.display()
        );
    }
}
```

This test was verified against these nine logs while the plan was written, so it should pass as soon as the files are present. To see it fail first, run it before Step 2 has been synced (the directory is then missing and `read_dir` panics).

- [ ] **Step 5: Run the test**

Run: `scripts/box.sh cargo test -p lazymkv-core --test thediscdb_corpus`
Expected: `test result: ok. 1 passed; 0 failed`.

- [ ] **Step 6: Commit**

```bash
git add scripts/phase0/fetch-thediscdb-fixtures.sh corpus/thediscdb corpus/thediscdb-api crates/core/tests
git commit -m "Pin TheDiscDB scan logs and check the fingerprint against them"
```

---

### Task 6: Analysis scripts

**Files:**
- Create: `scripts/phase0/robot.py`, `scripts/phase0/summarize_scan.py`, `scripts/phase0/compare_tracks.py`, `scripts/phase0/compare_scans.py`, `scripts/phase0/exp02_check.py`, `scripts/phase0/subtitle_alignment.py`, `scripts/phase0/hit_rate.py`, `scripts/phase0/check_encoding.py`
- Test: `scripts/phase0/test_analysis.py`

**Interfaces:**
- Consumes: recordings as laid out in Task 8, and `corpus/thediscdb/`.
- Produces, all run on the Mac from the repository root:
  - `robot.parse(text) -> Scan` with `messages: [(code, text, params)]`, `drives: [fields]`, `disc: {attribute: value}`, `titles: {title: {attribute: value}}`, `streams: {(title, stream): {attribute: value}}`; `robot.split_fields(text)`; `robot.duration_seconds("H:MM:SS")`.
  - `python3 scripts/phase0/summarize_scan.py <recording dir>` prints the MakeMKV version, whether Java was in use, and one row per title.
  - `python3 scripts/phase0/compare_tracks.py <scan.txt> <title> <mkvmerge.json> [--selection all|eng]` prints the scan's streams beside the file's tracks and exits 0 when count and order match.
  - `python3 scripts/phase0/compare_scans.py <scan A> <scan B>` exits 0 when both list the same titles by source file, segment map and duration.
  - `python3 scripts/phase0/exp02_check.py <recording dir>` evaluates experiment 2.
  - `python3 scripts/phase0/subtitle_alignment.py [--clear-max-bytes N] [--tolerance-ms N] name=packets.json ...` prints event counts and the alignment matrix.
  - `python3 scripts/phase0/hit_rate.py corpus/owner` prints TheDiscDB hits per recorded disc.
  - `python3 scripts/phase0/check_encoding.py <scan.txt>` reports the output's encoding.

- [ ] **Step 1: Write the failing tests**

`scripts/phase0/test_analysis.py`:

```python
import pathlib
import unittest

import robot
import subtitle_alignment

CORPUS = pathlib.Path(__file__).resolve().parents[2] / "corpus" / "thediscdb"
JOHN_WICK = CORPUS / "movie" / "John Wick (2014)" / "4k-ultra-hd--blu-ray" / "disc01.txt"


class SplitFields(unittest.TestCase):
    def test_quoted_fields_keep_commas_and_unescape_backslashes(self):
        line = (
            '3344,0,1,"Using Java runtime from C:\\\\Program Files\\\\Java\\\\bin\\\\java.exe",'
            '"Using Java runtime from %1","C:\\\\Program Files\\\\Java\\\\bin\\\\java.exe"'
        )
        fields = robot.split_fields(line)
        self.assertEqual(fields[0:3], ["3344", "0", "1"])
        self.assertEqual(fields[5], "C:\\Program Files\\Java\\bin\\java.exe")

    def test_both_quote_escapes_are_accepted(self):
        self.assertEqual(robot.split_fields('1,"a \\"b\\", c","d ""e"""'), ["1", 'a "b", c', 'd "e"'])


class Parse(unittest.TestCase):
    def setUp(self):
        self.scan = robot.parse(JOHN_WICK.read_text(encoding="utf-8", errors="replace"))

    def test_titles_and_streams(self):
        self.assertEqual(self.scan.titles[0][16], "00001.mpls")
        self.assertEqual(self.scan.titles[0][9], "1:41:13")
        self.assertEqual(self.scan.titles[0][8], "16")
        self.assertEqual(self.scan.streams[(0, 1)][1], "Audio")
        self.assertEqual(self.scan.streams[(0, 1)][6], "TrueHD")
        self.assertEqual(self.scan.streams[(0, 1)][3], "eng")

    def test_messages_carry_their_parameters(self):
        added = [params for code, _, params in self.scan.messages if code == 3307]
        self.assertEqual(added[0], ["00001.mpls", "0"])
        version = [params for code, _, params in self.scan.messages if code == 1005]
        self.assertTrue(version[0][0].startswith("MakeMKV v"))

    def test_drives(self):
        self.assertEqual(len(self.scan.drives), 16)
        self.assertEqual(self.scan.drives[0][1], "2")

    def test_duration_seconds(self):
        self.assertEqual(robot.duration_seconds("1:41:13"), 6073)
        self.assertEqual(robot.duration_seconds("0:06:00"), 360)


class Alignment(unittest.TestCase):
    def test_events_drop_small_clear_packets(self):
        packets = [
            {"pts_time": "1.000", "size": "5000"},
            {"pts_time": "3.000", "size": "30"},
            {"pts_time": "9.500", "size": "4000"},
            {"size": "4000"},
        ]
        self.assertEqual(subtitle_alignment.events(packets, 64), [1.0, 9.5])

    def test_aligned_is_the_share_of_events_with_a_near_start(self):
        a = [1.0, 2.0, 3.0]
        b = [1.1, 2.5, 3.0]
        self.assertAlmostEqual(subtitle_alignment.aligned(a, b, 0.3), 2 / 3)
        self.assertEqual(subtitle_alignment.aligned([], b, 0.3), 0.0)
        self.assertEqual(subtitle_alignment.aligned(a, [], 0.3), 0.0)


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `python3 -m unittest discover -s scripts/phase0 -p 'test_*.py'`
Expected: `ModuleNotFoundError: No module named 'robot'`.

- [ ] **Step 3: Write the parser**

`scripts/phase0/robot.py`:

```python
"""Parser for makemkvcon robot-mode output (spec 9.4)."""
from dataclasses import dataclass, field


def split_fields(text):
    """Split the text after 'KIND:' into fields.

    Quoted fields may contain commas, backslash escapes and doubled quotes.
    """
    fields, current, quoted = [], [], False
    i, n = 0, len(text)
    while i < n:
        c = text[i]
        if quoted:
            if c == "\\" and i + 1 < n:
                current.append(text[i + 1])
                i += 2
                continue
            if c == '"':
                if i + 1 < n and text[i + 1] == '"':
                    current.append('"')
                    i += 2
                    continue
                quoted = False
            else:
                current.append(c)
        elif c == '"':
            quoted = True
        elif c == ",":
            fields.append("".join(current))
            current = []
        else:
            current.append(c)
        i += 1
    fields.append("".join(current))
    return fields


@dataclass
class Scan:
    messages: list = field(default_factory=list)  # (code, text, params)
    drives: list = field(default_factory=list)    # one field list per DRV line
    disc: dict = field(default_factory=dict)      # attribute -> value
    titles: dict = field(default_factory=dict)    # title -> {attribute: value}
    streams: dict = field(default_factory=dict)   # (title, stream) -> {attribute: value}


def parse(text):
    scan = Scan()
    for raw in text.splitlines():
        kind, separator, rest = raw.lstrip("\ufeff").partition(":")
        if not separator:
            continue
        f = split_fields(rest)
        try:
            if kind == "MSG":
                scan.messages.append((int(f[0]), f[3], f[5:]))
            elif kind == "DRV":
                scan.drives.append(f)
            elif kind == "CINFO":
                scan.disc[int(f[0])] = f[2]
            elif kind == "TINFO":
                scan.titles.setdefault(int(f[0]), {})[int(f[1])] = f[3]
            elif kind == "SINFO":
                scan.streams.setdefault((int(f[0]), int(f[1])), {})[int(f[2])] = f[4]
        except (ValueError, IndexError):
            continue
    return scan


def duration_seconds(text):
    hours, minutes, seconds = (int(part) for part in text.split(":"))
    return hours * 3600 + minutes * 60 + seconds
```

- [ ] **Step 4: Write the subtitle alignment script**

`scripts/phase0/subtitle_alignment.py`:

```python
"""Event counts and start-time alignment between subtitle streams (spec 8.7).

Each input is the JSON printed by
  ffprobe -v error -select_streams s:N -show_entries packet=pts_time,duration_time,size -of json FILE
"""
import argparse
import bisect
import json
import pathlib


def events(packets, clear_max_bytes):
    """Start times, in seconds, of packets large enough to show something."""
    return sorted(
        float(p["pts_time"])
        for p in packets
        if "pts_time" in p and int(p.get("size", 0)) > clear_max_bytes
    )


def aligned(a, b, tolerance_s):
    """Share of the events in a that start within tolerance_s of some event in b."""
    if not a:
        return 0.0
    hits = 0
    for start in a:
        i = bisect.bisect_left(b, start - tolerance_s)
        if i < len(b) and b[i] <= start + tolerance_s:
            hits += 1
    return hits / len(a)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("inputs", nargs="+", help="name=path to a packets JSON file")
    ap.add_argument("--clear-max-bytes", type=int, default=64)
    ap.add_argument("--tolerance-ms", type=int, default=300)
    args = ap.parse_args()

    tracks = {}
    for item in args.inputs:
        name, _, path = item.partition("=")
        packets = json.loads(pathlib.Path(path).read_text(encoding="utf-8-sig")).get("packets", [])
        sizes = [int(p.get("size", 0)) for p in packets]
        buckets = [sum(1 for s in sizes if s <= limit) for limit in (32, 64, 128, 256)]
        print(f"{name}: packets={len(packets)} sized<=32/64/128/256: {buckets}")
        tracks[name] = events(packets, args.clear_max_bytes)

    print()
    for name, starts in tracks.items():
        print(f"{name}: events={len(starts)}")
    print()
    names = list(tracks)
    tolerance = args.tolerance_ms / 1000
    print("aligned(row, column):")
    print(" " * 16 + "".join(f"{n[:12]:>14}" for n in names))
    for a in names:
        row = "".join(f"{aligned(tracks[a], tracks[b], tolerance):>14.2f}" for b in names)
        print(f"{a[:16]:<16}{row}")


if __name__ == "__main__":
    main()
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `python3 -m unittest discover -s scripts/phase0 -p 'test_*.py'`
Expected: `Ran 8 tests` and `OK`.

- [ ] **Step 6: Write the reporting scripts**

`scripts/phase0/summarize_scan.py`:

```python
"""Print what a recorded scan says about a disc."""
import pathlib
import sys

from robot import parse


def main(recording):
    scan = parse((pathlib.Path(recording) / "scan.txt").read_text(encoding="utf-8", errors="replace"))
    version = next((p[0] for code, _, p in scan.messages if code == 1005 and p), "?")
    java = next((p[0] for code, _, p in scan.messages if code == 3344 and p), None)
    print(f"MakeMKV: {version}")
    print(f"Java in use: {java or 'no (MSG:3344 absent)'}")
    print(f"Disc: type={scan.disc.get(1)} name={scan.disc.get(2)} volume={scan.disc.get(32)}")
    codes = sorted({code for code, _, _ in scan.messages})
    print(f"Message codes seen: {codes}")
    print()
    print(f"{'#':>3} {'source':<14} {'duration':>9} {'chap':>4} {'bytes':>13} {'segs':>4} {'aud':>3} {'sub':>3}  comment / segment map")
    for title in sorted(scan.titles):
        a = scan.titles[title]
        kinds = [s.get(1) for (t, _), s in scan.streams.items() if t == title]
        print(
            f"{title:>3} {a.get(16, ''):<14} {a.get(9, ''):>9} {a.get(8, '0'):>4} {a.get(11, ''):>13} "
            f"{a.get(25, ''):>4} {kinds.count('Audio'):>3} {kinds.count('Subtitles'):>3}  "
            f"{a.get(49, '')} [{a.get(26, '')[:60]}]"
        )


if __name__ == "__main__":
    main(sys.argv[1])
```

`scripts/phase0/compare_tracks.py`:

```python
"""Compare the streams MakeMKV listed for a title with the tracks in the ripped file."""
import argparse
import json
import pathlib
import sys

from robot import duration_seconds, parse

TYPE = {"Video": "video", "Audio": "audio", "Subtitles": "subtitles"}


def expected_rows(scan, title, selection):
    rows = []
    for (t, stream), a in sorted(scan.streams.items()):
        if t != title:
            continue
        kind = TYPE.get(a.get(1), a.get(1))
        flags = int(a.get(22, "0") or 0)
        lang = a.get(3) or a.get(28) or "und"
        if "MVC" in a.get(6, ""):
            continue
        if selection == "eng":
            if kind == "audio" and (lang not in ("eng", "und") or flags & 256 or flags & 7):
                continue
            if kind == "subtitles" and lang != "eng":
                continue
        rows.append(
            {"stream": stream, "type": kind, "lang": lang, "codec": a.get(6, ""),
             "out": a.get(41, ""), "ch": a.get(14, ""), "flags": flags}
        )
    return rows


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("scan")
    ap.add_argument("title", type=int)
    ap.add_argument("mkvmerge_json")
    ap.add_argument("--selection", choices=["all", "eng"], default="all")
    args = ap.parse_args()

    scan = parse(pathlib.Path(args.scan).read_text(encoding="utf-8", errors="replace"))
    ident = json.loads(pathlib.Path(args.mkvmerge_json).read_text(encoding="utf-8-sig"))
    want = expected_rows(scan, args.title, args.selection)
    got = [
        {"id": t["id"], "type": t["type"], "lang": t["properties"].get("language", "und"),
         "codec": t["codec"], "ch": t["properties"].get("audio_channels", ""),
         "tags": sorted(k for k in t["properties"] if k.startswith("tag_"))}
        for t in ident["tracks"]
    ]

    print(f"{'scan':<54} | file")
    for i in range(max(len(want), len(got))):
        w = want[i] if i < len(want) else None
        g = got[i] if i < len(got) else None
        left = f"{w['stream']:>2} {w['type']:<9} {w['lang']:<4} {w['codec']:<12} out={w['out']:<8} ch={w['ch']:<2} f={w['flags']}" if w else ""
        right = f"{g['id']:>2} {g['type']:<9} {g['lang']:<4} {g['codec']} ch={g['ch']}" if g else ""
        print(f"{left:<54} | {right}")

    order_ok = [(w["type"], w["lang"]) for w in want] == [(g["type"], g["lang"]) for g in got]
    title = scan.titles[args.title]
    file_chapters = sum(c.get("num_entries", 0) for c in ident.get("chapters", []))
    file_seconds = ident["container"]["properties"].get("duration", 0) / 1e9
    print()
    print(f"TRACK COUNT      scan={len(want)} file={len(got)}")
    print(f"ORDER MATCH      {'yes' if order_ok else 'no'} (type and language, position by position)")
    print(f"CHAPTERS         scan={title.get(8, '0')} file={file_chapters}")
    print(f"DURATION         scan={duration_seconds(title.get(9, '0:00:00'))}s file={file_seconds:.1f}s")
    print(f"VIDEO TRACKS     {sum(1 for g in got if g['type'] == 'video')}")
    print(f"STATISTICS TAGS  {'present' if any(g['tags'] for g in got) else 'absent'}")
    sys.exit(0 if order_ok and len(want) == len(got) else 1)


if __name__ == "__main__":
    main()
```

`scripts/phase0/compare_scans.py`:

```python
"""Compare the title lists of two recorded scans by source file, segment map and duration."""
import pathlib
import sys

from robot import parse


def titles(path):
    scan = parse(pathlib.Path(path).read_text(encoding="utf-8", errors="replace"))
    return {(a.get(16), a.get(26), a.get(9)): a.get(49, "") for a in scan.titles.values()}


def main(path_a, path_b):
    a, b = titles(path_a), titles(path_b)
    only_a = sorted(str(k) for k in set(a) - set(b))
    only_b = sorted(str(k) for k in set(b) - set(a))
    labels_differ = sorted(str(k) for k in set(a) & set(b) if a[k] != b[k])
    print(f"titles: A={len(a)} B={len(b)} common={len(set(a) & set(b))}")
    for name, items in (("only in A", only_a), ("only in B", only_b), ("comment differs", labels_differ)):
        print(f"{name}: {len(items)}")
        for item in items:
            print(f"  {item}")
    sys.exit(0 if not only_a and not only_b else 1)


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
```

`scripts/phase0/exp02_check.py`:

```python
"""Experiment 2: do the 3307/3308 title numbers of a rip call equal the index ripped?"""
import pathlib
import re
import sys

from robot import parse


def main(recording):
    d = pathlib.Path(recording)
    scan = parse((d / "scan.txt").read_text(encoding="utf-8", errors="replace"))
    all_ok = True
    runs = sorted((d / "exp02").glob("title-*.txt"), key=lambda p: int(re.search(r"\d+", p.stem).group()))
    for path in runs:
        title = int(re.search(r"\d+", path.stem).group())
        run = parse(path.read_text(encoding="utf-8-sig", errors="replace"))
        codes = [code for code, _, _ in run.messages]
        added = {
            int(params[-1]): params[0]
            for code, _, params in run.messages
            if code in (3307, 3308) and len(params) >= 2 and params[-1].isdigit()
        }
        expected = scan.titles.get(title, {}).get(16)
        saw_5014 = 5014 in codes
        before = saw_5014 and all(
            i < codes.index(5014) for i, code in enumerate(codes) if code in (3307, 3308)
        )
        match = added.get(title) == expected
        all_ok = all_ok and saw_5014 and before and match
        print(
            f"title {title:>3}: scan source={expected} rip #{title}={added.get(title)} "
            f"match={'yes' if match else 'NO'} 5014={'yes' if saw_5014 else 'NO'} "
            f"added-before-5014={'yes' if before else 'NO'} 3308-lines={codes.count(3308)} 3309-lines={codes.count(3309)}"
        )
    print(f"\nEXPERIMENT 2: {'yes' if all_ok and runs else 'no'} ({len(runs)} titles)")


if __name__ == "__main__":
    main(sys.argv[1])
```

`scripts/phase0/hit_rate.py`:

```python
"""Experiment 10: how many recorded discs does TheDiscDB know?"""
import json
import pathlib
import sys


def hit(path):
    try:
        data = json.loads(path.read_text(encoding="utf-8-sig"))
    except (OSError, ValueError):
        return False
    if isinstance(data, list):
        return len(data) > 0
    return isinstance(data, dict) and bool(data.get("titles"))


def main(root):
    rows = []
    for disc in sorted(p for p in pathlib.Path(root).iterdir() if (p / "meta.json").exists()):
        meta = json.loads((disc / "meta.json").read_text(encoding="utf-8-sig"))
        by_hash = hit(disc / "thediscdb-hash.json")
        by_id = hit(disc / "thediscdb-discid.json")
        rows.append((disc.name, meta.get("format"), by_hash, by_id))
        print(f"{disc.name:<40} {str(meta.get('format')):<7} hash={'hit' if by_hash else 'miss':<4} discid={'hit' if by_id else 'miss'}")
    known = sum(1 for _, _, h, i in rows if h or i)
    print(f"\nTheDiscDB knows {known} of {len(rows)} discs")


if __name__ == "__main__":
    main(sys.argv[1])
```

`scripts/phase0/check_encoding.py`:

```python
"""Experiment 8: which encoding does makemkvcon use for robot output?"""
import pathlib
import sys


def main(path):
    data = pathlib.Path(path).read_bytes()
    try:
        data.decode("utf-8")
        print("valid UTF-8: yes")
    except UnicodeDecodeError as error:
        print(f"valid UTF-8: NO ({error})")
    shown = 0
    for raw in data.splitlines():
        if any(byte > 127 for byte in raw):
            print("utf-8 :", raw.decode("utf-8", errors="replace")[:160])
            print("cp1252:", raw.decode("cp1252", errors="replace")[:160])
            shown += 1
            if shown == 5:
                break
    print(f"lines with non-ASCII bytes shown: {shown}")
    quoted = [raw for raw in data.splitlines() if b'\\"' in raw]
    print(f"lines with a backslash-escaped quote: {len(quoted)}")
    for raw in quoted[:3]:
        print("  ", raw.decode("utf-8", errors="replace")[:160])


if __name__ == "__main__":
    main(sys.argv[1])
```

- [ ] **Step 7: Try the reporting scripts on a public log**

Run:

```bash
mkdir -p /tmp/lazymkv-jw && cp "corpus/thediscdb/movie/John Wick (2014)/4k-ultra-hd--blu-ray/disc01.txt" /tmp/lazymkv-jw/scan.txt
python3 scripts/phase0/summarize_scan.py /tmp/lazymkv-jw
python3 scripts/phase0/compare_scans.py /tmp/lazymkv-jw/scan.txt /tmp/lazymkv-jw/scan.txt
python3 scripts/phase0/check_encoding.py /tmp/lazymkv-jw/scan.txt
```

Expected: the summary's first lines are `MakeMKV: MakeMKV v1.18.4 win(x64-release)` and `Java in use: ...`, with a row `0 00001.mpls 1:41:13 16 ...`; `compare_scans` prints `only in A: 0` and `only in B: 0`; `check_encoding` prints `valid UTF-8: yes`.

- [ ] **Step 8: Commit**

```bash
git add scripts/phase0/*.py
git commit -m "Add robot-output parser and phase 0 analysis scripts"
```

---

### Task 7: Mini PC setup

**Files:**
- Create: `scripts/phase0/env.ps1`, `scripts/phase0/run-interactive.ps1`, `scripts/phase0/setup-check.ps1`, `scripts/phase0/job.sh`, `scripts/phase0/wait.sh`
- Create: `docs/phase0/environment.md`, `docs/phase0/experiments/TEMPLATE.md`
- On the mini PC only (not in the repository): `C:\lazymkv\phase0\config.ps1`, `C:\lazymkv\phase0\profiles\*.mmcp.xml`

**Interfaces:**
- Consumes: `lazymkv-capture profile` (Task 3).
- Produces, for every later task:
  - `env.ps1`, dot-sourced by every script. It defines `$MK`, `$P0`, `$CAP`, `$AllTracksProfile`, `$EngProfile`, `$Staging`, `$Drive`, `$TestLibrary`, `$MKVMERGE`, `$FFPROBE`, `$FFMPEG`, `$MEDIAINFO`, and the functions `Get-MkFlags [-Profile <path>] [-CacheMb <n>]` (returns the argument array), `Get-DiscIndex` (MakeMKV's index of drive `$Drive`) and `Get-FreeRamGB`.
  - `run-interactive.ps1 -Script <name.ps1> [-ArgLine '<args>'] -Log <file>` starts a script from `scripts\phase0` in the owner's desktop session and returns at once. The log ends with a line starting `PHASE0-DONE` when the script has finished.
  - `scripts/phase0/job.sh <script.ps1> <log name> [arguments]` (Mac) calls `run-interactive.ps1` with the log `C:\lazymkv\phase0\logs\<log name>.txt`; `scripts/phase0/wait.sh <log name>` (Mac) waits for that log's `PHASE0-DONE` line and prints the log.
  - The profiles `C:\lazymkv\phase0\profiles\all-tracks.mmcp.xml` and `eng-only.mmcp.xml`.

- [ ] **Step 1: Get the owner's approval and inputs**

Ask the owner in chat, and wait for the answers:

1. "May I install or upgrade these on the mini PC with winget: MakeMKV 2.0.0 (`GuinpinSoft.MakeMKV`, upgrade from 1.18.3), MKVToolNix (`MoritzBunkus.MKVToolNix`), FFmpeg (`Gyan.FFmpeg`) and MediaInfo command-line (`MediaArea.MediaInfo`)? Java 8 is already installed, so no Java install is needed."
2. "What is the staging folder on the NAS (full `\\server\share\folder` path)? Please open it once in File Explorer on the mini PC and tick 'remember my credentials'."
3. "Please create a TMDb API read access token and set it on the mini PC by running `setx TMDB_TOKEN <token>` in a normal (non-admin) terminal, then sign out and back in."
4. "For experiment 9 I need an empty folder that you will add to Plex and Jellyfin as a movie library. What path should I use? It must not be inside your real libraries."

Do not continue until the owner has answered 1 and 2. Items 3 and 4 can arrive later (Tasks 8 and 16 need them).

- [ ] **Step 2: Install the approved packages**

Run, with only the packages the owner approved:

```bash
ssh USER@makemkv-box "powershell -NoProfile -Command \"foreach (\$id in 'GuinpinSoft.MakeMKV','MoritzBunkus.MKVToolNix','Gyan.FFmpeg','MediaArea.MediaInfo') { winget install --id \$id -e --silent --accept-package-agreements --accept-source-agreements --disable-interactivity | Select-Object -Last 2 }\""
```

Expected: each package ends with `Successfully installed`. If winget reports that MakeMKV is already installed, run the same command for that package with `upgrade` in place of `install`.

- [ ] **Step 3: Write the shared environment script**

`scripts/phase0/env.ps1`:

```powershell
# Dot-source on the mini PC:  . C:\lazymkv\src\scripts\phase0\env.ps1
$MK  = 'C:\Program Files (x86)\MakeMKV\makemkvcon64.exe'
$P0  = 'C:\lazymkv\phase0'
$CAP = 'C:\lazymkv\target\release\lazymkv-capture.exe'
$AllTracksProfile = "$P0\profiles\all-tracks.mmcp.xml"
$EngProfile       = "$P0\profiles\eng-only.mmcp.xml"

# Owner-specific values: $Staging, $Drive, $TestLibrary.
. "$P0\config.ps1"

function Resolve-Tool([string]$Name, [string[]]$Fallbacks) {
    $found = Get-Command $Name -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($found) { return $found.Source }
    foreach ($path in $Fallbacks) { if (Test-Path $path) { return $path } }
    return $null
}

$links = @("$env:LOCALAPPDATA\Microsoft\WinGet\Links", 'C:\Program Files\WinGet\Links')
$MKVMERGE  = Resolve-Tool 'mkvmerge.exe'  @('C:\Program Files\MKVToolNix\mkvmerge.exe')
$FFPROBE   = Resolve-Tool 'ffprobe.exe'   ($links | ForEach-Object { "$_\ffprobe.exe" })
$FFMPEG    = Resolve-Tool 'ffmpeg.exe'    ($links | ForEach-Object { "$_\ffmpeg.exe" })
$MEDIAINFO = Resolve-Tool 'mediainfo.exe' ($links | ForEach-Object { "$_\mediainfo.exe" })

function Get-MkFlags([string]$Profile = $AllTracksProfile, [int]$CacheMb = 1024) {
    @('-r', '--messages=-stdout', '--progress=-same', '--minlength=120', "--cache=$CacheMb", "--profile=$Profile")
}

# MakeMKV's index for the drive letter in $Drive, read from the seventh DRV field.
function Get-DiscIndex {
    $pattern = '^DRV:(\d+),\d+,\d+,\d+,".*",".*","' + [regex]::Escape("${Drive}:") + '"\s*$'
    $line = & $MK -r --cache=1 info disc:9999 | Where-Object { $_ -match $pattern } | Select-Object -First 1
    if (-not $line) { throw "MakeMKV lists no drive at ${Drive}:" }
    [int]([regex]::Match($line, '^DRV:(\d+),').Groups[1].Value)
}

function Get-FreeRamGB {
    [math]::Round((Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory / 1MB, 2)
}
```

- [ ] **Step 4: Write the desktop-session launcher**

`scripts/phase0/run-interactive.ps1`:

```powershell
# Starts a phase 0 script in the owner's logged-on desktop session, elevated, and returns at once.
# Saved share credentials and the optical drive are available there. Poll the log for PHASE0-DONE.
# ArgLine must not contain double quotes; use single quotes around values with spaces.
param(
    [Parameter(Mandatory)][string]$Script,
    [string]$ArgLine = '',
    [Parameter(Mandatory)][string]$Log
)
$full = "C:\lazymkv\src\scripts\phase0\$Script"
if (-not (Test-Path $full)) { throw "no such script: $full" }
New-Item -ItemType Directory -Force (Split-Path $Log) | Out-Null
Remove-Item $Log -ErrorAction SilentlyContinue
$inner = "& '$full' $ArgLine *> '$Log'; Add-Content '$Log' ('PHASE0-DONE exit=' + `$LASTEXITCODE)"
$action = New-ScheduledTaskAction -Execute 'powershell.exe' `
    -Argument "-NoProfile -WindowStyle Hidden -ExecutionPolicy Bypass -Command `"$inner`""
$principal = New-ScheduledTaskPrincipal -UserId (whoami) -LogonType Interactive -RunLevel Highest
Register-ScheduledTask -TaskName 'lazymkv-phase0' -Action $action -Principal $principal -Force | Out-Null
Start-ScheduledTask -TaskName 'lazymkv-phase0'
"started $Script; log: $Log"
```

- [ ] **Step 5: Write the Mac-side job helpers**

`scripts/phase0/job.sh`:

```bash
#!/usr/bin/env bash
# Start a phase 0 script in the owner's desktop session on the mini PC.
# Usage: scripts/phase0/job.sh <script.ps1> <log name> [script arguments, without quotes]
set -euo pipefail
HOST="${LAZYMKV_BOX:-USER@makemkv-box}"
SCRIPT="$1"
LOG="$2"
shift 2
ssh -o BatchMode=yes "$HOST" "powershell -NoProfile -ExecutionPolicy Bypass -Command \"& C:\\lazymkv\\src\\scripts\\phase0\\run-interactive.ps1 -Script $SCRIPT -ArgLine '$*' -Log C:\\lazymkv\\phase0\\logs\\$LOG.txt\""
```

`scripts/phase0/wait.sh`:

```bash
#!/usr/bin/env bash
# Wait up to about nine minutes for a desktop-session job, then print its log.
# Exit code 3 means it is still running: run this again.
set -euo pipefail
HOST="${LAZYMKV_BOX:-USER@makemkv-box}"
LOG="C:\\lazymkv\\phase0\\logs\\$1.txt"
for _ in $(seq 1 52); do
  if ssh -o BatchMode=yes "$HOST" "findstr /b PHASE0-DONE $LOG" >/dev/null 2>&1; then
    ssh -o BatchMode=yes "$HOST" "type $LOG"
    exit 0
  fi
  sleep 10
done
echo "still running; last lines:"
ssh -o BatchMode=yes "$HOST" "powershell -NoProfile -Command \"Get-Content '$LOG' -Tail 5\""
exit 3
```

Run: `chmod +x scripts/phase0/job.sh scripts/phase0/wait.sh`

- [ ] **Step 6: Write the setup check**

`scripts/phase0/setup-check.ps1`:

```powershell
# Verifies tools, profiles, the drive and the staging share from the owner's desktop session.
. C:\lazymkv\src\scripts\phase0\env.ps1

"makemkvcon : " + ((& $MK -r --cache=1 info disc:9999 | Select-Object -First 1))
"mkvmerge   : $MKVMERGE : " + $(if ($MKVMERGE) { (& $MKVMERGE --version) } else { 'MISSING' })
"ffprobe    : $FFPROBE : " + $(if ($FFPROBE) { (& $FFPROBE -version | Select-Object -First 1) } else { 'MISSING' })
"ffmpeg     : $FFMPEG"
"mediainfo  : $MEDIAINFO : " + $(if ($MEDIAINFO) { (& $MEDIAINFO --Version | Select-Object -Last 1) } else { 'MISSING' })
"java       : " + (Get-ItemProperty 'HKCU:\Software\MakeMKV' -ErrorAction SilentlyContinue).app_Java
"drive      : ${Drive}: is MakeMKV disc index $(Get-DiscIndex)"
"free RAM   : $(Get-FreeRamGB) GB"
"TMDB_TOKEN : " + $(if ($env:TMDB_TOKEN) { 'set' } else { 'NOT SET' })
"profiles   : all-tracks=$(Test-Path $AllTracksProfile) eng-only=$(Test-Path $EngProfile)"

$probe = "$Staging\lazymkv-write-test.txt"
try {
    New-Item -ItemType Directory -Force $Staging | Out-Null
    Set-Content $probe 'ok'
    Rename-Item $probe 'lazymkv-write-test-renamed.txt'
    Remove-Item "$Staging\lazymkv-write-test-renamed.txt"
    "staging    : $Staging : write, rename and delete OK"
} catch {
    "staging    : $Staging : FAILED: $($_.Exception.Message)"
}
Add-Type -Namespace Win32 -Name Disk -MemberDefinition '[DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)] public static extern bool GetDiskFreeSpaceEx(string path, out ulong freeToCaller, out ulong total, out ulong totalFree);'
[uint64]$free = 0; [uint64]$total = 0; [uint64]$totalFree = 0
[Win32.Disk]::GetDiskFreeSpaceEx("$Staging\", [ref]$free, [ref]$total, [ref]$totalFree) | Out-Null
"staging free: $([math]::Round($free / 1GB, 1)) GB"
```

- [ ] **Step 7: Write the owner's config and the profiles on the mini PC**

Sync, then write the config with the staging path and test-library path the owner gave (replace the two example paths; if the owner has not yet given a test-library path, leave `$TestLibrary = ''` and fill it in before Task 16):

```bash
scripts/box.sh cargo build --release -p lazymkv-capture
cat > /tmp/lazymkv-config.ps1 <<'CONFIG'
$Staging = '\\NAS\media\lazymkv-staging'
$Drive = 'D'
$TestLibrary = '\\NAS\media\lazymkv-test-library'
CONFIG
ssh USER@makemkv-box "mkdir C:\lazymkv\phase0\profiles 2>nul & exit /b 0"
scp -q /tmp/lazymkv-config.ps1 USER@makemkv-box:C:/lazymkv/phase0/config.ps1
```

Then extract MakeMKV's default profile and build the two job profiles:

```bash
ssh USER@makemkv-box "cd /d C:\lazymkv\phase0\profiles && tar -xf \"C:\Program Files (x86)\MakeMKV\appdata.tar\" default.mmcp.xml && C:\lazymkv\target\release\lazymkv-capture.exe profile --default default.mmcp.xml --name \"lazymkv all tracks\" --selection \"+sel:all,-sel:mvcvideo,=100:all\" --out all-tracks.mmcp.xml && C:\lazymkv\target\release\lazymkv-capture.exe profile --default default.mmcp.xml --name \"lazymkv eng only\" --selection \"-sel:all,+sel:video,+sel:(audio*(eng|nolang)),+sel:(subtitle*eng),-sel:mvcvideo,-sel:core,-sel:special,=100:all\" --out eng-only.mmcp.xml && findstr /c:\"defaultSelection\" all-tracks.mmcp.xml eng-only.mmcp.xml"
```

Expected: `findstr` prints `defaultSelection="+sel:all,-sel:mvcvideo,=100:all"` lines for the first file and the language selection for the second. (`*` is MakeMKV's alias for "and", which avoids `&` on the command line.)

- [ ] **Step 8: Run the setup check in the desktop session**

```bash
scripts/phase0/job.sh setup-check.ps1 setup-check
scripts/phase0/wait.sh setup-check
```

Expected: a `makemkvcon` line containing `MakeMKV v2.0.0`; versions for `mkvmerge` (54 or later) and `ffprobe` (6.1 or later); `drive : D: is MakeMKV disc index 0`; `profiles : all-tracks=True eng-only=True`; `staging ... write, rename and delete OK`; and a final line `PHASE0-DONE exit=`. If the staging line says FAILED, ask the owner to open the share in File Explorer with "remember my credentials" and run this step again. If the log is empty, the owner is not logged in at the console: ask them to log in.

- [ ] **Step 9: Write the environment report and the experiment template**

`docs/phase0/environment.md`: copy the lines of `setup-check.txt` into a fenced block under the heading `# Mini PC environment (date)`, and add the Windows build (`ssh USER@makemkv-box ver`) and both drive models as they appear in MakeMKV's `DRV` line.

`docs/phase0/experiments/TEMPLATE.md`:

```markdown
# Experiment NN: title

- Date:
- Disc(s): slug, format, drive
- MakeMKV version:
- Spec reference: section 17, row NN

## Question

## What was run

## What was observed

## Answer

yes / no, in one sentence.

## Consequence for the spec

Which section changes and how, or "none".
```

- [ ] **Step 10: Commit**

```bash
git add scripts/phase0/env.ps1 scripts/phase0/run-interactive.ps1 scripts/phase0/setup-check.ps1 scripts/phase0/job.sh scripts/phase0/wait.sh docs/phase0
git commit -m "Set up the mini PC for phase 0 and record its environment"
```

---

### Task 8: Disc recording procedure

**Files:**
- Create: `scripts/phase0/record-disc.ps1`, `scripts/phase0/pull-recording.sh`
- Create: `corpus/owner/README.md`

**Interfaces:**
- Consumes: `env.ps1`, `lazymkv-capture disc` and `record`.
- Produces: `record-disc.ps1 -Slug <name> [-NoScan] [-ScanName <name>]` (the scan is written to `<ScanName>.txt`, default `scan.txt`) writes a recording to `C:\lazymkv\phase0\recordings\<slug>\`: `meta.json`, `files.json`, `nav\`, `scan.txt` (with `.timing.jsonl`, `.stderr`, `.exit.json`), `thediscdb-hash.json`, `thediscdb-discid.json`, `tmdb.json` (when TheDiscDB gives a TMDb ID and `TMDB_TOKEN` is set) and `lookup-status.txt`. `scripts/phase0/pull-recording.sh <slug>` copies it to `corpus/owner/<slug>/` on the Mac. A slug is lowercase letters, digits and hyphens, for example `dune-2021-uhd`.

- [ ] **Step 1: Write the recording script**

`scripts/phase0/record-disc.ps1`:

```powershell
# Records what can be learned about the inserted disc without ripping it.
param(
    [Parameter(Mandatory)][string]$Slug,
    [switch]$NoScan,
    [string]$ScanName = 'scan'
)
. C:\lazymkv\src\scripts\phase0\env.ps1
$out = "$P0\recordings\$Slug"
New-Item -ItemType Directory -Force $out | Out-Null

$label = (Get-Volume -DriveLetter $Drive).FileSystemLabel
if (-not $label) { $label = '(none)' }
& $CAP disc --root "${Drive}:\" --label "$label" --out $out | Out-Null
$meta = Get-Content "$out\meta.json" -Raw | ConvertFrom-Json
"disc: label=$label format=$($meta.format) fingerprint=$($meta.fingerprint) disc_id=$($meta.disc_id)"

$status = @()
if ($meta.fingerprint) {
    $code = curl.exe -s -m 30 -o "$out\thediscdb-hash.json" -w '%{http_code}' "https://thediscdb.com/api/dischash/$($meta.fingerprint)"
    $status += "dischash $code"
}
if ($meta.disc_id) {
    $code = curl.exe -s -m 30 -o "$out\thediscdb-discid.json" -w '%{http_code}' "https://thediscdb.com/api/discid/$($meta.disc_id)"
    $status += "discid $code"
}

$tmdbId = $null
$kind = $null
try {
    $hit = Get-Content "$out\thediscdb-hash.json" -Raw -ErrorAction Stop | ConvertFrom-Json
    $tmdbId = $hit.media.external_ids.tmdb
    $kind = if ($hit.media.type -eq 'Movie') { 'movie' } else { 'tv' }
} catch { }
if ($tmdbId -and $env:TMDB_TOKEN) {
    $code = curl.exe -s -m 30 -o "$out\tmdb.json" -w '%{http_code}' -H "Authorization: Bearer $env:TMDB_TOKEN" "https://api.themoviedb.org/3/$kind/${tmdbId}?append_to_response=external_ids"
    $status += "tmdb $kind/$tmdbId $code"
}
Set-Content "$out\lookup-status.txt" $status
"lookups: $($status -join '; ')"

if (-not $NoScan) {
    $index = Get-DiscIndex
    $flags = Get-MkFlags
    & $CAP record --out "$out\$ScanName.txt" -- $MK @flags info "disc:$index" | Out-Null
    "scan: " + (Get-Content "$out\$ScanName.txt.exit.json")
}
```

- [ ] **Step 2: Write the pull script**

`scripts/phase0/pull-recording.sh`:

```bash
#!/usr/bin/env bash
# Copy a recording from the mini PC into corpus/owner/<slug>.
set -euo pipefail
SLUG="$1"
HOST="${LAZYMKV_BOX:-USER@makemkv-box}"
cd "$(git rev-parse --show-toplevel)"
mkdir -p corpus/owner
rm -rf "corpus/owner/$SLUG"
scp -q -r -o BatchMode=yes "$HOST:C:/lazymkv/phase0/recordings/$SLUG" corpus/owner/
echo "pulled corpus/owner/$SLUG ($(find "corpus/owner/$SLUG" -type f | wc -l | tr -d ' ') files)"
```

Run: `chmod +x scripts/phase0/pull-recording.sh`

`corpus/owner/README.md`:

```markdown
# Recordings of the owner's discs

One folder per disc, written by `scripts/phase0/record-disc.ps1` and the
experiment scripts on the mini PC, then copied here with
`scripts/phase0/pull-recording.sh`. Layout: spec section 15, "Recordings".
`AACS/Unit_Key_RO.inf` is never stored; `meta.json` holds only its SHA-1.
```

- [ ] **Step 3: Record the first disc, without a scan**

Ask the owner to insert any disc and to say what it is. Choose a slug for it. Then:

```bash
scripts/box.sh
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\record-disc.ps1 -Slug SLUG -NoScan"
scripts/phase0/pull-recording.sh SLUG
python3 -c "import json; m=json.load(open('corpus/owner/SLUG/meta.json', encoding='utf-8-sig')); print(m)"
```

Expected: a `disc:` line with a 32-character fingerprint, a `lookups:` line with HTTP codes, and `meta.json` on the Mac with `file_count` above 0. For a Blu-ray or UHD, `disc_id` is 40 hex characters; if it is null, note that `AACS/Unit_Key_RO.inf` could not be read (this is part of experiment 5).

Do not use Disc A (the never-scanned UHD kept for Task 9) for this step.

- [ ] **Step 4: Check the fingerprint against TheDiscDB, if it knows the disc**

Run: `python3 scripts/phase0/hit_rate.py corpus/owner`
Expected: one row for the disc. If it is a hit, open `corpus/owner/SLUG/thediscdb-hash.json` and confirm its `content_hash` equals `meta.json`'s `fingerprint`. A hit proves the fingerprint is computed correctly from a real disc on this machine. A miss proves nothing either way; repeat this step with another disc until one hits or five discs have missed, and record the outcome.

- [ ] **Step 5: Commit**

```bash
git add scripts/phase0/record-disc.ps1 scripts/phase0/pull-recording.sh corpus/owner
git commit -m "Add disc recording procedure and the first recording"
```

---

### Task 9: First contact with Disc A (experiments 5 and 12)

**Files:**
- Create: `corpus/owner/<slug>/` (recording), `docs/phase0/experiments/EXP-05-offline-and-unit-key.md`, `docs/phase0/experiments/EXP-12-java-labels.md`

**Interfaces:**
- Consumes: `record-disc.ps1`, `run-interactive.ps1`, `pull-recording.sh`, `summarize_scan.py`, `compare_scans.py`.
- Produces: the recording of Disc A (`scan-offline.txt` and `scan.txt`) used by Tasks 10 to 14, and the slug chosen for it, written at the top of both reports.

Disc A is a UHD film that MakeMKV has never scanned on this machine. Experiment 5 must be the first time MakeMKV sees it.

- [ ] **Step 1: Ask the owner to block MakeMKV's network access**

Ask the owner to run this in an administrator PowerShell on the mini PC and to say when it is done:

```powershell
New-NetFirewallRule -DisplayName 'lazymkv-exp05' -Direction Outbound -Program 'C:\Program Files (x86)\MakeMKV\makemkvcon64.exe' -Action Block
```

Verify (read-only): `ssh USER@makemkv-box "powershell -NoProfile -Command \"(Get-NetFirewallRule -DisplayName 'lazymkv-exp05').Enabled\""`
Expected: `True`.

- [ ] **Step 2: Ask the owner to insert Disc A, then record it offline**

Choose the slug from the film's title, year and format (for example `dune-2021-uhd`). Use it in place of `SLUG` for the rest of this plan's Disc A steps.

```bash
scripts/box.sh
scripts/phase0/job.sh record-disc.ps1 record-offline -Slug SLUG -ScanName scan-offline
```

Wait with `scripts/phase0/wait.sh record-offline`. A UHD scan with Java analysis can take ten minutes or more.
Expected: the log shows a `disc:` line, a `lookups:` line and a `scan:` line, then `PHASE0-DONE`.

- [ ] **Step 3: Ask the owner to remove the block**

```powershell
Remove-NetFirewallRule -DisplayName 'lazymkv-exp05'
```

Verify: `ssh USER@makemkv-box "powershell -NoProfile -Command \"@(Get-NetFirewallRule -DisplayName 'lazymkv-exp05' -ErrorAction SilentlyContinue).Count\""`
Expected: `0`.

- [ ] **Step 4: Record the disc again, online**

```bash
scripts/phase0/job.sh record-disc.ps1 record-online -Slug SLUG
```

Wait with `scripts/phase0/wait.sh record-online`, then:

```bash
scripts/phase0/pull-recording.sh SLUG
python3 scripts/phase0/summarize_scan.py corpus/owner/SLUG
python3 scripts/phase0/compare_scans.py corpus/owner/SLUG/scan-offline.txt corpus/owner/SLUG/scan.txt
python3 - <<'EOF'
import pathlib, sys
sys.path.insert(0, "scripts/phase0")
from robot import parse
d = pathlib.Path("corpus/owner/SLUG")
for name in ("scan-offline.txt", "scan.txt"):
    scan = parse((d / name).read_text(encoding="utf-8", errors="replace"))
    print(name, "titles:", len(scan.titles))
    for code, text, _ in scan.messages:
        if code >= 5000 or code in (2003, 3303, 3032):
            print("  ", code, text[:140])
EOF
```

Expected: the summary lists the disc's titles. The comparison and the message list show how the offline scan differed.

- [ ] **Step 5: Write the experiment 5 report**

Create `docs/phase0/experiments/EXP-05-offline-and-unit-key.md` from the template. Record:

- the exact offline messages (code and text), and whether the offline scan listed the same titles as the online scan;
- whether `meta.json` has a 40-character `disc_id` (that is, whether `AACS/Unit_Key_RO.inf` was readable in this drive);
- that MakeMKV's data folder on this machine already holds `keydb.cfg` and cached keys, which can make a disc open offline.

Answer: "yes, MakeMKV reports `<code and text>` when it cannot reach the network" or "no distinct message was seen; the disc opened offline". Consequence: the first gives `NeedsNetwork` (spec 10.2) its trigger codes; the second leaves it `Unknown`, as the spec's fallback says.

- [ ] **Step 6: Write the experiment 12 report**

Create `docs/phase0/experiments/EXP-12-java-labels.md`. From the online summary record: the `Java in use` line (the path MakeMKV printed in `MSG:3344`, or its absence), and every title whose comment column is not empty, with its source file. Answer "yes" only if `MSG:3344` is present and at least one title's comment starts `FPL_MainFeature`. If Disc A has no Java menus (no labels and no `3344`), say so and repeat this check on each later UHD until one answers it; the report is updated then.

- [ ] **Step 7: Commit**

```bash
git add corpus/owner docs/phase0/experiments
git commit -m "Record Disc A offline and online; report experiments 5 and 12"
```

---

### Task 10: Index guard (experiment 2)

**Files:**
- Create: `scripts/phase0/exp02-index-guard.ps1`, `docs/phase0/experiments/EXP-02-index-guard.md`

**Interfaces:**
- Consumes: Disc A's recording (`scan.txt`), `exp02_check.py`.
- Produces: `exp02-index-guard.ps1 -Slug <slug> [-MaxTitles 12]` writes `recordings\<slug>\exp02\title-<n>.txt`, the robot output of a rip call for title `n` up to its `MSG:5014` line.

- [ ] **Step 1: Write the script**

`scripts/phase0/exp02-index-guard.ps1`:

```powershell
# Experiment 2: start a rip for a title, stop it when saving begins (MSG:5014), keep the output.
param(
    [Parameter(Mandatory)][string]$Slug,
    [int]$MaxTitles = 12
)
. C:\lazymkv\src\scripts\phase0\env.ps1
$rec = "$P0\recordings\$Slug"
$out = "$rec\exp02"
New-Item -ItemType Directory -Force $out | Out-Null

$all = @(Get-Content "$rec\scan.txt" | ForEach-Object { if ($_ -match '^TINFO:(\d+),') { [int]$Matches[1] } } | Sort-Object -Unique)
if ($all.Count -le $MaxTitles) {
    $titles = $all
} else {
    # Evenly spaced, always including the first and the last title.
    $titles = @(0..($MaxTitles - 1) | ForEach-Object { $all[[int][math]::Round($_ * ($all.Count - 1) / ($MaxTitles - 1))] } | Sort-Object -Unique)
}
"testing titles: $($titles -join ', ') of $($all.Count)"

$index = Get-DiscIndex
$flags = (Get-MkFlags) -join ' '
foreach ($title in $titles) {
    $tmp = "$P0\tmp\exp02-$title"
    Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force $tmp | Out-Null

    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $MK
    $psi.Arguments = "$flags mkv disc:$index $title `"$tmp`""
    $psi.RedirectStandardOutput = $true
    $psi.UseShellExecute = $false
    $psi.CreateNoWindow = $true
    $process = [System.Diagnostics.Process]::Start($psi)

    $lines = New-Object System.Collections.Generic.List[string]
    $stopped = $false
    while ($null -ne ($line = $process.StandardOutput.ReadLine())) {
        $lines.Add($line)
        if ($line.StartsWith('MSG:5014')) {
            $stopped = $true
            try { $process.Kill() } catch { }
            break
        }
    }
    $process.WaitForExit()
    Set-Content "$out\title-$title.txt" $lines -Encoding UTF8
    Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
    "title ${title}: lines=$($lines.Count) stopped-at-5014=$stopped"
}
```

- [ ] **Step 2: Run it on Disc A**

```bash
scripts/box.sh
scripts/phase0/job.sh exp02-index-guard.ps1 exp02 -Slug SLUG
```

Wait with `scripts/phase0/wait.sh exp02`. Each title costs one disc scan, so twelve titles can take an hour or more.
Expected: one `title N: lines=... stopped-at-5014=True` line per tested title.

- [ ] **Step 3: Evaluate**

```bash
scripts/phase0/pull-recording.sh SLUG
python3 scripts/phase0/exp02_check.py corpus/owner/SLUG
```

Expected: one line per title with `match=yes 5014=yes added-before-5014=yes`, and a final `EXPERIMENT 2: yes (N titles)`. Any `NO` is a finding, not a failure of the task.

- [ ] **Step 4: Write the report**

Create `docs/phase0/experiments/EXP-02-index-guard.md`. Paste the checker's output. State whether the disc has angle titles (`3308` lines) or skipped duplicates (`3309` lines); if it has neither, say that the experiment's "disc with angles or duplicate titles" condition is not yet met and that Task 15 repeats it on the decoy-playlist disc. Answer yes or no. Consequence if no: spec 6.2 step 6 drops the pre-read guard, as its fallback says.

- [ ] **Step 5: Commit**

```bash
git add scripts/phase0/exp02-index-guard.ps1 corpus/owner docs/phase0/experiments
git commit -m "Run experiment 2: index guard"
```

---

### Task 11: All-tracks rip to the share (experiments 1 and 4)

**Files:**
- Create: `scripts/phase0/rip-title.ps1`, `scripts/phase0/probe-title.ps1`
- Create: `docs/phase0/experiments/EXP-01-profile-and-order.md`, `docs/phase0/experiments/EXP-04-rip-to-share.md`

**Interfaces:**
- Consumes: Disc A in the drive, its recording, `compare_tracks.py`.
- Produces:
  - `rip-title.ps1 -Slug <slug> -Title <n> -Label <name> [-Profile <path>] [-CacheMb <n>]` rips one title to `$Staging\<slug>\<label>\` and writes `recordings\<slug>\<label>\rip.txt` (with timing) and `rip-summary.json` (`title`, `elapsed_s`, `bytes`, `mb_per_s`, `min_free_ram_gb`, `files`, `dest`).
  - `probe-title.ps1 -Slug <slug> -Label <name>` writes, in the same recording folder, `mkvmerge.json`, `ffprobe-streams.json`, `mediainfo.json` (when MediaInfo is installed), `subs\index.json`, `subs\sub-<n>.packets.json` and `probe-summary.json` (`evidence_s`, `evidence_bytes`, `remux_s`).

- [ ] **Step 1: Write the rip script**

`scripts/phase0/rip-title.ps1`:

```powershell
# Rips one title to the staging share with a given profile, recording output, time and memory.
param(
    [Parameter(Mandatory)][string]$Slug,
    [Parameter(Mandatory)][int]$Title,
    [Parameter(Mandatory)][string]$Label,
    [string]$Profile = '',
    [int]$CacheMb = 1024
)
. C:\lazymkv\src\scripts\phase0\env.ps1
if (-not $Profile) { $Profile = $AllTracksProfile }
$rec = "$P0\recordings\$Slug\$Label"
$dest = "$Staging\$Slug\$Label"
New-Item -ItemType Directory -Force $rec | Out-Null
if (Test-Path $dest) { Remove-Item $dest -Recurse -Force }
New-Item -ItemType Directory -Force $dest | Out-Null

$memFile = "$rec\min-free-ram.txt"
$memJob = Start-Job -ArgumentList $memFile -ScriptBlock {
    param($memFile)
    $min = [double]::MaxValue
    while ($true) {
        $free = (Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory / 1MB
        if ($free -lt $min) { $min = $free; Set-Content $memFile ([math]::Round($min, 2)) }
        Start-Sleep -Seconds 5
    }
}

$index = Get-DiscIndex
$flags = Get-MkFlags -Profile $Profile -CacheMb $CacheMb
$start = Get-Date
& $CAP record --out "$rec\rip.txt" -- $MK @flags mkv "disc:$index" $Title $dest | Out-Null
$elapsed = ((Get-Date) - $start).TotalSeconds
Stop-Job $memJob
Remove-Job $memJob

$files = @(Get-ChildItem $dest -Filter *.mkv -ErrorAction SilentlyContinue)
$bytes = ($files | Measure-Object Length -Sum).Sum
$summary = [ordered]@{
    title           = $Title
    profile         = $Profile
    cache_mb        = $CacheMb
    elapsed_s       = [int]$elapsed
    bytes           = $bytes
    mb_per_s        = if ($elapsed -gt 0 -and $bytes) { [math]::Round($bytes / 1MB / $elapsed, 1) } else { 0 }
    min_free_ram_gb = if (Test-Path $memFile) { [double](Get-Content $memFile) } else { $null }
    files           = @($files | ForEach-Object { $_.Name })
    dest            = $dest
}
$summary | ConvertTo-Json | Set-Content "$rec\rip-summary.json"
$summary | ConvertTo-Json
```

- [ ] **Step 2: Write the probe script**

`scripts/phase0/probe-title.ps1`:

```powershell
# Inspects a ripped title on the staging share: identification, stream profiles, subtitle evidence, remux time.
param(
    [Parameter(Mandatory)][string]$Slug,
    [Parameter(Mandatory)][string]$Label
)
. C:\lazymkv\src\scripts\phase0\env.ps1
$rec = "$P0\recordings\$Slug\$Label"
$dest = "$Staging\$Slug\$Label"
$mkv = (Get-ChildItem $dest -Filter *.mkv | Sort-Object Length -Descending | Select-Object -First 1).FullName
if (-not $mkv) { throw "no .mkv in $dest" }
"probing $mkv"

& $CAP record --out "$rec\mkvmerge.json" -- $MKVMERGE -J $mkv | Out-Null
& $CAP record --out "$rec\ffprobe-streams.json" -- $FFPROBE -v error -show_streams -show_format -of json $mkv | Out-Null
if ($MEDIAINFO) { & $CAP record --out "$rec\mediainfo.json" -- $MEDIAINFO --Output=JSON $mkv | Out-Null }

$ident = Get-Content "$rec\mkvmerge.json" -Raw | ConvertFrom-Json
$subs = @($ident.tracks | Where-Object { $_.type -eq 'subtitles' })
$evidenceSeconds = 0
$evidenceBytes = 0
if ($subs.Count -gt 0) {
    New-Item -ItemType Directory -Force "$rec\subs" | Out-Null
    $evidence = "$dest\evidence.mks"
    $ids = ($subs | ForEach-Object { $_.id }) -join ','
    $start = Get-Date
    & $MKVMERGE -q -o $evidence -D -A -s $ids $mkv | Out-Null
    $evidenceSeconds = [int]((Get-Date) - $start).TotalSeconds
    $evidenceBytes = (Get-Item $evidence).Length

    $index = @()
    for ($n = 0; $n -lt $subs.Count; $n++) {
        & $CAP record --out "$rec\subs\sub-$n.packets.json" -- $FFPROBE -v error -select_streams "s:$n" -show_entries 'packet=pts_time,duration_time,size' -of json $evidence | Out-Null
        $index += [ordered]@{
            n          = $n
            track_id   = $subs[$n].id
            language   = $subs[$n].properties.language
            codec      = $subs[$n].codec
            track_name = $subs[$n].properties.track_name
            forced     = $subs[$n].properties.forced_track
            default    = $subs[$n].properties.default_track
        }
    }
    ConvertTo-Json @($index) | Set-Content "$rec\subs\index.json"
}

# Time a copy-everything remux from the share back to the share (the file phase's cost, spec 6.4 step 13).
$remux = "$dest\remux-test.mkv"
$start = Get-Date
& $MKVMERGE -q -o $remux $mkv | Out-Null
$remuxSeconds = [int]((Get-Date) - $start).TotalSeconds
Remove-Item $remux -ErrorAction SilentlyContinue

$summary = [ordered]@{ file = $mkv; subtitle_tracks = $subs.Count; evidence_s = $evidenceSeconds; evidence_bytes = $evidenceBytes; remux_s = $remuxSeconds }
$summary | ConvertTo-Json | Set-Content "$rec\probe-summary.json"
$summary | ConvertTo-Json
```

- [ ] **Step 3: Rip the main feature with the all-tracks profile**

From Task 9's summary, pick the main feature's title number (the feature-length title; on a labelled disc, the one whose comment starts `FPL_MainFeature`). Use it in place of `N`.

```bash
scripts/box.sh
scripts/phase0/job.sh rip-title.ps1 rip-all-tracks -Slug SLUG -Title N -Label all-tracks
```

Wait with `scripts/phase0/wait.sh rip-all-tracks`; a UHD feature takes roughly an hour.
Expected: the log ends with the summary JSON, with one file name in `files`, then `PHASE0-DONE`.

- [ ] **Step 4: Probe the rip**

```bash
scripts/phase0/job.sh probe-title.ps1 probe-all-tracks -Slug SLUG -Label all-tracks
```

Wait with `scripts/phase0/wait.sh probe-all-tracks`, then:

```bash
scripts/phase0/pull-recording.sh SLUG
python3 scripts/phase0/compare_tracks.py corpus/owner/SLUG/scan.txt N corpus/owner/SLUG/all-tracks/mkvmerge.json
grep -c '^MSG:1009' corpus/owner/SLUG/all-tracks/rip.txt
grep -E '^MSG:(5003|5004|5005|5036|5037|2003|2008|2019)' corpus/owner/SLUG/all-tracks/rip.txt | cut -c1-160
cat corpus/owner/SLUG/all-tracks/rip-summary.json corpus/owner/SLUG/all-tracks/probe-summary.json
```

Expected: a side-by-side table and the lines `TRACK COUNT`, `ORDER MATCH`, `CHAPTERS`, `DURATION`, `VIDEO TRACKS`, `STATISTICS TAGS`; `0` profile errors; a success summary message.

- [ ] **Step 5: Write the experiment 1 report**

Create `docs/phase0/experiments/EXP-01-profile-and-order.md` with the table and these findings, each stated as observed:

1. Profile honoured: no `MSG:1009`, and the file's track count equals the scan's stream count (every stream except 3D video).
2. Order: `ORDER MATCH`.
3. Output codecs: for each row, whether the file's codec corresponds to the scan's `out=` value (attribute 41); list any LPCM or closed-caption rows and what they became.
4. Chapters: scan count against file count (MakeMKV's default profile has `insertFirstChapter00IfMissing="true"`).
5. Forced-only streams: from the table's `f=6144` rows, whether each directly follows a non-derived subtitle stream of the same language.
6. Dolby Vision: `VIDEO TRACKS`, and the video codec text in the scan.
7. Statistics tags: `STATISTICS TAGS`.

Answer yes only if 1 to 4 hold. Consequences: a no on 1 or 2 switches spec 9.3 to its fallback; 5 confirms or corrects the parent rule in spec 8.7; 6 fixes spec 8.10's wording; 7 decides whether counts can come from the header (spec 6.4 step 10).

- [ ] **Step 6: Run the share-drop test**

From the summary, pick a title of 5 to 20 minutes; use its number in place of `M`.

```bash
scripts/phase0/job.sh rip-title.ps1 rip-share-drop -Slug SLUG -Title M -Label share-drop
```

Ask the owner to disconnect the NAS from the network about one minute after the rip starts, wait one minute, and reconnect it. Wait with `scripts/phase0/wait.sh rip-share-drop`, pull the recording, and list the messages:

```bash
scripts/phase0/pull-recording.sh SLUG
grep -E '^MSG:' corpus/owner/SLUG/share-drop/rip.txt | cut -d, -f1,4 | sort | uniq -c
cat corpus/owner/SLUG/share-drop/rip.txt.exit.json
```

- [ ] **Step 7: Write the experiment 4 report**

Create `docs/phase0/experiments/EXP-04-rip-to-share.md` with: `elapsed_s`, `bytes`, `mb_per_s` and `min_free_ram_gb` of the all-tracks rip; whether any slow-write message appeared (`MSG:2008` or any message mentioning writing speed); `evidence_s`, `evidence_bytes` and `remux_s`; and the share-drop message sequence, how long `makemkvcon` took to exit, and its exit code.

Answer yes if free RAM stayed at 1 GB or more and no slow-write message appeared. If no, repeat Step 3 with `-CacheMb 512` (label `all-tracks-512`), then `256`, and report each. Consequences: the working `cache_mb` (spec section 14); the share-drop codes for the `Output` class (spec 10.2); the measured remux time replaces the estimate in spec section 18.

- [ ] **Step 8: Commit**

```bash
git add scripts/phase0/rip-title.ps1 scripts/phase0/probe-title.ps1 corpus/owner docs/phase0/experiments
git commit -m "Run experiments 1 and 4: all-tracks rip to the share"
```

---

### Task 12: Audio profiles and subtitle classification (experiment 6)

**Files:**
- Create: `docs/phase0/experiments/EXP-06-audio-profiles-and-subtitles.md`
- Create: recordings of two more discs under `corpus/owner/`

**Interfaces:**
- Consumes: `rip-title.ps1`, `probe-title.ps1`, `subtitle_alignment.py`, Disc A's `all-tracks` recording.
- Produces: the measured values for `subs_same_min`, `subs_diff_max` and `sub_clear_max_bytes` (spec 8.7, section 14), and the `ffprobe` profile strings for Atmos and DTS:X.

- [ ] **Step 1: Read the audio profiles of Disc A's rip**

```bash
python3 - <<'EOF'
import json
for name in ("ffprobe-streams.json", "mediainfo.json"):
    try:
        data = json.load(open(f"corpus/owner/SLUG/all-tracks/{name}", encoding="utf-8-sig"))
    except OSError:
        print(name, "absent"); continue
    if name.startswith("ffprobe"):
        for s in data["streams"]:
            if s["codec_type"] == "audio":
                print("ffprobe  ", s["index"], s["codec_name"], repr(s.get("profile")), s.get("channels"), s.get("channel_layout"), s.get("tags", {}).get("language"))
    else:
        for t in data["media"]["track"]:
            if t["@type"] == "Audio":
                print("mediainfo", t.get("ID"), t.get("Format"), repr(t.get("Format_Commercial_IfAny")), repr(t.get("Format_AdditionalFeatures")), t.get("Channels"))
EOF
```

Expected: one line per audio track. Compare with what the disc's packaging says (ask the owner which tracks are Atmos or DTS:X).

- [ ] **Step 2: Measure subtitle alignment on Disc A**

```bash
cat corpus/owner/SLUG/all-tracks/subs/index.json
python3 scripts/phase0/subtitle_alignment.py $(python3 -c "
import json
for s in json.load(open('corpus/owner/SLUG/all-tracks/subs/index.json', encoding='utf-8-sig')):
    print(f\"s{s['n']}-{s['language']}=corpus/owner/SLUG/all-tracks/subs/sub-{s['n']}.packets.json\")
")
```

Expected: packet-size buckets and event counts per stream, then the matrix. Note which streams the owner knows to be regular, SDH, commentary or forced-only (ask the owner, or play the staged file on the share).

- [ ] **Step 3: Repeat on two more discs**

Ask the owner for (a) a disc with a commentary subtitle track and (b) a DVD with closed captions. For each: record it (Task 8 procedure, full scan, via `run-interactive.ps1`), rip its main feature with `rip-title.ps1 -Label all-tracks`, run `probe-title.ps1`, pull, and run Steps 1 and 2 with that disc's slug. If Disc A already has a known commentary subtitle track, (a) is still needed: the spec asks for at least three discs.

- [ ] **Step 4: Choose the thresholds**

For every pair of full English tracks across the three discs, list the larger of the two alignment values and whether the pair is known to be the same content (regular and SDH) or different content (dialogue and commentary). The experiment succeeds if every same-content pair is at 0.70 or more and every different-content pair at 0.30 or less. If the two groups separate at other values, choose `subs_same_min` as the lowest same-content value rounded down to 0.05 and `subs_diff_max` as the highest different-content value rounded up to 0.05, provided the first is at least 0.2 above the second. Choose `sub_clear_max_bytes` from the packet-size buckets: the largest bucket limit below which packets are plainly "clear" packets (roughly half of all packets on PGS streams), or 0 if the streams have no such small packets.

- [ ] **Step 5: Write the report**

Create `docs/phase0/experiments/EXP-06-audio-profiles-and-subtitles.md` with the profile lines, the matrices, the pair table and the chosen values. Answer the two questions separately: does `ffprobe` tell Atmos and DTS:X apart (if not, does MediaInfo), and do the alignment values separate dialogue from commentary. Consequences: the detector named in spec 6.4 step 9; the three settings in spec section 14; or, on a no, the fallbacks in spec section 17 row 6.

- [ ] **Step 6: Commit**

```bash
git add corpus/owner docs/phase0/experiments
git commit -m "Run experiment 6: audio profiles and subtitle alignment"
```

---

### Task 13: Selection by language (experiment 11)

**Files:**
- Create: `docs/phase0/experiments/EXP-11-language-selection.md`

**Interfaces:**
- Consumes: Disc A in the drive, `eng-only.mmcp.xml` (Task 7), `rip-title.ps1`, `probe-title.ps1`, `compare_tracks.py --selection eng`.
- Produces: whether the later fast path (spec section 16, item 1) can be built.

The `eng-only` profile's selection is `-sel:all,+sel:video,+sel:(audio*(eng|nolang)),+sel:(subtitle*eng),-sel:mvcvideo,-sel:core,-sel:special,=100:all`. The prediction is: every video stream that is not 3D; every audio stream whose language is English or absent, except derived cores (flag 256) and flagged commentary or visually-impaired streams (flags 1, 2, 4); and every English subtitle stream.

- [ ] **Step 1: Rip the main feature with the language profile**

```bash
scripts/phase0/job.sh rip-title.ps1 rip-eng-only -Slug SLUG -Title N -Label eng-only -Profile C:\lazymkv\phase0\profiles\eng-only.mmcp.xml
```

Wait with `scripts/phase0/wait.sh rip-eng-only`.

- [ ] **Step 2: Identify the result and compare with the prediction**

```bash
scripts/phase0/job.sh probe-title.ps1 probe-eng-only -Slug SLUG -Label eng-only
```

Wait with `scripts/phase0/wait.sh probe-eng-only`, then:

```bash
scripts/phase0/pull-recording.sh SLUG
grep -c '^MSG:1009' corpus/owner/SLUG/eng-only/rip.txt
python3 scripts/phase0/compare_tracks.py corpus/owner/SLUG/scan.txt N corpus/owner/SLUG/eng-only/mkvmerge.json --selection eng
```

Expected: the table of predicted against actual tracks, and exit code 0 if they match in count and order.

- [ ] **Step 3: Write the report**

Create `docs/phase0/experiments/EXP-11-language-selection.md` with the table. List every predicted stream that is missing and every unpredicted stream that is present, with its language, codec and flags. Answer yes only if the two lists are empty and the order matched. Consequence: on yes, spec section 16 item 1 stands; on no, record which tokens behaved differently (for example whether `core` removed derived cores, and whether `nolang` kept streams with no language) and mark the fast path as not buildable with this selection string.

- [ ] **Step 4: Commit**

```bash
git add corpus/owner docs/phase0/experiments
git commit -m "Run experiment 11: selection by language"
```

---

### Task 14: Whole-disc copy and eject (experiments 3 and 7)

**Files:**
- Create: `scripts/phase0/backup-disc.ps1`
- Create: `docs/phase0/experiments/EXP-03-whole-disc-copy.md`, `docs/phase0/experiments/EXP-07-eject.md`

**Interfaces:**
- Consumes: Disc A in the drive, `compare_scans.py`, `compare_tracks.py`, `lazymkv-capture eject`.
- Produces: `backup-disc.ps1 -Slug <slug> [-SkipCopy] [-ExtractTitle <n>]`. Without `-SkipCopy` it copies the disc to `$Staging\<slug>\backup\` and writes `recordings\<slug>\backup\backup.txt` and `backup-summary.json` (`seconds`, `bytes`); it always scans the copy to `scan-file.txt`; with `-ExtractTitle` it extracts that title of the copy to `$Staging\<slug>\extract\` and writes `extract.txt` and `extract-mkvmerge.json`.

- [ ] **Step 1: Write the script**

`scripts/phase0/backup-disc.ps1`:

```powershell
# Experiment 3: decrypted whole-disc copy to the share, a scan of the copy, and an optional title extraction.
param(
    [Parameter(Mandatory)][string]$Slug,
    [switch]$SkipCopy,
    [int]$ExtractTitle = -1
)
. C:\lazymkv\src\scripts\phase0\env.ps1
$rec = "$P0\recordings\$Slug\backup"
$dest = "$Staging\$Slug\backup"
New-Item -ItemType Directory -Force $rec | Out-Null

if (-not $SkipCopy) {
    if (Test-Path $dest) { Remove-Item $dest -Recurse -Force }
    New-Item -ItemType Directory -Force $dest | Out-Null
    $index = Get-DiscIndex
    $start = Get-Date
    & $CAP record --out "$rec\backup.txt" -- $MK -r --messages=-stdout --progress=-same --cache=1024 backup --decrypt "disc:$index" $dest | Out-Null
    $seconds = [int]((Get-Date) - $start).TotalSeconds
    $bytes = (Get-ChildItem $dest -Recurse -File | Measure-Object Length -Sum).Sum
    [ordered]@{ seconds = $seconds; bytes = $bytes } | ConvertTo-Json | Set-Content "$rec\backup-summary.json"
    "copy: seconds=$seconds bytes=$bytes " + (Get-Content "$rec\backup.txt.exit.json")
}

$flags = Get-MkFlags
& $CAP record --out "$rec\scan-file.txt" -- $MK @flags info "file:$dest" | Out-Null
"scan of the copy: " + (Get-Content "$rec\scan-file.txt.exit.json")

if ($ExtractTitle -ge 0) {
    $extract = "$Staging\$Slug\extract"
    if (Test-Path $extract) { Remove-Item $extract -Recurse -Force }
    New-Item -ItemType Directory -Force $extract | Out-Null
    & $CAP record --out "$rec\extract.txt" -- $MK @flags mkv "file:$dest" $ExtractTitle $extract | Out-Null
    $mkv = (Get-ChildItem $extract -Filter *.mkv | Select-Object -First 1).FullName
    if ($mkv) { & $CAP record --out "$rec\extract-mkvmerge.json" -- $MKVMERGE -J $mkv | Out-Null }
    "extract: file=$mkv " + (Get-Content "$rec\extract.txt.exit.json")
}
```

- [ ] **Step 2: Copy Disc A and scan the copy**

Check first that the staging share has room for the whole disc (`total_bytes` in `corpus/owner/SLUG/meta.json`, plus the rips already there; the free space is in Task 7's setup check, which can be run again).

```bash
scripts/box.sh
scripts/phase0/job.sh backup-disc.ps1 backup -Slug SLUG
```

Wait with `scripts/phase0/wait.sh backup`; a UHD copy takes about as long as reading the whole disc.

```bash
scripts/phase0/pull-recording.sh SLUG
grep -E '^MSG:(5069|5070|5079|5010|2003)' corpus/owner/SLUG/backup/backup.txt | cut -c1-160
python3 scripts/phase0/compare_scans.py corpus/owner/SLUG/scan.txt corpus/owner/SLUG/backup/scan-file.txt
```

Expected: `MSG:5070` (copy completed). The comparison lists titles present in only one scan and titles whose comment (label) differs.

- [ ] **Step 3: Extract the main feature from the copy**

In `corpus/owner/SLUG/backup/scan-file.txt`, find the title whose source file (attribute 16) is the main feature's source file; use its number in place of `K` (it may differ from `N`).

```bash
python3 - <<'EOF'
import pathlib, sys
sys.path.insert(0, "scripts/phase0")
from robot import parse
scan = parse(pathlib.Path("corpus/owner/SLUG/backup/scan-file.txt").read_text(encoding="utf-8", errors="replace"))
for t, a in sorted(scan.titles.items()):
    print(t, a.get(16), a.get(9), a.get(49, ""))
EOF
scripts/phase0/job.sh backup-disc.ps1 extract -Slug SLUG -SkipCopy -ExtractTitle K
```

Wait with `scripts/phase0/wait.sh extract`, then:

```bash
scripts/phase0/pull-recording.sh SLUG
python3 scripts/phase0/compare_tracks.py corpus/owner/SLUG/backup/scan-file.txt K corpus/owner/SLUG/backup/extract-mkvmerge.json
```

Expected: `ORDER MATCH yes`, and the same track count and duration as the direct rip in Task 11.

- [ ] **Step 4: Write the experiment 3 report**

Create `docs/phase0/experiments/EXP-03-whole-disc-copy.md` with: the copy's time and size; its result message; the scan comparison (titles only in one scan, labels that differ); and the extraction comparison. Answer yes only if the copy completed, every disc-scan title is in the copy's scan by source file, segment map and duration, and the extracted feature matches the direct rip. Note separately whether main-feature labels survived in the copy's scan. Consequence if no: spec 6.3 is replaced by its fallback (spec section 17 row 3).

- [ ] **Step 5: Test eject on this drive**

```bash
ssh USER@makemkv-box "tasklist /fi \"imagename eq makemkvcon64.exe\" /nh & C:\lazymkv\target\release\lazymkv-capture.exe eject --drive D & echo exit=%errorlevel%"
```

Expected: no running `makemkvcon64.exe`, a line such as `lock=ok, dismount=ok, allow-removal=ok, eject=ok`, and `exit=0`. Ask the owner whether the tray opened.

- [ ] **Step 6: Test the second drive**

Ask the owner to swap to the other drive and insert any Blu-ray or UHD. Choose a slug for that disc and use it in place of `SLUG2`. Then:

```bash
ssh USER@makemkv-box "\"C:\Program Files (x86)\MakeMKV\makemkvcon64.exe\" -r --cache=1 info disc:9999" | grep '^DRV:0'
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\record-disc.ps1 -Slug SLUG2 -NoScan"
ssh USER@makemkv-box "C:\lazymkv\target\release\lazymkv-capture.exe eject --drive D & echo exit=%errorlevel%"
```

Expected: the `DRV` line names the second drive's model; the recording's `disc:` line has a `disc_id`; the eject report and `exit=0`. If the drive letter is not `D`, update `$Drive` in `C:\lazymkv\phase0\config.ps1` and the `--drive` argument first.

- [ ] **Step 7: Write the experiment 7 report and complete experiment 5**

Create `docs/phase0/experiments/EXP-07-eject.md` with each drive's model, the step results and whether the tray opened. Answer yes only if both trays opened. Add to `EXP-05-offline-and-unit-key.md` whether `disc_id` was read in the second drive. Add both drive models to `docs/phase0/environment.md`.

- [ ] **Step 8: Commit**

```bash
git add scripts/phase0/backup-disc.ps1 corpus/owner docs/phase0
git commit -m "Run experiments 3 and 7: whole-disc copy and eject"
```

---

### Task 15: The remaining discs (experiments 8 and 10, and repeats)

**Files:**
- Create: recordings under `corpus/owner/`
- Create: `docs/phase0/experiments/EXP-08-encoding.md`, `docs/phase0/experiments/EXP-10-thediscdb-hit-rate.md`
- Modify: `docs/phase0/experiments/EXP-02-index-guard.md`, `docs/phase0/experiments/EXP-12-java-labels.md`

**Interfaces:**
- Consumes: `record-disc.ps1`, `exp02-index-guard.ps1`, `summarize_scan.py`, `check_encoding.py`, `hit_rate.py`.
- Produces: full recordings (with scan) of the discs listed below, and fingerprint-only recordings of the rest of about 20 discs.

- [ ] **Step 1: Record these discs with a full scan**

For each disc the owner has, in this order: a disc with decoy playlists; a two-cut disc; a TV disc; a DVD with a commentary, closed captions and forced subtitles; a non-English film; a disc whose title has non-ASCII characters; a UHD combo pack's Blu-ray. One disc may cover several items. For each:

```bash
scripts/phase0/job.sh record-disc.ps1 record-SLUG -Slug SLUG
```

Wait with `scripts/phase0/wait.sh record-SLUG`, then:

```bash
scripts/phase0/pull-recording.sh SLUG
python3 scripts/phase0/summarize_scan.py corpus/owner/SLUG
```

Record in `corpus/owner/SLUG/notes.md` what the owner says the disc is (film or show, cuts, which tracks are commentary), so the recording has ground truth.

- [ ] **Step 2: Repeat experiment 2 on the decoy-playlist disc**

Run Task 10 Steps 2 and 3 with that disc's slug, and add the result to `EXP-02-index-guard.md`. If it has `3308` or `3309` lines, the experiment's condition is now met; state the final answer.

- [ ] **Step 3: Check the DVD's flags**

```bash
python3 - <<'EOF'
import pathlib, sys
sys.path.insert(0, "scripts/phase0")
from robot import parse
scan = parse(pathlib.Path("corpus/owner/SLUG/scan.txt").read_text(encoding="utf-8", errors="replace"))
for (t, s), a in sorted(scan.streams.items()):
    flags = int(a.get(22, "0") or 0)
    if a.get(1) in ("Audio", "Subtitles") and (flags or a.get(42)):
        print(t, s, a.get(1), a.get(3), a.get(6), "flags=", flags, "conversion=", a.get(42, ""), a.get(30, ""))
EOF
```

Add to `corpus/owner/SLUG/notes.md` which streams carry flags 1, 2, 4 or 4096 and how the closed-caption stream appears (its codec and conversion text). These settle the two items spec 8.7 marks as unverified on DVD.

- [ ] **Step 4: Experiment 8, output encoding**

```bash
python3 scripts/phase0/check_encoding.py corpus/owner/SLUG/scan.txt
```

Run it on the non-ASCII disc's scan and on every other scan recorded. Create `docs/phase0/experiments/EXP-08-encoding.md` with the output. Answer: "robot output is UTF-8" (yes) or name the encoding observed. Record which quote escapes appear. If the owner has no disc with a non-ASCII title, report the experiment as not run.

- [ ] **Step 5: Experiment 10, TheDiscDB hit rate**

For each further disc, until about 20 discs are recorded in all, fingerprint only (about half a minute each):

```bash
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\record-disc.ps1 -Slug SLUG -NoScan"
scripts/phase0/pull-recording.sh SLUG
```

Then:

```bash
python3 scripts/phase0/hit_rate.py corpus/owner
```

Create `docs/phase0/experiments/EXP-10-thediscdb-hit-rate.md` with the table, the share of discs known, and the split by format. State the number of discs; if fewer than 20 were available, say so.

- [ ] **Step 6: Update experiment 12 if a later disc answered it**

If Disc A showed no Java labels, add the first disc that did (or the fact that none did) to `EXP-12-java-labels.md`.

- [ ] **Step 7: Commit**

```bash
git add corpus/owner docs/phase0/experiments
git commit -m "Record the remaining discs; run experiments 8 and 10"
```

---

### Task 16: Naming on the media servers (experiment 9)

**Files:**
- Create: `scripts/phase0/make-test-library.ps1`, `docs/phase0/experiments/EXP-09-naming.md`

**Interfaces:**
- Consumes: `$TestLibrary` in `C:\lazymkv\phase0\config.ps1`, `$FFMPEG`.
- Produces: five short generated video files, named as spec 8.9 prescribes, in the test library folder.

- [ ] **Step 1: Confirm the test library folder**

Confirm with the owner that `$TestLibrary` in `C:\lazymkv\phase0\config.ps1` is the empty folder they chose in Task 7 and that it is not inside a real library. If it is still empty or an example path, write the owner's path into the config file now.

- [ ] **Step 2: Write the script**

`scripts/phase0/make-test-library.ps1`:

```powershell
# Experiment 9: generated files named as spec 8.9 prescribes, for the owner to add to Plex and Jellyfin.
. C:\lazymkv\src\scripts\phase0\env.ps1
if (-not $TestLibrary) { throw 'Set $TestLibrary in C:\lazymkv\phase0\config.ps1 first.' }
$tmp = "$P0\tmp\exp09"
New-Item -ItemType Directory -Force $tmp | Out-Null

& $FFMPEG -v error -y -f lavfi -i 'testsrc2=size=3840x2160:rate=24' -f lavfi -i 'sine=frequency=440:sample_rate=48000' -t 8 -c:v libx264 -preset ultrafast -pix_fmt yuv420p -c:a aac "$tmp\2160.mkv"
& $FFMPEG -v error -y -f lavfi -i 'testsrc2=size=1920x1080:rate=24' -f lavfi -i 'sine=frequency=440:sample_rate=48000' -t 8 -c:v libx264 -preset ultrafast -pix_fmt yuv420p -c:a aac "$tmp\1080.mkv"

function Place([string]$Source, [string]$Relative) {
    $destination = Join-Path $TestLibrary $Relative
    New-Item -ItemType Directory -Force (Split-Path $destination) | Out-Null
    Copy-Item -LiteralPath $Source -Destination $destination -Force
    $destination
}

# A lone file with a resolution label.
Place "$tmp\2160.mkv" 'Alien (1979) {tmdb-348}\Alien (1979) {tmdb-348} - 2160p.mkv'
# Two resolutions of one film: versions (D11).
Place "$tmp\2160.mkv" 'Inception (2010) {tmdb-27205}\Inception (2010) {tmdb-27205} - 2160p.mkv'
Place "$tmp\1080.mkv" 'Inception (2010) {tmdb-27205}\Inception (2010) {tmdb-27205} - 1080p.mkv'
# Two cuts in the "combined" edition style.
Place "$tmp\2160.mkv" 'Blade Runner (1982) {tmdb-78}\Blade Runner (1982) {tmdb-78} - 2160p Theatrical Cut {edition-Theatrical Cut}.mkv'
Place "$tmp\2160.mkv" 'Blade Runner (1982) {tmdb-78}\Blade Runner (1982) {tmdb-78} - 2160p Final Cut {edition-Final Cut}.mkv'
```

- [ ] **Step 3: Run it**

```bash
scripts/box.sh
scripts/phase0/job.sh make-test-library.ps1 exp09
```

Wait with `scripts/phase0/wait.sh exp09`.
Expected: five paths printed, then `PHASE0-DONE`.

- [ ] **Step 4: Ask the owner to check both servers**

Ask the owner to add the folder as a movie library in Plex and in Jellyfin, let both scan it, and answer for each server:

1. Does "Alien" appear once, with the right poster and title?
2. Does "Inception" appear once, with two versions to choose from (2160p and 1080p)?
3. Does "Blade Runner" appear once with two editions or versions, named "Theatrical Cut" and "Final Cut"? If not, how does it appear?
4. Which versions of Plex and Jellyfin are these, and does the Plex account have Plex Pass?

- [ ] **Step 5: Write the report**

Create `docs/phase0/experiments/EXP-09-naming.md` with the five paths and the owner's answers per server. Answer yes only if 1 to 3 hold on both servers. Consequences: on a no for 3, the default `edition_style` becomes `plex` or `jellyfin` as spec 8.9 says, and the script is run again with that style's layout to confirm; on a no for 1 or 2, stop and return to the owner, as spec section 17 row 9 says. Record the server versions in spec section 19's open item.

- [ ] **Step 6: Commit**

```bash
git add scripts/phase0/make-test-library.ps1 docs/phase0/experiments
git commit -m "Run experiment 9: naming on the media servers"
```

---

### Task 17: Replay check, results into the spec, and clean-up

**Files:**
- Create: `corpus/scenarios/disc-a-scan/makemkvcon64.json`, `docs/phase0/REPORT.md`
- Modify: `docs/superpowers/specs/2026-10-04-lazymkv-design.md`

**Interfaces:**
- Consumes: every experiment report, Disc A's recording, `fake-tool`.
- Produces: a spec that states measured facts where it stated assumptions, and a summary report. Phase 1's plan is written against this spec.

- [ ] **Step 1: Replay a real recording through the stand-in**

Create the scenario folder with copies of Disc A's scan and timing:

```bash
mkdir -p corpus/scenarios/disc-a-scan
cp corpus/owner/SLUG/scan.txt corpus/scenarios/disc-a-scan/scan.txt
cp corpus/owner/SLUG/scan.txt.timing.jsonl corpus/scenarios/disc-a-scan/scan.txt.timing.jsonl
cat > corpus/scenarios/disc-a-scan/makemkvcon64.json <<'EOF'
{"responses": [
  {"args_contain": ["info", "disc:0"], "stdout": "scan.txt", "timing": "scan.txt.timing.jsonl"}
]}
EOF
scripts/box.sh cargo build --release -p fake-tools
ssh USER@makemkv-box "copy /y C:\lazymkv\target\release\fake-tool.exe C:\lazymkv\phase0\tmp\makemkvcon64.exe >nul & set LAZYMKV_FAKE_DIR=C:\lazymkv\src\corpus\scenarios\disc-a-scan& C:\lazymkv\phase0\tmp\makemkvcon64.exe -r info disc:0 > C:\lazymkv\phase0\tmp\replayed.txt & fc /b C:\lazymkv\phase0\tmp\replayed.txt C:\lazymkv\src\corpus\scenarios\disc-a-scan\scan.txt"
```

Expected: `FC: no differences encountered`. The stand-in reproduces a real scan byte for byte.

- [ ] **Step 2: Update the spec from the reports**

For each experiment report, apply its "Consequence for the spec" to `docs/superpowers/specs/2026-10-04-lazymkv-design.md`. At minimum:

- Section 3: MakeMKV version now installed; both drive models.
- Section 4.4: the installed tool versions.
- Section 6.2 step 6 and section 17 row 2: the index guard stays, or is removed per its fallback.
- Section 6.3 and section 17 row 3: whole-disc copy stays, or is replaced per its fallback.
- Section 6.4 steps 9 and 10: the detector for Atmos and DTS:X; where subtitle counts come from.
- Section 8.1: replace the sentence about fields "mapped from responses saved in phase 0" with the actual JSON field names, read from `corpus/thediscdb-api/` and the owner's recordings.
- Section 8.7 and section 14: `subs_same_min`, `subs_diff_max`, `sub_clear_max_bytes` as measured; the forced-stream parent rule; the DVD flag and closed-caption findings.
- Section 8.9: the working `edition_style` default; the owner's server versions and Plex Pass.
- Section 8.10: how a Dolby Vision title's video tracks appear.
- Section 9.3: the job profile as verified; chapter-count behaviour.
- Section 9.4: the output encoding and quote escapes; value formats confirmed on MakeMKV 2.0.0; any message code seen that the table lacks.
- Section 10.2: message codes for `NeedsNetwork`, share loss and any other class the experiments exercised.
- Section 14: `cache_mb`.
- Section 17: mark each row with its result and the report's file name.
- Section 18: replace the unmeasured estimates (remux time, network cost) with measurements.
- Section 5: note that the DVD disc ID is not yet implemented.

Change only what a report supports. Where an experiment was not run, the spec keeps its assumption and says "not yet verified".

- [ ] **Step 3: Write the phase report**

`docs/phase0/REPORT.md`: a table of the twelve experiments (question, answer, report file), the list of spec sections changed, the discs recorded (slug, format, what it covers), what could not be done and why, and the installs made on the mini PC.

- [ ] **Step 4: Run every test once more**

```bash
scripts/box.sh cargo test --workspace
python3 -m unittest discover -s scripts/phase0 -p 'test_*.py'
```

Expected: all Rust tests pass (8 in `lazymkv-core` plus the corpus test, 5 in `lazymkv-capture`, 3 in `fake-tools`) and `Ran 8 tests ... OK`.

- [ ] **Step 5: Clean up, with the owner's agreement**

List what the experiments left on the share and the mini PC, and ask the owner which to delete:

```bash
scripts/phase0/job.sh setup-check.ps1 final-check
ssh USER@makemkv-box "dir /s /b C:\lazymkv\phase0\tmp & schtasks /query /tn lazymkv-phase0 /fo list | findstr TaskName"
```

The staging folders `<staging>\<slug>\` (rips, the whole-disc copy, extractions), the test library folder, the scheduled task `lazymkv-phase0` and `C:\lazymkv\phase0\tmp` are all created by this plan. Delete only what the owner agrees to; the rips may be wanted as inputs for phase 2. Confirm the firewall rule `lazymkv-exp05` is gone.

- [ ] **Step 6: Commit**

```bash
git add corpus/scenarios docs/phase0/REPORT.md docs/superpowers/specs/2026-10-04-lazymkv-design.md
git commit -m "Fold phase 0 results into the spec; add the phase report"
```
