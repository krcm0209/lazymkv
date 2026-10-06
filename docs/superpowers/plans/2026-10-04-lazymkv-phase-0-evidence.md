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
- A desktop-session job is started with `scripts/phase0/job.sh <script.ps1> <log name> [arguments]` and awaited with `scripts/phase0/wait.sh <log name>` (both written in Task 7). `wait.sh` exits 0 when the job finished (it prints the log, which ends in `PHASE0-DONE status=ok exit=N`), 1 when the job's script threw (`status=error`, with an `ERROR:` line), 3 after about nine minutes when the job is still running (run it in the background and run it again), 4 when the job is not running and wrote no `PHASE0-DONE` line, and 5 when the mini PC cannot be reached. `exit=N` is only the exit code of the last native command the script ran, not a verdict. `run-interactive.ps1` refuses to start a job while another is running. Script arguments must not contain quotes.
- Never pipe `lazymkv-capture record` into `Select-Object -First`: PowerShell then stops the recorded process early. Pipe it to `Out-Null`.
- Text printed by MakeMKV is never used to build a path. Output files are found by listing a directory that was empty before the call.
- Secrets never enter the repository: no TMDb token, no share password, no `Unit_Key_RO.inf` contents (only its SHA-1).
- The agent must not fetch pages from dvdcompare.net (its robots.txt blocks AI agents).
- Nothing is written to the owner's media library roots. Experiment 9 uses a separate test library folder.
- Every experiment report states what was run, what was observed, and "yes" or "no" to the experiment's question, using `docs/phase0/experiments/TEMPLATE.md`. A step that could not be run is reported as not run, with the reason.
- Commit after every task. End commit messages with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
- Tasks 8 to 17 were revised on 2026-10-04, before any disc session, after a pre-flight audit confirmed 49 defects in their steps. The experiments, their questions and their order are unchanged. The lines below are the conventions the revised tasks share.
- The network share is reachable only from a script started with `scripts/phase0/job.sh`; a plain `ssh` command gets "Access is denied" there. Rips, copies, extractions and remuxes are written to `$Staging`, never to `C:`.
- `env.ps1` provides `$Recordings`, `Assert-Slug`, `Assert-EmptyDir`, `Get-InsertedDisc` and `Assert-DiscIs`. Every name that becomes part of a path is checked with `Assert-Slug` (lowercase letters, digits and hyphens); every script that works on "the disc of recording <slug>" calls `Assert-DiscIs` before its first MakeMKV call; every folder MakeMKV writes into is checked with `Assert-EmptyDir` first.
- Evidence is never silently replaced: a script refuses to overwrite an existing scan, log or summary unless started with `-Force`, and the task says when `-Force` is right. `scripts/phase0/pull-recording.sh` never deletes anything on the Mac.
- A script that drives a rip, copy, extraction or remux decides itself whether it worked, prints `RESULT ok` or `RESULT FAILED: <reason>`, and throws on failure, so `wait.sh` exits 1.
- A job script never sets `$ErrorActionPreference = 'Stop'` globally (inside a job that turns any line a native tool writes to stderr into a failure). It uses `-ErrorAction Stop` on individual cmdlets and checks `$LASTEXITCODE`.
- Subtitle text (decision D16; from Task 12 Step 31 on): every rip that ends `RESULT ok` and whose `subs/index.json` lists an English or no-language bitmap subtitle stream (`HDMV PGS` or `VobSub`) is read with `scripts/phase0/job.sh ocr-subs.ps1 ocr-<slug>-<label> -Slug <slug> -Label <label>` and its `wait.sh` after its probe and pull, before the audio tuning-dataset jobs and before it leaves staging; then `scripts/phase0/pull-recording.sh <slug>`. ocr-subs.ps1 writes the text, its example lines, the frames and a copy of evidence.mks to `$TuningData\<slug>\<label>\subs-ocr\` only; the recording gets counts, never text. A rip is not deleted from staging before this reading ended `RESULT ok` or was written down as failed with its reason (Task 12 Step 31).
- A job script parameter that takes a list is declared `[string[]]` (or `[int[]]` for numbers): `job.sh` hands the arguments to `run-interactive.ps1` as one `ArgLine`, which is evaluated inside `-Command`, where `eng,und` is an array literal, so a `[string]` parameter would receive `eng und`.
- Evidence files written by PowerShell are UTF-8 (`-Encoding UTF8`); Python reads them with `encoding="utf-8-sig"`.
- More than a trivial command on the mini PC is run as a `.ps1` file copied with `scp` and started with `powershell -NoProfile -ExecutionPolicy Bypass -File`; `C:\lazymkv\phase0\config.ps1` is changed by writing the whole file on the Mac and copying it over.
- Commit commands take the form `git commit -m "<message>" -m "Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"`.
- The saving message is `MSG:5014`, or `MSG:5015` when a profile is in use. Found on 2026-10-05, on the first real rip: with `--profile`, MakeMKV 2.0.0 prints `MSG:5015` ("Saving %1 titles into directory %2 using profile '%3' from file '%4'") and no `MSG:5014`. Every rip in this plan uses a profile. Wherever a task says `MSG:5014`, read "`MSG:5014` or `MSG:5015`"; the scripts accept both, and their labels (`5014=`, `saw_5014`, `stopped-at-5014`, `added-before-5014`) keep their names and are true for either code. A `codes=` list then ends in 5015. The profile named in the `MSG:5015` line is the proof that MakeMKV used the job profile.
- Decision D16 (owner, 2026-10-05): every MakeMKV call runs with `--debug` (`Get-MkFlags` adds it) and its debug log, which MakeMKV 2.0.0 always writes to `%USERPROFILE%\MakeMKV_log.txt`, is kept beside the call's output as `<output>.makemkv-debug.txt` by `Save-MkDebugLog`. With `--debug` the robot output also carries `MSG:1004` and `MSG:1003` lines. Every rip also keeps `system.csv` (CPU load, free RAM and bytes written every 5 s) and `system-events.json` (Windows warnings and errors about storage, USB and the network during the rip). A script that calls MakeMKV without `Get-MkFlags` adds `--debug` and calls `Save-MkDebugLog` itself.
- Tuning dataset (decision D16): every rip that ends `RESULT ok` is added to the audio tuning dataset after its probe and pull, while the rip is still in staging, and at the latest before its task's commit. `python3 scripts/phase0/audio_dataset.py args corpus/owner/<slug> <label>` prints one job line and its `wait.sh` line for the title's programme reference against every other stream in a wanted language (or the reference alone), then one per language that is not wanted and has two or more streams (dataset only). Run each pair, then `scripts/phase0/pull-recording.sh <slug>`, `python3 scripts/phase0/audio_labels.py auto corpus/owner/<slug>` and `python3 scripts/phase0/audio_dataset.py`. audio-levels.ps1 writes, besides its level folder, a compact copy of every audio stream (`audio-proxy.mka`: FLAC, 8 kHz, mono, 24 bit) and a copy of the level folder to `$TuningData\<slug>\<label>\` (`\\unas-pro\media\lazymkv-tuning`); it writes `audio-proxy.json` into the level folder last, so a level folder without it is from a run that did not finish. The harness's last list names every successful rip not yet in the dataset. A rip in staging is deleted only once that list no longer names it. Nothing under `$TuningData` is deleted without the owner's say. The one exception to writing large files only to the share (never to `C:`): a proxy may be copied to C:\lazymkv\phase0\tmp\tuning-fetch for the minutes an scp takes, at most 2 GB, deleted right after. `scripts/phase0/tuning-fetch.ps1 -Slug <slug> -Label <label>` (a job) makes that copy of one title's dataset folder, refuses more than 2 GB or a second copy while one is still there, and prints the size, the `scp` command and the clean-up command; the Mac runs the clean-up command immediately after the `scp`. It is needed only when a future measure needs the proxies on the Mac, never in this routine. The owner's label for each measured track is asked for without showing any verdict and is recorded only with `audio_labels.py set` or `unset` (never by editing the JSON); whether a label counts for experiment 13 is worked out by the harness, never stored. `audio_dataset.py` exits 1 when a track the owner labelled as programme audio gets a drop verdict, when a level folder is measured against the wrong reference, when a reference is labelled as unwanted audio, when a labelled programme track gets `commentary-cross-language`, or when a labelled candidate, taken as the reference, would give the programme the commentary verdict (`swapped reference`) (stop and report); 2 when a labels file does not validate; 3 when something could not be checked (level files missing, a label with no measurement, a measured track gone).
- MakeMKV removes a forced-only subtitle track that turns out to be empty and says so with `MSG:4047` ("Forced subtitles track #N turned out to be empty and was removed from output file", after `MSG:4007`). Found on the same rip. N counts the title's SELECTED streams in scan order (the output file's tracks before the removal), not the scan's stream numbers: on Disc A's eng-only rip "#3" was scan stream 5 (Task 13 Step 6, 2026-10-06); compare_tracks.py maps it accordingly. A ripped file can therefore hold fewer tracks than the scan lists; such a difference is explained, not a failure, when a `MSG:4047` line names the missing stream.

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
        // The echo is a convenience: a reader that closes the pipe must not stop the recording.
        let _ = echo.write_all(&line);
        let _ = echo.flush();
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

Expected: a `makemkvcon` line containing `MakeMKV v2.0.0`; versions for `mkvmerge` (54 or later) and `ffprobe` (6.1 or later); `drive : D: is MakeMKV disc index 0`; `profiles : all-tracks=True eng-only=True`; `staging ... write, rename and delete OK`; and a final line `PHASE0-DONE status=ok exit=`. If the staging line says FAILED, ask the owner to open the share in File Explorer with "remember my credentials" and run this step again. If the log is empty, the owner is not logged in at the console: ask them to log in.

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
- Modify: `scripts/phase0/record-disc.ps1`, `scripts/phase0/pull-recording.sh`, `corpus/owner/README.md`. Their final content is in Steps 1 and 2; the controller applies and tests it as a prerequisite before any disc session.
- Create: `corpus/owner/<slug>/`, one folder for each disc recorded in Steps 4 to 7 (never Disc A).
- Modify: `docs/phase0/environment.md` (new section `## Fingerprint check (Task 8)`).

**Interfaces:**
- Consumes: from `env.ps1`: `$P0`, `$Recordings`, `$CAP`, `$MK`, `$Drive`, `$AllTracksProfile`, `Get-MkFlags`, `Get-DiscIndex`, `Assert-Slug`, `Assert-DiscIs`, `Assert-EmptyDir`, `Get-InsertedDisc` (prerequisite item 4). From the capture tool: `lazymkv-capture disc --root <path> --label <text> --out <dir>` (exit 0, or 2 on failure) and `lazymkv-capture record --out <file> -- <command...>`. Scripts: `scripts/box.sh`, `scripts/phase0/job.sh`, `scripts/phase0/wait.sh`, `scripts/phase0/summarize_scan.py <recording dir>`, `scripts/phase0/hit_rate.py <corpus dir>`.
- Produces:
  - `record-disc.ps1 -Slug <slug> [-NoScan] [-ScanName <name>] [-ScanMayFail] [-Force]`. Run it as a job (`scripts/phase0/job.sh record-disc.ps1 <log> -Slug <slug> ...`) when it scans, or over plain ssh with `-NoScan`. It writes `$Recordings\<slug>\`, which is `C:\lazymkv\phase0\recordings\<slug>\`:
    - A new slug (no `meta.json` yet) gets a snapshot and lookups:
      - from `lazymkv-capture disc`: `meta.json` (`volume_label`, `format`, `fingerprint`, `disc_id`, `file_count`, `total_bytes`, `captured_unix`), `files.json` and `nav\`;
      - `thediscdb-hash.json`, always requested;
      - `thediscdb-discid.json`, when the disc has a disc ID;
      - `tmdb.json`, when the dischash answer names a TMDb ID and `TMDB_TOKEN` is set;
      - `lookup-status.txt`: UTF-8 with a BOM, one line per lookup made: `dischash <code>`, `discid <code>`, `tmdb <movie|tv>/<id> <code>`. The code `000` means no answer.
    - A slug that is already recorded is only ever extended with the same disc. `Assert-DiscIs <slug>` runs first and refuses any other disc, even with `-Force`. Its snapshot and lookups are kept; `-Force` replaces them with new ones of the same disc.
    - Unless `-NoScan`, the script also runs MakeMKV's scan `info disc:<index>` with `Get-MkFlags` (the all-tracks profile). The output goes to `<ScanName>.txt`, with `.timing.jsonl`, `.stderr` and `.exit.json` beside it. `-ScanName` defaults to `scan` and must pass Assert-Slug.
      - If any of those four files already exists, the run is refused before anything else happens. `-Force` overrides that and deletes the four files just before the new scan.
      - The scan passes when `.exit.json` was written, the output contains MakeMKV's `MSG:1005` version banner, and its `TCOUNT` is above 0.
      - With `-ScanMayFail` only the first two are required. This is for a scan whose failure is itself the result, such as Task 9's offline scan.
      - MakeMKV's exit code is printed but never judged (spec 9.2).
    - Output lines: `disc: ...`, `lookups: ...`, `scan: <ScanName>.txt exit.json=<json or MISSING> version_banner=<True|False> titles=<n or NO-TCOUNT-LINE>`. The last line is `RESULT ok` (with a note when `-ScanMayFail` accepted a scan that has no titles) or `RESULT FAILED: <reason>`. On failure the script then throws, so wait.sh exits 1 and an ssh run exits 1.
    - The script refuses before writing anything to the recording when:
      - a name is bad;
      - `$CAP` or the job profile is missing;
      - the scan files already exist;
      - no disc is readable in `$Drive` within 120 s;
      - the slug already belongs to another disc.
    - It fails without changing the recording when `lazymkv-capture disc` fails or the disc has no fingerprint.
    - `-Force` is right only where a plan step says so. It is never right for a first-sight scan such as Task 9's `scan-offline`.
  - `scripts/phase0/pull-recording.sh <slug>` copies `C:\lazymkv\phase0\recordings\<slug>` over `corpus/owner/<slug>/` on the Mac:
    - same-named files are replaced, and files that exist only on the Mac are kept;
    - nothing in `corpus/owner` is ever deleted;
    - a failed copy changes nothing;
    - the exit code is 2 for a bad slug, and 1 when the copy fails or the recording has no `meta.json`.
  - Recordings of one to five of the owner's discs (never Disc A). The first of them includes a scan made by a rehearsal job.
  - The fingerprint check's outcome, in `docs/phase0/environment.md` under `## Fingerprint check (Task 8)`.

Placeholders:
- SLUG: the slug of the disc being recorded. It is chosen in Step 4 from what the owner says the disc is (title, year, format), using lowercase letters, digits and hyphens only, for example `heat-1995-bluray`. Every disc gets a new slug. In Step 7, SLUG is the disc recorded last in Step 4.
- FIRST_SLUG: the slug of the first disc recorded in this task. Steps 5 and 6 run only for it.
- In the environment.md section of Step 7:
  - FORMAT: `meta.json`'s `format`;
  - DISCID_STATE: one of `yes`, `none: Unit_Key_RO.inf listed but unreadable`, `none: Unit_Key_RO.inf not listed`, `n/a (DVD)`, from Step 4;
  - HASH_RESULT and DISCID_RESULT: `hit_rate.py`'s `hash=` and `discid=` values for the disc;
  - CH_RESULT: `MATCH`, `DIFFERENT` or `none`, from the `thediscdb-discid.json` lines of the check in Step 7;
  - GID_RESULT: `MATCH`, `DIFFERENT` or `not compared`, from the `thediscdb-hash.json` line;
  - OUTCOME: one of the three sentences given in Step 7;
  - LABEL_ANSWER and RO_ANSWER: from Step 5;
  - REHEARSAL_ANSWER: from Step 6.
- DISC_A: the owner's title and year for Disc A, asked at the start of Step 4. No disc of that film, in any format, is recorded in this task.
- Disc A: the UHD film that MakeMKV has never scanned on this machine (DISC_A), kept for Task 9. It must not be inserted at any point in this task.

- [ ] **Step 1: The recording script**

Which disc is in the drive: none needed.

`scripts/phase0/record-disc.ps1` must have exactly this content. The controller applies it as a prerequisite before any disc session; if the file differs, replace it and run the prerequisite test again.

```powershell
# Records what can be learned about the disc in $Drive without ripping it, in $Recordings\<Slug>\
# (C:\lazymkv\phase0\recordings\<Slug>\):
#   meta.json, files.json, nav\     from lazymkv-capture disc (navigation-file copies; never Unit_Key_RO.inf)
#   thediscdb-hash.json, thediscdb-discid.json, tmdb.json, lookup-status.txt     the lookups
#   <ScanName>.txt with .timing.jsonl, .stderr and .exit.json     MakeMKV's scan (unless -NoScan)
# A new slug gets the snapshot and the lookups. An existing recording is only ever extended with the
# same disc (Assert-DiscIs); its snapshot and lookups are kept, or replaced by new ones with -Force.
# An existing scan file is refused unless -Force is given. -ScanMayFail is for a scan whose failure is
# itself the result (Task 9's offline scan): the scan then only has to prove that MakeMKV ran.
# The last line is "RESULT ok" or "RESULT FAILED: <reason>"; after a failure the script throws.
param(
    [Parameter(Mandatory)][string]$Slug,
    [switch]$NoScan,
    [string]$ScanName = 'scan',
    [switch]$ScanMayFail,
    [switch]$Force
)
. C:\lazymkv\src\scripts\phase0\env.ps1

try {
    Assert-Slug $Slug
    Assert-Slug $ScanName
    $note = ''
    $out = "$Recordings\$Slug"
    $stage = "$P0\tmp\record-$Slug"
    $scan = "$out\$ScanName.txt"
    $scanFiles = @($scan, "$scan.stderr", "$scan.timing.jsonl", "$scan.exit.json", "$scan.makemkv-debug.txt")

    # Refuse before anything is created, deleted or started.
    if (-not (Test-Path -LiteralPath $CAP)) { throw "missing $CAP; build it with: scripts/box.sh cargo build --release -p lazymkv-capture" }
    if (-not $NoScan) {
        if (-not (Test-Path -LiteralPath $AllTracksProfile)) { throw "missing job profile $AllTracksProfile" }
        foreach ($file in $scanFiles) {
            if ((Test-Path -LiteralPath $file) -and -not $Force) {
                throw "$file already exists and is kept as evidence; choose another -ScanName, or use -Force only where the plan says so"
            }
        }
    }
    $recorded = Test-Path -LiteralPath "$out\meta.json"
    if (-not $recorded -and @(Get-ChildItem -LiteralPath $out -Filter '*.exit.json' -ErrorAction SilentlyContinue).Count -gt 0) {
        throw "$out holds recorded runs but no meta.json (an earlier -Force run stopped part-way); put its meta.json back before running again"
    }

    # The disc may still be spinning up: wait up to two minutes until its root can be listed.
    $ready = $false
    for ($i = 0; $i -lt 24 -and -not $ready; $i++) {
        if (@(Get-ChildItem -LiteralPath "${Drive}:\" -ErrorAction SilentlyContinue).Count -gt 0) { $ready = $true } else { Start-Sleep -Seconds 5 }
    }
    if (-not $ready) { throw "no readable disc in ${Drive}: after 120 s" }

    # An existing recording is only ever extended with the same disc.
    if ($recorded) { Assert-DiscIs $Slug }

    if (-not $recorded -or $Force) {
        # Snapshot and lookups go to an empty local folder first, and replace the recording's files only when complete.
        if (Test-Path -LiteralPath $stage) { Remove-Item -LiteralPath $stage -Recurse -Force -ErrorAction Stop }
        Assert-EmptyDir $stage
        $label = $null
        try { $label = ([System.IO.DriveInfo]::new($Drive)).VolumeLabel } catch { }
        if (-not $label) { $label = '(none)' }
        & $CAP disc --root "${Drive}:\" --label $label --out $stage | Out-Null
        $capExit = $LASTEXITCODE
        if ($capExit -ne 0 -or -not (Test-Path -LiteralPath "$stage\meta.json")) { throw "lazymkv-capture disc failed (exit $capExit)" }
        $meta = Get-Content -LiteralPath "$stage\meta.json" -Raw -ErrorAction Stop | ConvertFrom-Json
        if (-not $meta.fingerprint) { throw "the disc has no fingerprint (no BDMV\STREAM or VIDEO_TS files were listed); see $stage\files.json" }
        "disc: label=$label format=$($meta.format) fingerprint=$($meta.fingerprint) disc_id=$($meta.disc_id) files=$($meta.file_count)"

        $status = @()
        $hashCode = curl.exe -s -m 30 -o "$stage\thediscdb-hash.json" -w '%{http_code}' "https://thediscdb.com/api/dischash/$($meta.fingerprint)"
        $status += "dischash $hashCode"
        if ($meta.disc_id) {
            $idCode = curl.exe -s -m 30 -o "$stage\thediscdb-discid.json" -w '%{http_code}' "https://thediscdb.com/api/discid/$($meta.disc_id)"
            $status += "discid $idCode"
        }
        $tmdbNote = 'tmdb not tried: TheDiscDB gave no TMDb ID'
        $tmdbId = $null
        if ($hashCode -eq '200') {
            try {
                $hit = Get-Content -LiteralPath "$stage\thediscdb-hash.json" -Raw -ErrorAction Stop | ConvertFrom-Json
                $tmdbId = [string]$hit.media.external_ids.tmdb
                # The ID comes from community data and goes onto a command line next to the token: digits only.
                if ($tmdbId -notmatch '^\d+$') { $tmdbId = $null }
                $kind = if ($hit.media.type -eq 'Movie') { 'movie' } else { 'tv' }
            } catch { }
        }
        if ($tmdbId -and -not $env:TMDB_TOKEN) { $tmdbNote = 'tmdb not tried: TMDB_TOKEN is not set in this session' }
        if ($tmdbId -and $env:TMDB_TOKEN) {
            $tmdbCode = curl.exe -s -m 30 -o "$stage\tmdb.json" -w '%{http_code}' -H "Authorization: Bearer $env:TMDB_TOKEN" "https://api.themoviedb.org/3/$kind/${tmdbId}?append_to_response=external_ids"
            $status += "tmdb $kind/$tmdbId $tmdbCode"
            $tmdbNote = $null
        }
        Set-Content -LiteralPath "$stage\lookup-status.txt" -Value $status -Encoding UTF8 -ErrorAction Stop
        "lookups: $($status -join '; ')" + $(if ($tmdbNote) { "; $tmdbNote" } else { '' })

        # Replace the recording's snapshot and lookups. meta.json goes first and comes back last,
        # so a recording that has a meta.json is always complete.
        New-Item -ItemType Directory -Force -Path $out -ErrorAction Stop | Out-Null
        foreach ($name in 'meta.json', 'files.json', 'nav', 'thediscdb-hash.json', 'thediscdb-discid.json', 'tmdb.json', 'lookup-status.txt') {
            if (Test-Path -LiteralPath "$out\$name") { Remove-Item -LiteralPath "$out\$name" -Recurse -Force -ErrorAction Stop }
        }
        foreach ($name in 'files.json', 'nav', 'thediscdb-hash.json', 'thediscdb-discid.json', 'tmdb.json', 'lookup-status.txt', 'meta.json') {
            if (Test-Path -LiteralPath "$stage\$name") { Move-Item -LiteralPath "$stage\$name" -Destination "$out\$name" -ErrorAction Stop }
        }
        Remove-Item -LiteralPath $stage -Recurse -Force -ErrorAction Stop
    } else {
        $meta = Get-Content -LiteralPath "$out\meta.json" -Raw -ErrorAction Stop | ConvertFrom-Json
        "disc: kept from the earlier recording of this same disc: label=$($meta.volume_label) format=$($meta.format) fingerprint=$($meta.fingerprint) disc_id=$($meta.disc_id) files=$($meta.file_count)"
        "lookups: kept: " + ((Get-Content -LiteralPath "$out\lookup-status.txt" -ErrorAction SilentlyContinue) -join '; ')
    }

    if (-not $NoScan) {
        # Scan files exist here only under -Force: remove them, so that stale files cannot pass for this scan's.
        foreach ($file in $scanFiles) {
            if (Test-Path -LiteralPath $file) { Remove-Item -LiteralPath $file -Force -ErrorAction Stop }
        }
        $index = Get-DiscIndex
        $flags = Get-MkFlags
        $scanStart = Get-Date
        & $CAP record --out $scan -- $MK @flags info "disc:$index" | Out-Null
        Save-MkDebugLog -Since $scanStart -To "$scan.makemkv-debug.txt"
        $exitJson = [string](Get-Content -LiteralPath "$scan.exit.json" -Raw -ErrorAction SilentlyContinue)
        $raw = [string](Get-Content -LiteralPath $scan -Raw -ErrorAction SilentlyContinue)
        $banner = [regex]::IsMatch($raw, '(?m)^MSG:1005,')
        $count = [regex]::Match($raw, '(?m)^TCOUNT:(\d+)')
        $titles = if ($count.Success) { $count.Groups[1].Value } else { 'NO-TCOUNT-LINE' }
        "scan: $ScanName.txt exit.json=$(if ($exitJson) { $exitJson.Trim() } else { 'MISSING' }) version_banner=$banner titles=$titles"
        if (-not $exitJson) { throw "the scan did not finish: $scan.exit.json was not written" }
        if (-not $banner) { throw "MakeMKV did not run: no MSG:1005 version banner in $scan" }
        $listed = $count.Success -and ([int]$titles -gt 0)
        if (-not $listed -and -not $ScanMayFail) { throw "the scan lists no titles (titles=$titles)" }
        if (-not $listed) { $note = ' (MakeMKV ran; the scan lists no titles, accepted because of -ScanMayFail)' }
    }
    "RESULT ok$note"
} catch {
    "RESULT FAILED: $($_.Exception.Message)"
    throw
}
```

Run: `git status --short -- scripts/phase0/record-disc.ps1; git log --oneline -1 -- scripts/phase0/record-disc.ps1; grep -n 'switch]\$ScanMayFail' scripts/phase0/record-disc.ps1`
Expected:
- no status line;
- a last commit that is the controller's prerequisite commit, not `0617064` (the first version);
- one grep line showing `[switch]$ScanMayFail,`.

Anything else means the prerequisite was not applied and committed: stop and report before any disc session.

- [ ] **Step 2: The pull script and the corpus README**

Which disc is in the drive: none needed.

`scripts/phase0/pull-recording.sh` must have exactly this content (applied by the controller as a prerequisite):

```bash
#!/usr/bin/env bash
# Copy a recording from the mini PC into corpus/owner/<slug>.
# Nothing in corpus/owner is ever deleted: the recording is first copied into a fresh temporary
# folder, and only then are its files copied over corpus/owner/<slug>/, replacing files of the same
# name and keeping files that exist only on the Mac (such as a hand-written notes.md).
# A failed copy from the mini PC changes nothing in corpus/owner.
set -euo pipefail
[ "$#" -eq 1 ] || { echo "usage: scripts/phase0/pull-recording.sh <slug>" >&2; exit 2; }
SLUG="$1"
# A slug is lowercase letters, digits and hyphens; it names a folder on both machines.
[[ "$SLUG" =~ ^[a-z0-9][a-z0-9-]*$ ]] || { echo "bad slug: '$SLUG'" >&2; exit 2; }
HOST="${LAZYMKV_BOX:-USER@makemkv-box}"
cd "$(git rev-parse --show-toplevel)"
TMP="$(mktemp -d "${TMPDIR:-/tmp}/lazymkv-pull.XXXXXX")"
trap 'rm -rf "$TMP"' EXIT
scp -q -r -o BatchMode=yes "$HOST:C:/lazymkv/phase0/recordings/$SLUG" "$TMP/"
[ -f "$TMP/$SLUG/meta.json" ] || { echo "recording $SLUG on the mini PC has no meta.json; nothing was changed in corpus/owner" >&2; exit 1; }
# The copies of the disc's navigation files may arrive read-only; make them replaceable by the next pull.
chmod -R u+w "$TMP/$SLUG"
mkdir -p "corpus/owner/$SLUG"
cp -Rf "$TMP/$SLUG/." "corpus/owner/$SLUG/"
copied="$(find "$TMP/$SLUG" -type f | wc -l | tr -d ' ')"
total="$(find "corpus/owner/$SLUG" -type f | wc -l | tr -d ' ')"
echo "pulled corpus/owner/$SLUG ($copied files copied, $total files there now)"
```

`corpus/owner/README.md` must have exactly this content:

```markdown
# Recordings of the owner's discs

One folder per disc, written on the mini PC under `C:\lazymkv\phase0\recordings\<slug>`
by `scripts/phase0/record-disc.ps1` and the experiment scripts, then copied here with
`scripts/phase0/pull-recording.sh <slug>`. Layout: spec section 15, "Recordings".
A slug is lowercase letters, digits and hyphens.

A pull never deletes anything here: files pulled again replace their old copies, and
files that exist only here (such as a hand-written `notes.md`) are kept. A file removed
or renamed on the mini PC therefore stays here until someone removes it by hand.

`AACS/Unit_Key_RO.inf` is never stored; `meta.json` holds only its SHA-1 (`disc_id`).
No token or password is ever written to a recording.
```

Run: `git status --short -- scripts/phase0/pull-recording.sh corpus/owner/README.md; git log --oneline -1 -- scripts/phase0/pull-recording.sh; grep -n 'ever deleted' scripts/phase0/pull-recording.sh; grep -n 'never deletes' corpus/owner/README.md; git ls-files -s scripts/phase0/pull-recording.sh`
Expected:
- no status line;
- a last commit that is the controller's prerequisite commit, not `0617064`;
- one grep line from each file (`# Nothing in corpus/owner is ever deleted: ...` and `A pull never deletes anything here: ...`);
- a line starting `100755`.

Anything else means the prerequisite was not applied and committed: stop and report before any disc session.

- [ ] **Step 3: Check the tools on the mini PC**

Which disc is in the drive: preferably none. With a correct Assert-Slug the last command never reaches the drive.

```bash
grep -E '^(function (Assert-Slug|Assert-DiscIs|Assert-EmptyDir|Get-InsertedDisc)|\$Recordings)' scripts/phase0/env.ps1
scripts/box.sh cargo build --release -p lazymkv-capture
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -Command \"(Get-Command C:\lazymkv\src\scripts\phase0\record-disc.ps1).Parameters.Keys\""
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\record-disc.ps1 -Slug BadSlug -NoScan"; echo "exit=$?"
```

Run the last command with a tool timeout of at least 300000 ms: if Assert-Slug is wrong, the script waits 120 s for a disc, and the default 120 s limit would end the command before it prints its result.

Expected:
- `grep` prints five lines: the `$Recordings` assignment and the four helper functions from D1. With fewer lines, the env.ps1 prerequisites are missing: stop.
- `box.sh` syncs the tree, then cargo ends with ``Finished `release` profile [optimized] target(s) in ...`` and box.sh exits 0. The capture tool (`C:\lazymkv\target\release\lazymkv-capture.exe`) is then present and built from the current sources. If the build fails, stop and report.
- The parameter list begins `Slug`, `NoScan`, `ScanName`, `ScanMayFail`, `Force`, and there is no parse error.
- The last command prints `RESULT FAILED:` with Assert-Slug's rejection of `BadSlug` within seconds, then `exit=1`. Two other outcomes mean stop:
  - it says that `Assert-Slug` is not recognized: env.ps1 is not the version with the D1 helpers;
  - it takes two minutes and ends with `no readable disc` (use a tool timeout of at least 300000 ms for that command), or records a disc that is in the drive: Assert-Slug compares case-insensitively, and pull-recording.sh would later refuse such a slug.

- [ ] **Step 4: Record a disc without a scan**

Which disc is in the drive: the disc SLUG, chosen by the owner, never Disc A. The owner's description of it is compared with DISC_A before it goes in, and TheDiscDB's answer is compared after the pull below. For a repeat from Step 7 it must also be a disc not recorded before; the fingerprint list below checks that.

Before the first disc, ask the owner: "Which film is Disc A (title and year)? Please keep it apart until Task 9 asks for it." Write the answer down as DISC_A, and report it with this task's results, so that Task 9 Step 3 can confirm it with the owner.

For each disc, ask the owner what it is (title, year and format) before it goes in, and compare it with DISC_A. If it is the same film, in any format, do not record it; ask for another disc. The same film is refused even as a Blu-ray, because a combo pack's UHD and Blu-ray share a case and are easily swapped. Otherwise ask the owner to insert it. For the first disc, a Blu-ray or UHD is preferred, because it also exercises the AACS disc ID. Ask the owner to say when the drive's light has stopped blinking. Choose SLUG from the title, year and format, and use it in the commands below. The scripts were synced in Step 3.

```bash
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\record-disc.ps1 -Slug SLUG -NoScan"; echo "exit=$?"
```

Run it with a tool timeout of at least 300000 ms: the script may wait up to 120 s for the disc, then up to 30 s for each of three lookups.

Expected:

```text
disc: label=<volume label or (none)> format=<bluray|dvd> fingerprint=<32 hex characters> disc_id=<40 hex characters, or empty> files=<N>
lookups: dischash <code>[; discid <code>][; tmdb <movie|tv>/<id> <code>][; tmdb not tried: <reason>]
RESULT ok
exit=0
```

A lookup code of `200` is an answer and `404` means unknown to TheDiscDB. `000` means no answer; Step 7 says how to retry. What the other results mean:
- `exit=1` with `RESULT FAILED: no readable disc in D: after 120 s`: nothing was written. Ask the owner whether the disc is in and the light has stopped, then run the same line again.
- `exit=1` with `RESULT FAILED: lazymkv-capture disc failed (exit N)`: this is the tool's first run on real media. Stop and report, with the error text printed above the RESULT line.
- `exit=1` with `RESULT FAILED: the disc has no fingerprint ...`: stop and report, with the output of `ssh USER@makemkv-box "type C:\lazymkv\phase0\tmp\record-SLUG\files.json"`.
- `exit=1` with a `RESULT FAILED:` line from Assert-Slug: the slug is malformed. Choose a valid one and run again.
- `exit=1` with a `RESULT FAILED:` line from Assert-DiscIs: the slug is already used by another disc. Choose a new slug.
- `disc: kept from the earlier recording ...`: this disc is already recorded under SLUG. It is not a new disc; ask the owner for another.
- `exit=255`: ssh could not reach the mini PC. Run the line again once it is reachable.
- Anything else: stop and report.

Then copy the recording to the Mac and look at it:

```bash
scripts/phase0/pull-recording.sh SLUG
python3 -c "import json,sys; print(json.load(open('corpus/owner/'+sys.argv[1]+'/meta.json', encoding='utf-8-sig')))" SLUG
python3 -c "import json,pathlib; [print(p.parent.name, json.loads(p.read_text(encoding='utf-8-sig')).get('fingerprint')) for p in sorted(pathlib.Path('corpus/owner').glob('*/meta.json'))]"
python3 - SLUG <<'EOF'
import json, pathlib, sys
d = pathlib.Path("corpus/owner") / sys.argv[1]
found = 0
for name in ("thediscdb-hash.json", "thediscdb-discid.json"):
    try:
        data = json.loads((d / name).read_text(encoding="utf-8-sig"))
    except (OSError, ValueError):
        continue
    for rec in (data if isinstance(data, list) else [data]):
        media = rec.get("media") if isinstance(rec, dict) else None
        if isinstance(media, dict):
            print(f"{name}: TheDiscDB says {media.get('title')} ({media.get('year')}), {rec.get('format')}")
            found += 1
if not found:
    print("TheDiscDB names no film for this disc")
EOF
```

Expected:
- `pulled corpus/owner/SLUG (N files copied, N files there now)`. If it fails, nothing on the Mac changed; run it again.
- `meta.json` printed with `file_count` above 0, a 32-character `fingerprint` and the `format`.
- For a Blu-ray or UHD, `disc_id` is 40 hex characters. If it is None, run `grep -i unit_key corpus/owner/SLUG/files.json`:
  - a listed path with a size means the file exists but could not be read;
  - no output means the drive does not expose it.

  Note which (DISCID_STATE). This is part of experiment 5, which Task 9 reports.
- The fingerprint list shows SLUG's fingerprint exactly once. If another slug has the same fingerprint, the disc was recorded before. Tell the owner, and leave this disc out of the count in Step 7.
- The last check prints `TheDiscDB names no film for this disc`, or one line per TheDiscDB record: `<file>: TheDiscDB says <title> (<year>), <format>`. Compare each named film with DISC_A, allowing for small differences in how the title is written. If one names DISC_A's film, the disc in the drive may be Disc A:
  - ask the owner to take it out now and keep it apart;
  - do not run Steps 5 to 7 on it, and record no other disc; stop and report.

  `-NoScan` ran no MakeMKV command, so Disc A's first sight is still intact. But this recording, on the mini PC and in `corpus/owner/SLUG`, must be dealt with before Task 9, whose fingerprint check would otherwise find it.

- [ ] **Step 5: See how the drive presents the first disc**

Which disc is in the drive: still the disc FIRST_SLUG (the disc of Step 4, the first time through), which passed Step 4's checks against DISC_A. Do this step only once.

```bash
ssh USER@makemkv-box "vol D:"
ssh USER@makemkv-box "dir /s /b /a:r C:\lazymkv\phase0\recordings\FIRST_SLUG\nav"
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -Command \". C:\lazymkv\src\scripts\phase0\env.ps1; (Get-InsertedDisc).fingerprint\""; echo "exit=$?"
```

Expected:
- `vol` prints `Volume in drive D is <label>` (or `Volume in drive D has no label`). Compare it with `volume_label` in `corpus/owner/FIRST_SLUG/meta.json`: the same text, or `(none)` for no label. Write down LABEL_ANSWER as `same` or `differs: vol says <x>, meta.json says <y>`. A difference is reported to the controller, because record-disc.ps1 reads the label through .NET's DriveInfo. It does not block the next steps.
- `dir` either lists the nav copies (they kept the disc's read-only attribute) or prints `File Not Found` (they did not). record-disc.ps1 handles both. Write down RO_ANSWER as `read-only` or `writable`.
- The last command prints exactly one line, the `fingerprint` from `corpus/owner/FIRST_SLUG/meta.json` (32 hex characters), then `exit=0`. This is the first run of Get-InsertedDisc on a disc, and Tasks 12 and 15 read its last line as the fingerprint. `no disc is mounted in D:` means the disc is not ready: wait half a minute and run it again. Any other output (extra lines, another fingerprint, an error) is reported to the controller before Task 12; it does not block Step 6, which checks the disc through the same helper and reports its own failure.

- [ ] **Step 6: Rehearse a scan job on the first disc**

Which disc is in the drive: still the disc FIRST_SLUG, which passed Step 4's checks against DISC_A. Never run this step on a disc that matched DISC_A. record-disc.ps1 checks that the disc is FIRST_SLUG's with Assert-DiscIs before it starts MakeMKV. Ask the owner to leave it in for about fifteen minutes. Do this step only once.

This step tests job.sh, record-disc.ps1's scan path and its evidence guard on an ordinary disc, so that Task 9 does not discover a defect on Disc A, whose first scan cannot be repeated.

```bash
scripts/phase0/job.sh record-disc.ps1 record-rehearsal -Slug FIRST_SLUG
scripts/phase0/wait.sh record-rehearsal
```

job.sh prints `started record-disc.ps1; log: C:\lazymkv\phase0\logs\record-rehearsal.txt`. If job.sh exits 1 because a phase 0 job is still running, wait for that job with its own wait.sh line, then start this one.

What wait.sh's exit codes mean for this job:
- **0**: the job finished. Check the log against the Expected below.
- **3**: the job is still running. This is normal: a Blu-ray scan takes a few minutes, a UHD with Java menus ten or more. Run `scripts/phase0/wait.sh record-rehearsal` again, and start nothing else meanwhile.
  If 45 minutes pass without a PHASE0-DONE line, tell the owner and end the job with `ssh USER@makemkv-box "powershell -NoProfile -Command Stop-ScheduledTask -TaskName lazymkv-phase0"`. Then check that `ssh USER@makemkv-box "tasklist | findstr /i makemkvcon"` prints nothing, and stop and report.
- **1**: the script threw. Read the `RESULT FAILED:` line. Two causes are not defects:
  - `no readable disc in D: after 120 s`: the disc was taken out or is not ready;
  - `is not the disc of recording FIRST_SLUG` (Assert-DiscIs): the disc was changed.

  For either, no scan file was written. Ask the owner to put FIRST_SLUG's disc back and to say when the drive's light has stopped, then run the job again with a new log name such as `record-rehearsal-b` (then `-c`). Use that name in the wait.sh line, and read it wherever the rest of this step says `record-rehearsal`. For any other reason, stop and report: this is the defect the rehearsal exists to find, and Task 9 must not start until it is fixed.
- **4**: the job is not running and wrote no PHASE0-DONE line (it never started, or was killed). Stop and report.
- **5**: the mini PC cannot be reached. Run wait.sh again once it can be.

Expected (exit 0), in the log:

```text
disc: kept from the earlier recording of this same disc: label=... format=... fingerprint=<32 hex> disc_id=... files=...
lookups: kept: dischash <code>; ...
scan: scan.txt exit.json={"exit_code":<n>,"elapsed_ms":<n>,"lines":<n>} version_banner=True titles=<n above 0>
RESULT ok
PHASE0-DONE status=ok exit=<n>
```

`exit=<n>` on the DONE line is only the last native command's exit code, not a verdict. Then:

```bash
scripts/phase0/pull-recording.sh FIRST_SLUG
python3 scripts/phase0/summarize_scan.py corpus/owner/FIRST_SLUG
```

Expected: the summary starts `MakeMKV: MakeMKV v2.0.0 win(x64-release)`, and its title table has as many rows as `titles=` in the log. A Python traceback is a defect: stop and report.

Now prove that the guard keeps an existing scan. Start the same job again under a second log name:

```bash
scripts/phase0/job.sh record-disc.ps1 record-rehearsal-2 -Slug FIRST_SLUG
scripts/phase0/wait.sh record-rehearsal-2
ssh USER@makemkv-box "type C:\lazymkv\phase0\recordings\FIRST_SLUG\scan.txt.exit.json"
```

Expected:
- wait.sh exits **1** within a minute.
- The log shows `RESULT FAILED: C:\lazymkv\phase0\recordings\FIRST_SLUG\scan.txt already exists and is kept as evidence; ...`, an `ERROR:` line and `PHASE0-DONE status=error exit=<n>`.
- `type` prints the same exit.json as the `record-rehearsal` log.

If wait.sh exits 0 here, the guard did not hold: stop and report, because Task 9 must not start until it does. Any other exit code is read as above.

Write down REHEARSAL_ANSWER: `RESULT ok, <titles> titles, scan took <elapsed_ms / 60000, rounded> minutes; the second run was refused`.

- [ ] **Step 7: Check the fingerprint against TheDiscDB**

Which disc is in the drive: the disc SLUG, the one recorded last in Step 4. It must still be in the drive if its lookups have to be retried (rule 3 below).

```bash
python3 scripts/phase0/hit_rate.py corpus/owner
python3 - SLUG <<'EOF'
import json, pathlib, sys
d = pathlib.Path("corpus/owner") / sys.argv[1]
meta = json.loads((d / "meta.json").read_text(encoding="utf-8-sig"))
fp = (meta.get("fingerprint") or "").upper()
did = (meta.get("disc_id") or "").upper()
print("ours: fingerprint", fp or "NONE", "disc_id", did or "NONE")
for name in ("thediscdb-hash.json", "thediscdb-discid.json"):
    path = d / name
    if not path.exists():
        print(name, "absent (lookup not made)")
        continue
    try:
        data = json.loads(path.read_text(encoding="utf-8-sig"))
    except ValueError:
        print(name, "is not JSON (see lookup-status.txt for the HTTP code)")
        continue
    records = [r for r in (data if isinstance(data, list) else [data]) if isinstance(r, dict) and r.get("content_hash")]
    if not records:
        print(name, "holds no disc record")
    for rec in records:
        theirs = str(rec["content_hash"]).upper()
        gid = str(rec.get("global_disc_id") or "").upper()
        ch = "MATCH" if fp and theirs == fp else "DIFFERENT"
        gd = ("MATCH" if gid == did else "DIFFERENT") if did and gid else "not compared"
        print(f"{name}: content_hash {theirs} {ch}; global_disc_id {gid or 'none'} {gd}")
EOF
```

Expected:
- `hit_rate.py` prints one row per recorded disc (`<slug> <format> hash=<hit|miss|failed|n/a> discid=<hit|miss|failed|n/a>`) and its summary lines. It exits 1 when a fingerprint lookup did not get an answer or a disc-ID lookup failed. That exit code is not an error here: read the rows. `discid=n/a` means no disc-ID lookup was made, as for a DVD.
- The check prints our fingerprint and disc ID, then one line per TheDiscDB record. Here SLUG is a command-line argument; nothing inside the heredoc needs editing.

Read SLUG's row and the check's lines with these rules:

1. **`hash=hit`**: TheDiscDB holds a disc whose content hash is exactly our fingerprint. This proves the fingerprint as computed on this machine from a real disc, through the drive's file listing.
   - The `thediscdb-hash.json` line's `content_hash ... MATCH` is not a second proof: that record was fetched by our own fingerprint, so it always matches.
   - The same line's `global_disc_id` tests our AACS disc ID: `MATCH` proves it too. `DIFFERENT` means either our disc ID is wrong or TheDiscDB holds another pressing with the same file sizes; report it. `not compared` means the disc has no disc ID.
2. **`hash=miss` and `discid=hit`**: TheDiscDB knows this disc by its AACS ID. Look at the `thediscdb-discid.json` lines.
   - If any says `content_hash ... MATCH`, the fingerprint is proven independently. Report that the dischash lookup missed anyway.
   - If every one says `DIFFERENT`, our fingerprint disagrees with TheDiscDB on a disc it knows. Stop and report it with `corpus/owner/SLUG/files.json`. This is not an ordinary miss.
3. **`hash=failed` or `discid=failed`** (code `000`, a rate limit or a server error): the lookup got no answer. With the same disc still in the drive, run the lookups again:

   ```bash
   ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\record-disc.ps1 -Slug SLUG -NoScan -Force"; echo "exit=$?"
   scripts/phase0/pull-recording.sh SLUG
   ```

   This is the one use of `-Force` in this task. It replaces this recording's snapshot and lookups with new ones of the same disc; Assert-DiscIs refuses any other disc, and no scan file is touched. Run the ssh line with a tool timeout of at least 300000 ms, as in Step 4. The output and exit codes read as in Step 4. Then repeat this step. If the lookup fails again, record `failed` and count the disc as proving nothing.
4. **`hash=miss` with `discid=miss` or `discid=n/a`**: this proves nothing either way.

If this disc proved nothing (rules 3 or 4), repeat Step 4 and this step with another disc: a new SLUG, never Disc A, never a disc recorded before. Steps 5 and 6 are not repeated. Stop when the first of these happens:
- one disc proves the fingerprint (rule 1, or rule 2 with MATCH);
- one disc shows a mismatch (rule 2 with every record DIFFERENT, which is also a stop-and-report);
- five discs have proved nothing.

The recordings stay in `corpus/owner`; experiment 10 (Task 15) counts them with the rest.

Then append this section to the end of `docs/phase0/environment.md`, with one table row per disc recorded in this task, in order. All placeholders are defined at the top of this task.

```markdown
## Fingerprint check (Task 8)

The fingerprint and AACS disc ID computed on this machine by `scripts/phase0/record-disc.ps1`
(drive D:), compared with TheDiscDB (`scripts/phase0/hit_rate.py` and the check in Task 8 Step 7).
A dischash hit proves the fingerprint; the content_hash of a discid record is an independent check
of it, and the global_disc_id of a dischash record checks the disc ID.

| Slug | Format | Disc ID | dischash | discid | discid record's content_hash | dischash record's global_disc_id |
|---|---|---|---|---|---|---|
| SLUG | FORMAT | DISCID_STATE | HASH_RESULT | DISCID_RESULT | CH_RESULT | GID_RESULT |

Outcome: OUTCOME

First disc (FIRST_SLUG): volume label in `meta.json` against `vol D:`: LABEL_ANSWER. Navigation-file
copies in the recording: RO_ANSWER. Scan job rehearsal (`record-rehearsal`): REHEARSAL_ANSWER.
```

OUTCOME is one of these sentences:
- `the fingerprint is proven: TheDiscDB knows <slug> (<format>) by it.`
- `the fingerprint disagrees with TheDiscDB on <slug> (<format>); reported.`
- `not proven: five discs were unknown to TheDiscDB by both lookups.`

If the stopping disc also gave a `DIFFERENT` global_disc_id under rule 1, add: `The disc ID of <slug> differs from TheDiscDB's global_disc_id; reported.`

- [ ] **Step 8: Commit**

Which disc is in the drive: does not matter.

```bash
find corpus/owner -iname '*unit_key*'
grep -rIl 'eyJhbGciOi' corpus/owner
git add scripts/phase0/record-disc.ps1 scripts/phase0/pull-recording.sh corpus/owner docs/phase0/environment.md
git status --short
git commit -m "Record the first discs, rehearse a scan job and check the fingerprint against TheDiscDB" -m "Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

Expected:
- `find` and `grep` print nothing: no AACS unit-key file, and no JSON Web Token in any recording (the TMDb read access token is one, and it starts with that text). If either prints a path, stop and do not commit.
- `git status --short` lists only the new `corpus/owner/<slug>/` files and `docs/phase0/environment.md`. The scripts are unchanged, because the prerequisites were committed earlier.
- The commit succeeds.

---

### Task 9: First contact with Disc A (experiments 5 and 12)

**Files:**
- Create: `scripts/phase0/exp05-evidence.ps1` (Step 1; committed in Step 2, before Disc A is inserted).
- Create: `corpus/owner/SLUG_A/`, Disc A's recording together with the experiment 5 evidence (Steps 5 to 9).
- Create: `docs/phase0/experiments/EXP-05-offline-and-unit-key.md`, `docs/phase0/experiments/EXP-12-java-labels.md`.

**Interfaces:**
- Consumes:
  - from `env.ps1`: `$Recordings`, `$CAP`, `$MK`, `$Drive`, `Assert-Slug`, `Assert-DiscIs` (which uses `Get-InsertedDisc`); through `record-disc.ps1` also `Assert-EmptyDir`, `Get-DiscIndex` and `Get-MkFlags`.
  - `record-disc.ps1 -Slug <slug> [-NoScan] [-ScanName <name>] [-ScanMayFail]` (Task 8's final version): with `-NoScan` over plain ssh (Step 5: lazymkv-capture and curl only, never MakeMKV), otherwise as a job. Its output lines are `disc: ...`, `lookups: ...`, `scan: <ScanName>.txt exit.json=... version_banner=... titles=...` (not with `-NoScan`), then `RESULT ok` or `RESULT FAILED: <reason>`. On a recording that already has a `meta.json` it runs `Assert-DiscIs <slug>` before anything else touches the disc.
  - `scripts/box.sh`, `scripts/phase0/job.sh`, `scripts/phase0/wait.sh`, `scripts/phase0/pull-recording.sh <slug>`.
  - `scripts/phase0/summarize_scan.py <recording dir>` (reads `scan.txt`); `scripts/phase0/compare_scans.py <scan A> <scan B>` (exits 1 when the title lists differ or one is empty); `robot.parse` and `robot.split_fields` from `scripts/phase0/robot.py`.
  - `lazymkv-capture record --out <file> -- <command...>`.
- Produces:
  - `scripts/phase0/exp05-evidence.ps1 -Slug <slug> -Listing <name> [-DriveList <name>] [-Force]`. Run it over plain ssh: it needs C: and the drive, never the share. It writes into `$Recordings\<slug>\`:
    - `makemkv-data-<Listing>.txt`: one line per file in MakeMKV's data folders (last write time in UTC, size, path), after a line `app_DataDir: ...` and one `folder: <path> files=<n>` or `folder: <path> (absent)` line per candidate folder. No file is opened. Of MakeMKV's registry settings only `app_DataDir` is read.
    - with `-DriveList`, first: `<DriveList>.txt` with `.timing.jsonl`, `.stderr` and `.exit.json`, MakeMKV's drive listing (`info disc:9999`, the same call that `Get-DiscIndex` makes before every scan). Before it starts MakeMKV it waits up to 120 s for a readable disc and, when the recording already has a meta.json, checks the disc with Assert-DiscIs.
    - It prints `drives: <DriveList>.txt exit.json=<json or MISSING> version_banner=<True|False> state=<DRV state of $Drive> label=<disc label> other_messages=<MSG codes other than 1005, or none>` (only with `-DriveList`), then `listing: makemkv-data-<Listing>.txt files=<n> folders=<folders found, or NONE-FOUND>`, and last `RESULT ok` or `RESULT FAILED: <reason>`. After a failure it throws, so an ssh run exits 1.
    - It refuses to replace any of its files unless `-Force` is given. `-Force` is never used in this task.
  - Disc A's recording, `corpus/owner/SLUG_A/`:
    - from `record-disc.ps1 -NoScan` (Step 5, before MakeMKV first sees the disc): `meta.json`, `files.json`, `nav/`, the lookup files;
    - the offline scan `scan-offline.txt` and the online scan `scan.txt`, each with `.timing.jsonl`, `.stderr` and `.exit.json`. Tasks 10 to 14 and 17 use `scan.txt`;
    - the drive listings `drives-offline.txt` and `drives-online.txt`, each with the same three companion files;
    - the data-folder listings `makemkv-data-0-before.txt`, `makemkv-data-1-after-drives-offline.txt`, `makemkv-data-2-after-scan-offline.txt`, `makemkv-data-3-after-drives-online.txt` and `makemkv-data-4-after-scan-online.txt`.
  - The slug SLUG_A, written in the `Disc(s):` line at the top of both reports.
  - The reports `EXP-05-offline-and-unit-key.md` (Task 14 adds the second drive to it) and `EXP-12-java-labels.md` (Task 15 updates it if a later UHD answers it).
  - Disc A left in drive D: at the end of the task, for Tasks 10 to 14.

Placeholders:
- Disc A: the owner's UHD film that MakeMKV has never scanned on this machine (owner inputs table). Experiment 5 must be the first time MakeMKV sees it, and that first time cannot be repeated.
- SLUG_A: Disc A's slug. It is chosen in Step 3 from the title, year and format the owner gives, using lowercase letters, digits and hyphens only, for example `dune-2021-uhd`. Tasks 10 to 17 use it for Disc A.
- FIRST_SLUG: the first disc recorded in Task 8. Its slug is in the line starting `First disc (` of the `## Fingerprint check (Task 8)` section of `docs/phase0/environment.md`. MakeMKV has already scanned it (Task 8 Step 6).
- REHEARSAL_SLUG: a disc that Task 8 recorded, chosen in Step 2. Assert-DiscIs proves it is in the drive before MakeMKV reads it.
- DATA_FOLDERS: the `folders=` value printed by the first listing in Step 2.
- PROBE_ANSWER: what the drive-listing rehearsal in Step 2 showed, written down there.
- RULE_ANSWER: the firewall check in Step 4, written down there.
- GUI_ANSWER: the MakeMKV process checks and the owner's answer in Step 5, written down there.

Every `ssh ...` line below is run as printed, with SLUG_A and REHEARSAL_SLUG replaced by the real slugs. Every evidence file in this task is made exactly once. A retry is allowed only where a step says so, and it never replaces an evidence file. Never pass `-Force` in this task, to either script.

- [ ] **Step 1: Write the experiment 5 evidence script**

Which disc is in the drive: none needed.

`record-disc.ps1` keeps the scan, but not two other things experiment 5 needs. The first is MakeMKV's drive listing. `Get-DiscIndex` runs it just before the scan and discards its output, so if the drive listing already opens the disc, MakeMKV's first messages about Disc A would be lost. The second is the state of MakeMKV's data folder (`keydb.cfg`, downloaded data, key caches) before and after each MakeMKV run. This script records both.

`scripts/phase0/exp05-evidence.ps1`:

```powershell
# Experiment 5 evidence that record-disc.ps1 does not keep (Task 9). Run it over plain ssh: it needs C: and
# the drive, never the share. It writes into $Recordings\<Slug>\ (C:\lazymkv\phase0\recordings\<Slug>\):
#   makemkv-data-<Listing>.txt  one line per file in MakeMKV's data folders: last write time (UTC), size, path.
#                               No file is opened. Of MakeMKV's registry settings only app_DataDir is read;
#                               app_Key (the licence key) is never read.
#   <DriveList>.txt             only with -DriveList, and made before the folder listing: MakeMKV's drive
#                               listing (info disc:9999, the call Get-DiscIndex makes before every scan),
#                               recorded by lazymkv-capture with .timing.jsonl, .stderr and .exit.json.
# With -DriveList on a recording that already has a meta.json, Assert-DiscIs checks the disc before MakeMKV is started.
# An existing evidence file is refused unless -Force is given.
# The last line is "RESULT ok" or "RESULT FAILED: <reason>"; after a failure the script throws.
param(
    [Parameter(Mandatory)][string]$Slug,
    [Parameter(Mandatory)][string]$Listing,
    [string]$DriveList = '',
    [switch]$Force
)
. C:\lazymkv\src\scripts\phase0\env.ps1

try {
    Assert-Slug $Slug
    Assert-Slug $Listing
    if ($DriveList) { Assert-Slug $DriveList }
    $out = "$Recordings\$Slug"
    $list = "$out\makemkv-data-$Listing.txt"
    $files = @($list)
    if ($DriveList) {
        $drives = "$out\$DriveList.txt"
        $files += @($drives, "$drives.stderr", "$drives.timing.jsonl", "$drives.exit.json", "$drives.makemkv-debug.txt")
    }

    # Refuse before anything is created, deleted or started.
    foreach ($file in $files) {
        if ((Test-Path -LiteralPath $file) -and -not $Force) {
            throw "$file already exists and is kept as evidence; choose another name"
        }
    }
    if ($DriveList -and -not (Test-Path -LiteralPath $CAP)) { throw "missing $CAP; build it with: scripts/box.sh cargo build --release -p lazymkv-capture" }
    New-Item -ItemType Directory -Force -Path $out -ErrorAction Stop | Out-Null
    foreach ($file in $files) {
        if (Test-Path -LiteralPath $file) { Remove-Item -LiteralPath $file -Force -ErrorAction Stop }
    }

    if ($DriveList) {
        # The disc may still be spinning up: wait up to two minutes until its root can be listed.
        $ready = $false
        for ($i = 0; $i -lt 24 -and -not $ready; $i++) {
            if (@(Get-ChildItem -LiteralPath "${Drive}:\" -ErrorAction SilentlyContinue).Count -gt 0) { $ready = $true } else { Start-Sleep -Seconds 5 }
        }
        if (-not $ready) { throw "no readable disc in ${Drive}: after 120 s; MakeMKV was not started" }
        # A recording that already has a meta.json belongs to one disc: the drive listing must be of that disc.
        if (Test-Path -LiteralPath "$out\meta.json") { Assert-DiscIs $Slug }
        $listStart = Get-Date
        & $CAP record --out $drives -- $MK -r --cache=1 --debug info disc:9999 | Out-Null
        Save-MkDebugLog -Since $listStart -To "$drives.makemkv-debug.txt"
        $exitJson = [string](Get-Content -LiteralPath "$drives.exit.json" -Raw -ErrorAction SilentlyContinue)
        $raw = [string](Get-Content -LiteralPath $drives -Raw -ErrorAction SilentlyContinue)
        $banner = [regex]::IsMatch($raw, '(?m)^MSG:1005,')
        $drv = [regex]::Match($raw, '(?m)^DRV:\d+,(\d+),\d+,\d+,"[^"]*","([^"]*)","' + [regex]::Escape("${Drive}:") + '"\s*$')
        $state = if ($drv.Success) { $drv.Groups[1].Value } else { 'NO-DRV-LINE' }
        $label = if ($drv.Success) { $drv.Groups[2].Value } else { '' }
        $codes = @([regex]::Matches($raw, '(?m)^MSG:(\d+),') | ForEach-Object { $_.Groups[1].Value } | Where-Object { $_ -ne '1005' })
        $others = if ($codes.Count -gt 0) { $codes -join ',' } else { 'none' }
        "drives: $DriveList.txt exit.json=$(if ($exitJson) { $exitJson.Trim() } else { 'MISSING' }) version_banner=$banner state=$state label=$label other_messages=$others"
        if (-not $exitJson) { throw "the drive listing did not finish: $drives.exit.json was not written" }
        if (-not $banner) { throw "MakeMKV did not run: no MSG:1005 version banner in $drives" }
        if ($state -ne '2') { throw "MakeMKV does not list a disc in ${Drive}: (DRV state $state; 2 means a disc is inserted)" }
    }

    # MakeMKV's data folders: the one set in app_DataDir, if any, and the usual places.
    $settings = 'HKCU:\Software\MakeMKV'
    $dataDir = $null
    if (Test-Path -LiteralPath $settings) {
        $dataDir = (Get-ItemProperty -LiteralPath $settings -Name app_DataDir -ErrorAction SilentlyContinue).app_DataDir
        $lines = @("app_DataDir: $(if ($dataDir) { $dataDir } else { '(not set)' })")
    } else {
        $lines = @("app_DataDir: (no $settings key in this session)")
    }
    $folders = @("$env:USERPROFILE\.MakeMKV", "$env:APPDATA\MakeMKV", "$env:LOCALAPPDATA\MakeMKV", "$env:ProgramData\MakeMKV")
    if ($dataDir) { $folders = @([Environment]::ExpandEnvironmentVariables([string]$dataDir)) + $folders }
    $found = @()
    $total = 0
    foreach ($folder in @($folders | Select-Object -Unique)) {
        if (-not (Test-Path -LiteralPath $folder)) { $lines += "folder: $folder (absent)"; continue }
        $items = @(Get-ChildItem -LiteralPath $folder -Recurse -Force -File -ErrorAction Stop | Sort-Object FullName)
        $lines += "folder: $folder files=$($items.Count)"
        foreach ($item in $items) {
            $lines += ('{0} UTC {1,12} {2}' -f $item.LastWriteTimeUtc.ToString('yyyy-MM-dd HH:mm:ss'), $item.Length, $item.FullName)
        }
        $found += $folder
        $total += $items.Count
    }
    Set-Content -LiteralPath $list -Value $lines -Encoding UTF8 -ErrorAction Stop
    "listing: makemkv-data-$Listing.txt files=$total folders=$(if ($found.Count -gt 0) { $found -join ';' } else { 'NONE-FOUND' })"
    'RESULT ok'
} catch {
    "RESULT FAILED: $($_.Exception.Message)"
    throw
}
```

Run: `LC_ALL=C grep -n '[^ -~]' scripts/phase0/exp05-evidence.ps1`
Expected: no output. The file must be plain ASCII: PowerShell 5.1 reads a file without a BOM in the ANSI code page.

- [ ] **Step 2: Check the tools and rehearse the evidence script on another disc**

Which disc is in the drive: none needed for the first commands. The rehearsal needs REHEARSAL_SLUG's disc, a disc that Task 8 recorded (never Disc A). `Assert-DiscIs REHEARSAL_SLUG` proves it is in the drive before MakeMKV reads it. At the end of this step the drive is empty.

Everything here must pass before Disc A is inserted.

```bash
grep -n '^## Fingerprint check (Task 8)' docs/phase0/environment.md
grep -c 'the second run was refused' docs/phase0/environment.md
grep -n 'switch]\$ScanMayFail' scripts/phase0/record-disc.ps1
grep -E '^(function (Assert-Slug|Assert-DiscIs|Assert-EmptyDir|Get-InsertedDisc)|\$Recordings)' scripts/phase0/env.ps1
scripts/box.sh cargo build --release -p lazymkv-capture
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -Command \"(Get-Command C:\lazymkv\src\scripts\phase0\record-disc.ps1).Parameters.Keys\""
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -Command \"(Get-Command C:\lazymkv\src\scripts\phase0\exp05-evidence.ps1).Parameters.Keys\""
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\exp05-evidence.ps1 -Slug BadSlug -Listing x"; echo "exit=$?"
ssh USER@makemkv-box "tasklist | findstr /i makemkv"
```

Expected:
- One `## Fingerprint check (Task 8)` line and the count `1`: Task 8 is finished, and its scan-job rehearsal passed, including the guard.
- One grep line showing `[switch]$ScanMayFail,`: record-disc.ps1 is Task 8's final version.
- Five lines from env.ps1: the `$Recordings` assignment and the four D1 helper functions.
- `box.sh` syncs the tree, which brings `exp05-evidence.ps1` to the mini PC. cargo ends with ``Finished `release` profile [optimized] target(s) in ...``, so the capture tool is present and current.
- record-disc.ps1's parameters begin `Slug`, `NoScan`, `ScanName`, `ScanMayFail`, `Force`. exp05-evidence.ps1's begin `Slug`, `Listing`, `DriveList`, `Force`. Neither shows a parse error.
- The `BadSlug` run prints `RESULT FAILED:` with Assert-Slug's rejection of `BadSlug` within seconds, then `exit=1`.
- `tasklist | findstr` prints nothing: no MakeMKV program is running.

Anything else: stop and report. Disc A is not inserted until this step passes.

Now rehearse the evidence script, and see what a drive listing does with a disc. The slugs Task 8 recorded are the rows of the table in the `## Fingerprint check (Task 8)` section of `docs/phase0/environment.md`. Ask the owner: "Please put one of these discs in the drive: <each Task 8 slug with the title the owner gave for it>. Not Disc A. Tell me when the drive's light has stopped blinking." Prefer a disc other than FIRST_SLUG: Task 8 recorded those with `-NoScan` only, so MakeMKV has never scanned them. Use FIRST_SLUG only if Task 8 recorded no other disc. Call the disc REHEARSAL_SLUG. If the last disc of Task 8 is still in the drive and is not FIRST_SLUG's, it is REHEARSAL_SLUG.

Before MakeMKV reads it, prove which disc it is (lazymkv-capture only, no MakeMKV):

```bash
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -Command \". C:\lazymkv\src\scripts\phase0\env.ps1; Assert-DiscIs REHEARSAL_SLUG; 'disc ok: REHEARSAL_SLUG'\""; echo "exit=$?"
```

Expected: the last line is `disc ok: REHEARSAL_SLUG`, then `exit=0`. For anything else, run no exp05-evidence line: ask the owner for REHEARSAL_SLUG's disc and run the check again. If it fails the same way again with the owner sure that the right disc is in, stop and report.

Then run:

```bash
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\exp05-evidence.ps1 -Slug zz-exp05-test -Listing 0-before"; echo "exit=$?"
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\exp05-evidence.ps1 -Slug zz-exp05-test -Listing 1-after-drives -DriveList drives"; echo "exit=$?"
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\exp05-evidence.ps1 -Slug zz-exp05-test -Listing 1-after-drives -DriveList drives"; echo "exit=$?"
ssh USER@makemkv-box "type C:\lazymkv\phase0\recordings\zz-exp05-test\drives.txt"
ssh USER@makemkv-box "type C:\lazymkv\phase0\recordings\zz-exp05-test\makemkv-data-0-before.txt"
ssh USER@makemkv-box "fc C:\lazymkv\phase0\recordings\zz-exp05-test\makemkv-data-0-before.txt C:\lazymkv\phase0\recordings\zz-exp05-test\makemkv-data-1-after-drives.txt"
ssh USER@makemkv-box "rmdir /s /q C:\lazymkv\phase0\recordings\zz-exp05-test"
```

Expected:
1. `listing: makemkv-data-0-before.txt files=<n> folders=<one or more paths>`, `RESULT ok`, `exit=0`. Write down DATA_FOLDERS (the `folders=` value).
   - `folders=NONE-FOUND` means MakeMKV's data folder is in none of the places the script looks. Stop and report. The controller then finds it (for example from the owner, in MakeMKV's preferences) and adds it to `$folders` in the script, and this step is run again. The before/after listings of Disc A cannot be taken later.
2. A `drives: drives.txt exit.json={"exit_code":<n>,"elapsed_ms":<n>,"lines":<n>} version_banner=True state=2 label=<the disc's label> other_messages=<codes or none>` line, then a `listing: ...` line, `RESULT ok` and `exit=0`.
   - `RESULT FAILED: no readable disc in D: after 120 s; MakeMKV was not started`: ask the owner whether REHEARSAL_SLUG's disc is in and the light has stopped, then run the disc check and line 2 again.
   - `state=` other than 2: run the `rmdir` line, ask the owner the same, and run the disc check and lines 1 and 2 again.
   - Any other failure is a defect in the script: stop and report.
3. Within seconds, `RESULT FAILED: C:\lazymkv\phase0\recordings\zz-exp05-test\makemkv-data-1-after-drives.txt already exists and is kept as evidence; choose another name`, then `exit=1`. MakeMKV was not started. If this run succeeds instead, the guard does not hold: stop and report.
4. `drives.txt` shows the `MSG:1005` banner and the `DRV` lines. Note any other `MSG` lines and whether they come before or after the `DRV` lines.
5. The listing shows the `app_DataDir:` line and one `folder:` line per candidate folder. Each file line shows only a time, a size and a path, never any file content.
6. `fc` prints `FC: no differences encountered`, or the lines that differ. Those files were written or changed by the drive listing.

Write down PROBE_ANSWER: `on a disc MakeMKV had <never|already> scanned on this machine, the drive listing printed <only the banner and DRV lines | MSG codes ..., before/after the DRV lines> and changed <nothing | these files: ...> in MakeMKV's data folders`. Whatever it shows, Disc A's drive listing is recorded in Step 5 before any scan, so MakeMKV's first messages about Disc A are kept either way.

Ask the owner to take the rehearsal disc out, keep the drive empty and keep Disc A at hand. Then:

```bash
ssh USER@makemkv-box "vol D:"
git add scripts/phase0/exp05-evidence.ps1
git commit -m "Add the experiment 5 evidence script" -m "Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

Expected: `vol` prints `The device is not ready.` (the drive is empty), and the commit succeeds.

- [ ] **Step 3: Choose Disc A's slug**

Which disc is in the drive: none. Disc A stays out of the drive.

Ask the owner: "Which film is Disc A (title and year)? Please confirm that it is a UHD that has never been in this mini PC's drive while MakeMKV was running, GUI or command line. Keep it out of the drive until I ask for it." Choose SLUG_A from the title, year and format, for example `dune-2021-uhd`. Then check that it is free:

```bash
ls -d corpus/owner/SLUG_A
ssh USER@makemkv-box "if exist C:\lazymkv\phase0\recordings\SLUG_A (echo EXISTS) else (echo absent)"
```

Expected: `ls` reports `No such file or directory`, and ssh prints `absent`. If either exists, choose another slug. If the owner is not sure that MakeMKV has never seen the disc, stop and report: experiment 5 needs a UHD that MakeMKV has never scanned on this machine.

- [ ] **Step 4: Ask the owner to block MakeMKV's network access**

Which disc is in the drive: none. Disc A stays out of the drive.

First check that no earlier rule is left over:

```bash
ssh USER@makemkv-box "netsh advfirewall firewall show rule name=lazymkv-exp05"
```

Expected: `No rules match the specified criteria.` If a rule is listed, ask the owner to remove it first with `Remove-NetFirewallRule -DisplayName 'lazymkv-exp05'` in an administrator PowerShell, then check again.

Ask the owner to run this in an administrator PowerShell on the mini PC and to say when it is done. The agent never changes the firewall itself.

```powershell
New-NetFirewallRule -DisplayName 'lazymkv-exp05' -Direction Outbound -Program 'C:\Program Files (x86)\MakeMKV\makemkvcon64.exe' -Action Block
```

Verify (read-only):

```bash
ssh USER@makemkv-box "netsh advfirewall firewall show rule name=lazymkv-exp05 verbose"
ssh USER@makemkv-box "netsh advfirewall show currentprofile state"
```

Expected:
- Exactly one rule block, starting `Rule Name: lazymkv-exp05`. It shows:
  - `Enabled: Yes`, `Direction: Out` and `Action: Block`;
  - `Program: C:\Program Files (x86)\MakeMKV\makemkvcon64.exe`;
  - a `Profiles:` list that includes the current profile (`Domain,Private,Public` for the command above).
- The second command prints `State ON` for every profile it lists.

Anything else means stop: no rule, two rules, `Action: Allow`, another program path, inbound direction, or `State OFF`. Tell the owner what differs, and do not ask for Disc A until the owner has fixed it and the check passes. Write down RULE_ANSWER: `one rule, Out, Block, <program>, profiles <list>; current profile <name> State ON`.

- [ ] **Step 5: First sight of Disc A: the snapshot, then the drive listing, offline**

Which disc is in the drive: none at first. Disc A is inserted in this step. Disc A's snapshot is taken first (no MakeMKV), so record-disc.ps1's Assert-DiscIs proves the disc before the offline scan (Step 6) and the online scan (Step 8). The snapshot's `meta.json` also makes exp05-evidence.ps1 run `Assert-DiscIs SLUG_A` before the offline drive listing below and the online one (Step 8). So every MakeMKV run on Disc A is preceded by a fingerprint check; Step 9's label comparison is only a cross-check.

MakeMKV's GUI must not see Disc A before the recorded runs do: spec 10.4 notes that the GUI holds the drive. Before the disc goes in:

```bash
ssh USER@makemkv-box "tasklist | findstr /i makemkv"
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\exp05-evidence.ps1 -Slug SLUG_A -Listing 0-before"; echo "exit=$?"
```

Expected:
- `tasklist | findstr` prints nothing. If it prints a row (`makemkv.exe`, `makemkvcon64.exe`), ask the owner to close the MakeMKV window, and check again. Do not stop the process yourself.
- `listing: makemkv-data-0-before.txt files=<n> folders=DATA_FOLDERS`, `RESULT ok`, `exit=0`. `exit=1`: stop and report, before Disc A goes in. `exit=255`: ssh could not reach the mini PC; run the line again.

Ask the owner: "Please make sure no MakeMKV window is open, then insert Disc A and tell me when the drive's light has stopped blinking. Did anything open by itself when the disc went in, such as a MakeMKV window, a player or an AutoPlay prompt? If so, please close it (choose 'Take no action' for a prompt)." Then:

```bash
ssh USER@makemkv-box "tasklist | findstr /i makemkv"
```

Run the tasklist line on its own first. Run the lines after it (the snapshot block and the drive listing below) only if it printed nothing. Here a printed row is not just a window to close: it is the second case below.

Expected:
- `tasklist | findstr` prints nothing.
  - If the owner says a MakeMKV window opened by itself, or a MakeMKV row is printed, MakeMKV may have seen Disc A before this recording. Stop and report; run nothing else on Disc A until the controller decides.
  - A player or an AutoPlay prompt that the owner closed does not matter: it is not MakeMKV.
  - Write down GUI_ANSWER: `no MakeMKV process before or after insertion; on insertion <nothing | what> opened`.

Take Disc A's snapshot and check that it was never recorded before. With `-NoScan`, record-disc.ps1 runs lazymkv-capture and curl only, never MakeMKV, so this can be done before MakeMKV's first contact:

```bash
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\record-disc.ps1 -Slug SLUG_A -NoScan"; echo "exit=$?"
scripts/phase0/pull-recording.sh SLUG_A
python3 -c "import json,pathlib; [print(p.parent.name, json.loads(p.read_text(encoding='utf-8-sig')).get('fingerprint')) for p in sorted(pathlib.Path('corpus/owner').glob('*/meta.json'))]"
```

Expected:
- `disc: label=<volume label or (none)> format=bluray fingerprint=<32 hex characters> disc_id=<40 hex characters, or empty> files=<N>`, `lookups: dischash <code>[; discid <code>][; tmdb <movie|tv>/<id> <code>][; tmdb not tried: <reason>]`, `RESULT ok`, `exit=0`. A UHD's `format` is `bluray`. The lookups go to TheDiscDB and TMDb through curl, which the rule does not block.
- `pulled corpus/owner/SLUG_A (N files copied, N files there now)`. If the pull fails, nothing on the Mac changed; run it again. Do not go on until it has succeeded, because the next check reads the pulled `meta.json`.
- The list shows SLUG_A's fingerprint exactly once. If another slug has it, Disc A was recorded before: stop and report now, before MakeMKV sees it.

Other results of the first line (MakeMKV has not seen Disc A in any of them):
- `RESULT FAILED: no readable disc in D: after 120 s`: ask the owner, then run the first line again.
- `RESULT FAILED: lazymkv-capture disc failed (exit N)` or `RESULT FAILED: the disc has no fingerprint ...`: stop and report. Leave Disc A in and the block on.
- `disc: kept from the earlier recording ...`, or a `format` other than `bluray`: this is not a fresh UHD recording. Stop and report.
- `exit=255`: ssh could not reach the mini PC; run the first line again.

Only when the tasklist check, the snapshot, the pull and the fingerprint list have all passed, run MakeMKV's first contact with Disc A:

```bash
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\exp05-evidence.ps1 -Slug SLUG_A -Listing 1-after-drives-offline -DriveList drives-offline"; echo "exit=$?"
```

Expected:
- `drives: drives-offline.txt exit.json={...} version_banner=True state=2 label=<Disc A's label> other_messages=<codes or none>`, then `listing: makemkv-data-1-after-drives-offline.txt ...`, `RESULT ok`, `exit=0`. This was MakeMKV's first contact with Disc A, with the network blocked and the output recorded.

Other results:
- `RESULT FAILED: no readable disc in D: after 120 s; MakeMKV was not started`: nothing about the disc was written. Ask the owner whether Disc A is in and the light has stopped, then run the line again.
- `RESULT FAILED: ... is not the disc of recording SLUG_A` (Assert-DiscIs): the disc in the drive is not the one just snapshotted. MakeMKV was not started and nothing was written. Ask the owner to put Disc A back and say when the light has stopped blinking, then run the line again. If the owner says Disc A was in the drive all along, stop and report instead: the snapshot may be of another disc.
- Any other `RESULT FAILED`: stop and report. Leave Disc A in and the block on. Do not start the offline job until the controller decides: the files kept may already hold MakeMKV's first messages about Disc A.
- `exit=255`: ssh could not reach the mini PC; run the line again.

- [ ] **Step 6: Record Disc A offline**

Which disc is in the drive: Disc A, inserted in Step 5. The block from Step 4 is on. record-disc.ps1 checks the disc itself with `Assert-DiscIs SLUG_A` before it starts MakeMKV, because the recording has a `meta.json` since Step 5.

`-ScanMayFail` is used because the offline scan may legitimately list no titles: the scan then only has to prove that MakeMKV ran. Never pass `-Force` here: `scan-offline` is a first-sight scan.

```bash
ssh USER@makemkv-box "tasklist | findstr /i makemkv"
```

Run the tasklist line on its own first. Run the lines after it only if it printed nothing: job.sh must wait for the empty tasklist result. If it printed a row, do not start the job: ask the owner to close the MakeMKV window (never stop the process yourself) and run the tasklist line again. Note the row for the report.

```bash
scripts/phase0/job.sh record-disc.ps1 record-offline -Slug SLUG_A -ScanName scan-offline -ScanMayFail
scripts/phase0/wait.sh record-offline
```

job.sh prints `started record-disc.ps1; log: C:\lazymkv\phase0\logs\record-offline.txt`. If job.sh exits 1 because a phase 0 job is still running, stop and report: no other job should run during this task.

What wait.sh's exit codes mean for this job:
- **0**: the job finished and record-disc.ps1 did not throw. Check the log against the Expected below. The `RESULT` line is the verdict. `exit=<n>` on the DONE line is only the last native command's exit code.
- **3**: still running. This is normal: a UHD scan with Java analysis takes ten minutes or more, and offline MakeMKV may also wait for network timeouts. Run `scripts/phase0/wait.sh record-offline` again. Start no other job, run no other MakeMKV command, and do not go on to Step 7 while it runs.
  If 45 minutes pass after job.sh without a PHASE0-DONE line, tell the owner and end the job:
  1. Run `ssh USER@makemkv-box "powershell -NoProfile -Command Stop-ScheduledTask -TaskName lazymkv-phase0"`.
  2. `ssh USER@makemkv-box "tasklist | findstr /i makemkvcon"` must then print nothing. If it prints a row, stop and report.
  3. `scripts/phase0/wait.sh record-offline` then exits 4.

  The partial `scan-offline.txt`, which has no `.exit.json`, is still the evidence: go on with the rest of this step, and the report says the offline scan was ended after 45 minutes.
- **1**: the script threw. Read the `RESULT FAILED:` line and look for a `scan:` line above it:
  - `is not the disc of recording SLUG_A` (Assert-DiscIs, before any MakeMKV call): the disc was changed. Ask the owner for Disc A, then start the job again as `record-offline-b` (and wait with that name); no scan file was written.
  - No `scan:` line: MakeMKV has not scanned Disc A yet. With `no readable disc in D: after 120 s`, ask the owner whether Disc A is still in, then start the job again under the log name `record-offline-b` (and wait with that name). For any other reason (for example a failure inside Assert-DiscIs while it reads the disc, or `MakeMKV lists no drive at D:`), stop and report. Once the controller has fixed the cause, start the job again as `record-offline-b`, with the block still on and Disc A still in.
  - A `scan:` line is present (`the scan did not finish` or `MakeMKV did not run`): MakeMKV has run the scan. Never run the offline job again, with or without `-Force`. Stop and report, and leave the block on and Disc A in until the controller decides.
- **4**: the job is not running and wrote no PHASE0-DONE line. If you ended it under the 45-minute rule, go on as described there. Otherwise check that the owner is logged in at the mini PC's console, look with `ssh USER@makemkv-box "dir C:\lazymkv\phase0\recordings\SLUG_A"` for a `scan-offline.txt`, and stop and report.
- **5**: the mini PC cannot be reached. The job keeps running there; run wait.sh again once it can be reached.

Expected (exit 0), in the log:

```text
disc: kept from the earlier recording of this same disc: label=... format=bluray fingerprint=<the same as in Step 5> disc_id=... files=...
lookups: kept: dischash <code>; ...
scan: scan-offline.txt exit.json={"exit_code":<n>,"elapsed_ms":<n>,"lines":<n>} version_banner=True titles=<n, 0 or NO-TCOUNT-LINE>
RESULT ok
PHASE0-DONE status=ok exit=<n>
```

The `RESULT` line may instead read `RESULT ok (MakeMKV ran; the scan lists no titles, accepted because of -ScanMayFail)`. That is a result for experiment 5, not a failure. The snapshot and lookups were made in Step 5 and are kept, not repeated.

Then take the third data-folder listing, and copy what exists so far to the Mac:

```bash
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\exp05-evidence.ps1 -Slug SLUG_A -Listing 2-after-scan-offline"; echo "exit=$?"
scripts/phase0/pull-recording.sh SLUG_A
ssh USER@makemkv-box "tasklist | findstr /i makemkv"
```

Expected:
- `listing: makemkv-data-2-after-scan-offline.txt ...`, `RESULT ok`, `exit=0`. With `exit=1`, note the `RESULT FAILED` line for the report and go on: a missing listing loses one comparison, not a scan.
- `pulled corpus/owner/SLUG_A (N files copied, N files there now)`. If the pull fails, nothing on the Mac changed; Step 9 pulls again.
- `tasklist | findstr` prints nothing.

- [ ] **Step 7: Ask the owner to remove the block**

Which disc is in the drive: Disc A. It stays in.

Only after Step 6 has finished: wait.sh exited 0, or exited 4 after the 45-minute rule, and no MakeMKV process is left. Ask the owner to run this in an administrator PowerShell and to say when it is done:

```powershell
Remove-NetFirewallRule -DisplayName 'lazymkv-exp05'
```

Verify (read-only):

```bash
ssh USER@makemkv-box "netsh advfirewall firewall show rule name=lazymkv-exp05"
```

Expected: `No rules match the specified criteria.` If a rule is still listed, ask the owner to run the command again and check again. Do not start Step 8 while a rule is listed.

- [ ] **Step 8: Record Disc A online**

Which disc is in the drive: Disc A. Both exp05-evidence.ps1 (with -DriveList) and record-disc.ps1 check this themselves with Assert-DiscIs SLUG_A before they start MakeMKV, because the recording now has a meta.json.

```bash
ssh USER@makemkv-box "tasklist | findstr /i makemkv"
```

Run the tasklist line on its own first. Run the lines after it only if it printed nothing: both the drive listing and job.sh must wait for the empty tasklist result. If it printed a row, do not start the drive listing or the job: ask the owner to close the MakeMKV window (never stop the process yourself) and run the tasklist line again.

```bash
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\exp05-evidence.ps1 -Slug SLUG_A -Listing 3-after-drives-online -DriveList drives-online"; echo "exit=$?"
```

Expected before the job:
- `drives: drives-online.txt ... state=2 label=<the same label as in Step 5> ...`, then `listing: makemkv-data-3-after-drives-online.txt ...`, `RESULT ok` and `exit=0`. This online drive listing is the baseline for the offline one.
  - `RESULT FAILED: no readable disc in D: after 120 s; MakeMKV was not started`: ask the owner whether Disc A is in, then run the line again.
  - `RESULT FAILED: ... is not the disc of recording SLUG_A`: the disc in the drive is not Disc A. Nothing was written and MakeMKV was not started. Ask the owner to put Disc A back and say when the light has stopped blinking, then run the line again.
  - Any other `RESULT FAILED`: note it for the report and go on with the job. Step 9 then reports that the drive-listing comparison is missing.

Then start the online scan job:

```bash
scripts/phase0/job.sh record-disc.ps1 record-online -Slug SLUG_A
scripts/phase0/wait.sh record-online
```

Do not pass `-ScanMayFail` or `-Force`: the online scan must list titles. job.sh prints `started record-disc.ps1; log: C:\lazymkv\phase0\logs\record-online.txt`.

What wait.sh's exit codes mean for this job:
- **0**: the job finished. Check the log against the Expected below.
- **3**: still running. This is normal for a UHD, ten minutes or more. Run `scripts/phase0/wait.sh record-online` again and start nothing else meanwhile. If 45 minutes pass without a PHASE0-DONE line, end the job as in Step 6 (Stop-ScheduledTask, then `tasklist | findstr /i makemkvcon` prints nothing), then stop and report: Tasks 10 to 14 need an online scan.
- **1**: the script threw. Read the `RESULT FAILED:` line:
  - `is not the disc of recording SLUG_A` (Assert-DiscIs): the disc in the drive is not the one recorded offline. Ask the owner to put Disc A back, then start the job again as `record-online-b`. No scan file was written.
  - Any other reason with no `scan:` line above it (for example a failure inside Assert-DiscIs while it reads the disc): MakeMKV has not scanned. Stop and report. Once the controller has fixed the cause, start the job again as `record-online-b`.
  - A `scan:` line is present, for example `the scan lists no titles (titles=0)` or `the scan did not finish`: MakeMKV could not list Disc A even online. That is a result for experiment 5, and it blocks Tasks 10 to 14. Stop and report. Never rerun with `-Force`.
- **4**: the job is not running and wrote no PHASE0-DONE line. Check that the owner is logged in at the mini PC's console, then stop and report.
- **5**: the mini PC cannot be reached. Run wait.sh again once it can be.

Expected (exit 0), in the log:

```text
disc: kept from the earlier recording of this same disc: label=... format=bluray fingerprint=<the same 32 hex characters as in Steps 5 and 6> disc_id=... files=...
lookups: kept: dischash <code>; ...
scan: scan.txt exit.json={"exit_code":<n>,"elapsed_ms":<n>,"lines":<n>} version_banner=True titles=<n above 0>
RESULT ok
PHASE0-DONE status=ok exit=<n>
```

Then take the last data-folder listing:

```bash
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\exp05-evidence.ps1 -Slug SLUG_A -Listing 4-after-scan-online"; echo "exit=$?"
```

Expected: `listing: makemkv-data-4-after-scan-online.txt ...`, `RESULT ok`, `exit=0`. With `exit=1`, note the `RESULT FAILED` line for the report.

- [ ] **Step 9: Pull the recording and compare the offline and online runs**

Which disc is in the drive: Disc A. It stays in for Tasks 10 to 14; this step does not touch it.

Run these one after another, not chained with `&&`. Several of them exit 1 when their answer is "different", and that is a result, not an error. A Python traceback is an error: stop and report.

```bash
scripts/phase0/pull-recording.sh SLUG_A
python3 -c "import json,sys; print(json.load(open('corpus/owner/'+sys.argv[1]+'/meta.json', encoding='utf-8-sig')))" SLUG_A
grep -i -A1 unit_key corpus/owner/SLUG_A/files.json
python3 -c "import json,pathlib; [print(p.parent.name, json.loads(p.read_text(encoding='utf-8-sig')).get('fingerprint')) for p in sorted(pathlib.Path('corpus/owner').glob('*/meta.json'))]"
grep -H . corpus/owner/SLUG_A/*.exit.json
python3 scripts/phase0/summarize_scan.py corpus/owner/SLUG_A
python3 scripts/phase0/compare_scans.py corpus/owner/SLUG_A/scan-offline.txt corpus/owner/SLUG_A/scan.txt; echo "compare_scans exit=$?"
```

Then compare the messages of the offline runs with those of the online runs. SLUG_A is a command-line argument here, so nothing inside the heredoc needs editing:

```bash
python3 - SLUG_A <<'EOF'
import pathlib, sys
sys.path.insert(0, "scripts/phase0")
from robot import parse, split_fields
d = pathlib.Path("corpus/owner") / sys.argv[1]
TITLE_LIST = {3025, 3307, 3308, 3309}  # judged with compare_scans.py; only counted here
WATCH = {1009, 1011, 2003, 3007, 3032, 3303, 3338, 3339, 3344}

def read(name):
    path = d / name
    if not path.exists():
        print(f"{name}: MISSING")
        return ""
    return path.read_text(encoding="utf-8-sig", errors="replace")

def messages(text):
    """(code, text, where) per MSG line, in order: where is 'start' before the first DRV line, 'disc' after it."""
    found, where = [], "start"
    for raw in text.split("\n"):
        line = raw.rstrip("\r")
        if line.startswith("DRV:"):
            where = "disc"
        elif line.startswith("MSG:"):
            fields = split_fields(line[4:])
            try:
                found.append((int(fields[0]), fields[3], where))
            except (ValueError, IndexError):
                continue
    return found

for offline, online in (("drives-offline.txt", "drives-online.txt"), ("scan-offline.txt", "scan.txt")):
    texts = {name: read(name) for name in (offline, online)}
    msgs = {name: messages(text) for name, text in texts.items()}
    pairs = {name: {(c, t) for c, t, _ in found if c != 1003} for name, found in msgs.items()}
    for name, other in ((offline, online), (online, offline)):
        scan = parse(texts[name])
        print(f"== {name}: titles={len(scan.titles)} messages={len(msgs[name])} (compared with {other})")
        for fields in scan.drives:
            if len(fields) > 1 and fields[1] != "256":  # 256: no drive at this index
                print("   DRV", ",".join(fields))
        hidden = 0
        for code, text, where in msgs[name]:
            if code == 1003:
                continue  # DEBUG lines differ on every run
            only_here = (code, text) not in pairs[other]
            if code in TITLE_LIST:
                hidden += only_here
                continue
            if only_here or code >= 5000 or code in WATCH:
                print("  ", "ONLY HERE" if only_here else "both     ", where, code, text)
        print(f"   title-list messages (3025, 3307, 3308, 3309) only in {name}: {hidden}; compare_scans.py judges the titles")
EOF
```

Then the data-folder listings, each against the one before it:

```bash
grep -i -E 'keydb|^folder|app_DataDir' corpus/owner/SLUG_A/makemkv-data-0-before.txt
diff corpus/owner/SLUG_A/makemkv-data-0-before.txt corpus/owner/SLUG_A/makemkv-data-1-after-drives-offline.txt
diff corpus/owner/SLUG_A/makemkv-data-1-after-drives-offline.txt corpus/owner/SLUG_A/makemkv-data-2-after-scan-offline.txt
diff corpus/owner/SLUG_A/makemkv-data-2-after-scan-offline.txt corpus/owner/SLUG_A/makemkv-data-3-after-drives-online.txt
diff corpus/owner/SLUG_A/makemkv-data-3-after-drives-online.txt corpus/owner/SLUG_A/makemkv-data-4-after-scan-online.txt
```

Expected:
- `pulled corpus/owner/SLUG_A (N files copied, N files there now)`. If it fails, nothing on the Mac changed; run it again.
- `meta.json` printed with a 32-character `fingerprint`, `format` `bluray`, and `volume_label` equal to the label in the `D:` DRV line of `drives-offline.txt`, which the heredoc prints. Equal labels are a cross-check that the drive listing saw the same disc. Note whether `disc_id` is 40 hex characters or None. If the two labels differ, do not stop. A `volume_label` of `(none)` equals an empty DRV label, and MakeMKV's label can legitimately differ from the .NET volume label that record-disc.ps1 stores. Write both labels in the EXP-05 report under "What was observed". The identity of each MakeMKV run rests on Assert-DiscIs, which ran before both drive listings (Steps 5 and 8) and both scans (Steps 6 and 8), not on the label.
- `grep -A1 unit_key` prints the `"path": ".../Unit_Key_RO.inf"` line and the `"size": <n>` line below it when the drive lists the file, and nothing when it does not.
- The fingerprint list shows SLUG_A's fingerprint exactly once. Step 5 checked this before MakeMKV first saw Disc A; this is a re-check. If another slug has the same fingerprint, Disc A was recorded before and MakeMKV may have scanned it. Stop and report, because the first sight would then be unproven.
- `grep -H` prints one `{"exit_code":...,"elapsed_ms":...,"lines":...}` line for each of `drives-offline.txt`, `drives-online.txt`, `scan-offline.txt` and `scan.txt`. A missing one means that run did not finish, for example the offline scan under the 45-minute rule.
- The summary starts `MakeMKV: MakeMKV v2.0.0 win(x64-release)` and lists the online scan's titles.
- `compare_scans.py` prints `titles: A=<offline> B=<online> common=<n>` and the titles found in only one scan. `exit=0` means the offline scan listed the same titles. `exit=1` means they differ, or the offline scan has none (`EMPTY SCAN`). Either is a result to report.
- The heredoc prints four sections: the two drive listings, then the two scans.
  - Each message line is marked `ONLY HERE` (the same code and text are absent from the paired online or offline run) or `both`. It also says where the message was printed: `start` (before MakeMKV listed the drives, at its own start-up, the same for any disc) or `disc` (after it, once the disc is being opened).
  - Every message only in one run is printed with its full text. So is every message in the 5000 range, and every message with code 1009, 1011, 2003, 3007, 3032, 3303, 3338, 3339 or 3344, in either run.
  - Title-list messages are only counted, because compare_scans.py judges the titles.
  - A `MISSING` line means that file was not recorded; that comparison is then reported as not made.
- `grep` shows the folders, the `app_DataDir` value, and whether a `keydb.cfg` existed before MakeMKV first saw Disc A. Each `diff` prints nothing when MakeMKV's data folders did not change across that run. Otherwise it prints the files that appeared (`>`), disappeared (`<`) or changed (both). It exits 1 when they differ; that is a result.

- [ ] **Step 10: Write the experiment 5 report**

Which disc is in the drive: Disc A (not used).

Create `docs/phase0/experiments/EXP-05-offline-and-unit-key.md` from `docs/phase0/experiments/TEMPLATE.md`. Fill in the header:
- `Disc(s): SLUG_A, UHD (meta.json format bluray), drive 1 (D:, HL-DT-ST BD-RE BU40N)`;
- the MakeMKV version from the `MSG:1005` line;
- `Spec reference: section 17, row 5`.

Under "Question", give section 17 row 5's question.

Under "What was run", record:
- the rule as verified (RULE_ANSWER), and its removal as verified in Step 7;
- GUI_ANSWER;
- the order of the MakeMKV runs and listings:
  1. listing 0;
  2. Disc A inserted, its snapshot and lookups taken with `record-disc.ps1 -NoScan` (lazymkv-capture and curl, no MakeMKV), and its fingerprint found in no other recording;
  3. the drive listing offline, then listing 1;
  4. the offline scan job `record-offline` (with `-ScanMayFail`), then listing 2;
  5. the rule removed;
  6. the drive listing online, then listing 3;
  7. the online scan job `record-online`, then listing 4;
- that `Assert-DiscIs SLUG_A` checked the disc's fingerprint before each of the four MakeMKV runs;
- the rehearsal on REHEARSAL_SLUG's disc in Step 2, and PROBE_ANSWER.

Under "What was observed", record:
- For both scans and both drive listings: the exit code, the elapsed time in minutes and the line count from `.exit.json`. Say whether the offline scan was ended under the 45-minute rule. The offline and online durations side by side show whether the blocked network made MakeMKV wait, which is the open question of whether the firewall refuses at once or by timeout.
- The titles: the offline and online title counts, and compare_scans.py's result, quoting `titles: A=... B=... common=...` and its exit code.
- Every `ONLY HERE` message of `drives-offline.txt` and `scan-offline.txt`, with its code, its full text exactly as printed, and `start` or `disc`. Also list any `ONLY HERE` message of the online runs.
- Every `3338` ("Downloading latest ...") line in the offline runs, and what follows it: a failure such as `3339`, or nothing.
- Whether the drive listing opened Disc A: any message after its `DRV` lines, and the listing 0 to 1 diff. Compare this with PROBE_ANSWER.
- If `meta.json`'s `volume_label` and the label in the `D:` DRV line of `drives-offline.txt` differed in Step 9, both labels.
- MakeMKV's data folders: DATA_FOLDERS and the `app_DataDir` value. Say which data files existed before MakeMKV first saw Disc A, for example whether a `keydb.cfg` was present. Then say which files appeared or changed after each of the four runs, from the four diffs. Name the files only, never their contents. This replaces the old assumption that the data folder "already holds keydb.cfg and cached keys". A `keydb.cfg` present before the first sight can let a disc open offline. A file that the offline scan wrote, and the online scan then read, means the online scan was not a clean baseline.
- Whether either scan printed `MSG:1009` (profile parsing error), which would mean MakeMKV rejected the job profile (spec 9.3).
- The disc ID:
  - `disc_id` 40 hex characters: `AACS/Unit_Key_RO.inf` was readable in this drive.
  - None, with `grep -A1 unit_key` showing the path and a size: the file exists but could not be read.
  - None, with no grep output: the drive does not expose the file.
- That only drive 1 (D:) was tested. The second-drive half of the question is not run here; Task 14 adds it to this report.

Under "Answer", decide the offline question with these rules, in order:
1. **The block did not hold.** An offline run (`drives-offline.txt` or `scan-offline.txt`) shows a `3338` "Downloading latest ..." line, and no message after it says that download failed (for the SDF download that is `3339`). The download may then have succeeded, and the block may not have held. The answer is `not answered: the network block may not have held (3338 "<text>" with no failure after it)`. Stop and tell the controller: experiment 5 then needs another UHD that MakeMKV has never scanned on this machine.
2. **Candidates.** A candidate is a message that meets all three of these:
   - it is marked `ONLY HERE` in `scan-offline.txt` or `drives-offline.txt`;
   - it is in the `disc` position;
   - it concerns downloading, keys, decryption or opening the disc, for example a `3338`/`3339` about HK or any code from 5000 up.

   A message printed by both an offline and an online run is not evidence. A `start` message is not a candidate. For example, the SDF download failing at start-up shows that MakeMKV noticed the network was gone, not that this disc needs it. Report such messages separately. Title-list messages are judged through compare_scans.py, not here.

   If there is at least one candidate, the answer is `yes, MakeMKV reports <code and full text of each candidate> when it cannot reach the network`. For each candidate, also say whether spec 9.4's message table already gives its code another meaning: `5010` is an open failure, and `5021`, `5050`, `5052`, `5055` and `5095` are licence messages.
3. **Same titles.** There is no candidate, and compare_scans.py exited 0. The answer is `no distinct message was seen; the disc opened offline`. Mention any offline-only `start` messages.
4. **Different titles.** There is no candidate, but compare_scans.py exited 1. The answer is `no distinct message was seen; the offline scan listed <A> titles against <B> online`. Mention any offline-only `start` messages.
5. **Ended after 45 minutes.** If the offline scan was ended under the 45-minute rule, judge rules 1 to 4 on what it printed before then, and add `the offline scan did not finish within 45 minutes`.

Then answer the second question: `AACS/Unit_Key_RO.inf` was, or was not, readable in drive 1, as above.

Under "Consequence for the spec":
- A yes gives `NeedsNetwork` (spec 10.2) its trigger: the candidate codes that spec 9.4 does not already give another meaning. The sentence in 10.2 "Experiment 5 captures `NeedsNetwork`" is replaced by those codes. If every candidate already has another meaning (for example only `5010`), say that offline cannot be told apart by message: `NeedsNetwork` stays `Unknown`, as the spec's fallback says.
- A no (rules 3 and 4) leaves `NeedsNetwork` `Unknown`, as the spec's fallback says.
- Under rule 5, add that spec 10.4's scan watchdog (`scan_timeout_min`), not a message, is what catches an offline UHD.
- If the file is unreadable in this drive, lookups use the fingerprint only (identity 0.95 at best from TheDiscDB), as row 5's fallback says. Task 14 completes this for the second drive.

- [ ] **Step 11: Write the experiment 12 report**

Which disc is in the drive: Disc A (not used).

Create `docs/phase0/experiments/EXP-12-java-labels.md` from `docs/phase0/experiments/TEMPLATE.md`, with `Disc(s): SLUG_A, UHD, drive 1 (D:)`, the MakeMKV version and `Spec reference: section 17, row 12`. Use the online summary from Step 9 (`summarize_scan.py`, which reads `scan.txt`).

Record:
- the `Java in use` line: the path MakeMKV printed in `MSG:3344`, or its absence. `docs/phase0/environment.md` notes that Java 8 is installed and MakeMKV's `app_Java` setting is unset. Also say whether the offline scan printed `3344`; the heredoc in Step 9 shows it;
- every title whose comment column is not empty, with its source file.

Answer "yes" only if `MSG:3344` is present and at least one title's comment starts `FPL_MainFeature`. If Disc A has no Java menus (no labels and no `3344`), say so and repeat this check on each later UHD until one answers it; the report is updated then (Task 15 Step 6). Consequence if no: label rule 8.3.2 is unavailable, and titles are chosen by database and structure only (section 17 row 12).

- [ ] **Step 12: Commit**

Which disc is in the drive: Disc A. Leave it in: Tasks 10 to 14 use it.

```bash
find corpus/owner -iname '*unit_key*'
grep -rIl 'eyJhbGciOi' corpus/owner
git add corpus/owner/SLUG_A docs/phase0/experiments/EXP-05-offline-and-unit-key.md docs/phase0/experiments/EXP-12-java-labels.md
git diff --cached --name-only
git commit -m "Record Disc A offline and online; report experiments 5 and 12" -m "Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

Expected:
- `find` and `grep` print nothing: no AACS unit-key file, and no JSON Web Token (the TMDb read access token is one) in any recording. If either prints a path, stop and do not commit.
- `git diff --cached --name-only` lists only files under `corpus/owner/SLUG_A/` and the two reports.
  - `corpus/owner/SLUG_A/` holds `meta.json`, `files.json`, `nav/...`, the lookup files, the four `scan*`/`drives*` outputs with their companion files, and the five `makemkv-data-*.txt` listings.
  - The listings hold file names, sizes and times only.
  - `scripts/phase0/exp05-evidence.ps1` was committed in Step 2.
- The commit succeeds.

---

### Task 10: Index guard (experiment 2)

**Files:**
- Create: `scripts/phase0/exp02-index-guard.ps1` (Step 1).
- Create: `corpus/owner/SLUG/exp02/`, the results for Disc A, copied from the mini PC in Steps 5 and 6.
- Create: `docs/phase0/experiments/EXP-02-index-guard.md` (Step 7).

**Interfaces:**
- Consumes:
  - From `env.ps1`: `$Recordings`, `$Staging`, `$Drive`, `$MK`, `$AllTracksProfile`, `Get-MkFlags`, `Get-DiscIndex`, `Assert-Slug`, `Assert-DiscIs`, `Assert-EmptyDir` (decision D1).
  - Disc A's recording from Task 9, in `$Recordings\<slug>\` on the mini PC:
    - `meta.json`: its `fingerprint` is what Assert-DiscIs compares with the inserted disc.
    - `scan.txt`: the online scan, made with `Get-MkFlags`. Its `TINFO` lines give the title numbers, and its `MSG:3308` and `MSG:3309` lines are counted.
    - `scan.txt.exit.json`: its `elapsed_ms` sets the time limit and the estimate.
  - Scripts:
    - `scripts/box.sh`, `scripts/phase0/job.sh` and `scripts/phase0/wait.sh`.
    - `scripts/phase0/pull-recording.sh <slug>`, Task 8's version: it copies over `corpus/owner/<slug>/` and never deletes anything.
    - `scripts/phase0/exp02_check.py <recording dir>`: it reads only `exp02/title-[0-9]*.txt`, and exits 0 when its last line is `EXPERIMENT 2: yes` and 1 when it is `EXPERIMENT 2: no`.
- Produces:
  - `scripts/phase0/exp02-index-guard.ps1 -Slug <slug> [-MaxTitles <n>] [-Redo <n,n,...>] [-Plan]`
    - Parameters:
      - `-Slug` is required. Assert-Slug checks it, and an empty value fails.
      - `-MaxTitles` (default 12, at least 2): test at most this many of the scan's titles, evenly spaced, with the first and last always included.
      - `-Redo`: test exactly these titles. Each must be a title of the scan.
      - `-Plan`: check everything, print the plan and start no MakeMKV call. It needs no share, so it also runs over plain ssh. Pass `-Redo` with two or more titles (`-Redo 5,9`) only to a job: over ssh with `-File` the list arrives as one string and fails to bind. A single title (`-Redo 5`) works either way, as in Step 2.
    - A real run (without `-Plan`) runs only as a job, `scripts/phase0/job.sh exp02-index-guard.ps1 <log> -Slug <slug> [...]`, because its calls write to the share.
    - It refuses before any MakeMKV call when:
      - a value is bad;
      - the recording has no `meta.json` or no `scan.txt`;
      - `scan.txt` has no `TINFO` lines;
      - a `-Redo` title is not in the scan;
      - `exp02` already holds title files and `-Redo` was not given. `-Plan` only reports them;
      - MakeMKV (the window or a console process) is running;
      - the disc in `$Drive` is not the recorded one (Assert-DiscIs);
      - `$MK` or the all-tracks profile is missing.
    - For each tested title it empties `$Staging\<slug>\exp02-tmp` (Assert-EmptyDir) and starts `makemkvcon64 <Get-MkFlags> mkv disc:<index> <title> <that folder>`. It reads the output until the `MSG:5014` line, kills the call, and waits 10 s before the next one.
    - A call that has written more than 512 MB, or has run longer than max(1800 s, three times the recorded scan time), without `MSG:5014` is stopped, and the run ends.
    - It writes in `$Recordings\<slug>\exp02\`:
      - `title-<n>.txt`: UTF-8 with a BOM. MakeMKV's standard output of that call, up to and including the `MSG:5014` line.
      - `title-<n>.txt.stderr`: only when MakeMKV wrote to standard error.
      - `run-<yyyyMMdd-HHmmss>.json`: one per run, written from the first MakeMKV call on and rewritten after every title. Keys:
        - `slug`, `started`, `fingerprint`, `command`, `time_limit_s`, `byte_limit`, `tested`;
        - `results`: one entry per title, with `title`, `saw_5014`, `ended`, `lines`, `written_bytes`, `exit_code`, `seconds` and `msg_codes`;
        - `result`: `running`, `ok` or `FAILED: <reason>`.
      - `superseded\<yyyyMMdd-HHmmss>\`: the earlier title files that a `-Redo` run moved aside. No evidence file is ever replaced, so the script has no `-Force` switch.
    - Output lines, in order:
      - `scan: ...`, `testing titles: ...`, `earlier results: ...`, `disc: ...`, `calls write into ...`, `limits per call: ...`;
      - in a real run, `run summary: ...` and then, for each title, any `kept the earlier ...` lines and `title <n>: lines=<L> stopped-at-5014=<True|False> written-bytes=<B> exit=<E> seconds=<S> codes=<list>[ STOPPED-EARLY: <why>]`;
      - possibly `WARNING: ...`, when the temporary folder could not be removed.
    - The last line is `RESULT ok: ...` or `RESULT FAILED: <reason>`. After a failure the script throws, so wait.sh exits 1 and an ssh `-File` run exits 1. `RESULT ok` requires every tested call to have reached `MSG:5014`.
  - Disc A's results in `corpus/owner/SLUG/exp02/`, and `docs/phase0/experiments/EXP-02-index-guard.md`. Task 15 Step 2 repeats Steps 3 to 6 of this task for the decoy-playlist disc, and adds that disc to the report.

Placeholders:
- SLUG: in this task, SLUG_A. That is Disc A's slug, chosen in Task 9 and written at the top of the EXP-05 and EXP-12 reports; `ls corpus/owner` shows it. When Task 15 Step 2 repeats Steps 3 to 6 for the decoy-playlist disc, SLUG is that disc's slug, and the log names below change with it.
- PLANNED, SCAN_3308, SCAN_3309, LIMIT_S, ESTIMATE: from the `-Plan` output in Step 3.
  - PLANNED: the title numbers after `testing titles:`.
  - SCAN_3308 and SCAN_3309: the two counts on the `scan:` line.
  - LIMIT_S: the seconds on the `limits per call:` line.
  - ESTIMATE: the minutes on the `limits per call:` line.
- K: a counter for redo runs: 1 for the first redo of SLUG, 2 for the next. Every log name is then new, so no log is overwritten.
- LIST: the title numbers to test again, comma-separated with no spaces (for example `5,9`), chosen in Step 5.
- T: one title number named in a `RESULT FAILED:` line or in a checker line.
- PID: the process id named in a `did not exit` message.
- WRITTEN_MAX: the largest `written_bytes` of a call stopped at `MSG:5014`, from the run summary in Step 6.

- [ ] **Step 1: Write the script**

Which disc is in the drive: none needed.

`scripts/phase0/exp02-index-guard.ps1`:

```powershell
# Experiment 2 (spec section 17, row 2): while a rip call scans the disc, does MakeMKV add each title
# under the index that was asked for, and before saving begins (MSG:5014)?
# For each tested title this script starts "mkv disc:<index> <title> <dir>" with the scan's flags, reads
# MakeMKV's output until the MSG:5014 line, stops the call there and keeps the output in
# $Recordings\<Slug>\exp02\title-<title>.txt, which exp02_check.py reads on the Mac.
#   -MaxTitles n  test at most n of the scan's titles, evenly spaced, the first and the last always included
#   -Redo 3,7     test exactly these titles; an earlier title-N.txt is moved to exp02\superseded\<time>\
#   -Plan         check the recording, the disc and MakeMKV, print the titles to test, start nothing
#                 (it needs no share, so it also runs over plain ssh)
# A run without -Redo refuses to start when exp02 already holds title files: they are evidence.
# The calls write into $Staging\<Slug>\exp02-tmp (never C:), emptied before each call and removed at
# the end. A call that writes more than 512 MB, or runs longer than the time limit, without MSG:5014
# is stopped, and the run ends there. From the first MakeMKV call on, each run writes
# exp02\run-<time>.json. The last line is "RESULT ok: ..." or "RESULT FAILED: <reason>"; after a
# failure the script throws, so wait.sh exits 1.
[CmdletBinding()]
param(
    [string]$Slug = '',    # required: Assert-Slug rejects an empty value (a Mandatory prompt would hang a job)
    [int]$MaxTitles = 12,
    [int[]]$Redo = @(),
    [switch]$Plan
)
. C:\lazymkv\src\scripts\phase0\env.ps1

$process = $null
$runFile = $null
$tmpUsed = $false
$results = New-Object System.Collections.Generic.List[object]
$stamp = (Get-Date).ToString('yyyyMMdd-HHmmss')

# Removes the temporary rip folder, retrying while a just-stopped call releases its file.
function Clear-Tmp {
    for ($i = 0; $i -lt 6 -and (Test-Path -LiteralPath $tmp); $i++) {
        Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue
        if (Test-Path -LiteralPath $tmp) { Start-Sleep -Seconds 5 }
    }
}

# Writes this run's summary. It is rewritten after every title, so an interrupted run still leaves one.
function Save-Run([string]$Result) {
    $doc = [ordered]@{
        slug         = $Slug
        started      = $startedAt
        fingerprint  = $fingerprint
        command      = "makemkvcon64 $flags mkv disc:$index <title> $tmp"
        time_limit_s = $limitSeconds
        byte_limit   = $limitBytes
        tested       = @($titles)
        results      = $results.ToArray()
        result       = $Result
    }
    Set-Content -LiteralPath $runFile -Value (ConvertTo-Json $doc -Depth 5) -Encoding UTF8 -ErrorAction Stop
}

try {
    Assert-Slug $Slug
    if ($MaxTitles -lt 2) { throw "-MaxTitles must be 2 or more (it was $MaxTitles)" }
    $rec = "$Recordings\$Slug"
    $out = "$rec\exp02"
    $tmp = "$Staging\$Slug\exp02-tmp"
    foreach ($name in 'meta.json', 'scan.txt') {
        if (-not (Test-Path -LiteralPath "$rec\$name")) { throw "no $name in $rec (record the disc first: Task 8 or Task 9)" }
    }

    # The scan's titles, in the scan's numbering, and its angle (3308) and duplicate (3309) messages.
    $scanLines = @(Get-Content -LiteralPath "$rec\scan.txt" -ErrorAction Stop)
    $all = @($scanLines | ForEach-Object { if ($_ -match '^TINFO:(\d+),') { [int]$Matches[1] } } | Sort-Object -Unique)
    if ($all.Count -eq 0) { throw "$rec\scan.txt has no TINFO lines" }
    $n3308 = @($scanLines | Where-Object { $_.StartsWith('MSG:3308,') }).Count
    $n3309 = @($scanLines | Where-Object { $_.StartsWith('MSG:3309,') }).Count
    "scan: $($all.Count) titles, numbers $($all[0]) to $($all[-1]); MSG:3308 lines=$n3308; MSG:3309 lines=$n3309"

    # Evenly spaced, always including the first and the last title.
    if ($all.Count -le $MaxTitles) {
        $planned = $all
    } else {
        $planned = @(0..($MaxTitles - 1) | ForEach-Object { $all[[int][math]::Round($_ * ($all.Count - 1) / ($MaxTitles - 1))] } | Sort-Object -Unique)
    }
    if ($Redo.Count -gt 0) {
        $unknown = @($Redo | Where-Object { $all -notcontains $_ })
        if ($unknown.Count -gt 0) { throw "-Redo names titles that $rec\scan.txt does not list: $($unknown -join ',')" }
        $titles = @($Redo | Sort-Object -Unique)
    } else {
        $titles = $planned
    }
    "testing titles: $($titles -join ', ') ($($titles.Count) of $($all.Count))"

    # Earlier results are evidence: a run without -Redo refuses to mix with them.
    $have = @{}
    foreach ($found in @(Get-ChildItem -LiteralPath $out -File -ErrorAction SilentlyContinue | Where-Object { $_.Name -match '^title-\d+\.txt$' })) {
        $have[[int]($found.Name -replace '^title-(\d+)\.txt$', '$1')] = [bool](Select-String -LiteralPath $found.FullName -Pattern '^MSG:501[45],' -Quiet)
    }
    if ($have.Count -eq 0) {
        'earlier results: none'
    } else {
        $todo = @(@($planned | Where-Object { -not $have.ContainsKey([int]$_) }) + @($have.Keys | Where-Object { -not $have[$_] }) | Sort-Object -Unique)
        $todoText = if ($todo.Count -gt 0) { $todo -join ',' } else { 'none' }
        "earlier results: $($have.Count) title file(s) in $out; titles still to test: $todoText"
        if ($Redo.Count -eq 0 -and -not $Plan) {
            throw "$out already holds results of an earlier run, which are kept as evidence; test the titles still to test ($todoText) with -Redo (Task 10 Step 5)"
        }
    }

    # Nothing else may be using MakeMKV or the drive.
    $running = @(Get-Process -Name makemkv, makemkvcon, makemkvcon64 -ErrorAction SilentlyContinue)
    if ($running.Count -gt 0) {
        $list = ($running | ForEach-Object { $_.ProcessName + ' pid ' + $_.Id }) -join '; '
        throw "MakeMKV is already running ($list): ask the owner to close the MakeMKV window; a leftover makemkvcon64 is ended with: taskkill /f /pid <pid>"
    }

    # The disc in the drive must be the recorded one, or the scan's titles say nothing about it.
    Assert-DiscIs $Slug
    $meta = Get-Content -LiteralPath "$rec\meta.json" -Raw -ErrorAction Stop | ConvertFrom-Json
    $fingerprint = $meta.fingerprint
    "disc: ${Drive}: holds the disc of recording $Slug (fingerprint $fingerprint)"
    if (-not (Test-Path -LiteralPath $MK)) { throw "missing $MK" }
    if (-not (Test-Path -LiteralPath $AllTracksProfile)) { throw "missing job profile $AllTracksProfile" }
    "calls write into $tmp (emptied before each call, removed at the end)"

    # Limits: no call may write much, or run much longer than the recorded scan, without MSG:5014.
    $limitBytes = 512MB
    $limitSeconds = 1800
    $scanSeconds = 0
    try {
        $scanExit = Get-Content -LiteralPath "$rec\scan.txt.exit.json" -Raw -ErrorAction Stop | ConvertFrom-Json
        $scanSeconds = [int]([double]$scanExit.elapsed_ms / 1000)
    } catch { }
    if (3 * $scanSeconds -gt $limitSeconds) { $limitSeconds = 3 * $scanSeconds }
    $estimate = if ($scanSeconds -gt 0) { "about $([math]::Ceiling($titles.Count * ($scanSeconds + 20) / 60)) minutes" } else { 'unknown (scan.txt.exit.json gives no scan time)' }
    "limits per call: $limitSeconds s, or $limitBytes bytes written, without MSG:5014; estimate: $estimate"
    if ($Plan) {
        'RESULT ok: plan only; no MakeMKV call was made'
        return
    }

    $tmpUsed = $true
    Clear-Tmp
    Assert-EmptyDir $tmp
    New-Item -ItemType Directory -Force -Path $out -ErrorAction Stop | Out-Null
    $index = Get-DiscIndex
    $flags = (Get-MkFlags) -join ' '
    $startedAt = (Get-Date).ToString('s')
    $runFile = "$out\run-$stamp.json"
    Save-Run 'running'
    "run summary: $runFile"

    foreach ($title in $titles) {
        $file = "$out\title-$title.txt"
        # -Redo: an earlier result is evidence too; it is moved aside, never replaced.
        $keep = "$out\superseded\$stamp"
        foreach ($old in @($file, "$file.stderr")) {
            if (Test-Path -LiteralPath $old) {
                New-Item -ItemType Directory -Force -Path $keep -ErrorAction Stop | Out-Null
                Move-Item -LiteralPath $old -Destination $keep -ErrorAction Stop
                "kept the earlier $(Split-Path -Leaf $old) in exp02\superseded\$stamp"
            }
        }
        Clear-Tmp
        Assert-EmptyDir $tmp

        $psi = New-Object System.Diagnostics.ProcessStartInfo
        $psi.FileName = $MK
        $psi.Arguments = "$flags mkv disc:$index $title `"$tmp`""
        $psi.UseShellExecute = $false
        $psi.CreateNoWindow = $true
        $psi.RedirectStandardOutput = $true
        $psi.RedirectStandardError = $true
        $psi.StandardOutputEncoding = [System.Text.Encoding]::UTF8
        $psi.StandardErrorEncoding = [System.Text.Encoding]::UTF8
        $started = Get-Date
        $process = [System.Diagnostics.Process]::Start($psi)
        $errRead = $process.StandardError.ReadToEndAsync()

        # Read line by line, but never wait more than 2 s for a line, so that the limits are checked.
        $lines = New-Object System.Collections.Generic.List[string]
        $saw5014 = $false
        $why = ''
        $lastCheck = $started
        $read = $process.StandardOutput.ReadLineAsync()
        while ($true) {
            if ($read.Wait(2000)) {
                $line = $read.Result
                if ($null -eq $line) { break }
                $lines.Add($line)
                if ($line.StartsWith('MSG:5014,') -or $line.StartsWith('MSG:5015,')) { $saw5014 = $true; break }
                $read = $process.StandardOutput.ReadLineAsync()
            }
            $now = Get-Date
            if (($now - $lastCheck).TotalSeconds -ge 5) {
                $lastCheck = $now
                $written = [long]((Get-ChildItem -LiteralPath $tmp -Recurse -File -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum)
                if ($written -gt $limitBytes) { $why = "wrote $written bytes without MSG:5014"; break }
                if (($now - $started).TotalSeconds -gt $limitSeconds) { $why = "no MSG:5014 within $limitSeconds s"; break }
            }
        }
        $stopped = -not $process.HasExited
        if ($stopped) { try { $process.Kill() } catch { } }
        if (-not $process.WaitForExit(60000)) {
            throw "makemkvcon64 (pid $($process.Id)) did not exit within 60 s of being stopped; end it with: taskkill /f /pid $($process.Id)"
        }
        $exitCode = $process.ExitCode
        $seconds = [int]((Get-Date) - $started).TotalSeconds
        if ($stopped) { Start-Sleep -Seconds 10 }   # let the drive settle after a killed call
        $errText = ''
        try { if ($errRead.Wait(10000)) { $errText = $errRead.Result } } catch { }
        $written = [long]((Get-ChildItem -LiteralPath $tmp -Recurse -File -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum)
        $codes = @($lines | ForEach-Object { if ($_ -match '^MSG:(\d+),') { [int]$Matches[1] } } | Sort-Object -Unique)

        if ($lines.Count -gt 0) {
            Set-Content -LiteralPath $file -Value $lines.ToArray() -Encoding UTF8 -ErrorAction Stop
        } else {
            Set-Content -LiteralPath $file -Value $null -Encoding UTF8 -ErrorAction Stop
        }
        if ($errText) { Set-Content -LiteralPath "$file.stderr" -Value $errText -Encoding UTF8 -ErrorAction Stop }

        if ($why) { $ended = "stopped early: $why" } elseif ($saw5014) { $ended = 'stopped at MSG:5014' } else { $ended = 'ended by itself without MSG:5014' }
        $entry = [ordered]@{
            title         = $title
            saw_5014      = $saw5014
            ended         = $ended
            lines         = $lines.Count
            written_bytes = $written
            exit_code     = $exitCode
            seconds       = $seconds
            msg_codes     = $codes
        }
        $results.Add($entry)
        $process = $null
        Save-Run 'running'
        $summaryLine = "title ${title}: lines=$($lines.Count) stopped-at-5014=$saw5014 written-bytes=$written exit=$exitCode seconds=$seconds codes=$($codes -join ',')"
        if ($why) { $summaryLine += " STOPPED-EARLY: $why" }
        $summaryLine
        if ($why) {
            $rest = @($titles | Where-Object { $_ -gt $title })
            $restText = if ($rest.Count -gt 0) { $rest -join ',' } else { 'none' }
            throw "title $title was stopped early ($why); titles not tested in this run: $restText (Task 10 Step 5)"
        }
    }

    Clear-Tmp
    if (Test-Path -LiteralPath $tmp) { "WARNING: $tmp could not be removed (it holds what a stopped call wrote); delete it later" }
    $failed = @($results | Where-Object { -not $_.saw_5014 } | ForEach-Object { $_.title })
    if ($failed.Count -gt 0) {
        throw "no MSG:5014 for title(s) $($failed -join ','); read Task 10 Step 5 before testing them again with -Redo $($failed -join ',')"
    }
    Save-Run 'ok'
    "RESULT ok: $($titles.Count) title(s) tested; every call reached MSG:5014 and was stopped there"
} catch {
    $err = $_
    $reason = $err.Exception.Message
    # Never leave a rip running: a call that is still going is stopped here.
    if ($process -and -not $process.HasExited) {
        try { $process.Kill() } catch { }
        [void]$process.WaitForExit(60000)
    }
    if ($tmpUsed) { Clear-Tmp }
    if ($runFile) {
        try { Save-Run "FAILED: $reason" } catch { $reason += " ($runFile could not be written)" }
    }
    "RESULT FAILED: $reason"
    throw $err
}
```

The script is plain ASCII on purpose: PowerShell 5.1 reads a `.ps1` without a BOM in the ANSI code page. It sets no global `$ErrorActionPreference`. Instead, each cmdlet whose failure matters carries `-ErrorAction Stop`, and the script throws on its own when a step fails.

Run: `LC_ALL=C grep -n '[^ -~]' scripts/phase0/exp02-index-guard.ps1; echo "rc=$?"`
Expected: no line, then `rc=1`. The file has no tab and no character outside printable ASCII.

- [ ] **Step 2: Check the script on the mini PC without a disc**

Which disc is in the drive: does not matter. No command in this step reaches the drive.

```bash
scripts/box.sh
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -Command \"(Get-Command C:\lazymkv\src\scripts\phase0\exp02-index-guard.ps1).Parameters.Keys\""
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\exp02-index-guard.ps1 -Slug BadSlug -Plan"; echo "exit=$?"
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\exp02-index-guard.ps1 -Slug zz-none -MaxTitles 1 -Plan"; echo "exit=$?"
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\exp02-index-guard.ps1 -Slug zz-none -Plan"; echo "exit=$?"
ssh USER@makemkv-box "if exist C:\lazymkv\phase0\recordings\zz-none (echo CREATED) else (echo absent)"
```

Expected:
- `box.sh` syncs the tree and exits 0.
- The parameter list begins `Slug`, `MaxTitles`, `Redo`, `Plan`, followed by PowerShell's common parameters, with no parse error. A parse error means Step 1's file is wrong: fix it and run this step again.
- `BadSlug`: `RESULT FAILED:` with Assert-Slug's rejection of `BadSlug`, then `exit=1`.
  - If the message says that `Assert-Slug` is not recognized, the env.ps1 helpers from D1 are missing: stop.
- `-MaxTitles 1`: `RESULT FAILED: -MaxTitles must be 2 or more (it was 1)`, then `exit=1`.
- `zz-none`: `RESULT FAILED: no meta.json in C:\lazymkv\phase0\recordings\zz-none (record the disc first: Task 8 or Task 9)`, then `exit=1`.
- The last command prints `absent`: a refused run creates nothing.

Now prove that the script refuses to mix with earlier results, and that it checks `-Redo` against the scan. The test recording is a stand-in: a one-title scan, an empty `meta.json`, and one earlier title file without `MSG:5014`.

```bash
ssh USER@makemkv-box "mkdir C:\lazymkv\phase0\recordings\zz-exp02-guard\exp02 && echo TINFO:0,2,0,x> C:\lazymkv\phase0\recordings\zz-exp02-guard\scan.txt && echo {}> C:\lazymkv\phase0\recordings\zz-exp02-guard\meta.json && echo old> C:\lazymkv\phase0\recordings\zz-exp02-guard\exp02\title-0.txt"
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\exp02-index-guard.ps1 -Slug zz-exp02-guard -Redo 5"; echo "exit=$?"
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\exp02-index-guard.ps1 -Slug zz-exp02-guard"; echo "exit=$?"
ssh USER@makemkv-box "type C:\lazymkv\phase0\recordings\zz-exp02-guard\exp02\title-0.txt & dir /b C:\lazymkv\phase0\recordings\zz-exp02-guard\exp02"
ssh USER@makemkv-box "rmdir /s /q C:\lazymkv\phase0\recordings\zz-exp02-guard"
```

If `mkdir` reports that the folder already exists, a previous attempt left it behind. Run the `rmdir` line first.

Expected:
- `mkdir` prints nothing.
- The `-Redo 5` run prints `scan: 1 titles, numbers 0 to 0; MSG:3308 lines=0; MSG:3309 lines=0`, then `RESULT FAILED: -Redo names titles that C:\lazymkv\phase0\recordings\zz-exp02-guard\scan.txt does not list: 5`, then `exit=1`.
- The run without `-Redo` fails within seconds. It prints, in order:

  ```text
  scan: 1 titles, numbers 0 to 0; MSG:3308 lines=0; MSG:3309 lines=0
  testing titles: 0 (1 of 1)
  earlier results: 1 title file(s) in C:\lazymkv\phase0\recordings\zz-exp02-guard\exp02; titles still to test: 0
  RESULT FAILED: C:\lazymkv\phase0\recordings\zz-exp02-guard\exp02 already holds results of an earlier run, which are kept as evidence; test the titles still to test (0) with -Redo (Task 10 Step 5)
  exit=1
  ```

  It prints no `disc:` line, because the refusal comes before the disc and the share are touched.
- `type` prints `old`, and `dir /b` lists only `title-0.txt`.
- `rmdir` prints nothing.

If the run without `-Redo` gets past `earlier results:`, the guard does not hold: stop and report. The empty `meta.json` makes Assert-DiscIs refuse any disc, so even then no MakeMKV call can start.

- [ ] **Step 3: Plan the run on the disc**

Which disc is in the drive: SLUG's disc, which in this task is Disc A. The script checks it with Assert-DiscIs and prints `disc: D: holds the disc of recording SLUG` only when the fingerprints match. In Task 15 the decoy-playlist disc was recorded earlier and the owner has changed discs since, so ask the owner to put it back first.

Ask the owner:
- to confirm that the disc SLUG is in drive D: and that the drive's light has stopped blinking;
- to close the MakeMKV window if it is open;
- to leave the disc in the drive, with the tray closed and without starting MakeMKV, until told that the experiment is finished. The run below takes about ESTIMATE minutes, printed by this step.
- to stay logged in at the mini PC's console.

```bash
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\exp02-index-guard.ps1 -Slug SLUG -Plan"; echo "exit=$?"
```

Expected (lines from the disc check may appear between `earlier results:` and `disc:`):

```text
scan: <M> titles, numbers <first> to <last>; MSG:3308 lines=<SCAN_3308>; MSG:3309 lines=<SCAN_3309>
testing titles: <PLANNED> (<n> of <M>)
earlier results: none
disc: D: holds the disc of recording SLUG (fingerprint <32 hex characters>)
calls write into \\unas-pro\media\lazymkv-staging\SLUG\exp02-tmp (emptied before each call, removed at the end)
limits per call: <LIMIT_S> s, or 536870912 bytes written, without MSG:5014; estimate: about <ESTIMATE> minutes
RESULT ok: plan only; no MakeMKV call was made
exit=0
```

Write down PLANNED, SCAN_3308, SCAN_3309, LIMIT_S and ESTIMATE. Each tested title costs one disc scan, so twelve titles can take an hour or more, and a UHD with Java menus much longer. The estimate is the better guide.

What the other results mean:
- `exit=1` with `is not the disc of recording SLUG`: another disc is in the drive. Ask the owner to insert SLUG's disc and to say when the light has stopped, then run the same line again.
- `exit=1` with an error from the disc check saying that no disc is mounted, or that `lazymkv-capture disc` failed: ask the owner whether the disc is in and the light has stopped. Wait 30 seconds and run the line again. If it fails a second time, stop and report.
- `exit=1` with `MakeMKV is already running (...)`: ask the owner to close the MakeMKV window. If the message names `makemkvcon64` and no MakeMKV window is open, end that process with `ssh USER@makemkv-box "taskkill /f /pid PID"`. Then run the line again.
- `exit=1` with Assert-Slug's rejection, or `no meta.json in ...` or `no scan.txt in ...`: SLUG is wrong. Take it from the top of the EXP-05 report and run the line again.
- `exit=1` with `missing ...` (MakeMKV or the job profile): stop and report. Task 7's setup is incomplete.
- `earlier results:` is not `none`: this slug already has results from an earlier run. Do not start Step 4; go to Step 5, which uses the `titles still to test:` list.
- `exit=255`: ssh could not reach the mini PC. Run the line again once it can.
- Anything else: stop and report.

- [ ] **Step 4: Run the experiment**

Which disc is in the drive: SLUG's disc, for the whole run. The script checks it with Assert-DiscIs before its first MakeMKV call.

```bash
scripts/phase0/job.sh exp02-index-guard.ps1 exp02-SLUG -Slug SLUG
scripts/phase0/wait.sh exp02-SLUG
```

job.sh prints `started exp02-index-guard.ps1; log: C:\lazymkv\phase0\logs\exp02-SLUG.txt`. If job.sh exits 1 because a phase 0 job is still running, wait for that job with its own wait.sh line, then start this one. Until this job has ended, start no other job, and do not run `scripts/box.sh`: it replaces `C:\lazymkv\src`, where the running script lives.

What wait.sh's exit codes mean for this job:
- **0**: the job finished and every tested call reached `MSG:5014`. Check the log against the Expected below, then go to Step 6.
- **3**: the job is still running. This is normal for this job: each title costs about one disc scan, ESTIMATE minutes in all. Run `scripts/phase0/wait.sh exp02-SLUG` again.
  - The last lines it prints are the `title` lines finished so far.
  - If the newest `title` line stays the same for longer than LIMIT_S seconds plus 10 minutes, the script's own limit did not act. Tell the owner, and end the job with `ssh USER@makemkv-box "powershell -NoProfile -Command Stop-ScheduledTask -TaskName lazymkv-phase0"`. Check that `ssh USER@makemkv-box "tasklist | findstr /i makemkvcon"` prints nothing, then stop and report.
- **1**: the script threw. Its `RESULT FAILED:` line gives the reason.
  - The log has no `title` line: a start-up check failed. Handle it as in Step 3, then start this step again with the log name `exp02-SLUG-b`. If the reason is `... already holds results ...`, go to Step 5 instead.
  - The error names `ReadLineAsync`, `Wait` or `Process`: these are the .NET calls of the read loop, which have never run in this setting before. Stop and report, because the script needs a fix.
  - Otherwise go to Step 5.
- **4**: the job is not running and its log has no PHASE0-DONE line: it never started, or it was killed.
  - Run `ssh USER@makemkv-box "tasklist | findstr /i makemkvcon"`. If it lists `makemkvcon64.exe`, end it with `ssh USER@makemkv-box "taskkill /f /im makemkvcon64.exe"` and run the check again: it must print nothing.
  - If the log has no line at all, the job never started: stop and report.
  - Otherwise go to Step 5, rule (e).
- **5**: the mini PC cannot be reached. Run wait.sh again once it can be.

Expected (exit 0), in the log:

```text
scan: <M> titles, numbers <first> to <last>; MSG:3308 lines=<SCAN_3308>; MSG:3309 lines=<SCAN_3309>
testing titles: <PLANNED> (<n> of <M>)
earlier results: none
disc: D: holds the disc of recording SLUG (fingerprint <32 hex characters>)
calls write into \\unas-pro\media\lazymkv-staging\SLUG\exp02-tmp (emptied before each call, removed at the end)
limits per call: <LIMIT_S> s, or 536870912 bytes written, without MSG:5014; estimate: about <ESTIMATE> minutes
run summary: C:\lazymkv\phase0\recordings\SLUG\exp02\run-<yyyyMMdd-HHmmss>.json
title <t>: lines=<L> stopped-at-5014=True written-bytes=<B> exit=-1 seconds=<S> codes=<...,5014>
RESULT ok: <n> title(s) tested; every call reached MSG:5014 and was stopped there
PHASE0-DONE status=ok exit=<n>
```

There is one `title` line for each title in PLANNED.
- `exit=-1` on a `title` line is normal: the script killed the call.
- `exit=<n>` on the DONE line is only the exit code of the last native command (usually -1, from Get-DiscIndex). It is not a verdict.
- Look at the `written-bytes=` values. This answers an open question: whether MakeMKV's piped output reaches the script as soon as it is printed. A few MB at most means it does. Much more means MakeMKV holds its output back, and Step 7 reports it.

- [ ] **Step 5: Test failed or missing titles again**

Which disc is in the drive: SLUG's disc, as in Step 4. The script checks it with Assert-DiscIs.

Do this step only when Step 4, or an earlier pass of this step, ended with wait.sh exit 1 naming titles, or with exit 4. Also do it when Step 3 reported earlier results. Otherwise go to Step 6.

First see why the calls failed. The pull never deletes anything, so it can be run at any time:

```bash
scripts/phase0/pull-recording.sh SLUG
grep '^MSG:' corpus/owner/SLUG/exp02/title-T.txt | tail -5
```

Run the `grep` once for each title T named in the `RESULT FAILED:` line. After a wait.sh exit 4 there is no such line: skip the `grep`.

Then decide for each title, from its `RESULT FAILED:` reason and those messages:
- **(a)** `stopped early (wrote <B> bytes without MSG:5014)`: MakeMKV began saving without printing `MSG:5014`. This is not a drive failure: do not test the title again. Stop and report to the controller, with the `RESULT FAILED:` line and that title's last messages. The EXP-02 report is written only after the controller decides.
- **(b)** `stopped early (no MSG:5014 within <LIMIT_S> s)`: the drive or the disc hung.
  - Ask the owner whether the drive still shows activity, and whether anything was touched.
  - Test the title again once, together with the titles that the same line lists as not tested in that run.
  - If it is stopped early a second time, stop and report.
- **(c)** `no MSG:5014 for title(s) ...`: the calls ended by themselves.
  - Test a title again once if its last messages show one of these:
    - an open failure (`MSG:5010`);
    - read errors (`MSG:2003`);
    - a key or licence message (`MSG:5021`, `MSG:5050`, `MSG:5052`, `MSG:5055` or `MSG:5095`).
  - Do not test it again if the messages show anything else (for example MakeMKV saying that the title does not exist), or if the same title fails the same way a second time. Step 7 reports it, with those messages.
  - If every title of Step 4's run failed this way, do not test again: stop and report. The drive or MakeMKV is not working.
  - If titles that come straight after a stopped call fail with `MSG:5010` while the others pass, note it for Step 7 (an open question: whether the drive needs a pause after a killed call).
- **(d)** `did not exit within 60 s of being stopped`: the message names PID.
  - End the process with `ssh USER@makemkv-box "taskkill /f /pid PID"`.
  - Check that `ssh USER@makemkv-box "tasklist | findstr /i makemkvcon"` prints nothing.
  - Then test the title again as in (b).
- **(e)** wait.sh exit 4 (the job was killed): test again every title that is still to test.

Then list what is still to test:

```bash
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\exp02-index-guard.ps1 -Slug SLUG -Plan"; echo "exit=$?"
```

Expected: `exit=0`, with the line `earlier results: <n> title file(s) in C:\lazymkv\phase0\recordings\SLUG\exp02; titles still to test: <list>`. The list holds:
- every title of the plan that has no title file yet;
- every title whose file has no `MSG:5014` line.

Other `exit=1` results read as in Step 3.

LIST is that list without the titles that rules (a) and (c) say not to test again. If LIST is empty, go to Step 6. Otherwise:

```bash
scripts/phase0/job.sh exp02-index-guard.ps1 exp02-SLUG-redoK -Slug SLUG -Redo LIST
scripts/phase0/wait.sh exp02-SLUG-redoK
```

What wait.sh's exit codes mean for this job:
- **0**: every call in LIST reached `MSG:5014`. Check the log against the Expected below, then go to Step 6.
- **3**: still running, which is normal. Each title costs about one disc scan. Run `scripts/phase0/wait.sh exp02-SLUG-redoK` again. Stalled `title` lines are handled as in Step 4.
- **1**: the script threw. Read its `RESULT FAILED:` line, then go through this step again with K+1. A start-up failure (no `title` line) is handled as in Step 3 first.
- **4**: the job was killed or never started. Run the tasklist check from Step 4. Then go through this step again with K+1, rule (e). If the log has no line at all, stop and report.
- **5**: the mini PC cannot be reached. Run wait.sh again once it can be.

Expected (exit 0), in the log:
- the start-up lines, with `testing titles: <LIST> (...)` and the `earlier results:` line;
- for each title that already had a file, `kept the earlier title-T.txt in exp02\superseded\<yyyyMMdd-HHmmss>`. The earlier result stays as evidence and nothing is replaced;
- one `title T: ... stopped-at-5014=True ...` line for each title in LIST;
- `RESULT ok: ...` and `PHASE0-DONE status=ok exit=<n>`.

Test a title at most twice in all. If a title fails both times, Step 7 reports it.

- [ ] **Step 6: Evaluate**

Which disc is in the drive: does not matter.

Run this only when the last job of Steps 4 and 5 ended with wait.sh exit 0, or when the only titles left to test are ones that Step 5 set aside.

```bash
scripts/phase0/pull-recording.sh SLUG
diff <(ssh USER@makemkv-box "dir /b C:\lazymkv\phase0\recordings\SLUG\exp02\title-*.txt" | tr -d '\r' | sort) <(cd corpus/owner/SLUG/exp02 && ls title-*.txt | sort); echo "diff=$?"
python3 scripts/phase0/exp02_check.py corpus/owner/SLUG; echo "check=$?"
python3 - SLUG <<'EOF'
import json, pathlib, sys
d = pathlib.Path("corpus/owner") / sys.argv[1] / "exp02"
paths = sorted(d.glob("run-*.json"))
if not paths:
    sys.exit(f"no run-*.json in {d}")
largest = 0
for path in paths:
    run = json.loads(path.read_text(encoding="utf-8-sig"))
    results = run.get("results") or []
    if isinstance(results, dict):
        results = [results]
    print(f"{path.name}: result={run.get('result')} time_limit_s={run.get('time_limit_s')}")
    for r in results:
        print(f"  title {r['title']}: 5014={r['saw_5014']} ended={r['ended']} written_bytes={r['written_bytes']} seconds={r['seconds']} codes={r['msg_codes']}")
        if r["saw_5014"]:
            largest = max(largest, int(r["written_bytes"]))
print("largest written_bytes of a call stopped at MSG:5014:", largest)
EOF
grep -l '^MSG:1009,' corpus/owner/SLUG/exp02/title-*.txt
```

Expected:
- `pulled corpus/owner/SLUG (<n> files copied, <n> files there now)`. If the pull fails, nothing on the Mac changed: run it again.
- `diff` prints nothing, then `diff=0`: the Mac and the mini PC hold the same title files.
  - A line starting `>` names a title file that exists only on the Mac. A redo moved the mini PC's copy to `superseded` and did not finish. Go back to Step 5: its `-Plan` lists that title as still to test.
  - A line starting `<` names a file that the pull did not bring. Run the pull and the `diff` again.
- The checker prints one line per title file, then `EXPERIMENT 2: yes (<n> titles)` and `check=0`. Each line reads `title   T: scan source=<file> rip #T=<file> match=yes 5014=yes added-before-5014=yes 3308-lines=<n> 3309-lines=<n>`. `check=1` with `EXPERIMENT 2: no` is an answer, not a command failure.
- The run summary prints each `run-<time>.json` with its `result` and its titles. Its last line gives WRITTEN_MAX: `largest written_bytes of a call stopped at MSG:5014: <n>`.
- `grep -l` prints nothing. A file name means that MakeMKV rejected the job profile in that call (`MSG:1009`, spec 9.3). Note it for Step 7.

How to read the checker:
- **Every title of PLANNED must appear**, plus any title that a redo added. If a planned title is missing, or a title that Step 5 did not set aside shows `5014=NO`, a call failed: go back to Step 5.
- **`5014=NO` on a title that Step 5 set aside**: this is not an answer, and Step 7 reports the title as not tested. There is one exception: if its `rip #T=` names a file and that file differs from `scan source=`, MakeMKV numbered the title differently. That is a finding even without `MSG:5014`.
- **`match=NO` or `added-before-5014=NO` on a title with `5014=yes`**: this is a finding, and the answer is no. Note that title's `scan source=` and `rip #T=` values.
- **Probe for an open question** (how a `MSG:3308` line is laid out): if any line shows `3308-lines=` above 0, run `grep -h '^MSG:3308,' corpus/owner/SLUG/exp02/title-*.txt | head -5`.
  - exp02_check.py takes the first quoted parameter after the two message texts as the file, and the last parameter as the title number.
  - If the last parameter is not the title number (for example, it is an angle number), the checker misreads angle titles. Stop and report to the controller with those lines before Step 7.

- [ ] **Step 7: Write the report**

Which disc is in the drive: does not matter.

Create `docs/phase0/experiments/EXP-02-index-guard.md` from `docs/phase0/experiments/TEMPLATE.md`. Fill it in as follows:

- **Header**:
  - Date: the date of Step 4's run.
  - Disc(s): SLUG, its `format` from `corpus/owner/SLUG/meta.json`, and drive D: (BU40N).
  - MakeMKV version: the `MSG:1005` banner, from `grep -h -m1 'MSG:1005,' corpus/owner/SLUG/exp02/title-*.txt | head -1`. The line may start with an invisible byte-order mark: the title files are UTF-8 with a BOM and `MSG:1005` is their first line, so do not anchor the pattern with `^`. (The anchored form works only on `scan.txt`, which the capture tool writes without a BOM: `grep -m1 '^MSG:1005,' corpus/owner/SLUG/scan.txt`.)
  - Spec reference: section 17, row 2.
- **Question**: row 2's question.
- **What was run**:
  - `scripts/phase0/exp02-index-guard.ps1`, with the flags in the `command` field of the run JSON.
  - The titles tested: PLANNED, which is n of M, sampled evenly with `-MaxTitles 12`. Say plainly that this is a sample and not every title index, as row 2 asks.
  - Each redo, with its titles and the reason from Step 5.
  - Every title set aside in Step 5, with its last messages.
- **What was observed**:
  - The checker's output, pasted in full in a `text` block.
  - Whether the disc has angle titles (`3308` lines) or skipped duplicates (`3309` lines): SCAN_3308 and SCAN_3309 from the scan, and the checker's `3308-lines` and `3309-lines` columns.
  - WRITTEN_MAX, and what it says about how promptly MakeMKV's piped output reaches a reader. A few MB at most is prompt. Much more means MakeMKV holds its output back, and a guard in the engine would react late by about that much.
  - Any `MSG:1009` (profile rejected).
  - The `MSG:3308` parameter layout seen in Step 6, if there were such lines.
  - Any open failures (`MSG:5010`) straight after a stopped call (Step 5, rule (c)).
- **Answer**:
  - State whether the disc has angle titles or skipped duplicates.
  - If it has neither, say that the experiment's condition, "a disc with angles or duplicate titles", is not yet met, and that Task 15 repeats the experiment on the decoy-playlist disc.
  - Answer yes or no in one sentence:
    - yes when every title that reached `MSG:5014` has `match=yes` and `added-before-5014=yes`;
    - no otherwise, naming the titles.
  - If the disc has neither `3308` nor `3309` lines, word it as "yes on this disc, which has neither angles nor duplicate titles" (or "no on this disc ...").
  - Name any title set aside in Step 5. The answer covers only the titles that reached `MSG:5014`.
- **Consequence for the spec**:
  - If no: spec 6.2 step 6 drops the pre-read guard, as the fallback in section 17 row 2 says (drift is then caught by verification after the read).
  - If yes: none yet. The guard stays, and when this disc did not meet the condition, the final answer waits for Task 15.

- [ ] **Step 8: Commit**

Which disc is in the drive: does not matter. Tell the owner that the experiment is finished. Disc A stays in the drive for Task 11.

```bash
find corpus/owner/SLUG -iname '*unit_key*'
grep -rIl 'eyJhbGciOi' corpus/owner/SLUG
git add scripts/phase0/exp02-index-guard.ps1 corpus/owner/SLUG docs/phase0/experiments/EXP-02-index-guard.md
git status --short
git commit -m "Run experiment 2: index guard" -m "Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

Expected:
- `find` and `grep` print nothing: there is no AACS unit-key file and no JSON Web Token in the recording. If either prints a path, stop and do not commit.
- `git status --short` lists as staged (`A `) only these:
  - the script;
  - the report;
  - the new files under `corpus/owner/SLUG/exp02/`: the `title-<n>.txt` files, any `title-<n>.txt.stderr`, the `run-<time>.json` files and any `superseded/` files.
- The commit succeeds.

---

### Task 11: All-tracks rip to the share (experiments 1 and 4)

**Files:**
- Create: `scripts/phase0/rip-title.ps1`, `scripts/phase0/probe-title.ps1`
- Create: `docs/phase0/experiments/EXP-01-profile-and-order.md`, `docs/phase0/experiments/EXP-04-rip-to-share.md`
- Create (pulled from the mini PC): `corpus/owner/SLUG/rehearsal/`, `corpus/owner/SLUG/all-tracks/`, `corpus/owner/SLUG/share-drop/`, and when their steps run, `corpus/owner/SLUG/share-drop-2/`, `corpus/owner/SLUG/all-tracks-512/`, `corpus/owner/SLUG/all-tracks-256/`
- Prerequisite, applied and committed by the controller before this task: `scripts/phase0/run-interactive.ps1` (`-NonInteractive`)
- Create (pulled from the mini PC): `corpus/owner/SLUG/<label>/audio-levels-20ms-<M>[-<C>...]/` for every successful rip (its level files are ignored by git; `streams.json`, `proxy-check.json` and `audio-proxy.json` are committed); modify `corpus/owner/audio-dataset.json`; create or modify `corpus/owner/SLUG/audio-labels.json`.

**Interfaces:**
- Consumes:
  - From `env.ps1`: `$P0`, `$Recordings`, `$Staging`, `$Drive`, `$CAP`, `$MK`, `$MKVMERGE`, `$FFPROBE`, `$MEDIAINFO`, `$AllTracksProfile`, `Get-MkFlags -Profile <path> -CacheMb <n>`, `Get-DiscIndex`, `Get-FreeRamGB`, `Assert-Slug`, `Assert-DiscIs`, `Assert-EmptyDir`.
  - `lazymkv-capture record --out <file> -- <command...>`: writes `<file>` (the command's standard output), `<file>.timing.jsonl` (one `{"n":<line>,"t_ms":<ms since start>}` per line), `<file>.stderr` and, when the command has ended, `<file>.exit.json` (`exit_code`, `elapsed_ms`, `lines`). Its own exit code is the command's, or 2 when the tool itself fails.
  - Scripts: `scripts/box.sh`, `scripts/phase0/job.sh`, `scripts/phase0/wait.sh`, `scripts/phase0/pull-recording.sh <slug>` (Task 8: never deletes on the Mac), `scripts/phase0/setup-check.ps1` (Task 7), `scripts/phase0/summarize_scan.py <recording dir>`, `scripts/phase0/compare_tracks.py <scan> <title> <mkvmerge json> [--selection all|eng]` (exits 1 when any verdict is no), `scripts/phase0/robot.py` (`parse`).
  - Disc A's recording from Task 9: `C:\lazymkv\phase0\recordings\SLUG\` and `corpus/owner/SLUG/`, with `meta.json` and `scan.txt` (the online scan, made with `Get-MkFlags`' defaults: the all-tracks profile and `--cache=1024`).
  - Prerequisite D16, applied by the controller before this task: `scripts/phase0/audio-levels.ps1 -Slug <slug> -Label <label> -Main <stream> [-Cand <stream>[,<stream>...]] [-Force]` (a job), `scripts/phase0/audio_dataset.py [args corpus/owner/<slug> <label>]`, `scripts/phase0/audio_labels.py` (`auto`, `set`, `unset`, `title`, `kinds`, `check`), and `$TuningData` from env.ps1.
- Produces:
  - `scripts/phase0/rip-title.ps1 -Slug <slug> -Title <n> -Label <name> [-Profile <path>] [-CacheMb <n>] [-Force]`. Run it as a job: `scripts/phase0/job.sh rip-title.ps1 <log> -Slug <slug> -Title <n> -Label <name> ...`. A `-Profile` path typed in the Mac shell must be in single quotes there (`-Profile 'C:\lazymkv\phase0\profiles\eng-only.mmcp.xml'`), or bash deletes its backslashes; job.sh receives it without quotes. `-Profile` defaults to `$AllTracksProfile`, `-CacheMb` to 1024.
    - It refuses before anything is created or deleted in the recording or on the share (only Assert-DiscIs's own `$P0\tmp\disc-check` is written), when:
      - `-Slug` or `-Label` fails Assert-Slug;
      - the profile file or `$CAP` does not exist;
      - `$Recordings\<slug>\scan.txt` is missing or lists no title `<n>`;
      - a MakeMKV process (`makemkvcon64`, `makemkvcon` or the GUI `makemkv`) is running;
      - `$Staging\<slug>\<label>\` or `$Recordings\<slug>\<label>\` is not empty and `-Force` was not given;
      - Assert-DiscIs rejects the disc in the drive.
    - Then, with `-Force`, it deletes both folders. In every case it creates both empty with Assert-EmptyDir, the staging folder first, so an unreachable share stops it before anything is created on C:.
    - It samples free RAM every 5 seconds in a background job. It then runs `lazymkv-capture record --out <rec>\rip.txt -- makemkvcon64 <Get-MkFlags -Profile -CacheMb> mkv disc:<index> <n> <staging folder>`. `<rec>` is `$Recordings\<slug>\<label>`, which is `C:\lazymkv\phase0\recordings\<slug>\<label>`.
    - It writes, in `<rec>`:
      - `rip.txt`, with `.timing.jsonl`, `.stderr` and `.exit.json` beside it;
      - `free-ram.csv`: a `time,free_mb` header, then one `yyyy-MM-dd HH:mm:ss,<MB>` line per sample;
      - `rip-summary.json` (UTF-8 with a BOM). Its fields are `ok`, `result`, `slug`, `label`, `title`, `profile`, `cache_mb`, `started` (the mini PC's clock, `yyyy-MM-dd HH:mm:ss.fff`), `elapsed_s`, `saving_started_s` (when `MSG:5014` arrived, in seconds after the call started), `bytes`, `mb_per_s` (for the whole call), `write_mb_per_s` (after `MSG:5014`), `free_ram_before_gb`, `min_free_ram_gb`, `min_free_ram_at_s` (seconds after the call started), `ram_samples`, `files`, `dest`, `exit_code`, `success_message`, `fail_codes`, `profile_errors` (count of `MSG:1009`), `read_errors` (`2003`), `write_errors` (`2019`), `scan_source_file` (scan.txt's `TINFO:<n>,16`) and `rip_source_file` (this rip's `MSG:3307` for title `<n>`).
      - The summary is written whenever the folders were prepared, for a success and for a failure.
    - The rip succeeded (spec 9.4) only if all of these hold:
      - a success summary (`MSG:5005` or `5036`) was seen, and it reports one title when it states a number;
      - none of `5003`, `5004`, `5037` was seen;
      - there is exactly one non-empty `.mkv` in the staging folder;
      - `rip_source_file` equals `scan_source_file` when both are known. A DVD scan has no attribute 16, and then nothing is compared.
    - MakeMKV's exit code is recorded, never judged.
    - Output lines: `rip: ...`, the summary JSON, then `RESULT ok` or `RESULT FAILED: <reason>`. `RESULT ok` may carry a note when `MSG:1009` was seen. After a failure the script throws, so wait.sh exits 1.
  - `scripts/phase0/probe-title.ps1 -Slug <slug> -Label <name> [-SkipRemux] [-Force]`, run as a job.
    - It refuses unless `<rec>\rip-summary.json` says `"ok": true`.
    - It refuses when probe evidence already exists in `<rec>`, unless `-Force` is given; `-Force` deletes that evidence first. The rip's own files are never touched.
    - It finds the one `.mkv` in the staging folder, which must be the file rip-title.ps1 recorded, and writes, in `<rec>`:
      - `mkvmerge.json`, `ffprobe-streams.json` and `mediainfo.json` (when MediaInfo is installed);
      - `subs\index.json`, a JSON array of `n`, `track_id`, `language`, `codec`, `track_name`, `forced`, `default`;
      - `subs\sub-<n>.packets.json`;
      - `evidence-mkvmerge.txt` and `remux-mkvmerge.txt`;
      - `probe-summary.json` (UTF-8 with a BOM): `ok`, `result`, `file`, `file_bytes`, `subtitle_tracks`, `evidence_s`, `evidence_bytes`, `remux_skipped`, `remux_s`, `remux_exit`, `remux_bytes`, `remux_valid`, `remux_deleted`.
    - On the share it leaves `evidence.mks`. A timed remux, `remux-test.tmp`, is deleted afterwards; it is not a `.mkv`, so it can never be taken for MakeMKV's file.
    - `-SkipRemux` leaves out the remux. Only experiment 4 needs it; Task 13 skips it.
    - Every native call is checked: `mkvmerge` exit 2, or an `ffprobe` or `MediaInfo` exit other than 0, or a missing or empty output, is a failure. A remux is valid only with exit 0 or 1 and at least 90% of the source's size.
    - The last line is `RESULT ok` or `RESULT FAILED: <reason>`, and the summary is written either way once the file has been found. After a failure the script throws.
  - The two experiment reports, and the recordings listed under Files.

Placeholders:
- SLUG: Disc A's slug, chosen in Task 9 and written at the top of EXP-05 and EXP-12.
- N: the main feature's title number, chosen in Step 4 from `summarize_scan.py`'s table.
- K: the rehearsal title's number, chosen in Step 4.
- M: the share-drop title's number, chosen in Step 8.
- DROP: the share-drop label: `share-drop` the first time, `share-drop-2` for the one repeat with a five-minute outage (Step 8).
- T_UNPLUG, T_REPLUG, T_KILL: the mini PC's time (`yyyy-MM-dd HH:mm:ss.fff`, from the time command in Step 8) when the owner said the NAS cable was out, when it was back in, and when makemkvcon was ended with taskkill (only if that happened).
- CACHE: `512`, then `256` if 512 still answers experiment 4 with no (Step 9).
- LABEL: in the `-Force` rule below, the label of the attempt being repeated: `rehearsal`, `all-tracks`, DROP or `all-tracks-CACHE`.
- Disc A: the UHD film of Task 9. It stays in the drive for the whole task; Task 12 starts with it. rip-title.ps1 checks it before every rip with Assert-DiscIs.

When `-Force` is right (decision D3):
- `rip-title.ps1 ... -Force`: only to replace a failed or interrupted attempt at the same label, and only once its cause has been dealt with as the step says. Such an attempt has a `rip-summary.json` that is missing or says `"ok": false`. Check with `ssh USER@makemkv-box "type C:\lazymkv\phase0\recordings\SLUG\LABEL\rip-summary.json"`.
  - Before using it, pull the recording (`scripts/phase0/pull-recording.sh SLUG`) and keep the failed attempt on the Mac with `mv corpus/owner/SLUG/LABEL corpus/owner/SLUG/LABEL-failed-1` (`-failed-2` if `-failed-1` already exists, and so on: `mv` onto an existing folder would put LABEL inside it). Do not copy it instead: a pull never deletes on the Mac, so a file that only the failed attempt has would stay in `corpus/owner/SLUG/LABEL` beside the repeat's files. The next pull then creates `corpus/owner/SLUG/LABEL` afresh from the repeated attempt, and the moved folder is committed as evidence of the failure.
  - Never use `-Force` over a rip whose summary says `"ok": true`.
- `probe-title.ps1 ... -Force`: only to repeat a probe whose `probe-summary.json` is missing or says `"ok": false`, once its cause has been dealt with.
- In no other case.

Log names: run-interactive.ps1 deletes a log of the same name before it starts a job. So a job started again after it failed, was refused or was ended gets a new log name, and the old log (with its `RESULT` line and printed summary) stays in `C:\lazymkv\phase0\logs` as the record of that attempt. Add `-b` to the log name (then `-c`, and so on), in the job.sh command and in the wait.sh command alike. For example: `scripts/phase0/job.sh rip-title.ps1 rip-all-tracks-b -Slug SLUG -Title N -Label all-tracks -Force`, then `scripts/phase0/wait.sh rip-all-tracks-b`.

- [ ] **Step 1: Write the rip script**

Which disc is in the drive: none needed.

`scripts/phase0/rip-title.ps1`:

```powershell
# Rips one title of the disc recorded under -Slug to $Staging\<Slug>\<Label>\ with a job profile, and judges the rip.
# The evidence goes to $Recordings\<Slug>\<Label>\ (C:\lazymkv\phase0\recordings\<Slug>\<Label>\):
#   rip.txt with .timing.jsonl, .stderr and .exit.json   MakeMKV's output, recorded by lazymkv-capture
#   rip.txt.makemkv-debug.txt   MakeMKV's debug log of this run (decision D16)
#   free-ram.csv       free RAM every 5 s while MakeMKV runs (mini PC clock, MB)
#   system.csv         every 5 s: CPU load in percent, free RAM in MB, bytes written to the staging folder so far
#   system-events.json warnings and errors Windows logged about storage, USB and the network during the rip
#   rip-summary.json   timing, size, throughput, memory and the verdict
# One label holds one rip. If the label's staging or evidence folder is not empty the run is refused, unless
# -Force is given: -Force deletes both, but only after Assert-DiscIs has accepted the disc.
# Success (spec 9.4): a success summary (MSG:5005 or 5036) for one title; none of 5003, 5004 or 5037; exactly
# one non-empty .mkv in the staging folder; and, where both name one, the same source file for the title in
# this rip (MSG:3307) as in the recording's scan.txt (TINFO attribute 16).
# MakeMKV's exit code is recorded, never judged (spec 9.2). MakeMKV text never builds a path.
# The last line is "RESULT ok" or "RESULT FAILED: <reason>"; after a failure the script throws.
param(
    [Parameter(Mandatory)][string]$Slug,
    [Parameter(Mandatory)][int]$Title,
    [Parameter(Mandatory)][string]$Label,
    [string]$Profile = '',
    [int]$CacheMb = 1024,
    [switch]$Force
)
. C:\lazymkv\src\scripts\phase0\env.ps1

$failure = $null
$note = ''
$summary = $null
$memJob = $null
$ripRan = $false
try {
    Assert-Slug $Slug
    Assert-Slug $Label
    if (-not $Profile) { $Profile = $AllTracksProfile }
    if (-not (Test-Path -LiteralPath $Profile)) { throw "profile not found: $Profile" }
    if (-not (Test-Path -LiteralPath $CAP)) { throw "missing $CAP; build it with: scripts/box.sh cargo build --release -p lazymkv-capture" }
    $rec = "$Recordings\$Slug\$Label"
    $dest = "$Staging\$Slug\$Label"

    # The title must be in the recording's scan (made with Get-MkFlags' defaults); its source file is compared below.
    $scanFile = "$Recordings\$Slug\scan.txt"
    if (-not (Test-Path -LiteralPath $scanFile)) { throw "no $scanFile; record the disc with a scan first (record-disc.ps1)" }
    $scanText = [IO.File]::ReadAllText($scanFile)
    if (-not [regex]::IsMatch($scanText, '(?m)^TINFO:' + $Title + ',')) { throw "scan.txt lists no title $Title" }
    $found = [regex]::Match($scanText, '(?m)^TINFO:' + $Title + ',16,\d+,"(.*)"\r?$')
    $scanSource = $null
    if ($found.Success) { $scanSource = $found.Groups[1].Value }

    # Only one MakeMKV may use the drive (spec 10.4); the GUI holds it too.
    $running = @(Get-Process -Name makemkvcon64, makemkvcon, makemkv -ErrorAction SilentlyContinue)
    if ($running.Count -gt 0) { throw "MakeMKV is still running ($($running[0].Name).exe, process id $($running[0].Id)); it must end before a rip starts" }

    # Evidence and staged files are never replaced without -Force.
    foreach ($dir in $dest, $rec) {
        if (-not $Force -and (Test-Path -LiteralPath $dir) -and @(Get-ChildItem -LiteralPath $dir -Force -ErrorAction Stop).Count -gt 0) {
            throw "$dir is not empty: label $Label was ripped before and is kept; choose another -Label, or use -Force only where the plan says so"
        }
    }
    Assert-DiscIs $Slug
    # The staging folder first: if the share cannot be reached, nothing is created on C:.
    foreach ($dir in $dest, $rec) {
        if ($Force -and (Test-Path -LiteralPath $dir)) { Remove-Item -LiteralPath $dir -Recurse -Force -ErrorAction Stop }
        Assert-EmptyDir $dir
    }

    $summary = [ordered]@{
        ok = $false; result = $null; slug = $Slug; label = $Label; title = $Title; profile = $Profile; cache_mb = $CacheMb
        started = $null; elapsed_s = $null; saving_started_s = $null; bytes = $null; mb_per_s = $null; write_mb_per_s = $null
        free_ram_before_gb = $null; min_free_ram_gb = $null; min_free_ram_at_s = $null; ram_samples = 0
        files = @(); dest = $dest; exit_code = $null; success_message = $null; fail_codes = @()
        profile_errors = $null; read_errors = $null; write_errors = $null
        scan_source_file = $scanSource; rip_source_file = $null
    }

    # Free RAM every 5 s while MakeMKV runs, sampled by a background job (mini PC clock, MB).
    $memFile = "$rec\free-ram.csv"
    Set-Content -LiteralPath $memFile -Value 'time,free_mb' -Encoding UTF8 -ErrorAction Stop
    $summary['free_ram_before_gb'] = Get-FreeRamGB
    $sysFile = "$rec\system.csv"
    Set-Content -LiteralPath $sysFile -Value 'time,cpu_percent,free_mb,written_bytes' -Encoding UTF8 -ErrorAction Stop
    $memJob = Start-Job -ArgumentList $memFile, $sysFile, $dest -ScriptBlock {
        param($file, $sys, $folder)
        while ($true) {
            $time = Get-Date -Format 'yyyy-MM-dd HH:mm:ss'
            $mb = ''
            try {
                $mb = [int]((Get-CimInstance Win32_OperatingSystem -ErrorAction Stop).FreePhysicalMemory / 1024)
                [IO.File]::AppendAllText($file, $time + ',' + $mb + "`r`n")
            } catch { }
            # The extra columns never hold up the RAM sample above; an unreadable value stays empty.
            $cpu = ''
            try { $cpu = [int]((Get-CimInstance Win32_Processor -ErrorAction Stop | Measure-Object -Property LoadPercentage -Average).Average) } catch { }
            $written = ''
            try { $written = [int64]((Get-ChildItem -LiteralPath $folder -File -Force -ErrorAction Stop | Measure-Object -Property Length -Sum).Sum) } catch { }
            try { [IO.File]::AppendAllText($sys, "$time,$cpu,$mb,$written`r`n") } catch { }
            Start-Sleep -Seconds 5
        }
    }

    $index = Get-DiscIndex
    $flags = Get-MkFlags -Profile $Profile -CacheMb $CacheMb
    $start = Get-Date
    $summary['started'] = $start.ToString('yyyy-MM-dd HH:mm:ss.fff')
    "rip: $Slug title $Title (scan source file: $scanSource) to $dest with --cache=$CacheMb, started $($summary['started'])"
    & $CAP record --out "$rec\rip.txt" -- $MK @flags mkv "disc:$index" $Title $dest | Out-Null
    $summary['exit_code'] = $LASTEXITCODE
    $elapsed = ((Get-Date) - $start).TotalSeconds
    $summary['elapsed_s'] = [int]$elapsed
    $ripRan = $true
    Save-MkDebugLog -Since $start -To "$rec\rip.txt.makemkv-debug.txt"
    Save-SystemEvents -Since $start -To "$rec\system-events.json"
} catch {
    $failure = $_.Exception.Message
}

if ($memJob) {
    Stop-Job -Job $memJob -ErrorAction SilentlyContinue
    Remove-Job -Job $memJob -Force -ErrorAction SilentlyContinue
}
if ($null -ne $summary) {
    try {
        $samples = @(Get-Content -LiteralPath $memFile -ErrorAction SilentlyContinue | ForEach-Object {
            if ($_ -match '^(\d{4}-\d\d-\d\d \d\d:\d\d:\d\d),(\d+)$') {
                [pscustomobject]@{ at = [datetime]::ParseExact($Matches[1], 'yyyy-MM-dd HH:mm:ss', $null); mb = [int]$Matches[2] }
            }
        })
        $summary['ram_samples'] = $samples.Count
        if ($samples.Count -gt 0) {
            $low = $samples | Sort-Object -Property mb | Select-Object -First 1
            $summary['min_free_ram_gb'] = [math]::Round($low.mb / 1024, 2)
            if ($summary['started']) { $summary['min_free_ram_at_s'] = [int]($low.at - $start).TotalSeconds }
        }

        if ($ripRan) {
            $lines = @()
            if (Test-Path -LiteralPath "$rec\rip.txt") { $lines = [IO.File]::ReadAllText("$rec\rip.txt").Split("`n") }
            $timing = @()
            if (Test-Path -LiteralPath "$rec\rip.txt.timing.jsonl") { $timing = [IO.File]::ReadAllText("$rec\rip.txt.timing.jsonl").Split("`n") }
            # rip.txt split at newlines lines up one for one with the timing lines.
            $codes = @()
            $successLine = $null
            $savedCount = $null
            for ($i = 0; $i -lt $lines.Count; $i++) {
                $line = $lines[$i]
                if (-not ($line -match '^MSG:(\d+),')) { continue }
                $code = [int]$Matches[1]
                $codes += $code
                if ($code -eq 5005 -or $code -eq 5036) {
                    $successLine = $line.Trim()
                    $savedCount = $null
                    if ($line -match ',"(\d+)"\s*$') { $savedCount = [int]$Matches[1] }
                }
                if ($code -eq 3307 -and $line -match ',"([^"]*)","(\d+)"\s*$' -and [int]$Matches[2] -eq $Title) {
                    $summary['rip_source_file'] = $Matches[1]
                }
                if ($code -in 5014, 5015 -and $null -eq $summary['saving_started_s'] -and $i -lt $timing.Count -and $timing[$i] -match '"t_ms":(\d+)') {
                    $summary['saving_started_s'] = [math]::Round([double]$Matches[1] / 1000, 1)
                }
            }
            $failCodes = @($codes | Where-Object { $_ -eq 5003 -or $_ -eq 5004 -or $_ -eq 5037 } | Sort-Object -Unique)
            $summary['fail_codes'] = $failCodes
            $summary['success_message'] = $successLine
            $summary['profile_errors'] = @($codes | Where-Object { $_ -eq 1009 }).Count
            $summary['read_errors'] = @($codes | Where-Object { $_ -eq 2003 }).Count
            $summary['write_errors'] = @($codes | Where-Object { $_ -eq 2019 }).Count

            # The output is found by listing the folder that was empty before the call.
            $files = @(Get-ChildItem -LiteralPath $dest -File -Force -ErrorAction SilentlyContinue)
            $mkvs = @($files | Where-Object { $_.Extension -eq '.mkv' })
            $bytes = [int64](($mkvs | Measure-Object -Property Length -Sum).Sum)
            $summary['files'] = @($files | ForEach-Object { $_.Name })
            $summary['bytes'] = $bytes
            if ($elapsed -gt 0 -and $bytes -gt 0) { $summary['mb_per_s'] = [math]::Round($bytes / 1MB / $elapsed, 1) }
            $saveStart = $summary['saving_started_s']
            if ($null -ne $saveStart -and $bytes -gt 0 -and $elapsed -gt $saveStart) {
                $summary['write_mb_per_s'] = [math]::Round($bytes / 1MB / ($elapsed - $saveStart), 1)
            }

            $problems = @()
            if (-not (Test-Path -LiteralPath "$rec\rip.txt.exit.json")) { $problems += 'lazymkv-capture wrote no rip.txt.exit.json' }
            if (-not $successLine) { $problems += 'no success summary (MSG:5005 or 5036)' } elseif ($null -ne $savedCount -and $savedCount -ne 1) { $problems += "the success summary reports $savedCount titles" }
            if ($failCodes.Count -gt 0) { $problems += "failure messages $($failCodes -join ',')" }
            if ($mkvs.Count -ne 1) { $problems += "$($mkvs.Count) .mkv files in $dest" } elseif ($mkvs[0].Length -eq 0) { $problems += "$($mkvs[0].Name) is empty" }
            $ripSource = $summary['rip_source_file']
            if ($scanSource -and $ripSource -and $ripSource -ne $scanSource) { $problems += "title $Title is $ripSource in this rip but $scanSource in scan.txt" }
            if ($problems.Count -gt 0) { $failure = $problems -join '; ' }
            if ($summary['profile_errors'] -gt 0) { $note = " (MSG:1009 seen $($summary['profile_errors']) times: MakeMKV rejected the profile; see rip.txt)" }
        }
    } catch {
        if (-not $failure) { $failure = "judging the rip failed: $($_.Exception.Message)" }
    }
    $summary['ok'] = -not $failure
    $summary['result'] = 'ok'
    if ($failure) { $summary['result'] = "FAILED: $failure" }
    $json = $summary | ConvertTo-Json
    try { Set-Content -LiteralPath "$rec\rip-summary.json" -Value $json -Encoding UTF8 -ErrorAction Stop } catch { if (-not $failure) { $failure = "cannot write rip-summary.json: $($_.Exception.Message)" } }
    $json
}
if ($failure) {
    "RESULT FAILED: $failure"
    throw "rip-title.ps1: $failure"
}
"RESULT ok$note"
```

The script does not set `$ErrorActionPreference = 'Stop'`: inside a job, that would turn any line a native program writes to standard error into a failure. Each cmdlet that must not fail silently has `-ErrorAction Stop` instead.

- [ ] **Step 2: Write the probe script**

Which disc is in the drive: none needed.

`scripts/phase0/probe-title.ps1`:

```powershell
# Inspects a title that rip-title.ps1 ripped successfully: identification, stream profiles, subtitle evidence and
# the time of a copy-everything remux on the share (spec 6.4 steps 9, 10 and 13).
# It reads the one .mkv in $Staging\<Slug>\<Label>\ and writes to $Recordings\<Slug>\<Label>\:
#   mkvmerge.json, ffprobe-streams.json, mediainfo.json     identification and stream profiles
#   subs\index.json, subs\sub-<n>.packets.json             subtitle evidence, when the title has subtitles
#   evidence-mkvmerge.txt, remux-mkvmerge.txt               mkvmerge's messages from the extraction and the remux
#   probe-summary.json                                      times, sizes and the verdict
# lazymkv-capture adds .timing.jsonl, .stderr and .exit.json beside each recorded output.
# On the share it writes evidence.mks (kept) and remux-test.tmp (deleted once timed; not a .mkv, so it can never
# be taken for MakeMKV's file). -SkipRemux leaves the remux out. Probe evidence that already exists is refused
# unless -Force is given, which deletes it first; the rip's own files are never touched.
# The last line is "RESULT ok" or "RESULT FAILED: <reason>"; after a failure the script throws.
param(
    [Parameter(Mandatory)][string]$Slug,
    [Parameter(Mandatory)][string]$Label,
    [switch]$SkipRemux,
    [switch]$Force
)
. C:\lazymkv\src\scripts\phase0\env.ps1

$failure = $null
$summary = $null
try {
    Assert-Slug $Slug
    Assert-Slug $Label
    if (-not (Test-Path -LiteralPath $CAP)) { throw "missing $CAP; build it with: scripts/box.sh cargo build --release -p lazymkv-capture" }
    if (-not $MKVMERGE) { throw 'mkvmerge.exe not found' }
    if (-not $FFPROBE) { throw 'ffprobe.exe not found' }
    $rec = "$Recordings\$Slug\$Label"
    $dest = "$Staging\$Slug\$Label"

    # Only a rip that rip-title.ps1 judged good is probed.
    $ripFile = "$rec\rip-summary.json"
    if (-not (Test-Path -LiteralPath $ripFile)) { throw "no $ripFile; rip the title with rip-title.ps1 first" }
    $rip = Get-Content -LiteralPath $ripFile -Raw -Encoding UTF8 -ErrorAction Stop | ConvertFrom-Json
    if ($rip.ok -ne $true) { throw "the rip of label $Label did not succeed ($($rip.result)); only a rip whose rip-summary.json says ok is probed" }

    # Probe evidence is never replaced without -Force; the rip's own files stay.
    # What rip-title.ps1 leaves here, and the audio measurements, are not probe evidence.
    $ripFiles = 'rip.txt', 'rip.txt.timing.jsonl', 'rip.txt.stderr', 'rip.txt.exit.json', 'rip.txt.makemkv-debug.txt',
        'free-ram.csv', 'system.csv', 'system-events.json', 'rip-summary.json'
    $old = @(Get-ChildItem -LiteralPath $rec -Force -ErrorAction Stop | Where-Object { $ripFiles -notcontains $_.Name -and $_.Name -notlike 'audio-levels-*' })
    if ($old.Count -gt 0 -and -not $Force) {
        throw "$rec already holds probe evidence ($(($old | ForEach-Object { $_.Name }) -join ', ')), which is kept; use -Force only where the plan says so"
    }
    foreach ($item in $old) { Remove-Item -LiteralPath $item.FullName -Recurse -Force -ErrorAction Stop }

    # The file is found by listing the folder: it must be the one .mkv that rip-title.ps1 recorded.
    $mkvs = @(Get-ChildItem -LiteralPath $dest -File -Force -ErrorAction Stop | Where-Object { $_.Extension -eq '.mkv' })
    if ($mkvs.Count -ne 1) { throw "expected one .mkv in $dest, found $($mkvs.Count)" }
    $file = $mkvs[0]
    if (@($rip.files) -notcontains $file.Name -or $file.Length -ne [int64]$rip.bytes) {
        throw "$($file.Name) ($($file.Length) bytes) is not the file rip-title.ps1 recorded ($(@($rip.files) -join ', '), $($rip.bytes) bytes)"
    }
    $mkv = $file.FullName
    $evidence = "$dest\evidence.mks"
    $remux = "$dest\remux-test.tmp"
    foreach ($scratch in $evidence, $remux) {
        if (Test-Path -LiteralPath $scratch) { Remove-Item -LiteralPath $scratch -Force -ErrorAction Stop }
    }

    $summary = [ordered]@{
        ok = $false; result = $null; file = $mkv; file_bytes = $file.Length; subtitle_tracks = $null
        evidence_s = $null; evidence_bytes = $null
        remux_skipped = [bool]$SkipRemux; remux_s = $null; remux_exit = $null; remux_bytes = $null; remux_valid = $null; remux_deleted = $null
    }
    "probe: $mkv ($($file.Length) bytes)"

    # Identification and stream profiles: header reads. mkvmerge exits 1 for warnings only.
    & $CAP record --out "$rec\mkvmerge.json" -- $MKVMERGE -J $mkv | Out-Null
    if ($LASTEXITCODE -gt 1) { throw "mkvmerge -J exited $LASTEXITCODE; see mkvmerge.json" }
    $ident = Get-Content -LiteralPath "$rec\mkvmerge.json" -Raw -Encoding UTF8 -ErrorAction Stop | ConvertFrom-Json
    if (@($ident.tracks).Count -eq 0) { throw 'mkvmerge -J lists no tracks; see mkvmerge.json' }
    & $CAP record --out "$rec\ffprobe-streams.json" -- $FFPROBE -v error -show_streams -show_format -of json $mkv | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "ffprobe exited $LASTEXITCODE; see ffprobe-streams.json.stderr" }
    if ($MEDIAINFO) {
        & $CAP record --out "$rec\mediainfo.json" -- $MEDIAINFO --Output=JSON $mkv | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "MediaInfo exited $LASTEXITCODE; see mediainfo.json.stderr" }
    }

    # Subtitle evidence (spec 6.4 step 10): every subtitle track into one small file, then each stream's packets.
    $subs = @($ident.tracks | Where-Object { $_.type -eq 'subtitles' })
    $summary['subtitle_tracks'] = $subs.Count
    $summary['evidence_s'] = 0
    $summary['evidence_bytes'] = 0
    if ($subs.Count -gt 0) {
        New-Item -ItemType Directory -Force -Path "$rec\subs" -ErrorAction Stop | Out-Null
        $ids = ($subs | ForEach-Object { $_.id }) -join ','
        $t0 = Get-Date
        & $CAP record --out "$rec\evidence-mkvmerge.txt" -- $MKVMERGE -q -o $evidence -D -A -s $ids $mkv | Out-Null
        $code = $LASTEXITCODE
        $summary['evidence_s'] = [int]((Get-Date) - $t0).TotalSeconds
        if ($code -gt 1 -or -not (Test-Path -LiteralPath $evidence)) { throw "the subtitle extraction failed (mkvmerge exit $code); see evidence-mkvmerge.txt" }
        $summary['evidence_bytes'] = (Get-Item -LiteralPath $evidence -ErrorAction Stop).Length
        if ($summary['evidence_bytes'] -eq 0) { throw "$evidence is empty" }
        $index = @()
        for ($n = 0; $n -lt $subs.Count; $n++) {
            & $CAP record --out "$rec\subs\sub-$n.packets.json" -- $FFPROBE -v error -select_streams "s:$n" -show_entries 'packet=pts_time,duration_time,size' -of json $evidence | Out-Null
            if ($LASTEXITCODE -ne 0) { throw "ffprobe could not list the packets of subtitle stream s:$n (exit $LASTEXITCODE)" }
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
        Set-Content -LiteralPath "$rec\subs\index.json" -Value (ConvertTo-Json @($index)) -Encoding UTF8 -ErrorAction Stop
    }

    # A copy-everything remux from the share back to the share: the file phase's cost (spec 6.4 step 13, section 18).
    if (-not $SkipRemux) {
        $t0 = Get-Date
        & $CAP record --out "$rec\remux-mkvmerge.txt" -- $MKVMERGE -q -o $remux $mkv | Out-Null
        $code = $LASTEXITCODE
        $summary['remux_s'] = [int]((Get-Date) - $t0).TotalSeconds
        $summary['remux_exit'] = $code
        $summary['remux_bytes'] = 0
        if (Test-Path -LiteralPath $remux) { $summary['remux_bytes'] = (Get-Item -LiteralPath $remux).Length }
        $summary['remux_valid'] = ($code -le 1 -and $summary['remux_bytes'] -gt 0.9 * $file.Length)
        Remove-Item -LiteralPath $remux -Force -ErrorAction SilentlyContinue
        $summary['remux_deleted'] = -not (Test-Path -LiteralPath $remux)
        if (-not $summary['remux_valid']) { throw "the remux failed (mkvmerge exit $code, $($summary['remux_bytes']) of $($file.Length) bytes); see remux-mkvmerge.txt" }
    }
} catch {
    $failure = $_.Exception.Message
}

if ($null -ne $summary) {
    $summary['ok'] = -not $failure
    $summary['result'] = 'ok'
    if ($failure) { $summary['result'] = "FAILED: $failure" }
    $json = $summary | ConvertTo-Json
    try { Set-Content -LiteralPath "$rec\probe-summary.json" -Value $json -Encoding UTF8 -ErrorAction Stop } catch { if (-not $failure) { $failure = "cannot write probe-summary.json: $($_.Exception.Message)" } }
    $json
}
if ($failure) {
    "RESULT FAILED: $failure"
    throw "probe-title.ps1: $failure"
}
'RESULT ok'
```

- [ ] **Step 3: Sync and check the scripts without ripping**

Which disc is in the drive: Disc A (SLUG). The `zz-noshare` check below reads its fingerprint through Assert-DiscIs. Nothing is ripped in this step.

```bash
grep -E '^(function (Assert-Slug|Assert-DiscIs|Assert-EmptyDir|Get-InsertedDisc)|\$Recordings)' scripts/phase0/env.ps1
grep -n -- '-NoProfile -NonInteractive' scripts/phase0/run-interactive.ps1
scripts/box.sh
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -Command \"(Get-Command C:\lazymkv\src\scripts\phase0\rip-title.ps1).Parameters.Keys\""
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -Command \"(Get-Command C:\lazymkv\src\scripts\phase0\probe-title.ps1).Parameters.Keys\""
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\rip-title.ps1 -Slug SLUG -Title 0 -Label Bad_Label"; echo "exit=$?"
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\probe-title.ps1 -Slug SLUG -Label Bad_Label"; echo "exit=$?"
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\rip-title.ps1 -Slug SLUG -Title 0 -Label zz-noshare"; echo "exit=$?"
ssh USER@makemkv-box "if exist C:\lazymkv\phase0\recordings\SLUG\zz-noshare (echo CREATED) else (echo absent)"
ssh USER@makemkv-box "tasklist | findstr /i makemkvcon"
```

Expected, in order:
- The first `grep` prints five lines: the `$Recordings` assignment and the four D1 helper functions. The second prints one line, the `-Argument "-NoProfile -NonInteractive ...` line. Anything else means the prerequisites are missing: stop and report.
- `box.sh` syncs the tree and exits 0.
- The parameter lists begin `Slug`, `Title`, `Label`, `Profile`, `CacheMb`, `Force` and `Slug`, `Label`, `SkipRemux`, `Force`. PowerShell's common parameters follow. A parse error here is a defect in Step 1 or 2: fix it before going on.
- Both `Bad_Label` runs print `RESULT FAILED:` with Assert-Slug's rejection of `Bad_Label` within seconds, then `exit=1`. If the message says that `Assert-Slug` is not recognized, the env.ps1 prerequisites are missing.
- The `zz-noshare` run prints `RESULT FAILED:` within a minute, with the error from creating the staging folder under `\\unas-pro\media\lazymkv-staging\SLUG` (typically `Access is denied` or `Access to the path ... is denied`), then `exit=1`. Over plain ssh the share gives "Access is denied" (fact 5). So this proves that a share that cannot be reached stops the script before MakeMKV starts. Other results:
  - Assert-DiscIs's message: Disc A is not in the drive. Ask the owner to insert it, then run the line again.
  - `MakeMKV is still running`: end it with `ssh USER@makemkv-box "taskkill /im makemkvcon64.exe /f"`, or ask the owner to close the MakeMKV window, then run the line again.
  - Anything else, or a run that goes on for minutes (a rip started): stop and report.
- The `if exist` line prints `absent`. Nothing was created on C:.
- `tasklist` prints nothing.

- [ ] **Step 4: Rehearse on a short title**

Which disc is in the drive: Disc A (SLUG). rip-title.ps1 checks this itself with Assert-DiscIs before MakeMKV starts.

Both scripts run here for the first time, on a short title, so that a defect shows before the hour-long rip. Choose N and K first:

```bash
python3 scripts/phase0/summarize_scan.py corpus/owner/SLUG
```

- N, the main feature: the feature-length title. On a disc whose titles carry labels (the comment column), it is the one whose comment starts `FPL_MainFeature`. If more than one title remains, take the longest and note the others' numbers for EXP-01.
- K, the rehearsal title: the shortest title other than N whose `sub` column is above 0 and whose duration is under 15 minutes. If no title qualifies, take the shortest title other than N. If the disc lists only N, K is N (the rehearsal then takes as long as Step 5).

Check the share from a job. The first command prints how much room this task needs on the share. Task 11 can leave up to five feature-sized files there (`all-tracks`, `share-drop`, `share-drop-2`, `all-tracks-512`, `all-tracks-256`), and Step 6's probe writes one more full-size file (the remux test) for a while. Disc A's total size is an upper bound for each of them.

```bash
python3 -c "import json; b = json.load(open('corpus/owner/SLUG/meta.json', encoding='utf-8-sig'))['total_bytes']; print('Disc A:', round(b / 2**30, 1), 'GB; staging free must be at least', round(6 * b / 2**30, 1), 'GB')"
scripts/phase0/job.sh setup-check.ps1 setup-check-task11
scripts/phase0/wait.sh setup-check-task11
```

What wait.sh's exit codes mean for this job:
- **0**: read the log.
  - It must contain `staging    : \\unas-pro\media\lazymkv-staging : write, rename and delete OK`. If it says `FAILED`, the share is not reachable: tell the owner, and do not start a rip until a new run of this job (log name `setup-check-task11-b`, then `-c`) says OK.
  - Its `staging free:` value must be at least the number the first command printed (6 times Disc A's size: one rip each for all-tracks, the two share drops and the two cache repeats, plus the remux test). If it is less, or says `staging free: FAILED`, tell the owner both numbers and start no rip until they have made room and a new run of this job (log name `setup-check-task11-b`, then `-c`) shows enough.
- **1**: setup-check.ps1 threw (env.ps1 or config.ps1 is broken): stop and report with the `ERROR:` line.
- **3**: still running (it normally takes under a minute). Run wait.sh again.
- **4**: the job is not running and wrote no DONE line: stop and report.
- **5**: the mini PC cannot be reached. Run wait.sh again once it can.

Rip the rehearsal title:

```bash
scripts/phase0/job.sh rip-title.ps1 rip-rehearsal -Slug SLUG -Title K -Label rehearsal
scripts/phase0/wait.sh rip-rehearsal
```

MakeMKV scans the disc again before it writes anything; this takes about as long as Task 9's scan. On a UHD with Java menus that can be ten minutes or more.

What wait.sh's exit codes mean for this job:
- **0**: check the log against the Expected below.
- **1**: read the `RESULT FAILED:` line.
  - A reason from the checks before MakeMKV starts means nothing was ripped. These are: Assert-Slug, `profile not found`, `scan.txt lists no title`, `MakeMKV is still running`, `is not empty`, Assert-DiscIs, or an error creating a folder. Fix the cause as in Step 3, or run setup-check again (log name `setup-check-task11-b`) for a folder on the share, then start the same job again under the log name `rip-rehearsal-b` (see "Log names").
  - Any other reason means the rip ran and the script judged it a failure, or the script has a defect. Pull the recording (`scripts/phase0/pull-recording.sh SLUG`), read `grep -E '^MSG:' corpus/owner/SLUG/rehearsal/rip.txt | cut -c1-160`, and stop and report. Step 5 must not start until the cause is understood. A repeat after the fix uses the same job command with `-Force` added (see "When -Force is right"), under a new log name: add `-b` (then `-c`) to the log name, for example `scripts/phase0/job.sh rip-title.ps1 rip-rehearsal-b -Slug SLUG -Title K -Label rehearsal -Force` and `scripts/phase0/wait.sh rip-rehearsal-b`.
- **3**: still running; this is normal while MakeMKV scans. Run `scripts/phase0/wait.sh rip-rehearsal` again. If 45 minutes pass without a DONE line, end MakeMKV, not the job, with `ssh USER@makemkv-box "taskkill /im makemkvcon64.exe /f"`. The script then writes its summary and fails, and wait.sh exits 1. Then stop and report.
- **4**: the job ended without a DONE line (it never started, or was killed). Check `ssh USER@makemkv-box "tasklist | findstr /i makemkvcon"`, then stop and report.
- **5**: the mini PC cannot be reached. Run wait.sh again once it can.

Expected (exit 0), in the log, in this order (lines printed by the env.ps1 helpers may come first):
- `rip: SLUG title K (scan source file: <file name or empty>) to \\unas-pro\media\lazymkv-staging\SLUG\rehearsal with --cache=1024, started <yyyy-MM-dd HH:mm:ss.fff>`;
- the summary JSON;
- `RESULT ok`;
- `PHASE0-DONE status=ok exit=<n>`. `exit=<n>` is only the last native command's exit code, not a verdict.

In the JSON:
- `ok` is true;
- `files` is a JSON list (`[ ... ]`) holding one name that ends in `.mkv`, and `bytes` is above 0;
- `fail_codes` is an empty list and `profile_errors` is 0;
- `saving_started_s` is a number below `elapsed_s`;
- `ram_samples` is above 0, and `min_free_ram_gb` is a number;
- `rip_source_file` equals `scan_source_file` (the script fails the rip when both are known and differ). If `rip_source_file` is null on this UHD, the rip printed no `MSG:3307` for title K; note that for EXP-01, since experiment 2 depends on those lines.

If `files` appears as an object with `value` and `Count` fields, or `ram_samples` is 0, the script has a defect: stop and report.

Now check the command that Step 8 uses to see when saving begins. Run it on the finished rehearsal recording:

```bash
ssh USER@makemkv-box "powershell -NoProfile -Command \"Get-Date -Format 'yyyy-MM-dd HH:mm:ss.fff'; Get-Content C:\lazymkv\phase0\recordings\SLUG\rehearsal\rip.txt | Where-Object { \$_ -like 'MSG:501[45]*' }\""
```

Expected: the mini PC's current time, then one `MSG:5014,...` line. If the quoting fails, fix it here, not during the share drop.

Probe the rehearsal rip:

```bash
scripts/phase0/job.sh probe-title.ps1 probe-rehearsal -Slug SLUG -Label rehearsal
scripts/phase0/wait.sh probe-rehearsal
```

What wait.sh's exit codes mean for this job:
- **0**: check the log against the Expected below.
- **1**: read the `RESULT FAILED:` line.
  - `did not succeed`: the rip above did not end `RESULT ok`.
  - Any other reason (mkvmerge, ffprobe or MediaInfo, the extraction or the remux): pull the recording and read the file the reason names, for example `corpus/owner/SLUG/rehearsal/remux-mkvmerge.txt`. Then stop and report. A repeat after the fix uses the same command with `-Force` added, under a new log name: add `-b` (then `-c`) to the log name, for example `scripts/phase0/job.sh probe-title.ps1 probe-rehearsal-b -Slug SLUG -Label rehearsal -Force` and `scripts/phase0/wait.sh probe-rehearsal-b`.
- **3**: still running (the probe reads the file twice and writes it once over the network). Run wait.sh again. If an hour passes, end the job, not mkvmerge.
  - Why: a mkvmerge ended with taskkill exits with code 1, which the script reads as "warnings only". An interrupted subtitle extraction would then count as finished, and the probe could end `RESULT ok` with truncated `subs\*.packets.json`.
  - End it with `ssh USER@makemkv-box "powershell -NoProfile -Command Stop-ScheduledTask -TaskName lazymkv-phase0"`.
  - Then run `ssh USER@makemkv-box "tasklist | findstr /i \"mkvmerge ffprobe mediainfo lazymkv\""`. It must print nothing. If it prints a line, end each process it names, for example `ssh USER@makemkv-box "taskkill /im mkvmerge.exe /f"` (likewise `ffprobe.exe`, `MediaInfo.exe` or `lazymkv-capture.exe`), and run the tasklist line again.
  - Run `scripts/phase0/wait.sh probe-rehearsal` once more: it exits 4 (no DONE line, and no probe-summary.json was written). Stop and report.
  - A repeat after the cause is understood uses the same command with `-Force` added, under the log name `probe-rehearsal-b` (see "Log names"); the script itself deletes the partial `evidence.mks` and `remux-test.tmp` on the share before it starts.
- **4**: stop and report.
- **5**: run wait.sh again once the mini PC can be reached.

Expected (exit 0), in the log:
- `probe: \\unas-pro\media\lazymkv-staging\SLUG\rehearsal\<name>.mkv (<bytes> bytes)`;
- the summary JSON: `ok` true, `subtitle_tracks` equal to K's `sub` column, `evidence_bytes` above 0 when there are subtitles, `remux_valid` true and `remux_deleted` true;
- `RESULT ok`, then `PHASE0-DONE status=ok exit=<n>`.

Prove that a finished rip is never replaced without `-Force`:

```bash
scripts/phase0/job.sh rip-title.ps1 rip-rehearsal-again -Slug SLUG -Title K -Label rehearsal
scripts/phase0/wait.sh rip-rehearsal-again
```

What wait.sh's exit codes mean for this job:
- **1**: expected, within a minute. The log shows `RESULT FAILED: \\unas-pro\media\lazymkv-staging\SLUG\rehearsal is not empty: label rehearsal was ripped before and is kept; ...`, an `ERROR:` line and `PHASE0-DONE status=error exit=<n>`. Any other `RESULT FAILED:` reason means the guard was not what stopped it: stop and report.
- **0**: the guard did not hold and a second rip ran: stop and report.
- **3**: the job is still running, so the guard did not stop it. Run wait.sh once more; if it still exits 3, end MakeMKV with `ssh USER@makemkv-box "taskkill /im makemkvcon64.exe /f"`, and stop and report.
- **4**: stop and report.
- **5**: run wait.sh again once the mini PC can be reached.

Copy the recording to the Mac and look at it. Run the commands one at a time, not joined with `&&` and not under `set -e`: compare_tracks.py exits 1 when any verdict is no, which is a finding here, not a failure.

```bash
scripts/phase0/pull-recording.sh SLUG
python3 scripts/phase0/compare_tracks.py corpus/owner/SLUG/scan.txt K corpus/owner/SLUG/rehearsal/mkvmerge.json; echo "compare_tracks exit=$?"
grep -E '^MSG:' corpus/owner/SLUG/rehearsal/rip.txt | cut -d, -f1,4 | sort | uniq -c
python3 -c "import json; print(len(json.load(open('corpus/owner/SLUG/rehearsal/subs/index.json', encoding='utf-8-sig'))), 'tracks in subs/index.json')"
```

Expected:
- `pulled corpus/owner/SLUG (...)`.
- compare_tracks prints the side-by-side table and the lines `TRACK COUNT`, `ORDER MATCH`, `CHAPTERS`, `DURATION`, `VIDEO TRACKS`, `STATISTICS TAGS`, `VERDICT` and the codec reminder.
- The message list has a success summary (`MSG:5005` or `MSG:5036`) and no `MSG:1009`.
- The last command (only when `subtitle_tracks` is above 0) prints the same number as `subtitle_tracks`. A Python error here means index.json is not a valid UTF-8 JSON list: stop and report.

How MakeMKV 2.0.0 reports a bad profile with a disc inserted is unknown (fact 11). So read every distinct message in the list. If `MSG:1009` or any message that mentions the profile appears, MakeMKV did not use the job profile. Stop and report before Step 5, with the message text: the profile may be fixable before an hour of the owner's time is spent. If the track counts differ without any such message, that is experiment 1's own finding: continue.

Note for both reports: K, its duration, and the rehearsal's `elapsed_s`, `saving_started_s` and `ok`.

- [ ] **Step 5: Rip the main feature with the all-tracks profile**

Which disc is in the drive: Disc A (SLUG). rip-title.ps1 checks this itself with Assert-DiscIs before MakeMKV starts.

```bash
scripts/phase0/job.sh rip-title.ps1 rip-all-tracks -Slug SLUG -Title N -Label all-tracks
scripts/phase0/wait.sh rip-all-tracks
```

MakeMKV scans the disc again first (as long as Task 9's scan), then writes; a UHD feature takes roughly an hour more. To see progress at any time:

```bash
ssh USER@makemkv-box "powershell -NoProfile -Command \"Get-Date -Format 'yyyy-MM-dd HH:mm:ss.fff'; Get-Content C:\lazymkv\phase0\recordings\SLUG\all-tracks\rip.txt -Tail 2\""
```

It prints the mini PC's time and the last two lines MakeMKV wrote (`PRGV:current,total,max` while it works).

What wait.sh's exit codes mean for this job:
- **0**: check the log against the Expected below.
- **1**: read the `RESULT FAILED:` line.
  - A reason from the checks before MakeMKV starts (see Step 4): nothing was ripped. Fix the cause and start the same job again under the log name `rip-all-tracks-b` (see "Log names").
  - Any other reason: the rip ran and failed. Pull the recording and read `grep -E '^MSG:' corpus/owner/SLUG/all-tracks/rip.txt | cut -c1-160`. Do not start Step 6. Stop and report with those lines, including the `read_errors` and `write_errors` counts.
  - A repeat is decided with the owner (for example after cleaning the disc, or after setup-check shows the share healthy again). It is the same job command with `-Force` added, after the failed attempt has been kept as "When -Force is right" says, under a new log name: add `-b` (then `-c`) to the log name, for example `scripts/phase0/job.sh rip-title.ps1 rip-all-tracks-b -Slug SLUG -Title N -Label all-tracks -Force` and `scripts/phase0/wait.sh rip-all-tracks-b`.
- **3**: still running. This is normal for well over an hour: run `scripts/phase0/wait.sh rip-all-tracks` again, and start nothing else meanwhile.
  - Run the progress command now and then. MakeMKV is stuck if two progress checks 15 minutes apart show the same last line, or if three hours pass without a DONE line.
  - Then end MakeMKV, not the job: `ssh USER@makemkv-box "taskkill /im makemkvcon64.exe /f"`. The script then writes its summary and fails, and wait.sh exits 1. Stop and report.
- **4**: the job ended without a DONE line. Check `ssh USER@makemkv-box "tasklist | findstr /i makemkvcon"`, then stop and report.
- **5**: the mini PC cannot be reached. Run wait.sh again once it can.

Expected (exit 0):
- The log shows the `rip: SLUG title N ...` line, the summary JSON, `RESULT ok` and `PHASE0-DONE status=ok exit=<n>`.
- In the JSON:
  - `ok` is true;
  - `files` holds one `.mkv` name and `bytes` is above 0;
  - `fail_codes` is empty;
  - `rip_source_file` equals `scan_source_file`, so the rip is the title the scan calls N (a null `rip_source_file` means the rip printed no `MSG:3307` for N; note it for EXP-01);
  - `saving_started_s`, `write_mb_per_s`, `min_free_ram_gb` and `ram_samples` are filled in.
- `RESULT ok` with a `MSG:1009` note means MakeMKV rejected the profile and used its default. Continue, because the rip is still experiment 1's evidence (item 1 is then no), and mention it in the report.

- [ ] **Step 6: Probe the rip and compare it with the scan**

Which disc is in the drive: Disc A stays in. The probe reads only the share.

```bash
scripts/phase0/job.sh probe-title.ps1 probe-all-tracks -Slug SLUG -Label all-tracks
scripts/phase0/wait.sh probe-all-tracks
```

What wait.sh's exit codes mean for this job:
- **0**: check the log: `probe: ...`, the summary JSON with `ok` true and `remux_valid` true, `RESULT ok`, `PHASE0-DONE status=ok exit=<n>`.
- **1**: read the `RESULT FAILED:` line.
  - `already holds probe evidence`: a probe ran before. Use `-Force` only as "When -Force is right" says.
  - Any other reason: pull the recording and read the file it names (`mkvmerge.json`, `evidence-mkvmerge.txt`, `remux-mkvmerge.txt`, or the `.stderr` beside it). Stop and report. The summary still holds the times measured before the failure. A repeat after the fix uses the same command with `-Force` added, under a new log name: add `-b` (then `-c`) to the log name, for example `scripts/phase0/job.sh probe-title.ps1 probe-all-tracks-b -Slug SLUG -Label all-tracks -Force` and `scripts/phase0/wait.sh probe-all-tracks-b`.
- **3**: still running. This is normal: the probe reads the whole file twice and writes it once over the network, which takes 30 to 60 minutes for a UHD feature. Run wait.sh again. If two hours pass, end the job, not mkvmerge.
  - Why: a mkvmerge ended with taskkill exits with code 1, which the script reads as "warnings only". An interrupted subtitle extraction would then count as finished, and the probe could end `RESULT ok` with truncated `subs\*.packets.json`, which Task 12 reads as experiment 6 evidence.
  - End it with `ssh USER@makemkv-box "powershell -NoProfile -Command Stop-ScheduledTask -TaskName lazymkv-phase0"`.
  - Then run `ssh USER@makemkv-box "tasklist | findstr /i \"mkvmerge ffprobe mediainfo lazymkv\""`. It must print nothing. If it prints a line, end each process it names, for example `ssh USER@makemkv-box "taskkill /im mkvmerge.exe /f"` (likewise `ffprobe.exe`, `MediaInfo.exe` or `lazymkv-capture.exe`), and run the tasklist line again.
  - Run `scripts/phase0/wait.sh probe-all-tracks` once more: it exits 4 (no DONE line, and no probe-summary.json was written). Stop and report.
  - A repeat after the cause is understood uses the same command with `-Force` added, under the log name `probe-all-tracks-b` (see "Log names"); the script itself deletes the partial `evidence.mks` and `remux-test.tmp` on the share before it starts.
- **4**: stop and report.
- **5**: run wait.sh again once the mini PC can be reached.

Then copy everything to the Mac and read it. Run the commands one at a time, not joined with `&&` and not under `set -e`. compare_tracks.py exits 1 when any verdict is no, and `grep -c` exits 1 when it counts 0; both are findings, not failures.

```bash
scripts/phase0/pull-recording.sh SLUG
cat corpus/owner/SLUG/all-tracks/rip-summary.json corpus/owner/SLUG/all-tracks/probe-summary.json
python3 scripts/phase0/compare_tracks.py corpus/owner/SLUG/scan.txt N corpus/owner/SLUG/all-tracks/mkvmerge.json; echo "compare_tracks exit=$?"
grep -E '^SINFO:N,[0-9]+,(1|6|7|41|42),' corpus/owner/SLUG/scan.txt
grep -c -E '^SINFO:[0-9]+,[0-9]+,41,' corpus/owner/SLUG/scan.txt
grep -E '^MSG:' corpus/owner/SLUG/all-tracks/rip.txt | cut -d, -f1,4 | sort | uniq -c
python3 - corpus/owner/SLUG/all-tracks <<'EOF'
import json, pathlib, sys
d = pathlib.Path(sys.argv[1])
lines = (d / "rip.txt").read_bytes().split(b"\n")
times = [json.loads(l)["t_ms"] for l in (d / "rip.txt.timing.jsonl").read_text(encoding="utf-8-sig").splitlines() if l.strip()]
prgv = [t for raw, t in zip(lines, times) if raw.startswith(b"PRGV:")]
print("progress lines:", len(prgv), "distinct arrival times:", len(set(prgv)))
gaps = sorted(b - a for a, b in zip(prgv, prgv[1:]))
if gaps:
    print("gap between progress lines in ms: median", gaps[len(gaps) // 2], "largest", gaps[-1])
EOF
python3 -c "import json; json.load(open('corpus/owner/SLUG/all-tracks/mediainfo.json', encoding='utf-8-sig')); print('mediainfo.json reads as UTF-8')"
```

Expected:
- The two summaries: `"ok": true` in both, `rip_source_file` equal to `scan_source_file`, and `remux_valid` true.
- compare_tracks:
  - the side-by-side table;
  - the lines `TRACK COUNT`, `ORDER MATCH`, `CHAPTERS`, `DURATION`, `VIDEO TRACKS` and `STATISTICS TAGS`;
  - then `VERDICT count=... order=... video=... chapters=... duration=...`, the codec reminder line, and `compare_tracks exit=0` (all yes) or `1` (some no).
- The `SINFO` lines give, for each of title N's streams, its type (1), codec short and long names (6, 7), and, where MakeMKV states them, output codec (41) and conversion type (42).
- The `grep -c` prints how many streams in the whole scan have an attribute 41. MakeMKV 1.18 gave it only to converted streams; this count shows whether 2.0.0 does the same.
- The message list holds every distinct message of the rip, with its count. It has a success summary (`5005` or `5036`), none of `5003`, `5004`, `5037`, and no `1009`.
- The timing check: if the distinct arrival times are at least half the progress lines, MakeMKV's lines arrive as they are written, so `saving_started_s` and the share-drop times in Step 8 are accurate. If far fewer, the lines arrive in bursts, and those times are accurate only to about the largest gap; note that in EXP-04.
- `mediainfo.json reads as UTF-8`. A `UnicodeDecodeError` means MediaInfo wrote another encoding. Note it in EXP-01, because Task 12 reads that file as UTF-8.

- [ ] **Step 7: Write the experiment 1 report**

Which disc is in the drive: Disc A stays in.

Create `docs/phase0/experiments/EXP-01-profile-and-order.md` from `docs/phase0/experiments/TEMPLATE.md`.
- Under "What was run": the rehearsal (title K) and the all-tracks rip and probe of title N, with N's source file, plus any other feature-length title numbers noted in Step 4.
- Under "What was observed": compare_tracks' table and the findings below, each stated as observed:

1. Profile honoured: no `MSG:1009`, no other message in Step 6's list that mentions the profile, and the file's track count equal to the scan's stream count (every stream except 3D video): `TRACK COUNT`.
2. Order: `ORDER MATCH`.
3. Output codecs:
   - For each row, record the scan's attribute 42 (conversion type) and attribute 41 where the scan has one. MakeMKV gives 41 only to converted streams, so the table's `out=` column is blank for streams copied unchanged. For those, compare the file's codec with the input codec (attributes 6 and 7).
   - List every row that has a 41, or a 42 other than `( Lossless conversion )`, and every LPCM or closed-caption row, with what each became.
   - State the whole-scan count of attribute 41 from Step 6, and whether MakeMKV 2.0.0 still gives 41 only to converted streams.
4. Chapters: the scan's count against the file's count. MakeMKV's default profile has `insertFirstChapter00IfMissing="true"`.
5. Forced-only streams: from the table's `f=6144` rows, whether each directly follows a non-derived subtitle stream of the same language.
6. Dolby Vision: `VIDEO TRACKS`, and the video codec text in the scan.
7. Statistics tags: `STATISTICS TAGS`.
8. Duration: the `DURATION` line and the `duration=` verdict. Spec 6.2 step 7 verifies a rip to within 2 s of the scan.
9. Language codes: from the table's raw codes, which ISO 639-2 form the file carries for each language (for example `fre` or `fra`, `ger` or `deu`), against the scan's form.
10. Title identity: `rip_source_file` against `scan_source_file`. Also whether `mediainfo.json` reads as UTF-8 (for Task 12).

Answer yes only if 1 to 4 hold. Consequences:
- a no on 1 or 2 switches spec 9.3 to its fallback;
- 5 confirms or corrects the parent rule in spec 8.7;
- 6 fixes spec 8.10's wording;
- 7 decides whether counts can come from the header (spec 6.4 step 10);
- 8 confirms or corrects the 2-second rule of spec 6.2 step 7.

- [ ] **Step 8: Run the share-drop test**

Which disc is in the drive: Disc A (SLUG). rip-title.ps1 checks this itself with Assert-DiscIs.

Choose M. The drop must fall while MakeMKV is writing, and writing must go on long enough for the five-minute repeat as well. List the titles with their estimated writing time at Step 5's rate:

```bash
python3 - corpus/owner/SLUG <<'EOF'
import json, pathlib, sys
sys.path.insert(0, "scripts/phase0")
from robot import parse
d = pathlib.Path(sys.argv[1])
summary = json.loads((d / "all-tracks" / "rip-summary.json").read_text(encoding="utf-8-sig"))
rate = summary.get("write_mb_per_s") or summary.get("mb_per_s")
print("write rate MB/s:", rate)
scan = parse((d / "scan.txt").read_text(encoding="utf-8-sig", errors="replace"))
for t, a in sorted(scan.titles.items(), key=lambda kv: int(kv[1].get(11, "0") or 0)):
    size = int(a.get(11, "0") or 0)
    est = size / 1048576 / rate if rate else 0
    print(f"title {t:>3}  {a.get(9, ''):>8}  {size:>13} bytes  about {est:.0f} s of writing")
EOF
```

M is the first title in this list (the smallest) whose estimate is 600 s or more. The main feature N always qualifies; a 5 to 20 minute extra usually does not.

Tell the owner the plan and ask them to stand by at the NAS:
- the rip starts now;
- MakeMKV scans the disc for several minutes and writes nothing to the share during that time;
- when writing has run for a minute, they will be asked to unplug the NAS's network cable and say when it is out;
- after one minute (five minutes for the repeat) they will be asked to plug it back in and say when it is in.

DROP is `share-drop` the first time. Check that no MakeMKV is left over, then start the rip:

```bash
ssh USER@makemkv-box "tasklist | findstr /i makemkvcon"
scripts/phase0/job.sh rip-title.ps1 rip-DROP -Slug SLUG -Title M -Label DROP
```

Expected: `tasklist` prints nothing (if it prints a line, end that process with `ssh USER@makemkv-box "taskkill /im makemkvcon64.exe /f"` first). job.sh prints `started rip-title.ps1; log: C:\lazymkv\phase0\logs\rip-DROP.txt`.

Every 30 seconds or so, run:

```bash
ssh USER@makemkv-box "powershell -NoProfile -Command \"Get-Date -Format 'yyyy-MM-dd HH:mm:ss.fff'; Get-Content C:\lazymkv\phase0\recordings\SLUG\DROP\rip.txt | Where-Object { \$_ -like 'MSG:501[45]*' }\""
```

It prints the mini PC's time and, once saving has begun, the `MSG:5014` line. Get-Content can read the file while lazymkv-capture is writing it.
- If it reports that it cannot find `rip.txt` for more than two minutes after the job started, the script stopped before MakeMKV started. Run `scripts/phase0/wait.sh rip-DROP`, read the `RESULT FAILED:` line, fix the cause as in Step 4 and start again under the log name `rip-DROP-b` (see "Log names"), in the job.sh and wait.sh commands alike. The owner has not unplugged anything yet.

One minute after the `MSG:5014` line first appears, ask the owner to unplug the NAS's network cable and to say when it is out. At that moment run:

```bash
ssh USER@makemkv-box "powershell -NoProfile -Command \"Get-Date -Format 'yyyy-MM-dd HH:mm:ss.fff'\""
```

Note the printed time as T_UNPLUG. One minute later (five minutes when DROP is `share-drop-2`), ask the owner to plug the cable back in and to say when it is in. Run the same time command and note T_REPLUG. Then:

```bash
scripts/phase0/wait.sh rip-DROP
```

What wait.sh's exit codes mean for this job:
- **1** with `RESULT FAILED:` naming the rip's messages or files: the expected outcome when the outage broke the rip. The failure is the observation, and it is recorded. Go on below.
  - If instead the reason comes from a check before MakeMKV started, no drop was tested. Fix the cause and start this step again with the same DROP, under a new log name: add `-b` (then `-c`) to the log name in the job.sh and wait.sh commands, for example `scripts/phase0/job.sh rip-title.ps1 rip-DROP-b -Slug SLUG -Title M -Label DROP` and `scripts/phase0/wait.sh rip-DROP-b`; its folders were never written. If the folders were written, use `-Force` as "When -Force is right" says, under the same new log name (`... -Label DROP -Force`).
- **0** with `RESULT ok`: SMB rode out the outage. That is the observation for this DROP. Go on below; when DROP is `share-drop`, this step is then repeated once.
- **3**: makemkvcon is still running. Run wait.sh again.
  - Once the mini PC's clock passes T_REPLUG plus 30 minutes (spec 10.4's `exit_wait_min`), end MakeMKV, not the job: `ssh USER@makemkv-box "taskkill /im makemkvcon64.exe /f"`. Then run the time command and note T_KILL.
  - lazymkv-capture then writes `rip.txt.exit.json`, the script writes its summary and fails, and wait.sh exits 1.
  - Do not use Stop-ScheduledTask here: it would lose the summary and the exit record.
  - If taskkill reports an error, stop and report.
  - Report this outcome as "did not exit within 30 minutes of the reconnect".
- **4**: the job ended without a DONE line: check `tasklist` as below, then stop and report.
- **5**: the mini PC cannot be reached. Run wait.sh again once it can.

Before anything else runs, check that the share is back and nothing is left over:

```bash
ssh USER@makemkv-box "tasklist | findstr /i makemkvcon"
scripts/phase0/job.sh setup-check.ps1 setup-check-after-DROP
scripts/phase0/wait.sh setup-check-after-DROP
```

Expected:
- `tasklist` prints nothing. If it prints a line, end that process with `ssh USER@makemkv-box "taskkill /im makemkvcon64.exe /f"` and run `tasklist` again.

What wait.sh's exit codes mean for the setup-check job:
- **0**: the log must contain `staging    : \\unas-pro\media\lazymkv-staging : write, rename and delete OK`. If the staging line says `FAILED`, the share is not back: ask the owner to check the NAS, run the setup-check job again after a few minutes under the log name `setup-check-after-DROP-b` (then `-c`), and start no rip until it says OK.
- **1**: setup-check.ps1 threw: stop and report with the `ERROR:` line.
- **3**: still running (it normally takes under a minute). Run wait.sh again.
- **4**: stop and report.
- **5**: run wait.sh again once the mini PC can be reached.

Copy the recording to the Mac and print the timeline. If makemkvcon was ended with taskkill, add the argument `"kill=T_KILL"` after `"replug=T_REPLUG"`.

```bash
scripts/phase0/pull-recording.sh SLUG
python3 - corpus/owner/SLUG/DROP "unplug=T_UNPLUG" "replug=T_REPLUG" <<'EOF'
import datetime, json, pathlib, sys
d = pathlib.Path(sys.argv[1])
def read(name):
    p = d / name
    return p.read_text(encoding="utf-8-sig", errors="replace") if p.exists() else None
summary = json.loads(read("rip-summary.json") or "{}")
started = summary.get("started")
fmt = "%Y-%m-%d %H:%M:%S.%f"
events = []
for arg in sys.argv[2:]:
    name, _, when = arg.partition("=")
    if not started:
        print(f"{name} at {when}: cannot be placed, rip-summary.json has no started time")
        continue
    s = (datetime.datetime.strptime(when, fmt) - datetime.datetime.strptime(started, fmt)).total_seconds()
    events.append((s * 1000, f"{s:9.1f}s  == {name} ({when})", name, s))
lines = (d / "rip.txt").read_bytes().split(b"\n")
times = [json.loads(l)["t_ms"] for l in (read("rip.txt.timing.jsonl") or "").splitlines() if l.strip()]
rows = []
last = None
for raw, t in zip(lines, times):
    text = raw.rstrip(b"\r").decode("utf-8", "replace")
    if text.startswith("PRGV:"):
        if last is not None and t - last > 10000:
            rows.append((last, f"{last / 1000:9.1f}s .. {t / 1000:.1f}s  no progress"))
        last = t
    elif text.startswith("MSG:"):
        rows.append((t, f"{t / 1000:9.1f}s  {text[:160]}"))
for row in sorted(rows + [(e[0], e[1]) for e in events], key=lambda r: r[0]):
    print(row[1])
print("started (mini PC clock):", started)
print("last progress line at", "none" if last is None else f"{last / 1000:.1f}s")
exit_text = read("rip.txt.exit.json")
print("exit.json:", exit_text.strip() if exit_text else "missing")
if exit_text:
    end = json.loads(exit_text)["elapsed_ms"] / 1000
    for _, _, name, s in events:
        print(f"makemkvcon ended {end - s:.1f}s after {name}")
print("result:", summary.get("result"))
EOF
```

Expected: every message of the call in order, with its time in seconds from the start of the call. Between them are the `== unplug` and `== replug` marks and any `no progress` stretch of more than 10 s. Then come the last progress time, `exit.json` (`exit_code`, `elapsed_ms`), how long makemkvcon ran after each mark, and the RESULT. The offsets are accurate to about a second, or to the burst size found in Step 6.

When DROP was `share-drop` and the result was `RESULT ok` with no error message in the timeline, run this step once more from "Tell the owner the plan", with DROP = `share-drop-2`, the same M, and a five-minute outage. Ask the owner first, since this is longer than the one minute agreed for the plan. If `share-drop-2` also rides it out, record that and do not repeat again.

- [ ] **Step 9: Repeat the rip at lower cache sizes, if needed**

Which disc is in the drive: Disc A (SLUG). rip-title.ps1 checks this itself with Assert-DiscIs.

Experiment 4 at `cache_mb` 1024 answers yes if both of these hold:
- `min_free_ram_gb` in `corpus/owner/SLUG/all-tracks/rip-summary.json` is 1 or more;
- no slow-write message is in Step 6's full message list. Read every distinct message there: any that mentions writing, speed, buffer, disk or output counts. `MSG:2008` is not a confirmed code.

If the answer is yes, skip this step and write "not needed: the 1024 run answered yes" in EXP-04.

If it is no, run this step with CACHE = 512; if that run still answers no by the same test, run it again with CACHE = 256. First check that nothing is left over and that the share is healthy:

```bash
ssh USER@makemkv-box "tasklist | findstr /i makemkvcon"
scripts/phase0/job.sh setup-check.ps1 setup-check-before-CACHE
scripts/phase0/wait.sh setup-check-before-CACHE
```

Expected: `tasklist` prints nothing; if it prints a line, end that process with `ssh USER@makemkv-box "taskkill /im makemkvcon64.exe /f"` first.

What wait.sh's exit codes mean for the setup-check job:
- **0**: the log must contain `staging    : \\unas-pro\media\lazymkv-staging : write, rename and delete OK`. If it says `FAILED`, ask the owner to check the NAS, run the setup-check job again after a few minutes under the log name `setup-check-before-CACHE-b` (then `-c`), and start no rip until it says OK.
- **1**: setup-check.ps1 threw: stop and report with the `ERROR:` line.
- **3**: still running (it normally takes under a minute). Run wait.sh again.
- **4**: stop and report.
- **5**: run wait.sh again once the mini PC can be reached.

Then:

```bash
scripts/phase0/job.sh rip-title.ps1 rip-all-tracks-CACHE -Slug SLUG -Title N -Label all-tracks-CACHE -CacheMb CACHE
scripts/phase0/wait.sh rip-all-tracks-CACHE
```

What wait.sh's exit codes mean for this job:
- **0**: the log shows the `rip:` line, the summary JSON with `ok` true and `cache_mb` equal to CACHE, `RESULT ok`, and `PHASE0-DONE status=ok exit=<n>`.
- **1**: read the `RESULT FAILED:` line.
  - A reason from the checks before MakeMKV starts (Assert-Slug, `profile not found`, `scan.txt lists no title`, `MakeMKV is still running`, `is not empty`, Assert-DiscIs, an error creating a folder): nothing was ripped. Fix the cause as in Step 4 and start the same job again under the log name `rip-all-tracks-CACHE-b` (see "Log names").
  - `title N is <file> in this rip but <file> in scan.txt`: the scan was made with `--cache=1024`, and spec 9.2 wants identical flags for scan and rip. So `--cache` changed MakeMKV's title numbering. Record that in EXP-04 and stop the repeats.
  - Any other reason: the rip ran and failed. Pull the recording, read `grep -E '^MSG:' corpus/owner/SLUG/all-tracks-CACHE/rip.txt | cut -c1-160`, and stop and report.
- **3**: still running; normal for well over an hour. Run wait.sh again. To see progress: `ssh USER@makemkv-box "powershell -NoProfile -Command \"Get-Date -Format 'yyyy-MM-dd HH:mm:ss.fff'; Get-Content C:\lazymkv\phase0\recordings\SLUG\all-tracks-CACHE\rip.txt -Tail 2\""`. If two such checks 15 minutes apart show the same last line, or three hours pass without a DONE line, end MakeMKV with `ssh USER@makemkv-box "taskkill /im makemkvcon64.exe /f"` (not the job), run wait.sh again (exit 1), and stop and report.
- **4**: the job ended without a DONE line. Check `ssh USER@makemkv-box "tasklist | findstr /i makemkvcon"`, then stop and report.
- **5**: run wait.sh again once the mini PC can be reached.

Then:

```bash
scripts/phase0/pull-recording.sh SLUG
cat corpus/owner/SLUG/all-tracks-CACHE/rip-summary.json
grep -E '^MSG:' corpus/owner/SLUG/all-tracks-CACHE/rip.txt | cut -d, -f1,4 | sort | uniq -c
```

Expected: the summary (`min_free_ram_gb`, `write_mb_per_s`) and every distinct message of this run, read with the same slow-write test.

- [ ] **Step 10: Write the experiment 4 report**

Which disc is in the drive: Disc A stays in.

Create `docs/phase0/experiments/EXP-04-rip-to-share.md` from `docs/phase0/experiments/TEMPLATE.md`. It must state:

- The all-tracks rip, from `rip-summary.json`:
  - `started`, `elapsed_s`, and `saving_started_s` (MakeMKV's own scan before saving);
  - `bytes` and `write_mb_per_s` as the throughput, with `mb_per_s` as the rate for the whole call;
  - `free_ram_before_gb`, `min_free_ram_gb`, `min_free_ram_at_s` (before or after `saving_started_s`) and `ram_samples`;
  - free RAM is `Win32_OperatingSystem.FreePhysicalMemory`, which on this machine includes the standby cache, so a filling file cache does not lower it;
  - Step 6's timing check: whether lines arrive as they are written or in bursts.
- Slow-write messages: every distinct message from Step 6's full list that mentions writing, speed, buffer, disk or output, with its code and count, or "none". Note that no slow-write code was known beforehand.
- The file-phase network cost, from `probe-summary.json`:
  - `evidence_s` and `evidence_bytes` (one full read);
  - `remux_s`, only if `remux_valid` is true (one full read and one full write);
  - their sum, against spec section 18's estimate of 25 to 40 minutes per title.
- Each share-drop run (`share-drop`, and `share-drop-2` if it ran):
  - M and its size;
  - T_UNPLUG and T_REPLUG as offsets from `started`;
  - the message sequence with offsets from the timeline;
  - the last progress line's time;
  - how long makemkvcon ran after the unplug and after the replug, or "did not exit within 30 minutes of the reconnect" when it was ended with taskkill;
  - `exit_code` from `rip.txt.exit.json`, and the RESULT line.
- Each `-CacheMb` repeat from Step 9 with the same memory, throughput and message facts, or "not needed".
- The rehearsal (title K) under "What was run".

Answer yes if free RAM stayed at 1 GB or more and no slow-write message appeared at `cache_mb` 1024. Consequences:
- the working `cache_mb` (spec section 14): 1024 on yes, otherwise the largest value whose repeat answered yes;
- the share-drop message codes for the `Output` class (spec 10.2);
- whether makemkvcon exits after a drop, for spec 10.4's `exit_wait_min`;
- the measured file-phase time replaces the estimate in spec section 18.

- [ ] **Step 11: Add this task's rips to the tuning dataset (decision D16)**

In this step SLUG and LABEL stand for the slug and the label of the title being added, whichever disc it belongs to; they replace Task 11's placeholders of the same names here.

Which disc is in the drive: Disc A stays in (not used). The jobs read only the share.

List the rips still to add:

```bash
python3 scripts/phase0/audio_dataset.py | grep '^Titles whose audio'
```

Expected: one line `Titles whose audio is in the tuning dataset (audio-proxy.json): <a> of <b> ...; missing: ...`. Every SLUG/LABEL it names under `missing` is a rip of this task that ended `RESULT ok` (normally `rehearsal` and `all-tracks`, and any `share-drop`, `share-drop-2`, `all-tracks-512` or `all-tracks-256` that succeeded), or a title of an earlier task. Do the following for each SLUG/LABEL named, one title at a time.

```bash
python3 scripts/phase0/audio_dataset.py args corpus/owner/SLUG LABEL
```

Expected: `SLUG/LABEL: <n> audio streams (from <source>); original language <...>`, one `stream <n>: <codec> ... lang=<code>` line per audio stream, a `programme reference: ...` line (and possibly `note: ...` and `dataset only: ...` lines), then one or more pairs of lines `scripts/phase0/job.sh audio-levels.ps1 audio-levels-SLUG-LABEL[-M] -Slug SLUG -Label LABEL -Main M[ -Cand C,...]` and `scripts/phase0/wait.sh audio-levels-SLUG-LABEL[-M]`. `<source>` is `ffprobe-streams.json`, or for a rip Steps 8 and 9 did not probe, `<other label>/ffprobe-streams.json: a rip of the same title <t> with the same size, <b> bytes`. When neither exists, `args` exits 2 and prints instead the two lines `scripts/phase0/job.sh probe-title.ps1 probe-LABEL -Slug SLUG -Label LABEL -SkipRemux` and its `wait.sh` line: run them (with Step 6's exit-code handling), pull, and run `args` again. Run the printed audio-levels pairs in order, one job at a time.

What wait.sh's exit codes mean for these jobs:
- **0**: check the log against the Expected below.
- **1**: read the `RESULT FAILED:` line.
  - Assert-Slug, `is the main stream and cannot also be a candidate`, `a candidate stream is listed twice`, `the file has no stream` or `is ... not audio`: the job line was not copied as printed. Start it again as printed, with `-b` added to the log name in both commands.
  - `already exists and is kept as evidence`: this title was measured before by a run that finished. Go on with the next pair; the harness below confirms that the title is no longer missing (if it still is, stop and report).
  - `is from a run that did not finish (it has no audio-proxy.json ...), or from before D16: repeat with -Force`: an earlier run of this pair failed after making its folders. If that failure was not reported yet, pull the recording and read its `ffmpeg.txt` first. Then start the job again as printed with `-Force` added and `-b` in the log name. A finished proxy of the same file is kept (`proxy: kept ...`).
  - `describes a proxy of ... but staging holds ...`: the dataset holds a proxy of another rip of this label. Stop and report; only the controller decides on `-Force` here, after comparing the two files.
  - `cannot reach the staging share`, `cannot reach the tuning dataset`, `Access ... is denied` or `Cannot find path`: run `scripts/phase0/job.sh setup-check.ps1 setup-check-tuning-LABEL` and `scripts/phase0/wait.sh setup-check-tuning-LABEL`. Its staging and tuning lines must say `write, rename and delete OK`. wait.sh exits 0: read the log; 1 or 4: stop and report with the log; 3: run wait.sh again; 5: run it again once the mini PC can be reached. If one says `FAILED`, ask the owner to check the NAS and run the check again with `-b`. Then start the audio-levels job again with `-b`, with `-Force` only if the job now says `did not finish`.
  - `expected one .mkv in ... found 0`: the rip is no longer in staging. Write "LABEL: not in the tuning dataset (rip deleted)" in this task's report and go on.
  - `ffprobe failed`, `ffmpeg failed`, `streamhash.txt holds`, `ffmpeg wrote no raw-`, `holds no level`, `proxy check failed` (including `may be clipped`), `the copy of ... in the dataset differs` or `the ffmpeg command line is`: pull the recording and read `corpus/owner/SLUG/LABEL/audio-levels-20ms-*/ffmpeg.txt` (`ffprobe.txt` after `ffprobe failed`) and `command.txt`; stop and report. `.partial` files on the share are removed by the script. A repeat after the fix adds `-Force`: these failures happen after the folders were made, and only for them is `-Force` right.
  - Anything else: stop and report with the log.
- **3**: still running. The job reads the whole file from the share once: a few minutes for an episode, and an hour or more for a UHD feature. Run wait.sh again, and start nothing else meanwhile. If three hours pass without a DONE line, end ffmpeg (not the job) with `ssh USER@makemkv-box "taskkill /im ffmpeg.exe /f"` and run wait.sh again: it exits 1 with `ffmpeg failed`. Stop and report.
- **4**: the job ended without a DONE line: stop and report.
- **5**: the mini PC cannot be reached. Run wait.sh again once it can.

Expected (exit 0), in the log:
- possibly `dataset: created \\unas-pro\media\lazymkv-tuning` and `dataset: removed ..., left by an interrupted run` first;
- `levels: <file>.mkv main=M cand=<C or nothing> centre=<True|False> ffmpeg_exit=0 seconds=<s> folder=audio-levels-20ms-M[-C...]`. Note `seconds` with the title's running time and size: the J4125's cost of the pass;
- one `stream <n>: ... frames=<n> md5=<32 hex characters>` line for M and for each candidate. A line ending `(layout guessed by ffmpeg: <name>)` is a stream whose layout ffprobe could not name (LPCM in Matroska reports `unknown`): note it for EXP-06;
- `proxy: \\unas-pro\media\lazymkv-tuning\SLUG\LABEL\audio-proxy.mka tracks=<k> bytes=<b> (<n> mix, <n> fc, <n> side, ...)`: one `mix` for every audio stream of the file, in file order, one `fc` after each stream whose layout has a centre channel (5.1, 7.1), and one `side` after each two-channel stream. `bytes` is about 51 MB per hour of running time per track, and between 40 and 70 MB; outside that range, note it. A second job on the same LABEL prints `proxy: kept ... (made by an earlier run from this same file)` instead;
- `dataset: \\unas-pro\media\lazymkv-tuning\SLUG\LABEL\audio-levels-20ms-...`, `RESULT ok`, `PHASE0-DONE status=ok exit=<n>`.

When every pair of the title has ended `RESULT ok`:

```bash
scripts/phase0/pull-recording.sh SLUG
python3 scripts/phase0/audio_labels.py auto corpus/owner/SLUG
python3 scripts/phase0/audio_dataset.py; echo "exit=$?"
```

Expected:
- `pulled corpus/owner/SLUG (...)`.
- `auto` prints a line `labelled LABEL stream <n>: duplicate (md5), counts=False (a duplicate track never counts)` or `... derived-core (flag), counts=True` for each such track, or `nothing to label automatically`. A line `LABEL: derived cores not labelled: the file has <a> audio streams and the scan's title <t> has <b>` means the streams could not be matched by position. Label the cores by hand from Step 6's compare_tracks table (`audio_labels.py set ... derived-core --how flag --saw-verdict no`).
- The harness prints its sections, then `RESULT ok` and `exit=0`. LABEL is no longer under `missing` in its last list. A note `... takes its label from <label> stream <n> (same packets)` is normal for a title ripped twice. A note `<slug>/<label>/<folder>: the reference, stream M, is not the first <lang> stream of the file ...` is not a failure: name it in the report.
- `exit=1` with `RESULT FAILED: ...`: read its `FAIL` lines and stop and report with them.
  - `labelled <kind> (programme audio), verdict <commentary|commentary-cross-language|duplicate>`: a track the owner labelled as programme audio got a drop verdict (`commentary`, `duplicate`) or the commentary pattern against a reference in another language (`commentary-cross-language`: kept, but a false verdict). On a counted track this is experiment 13's answer "no" in the making; otherwise it may be a wrong label.
  - `wrong reference: measured against stream <M> ..., but the top-ranked <lang> stream is <X> (...), the title's programme reference (placed first in ...)`: that level folder was measured with another `-Main` than `args` prints. Its tracks are left out.
  - `the reference is labelled <kind> (unwanted audio)`: a label is on the wrong stream, or the reference is not what its ranking says.
  - `swapped reference: labelled <kind> ...`: stop and report with the track's measures (EXP-13 addendum 9.3).
- `exit=2`: a labels file (or `corpus/owner/audio-fitted.json`) does not validate, or the spec's row 13 can no longer be read. The message names the file and the problem. Repair it only with `audio_labels.py set` or `unset` (each repairs one entry and prints the problems left), and run again.
- `exit=3` with `RESULT FAILED: not everything was checked: ...`: read `Folders that cannot be read`, `Labels without a measurement` and the `gone:` line.
  - `its level files are not here`: pull the recording again (`pull-recording.sh SLUG`); if it is still missing, stop and report.
  - A label without a measurement whose stream number is wrong: `python3 scripts/phase0/audio_labels.py unset corpus/owner/SLUG LABEL <stream> --note "<why>"`, then label the right stream.
  - `gone: ...`: tracks of the last committed snapshot are no longer measured. Stop and report. The baseline snapshot is not replaced by such a run.

The owner's labels: the harness's `To label` list names every measured track without a label, with its position in a player and its track name (and `they disagree` when two rips with the same packets were labelled differently). When the owner has time (this does not hold up the next step), ask what each track is, as Task 12 Step 18 describes, without showing any verdict. Then record each answer, and once per title its kind:

```bash
python3 scripts/phase0/audio_labels.py kinds
python3 scripts/phase0/audio_labels.py set corpus/owner/SLUG LABEL STREAM KIND --how HOW --saw-verdict no [--sure no] --note "<the owner's words>"
python3 scripts/phase0/audio_labels.py title corpus/owner/SLUG LABEL --title-kind TITLE_KIND --dialogue-heavy DIALOGUE
python3 scripts/phase0/audio_dataset.py; echo "exit=$?"
```

Placeholders in these two lines: STREAM is the ffprobe stream number from the harness's To-label line; KIND one of the kinds `python3 scripts/phase0/audio_labels.py kinds` lists; HOW one of listened, packaging, menu, flag, thediscdb or md5; TITLE_KIND one of feature, episode, extra or other; DIALOGUE one of yes, no or unknown.

Add `--film-audible yes` or `no` for a commentary (is the film clearly audible under the voices?). Add `--sure no` when the owner is guessing ("probably"); the label is kept and does not count. Use `--saw-verdict yes` if the owner had seen a verdict for that track: the label is then kept but does not count for experiment 13. Expected: `labelled LABEL stream <n>: <kind> (<how>, <date>, saw verdict no, sure yes), counts=True`, or `counts=False (<why>)` for a duplicate, an `inferred` or unsure label, or a disc or track the thresholds were fitted on; `title LABEL: <kind>, dialogue_heavy=<...>`; and the harness as above. A label given again replaces the old one, which is kept in its history (`replaces 1 earlier label(s)`). The same title ripped again under another label (a cache repeat) takes its labels from identical packets and needs none.

In this step's Commit (Step 12), `git status --short` also lists `corpus/owner/SLUG/audio-labels.json` (once labels exist), `corpus/owner/audio-dataset.json`, and in each new `audio-levels-20ms-*` folder `streams.json`, `proxy-check.json` and `audio-proxy.json`. No `*.txt` level file appears.

- [ ] **Step 12: Commit**

Which disc is in the drive: does not matter. Disc A stays in for Task 12.

```bash
find corpus/owner -iname '*unit_key*'
grep -rIl 'eyJhbGciOi' corpus/owner
git add scripts/phase0/rip-title.ps1 scripts/phase0/probe-title.ps1 corpus/owner docs/phase0/experiments
git status --short
git commit -m "Run experiments 1 and 4: all-tracks rip to the share" -m "Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

Expected:
- `find` and `grep` print nothing: no AACS unit-key file and no JSON Web Token in any recording. If either prints a path, stop and do not commit.
- `git status --short` lists the two new scripts, new files under `corpus/owner/SLUG/` (`rehearsal/`, `all-tracks/`, `share-drop/`, and any of `share-drop-2/`, `all-tracks-512/`, `all-tracks-256/` or `LABEL-failed-1/` folders that exist) and the two reports. `run-interactive.ps1` is not listed: the controller committed it as a prerequisite.
- The commit succeeds.

---

### Task 12: Audio profiles and subtitle classification (experiment 6)

**Files:**
- Create: `scripts/phase0/probe-audio.ps1`
- Create: `docs/phase0/experiments/EXP-06-audio-profiles-and-subtitles.md`
- Create (pulled from the mini PC): `corpus/owner/SLUG_A/all-tracks/ffprobe-audio-deep.json` with its `.timing.jsonl`, `.stderr` and `.exit.json`; and, for each disc added in Steps 6 to 8, `corpus/owner/NEW/` (the recording, when it is new) and `corpus/owner/NEW/all-tracks/`.
- Create or append to (on the Mac only; a pull never removes it): `corpus/owner/SLUG/notes.md` for every disc measured, with the owner's answers.
- Prerequisite, applied and committed by the controller before this task: `scripts/phase0/subtitle_alignment.py` (`--subs-dir`) and `scripts/phase0/test_analysis.py`.
- Experiment 14 (Steps 22 to 32; the full list is in the "Experiment 14 (added 2026-10-06)" paragraph before Step 22):
  - Prerequisite, committed by the controller: `docs/phase0/experiments/EXP-14-design.md`, `scripts/phase0/subtitle_text.py`, `scripts/phase0/test_subtitle_text.py` and the changes to `scripts/phase0/ocr-subs.ps1`.
  - Modify: `.gitignore` (two lines) and the proof of concept's `corpus/owner/<slug>/<label>/subs-ocr/summary.json` files (their example lines removed).
  - Create: `docs/phase0/experiments/EXP-14-subtitles-by-text.md`; `corpus/owner/<slug>/<label>/subs-ocr/` (pulled, no text) and `corpus/owner/<slug>/<label>/subtitle-text.txt` (numbers only) for every title read.

**Interfaces:**
- Consumes:
  - From `env.ps1`: `$P0`, `$Recordings`, `$Staging`, `$CAP`, `$FFPROBE`, `Assert-Slug`, `Get-InsertedDisc` (the scripts below use the rest).
  - `lazymkv-capture record --out <file> -- <command...>`: writes `<file>` (the command's standard output), `<file>.timing.jsonl`, `<file>.stderr` and, when the command has ended, `<file>.exit.json`. Its own exit code is the command's, or 2 when the tool itself fails.
  - Task 8: `record-disc.ps1 -Slug <slug>` run as a job (a new slug gets snapshot, lookups and `scan.txt`; a slug recorded without a scan keeps its snapshot and gains `scan.txt`; last line `RESULT ok` or `RESULT FAILED: <reason>`), and `scripts/phase0/pull-recording.sh <slug>` (never deletes on the Mac; exit 2 bad slug, 1 failed copy).
  - Task 11: `rip-title.ps1 -Slug <slug> -Title <n> -Label <name> [-Force]` (writes `$Recordings\<slug>\<label>\rip-summary.json` and the rip to `$Staging\<slug>\<label>\`; checks the disc with Assert-DiscIs), and `probe-title.ps1 -Slug <slug> -Label <name> -SkipRemux [-Force]` (writes `mkvmerge.json`, `ffprobe-streams.json`, `mediainfo.json`, `subs\index.json`, `subs\sub-<n>.packets.json`, `probe-summary.json`).
  - Scripts: `scripts/box.sh`, `scripts/phase0/job.sh`, `scripts/phase0/wait.sh`, `scripts/phase0/setup-check.ps1`, `scripts/phase0/summarize_scan.py <recording dir>`, `scripts/phase0/robot.py` (`parse`), `scripts/phase0/subtitle_alignment.py --subs-dir <subs dir> [--clear-max-bytes <n>] [--tolerance-ms <n>]` (prerequisite above; exit 2 when the folder has no `index.json` or no English stream).
  - Disc A's recording from Tasks 9 and 11: `corpus/owner/SLUG_A/scan.txt`, `corpus/owner/SLUG_A/all-tracks/` (`rip-summary.json`, `probe-summary.json`, `ffprobe-streams.json`, `mediainfo.json`, `subs/`), and the ripped file in `$Staging\SLUG_A\all-tracks\` (`\\unas-pro\media\lazymkv-staging\SLUG_A\all-tracks\`).
- Produces:
  - `scripts/phase0/probe-audio.ps1 -Slug <slug> -Label <name> [-Force]`, run as a job: `scripts/phase0/job.sh probe-audio.ps1 <log> -Slug <slug> -Label <name>`.
    - It refuses, before anything is deleted or written, when `-Slug` or `-Label` fails Assert-Slug, `$CAP` or ffprobe is missing, `<rec>\rip-summary.json` is missing or does not say `"ok": true`, its own evidence already exists and `-Force` was not given, or `$Staging\<slug>\<label>\` does not hold exactly one `.mkv` that is the file rip-title.ps1 recorded (name and size). `<rec>` is `$Recordings\<slug>\<label>`.
    - It runs `ffprobe -v error -probesize 100M -analyzeduration 100M -select_streams a -show_streams -of json <file>` through `lazymkv-capture record` and writes `<rec>\ffprobe-audio-deep.json` with `.timing.jsonl`, `.stderr` and `.exit.json` beside it. Nothing is written to the share. `-Force` deletes those four files first.
    - It fails when lazymkv-capture wrote no `.exit.json`, ffprobe exited other than 0, or the output lists no audio stream.
    - Output lines: `probe-audio: <file> (<bytes> bytes)`, one `audio <index>: <codec> profile=<profile> channels=<n> layout=<layout>` per audio stream, then `RESULT ok` or `RESULT FAILED: <reason>`. After a failure the script throws, so wait.sh exits 1 and a plain ssh run exits 1.
  - For each disc measured, a `## Experiment 6 (Task 12)` section in `corpus/owner/<slug>/notes.md`: the owner's Atmos and DTS:X answers and the owner's classification of each English subtitle stream.
  - Recordings of the discs added in Steps 6 to 8 (at least a disc with commentary subtitles and a DVD with closed captions), each with `all-tracks/`.
  - The measured values of `subs_same_min`, `subs_diff_max` and `sub_clear_max_bytes` (spec 8.7, section 14), and the `ffprobe` and MediaInfo strings for Atmos and DTS:X, in `docs/phase0/experiments/EXP-06-audio-profiles-and-subtitles.md`.

Placeholders:
- SLUG_A: Disc A's slug, chosen in Task 9 and written at the top of the EXP-05 and EXP-12 reports; `ls corpus/owner` shows it.
- SLUG: in Steps 3 to 5, the disc being measured. It is SLUG_A the first time; when Step 8 repeats Steps 3 to 5, it is NEW. In Step 10 it stands for each measured disc in turn (SLUG_A and every NEW that was ripped): run the command once for each.
- NEW: in Steps 6 to 8, the disc being added. It is a new slug chosen in Step 6 from the title, year and format the owner gives, using lowercase letters, digits and hyphens only and not already shown by `ls corpus/owner`, for example `heat-1995-bluray`. If Step 6's fingerprint check finds the disc already recorded (in Task 8) under a slug S, NEW is S. NEW is never SLUG_A.
- FINGERPRINT: the 32 hexadecimal characters printed by the first command of Step 6.
- M: NEW's main feature title number, chosen in Step 6 from `summarize_scan.py`'s table.
- CLEAR: the `sub_clear_max_bytes` value chosen in Step 10 (0, 32, 64, 128 or 256).
- The kinds of disc asked for in Step 6: (a) a disc with a commentary subtitle track; (b) a DVD with closed captions; (c) a disc with an Atmos or a DTS:X track, only while one of the two formats has no owner-confirmed stream; (d) one more disc with two or more English subtitle tracks, only while fewer than three measured discs qualify (Step 9). One disc may serve several kinds.
- Disc A: the UHD film of Tasks 9 to 11. It is in the drive when this task starts (Task 11 left it in) and stays there until Step 6. Task 13 starts by asking the owner to put it back.

Log names: every job has its own log name, built from the slug, so no earlier log is lost (run-interactive.ps1 deletes a log of the same name before it starts a job). When a job is started again after a failure, add `-2` to its log name (then `-3`, and so on).

When `-Force` is right (decision D3):
- `rip-title.ps1 ... -Force`: only to replace a failed or interrupted attempt at NEW's `all-tracks` label, once its cause has been dealt with and the owner has agreed to the repeat. Such an attempt has a `rip-summary.json` that is missing or says `"ok": false`; check with `ssh USER@makemkv-box "type C:\lazymkv\phase0\recordings\NEW\all-tracks\rip-summary.json"`. Before using it, pull the recording (`scripts/phase0/pull-recording.sh NEW`) and keep the failed attempt with `cp -R corpus/owner/NEW/all-tracks corpus/owner/NEW/all-tracks-failed-1` (`-failed-2` if that exists). Never over a summary that says `"ok": true`, and never for SLUG_A in this task.
- `probe-title.ps1 ... -Force`: only to repeat a probe of NEW whose `probe-summary.json` is missing or says `"ok": false`, once its cause has been dealt with.
- `probe-audio.ps1 ... -Force`: only to repeat a deep read that failed after ffprobe started (its `RESULT FAILED:` reason names ffprobe, lazymkv-capture or the output), once its cause has been dealt with. A refusal before that wrote nothing and is repeated without `-Force`.
- `record-disc.ps1 -Force` is not used in this task. No other use of `-Force` is right.

- [ ] **Step 1: Write the deeper audio probe script**

Which disc is in the drive: none needed (Disc A is still in from Task 11 and stays).

probe-title.ps1 read the audio streams with ffprobe's default probe size. If ffprobe gives no Atmos or DTS:X profile, that could be its answer or only a read that was too short. This script reads the audio streams once more, probing deeper, so that experiment 6 can tell the two apart.

`scripts/phase0/probe-audio.ps1`:

```powershell
# Reads the audio streams of a title that rip-title.ps1 ripped successfully once more with ffprobe, this time
# probing deeper than probe-title.ps1 does (-probesize 100M -analyzeduration 100M). Experiment 6 compares the two
# reads, so that a missing Atmos or DTS:X profile is known to be ffprobe's answer and not a read that was too short.
# It reads the one .mkv in $Staging\<Slug>\<Label>\ and writes, in $Recordings\<Slug>\<Label>\
# (C:\lazymkv\phase0\recordings\<Slug>\<Label>\), ffprobe-audio-deep.json with .timing.jsonl, .stderr and
# .exit.json beside it. Those files are never replaced unless -Force is given. Nothing is written to the share.
# The last line is "RESULT ok" or "RESULT FAILED: <reason>"; after a failure the script throws.
param(
    [Parameter(Mandatory)][string]$Slug,
    [Parameter(Mandatory)][string]$Label,
    [switch]$Force
)
. C:\lazymkv\src\scripts\phase0\env.ps1

try {
    Assert-Slug $Slug
    Assert-Slug $Label
    if (-not (Test-Path -LiteralPath $CAP)) { throw "missing $CAP; build it with: scripts/box.sh cargo build --release -p lazymkv-capture" }
    if (-not $FFPROBE) { throw 'ffprobe.exe not found' }
    $rec = "$Recordings\$Slug\$Label"
    $dest = "$Staging\$Slug\$Label"
    $out = "$rec\ffprobe-audio-deep.json"
    $outFiles = @($out, "$out.timing.jsonl", "$out.stderr", "$out.exit.json")

    # Only a rip that rip-title.ps1 judged good is read.
    $ripFile = "$rec\rip-summary.json"
    if (-not (Test-Path -LiteralPath $ripFile)) { throw "no $ripFile; rip the title with rip-title.ps1 first" }
    $rip = Get-Content -LiteralPath $ripFile -Raw -Encoding UTF8 -ErrorAction Stop | ConvertFrom-Json
    if ($rip.ok -ne $true) { throw "the rip of label $Label did not succeed ($($rip.result)); only a rip whose rip-summary.json says ok is read" }

    # Evidence is never replaced without -Force.
    foreach ($file in $outFiles) {
        if ((Test-Path -LiteralPath $file) -and -not $Force) {
            throw "$file already exists and is kept as evidence; use -Force only where the plan says so"
        }
    }

    # The file is found by listing the folder: it must be the one .mkv that rip-title.ps1 recorded.
    $mkvs = @(Get-ChildItem -LiteralPath $dest -File -Force -ErrorAction Stop | Where-Object { $_.Extension -eq '.mkv' })
    if ($mkvs.Count -ne 1) { throw "expected one .mkv in $dest, found $($mkvs.Count)" }
    $mkv = $mkvs[0]
    if (@($rip.files) -notcontains $mkv.Name -or $mkv.Length -ne [int64]$rip.bytes) {
        throw "$($mkv.Name) ($($mkv.Length) bytes) is not the file rip-title.ps1 recorded ($(@($rip.files) -join ', '), $($rip.bytes) bytes)"
    }

    # Files exist here only under -Force: remove them, so that stale files cannot pass for this read's.
    foreach ($file in $outFiles) {
        if (Test-Path -LiteralPath $file) { Remove-Item -LiteralPath $file -Force -ErrorAction Stop }
    }
    "probe-audio: $($mkv.FullName) ($($mkv.Length) bytes)"
    & $CAP record --out $out -- $FFPROBE -v error -probesize '100M' -analyzeduration '100M' -select_streams a -show_streams -of json $mkv.FullName | Out-Null
    $code = $LASTEXITCODE
    if (-not (Test-Path -LiteralPath "$out.exit.json")) { throw "lazymkv-capture wrote no ffprobe-audio-deep.json.exit.json (exit $code)" }
    if ($code -ne 0) { throw "ffprobe exited $code; see ffprobe-audio-deep.json.stderr" }
    $data = Get-Content -LiteralPath $out -Raw -Encoding UTF8 -ErrorAction Stop | ConvertFrom-Json
    $streams = @($data.streams | Where-Object { $_ })
    if ($streams.Count -eq 0) { throw 'ffprobe lists no audio stream; see ffprobe-audio-deep.json' }
    foreach ($s in $streams) {
        "audio $($s.index): $($s.codec_name) profile=$($s.profile) channels=$($s.channels) layout=$($s.channel_layout)"
    }
    'RESULT ok'
} catch {
    "RESULT FAILED: $($_.Exception.Message)"
    throw
}
```

The script does not set `$ErrorActionPreference = 'Stop'`: inside a job, that would turn any line a native program writes to standard error into a failure. Each cmdlet that must not fail silently has `-ErrorAction Stop` instead.

- [ ] **Step 2: Check the prerequisites, the new script and Disc A's evidence**

Which disc is in the drive: does not matter (Disc A stays in). Nothing here reads the drive. Run this step only when no phase 0 job is running: `box.sh` replaces `C:\lazymkv\src`, from which jobs run.

```bash
grep -E '^(function (Assert-Slug|Assert-DiscIs|Assert-EmptyDir|Get-InsertedDisc)|\$Recordings)' scripts/phase0/env.ps1
grep -n -- '--subs-dir' scripts/phase0/subtitle_alignment.py
python3 -m unittest discover -s scripts/phase0 -p 'test_*.py'
git ls-files scripts/phase0/rip-title.ps1 scripts/phase0/probe-title.ps1
grep -c 'SkipRemux' scripts/phase0/probe-title.ps1
scripts/box.sh
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -Command \"(Get-Command C:\lazymkv\src\scripts\phase0\probe-audio.ps1).Parameters.Keys\""
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\probe-audio.ps1 -Slug SLUG_A -Label Bad_Label"; echo "exit=$?"
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\probe-audio.ps1 -Slug SLUG_A -Label zz-nolabel"; echo "exit=$?"
ssh USER@makemkv-box "if exist C:\lazymkv\phase0\recordings\SLUG_A\zz-nolabel (echo CREATED) else (echo absent)"
python3 - SLUG_A <<'EOF'
import json, pathlib, sys
d = pathlib.Path("corpus/owner") / sys.argv[1] / "all-tracks"
for name in ("rip-summary.json", "probe-summary.json"):
    s = json.loads((d / name).read_text(encoding="utf-8-sig"))
    print(name, "ok:", s.get("ok"), "title:", s.get("title"), "subtitle_tracks:", s.get("subtitle_tracks"))
print("mediainfo.json:", "present" if (d / "mediainfo.json").exists() else "absent")
EOF
```

Expected:
- The first `grep` prints five lines: the `$Recordings` assignment and the four D1 helper functions. The second prints the `--subs-dir` lines of the new script. Fewer lines mean the prerequisites are missing: stop and report.
- The unit tests end `Ran 34 tests` (or more) and `OK`. Any failure: stop and report.
- `git ls-files` lists both Task 11 scripts, and the `grep -c` prints a number above 0. Otherwise Task 11 is not complete: stop.
- `box.sh` syncs the tree, including the new probe-audio.ps1, and exits 0.
- The parameter list begins `Slug`, `Label`, `Force`; PowerShell's common parameters follow. A parse error is a defect in Step 1: fix it and run this step again.
- The `Bad_Label` run prints `RESULT FAILED:` with Assert-Slug's rejection of `Bad_Label` within seconds, then `exit=1`. If it says that `Assert-Slug` is not recognized, the env.ps1 prerequisites are missing.
- The `zz-nolabel` run prints `RESULT FAILED: no C:\lazymkv\phase0\recordings\SLUG_A\zz-nolabel\rip-summary.json; rip the title with rip-title.ps1 first`, then `exit=1`. Any other reason: stop and report.
- The `if exist` line prints `absent`.
- The Python check prints `rip-summary.json ok: True title: <N> ...` and `probe-summary.json ok: True ... subtitle_tracks: <n>`, then whether `mediainfo.json` is present. A `False`, or a missing file (a Python error), means Task 11 did not finish: stop. An absent `mediainfo.json` means MediaInfo was not installed for Task 11; then question 1 cannot fall back to MediaInfo for Disc A, which the report states.

- [ ] **Step 3: Read the audio streams again, deeper**

Which disc is in the drive: does not matter. The job reads only the ripped file on the share.

```bash
scripts/phase0/job.sh probe-audio.ps1 probe-audio-SLUG -Slug SLUG -Label all-tracks
scripts/phase0/wait.sh probe-audio-SLUG
```

job.sh prints `started probe-audio.ps1; log: C:\lazymkv\phase0\logs\probe-audio-SLUG.txt`. If it exits 1 because a phase 0 job is still running, wait for that job with its own wait.sh line first.

What wait.sh's exit codes mean for this job:
- **0**: check the log against the Expected below.
- **1**: read the `RESULT FAILED:` line.
  - `already exists and is kept as evidence`: the deep read of SLUG ran before. Its files are the evidence: go on with the pull below.
  - Assert-Slug, `no ...rip-summary.json`, `did not succeed`: the wrong SLUG, or the rip did not succeed. Check SLUG with `ls corpus/owner` and start again with log `probe-audio-SLUG-2`.
  - An error reading `\\unas-pro\media\lazymkv-staging\SLUG\all-tracks` (for example `Cannot find path` or `Access ... is denied`): run `scripts/phase0/job.sh setup-check.ps1 setup-check-audio-SLUG` and `scripts/phase0/wait.sh setup-check-audio-SLUG`. That wait.sh exits 0 when the check ran: if its log says `staging    : \\unas-pro\media\lazymkv-staging : write, rename and delete OK`, start this job again with log `probe-audio-SLUG-2`; if it says `FAILED`, ask the owner to check the NAS, then run the check again with log `setup-check-audio-SLUG-2`. Exit 1 (setup-check threw) or 4: stop and report with the log. Exit 3: run wait.sh again (the check takes under a minute). Exit 5: run wait.sh again once the mini PC can be reached.
  - `expected one .mkv ... found 0` with the share healthy, or `is not the file rip-title.ps1 recorded`: the ripped file is gone or was changed. Stop and report.
  - `ffprobe exited`, `wrote no ffprobe-audio-deep.json.exit.json` or `lists no audio stream`: pull the recording (`scripts/phase0/pull-recording.sh SLUG`), read `corpus/owner/SLUG/all-tracks/ffprobe-audio-deep.json.stderr`, and stop and report. A repeat after the fix adds `-Force` (see "When -Force is right").
- **3**: still running. A deep read normally takes under two minutes; run wait.sh again. If 15 minutes pass without a DONE line, end ffprobe (not the job) with `ssh USER@makemkv-box "taskkill /im ffprobe.exe /f"`, run wait.sh again (it then exits 1), and stop and report.
- **4**: the job ended without a DONE line: stop and report.
- **5**: the mini PC cannot be reached. Run wait.sh again once it can.

Expected (exit 0), in the log:
- `probe-audio: \\unas-pro\media\lazymkv-staging\SLUG\all-tracks\<name>.mkv (<bytes> bytes)`;
- one `audio <index>: <codec> profile=<profile or empty> channels=<n> layout=<layout>` line per audio stream;
- `RESULT ok`, then `PHASE0-DONE status=ok exit=<n>`. `exit=<n>` is only the last native command's exit code, not a verdict.

Then copy the result to the Mac:

```bash
scripts/phase0/pull-recording.sh SLUG
```

Expected: `pulled corpus/owner/SLUG (...)`. If it fails, nothing on the Mac changed; run it again.

- [ ] **Step 4: List the audio profiles and ask the owner which tracks are Atmos or DTS:X**

Which disc is in the drive: does not matter (this runs on the Mac).

```bash
python3 - SLUG <<'EOF'
import json, pathlib, sys
d = pathlib.Path("corpus/owner") / sys.argv[1] / "all-tracks"
if not (d / "probe-summary.json").exists():
    sys.exit(f"no {d}/probe-summary.json: a wrong slug, or the recording was not pulled after probe-title.ps1")
for name in ("ffprobe-streams.json", "ffprobe-audio-deep.json", "mediainfo.json"):
    path = d / name
    if not path.exists():
        print(f"{name}: absent ({path})")
        continue
    data = json.loads(path.read_text(encoding="utf-8-sig", errors="replace"))
    if name.startswith("ffprobe"):
        for s in data.get("streams", []):
            if s.get("codec_type") == "audio":
                tags = s.get("tags", {})
                print(f"{name[:-5]:<19} index={s.get('index')} {s.get('codec_name')} profile={s.get('profile')!r} "
                      f"channels={s.get('channels')} layout={s.get('channel_layout')} lang={tags.get('language')} title={tags.get('title')!r}")
    else:
        for t in data.get("media", {}).get("track", []):
            if t.get("@type") == "Audio":
                print(f"{'mediainfo':<19} ID={t.get('ID')} {t.get('Format')} commercial={t.get('Format_Commercial_IfAny')!r} "
                      f"features={t.get('Format_AdditionalFeatures')!r} channels={t.get('Channels')}")
EOF
```

Expected:
- For each audio stream, one `ffprobe-streams` line (probe-title.ps1's read), one `ffprobe-audio-deep` line (Step 3's read) and one `mediainfo` line. MediaInfo's `ID` is the Matroska track number, normally ffprobe's `index` plus one. A file listed as `absent` with its path is missing from the recording; a deep read that is absent means Step 3 was not done for SLUG.
- An exit with `no corpus/owner/SLUG/all-tracks/probe-summary.json` means SLUG is mistyped or the recording was not pulled: check `ls corpus/owner`.

Ask the owner which audio tracks the disc's packaging or menu names as Dolby Atmos or DTS:X, and match each to a stream by language, codec and channel count. Note for each stream what ffprobe's two reads and MediaInfo say. For example, ffprobe may report a profile such as `Dolby TrueHD + Dolby Atmos` or `DTS-HD MA + DTS:X`, and MediaInfo a `commercial` name or a `features` value. Record the exact strings, whatever they are. Also note:
- any stream whose profile names Atmos or DTS:X although the owner does not name it;
- any stream where the deep read gives a different profile from the first read.

Then append the owner's answers to `corpus/owner/SLUG/notes.md` (create the file if it is missing; never replace what is in it), in this form:

```markdown
## Experiment 6 (Task 12)

Audio streams the owner names as Atmos or DTS:X (source: packaging or menu), by ffprobe index:
- index <i>: <language> <codec> <channels> ch: <Atmos | DTS:X>
```

Write `- none named by the owner` when the owner names no such track.

- [ ] **Step 5: Measure subtitle alignment and ask the owner what each English subtitle stream is**

Which disc is in the drive: does not matter (this runs on the Mac).

First list the title's subtitle streams as the scan describes them. The title number comes from `all-tracks/rip-summary.json`; Step 6 runs the same command before a rip, with the title number as a second argument.

```bash
cat corpus/owner/SLUG/all-tracks/subs/index.json
python3 - SLUG <<'EOF'
import json, pathlib, sys
sys.path.insert(0, "scripts/phase0")
from robot import parse
d = pathlib.Path("corpus/owner") / sys.argv[1]
if len(sys.argv) > 2:
    title = int(sys.argv[2])
else:
    title = json.loads((d / "all-tracks" / "rip-summary.json").read_text(encoding="utf-8-sig"))["title"]
scan = parse((d / "scan.txt").read_text(encoding="utf-8-sig", errors="replace"))
print(f"{d.name} title {title}: subtitle streams in scan order; s<k> in subtitle_alignment.py is line s<k>")
k = counted = 0
for (t, s), a in sorted(scan.streams.items()):
    if t != title or a.get(1) != "Subtitles":
        continue
    flags = int(a.get(22, "0") or 0)
    lang = a.get(3) or "und"
    derived_forced = bool(flags & 4096 and flags & 2048)
    counted += lang in ("eng", "und") and not derived_forced
    note = "  derived forced-only: set aside, not counted" if derived_forced else ""
    print(f"s{k:<3} stream {s:>2}  lang={lang:<4} codec={a.get(6, ''):<10} flags={flags:<5} conversion={a.get(42, '')}{note}")
    k += 1
print("English or no-language subtitle streams, derived forced-only streams not counted:", counted)
probe = d / "all-tracks" / "probe-summary.json"
if probe.exists():
    in_file = json.loads(probe.read_text(encoding="utf-8-sig")).get("subtitle_tracks")
    print(f"subtitle streams in the scan: {k}; subtitle tracks in the ripped file (probe-summary.json): {in_file}")
EOF
python3 scripts/phase0/subtitle_alignment.py --subs-dir corpus/owner/SLUG/all-tracks/subs --clear-max-bytes 64
```

Expected:
- `index.json`: a JSON list with one entry (`n`, `track_id`, `language`, `codec`, `track_name`, `forced`, `default`) per subtitle track of every language. `No such file` means the title has no subtitle track (`subtitle_tracks` 0 in `probe-summary.json`): the disc then counts for question 1 only, and the alignment command below exits 2 with `no .../index.json`.
- The listing: one line per subtitle stream of the title, in scan order, with its language (attribute 3), codec (6), flags (22) and conversion (42). Then the count of English or no-language streams that are not derived forced-only streams (flags 4096 and 2048 together, spec 8.7). Then the two totals, which must be equal: s<k> of the alignment output is then line s<k> of the listing. If they differ, the rip did not keep every subtitle stream in scan order; match the streams by language and position, and say so in the report.
- The alignment output:
  - one line per English or no-language stream, named `s<k>-<language>-<image|text>`, with `-forced` when the file flags the track forced: its codec, track name, packet count and the packet-size buckets (packets of 32, 64, 128 and 256 bytes or less);
  - the event count per stream;
  - the matrix, under a line that says image streams ignore packets of 64 bytes or less and text streams count every packet;
  - one line per pair with the larger of the two alignment values.

  Exit 2 with `lists no English or no-language subtitle stream` means the title has no English subtitles: the disc counts for question 1 only.
- On a DVD, the closed-caption stream is the `-text` stream; its listing line shows a conversion other than `( Lossless conversion )` (spec 8.7). Note its codec, conversion text and language for the report.

Ask the owner what each English or no-language stream is: regular, SDH, commentary, trivia, forced-only, closed captions, or unknown. The owner may know from the packaging, the disc's menu or the track names, or may play the ripped file from `\\unas-pro\media\lazymkv-staging\SLUG\all-tracks\` in a player that lists the subtitle tracks. s<k> is the subtitle track at position k counting from 0 over all languages; a player's list usually counts from 1. Append the answers under the Step 4 section of `corpus/owner/SLUG/notes.md`:

```markdown
English and no-language subtitle streams of title <title>, as the owner identifies them:
- s<k>: <regular | SDH | commentary | trivia | forced-only | closed captions | unknown> (<how the owner knows>)
```

- [ ] **Step 6: Add a disc: check it, record it and choose its title**

Which disc is in the drive: the disc NEW, which the owner inserts in place of the previous one. The first time through, Disc A comes out here.

The first time through, ask the owner, after Steps 3 to 5 for Disc A, for these discs (one disc may serve several kinds):
- (a) a disc with a commentary subtitle track, preferably a Blu-ray. This is needed even if Disc A has one: the spec asks for at least three discs.
- (b) a DVD with closed captions, preferably one that also has two or more English subtitle tracks.
- (c) only if Disc A has no owner-confirmed Atmos stream or no owner-confirmed DTS:X stream: a disc with the missing format.
- (d) only if Disc A's listing in Step 5 counted fewer than two English subtitle streams: one more disc with two or more English subtitle tracks, for example English and English SDH.

Steps 6 to 8 run once for each of these discs in turn, and again for any disc that Step 9 or Step 11 asks for. Write down each NEW with the kind or kinds it was asked for.

Ask the owner to insert the disc and to say when the drive's light has stopped blinking. Then read its fingerprint and look it up among the recordings:

```bash
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -Command \". C:\lazymkv\src\scripts\phase0\env.ps1; (Get-InsertedDisc).fingerprint\""; echo "exit=$?"
python3 - FINGERPRINT <<'EOF'
import json, pathlib, sys
fp = sys.argv[1].strip().upper()
owner = pathlib.Path("corpus/owner")
hits = [p.parent.name for p in sorted(owner.glob("*/meta.json"))
        if (json.loads(p.read_text(encoding="utf-8-sig")).get("fingerprint") or "").upper() == fp]
if not hits:
    print("not recorded yet: a new disc")
for slug in hits:
    print(f"already recorded as {slug}; scan.txt {'present' if (owner / slug / 'scan.txt').exists() else 'absent'}; "
          f"all-tracks rip {'present' if (owner / slug / 'all-tracks').exists() else 'absent'}")
EOF
```

Expected: the ssh command's last line is the disc's fingerprint, 32 hexadecimal characters (FINGERPRINT), and `exit=0`. An error saying that no disc is mounted means the disc is not ready: wait half a minute and run it again. Any other error: stop and report. The Python check then prints one of:
- `not recorded yet: a new disc`: choose NEW (see Placeholders) and record the disc below.
- `already recorded as <S>; ... all-tracks rip present`: this disc was measured already (it is Disc A, or a disc from an earlier pass of this step). Ask the owner for another disc.
- `already recorded as <S>; scan.txt present; all-tracks rip absent`: Task 8 recorded and scanned it. NEW is S. Skip the recording job and go on with the pull below.
- `already recorded as <S>; scan.txt absent; ...`: Task 8 recorded it without a scan. NEW is S. Record it below; the job keeps its snapshot and adds the scan.

Record the disc (snapshot, lookups and a full scan) as a job:

```bash
scripts/phase0/job.sh record-disc.ps1 record-NEW -Slug NEW
scripts/phase0/wait.sh record-NEW
```

What wait.sh's exit codes mean for this job:
- **0**: check the log against the Expected below.
- **1**: read the `RESULT FAILED:` line.
  - Assert-Slug's rejection: NEW is malformed. Choose a valid slug and start again (the log name follows the new slug).
  - `no readable disc in D: after 120 s`: nothing was written. Ask the owner whether the disc is in and the light has stopped, then start again with log `record-NEW-2`.
  - `is not the disc of recording NEW` (Assert-DiscIs): NEW already belongs to another disc. Choose a new slug.
  - `already exists and is kept as evidence`: the recording already has a scan, which the check above should have shown. Stop and report.
  - `lazymkv-capture disc failed`, `the disc has no fingerprint`, `did not finish`, `MakeMKV did not run` or `the scan lists no titles`: stop and report with the log.
- **3**: still running. This is normal: a DVD scan takes a minute or two, a Blu-ray a few minutes, a UHD with Java menus ten or more. Run `scripts/phase0/wait.sh record-NEW` again, and start nothing else meanwhile. If 45 minutes pass without a DONE line, end the job with `ssh USER@makemkv-box "powershell -NoProfile -Command Stop-ScheduledTask -TaskName lazymkv-phase0"`, check that `ssh USER@makemkv-box "tasklist | findstr /i makemkvcon"` prints nothing, and stop and report.
- **4**: the job ended without a DONE line: stop and report.
- **5**: the mini PC cannot be reached. Run wait.sh again once it can.

Expected (exit 0), in the log:
- for a new slug: `disc: label=... format=<bluray|dvd> fingerprint=FINGERPRINT disc_id=... files=...` and `lookups: ...`;
- for a slug Task 8 recorded without a scan: `disc: kept from the earlier recording of this same disc: ...` and `lookups: kept: ...`;
- in both cases, then `scan: scan.txt exit.json={...} version_banner=True titles=<n above 0>`, `RESULT ok`, and `PHASE0-DONE status=ok exit=<n>`.

Copy the recording to the Mac and choose the title:

```bash
scripts/phase0/pull-recording.sh NEW
python3 scripts/phase0/summarize_scan.py corpus/owner/NEW
```

Expected: `pulled corpus/owner/NEW (...)`, then the summary, starting `MakeMKV: MakeMKV v2.0.0 win(x64-release)`, with its title table. Choose M, the main feature:
- on a disc whose titles carry labels (the comment column), the title whose comment starts `FPL_MainFeature`;
- otherwise the feature-length title; if more than one remains, the longest. On a DVD this is the longest title.

List M's subtitle streams: run Step 5's listing command (the `python3 - SLUG <<'EOF'` block) with `NEW M` in place of `SLUG`, so that its first line is `python3 - NEW M <<'EOF'`.

Expected: the listing and its count of English or no-language subtitle streams. There is no `subtitle tracks in the ripped file` line yet. For a disc asked for as (a), (b) or (d), the count must be 2 or more: the closed-caption stream counts, and derived forced-only streams do not. With fewer, tell the owner what the scan shows and ask for another disc of that kind. This disc's recording stays in `corpus/owner` (Task 15 counts it with the others). Do not rip it unless it was also asked for as (c). A disc asked for only as (c) needs no subtitle count.

- [ ] **Step 7: Rip and probe the new disc's title**

Which disc is in the drive: the disc NEW. rip-title.ps1 checks this itself with Assert-DiscIs before MakeMKV starts. Once the rip has ended `RESULT ok`, the disc is no longer needed; the probe reads only the share.

```bash
scripts/phase0/job.sh rip-title.ps1 rip-NEW-all-tracks -Slug NEW -Title M -Label all-tracks
scripts/phase0/wait.sh rip-NEW-all-tracks
```

MakeMKV scans the disc again before it writes. A DVD feature then takes 10 to 30 minutes, a Blu-ray 30 to 60, a UHD about an hour or more. To see progress at any time:

```bash
ssh USER@makemkv-box "powershell -NoProfile -Command \"Get-Date -Format 'yyyy-MM-dd HH:mm:ss.fff'; Get-Content C:\lazymkv\phase0\recordings\NEW\all-tracks\rip.txt -Tail 2\""
```

What wait.sh's exit codes mean for this job:
- **0**: check the log against the Expected below.
- **1**: read the `RESULT FAILED:` line.
  - A reason from the checks before MakeMKV starts means nothing was ripped. These are: Assert-Slug, `profile not found`, `scan.txt lists no title`, `MakeMKV is still running`, `is not empty`, Assert-DiscIs (`is not the disc of recording NEW`), or an error creating a folder. Fix the cause, then start the same job again with log `rip-NEW-all-tracks-2`:
    - `MakeMKV is still running`: end it with `ssh USER@makemkv-box "taskkill /im makemkvcon64.exe /f"`, or ask the owner to close the MakeMKV window;
    - Assert-DiscIs: ask the owner to insert the disc NEW;
    - a folder on the share: run `scripts/phase0/job.sh setup-check.ps1 setup-check-NEW` and `scripts/phase0/wait.sh setup-check-NEW`. That wait.sh exits 0 when the check ran: its log's staging line must say `write, rename and delete OK`; if it says `FAILED`, ask the owner to check the NAS and run the check again with log `setup-check-NEW-2`. Exit 1 or 4: stop and report with the log. Exit 3: run wait.sh again. Exit 5: run wait.sh again once the mini PC can be reached;
    - `is not empty`: an earlier attempt exists; see "When -Force is right".
  - Any other reason: the rip ran and failed. Pull the recording (`scripts/phase0/pull-recording.sh NEW`), read `grep -E '^MSG:' corpus/owner/NEW/all-tracks/rip.txt | cut -c1-160`, and stop and report with those lines and the `read_errors` and `write_errors` counts. A repeat (for example after cleaning the disc, with `-Force` as "When -Force is right" says), or another disc of the same kind, is decided with the owner.
- **3**: still running; this is normal for a long time. Run `scripts/phase0/wait.sh rip-NEW-all-tracks` again, and start nothing else meanwhile. Run the progress command now and then. MakeMKV is stuck if two progress checks 15 minutes apart show the same last line, or if three hours pass without a DONE line. Then end MakeMKV, not the job: `ssh USER@makemkv-box "taskkill /im makemkvcon64.exe /f"`. The script then writes its summary and fails, and wait.sh exits 1. Stop and report.
- **4**: the job ended without a DONE line. Check `ssh USER@makemkv-box "tasklist | findstr /i makemkvcon"`, then stop and report.
- **5**: the mini PC cannot be reached. Run wait.sh again once it can.

Expected (exit 0):
- The log shows `rip: NEW title M (scan source file: <file name or empty>) to \\unas-pro\media\lazymkv-staging\NEW\all-tracks with --cache=1024, started <time>`, the summary JSON, `RESULT ok` and `PHASE0-DONE status=ok exit=<n>`.
- In the JSON:
  - `ok` is true;
  - `files` is a list holding one `.mkv` name, and `bytes` is above 0;
  - `fail_codes` is empty;
  - `rip_source_file` equals `scan_source_file`; both are null on a DVD, whose scan has no attribute 16.
- `RESULT ok` with a `MSG:1009` note means MakeMKV rejected the job profile and used its default, which may leave streams out. Continue, and mention it in the report.

Then probe the file without the remux, which this experiment does not need:

```bash
scripts/phase0/job.sh probe-title.ps1 probe-NEW-all-tracks -Slug NEW -Label all-tracks -SkipRemux
scripts/phase0/wait.sh probe-NEW-all-tracks
```

What wait.sh's exit codes mean for this job:
- **0**: check the log against the Expected below.
- **1**: read the `RESULT FAILED:` line.
  - `did not succeed`: the rip above did not end `RESULT ok`.
  - `already holds probe evidence`: a probe of NEW ran before; see "When -Force is right".
  - Any other reason (mkvmerge, ffprobe or MediaInfo, or the extraction): pull the recording, read the file the reason names (for example `corpus/owner/NEW/all-tracks/evidence-mkvmerge.txt`, or the `.stderr` beside it), and stop and report. A repeat after the fix uses the same command with `-Force` added and log `probe-NEW-all-tracks-2`.
- **3**: still running. This is normal: the subtitle extraction reads the whole file over the network once, which takes a few minutes for a DVD and up to 40 minutes for a UHD. Run wait.sh again. If two hours pass, end mkvmerge with `ssh USER@makemkv-box "taskkill /im mkvmerge.exe /f"`, run wait.sh again (exit 1), and stop and report.
- **4**: stop and report.
- **5**: run wait.sh again once the mini PC can be reached.

Expected (exit 0), in the log:
- `probe: \\unas-pro\media\lazymkv-staging\NEW\all-tracks\<name>.mkv (<bytes> bytes)`;
- the summary JSON with `ok` true, `remux_skipped` true, `subtitle_tracks` normally equal to the number of stream lines in Step 6's listing for M (a difference is not a failure here; Step 5 shows it again and the report notes it), and `evidence_bytes` above 0 when there are subtitles;
- `RESULT ok`, then `PHASE0-DONE status=ok exit=<n>`.

Copy the recording to the Mac:

```bash
scripts/phase0/pull-recording.sh NEW
```

Expected: `pulled corpus/owner/NEW (...)`.

- [ ] **Step 8: Measure the new disc**

Which disc is in the drive: does not matter. Step 3's job reads only the share; Steps 4 and 5 run on the Mac. The owner may insert the next disc for Step 6 meanwhile.

Run Steps 3, 4 and 5 with SLUG = NEW: the deep audio read (log `probe-audio-NEW`), the pull, the audio listing with the owner's Atmos and DTS:X answers, and the subtitle listing and alignment with the owner's classification. Both go into `corpus/owner/NEW/notes.md`. For a DVD, also note how its closed-caption stream appears: its codec in the scan and in the file, its conversion text and its language.

Then go back to Step 6 for the next disc on the list. When every disc asked for has been measured, or the owner has none left of a kind, go on to Step 9.

- [ ] **Step 9: Coverage check**

Which disc is in the drive: does not matter.

Use the rules below for every measured disc: SLUG_A, and each NEW that was ripped. Take the numbers from each disc's Step 5 output and the classifications from its `notes.md`.
1. **Tracks taking part.** These are the streams that subtitle_alignment.py lists, except:
   - derived forced-only streams (the listing says `derived forced-only`);
   - the closed-caption stream (on a DVD, the `-text` stream whose conversion is not `( Lossless conversion )`).

   Spec 8.7 sets both aside before counting, alignment and grouping.
2. **Full.** A track taking part is full when its event count is at least 10% of the largest event count among the disc's tracks taking part.
3. **Decisive pairs.** Each pair of full tracks taking part is one decisive pair. Its value is the pair line's number. Its label comes from the owner's classification:
   - **same-content**: regular and SDH, or two dialogue tracks;
   - **different-content**: a dialogue track and a commentary or trivia track;
   - **unknown**: the owner could not say for one of the two.

   A DVD stream flagged director's comments (scan flags 1 or 2) stays in and is marked `flagged`. The design drops such a stream by its flag, but it is a known different-content track, so it is good evidence.
4. **Closed-caption pairs.** The closed-caption stream against each full track taking part. Its value comes from the same pair line. It is labelled same-content when the other track is regular or SDH, and different-content when it is commentary or trivia.
5. **A qualifying disc** is one whose listing count (English or no-language subtitle streams, derived forced-only streams not counted, the closed-caption stream counted) is 2 or more.

Fill in this table, which goes at the top of the report:

```markdown
## Coverage

| Disc | Format | Title | English subtitle streams counted | Qualifies | Decisive pairs: same / different / unknown | Closed-caption pairs | Owner-confirmed Atmos streams | Owner-confirmed DTS:X streams |
|---|---|---|---|---|---|---|---|---|
| <slug> | <UHD, Blu-ray or DVD> | <n> | <count> | <yes or no> | <s> / <d> / <u> | <n or none> | <n> | <n> |

- Qualifying discs: <n> (3 or more needed).
- Decisive same-content pairs: <n>; decisive different-content pairs: <n> (1 or more of each needed; closed-caption pairs do not count).
- Closed-caption stream measured: <slug>, or "no DVD with closed captions was available".
- Atmos: <n> owner-confirmed streams; DTS:X: <n> owner-confirmed streams.
```

Each of these gaps is asked for once:
- fewer than three qualifying discs: ask for a disc of kind (d);
- no decisive same-content pair: ask for a disc with English and English SDH subtitles;
- no decisive different-content pair: ask for a disc with a commentary subtitle track;
- no owner-confirmed Atmos stream, or no owner-confirmed DTS:X stream: ask for a disc of kind (c).

If the owner has such a disc, run Steps 6 to 8 for it and then this step again. If not, write the gap under the table as "not available: <what>". Then:
- with fewer than three qualifying discs, or no decisive same-content pair, or no decisive different-content pair, question 2 is "not answered";
- a format with no owner-confirmed stream is "not tested" for question 1.

- [ ] **Step 10: Choose `sub_clear_max_bytes` and measure again at it**

Which disc is in the drive: does not matter.

Choose CLEAR from the packet-size buckets of the image streams only (the `-image` lines of every measured disc's Step 5 output). Text streams always count every packet as an event (spec 8.7), so their buckets play no part.
- On a PGS stream, roughly half of all packets are small "clear" packets.
- CLEAR is the largest bucket limit below which the packets are plainly clear packets. For example, counts of `[800, 800, 800, 812]` on a stream of 1600 packets mean that every packet of 128 bytes or less is a clear packet, and that some content packets lie between 129 and 256 bytes. That stream's value is therefore 128.
- Take the value that holds on every PGS stream, which is the smallest of the per-stream values.
- CLEAR is 0 if the image streams have no such small packets.
- VobSub streams usually have no separate clear packets. Report their buckets as observed. If a VobSub stream has packets at or below CLEAR, say so in the report: the design would ignore them.

If CLEAR is 64, the Step 5 outputs are already final: skip the rest of this step. Otherwise run the alignment again on every measured disc with the chosen value:

```bash
python3 scripts/phase0/subtitle_alignment.py --subs-dir corpus/owner/SLUG/all-tracks/subs --clear-max-bytes CLEAR
```

Expected: the same stream lines and buckets as in Step 5. The matrix header now names CLEAR. Only image streams' event counts and alignment values may differ from the 64 run.

From here on, the full test, the pair table and the thresholds use this final run only (the 64 run when CLEAR is 64). The report shows both runs' matrices.

- [ ] **Step 11: Pair table and thresholds**

Which disc is in the drive: does not matter.

Apply Step 9's rules 1 to 4 again to the final run of Step 10. When CLEAR is not 64, the full test and the decisive pairs can differ from Step 9's, which was measured at 64: fill in Step 9's Coverage table again from the final run (its qualifying-disc, decisive-pair, closed-caption-pair and "not available" lines), and use that table, not the first one, for "Not answered" below and in the report. If it shows a gap that the first table did not (no decisive same-content pair, or no decisive different-content pair), ask once for the disc as Step 9's list says. If the owner has one, run Steps 6 to 8 for it, then Steps 9 and 10 again (Step 9 does not ask again for a gap it has already asked for; Step 10 chooses CLEAR again with the new disc's image streams included and runs the alignment on every measured disc), and start this step again. If not, write the gap under the new table as "not available: <what>". Then build the pair table:

```markdown
| Disc | Track A | Track B | Larger alignment value | Label | Note |
|---|---|---|---|---|---|
| <slug> | s<k> (<owner's classification>) | s<k> (<owner's classification>) | <0.00> | <same-content, different-content or unknown> | <flagged, closed-caption pair, or empty> |
```

Decisive pairs come first, then the closed-caption pairs. Unknown pairs are listed but count for nothing.

Judge the decisive pairs labelled same-content or different-content, without the closed-caption pairs. Spec 8.7 sets the closed-caption stream aside before alignment, so no closed-caption pair can raise `subtitle_ambiguous`. Report the same judgement a second time with the closed-caption pairs added, for information only.
- **Provisional values confirmed**: every same-content pair is at 0.70 or more and every different-content pair at 0.30 or less. Then `subs_same_min` stays 0.70 and `subs_diff_max` 0.30, and the rounding rule below is not applied.
- **Separated at other values**: otherwise, let L be the lowest same-content value rounded down to a multiple of 0.05, and H the highest different-content value rounded up to a multiple of 0.05 (for example 0.83 becomes 0.80, and 0.12 becomes 0.15). If L is at least H + 0.20, then `subs_same_min` is L and `subs_diff_max` is H.
- **No clean separation**: L is less than H + 0.20. Then the fallback of spec 17 row 6 applies: any title with two or more full English tracks raises `subtitle_ambiguous`.
- **Not answered**: the final run's Coverage table (Step 9's own table when CLEAR is 64) shows fewer than three qualifying discs, or no decisive same-content pair, or no decisive different-content pair. Then the provisional 0.70 and 0.30 stay, marked unconfirmed.

`sub_clear_max_bytes` is CLEAR from Step 10.

- [ ] **Step 12: Write the report**

Which disc is in the drive: does not matter.

Create `docs/phase0/experiments/EXP-06-audio-profiles-and-subtitles.md` from `docs/phase0/experiments/TEMPLATE.md`. Header:
- the date;
- every measured disc with slug, format and drive D:;
- the MakeMKV version from `summarize_scan.py`;
- `section 17, row 6`.

Put the `## Coverage` section from Step 9 (the one made again in Step 11 when CLEAR is not 64), with its "not available" lines, right after the header, above `## Question`. The report must state:

- Under "What was run":
  - for each disc, the recording (and whether Task 8 had made it), the title ripped with `-Label all-tracks`, the probe (`-SkipRemux` for the discs added here; Disc A's comes from Task 11), and the deep audio read (`probe-audio.ps1`);
  - the alignment runs: at 64, and at CLEAR when CLEAR is not 64;
  - each disc's kind or kinds from Step 6.
- Under "What was observed":
  - The audio lines of Step 4 for every disc, beside the owner's Atmos and DTS:X answers:
    - the exact `ffprobe` profile string of each owner-confirmed Atmos or DTS:X stream, from the first read and from the deep read;
    - the MediaInfo `commercial` and `features` strings for the same streams;
    - any stream whose profile names Atmos or DTS:X although the owner did not name it;
    - whether the deep read changed any profile.
  - For each disc:
    - Step 5's listing (scan streams with flags and conversion, the count, and the two totals);
    - the stream lines with buckets;
    - the event counts;
    - the matrix: the 64 run, and also the CLEAR run when CLEAR is not 64.
  - The owner's classification of every English stream, with how the owner knew.
  - Whether MakeMKV wrote a `track_name` on any subtitle track, as `index.json` shows, and whether any name has non-ASCII characters.
  - The closed-caption stream on the DVD: its codec in the scan and in the file, its conversion text and its language. Or "no DVD with closed captions was available".
  - The pair table from Step 11, and both judgements: without and with the closed-caption pairs.
  - The chosen `sub_clear_max_bytes` and the buckets it was read from, with any VobSub stream that has packets at or below it.
- Under "Answer", the two questions separately:
  1. Does `ffprobe` tell Atmos and DTS:X apart? Answer for Atmos and for DTS:X separately:
     - yes, with the profile string;
     - no, with what ffprobe gives even after the deep read, and then whether MediaInfo tells it (yes or no, with its string);
     - not tested: no owner-confirmed stream.

     Question 1 is yes only if both formats are yes.
  2. Do the alignment values separate dialogue from commentary? One of:
     - "yes, the provisional values are confirmed";
     - "yes, at other values" with `subs_same_min` and `subs_diff_max`;
     - "no clean separation";
     - "not answered" with the missing disc or pair.
- Under "Consequence for the spec":
  - Question 1:
    - yes keeps `ffprobe` as the detector named in spec 6.4 step 9, and records the profile strings for spec 8.6's object-audio criterion;
    - a no where MediaInfo tells the format makes MediaInfo the detector for that format (spec 6.4 step 9 and section 4.4);
    - a no from both tools means 8.6's object-audio criterion is skipped for that format, and ties go on to its later criteria (spec 17 row 6);
    - not tested changes nothing, and the format is marked untested for Task 17.
  - Question 2:
    - the three settings in spec section 14: `subs_same_min`, `subs_diff_max` and `sub_clear_max_bytes`, as measured;
    - on no clean separation, the fallback of spec 17 row 6: any title with two or more full English tracks raises `subtitle_ambiguous`;
    - on not answered, the provisional values stay, marked unconfirmed.
  - The closed-caption findings for spec 8.7: how the stream appears, and whether it aligns with the dialogue tracks.

**Experiment 13 (added 2026-10-05).** The owner asked for a content-based way to tell commentary and audio description from the programme's own audio. Steps 13 to 21 measure it on every extra audio track in a wanted language (English and the original language) of the titles this task rips, against the title's programme reference (EXP-13-design.md section 9.1). The first two steps and the Office disc's two rips were done on 2026-10-05, when the design was made; their results are in `corpus/owner/the-office-2005-s3d3-bluray/`.

These steps go after Step 12 of Task 12. The old Step 13 (Commit) is replaced by Step 32 below (experiment 14's Steps 22 to 31 come before it). In them, "the design" is `docs/phase0/experiments/EXP-13-design.md` (this file, committed there by the controller before Task 12 runs). Task 12's header gains:

- **Files:**
  - Prerequisite, committed by the controller before this task: `docs/phase0/experiments/EXP-13-design.md` (the design).
  - Already applied by the controller, not changed by this task: `scripts/phase0/audio-levels.ps1`, `scripts/phase0/audio_similarity.py` and `scripts/phase0/test_audio_similarity.py` (EXP-13 commit 731247d, then the D16 prerequisite, which also added `audio_dataset.py`, `audio_labels.py`, `proxy_levels.py`, `test_audio_dataset.py` and `tuning-fetch.ps1`; then the EXP-13 addendum's `final/` (EXP-13-design.md section 9: the five Python scripts and the comment-only `audio-levels.ps1`), applied in one commit that also recorded Pan's Labyrinth's commentary as fitted (Step 13)). EXP-13-design.md section 6 holds the pre-D16 versions: never copy them over the scripts.
  - Create: `docs/phase0/experiments/EXP-13-commentary-by-audio.md`.
  - Create (pulled from the mini PC): `corpus/owner/<slug>/<label>/audio-levels-20ms-<M>-<C...>/`. Its level files are ignored by git; its `streams.json` is committed.
  - Create (on the Mac): `corpus/owner/<slug>/<label>/audio-content-20ms-<M>-<C...>.txt`.
  - Append to (on the Mac only): a `## Experiment 13 (Task 12)` section in `corpus/owner/<slug>/notes.md` for every disc measured.
  - Create or modify: `corpus/owner/<slug>/audio-labels.json` for every disc measured, and `corpus/owner/audio-dataset.json`.
- **Produces:** `audio-levels.ps1 -Slug <slug> -Label <label> -Main <stream> [-Cand <stream>[,<stream>...]] [-Force]`, run as a job. Its log lines are possibly `dataset: created ...` and `dataset: removed ...`, then `levels: <file> main=<M> cand=<C,... or nothing> centre=<True|False> ffmpeg_exit=<n> seconds=<s> folder=<folder>`, one `stream <n>: <codec> <channels> ch <layout> <language> frames=<n> md5=<hash>[ (layout guessed by ffmpeg: <name>)]` line per measured stream, `proxy: <path> tracks=<k> bytes=<b> (<stream> <mix|fc|side>, ...)` or `proxy: kept <path> ...`, `dataset: <path>`, then `RESULT ok` or `RESULT FAILED: <reason>`. After a failure the script throws, so wait.sh exits 1. `audio_dataset.py` (the harness, `args`) and `audio_labels.py` as in the Global Constraints.

Placeholders for Steps 15 to 18:
- **TITLES** is fixed when Step 15 starts, and written at the top of this task's report: every (SLUG, LABEL) pair that `python3 scripts/phase0/audio_dataset.py | grep '^Titles whose audio'` lists as missing at that moment, plus the pairs of the old list below that are not in it (already in the dataset, for example the Office titles after the D16 backfill; for them Step 16 says `already exists`, and Steps 17 and 18 still run), plus each pair ripped in Step 19 as it is ripped. A title with no other audio stream in a wanted language (English, the original language) or with no language code is measured with its reference alone (no `-Cand`), for the dataset only; `audio_dataset.py args` prints the job. The old list: `the-office-2005-s3d3-bluray` with `e16-stereo`, `rehearsal` and `t3-all-tracks`; SLUG_A with `all-tracks` (Task 11); each NEW with `all-tracks` from Steps 6 to 8.
- **M** is the reference stream number and **C** the candidate list, both chosen in Step 15. **CS** is C with hyphens in place of commas.
- **LOG** is `audio-levels-SLUG-LABEL`, with `-M` appended when `audio_dataset.py args` prints more than one job for the label (it then does so itself: the programme reference's job and a dataset-only job per language that is not wanted), and `-2`, `-3` and so on appended on a repeat.

When `-Force` is right for `audio-levels.ps1`: only to repeat a measurement that failed after its folders were created: a `RESULT FAILED:` reason beginning `ffprobe failed`, `ffmpeg failed`, `streamhash.txt holds`, `ffmpeg wrote no raw-`, `<series>.txt holds no level`, `proxy check failed` or `the copy of ... in the dataset differs`, once the cause has been dealt with, and a refusal saying `is from a run that did not finish ...: repeat with -Force`. A refusal before that (among them `ffmpeg -layouts printed no table of channel layouts`, `cannot reach the staging share` and `expected one .mkv`) wrote nothing and is repeated without `-Force`. Never over a folder whose run ended `RESULT ok` (it says `already exists and is kept as evidence`), except in the D16 prerequisite's backfill.

- [ ] **Step 13: Write and test the measurement scripts**

Which disc is in the drive: does not matter (this runs on the Mac).

The scripts are already in place: the controller applied EXP-13-design.md section 6 (commit 731247d) and then the D16 prerequisite, which changed `audio-levels.ps1` and `audio_similarity.py` again. Then the EXP-13 addendum (EXP-13-design.md section 9, commit b48ad9a) changed the reference and the reverse test. Do not replace any script, and never copy section 6 of the design over them: that would undo D16 (no proxy, `-Cand` mandatory, no `results()` for the harness). Only check:

```bash
python3 -m unittest discover -s scripts/phase0 -p 'test_*.py'
python3 -c "open('scripts/phase0/audio-levels.ps1', 'rb').read().decode('ascii'); print('audio-levels.ps1 is ASCII')"
grep -c 'measure_overall=RMS_level' scripts/phase0/audio-levels.ps1
grep -c 'def results' scripts/phase0/audio_similarity.py
python3 scripts/phase0/audio_similarity.py corpus/owner/the-office-2005-s3d3-bluray/e16-stereo/audio-levels-1-3; echo "exit=$?"
grep -c 'def placed_first' scripts/phase0/audio_similarity.py
python3 scripts/phase0/audio_labels.py check corpus/owner
```

Expected:
- the unit tests end `Ran 97 tests` (or more, if later tasks added tests) and `OK`;
- `audio-levels.ps1 is ASCII`;
- `1`, then `1`;
- one line ending `this folder cannot be analysed: measure the title again with the new audio-levels.ps1, which writes audio-levels-20ms-<main>-<candidates>`, and `exit=1`: the first measurement's folder is refused, with no number printed;
- `1`; and `ok ...` for each labels file and `ok corpus/owner/audio-fitted.json (1 entries)`.

A failing test, a `0` from any grep, a `UnicodeDecodeError`, or any measure printed for the old folder means the scripts are not those of commit b48ad9a (D16 and the EXP-13 addendum): stop and report.

- [ ] **Step 14: Check the measurement script on the mini PC**

Which disc is in the drive: does not matter. Nothing here reads the drive or the share. Run this step only when no phase 0 job is running: `box.sh` replaces `C:\lazymkv\src`, from which jobs run.

```bash
scripts/box.sh
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -Command \"(Get-Command C:\lazymkv\src\scripts\phase0\audio-levels.ps1).Parameters.Keys\""
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\audio-levels.ps1 -Slug Bad_Slug -Label x -Main 1 -Cand 3"; echo "exit=$?"
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\audio-levels.ps1 -Slug the-office-2005-s3d3-bluray -Label e16-stereo -Main 1 -Cand 1"; echo "exit=$?"
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -Command \". C:\lazymkv\src\scripts\phase0\env.ps1; & \$FFMPEG -hide_banner -version | Select-Object -First 1; & \$FFMPEG -hide_banner -muxers | Select-String streamhash; & \$FFMPEG -hide_banner -layouts | Select-String -SimpleMatch 'DECOMPOSITION','5.1(side)'\""
```

Expected:
- `box.sh` exits 0.
- The parameter list begins `Slug`, `Label`, `Main`, `Cand`, `Force`, followed by PowerShell's common parameters. A parse error is a defect in Step 13.
- The `Bad_Slug` run prints `RESULT FAILED:` with Assert-Slug's rejection, then `exit=1`.
- The `-Main 1 -Cand 1` run prints `RESULT FAILED: stream 1 is the main stream and cannot also be a candidate`, then `exit=1`, within seconds and before the share is touched.
- The last command prints `ffmpeg version 9.0.2 ...`, a line containing `streamhash`, a line `NAME` ... `DECOMPOSITION`, and a line `5.1(side)` ... `FL+FR+FC+LFE+SL+SR`. If no `streamhash` line appears, the ffmpeg on the mini PC cannot hash streams: stop and report. If either layout line is missing, audio-levels.ps1 cannot read ffmpeg's table of channel layouts (it would stop with `ffmpeg -layouts printed no table`, or miss the centre channel): stop and report.

- [ ] **Step 15: Choose the reference and the candidates of each title**

Which disc is in the drive: does not matter (this runs on the Mac).

For each (SLUG, LABEL) in TITLES (the Office pairs and SLUG_A first; the recordings must have been pulled with `ffprobe-streams.json` in them):

```bash
python3 scripts/phase0/audio_dataset.py args corpus/owner/SLUG LABEL
python3 - SLUG LABEL <<'EOF'
import json, pathlib, sys
sys.path.insert(0, "scripts/phase0")
from robot import parse
slug, label = sys.argv[1], sys.argv[2]
d = pathlib.Path("corpus/owner") / slug
rip = json.loads((d / label / "rip-summary.json").read_text(encoding="utf-8-sig"))
probe = json.loads((d / label / "ffprobe-streams.json").read_text(encoding="utf-8-sig"))
scan = parse((d / "scan.txt").read_text(encoding="utf-8-sig", errors="replace"))
flags = [int(a.get(22, "0") or 0) for (t, s), a in sorted(scan.streams.items()) if t == rip["title"] and a.get(1) == "Audio"]
audio = [s for s in probe["streams"] if s.get("codec_type") == "audio"]
print(f"{slug}/{label}: title {rip['title']}, {len(audio)} audio streams in the file, {len(flags)} in the scan")
for k, s in enumerate(audio):
    tags = s.get("tags", {})
    f = flags[k] if k < len(flags) else None
    marks = [m for bit, m in ((1, "director-comments"), (2, "alt-director-comments"), (4, "visually-impaired"),
                              (512, "secondary"), (2048, "derived")) if f is not None and f & bit]
    print(f"  stream {s['index']}: {s.get('codec_name')} profile={s.get('profile')} {s.get('channels')} ch "
          f"{s.get('channel_layout')} lang={tags.get('language', 'und')} title={tags.get('title')!r} scan_flags={f} {' '.join(marks)}")
EOF
```

Expected:
- From `args`:
  - a first line `SLUG/LABEL: <n> audio streams (from <source>); original language <codes> (TMDb <xx>)`, or `original language English (TMDb en)`, or `original language none (TMDb xx: ...)`, or `original language unknown (<why>)`;
  - one `stream <n>: ... lang=<code>` line per audio stream;
  - the line `  programme reference: stream M (<codec ...>), placed first in <the original language | English | any language (no stream is in a wanted language)> (spec 8.6 steps 3 and 4); candidates: every other stream in <eng[, xxx]> or with no language, whatever its language`;
  - possibly one or more `  note: ...` lines: `the file has no stream in the original language` (an English-only rip of a foreign film; Disc A's `eng-only` holds only the commentary: measure it alone as printed), `the reference is not the first <lang> stream of the file` (a score, an M&E track or a commentary may be ranked above the programme; or two languages under one tag), or `the reference is a lossy stream of <n> channel(s), and stream <k> with no language code is lossless or has more than 2 channels`. Measure as printed, and name every note in the report with the title;
  - possibly `  dataset only: <lang> is not a wanted language; stream <k> against <...>` lines;
  - the job and `wait.sh` lines: first the programme reference's job (`-Main M -Cand C`, or `-Main M` alone when no other stream is wanted), then one dataset-only job per such line.
- M is the stream that spec 8.6 places first in the work's original language (most channels, then lossless, then the earlier stream; a lossy stream of two channels or fewer comes neither before a lossless one nor before the language's first stream when that has fewer channels), else in English. C is every other stream in English, in the original language or with no language. An English commentary or an English dub on a foreign film is a candidate against the original-language main track (EXP-13 addendum 9.1); content never drops it (row 4b). Keep derived cores (`derived`) and flagged streams in C: a core is a real lossy companion of the reference, and a flag is a free label.
- From the listing: one line per audio stream with ffprobe's stream number, codec, profile, channels, layout, language, track name, the scan's flags for that stream (k-th audio stream of the file against the k-th of the scan), and the names of the flags that are set.
- For the Office disc (original language English): `-Main 1 -Cand 2,3` (e16-stereo and t3-all-tracks) and `-Main 1 -Cand 2,3,4` (rehearsal), no note. For Disc A (Pan's Labyrinth, original language Spanish): `-Main 1 -Cand 2,3` for `all-tracks`, no note; `-Main 1` with the no-original-language note for `eng-only`. Their folders `audio-levels-20ms-1-2-3` and `audio-levels-20ms-1` already exist: Step 16 then says `already exists`, and Step 17 analyses them again.
- If `args` says `original language unknown (TMDb '<xx>' is not in audio_similarity.ISO639)`, stop and report: the table needs that code before the title is measured. If it says `no readable tmdb.json`, the title was not identified on TMDb; English is the reference language. Note the title for the report and go on. `original language unknown (tmdb.json gives none)` is handled like `no readable tmdb.json`.

If the two counts of audio streams differ, match the streams by codec, channels and language by hand, and say so in the report.

- [ ] **Step 16: Measure each title**

Which disc is in the drive: does not matter. The job reads only the ripped file on the share.

For each (SLUG, LABEL) and each job line that `args` printed in Step 15 (the programme reference's first):

```bash
scripts/phase0/job.sh audio-levels.ps1 LOG -Slug SLUG -Label LABEL -Main M -Cand C
scripts/phase0/wait.sh LOG
```

job.sh prints `started audio-levels.ps1; log: C:\lazymkv\phase0\logs\LOG.txt`. If it exits 1 because a phase 0 job is still running, wait for that job with its own wait.sh line first.

What wait.sh's exit codes mean for this job:
- **0**: check the log against the Expected below.
- **1**: read the `RESULT FAILED:` line.
  - Assert-Slug's rejection: SLUG or LABEL is mistyped. Check with `ls corpus/owner/SLUG`, then start again with log `LOG-2`.
  - `already exists and is kept as evidence`: this title was measured before. Its folder is the evidence: go on with Step 17.
  - `expected one .mkv in ... found 0` (with or without `the folder does not exist`): the rip is no longer in staging. Ask the owner whether the disc may go back in to rip that title again with `rip-title.ps1` under a new label (for example `e16-again`). If not, write the title down as "not measured: rip deleted" for the report, and go on with the next title.
  - `the file has no stream`, `is ... not audio`, `is the main stream and cannot also be a candidate` or `a candidate stream is listed twice`: M or C is wrong. Repeat Step 15 for this title, then start again with log `LOG-2`.
  - An error reading the share (`cannot reach the staging share`, `Cannot find path` or `Access ... is denied`): run `scripts/phase0/job.sh setup-check.ps1 setup-check-LOG` and `scripts/phase0/wait.sh setup-check-LOG`, and proceed as in Step 3.
  - `ffprobe failed`, `ffmpeg failed`, `streamhash.txt holds`, `wrote no raw-` or `holds no level`: pull the recording (`scripts/phase0/pull-recording.sh SLUG`), read `corpus/owner/SLUG/LABEL/audio-levels-20ms-M-CS/ffprobe.txt` (after `ffprobe failed`) or `.../ffmpeg.txt` (otherwise; it is written only once ffmpeg has run), and stop and report. A repeat after the fix adds `-Force` (see "When -Force is right" above).
  - `cannot reach the tuning dataset`, `describes a proxy of ... but staging holds ...`, `proxy check failed`, `the copy of ... in the dataset differs`, `the ffmpeg command line is` or `is from a run that did not finish`: each is handled as in Task 11 Step 11.
  - Any other reason (`ffmpeg not found`, `ffprobe not found`, `ffmpeg -layouts printed no table of channel layouts`, or any other error message from the script or from PowerShell): stop and report with the log, and do not repeat the job before the cause is known.
- **3**: still running. A Blu-ray episode should take a few minutes; a feature film may take up to an hour (not yet measured). Run wait.sh again, and start nothing else meanwhile. If three hours pass without a DONE line, end ffmpeg (not the job) with `ssh USER@makemkv-box "taskkill /im ffmpeg.exe /f"`, run wait.sh again (it then exits 1), and stop and report.
- **4**: the job ended without a DONE line: stop and report.
- **5**: the mini PC cannot be reached. Run wait.sh again once it can.

Expected (exit 0), in the log:
- `levels: <name>.mkv main=M cand=C centre=<True or False> ffmpeg_exit=0 seconds=<n> folder=audio-levels-20ms-M-CS`. `centre` is True for 5.1 and 7.1 references and False for mono and 2.0 ones. When ffprobe cannot name the reference's layout (`unknown`, as LPCM in Matroska reports it, or `<n> channels`), the script takes ffmpeg's own guess for that channel count and its `stream M:` line ends `(layout guessed by ffmpeg: <name>)`: `centre` is then True for 4 to 8 channels (4.0, 5.0, 5.1, 6.1, 7.1) and False for 3 (2.1). Note such a title for the report. A reference whose `stream M:` line shows a layout such as `5.1(side)` or `7.1` with `centre=False` is a defect: stop and report with the log. `seconds` is the cost on the J4125: write it down with the title's running time.
- One `stream <n>: ...` line for M and for each candidate, with `frames=<n>` (about 50 per second of running time) and `md5=<32 hex characters>`. Candidates whose frame count is more than 1% away from M's are judged on their overlap only; note them.
- `proxy: \\unas-pro\media\lazymkv-tuning\SLUG\LABEL\audio-proxy.mka tracks=<k> bytes=<b> (<n> mix, <n> fc, <n> side, ...)`: one `mix` for every audio stream of the file, in file order, one `fc` after each stream whose layout has a centre channel (5.1, 7.1), and one `side` after each two-channel stream. `bytes` is about 51 MB per hour of running time per track, and between 40 and 70 MB; outside that range, note it. A second job on the same LABEL prints `proxy: kept ... (made by an earlier run from this same file)` instead.
- `dataset: \\unas-pro\media\lazymkv-tuning\SLUG\LABEL\audio-levels-20ms-...`.
- `RESULT ok`, then `PHASE0-DONE status=ok exit=<n>`.
- For the Office disc: e16-stereo streams 1, 2 and 3 have three different MD5s. In rehearsal, stream 3's MD5 equals stream 1's and stream 4's equals stream 2's, as notes.md found with an MD5 of each stream's packets.

- [ ] **Step 17: Copy the measurements to the Mac and analyse them**

Which disc is in the drive: does not matter (this runs on the Mac).

```bash
scripts/phase0/pull-recording.sh SLUG
python3 scripts/phase0/audio_similarity.py corpus/owner/SLUG/LABEL/audio-levels-20ms-M-CS | tee corpus/owner/SLUG/LABEL/audio-content-20ms-M-CS.txt
```

Expected:
- `pulled corpus/owner/SLUG (...)`.
- The analysis begins with the folder, the file name and `measured in <n> s`, then `main stream M: ...; dialogue reference: its centre channel` (or `its mono mixdown (no centre channel)`).
- A folder measured with the reference alone prints `no candidate stream: nothing to judge (measured for the tuning dataset only)` after the main-stream line: it has no row for notes.md.
- for each candidate, either `verdict: duplicate of stream <n>` with its `row:` line, or:
  - its head line, ending `(cross-language: <candidate language> against <main language>)` when the two languages differ;
  - the lag and overlap line;
  - the `DC ... AG ... tenths ... SMI ... LS ... ADL ... added` line;
  - the `reverse:` line: `reverse: <s> s of gaps, DC <x>, AG <x>, tenths <a>/<b>, SMI <x>, LS <x> dB, added <s> s: <no commentary pattern | commentary pattern (the main track looks like a commentary over this one) | not judged (<reason>)>`, or `reverse: not judged (too short)`;
  - for a cross-language candidate with the commentary pattern whose language has another stream, a `peer: against stream <k> (placed first in the candidate's language): DC <x>, AG <x>, tenths <a>/<b>, SMI <x>, LS <x> dB: <commentary pattern | <verdict>, so not confirmed>` line;
  - `verdict: <commentary | commentary-cross-language | reference-unclear | ad-pattern | note-commentary | note-ad | keep>` with its meaning, and `not judged: <reason>` when row 3 of the rule applies (`reference-unclear` may come with it: the reverse run has the commentary pattern although the forward run cannot be judged), or only `verdict: not-judged (too short)` for a track under 20 s;
  - a `row: | ... |` line for notes.md.
- Each candidate takes a few seconds for an episode, and under a minute for a feature.
- For the Office disc, the first check of the whole chain:
  - e16-stereo stream 3 must come out `commentary`, with DC at most 0.30 and AG at least 0.50. The 16 kHz extracts gave DC 0.03, AG 0.66, tenths 10/10, SMI 0.58, LS 10.1 dB. The 48 kHz decode may differ in the second decimal.
  - Stream 2 (the derived DTS core) must come out `keep` with DC of 0.90 or more.
  - In rehearsal, stream 3 must be `duplicate of stream 1`, stream 4 `duplicate of stream 2`, and stream 2 `keep`.

  Any other result means the measurement chain on the mini PC differs from the one in the design: stop and report with both outputs. Do not change a threshold.
- For Disc A, `all-tracks/audio-levels-20ms-1-2-3`, the second check of the reference and reverse rules (EXP-13 addendum 9.4, table A):
  - stream 3 (the English commentary) must come out `commentary-cross-language`, marked `(cross-language: eng against spa)`, with DC 0.08, AG 0.54, tenths 9/10, SMI 0.51, LS 10.8 dB, `reverse: 470 s of gaps, DC 0.06, AG 0.31, tenths 1/10, SMI 0.39, LS 17.1 dB, added 796 s: no commentary pattern`, and no `peer:` line (it is the only English stream);
  - stream 2 must come out `keep` with DC 1.00.

  Its earlier output said `reference-unclear`: replace `audio-content-20ms-1-2-3.txt` with the new output (`tee`), and say so in the report.
- For Disc A, `eng-only/audio-levels-20ms-1`: `no candidate stream: nothing to judge (measured for the tuning dataset only)`.
- If audio_similarity.py prints instead one line ending `measure the title again with the new audio-levels.ps1, which writes audio-levels-20ms-<main>-<candidates>` (on standard error; it exits 1 and prints no number), the folder was not written completely by the new script: check the path, delete the empty `audio-content-20ms-M-CS.txt` that `tee` created, and go back to Step 16 for this title.

- [ ] **Step 18: Ask the owner what each measured track is, and record it**

Which disc is in the drive: does not matter.

1. For every candidate that is not a duplicate, ask the owner what it is. Give the stream as ffprobe's number and as a player shows it: with one video stream (stream 0), ffprobe's audio stream `<s>` is the player's audio track `<s>` counting from 1; with two video streams (a Dolby Vision enhancement layer), it is track `<s> - 1`. Do not show the verdict first. The owner plays the file from the staging share in a player that lists the audio tracks, at the start, the middle and near the end, and answers with one of:
   - main programme, lossy companion, derived core, alternate or original mix;
   - isolated score, M&E, same-language dub, dub (the programme in another language, tagged with that language), foreign dub, alternate narration;
   - commentary, partial commentary, audio description, trivia or other speech;
   - laugh-track version, sing-along, unknown;

   with how the owner knows: listened, packaging, menu, a scan flag, or TheDiscDB.
2. A derived core (scan flag 2048) and a duplicate by MD5 need no listening: their label is "derived core" or "duplicate".
3. Then append, to `corpus/owner/SLUG/notes.md` (never replacing what is in it), one section per disc, with one row per measured track:

```markdown
## Experiment 13 (Task 12)

Extra audio tracks measured against the title's programme reference (the track spec 8.6 places first in the original
language, else English; EXP-13 addendum 9.1) (`scripts/phase0/audio_similarity.py`; full output in
`<label>/audio-content-20ms-<M>-<C>.txt`). The owner named each track before seeing the measures.
Reference streams: <label> stream <M> (<codec> <channels> ch <language>), ...

| File | Track | What the measures say | What the owner says it is |
|---|---|---|---|
| <label>/<file name> | <n>: <codec> <channels> ch <language> "<track name>", scan flags <f> | DC <x>, AG <x>, tenths <a>/<b>, SMI <x>, LS <x>: <verdict> | <kind> (<how the owner knows>, <date>) |
```

   The middle column is the `row:` line's measures and verdict. A duplicate's middle column is `same packets as stream <n>: duplicate`.
4. For the Office disc, e16-stereo stream 3's owner column is `commentary (listened, 2026-10-05)`. That is already recorded under "Owner's ground truth".
5. Record every answer in the disc's labels file as well, never by editing it:

   ```bash
   python3 scripts/phase0/audio_labels.py auto corpus/owner/SLUG
   python3 scripts/phase0/audio_labels.py set corpus/owner/SLUG LABEL STREAM KIND --how HOW --saw-verdict no [--sure no] [--film-audible yes|no] --note "<the owner's words>"
   python3 scripts/phase0/audio_labels.py title corpus/owner/SLUG LABEL --title-kind TITLE_KIND --dialogue-heavy DIALOGUE
   ```

   Placeholders in these two lines: STREAM is the ffprobe stream number from the harness's To-label line; KIND one of the kinds `python3 scripts/phase0/audio_labels.py kinds` lists; HOW one of listened, packaging, menu, flag, thediscdb or md5; TITLE_KIND one of feature, episode, extra or other; DIALOGUE one of yes, no or unknown.

   Expected: one `labelled ...` line per track, and `title LABEL: ...`. The kinds (`audio_labels.py kinds`) are those of item 1: `main`, `lossy-companion`, `derived-core`, `alternate-mix`, `isolated-score`, `music-effects`, `same-language-dub`, `dub`, `foreign-dub`, `alternate-narration`, `commentary`, `partial-commentary`, `audio-description`, `other-speech`, `laugh-track-version`, `sing-along`, `unknown`, plus `duplicate`. An answer the owner is not sure of ("probably") gets `--sure no`. The Office disc's labels are already recorded, and its tracks never count.

- [ ] **Step 19: Add discs until the experiment's kinds are covered**

Which disc is in the drive: the disc being added, which the owner inserts.

1. Count the labelled tracks so far against the list in the experiment-13 row (the design, section 5, "Experiment 13"). Leave out every track of the Office disc (`the-office-2005-s3d3-bluray`): the thresholds were fitted on two of its tracks, so none of its tracks counts towards any total and it is not one of the discs counted. Count:
   - commentaries (and how many discs, feature films, and over clearly audible film they come from);
   - audio descriptions;
   - programme tracks other than references, and how many of them are separately encoded lossy companions or alternate mixes;
   - a title whose reference has no centre channel;
   - a dialogue-heavy or speech-only title;
   - isolated scores, M&E tracks, second dubs or narrations;
   - dubs in a wanted language (row 13's floor: 3 from 2 discs, 1 on a dialogue-heavy title; they gate only row 4b).
2. Ask the owner, once per missing kind, for a disc that has it (the design's question 12). Also ask for a foreign-language film with an English dub (and, if any, an English 2.0 companion of the dub), the one kind no stand-in can replace (EXP-13 addendum 9.2), and a mono or 2.0 film with an isolated score (9.3). Also ask for the Office disc again (the design's question 13): its title 3 should carry a second commentary, and titles 2, 5 and 6 duplicates. Its tracks are measured and reported like any other, and a programme track of it with the commentary verdict still makes the answer no, but they count towards no total (point 1).
3. For each disc the owner provides:
   - Run Step 6 (fingerprint, record if new, pull, `summarize_scan.py`). For a disc already recorded (the Office disc), skip the recording.
   - List the disc's titles with their audio streams by language, derived streams (scan flag 2048, MakeMKV's cores) left out:

     ```bash
     python3 scripts/phase0/audio_similarity.py --titles corpus/owner/NEW
     ```

     Expected: a first line `NEW: <n> titles, longest first; TMDb original language: <code or unknown>`, a line `audio: scan stream numbers by language, ...`, then one line per title: its number, running time, segments and `audio <language> <stream numbers>; ...`, and at the end `<- <language> x<n>` when one language has two or more streams, `<- wanted x<n> (<lang> <k>, ...)` when no wanted language has two streams but the wanted languages and no-language streams have two or more together, `ripped as <label>` for a title ripped before, and `shares segments with title <t>` for overlapping playlists.
   - Choose at most 4 titles of the disc, going down the list (longest first) and taking a title only when all of these hold:
     - its `<-` mark names `eng` or the original language of the first line (TMDb gives two letters; the scan uses the three-letter code, for example `fr` is `fra` or `fre`), or is `<- wanted x<n> (...)` (one stream in each wanted language, such as an English commentary on a foreign film);
     - it is a feature or an episode, not an extra (when unsure, ask the owner);
     - it has no `ripped as` (other than a failed rip), and it shares no segment with a title already taken (a play-all playlist).

     On a film disc this is normally the feature alone. For the Office disc it is titles 3, 5, 2 and 6 (question 13): titles 0 and 1 are `ripped as` e16-stereo and rehearsal, and title 4 (two segments, no subtitles, only stereo tracks) is taken to be an extra.
   - For each chosen title T, run Step 7's rip and probe with the label `tT-all-tracks`: `scripts/phase0/job.sh rip-title.ps1 rip-NEW-tT -Slug NEW -Title T -Label tT-all-tracks`, then `probe-title.ps1 ... -Label tT-all-tracks -SkipRemux`. Use Step 7's exit-code handling and its rules for `-Force`.
   - Pull the recording.
   - Add (NEW, `tT-all-tracks`) to TITLES and run Steps 15 to 18 for it.
4. Stop asking when every kind is covered, or when the owner has no disc left of a missing kind. Write each such kind down as "not available: <kind>".

- [ ] **Step 20: Judge the experiment and write the report**

Which disc is in the drive: does not matter.

1. Run `python3 scripts/phase0/audio_dataset.py; echo "exit=$?"` with no `--set`. Its experiment-13 section is the coverage table's source: the counts against row 13, `false verdicts`, `as it stands`, and `not counted`. Its kind-against-verdict table and the notes.md sections give the track table. No threshold is changed. `exit=1` with a `FAIL ... (programme audio), verdict commentary` or `FAIL ... (programme audio), verdict commentary-cross-language` line is a false verdict (for `FAIL ... swapped reference`, see the bullet below); `exit=3` means not every track was checked: deal with it as Task 11 Step 11 says before judging. The coverage table and every total leave out each track of the Office disc (`the-office-2005-s3d3-bluray`), whose S03E16 commentary and S03E15 duplicate the thresholds were fitted on; the track table lists them, marked "fitting disc, not counted".
   - **False verdict:** a track with the verdict `commentary` or `commentary-cross-language` that the owner labelled as programme audio (`main`, `lossy-companion`, `alternate-mix`, `laugh-track-version`, `derived-core`, `dub`), or as a kind the owner has decided to keep in answer to the design's questions 3 to 6 (`audio_labels.py` then lists that kind as programme audio, so the harness gives it a `FAIL`). An isolated score or M&E track is not programme audio (D14: not wanted); one with the commentary verdict is a note, `dropped as a commentary`. This includes the tracks of the Office disc.
   - **Answer yes:** every kind in the row is covered without any track of the Office disc, there is no false verdict, and at least one commentary got the verdict on each of at least 3 different discs other than the Office disc. The yes covers candidates in the reference's language only: `commentary-cross-language` is never dropped, and it does not count towards the 3 discs with a commentary verdict (the harness counts `commentary` only).
   - **Answer no:** any false verdict, on any disc, whatever the coverage. Also no when coverage is met but fewer than 3 discs other than the Office disc had a commentary verdict.
   - **Not answered:** coverage is not met and there is no false verdict.
   - **Swapped reference:** `exit=1` with `FAIL ... swapped reference` means that, were that labelled track ranked first, the programme would get the commentary verdict. Report it with both runs' measures. Until the owner decides question 4 of the design's section 9.9, treat it as blocking a yes.
2. Create `docs/phase0/experiments/EXP-13-commentary-by-audio.md` from `docs/phase0/experiments/TEMPLATE.md`. Header:
   - the date;
   - every disc measured, with slug, format and the titles;
   - the MakeMKV version;
   - `section 17, row 13 (added by this experiment)`.
3. Right after the header, put `## Coverage`:

```markdown
## Coverage

| Kind | Needed | Labelled tracks (discs), without the Office disc |
|---|---|---|
| Commentary | 8 from 4 discs; 2 on feature films; 1 with the film clearly audible | <n> (<discs>); films <n>; audible film <n> |
| Audio description | 1 if the owner has one (up to 3) | <n> or "owner has none" |
| Programme tracks other than the reference | 20, of which 6 separately encoded lossy companions or alternate mixes | <n>; separately encoded <n> |
| Reference without a centre channel, with an extra track | 1 title | <n> |
| Dialogue-heavy or speech-only title, with an extra track | 1 title | <n> |
| Isolated score, M&E, second dub or narration | every one the owner has | <n> |
| Dub in a wanted language (gates only row 4b) | 3 from 2 discs, 1 on a dialogue-heavy title (spec row 13; EXP-13 addendum 9.5) | <n> (<discs>) |

- not available: <kind>, for each kind the owner could not supply.
- not counted: every track of the-office-2005-s3d3-bluray (the thresholds were fitted on its S03E16 commentary and S03E15 duplicate).
```

4. The report must state, under its template headings:
   - **What was run:**
     - for each title, the rip label, M and C, the `audio-levels.ps1` command and log name;
     - the `seconds=` of each run, with the title's running time and file size: the J4125's cost per minute of running time for an episode and for a feature;
     - the `audio_similarity.py` command.
   - **What was observed:**
     - one table row per measured track: file, track, DC, AG, tenths, SMI, LS, ADL, reverse, cross-language, peer, swapped (the harness snapshot's `cross_language`, `peer` and `swapped_verdict`), verdict, the owner's label, and whether the two agree;
     - the harness's `cross-language`, `dubs` and `swapped references` lines, and every first-of-language note;
     - every duplicate found by MD5;
     - every track the rule did not judge, with the reason;
     - for each owner label, how many tracks got each verdict;
     - stereo width is not measured; the earlier figures (commentary -41.8 dB by notes.md, -30.3 dB whole-file; duplicate -3.4 dB) are quoted as observations only.
   - **Answer:**
     - "yes", "no" or "not answered", by the rules above;
     - every false verdict, with its measures;
     - the commentaries found and missed (the recall), with the kind of each miss: film audible, sparse, partial, too few gaps;
     - what the audio-description tracks got. AD is never dropped by content in this version, whatever it got.
   - **Consequence for the spec:**
     - on yes: add row 13 to section 17; in 8.6 step 1, add the duplicate row and the content row behind `drop_commentary_by_audio`; in 8.8, add the review reasons `commentary_detected` and `audio_reference_unclear`; in section 14, add the setting; in 6.4 step 10, the whole-title decode with its measured cost; change D12 and R6 to say that content may confirm a commentary;
     - on no or not answered: the same spec text, except that the content row only flags for review (`commentary_detected`) and never drops; D12 changes accordingly;
     - in every case: the duplicate row, and the open questions still open;
     - in every case also: 8.8's meanings of `audio_reference_unclear` and `commentary_detected` and the new rule row 4b (row 13's wording from EXP-13 addendum 9.5 is already in the spec; D15's rewording is the owner's call, addendum 9.9 question 7).

- [ ] **Step 21: Add the remaining rips of this task to the tuning dataset (decision D16)**

Which disc is in the drive: does not matter. The jobs read only the share.

```bash
python3 scripts/phase0/audio_dataset.py | grep '^Titles whose audio'
```

Expected: the line `Titles whose audio is in the tuning dataset ...`. Every SLUG/LABEL under `missing` is a rip that has not been added yet: normally the experiment-6 rips of Steps 6 to 8 that were not in TITLES. For each, run Task 11 Step 11 from `audio_dataset.py args` onwards, with its exit codes and its Expected. Stop when the line ends `<b> of <b> ...` with no `missing`, apart from rips written down as deleted.

**Experiment 14 (added 2026-10-06).** The owner asked whether code can tell regular, SDH, forced and commentary subtitle tracks apart by their content, with OCR of the text (decision D15). Steps 22 to 31 measure it on every English and no-language subtitle stream of the titles this task rips. The proof of concept read the Office's S03E16 track and Pan's Labyrinth's two tracks on 2026-10-06; their text is in the tuning dataset, and the thresholds were fitted on them.

These steps go after Step 21 of Task 12. Experiment 13's commit step (the old Step 22, which replaced the old Step 13) is replaced by Step 32 below. In them, "the design" is `docs/phase0/experiments/EXP-14-design.md` (committed by the controller, with the scripts it describes, before these steps run). Task 12's header gains:

- **Files:**
  - Prerequisite, committed by the controller with the design: `docs/phase0/experiments/EXP-14-design.md`, `scripts/phase0/subtitle_text.py`, `scripts/phase0/test_subtitle_text.py` and the changes to `scripts/phase0/ocr-subs.ps1` (the design, section 6), and the Global Constraints' sentence on reading every rip's subtitle text (in force from Step 31 on). This task does not change them.
  - Modify: `.gitignore` (two lines, Step 22) and the proof of concept's `corpus/owner/<slug>/<label>/subs-ocr/summary.json` files (their `examples` removed, Step 22).
  - Create: `docs/phase0/experiments/EXP-14-subtitles-by-text.md`.
  - Create (pulled from the mini PC): `corpus/owner/<slug>/<label>/subs-ocr/` (streams.json, ffmpeg logs, `sub-<n>.frames.json`, summary.json; no text), and `subs-ocr-changes/` for the title of Step 25.
  - Create (on the Mac): `corpus/owner/<slug>/<label>/subtitle-text.txt` (numbers only).
  - Append to (on the Mac only): a `## Experiment 14 (Task 12)` section in `corpus/owner/<slug>/notes.md` for every disc read.
- **Produces:** `ocr-subs.ps1 -Slug <slug> -Label <label> [-Streams <n>[,<n>...]] [-Languages eng,und|all] [-Render canvas|changes] [-Folder <name>] [-Force]`, run as a job. It writes `$Recordings\<slug>\<label>\<Folder>\` (no subtitle text) and `$TuningData\<slug>\<label>\<Folder>\` (the text, `examples.json`, the frames and a copy of evidence.mks). Its log lines are `ocr: <evidence> tesseract=<path> lang=eng psm=6 fps=4 render=<mode>`, up to 30 lines `  s:<n> t=<t> | <text>` per stream (the log stays on the mini PC and is not committed), one `skipped s:<n> (<codec>): not a bitmap subtitle track` line for each chosen stream that is not a bitmap stream (the DVD closed captions, which spec 8.7 sets aside; listed in summary.json), possibly `warning: s:<n> showinfo lines=<k> frames=<m>; times come from the file names`, one `stream s:<n> lang=<l> codec=<c> canvas=<WxH> frames=<n> text_frames=<n> lines=<n> bracket=<n> note=<n> hash=<n> speaker=<n> upper=<n> ffmpeg_s=<s> tesseract_s=<s> pts_offset=<s>` line per stream, `evidence: copied <bytes> bytes`, `written: <recording folder> and <dataset folder>`, then `RESULT ok` or `RESULT FAILED: <reason>`. After a failure the script throws, so wait.sh exits 1.

Placeholders for Steps 24 to 29:
- **TITLES14** is fixed when Step 25 starts and written at the top of the report: every (SLUG, LABEL) of this task whose `subs/index.json` lists an English or no-language bitmap stream (`HDMV PGS` or `VobSub`; a text stream such as the closed captions is not read): SLUG_A (`pans-labyrinth-2006-uhd`) with `all-tracks`, each NEW with `all-tracks` from Steps 6 to 8, and `the-office-2005-s3d3-bluray` with `e16-stereo`; plus each pair ripped in Step 29 as it is ripped. The Office and Pan's titles are the fitting titles: read, reported, never counted.
- **TEXTDIR** is `$HOME/lazymkv-text/SLUG-LABEL` on the Mac, outside the repository; it is deleted at the end of Steps 24 and 27.
- **LOG** is `ocr-SLUG-LABEL`, with `-2`, `-3` appended on a repeat.
- **LANGS** is `eng,und`, plus, on the first title of each disc whose TMDb original language is not English, that language's tag as `subs/index.json` spells it (for example `eng,und,spa`), so that the language check meets one real other-language track per disc.

When `-Force` is right for `ocr-subs.ps1`: only to repeat a reading that failed after its folders were created (a `RESULT FAILED:` reason beginning `ffmpeg exited`, `ffmpeg wrote no frame`, `ffmpeg gave`, `tesseract exited`, `tesseract returned` or `the copy of`), once the cause has been dealt with. Never over a folder whose run ended `RESULT ok` (it says `already exists and is kept as evidence`); a second reading of such a title uses `-Folder` with a new name instead.

- [ ] **Step 22: Check the scripts, take the example lines out of the corpus, add the .gitignore lines**

Which disc is in the drive: does not matter (this runs on the Mac).

The scripts are already in place: the controller installed `scripts/phase0/subtitle_text.py`, `scripts/phase0/test_subtitle_text.py` and the changes to `scripts/phase0/ocr-subs.ps1` with the design. Do not replace them. Add these lines at the end of `.gitignore`:

```
# Subtitle text and bitmaps stay in the tuning dataset (decision D16).
*.ocr.jsonl
corpus/owner/**/*.mks
```

The proof of concept wrote up to 12 example lines per marker into each summary.json of the recording; the Office's (committed) holds subtitle lines, which break D16. The new ocr-subs.ps1 writes them to the tuning dataset only. Remove the `examples` key from the two existing files, then check:

```bash
python3 - corpus/owner/*/*/subs-ocr/summary.json <<'EOF'
import json, sys
for p in sys.argv[1:]:
    d = json.load(open(p, encoding='utf-8-sig'))
    for s in d['streams']:
        s.pop('examples', None)
    open(p, 'w', encoding='utf-8').write(json.dumps(d, indent=2) + '\n')
    print(p, 'has no examples now')
EOF
grep -l '"examples"' corpus/owner/*/*/subs-ocr/summary.json
grep -cxF -e '*.ocr.jsonl' -e 'corpus/owner/**/*.mks' .gitignore
PYTHONDONTWRITEBYTECODE=1 python3 -W error -m unittest discover -s scripts/phase0 -p 'test_*.py'
for f in scripts/phase0/ocr-subs.ps1 scripts/phase0/subtitle_text.py scripts/phase0/test_subtitle_text.py; do python3 -c "import sys; open(sys.argv[1], 'rb').read().decode('ascii'); print(sys.argv[1], 'is ASCII')" "$f"; done
python3 -c "import sys; print(sum(open(f, encoding='ascii').read().count(chr(92) + 'u') for f in sys.argv[1:]))" scripts/phase0/subtitle_text.py scripts/phase0/test_subtitle_text.py
grep -c "'lines'\|Languages\|Render\|evidence.mks" scripts/phase0/ocr-subs.ps1
python3 scripts/phase0/subtitle_text.py --rec corpus/owner/the-office-2005-s3d3-bluray/e16-stereo --text /nonexistent; echo "exit=$?"
```

Expected:
- one line `<path> has no examples now` per summary.json (two on 2026-10-06: the Office's `e16-stereo` and Pan's `all-tracks`);
- nothing from the `grep -l`;
- `2` for `.gitignore`;
- the unit tests end `Ran <n> tests` and `OK` (`<n>` was 129 when this step ran on 2026-10-06; 172 after the design's review fixes, with experiment 14's 44 tests and experiment 13's tests of commit b48ad9a; more if later tasks add tests);
- three lines `... is ASCII`;
- `0`: no backslash-u escape in either Python file;
- a count of at least 4 for ocr-subs.ps1;
- `no sub-<n>.ocr.jsonl in /nonexistent for the streams of ...` and `exit=2`.

A failing test, a `UnicodeDecodeError`, a backslash-u escape or a `0` from the ocr-subs.ps1 count means the scripts are not the ones installed with the design: stop and report.

- [ ] **Step 23: Check the changed reading script on the mini PC, and clean its copies of the summaries**

Which disc is in the drive: does not matter. Nothing here reads the drive or the share. Run it only when no phase 0 job is running (`box.sh` replaces `C:\lazymkv\src`).

`pull-recording.sh` replaces files of the same name, so the mini PC's copies of the two summary.json files must lose their example lines too, or the next pull of either disc brings them back.

```bash
scp -q -o BatchMode=yes corpus/owner/the-office-2005-s3d3-bluray/e16-stereo/subs-ocr/summary.json USER@makemkv-box:C:/lazymkv/phase0/recordings/the-office-2005-s3d3-bluray/e16-stereo/subs-ocr/summary.json
scp -q -o BatchMode=yes corpus/owner/pans-labyrinth-2006-uhd/all-tracks/subs-ocr/summary.json USER@makemkv-box:C:/lazymkv/phase0/recordings/pans-labyrinth-2006-uhd/all-tracks/subs-ocr/summary.json
ssh USER@makemkv-box "findstr /m examples C:\lazymkv\phase0\recordings\the-office-2005-s3d3-bluray\e16-stereo\subs-ocr\summary.json C:\lazymkv\phase0\recordings\pans-labyrinth-2006-uhd\all-tracks\subs-ocr\summary.json"; echo "exit=$?"
scripts/box.sh
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -Command \"(Get-Command C:\lazymkv\src\scripts\phase0\ocr-subs.ps1).Parameters.Keys\""
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\ocr-subs.ps1 -Slug Bad_Slug -Label x"; echo "exit=$?"
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\ocr-subs.ps1 -Slug the-office-2005-s3d3-bluray -Label e16-stereo -Render sideways"; echo "exit=$?"
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -Command \"& C:\lazymkv\src\scripts\phase0\ocr-subs.ps1 -Slug the-office-2005-s3d3-bluray -Label e16-stereo -Languages eng,Fr\""; echo "exit=$?"
```

Expected:
- The two `scp` commands print nothing; `findstr` prints nothing, then `exit=1` (no file names the word). Any file name printed: the copy failed; repeat it before going on.
- `box.sh` exits 0.
- The parameter list begins `Slug`, `Label`, `Streams`, `Languages`, `Fps`, `Canvas`, `Lang`, `Psm`, `Render`, `Folder`, `Show`, `Examples`, `Force`, then PowerShell's common parameters. A parse error is a defect in the installed script: stop and report.
- The `Bad_Slug` run prints `RESULT FAILED:` with Assert-Slug's rejection, then `exit=1`.
- The `-Render sideways` run prints `RESULT FAILED: -Render must be canvas or changes`, then `exit=1`, within seconds and before the share is touched.
- The `-Languages eng,Fr` run, started through `-Command` as job.sh starts a job, prints `RESULT FAILED: -Languages takes lower-case language tags such as eng,und, or all; 'Fr' is neither`, then `exit=1`, before the share is touched: the list arrived as two tags (a `[string]` parameter would have named `'eng Fr'`).

- [ ] **Step 24: Analyse the two fitting titles (first check of the chain)**

Which disc is in the drive: does not matter.

Their text was read by the proof of concept on 2026-10-06 and is in the tuning dataset. For (SLUG, LABEL) = (`the-office-2005-s3d3-bluray`, `e16-stereo`) and (`pans-labyrinth-2006-uhd`, `all-tracks`):

```bash
scripts/phase0/job.sh tuning-fetch.ps1 fetch-text-SLUG-LABEL -Slug SLUG -Label LABEL -Text
scripts/phase0/wait.sh fetch-text-SLUG-LABEL
```

Expected (wait.sh exit 0): `fetched: ...\subs-ocr -> C:\lazymkv\phase0\tmp\tuning-fetch\SLUG\LABEL\subs-ocr (<bytes> bytes)` (about 48 KB for the Office, 260 KB for Pan's), the `scp` line and the clean-up line, `RESULT ok`. Exit 1: read the `RESULT FAILED:` line; `holds no OCR text` means the title was not read (stop and report); `already exists: an earlier fetch was not deleted` means run the clean-up command it prints, then start again with `-2`; anything else: stop and report. Exit 3: run wait.sh again. Exit 4 or 5: as in the Global Constraints.

Then, on the Mac, with the scp line the log printed (DEST = TEXTDIR):

```bash
mkdir -p "$HOME/lazymkv-text"
scp -q -r -o BatchMode=yes USER@makemkv-box:C:/lazymkv/phase0/tmp/tuning-fetch/SLUG/LABEL/subs-ocr "$HOME/lazymkv-text/SLUG-LABEL"
ssh USER@makemkv-box "rmdir /s /q C:\lazymkv\phase0\tmp\tuning-fetch"
python3 scripts/phase0/subtitle_text.py --rec corpus/owner/SLUG/LABEL --text "$HOME/lazymkv-text/SLUG-LABEL" --orig-lang ORIG | tee corpus/owner/SLUG/LABEL/subtitle-text.txt
rm -rf "$HOME/lazymkv-text/SLUG-LABEL"
```

ORIG is `en` for the Office and `es` for Pan's. Expected:
- For the Office: `stream s0 eng: events=345 lines=590 words=2157 letters=1.000 wordlike=0.998 readable=yes lang=en ... CUE=10 LABEL=7 CHEV=0 M=17 rate=0.0493 SO=9 CS=10 ... ODD=0 class=sdh  [text by order, 345 frames with text, 0 unmatched]`, `corrections: i_am=3 pipe_I=70`, and the table row `| s0 | eng | 345 | sdh | sdh | - | - | regular | regular (review: subtitle_text_disagrees) |`.
- For Pan's: `stream s0 eng: events=887 ... M=0 ... class=regular`, `stream s1 spa: events=884 ... lang=es (en=0.000 es=0.326 ...)`, `pair s0 s1: aligned=0.995/0.999 timing=same CB=0.002/0.002 content=different ...`, and rows `| s0 | eng | 887 | regular | regular | - | - | regular | regular |` and `| s1 | spa | 884 | regular | drop | - | not English (tag spa) | drop | drop |`.
- The last column, `Before yes`, was added by the design's review fixes after this step ran on 2026-10-06: the two `subtitle-text.txt` files committed then end at `8.7 today`. A repeat prints the column; every other line is unchanged.

Any other number means the chain differs from the design's: stop and report with both outputs. Do not change a threshold. `subtitle-text.txt` holds numbers only (the script never prints subtitle text).

- [ ] **Step 25: Time the render modes on one title**

Which disc is in the drive: does not matter. The job reads only the share.

Take the first (SLUG, LABEL) of TITLES14 other than the two fitting titles whose `evidence.mks` is still in staging (`ls` on the share is not possible from the Mac: the job itself fails with `no ...evidence.mks; run probe-title.ps1 first` when it is gone; then try the next title). For that title, read its English streams twice:

```bash
scripts/phase0/job.sh ocr-subs.ps1 ocr-SLUG-LABEL -Slug SLUG -Label LABEL -Languages LANGS
scripts/phase0/wait.sh ocr-SLUG-LABEL
scripts/phase0/job.sh ocr-subs.ps1 ocr-SLUG-LABEL-changes -Slug SLUG -Label LABEL -Languages LANGS -Render changes -Folder subs-ocr-changes
scripts/phase0/wait.sh ocr-SLUG-LABEL-changes
scripts/phase0/pull-recording.sh SLUG
```

The first run is also that title's reading for Step 26. What wait.sh's exit codes mean for these jobs:
- **0**: check the log against the Expected below.
- **1**: read the `RESULT FAILED:` line.
  - Assert-Slug's rejection, or `-Languages takes lower-case language tags ...`: SLUG, LABEL or LANGS is mistyped; check with `ls corpus/owner/SLUG` and `subs/index.json`, and start again with `LOG-2`.
  - `already exists and is kept as evidence`: this title was read before; its folder is the evidence. For the second run, choose another `-Folder` name.
  - `no ...evidence.mks; run probe-title.ps1 first`: the rip is no longer in staging. Write "not read: rip deleted" for this title and take the next one.
  - `has no subtitle stream`, `no subtitle stream s:<n>`: the title has no English or no-language stream; drop it from TITLES14. `no subtitle stream with a language in <list>`: the title has no English or no-language stream; drop it from TITLES14. `no bitmap subtitle stream to read`: the title's English and no-language streams are text streams only (closed captions, set aside by 8.7); drop it from TITLES14.
  - `ffmpeg exited`, `ffmpeg wrote no frame`, `ffmpeg gave` (only with `changes`: showinfo's frame times do not match the frames), `tesseract exited`, `tesseract returned`: pull the recording and read `<folder>/ffmpeg-<n>.txt.stderr` (`subs-ocr` or `subs-ocr-changes`); for the `changes` run this is a finding (the mode does not work on this ffmpeg): note it for the report and go on with `canvas` only. For the `canvas` run, stop and report; a repeat after the fix adds `-Force`. When the title will not be read again, delete the failed run's frames from C: (`ssh USER@makemkv-box "rmdir /s /q C:\lazymkv\phase0\work\subs-ocr\SLUG-LABEL"`).
  - `the copy of ...`: the share copy of the frames or of evidence.mks is short (check the share's free space); once dealt with, repeat with `-Force`.
  - `tesseract.exe not found`, `ffmpeg/ffprobe not found` or any other message: stop and report; do not repeat before the cause is known.
- **3**: still running. An episode takes about 3 minutes per English stream, a film about 13 (canvas). Run wait.sh again and start nothing else meanwhile. If 90 minutes pass without a DONE line, end ffmpeg and tesseract (`ssh USER@makemkv-box "taskkill /im ffmpeg.exe /f & taskkill /im tesseract.exe /f"`), run wait.sh again (it then exits 1), and stop and report.
- **4**: the job ended without a DONE line: stop and report.
- **5**: the mini PC cannot be reached; run wait.sh again once it can.

Expected (exit 0), in each log: the `ocr:` line with `render=canvas` (then `render=changes`), one `skipped s:<n> (<codec>): not a bitmap subtitle track` line per English or no-language text stream (the closed captions; none on a Blu-ray), one `stream s:<n> ...` line per English or no-language bitmap stream with `ffmpeg_s`, `tesseract_s` and `pts_offset`, `evidence: copied <bytes> bytes`, `RESULT ok`, `PHASE0-DONE status=ok exit=<n>`. Then compare, per stream, from the pulled `subs-ocr/summary.json` and `subs-ocr-changes/summary.json`: `text_frames` must equal the stream's event count (packets over 64 bytes in `subs/sub-<n>.packets.json`) in both, and the marker counts must be equal. Write down for the report, per stream: the running time, `ffmpeg_s` and `tesseract_s` of both modes, and the seconds per hour of running time. If `changes` gives a different `text_frames`, it is not used: note why.

- [ ] **Step 26: Read every title of TITLES14**

Which disc is in the drive: does not matter. The jobs read only the share.

For each (SLUG, LABEL) of TITLES14 not read yet (not the two fitting titles, not the Step 25 title), with the render mode chosen in Step 25 (add `-Render changes` only if it matched `canvas` there):

```bash
scripts/phase0/job.sh ocr-subs.ps1 ocr-SLUG-LABEL -Slug SLUG -Label LABEL -Languages LANGS
scripts/phase0/wait.sh ocr-SLUG-LABEL
scripts/phase0/pull-recording.sh SLUG
```

Exit codes and Expected as in Step 25 (one `stream` line per bitmap stream whose tag is in LANGS, one `skipped` line per text stream).

- [ ] **Step 27: Fetch the text, analyse each title, delete the copy**

Which disc is in the drive: does not matter.

For each (SLUG, LABEL) read in Steps 25 and 26: Step 24's commands, with ORIG the TMDb original language from `corpus/owner/SLUG/tmdb.json` (leave `--orig-lang` out to let the script read it). Expected: one `stream` line per stream read, with `[text by order, ...]` (or `[text by time, <n> frames with text, <k> unmatched]`: write down k, which must be 0 or the report names the events), the `pair` lines, the verdict table and one `row:` line per stream; a closed-caption stream prints `stream s<n> eng: set aside (closed captions: ...)` and another language's stream that was not read prints `stream s<n> <tag>: not read (no OCR text), ...` (it serves T4's timing only). Exit 2 with `stream s<n> <tag>: no OCR text ...` means an English or no-language bitmap stream of the title has no text in the tuning dataset (Step 26's reading should have read it): stop and report; never analyse the title without it. `rm -rf "$HOME/lazymkv-text/SLUG-LABEL"` ends the step for each title: no subtitle text stays on the Mac, and none enters the repository: `find . -name '*.ocr.jsonl' -not -path './.git/*' | grep -c .` prints 0.

- [ ] **Step 28: Ask the owner what each English subtitle stream is, and record it**

Which disc is in the drive: does not matter.

1. For each stream of Step 27, use the owner's answer from Step 5 when there is one (it was given before any verdict). Otherwise ask, without showing the verdict, what each English or no-language stream is: regular, SDH (US style, UK style or closed-caption style), translation, dub transcript, forced, commentary, trivia, lyrics or sing-along, another language, or unknown, and how the owner knows (packaging, menu, track name, player, TheDiscDB). `s<n>` is the n-th subtitle track of the file counting from 0; a player usually counts from 1.
2. Append to `corpus/owner/SLUG/notes.md` (never replacing what is in it), one section per disc, with one row per stream:

```markdown
## Experiment 14 (Task 12)

English and no-language subtitle streams read by OCR (`scripts/phase0/ocr-subs.ps1`) and judged by
`scripts/phase0/subtitle_text.py` (full output in `<label>/subtitle-text.txt`; the text itself is only in the tuning
dataset). The owner named each stream before seeing the measures.

| File | Track | What the measures say | What the owner says it is |
|---|---|---|---|
| <label>/evidence.mks | s<n>: <codec> <language>, <events> events | M <x>, SO <x>, CS <x>, lang <x>, <class>: <rule's role>[ (review: <note>)] | <kind> (<how the owner knows>, <date>) |
```

   The first three columns are the `row:` line of `subtitle-text.txt`. For the Office the owner's column is what question 10 of the design gets back (until then `SDH by content, not asked`); for Pan's English, `translation, regular (owner's expectation, not checked)` until confirmed.

- [ ] **Step 29: Add discs until the experiment's kinds are covered**

Which disc is in the drive: the disc being added, which the owner inserts.

1. Count the labelled tracks so far against the list of row 14 (the design, section 5), leaving out every track of the two fitting discs: titles with both a regular and an SDH English track (and their discs, films and episodes), SDH and regular tracks, forced tracks with events, commentary subtitle tracks, DVDs with English VobSub, UK-style SDH tracks, trivia, lyrics, dub-transcript and second-translation tracks.
2. Ask the owner, once per missing kind, for a disc that has it (the design's question 11).
3. For each disc the owner provides: run Steps 6 to 8 (fingerprint, record if new, choose the title, rip with `all-tracks`, probe), Step 5's listing and its question, then Steps 26 to 28 for the new (SLUG, LABEL) while the rip is in staging. On a DVD, note the closed-caption stream (the `-text` stream of Step 5's listing) and its conversion text.
4. Stop when every kind is covered, or when the owner has no disc left of a missing kind; write each such kind down as "not available: <kind>".

- [ ] **Step 30: Judge the experiment and write the report**

Which disc is in the drive: does not matter.

1. Build the coverage table and the track table from the notes.md sections, with no threshold changed. The coverage table and every total leave out each track of the two fitting discs; the track table lists them, marked "fitting disc, not counted".
   - subtitle_text.py does not apply T4's forced-only-parent and TheDiscDB steps nor T5's TheDiscDB rows; apply them by hand from `corpus/owner/SLUG/thediscdb-*.json` and Step 5's listing (flags 4096 and 2048 for a forced-only stream and its parent; DVD director's-comments flags 1 or 2, which 8.7 sets aside), and mark each verdict so changed in the track table.
   - A stream the script sets aside (the closed captions, a text stream) is reported with the owner's label but not judged: it counts neither as a labelled track nor as placed.
   - **Costly verdict:** rule role `drop` (with or without review) and the owner names it dialogue (regular, SDH, translation, dub transcript, closed captions); or, on a track with no review (no `subtitle_ambiguous`): role `regular` and the owner says SDH; role `sdh` and the owner says regular or translation; role `forced` and the owner names anything but forced; role `regular` or `sdh` and the owner says forced, commentary, trivia or lyrics. Any of these, on any disc, makes part (a) no.
   - **Placed:** a labelled track with no review.
   - **(a) yes:** every kind covered without the fitting discs, no costly verdict, and at least half of the labelled tracks placed. **No:** any costly verdict, or coverage met with fewer than half placed. **Not answered:** coverage not met and no costly verdict.
   - **(b) yes** when every track read with the chosen render mode took at most 150 s of `ffmpeg_s + tesseract_s` per hour of running time; otherwise no.
2. Create `docs/phase0/experiments/EXP-14-subtitles-by-text.md` from `docs/phase0/experiments/TEMPLATE.md`. Header: the date; every disc read, with slug, format and titles; the Tesseract and ffmpeg versions; `section 17, row 14 (added by this experiment)`. Right after it, `## Coverage`:

```markdown
## Coverage

| Kind | Needed | Labelled tracks (discs), without the fitting discs |
|---|---|---|
| Titles with both a regular and an SDH English track | 3 from 2 discs; 1 film, 1 episode | <n> (<discs>); films <n>; episodes <n> |
| SDH and regular English tracks | 6 and 6 | <n> and <n> |
| Forced English track with events | 1 | <n> |
| Commentary subtitle track | 1 | <n> |
| DVD with English VobSub (and its closed captions) | 1 | <n> |
| UK-style SDH | 1 if the owner has a UK disc | <n> or "owner has none" |
| Trivia, lyrics, dub transcript, second translation | every one the owner has | <n> |

- not available: <kind>, for each kind the owner could not supply.
- not counted: every track of the-office-2005-s3d3-bluray and pans-labyrinth-2006-uhd (the thresholds were fitted on them).
```

3. Under the template's headings, the report states:
   - **What was run:** per title, the label, the streams read, the `ocr-subs.ps1` command, log name and render mode; the timings of Step 25 and of every reading, per hour of running time; the `subtitle_text.py` commands.
   - **What was observed:** one row per track: file, track, events, M, SO, CS, ODD, CAPS_CUE, lang, class, rule role and review note, 8.7's role (`8.7 today`), the outcome before a yes (`Before yes`), the owner's label, and whether rule and owner agree; per owner label, how many tracks got each role; every pair's CB and alignment; the corrections counted; every frame left unmatched; what the note stand-in and the loose tokens found on any track with songs (and whether those were lyrics); the language shares of every other-language stream read. Quote at most five short lines of subtitle text in the whole report.
   - **Answer:** (a) and (b) by the rules above; every costly verdict with its measures; every review and its reason; the SDH tracks found and missed, with the kind of each miss (no marker, inline cues only, brackets misread, UK style).
   - **Consequence for the spec:** on (a) yes: 8.7 as section 3 of the design says (the sentences it lists), the thresholds of `RULE` as settings in section 14, and 6.4 step 10 as (b) decides; on (a) no or not answered: 8.7 gains only the review path (`subtitle_text_disagrees`); in every case, the open questions still open and the OCR engine facts for D17 (what the English model did with accents, notes and brackets).

- [ ] **Step 31: Read the remaining rips of this task (decision D16)**

Which disc is in the drive: does not matter.

Every rip of this task that ended `RESULT ok` and has an English or no-language bitmap subtitle stream, and was not read in Steps 25 to 29, is read now with `ocr-subs.ps1` (Step 26's commands and exit codes) while its `evidence.mks` is in staging; then `scripts/phase0/pull-recording.sh SLUG`. Nothing is analysed or fetched for them; their text goes to the tuning dataset only. A rip already deleted from staging is written down as "not read: rip deleted".

From now on this is part of the routine after every rip (the Global Constraints' subtitle-text sentence, D16): after the rip's probe and pull, and before the audio tuning-dataset jobs, `scripts/phase0/job.sh ocr-subs.ps1 ocr-<slug>-<label> -Slug <slug> -Label <label>` and its `wait.sh`, for every rip whose `subs/index.json` lists an English or no-language bitmap stream (`HDMV PGS` or `VobSub`); then `scripts/phase0/pull-recording.sh <slug>`. The text, the example lines (`examples.json`), the frames and evidence.mks go to `$TuningData\<slug>\<label>\subs-ocr\`; the recording gets only frames.json, summary.json (counts, no text), streams.json and the ffmpeg logs. A rip is not deleted from staging before this reading ended `RESULT ok` or was written down as failed with its reason.

- [ ] **Step 32: Commit** (replaces the old Step 13, and experiment 13's commit step, the old Step 22)

Which disc is in the drive: does not matter. Disc A is needed again in Task 13, which starts by asking the owner to put it back.

```bash
find corpus/owner -iname '*unit_key*'
grep -rIl 'eyJhbGciOi' corpus/owner
find corpus/owner -name '*.ocr.jsonl' -o -name 'evidence.mks' -o -name 'f-*.png'
grep -l '"examples"' corpus/owner/*/*/subs-ocr*/summary.json
ls -d "$HOME"/lazymkv-text/* 2>/dev/null
python3 -m unittest discover -s scripts/phase0 -p 'test_*.py'
git add .gitignore scripts/phase0/probe-audio.ps1 scripts/phase0/audio-levels.ps1 scripts/phase0/audio_similarity.py scripts/phase0/test_audio_similarity.py scripts/phase0/audio_dataset.py scripts/phase0/audio_labels.py scripts/phase0/proxy_levels.py scripts/phase0/test_audio_dataset.py scripts/phase0/tuning-fetch.ps1 scripts/phase0/ocr-subs.ps1 scripts/phase0/subtitle_text.py scripts/phase0/test_subtitle_text.py corpus/owner docs/phase0/experiments
git status --short
git commit -m "Run experiments 6, 13 and 14: audio profiles, subtitle alignment, commentary by audio, subtitles by text" -m "Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

Expected:
- The two `find` commands, both `grep` commands and the `ls` print nothing: no AACS unit-key file, no JSON Web Token, no subtitle text, bitmap, frame or example line in any recording, and no text copy left on the Mac. If any prints a path, stop and do not commit (delete a leftover Mac copy first; for a summary.json with examples, repeat Step 22's removal and Step 23's copy for that file).
- The unit tests end `OK`.
- `git status --short` lists:
  - the changed scripts and `.gitignore`;
  - the new files under `corpus/owner/`. These are what the old Step 13 listed, plus each `audio-content-20ms-*.txt`, each `audio-levels-20ms-*/streams.json`, the new `tT-all-tracks/` recordings, each `subs-ocr*/` folder (without text), each `subtitle-text.txt`, the two summary.json files without examples, and the `notes.md` changes. No `audio-levels-*/*.txt` file appears: `.gitignore` keeps them out.
  - the `audio-labels.json` of each disc labelled, `corpus/owner/audio-dataset.json`, and in each new level folder `streams.json`, `proxy-check.json` and `audio-proxy.json`.
  - the three reports.
- The commit succeeds.

---

### Task 13: Selection by language (experiment 11)

**Files:**
- Create: `scripts/phase0/exp11_tracks.py`, `scripts/phase0/test_exp11_tracks.py`
- Create: `docs/phase0/experiments/EXP-11-language-selection.md`
- Create (pulled from the mini PC): `corpus/owner/SLUG_A/eng-only/`. When Step 7 runs, also `corpus/owner/SLUG_D/eng-only/`, and the rest of `corpus/owner/SLUG_D/` for a disc not recorded before.
- Prerequisite, applied and committed by the controller before this task: `scripts/phase0/compare_tracks.py` (language from attribute 3) and its test in `scripts/phase0/test_decisions.py`

**Interfaces:**
- Consumes:
  - From `env.ps1` (decision D1): `Assert-DiscIs`, called directly in Steps 4 and 7. Through the scripts below, also `$Recordings`, `$Staging`, `Assert-Slug`, `Get-InsertedDisc`, `Assert-EmptyDir`, `Get-MkFlags` and `Get-DiscIndex`.
  - Task 11: `scripts/phase0/rip-title.ps1 -Slug <slug> -Title <n> -Label <name> [-Profile <path>] [-CacheMb <n>] [-Force]`.
    - It prints `rip: ...`, the summary JSON, then `RESULT ok` or `RESULT FAILED: <reason>`. `RESULT ok` carries a note when `MSG:1009` was seen.
    - It writes `$Recordings\<slug>\<label>\`: `rip.txt` (with `.timing.jsonl`, `.stderr` and `.exit.json`), `free-ram.csv`, and `rip-summary.json`. The summary's fields include `ok`, `result`, `title`, `profile`, `files`, `bytes`, `success_message`, `fail_codes`, `profile_errors`, `scan_source_file` and `rip_source_file`.
  - Task 11: `scripts/phase0/probe-title.ps1 -Slug <slug> -Label <name> [-SkipRemux] [-Force]`. It writes `mkvmerge.json`, `ffprobe-streams.json`, `mediainfo.json`, `subs\`, `evidence-mkvmerge.txt` and `probe-summary.json` (`ok`, `result`, `file`, `subtitle_tracks`, `remux_skipped`, ...).
  - Task 8: `scripts/phase0/record-disc.ps1 -Slug <slug> [-NoScan] [-ScanName <name>] [-ScanMayFail] [-Force]`, run as a job; without `-NoScan` it adds MakeMKV's scan `scan.txt`. Also `scripts/phase0/pull-recording.sh <slug>`, which never deletes on the Mac and exits 2 for a bad slug, 1 when the copy fails.
  - `scripts/box.sh`, `scripts/phase0/job.sh`, `scripts/phase0/wait.sh` and `scripts/phase0/summarize_scan.py <recording dir>`.
  - `scripts/phase0/compare_tracks.py <scan> <title> <mkvmerge json> [--selection all|eng]`, which exits 1 when any verdict is no. It exports `expected_rows`, `file_rows` and `same_track`. Also `scripts/phase0/robot.py` (`parse`, `duration_seconds`).
  - Task 7:
    - the profile `C:\lazymkv\phase0\profiles\eng-only.mmcp.xml`, whose `defaultSelection` is `-sel:all,+sel:video,+sel:(audio*(eng|nolang)),+sel:(subtitle*eng),-sel:mvcvideo,-sel:core,-sel:special,=100:all`;
    - MakeMKV's `default.mmcp.xml` in the same folder;
    - `lazymkv-capture profile --default <xml file> --name <text> --selection <text> --out <file>`.
  - Disc A's recording (Task 9) and its all-tracks rip (Task 11): `corpus/owner/SLUG_A/meta.json`, `scan.txt`, `all-tracks/rip-summary.json` and `all-tracks/mkvmerge.json`.
  - Every other recording in `corpus/owner` that has a `scan.txt` (from Tasks 8, 10 and 12).
- Produces:
  - `scripts/phase0/exp11_tracks.py`, run on the Mac from the repository root:
    - `coverage <recording dir or scan file>[@<title>] ...`.
      - With `@<title>`, it prints every stream of that title: `attr3`, `attr28`, codec, channels, flags and `predicted: keep|drop`. Then comes `COVERAGE <where> title <t>: audio-lang=<n> nolang=<n> sub-lang=<n> core=<n> special=<n> sub-flagged=<n> audio-512=<n>`.
      - Without it, it prints one such `COVERAGE` line, with the duration, for each title of 20 minutes or more.
      - A recording without a scan gives `<where>: no scan.txt`.
    - `match <recording dir> <title> <label>`.
      - It prints one line per file track and per missing predicted stream, tagged `predicted`, `EXTRA`, `MISSING` or `UNMATCHED`, then `DIFFERENCES missing=<n> extra=<n> unmatched=<n>`.
      - Then it lists the label's tracks, and the all-tracks rip's tracks when `<dir>/all-tracks` is a rip of the same title, for checking by eye.
    - Both exit 0 unless their input is missing or broken.
  - `scripts/phase0/test_exp11_tracks.py`: five tests, run with the others.
  - `corpus/owner/SLUG_A/eng-only/`, and `corpus/owner/SLUG_D/eng-only/` when Step 7 runs: the evidence of the eng-only rip and its probe. On the share, `\\unas-pro\media\lazymkv-staging\<slug>\eng-only\` holds the ripped `.mkv` and `evidence.mks`.
  - `docs/phase0/experiments/EXP-11-language-selection.md`: whether the later fast path (spec section 16, item 1) can be built.

Placeholders:
- SLUG_A: Disc A's slug, chosen in Task 9 (Task 11 calls it SLUG). It is in the `Disc(s):` line of EXP-05 and EXP-12.
- N: Disc A's main feature title number, the title Task 11 ripped as `all-tracks`. Step 3 prints it from `corpus/owner/SLUG_A/all-tracks/rip-summary.json`.
- SLUG_D: the slug of a second disc. It is needed only if Step 3 finds that Disc A's title does not test every part of the selection. It is either a disc already recorded with a scan, chosen in Step 3, or a DVD recorded in Step 7 under a new slug of lowercase letters, digits and hyphens (for example `knowing-2009-dvd`).
- T_D: the title of SLUG_D that Step 7 rips, chosen from its `COVERAGE` line.
- SELECTION_2: used only if MakeMKV rejects the profile (Step 5). It is the corrected selection string the controller chooses from MakeMKV's message.
- After a profile correction, every later command uses `eng-only-2` in place of `eng-only`: in the label, the log names, the paths and the profile file name.
- Disc A: the UHD film of Task 9. Task 12 ends with another disc in the drive, so Step 4 asks for Disc A again. rip-title.ps1 also checks the disc itself with Assert-DiscIs before MakeMKV starts.

When `-Force` is right (decision D3):
- `rip-title.ps1 ... -Force`:
  - only to replace a failed or interrupted attempt at the same slug and label whose `rip-summary.json` is missing or says `"ok": false`, once its cause has been dealt with;
  - never when the failure was MakeMKV rejecting the profile. That attempt is evidence, and the repeat uses the label `eng-only-2` (Step 5);
  - before using it, pull the recording and keep the failed attempt: `cp -R corpus/owner/<slug>/eng-only corpus/owner/<slug>/eng-only-failed-1` (use `-failed-2` if `-failed-1` already exists, and so on).
- `probe-title.ps1 ... -Force`: only to repeat a probe whose `probe-summary.json` is missing or says `"ok": false`, once its cause has been dealt with.
- `record-disc.ps1 ... -Force`: never in this task.

The prediction is the rule of `compare_tracks.py --selection eng`, applied to the scan:
- every video stream that is not 3D;
- every audio stream whose language (attribute 3) is English or absent (empty or `und`), except derived cores (flag 256) and flagged commentary or visually-impaired streams (flags 1, 2, 4);
- every English subtitle stream, whatever its flags.

A part of the selection is tested only where the title has streams it acts on:
- `audio*(eng|nolang)` needs audio in another language;
- `nolang` needs audio with no language;
- `subtitle*eng` needs subtitles in another language;
- `core` needs a derived core;
- `special` needs a flagged audio stream. Blu-ray and UHD discs never carry these flags (spec 9.5); DVDs can.

Step 3 counts these before anything is ripped.

- [ ] **Step 1: Write the analysis script**

Which disc is in the drive: none needed.

`scripts/phase0/exp11_tracks.py`:

```python
"""Experiment 11 (spec section 17, row 11): which streams of a title put the eng-only selection to the
test, and which of the title's streams a rip made with that selection kept.

  exp11_tracks.py coverage <recording dir or scan file>[@<title>] ...
      With @<title>: every stream of that title (attributes 3 and 28, codec, channels, flags, and whether
      compare_tracks' eng prediction keeps it), then its COVERAGE line. Without @<title>: one COVERAGE
      line for each title of 20 minutes or more. A recording dir is read as <dir>/scan.txt.
  exp11_tracks.py match <recording dir> <title> <label>
      Pairs the tracks of <dir>/<label>/mkvmerge.json, in order, with the scan streams they can be
      (type, language, audio channels, as compare_tracks does), and names each predicted stream that is
      missing and each unpredicted one that is present. Then it lists the file's tracks, and those of
      <dir>/all-tracks/mkvmerge.json when that rip is of the same title, for checking by eye.

COVERAGE counts the streams that put each part of the selection to the test: audio-lang (audio with a
language code other than English), nolang (audio with no language code, or und), sub-lang (subtitles
not in English), core (audio flag 256), special (audio flags 1, 2 or 4). Two more are only observed:
sub-flagged (subtitles with flags 1, 2 or 4) and audio-512 (secondary audio, flag 512).
Both commands exit 0 unless their input is missing or broken.
"""
import argparse
import json
import pathlib
import sys

from compare_tracks import expected_rows, file_rows, same_track
from robot import duration_seconds, parse

TOKENS = ("audio-lang", "nolang", "sub-lang", "core", "special", "sub-flagged", "audio-512")
LONG_TITLE_S = 20 * 60


def scan_path(where):
    path = pathlib.Path(where)
    return path / "scan.txt" if path.is_dir() else path


def load_scan(where):
    return parse(scan_path(where).read_text(encoding="utf-8-sig", errors="replace"))


def coverage(scan, title):
    """How many of the title's streams test each token (a stream's language is its attribute 3)."""
    n = dict.fromkeys(TOKENS, 0)
    for (t, _), a in sorted(scan.streams.items()):
        if t != title:
            continue
        kind, lang, flags = a.get(1, ""), (a.get(3) or "").lower(), int(a.get(22, "0") or 0)
        if kind == "Audio":
            if lang in ("", "und"):
                n["nolang"] += 1
            elif lang != "eng":
                n["audio-lang"] += 1
            n["core"] += bool(flags & 256)
            n["special"] += bool(flags & 7)
            n["audio-512"] += bool(flags & 512)
        elif kind == "Subtitles":
            n["sub-lang"] += lang != "eng"
            n["sub-flagged"] += bool(flags & 7)
    return n


def match(scan, title, ident):
    """(tag, scan row or None, file row or None) for every file track and every missing predicted stream.

    Each file track, in order, takes the next scan stream it can be. Tags: predicted, EXTRA (present but
    not predicted), MISSING (predicted but skipped), UNMATCHED (no later scan stream can be this track).
    """
    every = expected_rows(scan, title, "all")
    predicted = {r["stream"] for r in expected_rows(scan, title, "eng")}
    rows, i = [], 0
    for g in file_rows(ident):
        j = next((k for k in range(i, len(every)) if same_track(every[k], g)), None)
        if j is None:
            rows.append(("UNMATCHED", None, g))
            continue
        rows += [("MISSING", r, None) for r in every[i:j] if r["stream"] in predicted]
        rows.append(("predicted" if every[j]["stream"] in predicted else "EXTRA", every[j], g))
        i = j + 1
    rows += [("MISSING", r, None) for r in every[i:] if r["stream"] in predicted]
    return rows


def describe(track):
    p = track["properties"]
    return (f"{track['id']:>2} {track['type']:<9} {p.get('language', 'und'):<4} {track['codec']:<22} "
            f"ch={p.get('audio_channels', '')!s:<2} forced={p.get('forced_track')} "
            f"bytes={p.get('tag_number_of_bytes', '-')} name={p.get('track_name', '')!r}")


def run_coverage(args):
    for arg in args:
        where, _, title = arg.rpartition("@") if "@" in arg else (arg, "", "")
        if not scan_path(where).is_file():
            print(f"{where}: no scan.txt")
            continue
        scan = load_scan(where)
        if not title:
            for t, a in sorted(scan.titles.items()):
                try:
                    long_enough = duration_seconds(a.get(9, "0:00:00")) >= LONG_TITLE_S
                except ValueError:
                    long_enough = False
                if long_enough:
                    n = coverage(scan, t)
                    print(f"COVERAGE {where} title {t} ({a.get(9)}): " + " ".join(f"{k}={v}" for k, v in n.items()))
            continue
        t = int(title)
        if t not in scan.titles:
            sys.exit(f"{where}: the scan lists no title {t}")
        kept = {r["stream"] for r in expected_rows(scan, t, "eng")}
        print(f"{where} title {t} ({scan.titles[t].get(9, '?')}), every stream:")
        for (tt, s), a in sorted(scan.streams.items()):
            if tt == t:
                flags = int(a.get(22, "0") or 0)
                print(f"  stream {s:>2} {a.get(1, ''):<9} attr3={a.get(3)!r:<7} attr28={a.get(28)!r:<7} "
                      f"{a.get(6, ''):<8} ch={a.get(14, ''):<2} flags={flags:<5} "
                      f"predicted: {'keep' if s in kept else 'drop'}")
        n = coverage(scan, t)
        print(f"COVERAGE {where} title {t}: " + " ".join(f"{k}={v}" for k, v in n.items()))


def run_match(folder, title, label):
    d = pathlib.Path(folder)
    scan = load_scan(d)
    if title not in scan.titles:
        sys.exit(f"{d / 'scan.txt'} lists no title {title}")
    ident = json.loads((d / label / "mkvmerge.json").read_text(encoding="utf-8-sig"))
    every = expected_rows(scan, title, "all")
    rows = match(scan, title, ident)
    print(f"title {title}: {len(every)} streams without 3D video, {len(expected_rows(scan, title, 'eng'))} "
          f"predicted; {len(ident['tracks'])} tracks in {label}/mkvmerge.json")
    for tag, r, g in rows:
        if r:
            line = f"scan {r['stream']:>2} {r['type']:<9} {r['lang_raw']:<4} {r['codec']:<10} ch={r['ch']!s:<2} f={r['flags']}"
            if g:
                line += f"  <- file {g['id']:>2} {g['codec']}"
        else:
            line = f"file {g['id']:>2} {g['type']:<9} {g['lang_raw']:<4} {g['codec']} ch={g['ch']}"
        print(f"{tag:<10} {line}")
    counts = {t: sum(1 for row in rows if row[0] == t) for t in ("MISSING", "EXTRA", "UNMATCHED")}
    print("DIFFERENCES " + " ".join(f"{t.lower()}={n}" for t, n in counts.items()))
    print()
    print(f"{label} tracks, for checking the lines above by eye:")
    for track in ident["tracks"]:
        print("  " + describe(track))
    base, summary = d / "all-tracks" / "mkvmerge.json", d / "all-tracks" / "rip-summary.json"
    if not (base.is_file() and summary.is_file()
            and json.loads(summary.read_text(encoding="utf-8-sig")).get("title") == title):
        print("no all-tracks rip of this title to compare with")
        return
    tracks = json.loads(base.read_text(encoding="utf-8-sig"))["tracks"]
    print("all-tracks tracks of the same title; 'scan' is the stream at the same position, which is that")
    print("track when compare_tracks gave the all-tracks rip count=yes and order=yes:")
    for k, track in enumerate(tracks):
        stream = every[k]["stream"] if len(tracks) == len(every) else "?"
        print(f"  scan {stream!s:>2}  " + describe(track))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="command", required=True)
    cov = sub.add_parser("coverage")
    cov.add_argument("where", nargs="+")
    mat = sub.add_parser("match")
    mat.add_argument("folder")
    mat.add_argument("title", type=int)
    mat.add_argument("label")
    args = ap.parse_args()
    if args.command == "coverage":
        run_coverage(args.where)
    else:
        run_match(args.folder, args.title, args.label)


if __name__ == "__main__":
    main()
```

`scripts/phase0/test_exp11_tracks.py`:

```python
"""exp11_tracks.py: what a title can test of the eng-only selection, and how a rip's tracks are paired with the scan."""
import pathlib
import unittest

import exp11_tracks

MOVIES = pathlib.Path(__file__).resolve().parents[2] / "corpus" / "thediscdb" / "movie"
JOHN_WICK = MOVIES / "John Wick (2014)" / "4k-ultra-hd--blu-ray" / "disc01.txt"
KNOWING = MOVIES / "Knowing (2009)" / "2010-dvd" / "disc01.txt"


def ident(tracks):
    """A stand-in for `mkvmerge -J`: tracks are (type, language, audio channels or None)."""
    return {"tracks": [
        {"id": i, "type": kind, "codec": "x",
         "properties": dict({"language": lang}, **({"audio_channels": ch} if ch else {}))}
        for i, (kind, lang, ch) in enumerate(tracks)
    ]}


def tags(rows):
    return [(tag, r["stream"] if r else None) for tag, r, _ in rows]


class Coverage(unittest.TestCase):
    def test_a_uhd_with_only_english_audio_tests_neither_special_nor_nolang(self):
        n = exp11_tracks.coverage(exp11_tracks.load_scan(JOHN_WICK), 0)
        self.assertEqual(n, {"audio-lang": 0, "nolang": 0, "sub-lang": 2, "core": 1, "special": 0,
                             "sub-flagged": 0, "audio-512": 0})

    def test_a_dvd_with_a_flagged_commentary_tests_special(self):
        n = exp11_tracks.coverage(exp11_tracks.load_scan(KNOWING), 0)
        self.assertEqual(n, {"audio-lang": 1, "nolang": 0, "sub-lang": 2, "core": 0, "special": 1,
                             "sub-flagged": 2, "audio-512": 0})


class Match(unittest.TestCase):
    # Knowing, title 0: 0 video; audio 1 eng 6ch, 2 spa 6ch, 3 eng 2ch flag 1; subtitles 4 eng, 5 spa,
    # 6 eng flag 1, 7 spa flag 1. The eng prediction keeps 0, 1, 4 and 6.
    def setUp(self):
        self.scan = exp11_tracks.load_scan(KNOWING)

    def test_a_file_as_predicted_has_no_difference(self):
        rows = exp11_tracks.match(self.scan, 0, ident(
            [("video", "und", None), ("audio", "eng", 6), ("subtitles", "eng", None), ("subtitles", "eng", None)]))
        self.assertEqual(tags(rows), [("predicted", 0), ("predicted", 1), ("predicted", 4), ("predicted", 6)])

    def test_a_kept_commentary_and_a_dropped_subtitle_are_named(self):
        rows = exp11_tracks.match(self.scan, 0, ident(
            [("video", "und", None), ("audio", "eng", 6), ("audio", "eng", 2), ("subtitles", "eng", None)]))
        self.assertEqual(tags(rows), [("predicted", 0), ("predicted", 1), ("EXTRA", 3), ("predicted", 4), ("MISSING", 6)])

    def test_a_track_out_of_scan_order_is_unmatched(self):
        rows = exp11_tracks.match(self.scan, 0, ident(
            [("video", "und", None), ("subtitles", "eng", None), ("audio", "eng", 6)]))
        self.assertEqual(tags(rows), [("predicted", 0), ("MISSING", 1), ("predicted", 4), ("UNMATCHED", None), ("MISSING", 6)])


if __name__ == "__main__":
    unittest.main()
```

Run (from the repository root, one command at a time):

```bash
python3 -m unittest discover -s scripts/phase0 -p 'test_*.py' 2>&1 | tail -3
python3 scripts/phase0/exp11_tracks.py coverage "corpus/thediscdb/movie/John Wick (2014)/4k-ultra-hd--blu-ray/disc01.txt@0" "corpus/thediscdb/movie/Knowing (2009)/2010-dvd/disc01.txt"
```

Expected:
- `Ran 40 tests in ...` and `OK`: the 29 of Task 7, 5 from Task 12's prerequisite, 1 from this task's compare_tracks prerequisite and these 5 (Task 17 expects 40; 35 if Task 12's prerequisite has not landed). `OK` is what matters. A failure in `test_a_stream_without_a_language_code_is_und_not_the_metadata_language` means the compare_tracks prerequisite is missing: stop and report.
- The coverage command lists John Wick's 12 streams of title 0, among them `stream  2 Audio ... DD ch=6 flags=2304 predicted: drop` and the two `attr3='zho'` subtitle streams with `predicted: drop`.
- Then it prints `COVERAGE ... disc01.txt title 0: audio-lang=0 nolang=0 sub-lang=2 core=1 special=0 sub-flagged=0 audio-512=0`.
- Then, for the Knowing DVD, `COVERAGE ... disc01.txt title 0 (2:01:14): audio-lang=1 nolang=0 sub-lang=2 core=0 special=1 sub-flagged=2 audio-512=0`.
- So a UHD with only English audio cannot test `special`, `nolang` or the audio language filter, and a DVD can test `special`.

- [ ] **Step 2: Check the prerequisites, the scripts on the mini PC and the profile**

Which disc is in the drive: none needed; nothing in this step reads the drive.

```bash
grep -E '^(function (Assert-Slug|Assert-DiscIs|Assert-EmptyDir|Get-InsertedDisc)|\$Recordings)' scripts/phase0/env.ps1
grep -n 'if kind == "video" else None' scripts/phase0/compare_tracks.py
ls scripts/phase0/rip-title.ps1 scripts/phase0/probe-title.ps1 corpus/owner/SLUG_A/scan.txt corpus/owner/SLUG_A/all-tracks/mkvmerge.json
scripts/box.sh
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -Command \"(Get-Command C:\lazymkv\src\scripts\phase0\rip-title.ps1).Parameters.Keys\""
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -Command \"(Get-Command C:\lazymkv\src\scripts\phase0\probe-title.ps1).Parameters.Keys\""
ssh USER@makemkv-box "findstr /c:defaultSelection C:\lazymkv\phase0\profiles\eng-only.mmcp.xml"
```

Expected:
- The first `grep` prints five lines: the `$Recordings` assignment and the four D1 helper functions. The second prints the one `lang_raw = ...` line of the prerequisite. With anything else, the prerequisites are missing: stop and report.
- `ls` lists all four paths. A missing one means Task 11, or Disc A's recording, is not done: stop.
- `box.sh` syncs the tree and exits 0.
- The parameter lists begin `Slug`, `Title`, `Label`, `Profile`, `CacheMb`, `Force` and `Slug`, `Label`, `SkipRemux`, `Force`. PowerShell's common parameters follow.
- `findstr` prints one or more lines, and every one of them contains exactly `defaultSelection="-sel:all,+sel:video,+sel:(audio*(eng|nolang)),+sel:(subtitle*eng),-sel:mvcvideo,-sel:core,-sel:special,=100:all"`.
  - A line with another selection, or with `$app_DefaultSelectionString`, means the profile is not the one this experiment tests: stop and report.
  - `FINDSTR: Cannot open` means the profile is missing (Task 7): stop and report.
  - Copy this output into the report.

- [ ] **Step 3: See what Disc A can test, and choose a second disc**

Which disc is in the drive: none needed (Mac only).

```bash
python3 -c "import json; print('N =', json.load(open('corpus/owner/SLUG_A/all-tracks/rip-summary.json', encoding='utf-8-sig'))['title'])"
python3 scripts/phase0/compare_tracks.py corpus/owner/SLUG_A/scan.txt N corpus/owner/SLUG_A/all-tracks/mkvmerge.json | grep -E '^(TRACK COUNT|ORDER MATCH|CHAPTERS|DURATION|VERDICT)'
python3 scripts/phase0/exp11_tracks.py coverage corpus/owner/SLUG_A@N corpus/owner/*/
```

Expected:
- `N = <number>`: Disc A's main feature, the title of Task 11's all-tracks rip. Use it as N from here on.
- The all-tracks comparison, re-read with the corrected language rule. Note the `VERDICT` line. With `count=yes order=yes`, the `scan` column that `exp11_tracks.py match` prints beside the all-tracks tracks is reliable. A `chapters=no` or `duration=no` here is EXP-01's finding, and Step 6 will show it again; it is not a language-selection matter.
- Disc A's streams and its line `COVERAGE corpus/owner/SLUG_A title N: audio-lang=... nolang=... sub-lang=... core=... special=... sub-flagged=... audio-512=...`. Copy both into the report. The `attr3` column also answers an open question: how MakeMKV 2.0.0 writes a stream with no language. It may be `''` (empty), `'und'` or `None` (absent). Note which, or "no such stream on this title".
- Then one `COVERAGE` line for every title of 20 minutes or more in every recording that has a scan (Disc A's own titles among them), and `<folder>: no scan.txt` for recordings made with `-NoScan`.

Decide with these rules, and write the decision down for the report:
1. The five parts of the selection are counted by `audio-lang`, `nolang`, `sub-lang`, `core` and `special`. A part whose count on Disc A's line is 0 is not tested by Disc A.
2. If none of the five is 0, Disc A tests them all: Step 7 is skipped. (This does not happen on a Blu-ray or UHD, which never carries `special`'s flags.)
3. Otherwise, look among the other recordings' `COVERAGE` lines (not SLUG_A's) for titles whose count is above 0 for parts that Disc A missed.
   - If Disc A missed `special`, only a line with `special=` above 0 qualifies.
   - Among the qualifying lines, choose the one that covers the most of the parts Disc A missed.
   - Among equals, prefer a title that was already ripped as `all-tracks` (`corpus/owner/<slug>/all-tracks/rip-summary.json` names that title), then the longer title.
   - That line's folder name is SLUG_D, and its title is T_D.
4. If `special` is 0 on Disc A and on every other line, Step 7 first records a DVD that has an audio commentary. If parts other than `special` remain untested after the choice in rule 3 or that DVD, they stay "not exercised"; no further disc is asked for them.

Tell the owner now:
- Disc A is needed in Step 4, for well over an hour.
- Name the second disc, if one was chosen in rule 3. If rule 4 applies, ask instead whether they have a DVD whose packaging lists an audio commentary, and to have it ready for Step 7.
- If the owner has no such DVD, Step 7 is skipped: `special` is "not exercised" and Step 8 cannot answer yes.

- [ ] **Step 4: Put Disc A in the drive and check it**

Which disc is in the drive: Disc A, inserted in this step. Task 12 left another disc in the drive.

Ask the owner to take out the disc that is in the drive, insert Disc A, and say when the drive's light has stopped blinking. Then:

```bash
ssh USER@makemkv-box "tasklist | findstr /i makemkv"
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -Command \". C:\lazymkv\src\scripts\phase0\env.ps1; Assert-DiscIs SLUG_A; 'disc ok: SLUG_A'\""; echo "exit=$?"
```

Expected:
- `tasklist` prints nothing. If it prints a `makemkvcon64.exe` line, end that process with `ssh USER@makemkv-box "taskkill /im makemkvcon64.exe /f"`. If it prints `makemkv.exe` (the GUI), ask the owner to close the MakeMKV window. In both cases, run `tasklist` again.
- The check's last line is `disc ok: SLUG_A`, then `exit=0`. Lines that the helpers print may come before it. Otherwise:
  - An error from Assert-DiscIs, naming the inserted disc's fingerprint and the one recorded for SLUG_A, with `exit=1`: the disc is not Disc A. Ask the owner to insert Disc A, then run the check again.
  - An error from Get-InsertedDisc that no disc is mounted (or that lazymkv-capture failed): the drive may still be spinning up. Wait 30 seconds and run the check again. If it fails three times, stop and report.
  - `Assert-DiscIs` is not recognized: the env.ps1 prerequisites are missing; stop and report.
  - A PowerShell parse error in the command itself: go on to Step 5. rip-title.ps1 runs the same Assert-DiscIs before MakeMKV starts, and refuses a wrong disc within a minute (wait.sh exit 1).
  - `exit=255`: ssh could not reach the mini PC. Run the line again once it can.

- [ ] **Step 5: Rip Disc A's main feature with the language profile**

Which disc is in the drive: Disc A (SLUG_A). rip-title.ps1 checks it itself with Assert-DiscIs before MakeMKV starts.

```bash
scripts/phase0/job.sh rip-title.ps1 rip-eng-only -Slug SLUG_A -Title N -Label eng-only -Profile 'C:\lazymkv\phase0\profiles\eng-only.mmcp.xml'
```

The single quotes are for the Mac shell, which would otherwise delete every backslash. job.sh receives the path without quotes. Expected: `started rip-title.ps1; log: C:\lazymkv\phase0\logs\rip-eng-only.txt`. If job.sh exits 1 because a phase 0 job is still running, wait for that job with its own wait.sh line, then start this one.

MakeMKV scans the disc again before it writes anything; on a UHD with Java menus that takes ten minutes or more. How MakeMKV 2.0.0 reports a profile it cannot use, with a disc inserted, is not known (with no disc it says nothing at all). So watch the messages until saving has begun. Every two minutes or so, run:

```bash
ssh USER@makemkv-box "powershell -NoProfile -Command \"Get-Date -Format 'yyyy-MM-dd HH:mm:ss.fff'; Get-Content C:\lazymkv\phase0\recordings\SLUG_A\eng-only\rip.txt | Where-Object { \$_ -like 'MSG:1009*' -or \$_ -like 'MSG:501[45]*' -or \$_ -like '*profile*' }\""
```

It prints the mini PC's time, then any `MSG:1009` line, the `MSG:5014` line once saving has begun, and any other line that mentions the profile.
- An error that `rip.txt` cannot be found, for more than two minutes after job.sh started the job, means the script stopped before MakeMKV started. Run `scripts/phase0/wait.sh rip-eng-only` and read its `RESULT FAILED:` line (exit code 1, below).
- A `MSG:1009` line, or a line that mentions the profile and reads as an error (cannot, failed, invalid, error), means MakeMKV is not using the eng-only profile. End MakeMKV now with `ssh USER@makemkv-box "taskkill /im makemkvcon64.exe /f"`. The script then writes its summary and fails, and wait.sh exits 1. Go to "If MakeMKV rejected the profile" below.
- A line that only names the profile in use is not an error. Note it for the report and keep watching.
- Once the `MSG:5014` line has appeared, check once more two minutes later, then stop checking and wait:

```bash
scripts/phase0/wait.sh rip-eng-only
```

To see progress at any time:

```bash
ssh USER@makemkv-box "powershell -NoProfile -Command \"Get-Date -Format 'yyyy-MM-dd HH:mm:ss.fff'; Get-Content C:\lazymkv\phase0\recordings\SLUG_A\eng-only\rip.txt -Tail 2\""
```

What wait.sh's exit codes mean for this job:
- **0**: check the log against the Expected below.
- **1**: read the `RESULT FAILED:` line.
  - A reason from the checks before MakeMKV starts means nothing was ripped:
    - Assert-Slug: SLUG_A or the label was mistyped.
    - `profile not found`: the `-Profile` path lost its backslashes (check the single quotes) or the profile is missing (Step 2).
    - `scan.txt lists no title`: N is wrong (Step 3).
    - `MakeMKV is still running`: as in Step 4.
    - `is not empty`: an earlier attempt at this label exists. Read its `rip-summary.json` with `ssh USER@makemkv-box "type C:\lazymkv\phase0\recordings\SLUG_A\eng-only\rip-summary.json"` and follow "When -Force is right".
    - Assert-DiscIs: the disc is not Disc A (Step 4).
    - An error creating the folder on `\\unas-pro\media\lazymkv-staging`: check the share with `scripts/phase0/job.sh setup-check.ps1 setup-check-task13`, then `scripts/phase0/wait.sh setup-check-task13`. For that job, wait.sh exit 0 means: read the log, which must contain `staging    : \\unas-pro\media\lazymkv-staging : write, rename and delete OK`. If it says `FAILED`, ask the owner to check the NAS and run the check again (log name `setup-check-task13-b`) after a few minutes. Exit 1 means setup-check.ps1 threw: stop and report with the `ERROR:` line. Exit 3 is normal for a few seconds more: run wait.sh again. Exit 4: stop and report. Exit 5: run wait.sh again once the mini PC can be reached.

    Fix the cause, then start the same job again with the log name `rip-eng-only-b`.
  - You ended MakeMKV because of a profile message: go to "If MakeMKV rejected the profile" below.
  - Any other reason means the rip ran and the script judged it a failure (no success summary, a failure message 5003, 5004 or 5037, not exactly one `.mkv`, or another title's source file).
    - Pull the recording (`scripts/phase0/pull-recording.sh SLUG_A`), read `grep -E '^MSG:' corpus/owner/SLUG_A/eng-only/rip.txt | cut -c1-160`, and stop and report.
    - A repeat is decided with the owner. It is the same job command with `-Force` added and the log name `rip-eng-only-b`, after the failed attempt has been kept as "When -Force is right" says.
- **3**: still running. This is normal for well over an hour: run `scripts/phase0/wait.sh rip-eng-only` again, and start nothing else meanwhile.
  - Run the progress command now and then. MakeMKV is stuck if two progress checks 15 minutes apart show the same last line, or if three hours pass without a DONE line.
  - Then end MakeMKV, not the job: `ssh USER@makemkv-box "taskkill /im makemkvcon64.exe /f"`. The script then writes its summary and fails, and wait.sh exits 1. Stop and report.
- **4**: the job ended without a DONE line. Check `ssh USER@makemkv-box "tasklist | findstr /i makemkvcon"`, then stop and report.
- **5**: the mini PC cannot be reached. Run wait.sh again once it can.

Expected (exit 0):
- The log shows these lines, in order:
  - `rip: SLUG_A title N (scan source file: <file>) to \\unas-pro\media\lazymkv-staging\SLUG_A\eng-only with --cache=1024, started <yyyy-MM-dd HH:mm:ss.fff>`;
  - the summary JSON;
  - `RESULT ok`;
  - `PHASE0-DONE status=ok exit=<n>`. `exit=<n>` is only the last native command's exit code, not a verdict.
- In the JSON:
  - `ok` is true.
  - `profile` is `C:\lazymkv\phase0\profiles\eng-only.mmcp.xml` (JSON doubles each backslash).
  - `files` holds exactly one name ending in `.mkv`, and `bytes` is above 0.
  - `fail_codes` is empty and `profile_errors` is 0.
  - `success_message` is a `MSG:5005` or `MSG:5036` line.
  - `rip_source_file` equals `scan_source_file`. A null `rip_source_file` means the rip printed no `MSG:3307` for title N, as EXP-01 may already have noted.
- `RESULT ok` followed by a note `(MSG:1009 seen ...)` means MakeMKV rejected the profile and ripped with its own default selection. The rip says nothing about experiment 11 and is not a "no". Go to "If MakeMKV rejected the profile".

If MakeMKV rejected the profile:
1. Pull the recording and print what MakeMKV said about the profile:

   ```bash
   scripts/phase0/pull-recording.sh SLUG_A
   grep -n -i -E '^MSG:1009|profile' corpus/owner/SLUG_A/eng-only/rip.txt | cut -c1-240
   ```

2. Keep `corpus/owner/SLUG_A/eng-only` as it is: it is the evidence of the rejection, and it is committed in Step 10. Never use `-Force` on this label.
3. Stop and report to the controller with those lines and Step 2's `findstr` output. The controller either chooses a corrected selection string, SELECTION_2, or decides that experiment 11 is "not run: profile rejected". In that case, go to Step 8; Steps 6 and 7 are skipped.
4. With SELECTION_2, build the corrected profile once. Check first that it does not exist yet:

   ```bash
   ssh USER@makemkv-box "if exist C:\lazymkv\phase0\profiles\eng-only-2.mmcp.xml (echo EXISTS) else (echo absent)"
   ssh USER@makemkv-box "cd /d C:\lazymkv\phase0\profiles && C:\lazymkv\target\release\lazymkv-capture.exe profile --default default.mmcp.xml --name \"lazymkv eng only 2\" --selection \"SELECTION_2\" --out eng-only-2.mmcp.xml && findstr /c:defaultSelection eng-only-2.mmcp.xml"
   ```

   Expected:
   - The first command prints `absent`. If it prints `EXISTS`, a corrected profile was built before: stop and report rather than overwrite it.
   - The second prints `defaultSelection="SELECTION_2"` lines only.
5. Repeat this step and Step 6 with `eng-only-2` in place of `eng-only`: the label, the log names, the paths, and `-Profile 'C:\lazymkv\phase0\profiles\eng-only-2.mmcp.xml'`. Step 7 then uses it too. If MakeMKV rejects `eng-only-2` as well, experiment 11 is "not run: profile rejected": go to Step 8 with both attempts' messages.

- [ ] **Step 6: Probe the rip and compare it with the prediction**

Which disc is in the drive: Disc A stays in. The probe reads only the share.

```bash
scripts/phase0/job.sh probe-title.ps1 probe-eng-only -Slug SLUG_A -Label eng-only -SkipRemux
scripts/phase0/wait.sh probe-eng-only
```

`-SkipRemux` leaves out the copy-everything remux, which only experiment 4 needs. A switch passes through job.sh without quotes.

What wait.sh's exit codes mean for this job:
- **0**: check the log against the Expected below.
- **1**: read the `RESULT FAILED:` line.
  - `did not succeed`: the rip in Step 5 did not end `RESULT ok`; go back to Step 5.
  - `already holds probe evidence`: a probe ran before. Use `-Force` only as "When -Force is right" says.
  - Any other reason (mkvmerge, ffprobe or MediaInfo, or the subtitle extraction): pull the recording and read the file the reason names, for example `corpus/owner/SLUG_A/eng-only/evidence-mkvmerge.txt` or the `.stderr` file beside it. Stop and report. A repeat after the fix uses the same command with `-Force` added and the log name `probe-eng-only-b`.
- **3**: still running. This is normal: the subtitle extraction reads the whole file once over the network, which takes 10 to 30 minutes for a UHD feature. Run wait.sh again. If 90 minutes pass without a DONE line, end mkvmerge with `ssh USER@makemkv-box "taskkill /im mkvmerge.exe /f"`, run wait.sh again (exit 1), and stop and report.
- **4**: stop and report.
- **5**: run wait.sh again once the mini PC can be reached.

Expected (exit 0), in the log:
- `probe: \\unas-pro\media\lazymkv-staging\SLUG_A\eng-only\<name>.mkv (<bytes> bytes)`;
- the summary JSON, with `ok` true, `remux_skipped` true and `subtitle_tracks` the number of subtitle tracks in the file;
- `RESULT ok`, then `PHASE0-DONE status=ok exit=<n>`.

Then copy everything to the Mac and read it. Run the commands one at a time, not joined with `&&` and not under `set -e`: compare_tracks.py exits 1 when any verdict is no, which is a finding here, not a failure.

```bash
scripts/phase0/pull-recording.sh SLUG_A
cat corpus/owner/SLUG_A/eng-only/rip-summary.json corpus/owner/SLUG_A/eng-only/probe-summary.json
grep -E '^MSG:' corpus/owner/SLUG_A/eng-only/rip.txt | cut -d, -f1,4 | sort | uniq -c
python3 scripts/phase0/compare_tracks.py corpus/owner/SLUG_A/scan.txt N corpus/owner/SLUG_A/eng-only/mkvmerge.json --selection eng; echo "compare_tracks exit=$?"
python3 scripts/phase0/exp11_tracks.py match corpus/owner/SLUG_A N eng-only
python3 -c "import json; r = lambda l: json.load(open('corpus/owner/SLUG_A/' + l + '/rip-summary.json', encoding='utf-8-sig')); print('bytes: all-tracks', r('all-tracks')['bytes'], 'eng-only', r('eng-only')['bytes'])"
```

Expected:
- `pulled corpus/owner/SLUG_A (...)`. If the pull fails, nothing on the Mac changed; run it again.
- The two summaries say `"ok": true`, and `profile_errors` is 0.
- The message list holds every distinct message of the rip with its count:
  - a success summary (`5005` or `5036`), none of `5003`, `5004` or `5037`, and no `1009`;
  - read every message. One that mentions the profile and reads as an error is handled as in "If MakeMKV rejected the profile" (Step 5).
- compare_tracks prints the side-by-side table, then `TRACK COUNT`, `ORDER MATCH`, `CHAPTERS`, `DURATION`, `VIDEO TRACKS`, `STATISTICS TAGS`, a `VERDICT` line and the codec reminder.
  - For this experiment, read `count=` and `order=` on the `VERDICT` line.
  - The exit code is 0 only if chapters and duration also match. A `chapters=no` or `duration=no` that Step 3's all-tracks `VERDICT` also showed is not a failure of the language selection. One that the all-tracks rip did not show is noted in the report.
- `exp11_tracks.py match` prints:
  - one line per file track (`predicted` or `EXTRA`, with the scan stream it was paired with), a `MISSING` line for each predicted stream the file lacks, and an `UNMATCHED` line for a file track that no later scan stream can be;
  - then `DIFFERENCES missing=<n> extra=<n> unmatched=<n>`;
  - then the eng-only tracks and the all-tracks tracks of title N, each with its codec, `forced`, `bytes` (MakeMKV's statistics tag, when present) and `name`.

  `DIFFERENCES missing=0 extra=0 unmatched=0` means the rip is exactly as predicted. Any other line is read in Step 8.
- The `bytes:` line. The eng-only file is smaller unless Step 3's table shows no `predicted: drop` row. If both are about equal, or the eng-only tracks are the same as the all-tracks tracks, MakeMKV may have ignored the profile without saying so: stop and report before Step 8.

- [ ] **Step 7: Repeat on the second disc**

Which disc is in the drive: SLUG_D's disc, inserted in this step. At the end of the step, Disc A goes back in for Task 14.

Skip this step if Step 3's rule 2 applied (Disc A tests all five parts), if experiment 11 is "not run: profile rejected", or if Step 3's rule 4 applied and the owner has no DVD whose packaging lists an audio commentary (then `special` is "not exercised"). In every skip case Disc A stays in the drive for Task 14.

**7a. Record a DVD first, only if Step 3's rule 4 applies.** Ask the owner for a DVD whose packaging lists an audio commentary, and whether it was used earlier in this phase (Tasks 8 to 12).
- If it was used earlier, SLUG_D is its existing slug (see `ls corpus/owner`). If `corpus/owner/SLUG_D/scan.txt` exists, skip the record job and go straight to the four commands after it.
- Otherwise choose a new SLUG_D from the title, year and format, and check that it is free: `ls -d corpus/owner/SLUG_D` must print `No such file or directory`.

Ask the owner to insert the DVD and say when the drive's light has stopped blinking. Then:

```bash
scripts/phase0/job.sh record-disc.ps1 record-SLUG_D -Slug SLUG_D
scripts/phase0/wait.sh record-SLUG_D
```

What wait.sh's exit codes mean for this job:
- **0**: the log ends with `scan: scan.txt exit.json=... version_banner=True titles=<n above 0>`, `RESULT ok` and `PHASE0-DONE status=ok exit=<n>`.
- **1**: read the `RESULT FAILED:` line.
  - `no readable disc in D: after 120 s`: ask the owner whether the disc is in and the light has stopped, then start the job again as `record-SLUG_D-b`.
  - A rejection from Assert-Slug: the slug is malformed. Choose a valid one and start again.
  - A rejection from Assert-DiscIs: the slug already belongs to another disc. Choose a new slug and start again.
  - `... scan.txt already exists and is kept as evidence`: this disc already has a scan under SLUG_D. Use it and go on below.
  - Any other reason: stop and report.
- **3**: still running; a DVD scan takes a few minutes. Run wait.sh again. If 30 minutes pass without a DONE line, end the job with `ssh USER@makemkv-box "powershell -NoProfile -Command Stop-ScheduledTask -TaskName lazymkv-phase0"`, check that `ssh USER@makemkv-box "tasklist | findstr /i makemkvcon"` prints nothing, and stop and report.
- **4**: stop and report.
- **5**: run wait.sh again once the mini PC can be reached.

Then:

```bash
scripts/phase0/pull-recording.sh SLUG_D
python3 -c "import json,pathlib; [print(p.parent.name, json.loads(p.read_text(encoding='utf-8-sig')).get('fingerprint')) for p in sorted(pathlib.Path('corpus/owner').glob('*/meta.json'))]"
python3 scripts/phase0/summarize_scan.py corpus/owner/SLUG_D
python3 scripts/phase0/exp11_tracks.py coverage corpus/owner/SLUG_D
```

Expected:
- `pulled corpus/owner/SLUG_D (...)`.
- The fingerprint list shows SLUG_D's fingerprint once. If another slug has the same one, the disc was recorded before under that slug. Tell the controller, so that experiment 10 (Task 15) counts it once.
- The scan summary, and one `COVERAGE` line per title of 20 minutes or more.
- T_D is the feature-length title (as Task 11 Step 4 chose N) if its line has `special=` above 0. Otherwise it is any title whose line has `special=` above 0.
- If no line has `special=` above 0, this DVD does not flag its commentary. Ask the owner for one more such DVD and repeat 7a once. If the owner has no other such DVD, or after two DVDs without one, `special` is "not exercised": skip to 7e.

**7b. Insert and check SLUG_D's disc.** Ask the owner to insert SLUG_D's disc, if it is not already in, and say when the light has stopped. Then:

```bash
ssh USER@makemkv-box "tasklist | findstr /i makemkv"
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -Command \". C:\lazymkv\src\scripts\phase0\env.ps1; Assert-DiscIs SLUG_D; 'disc ok: SLUG_D'\""; echo "exit=$?"
python3 scripts/phase0/exp11_tracks.py coverage corpus/owner/SLUG_D@T_D
```

Expected:
- the same results as in Step 4, with SLUG_D in place of SLUG_A, and read the same way;
- then every stream of title T_D and its `COVERAGE` line. Copy both into the report.

**7c. Rip T_D with the language profile.**

```bash
scripts/phase0/job.sh rip-title.ps1 rip-eng-only-SLUG_D -Slug SLUG_D -Title T_D -Label eng-only -Profile 'C:\lazymkv\phase0\profiles\eng-only.mmcp.xml'
scripts/phase0/wait.sh rip-eng-only-SLUG_D
```

A DVD title takes a minute or two of scanning and 10 to 30 minutes of ripping. The profile was already accepted on Disc A, so the message watch of Step 5 is not needed here.

What wait.sh's exit codes mean for this job (the details are as in Step 5, with SLUG_D and T_D):
- **0**: check the log against the Expected below.
- **1**: read the `RESULT FAILED:` line. A reason from the checks before MakeMKV starts: fix it as in Step 5 and start the job again as `rip-eng-only-SLUG_D-b`. Any other reason: pull the recording, read `grep -E '^MSG:' corpus/owner/SLUG_D/eng-only/rip.txt | cut -c1-160`, and stop and report.
- **3**: still running; this is normal: run wait.sh again. The progress command is Step 5's with `SLUG_D` in the path. MakeMKV is stuck if two progress checks 10 minutes apart show the same last line, or if an hour passes without a DONE line. Then end MakeMKV with `ssh USER@makemkv-box "taskkill /im makemkvcon64.exe /f"`, run wait.sh again (exit 1), and stop and report.
- **4**: check `ssh USER@makemkv-box "tasklist | findstr /i makemkvcon"`, then stop and report.
- **5**: run wait.sh again once the mini PC can be reached.

Expected (exit 0): as in Step 5, with SLUG_D and T_D. On a DVD the scan has no source file name (attribute 16), so `scan_source_file` is null and nothing is compared. `RESULT ok` with a `MSG:1009` note: handle it as in Step 5's "If MakeMKV rejected the profile".

**7d. Probe, pull and compare.**

```bash
scripts/phase0/job.sh probe-title.ps1 probe-eng-only-SLUG_D -Slug SLUG_D -Label eng-only -SkipRemux
scripts/phase0/wait.sh probe-eng-only-SLUG_D
```

What wait.sh's exit codes mean for this job (the details are as in Step 6, with SLUG_D):
- **0**: the log shows `probe: ...`, the summary JSON with `ok` true and `remux_skipped` true, `RESULT ok` and `PHASE0-DONE status=ok exit=<n>`.
- **1**: read the `RESULT FAILED:` line and act as in Step 6; a repeat after the fix uses `-Force` and the log name `probe-eng-only-SLUG_D-b`.
- **3**: still running: run wait.sh again. A DVD probe usually ends within the first wait.sh call. After 30 minutes without a DONE line, end mkvmerge with `ssh USER@makemkv-box "taskkill /im mkvmerge.exe /f"`, run wait.sh again (exit 1), and stop and report.
- **4**: stop and report.
- **5**: run wait.sh again once the mini PC can be reached.

Then, one command at a time:

```bash
scripts/phase0/pull-recording.sh SLUG_D
cat corpus/owner/SLUG_D/eng-only/rip-summary.json corpus/owner/SLUG_D/eng-only/probe-summary.json
grep -E '^MSG:' corpus/owner/SLUG_D/eng-only/rip.txt | cut -d, -f1,4 | sort | uniq -c
python3 scripts/phase0/compare_tracks.py corpus/owner/SLUG_D/scan.txt T_D corpus/owner/SLUG_D/eng-only/mkvmerge.json --selection eng; echo "compare_tracks exit=$?"
python3 scripts/phase0/exp11_tracks.py match corpus/owner/SLUG_D T_D eng-only
```

Expected: as in Step 6. On a DVD, a `chapters=no` from MakeMKV's `insertFirstChapter00IfMissing` is not a language-selection matter either. `match` prints `no all-tracks rip of this title to compare with` unless SLUG_D's `all-tracks` rip (Task 12) is of title T_D.

**7e. Put Disc A back.** Ask the owner to take out SLUG_D's disc and insert Disc A for Task 14. Task 14 checks it again.

- [ ] **Step 8: Write the experiment 11 report**

Which disc is in the drive: does not matter.

First check every `MISSING`, `EXTRA` and `UNMATCHED` line from Steps 6 and 7 by eye, against the track lists printed under it.
- The pairing uses only type, language and audio channels, as compare_tracks does. Where two streams share these (two English PGS tracks, say), it may name the wrong one of them.
- Where both lists show `bytes`, a track of the eng-only rip is the all-tracks track with the same `bytes`. The all-tracks track's `scan` number is its stream when the all-tracks rip's `VERDICT` was `count=yes order=yes`. For Disc A that is Step 3's `VERDICT`. For SLUG_D, when `match` listed an all-tracks rip, get it with `python3 scripts/phase0/compare_tracks.py corpus/owner/SLUG_D/scan.txt T_D corpus/owner/SLUG_D/all-tracks/mkvmerge.json | grep VERDICT`.
- Otherwise compare codec, `forced` and `name`.
- Correct any line that names the wrong stream, and say so in the report.

Then read each line with these rules. A "difference" means that MakeMKV did not do what the prediction says. An "observation" is recorded but does not decide the answer.
- `EXTRA` audio with flag 256 (for example 2304): `-sel:core` kept a derived core. Difference, `core`.
- `EXTRA` audio with flag 1, 2 or 4: `-sel:special` kept a flagged stream. Difference, `special`.
- `EXTRA` audio with a language code other than English: the audio language filter let it through. Difference, `audio*(eng|nolang)`.
- `EXTRA` subtitles not in English: `subtitle*eng` let them through. Difference, `subtitle*eng`.
- `MISSING` audio with no language (attribute 3 empty, absent or `und`): `nolang` did not keep it. Difference, `nolang`.
- `MISSING` English audio without flags 1, 2, 4, 256 or 512: the language filter dropped it. Difference, `audio*(eng|nolang)`.
- `MISSING` audio with flag 512 (secondary audio): the selection drops it, as spec 8.6 does anyway. Observation.
- `MISSING` English subtitles with flag 1, 2 or 4: `special` also drops flagged subtitles. Spec 8.7 drops DVD subtitles flagged 1 or 2 anyway. Observation; for flag 4, report it to the controller.
- `MISSING` English subtitles of any other kind, forced-only (6144) included: `subtitle*eng` dropped them. Difference, `subtitle*eng`. Exception (Global Constraints, `MSG:4047`): a forced-only stream that the rip log says MakeMKV removed as empty, with the message's index mapped through the selected streams (compare_tracks' `REMOVED` line), was selected and is not a difference; `match` does not read the rip log, so such a stream shows as `MISSING` there and the all-tracks listing shows `scan ?`. Likewise, `match` pairs greedily by type, language and channels: a dropped commentary can appear as an `EXTRA` plus a `MISSING` pair; the check by eye (bytes, codec, name) settles it.
- `MISSING` video: difference.
- `UNMATCHED`: a file track out of scan order, or one the scan does not explain. An `UNMATCHED` video track is most likely 3D video that `-sel:mvcvideo` should have dropped. Difference, order (or `mvcvideo` for video).

The order is the scan order when no `UNMATCHED` line remains after the check by eye, because the pairing only moves forward through the scan. When the rip has no difference, compare_tracks' `order=yes` says the same.

Create `docs/phase0/experiments/EXP-11-language-selection.md` from `docs/phase0/experiments/TEMPLATE.md`. Fill in:
- `Disc(s):` SLUG_A (UHD, drive D:) and, if Step 7 ran, SLUG_D with its format.
- `MakeMKV version:` 2.0.0, from the `MSG:1005` line of `rip.txt`.
- `Spec reference:` section 17, row 11.

It must state:
- Under "What was run":
  - the profile file and its selection string (Step 2's `findstr` output, and SELECTION_2 if Step 5 needed it);
  - for each disc: the title (N, T_D), the label, and `ok`, `elapsed_s` and `bytes` from `rip-summary.json`, against the all-tracks rip's `bytes` where one exists;
  - Step 3's choice of second disc and why, or why there was none.
- Under "What was observed":
  1. For each title ripped: its stream table and `COVERAGE` line (Steps 3 and 7b), compare_tracks' table and `VERDICT` line, and the `match` lines after the check by eye.
  2. Every predicted stream that is missing and every unpredicted stream that is present, each with its scan stream number, type, language, codec and flags. Write "none" when there are none.
  3. A table with one row per part of the selection:

     ```markdown
     | Part | Tested on (slug, title, streams) | As predicted? | Evidence |
     |---|---|---|---|
     | audio language filter (other languages dropped) | ... | yes / no / not exercised | ... |
     | `nolang` | ... | ... | ... |
     | `subtitle*eng` | ... | ... | ... |
     | `core` | ... | ... | ... |
     | `special` | ... | ... | ... |
     ```

     A part is "not exercised" when its `COVERAGE` count was 0 on every title ripped. `core` and `special` count as exercised only by a stream the language filter keeps (an English derived core; a flagged English track): `COVERAGE` counts them in every language, and a Spanish core that the language filter drops anyway shows nothing about `-sel:core`.
  4. Observations:
     - whether flagged subtitles (`sub-flagged`) were kept or dropped;
     - whether secondary audio (`audio-512`) was kept or dropped. Write "no such stream" where the count was 0. This is how MakeMKV's `special` treats flag 512; it is recorded, not judged;
     - how a stream with no language appears in attributes 3 and 28 (Step 3), and which language the file gives it;
     - every `MSG:1009` or message that mentions the profile, or "none";
     - any `chapters=no` or `duration=no` that the all-tracks rip did not show.
  5. Order: whether each rip's tracks came in scan order.
- Under "Answer", one of:
  - **yes**: no difference on any rip, and each of the five parts was exercised on at least one rip;
  - **no**: at least one difference. Name each one, with the part it belongs to;
  - **not answered**: no difference, but some parts were not exercised. Name them, and say which parts did behave as predicted;
  - **not run: profile rejected**: MakeMKV rejected the profile (and SELECTION_2, if tried). Quote the message.
- Under "Consequence for the spec":
  - On **yes**: spec section 16 item 1 stands. Add the observations that shape it. For example: if secondary audio was kept, a title that has it still needs a remux, because spec 8.6 drops it. Also say whether flagged subtitles are dropped by the selection.
  - On **no**: record which parts behaved differently (for example whether `core` removed derived cores, and whether `nolang` kept streams with no language). Mark the fast path as not buildable with this selection string: spec 17 row 11's fallback, every title is remuxed.
  - On **not answered** or **not run**: section 16 item 1 stays a later feature, marked "not yet verified", with the parts not exercised or the rejection message. Spec 17 row 11 says the same.

- [ ] **Step 9: Add this task's rips to the tuning dataset (decision D16)**

Which disc is in the drive: does not matter (Disc A should be back in for Task 14, as Step 7 says). The jobs read only the share.

```bash
python3 scripts/phase0/audio_dataset.py | grep '^Titles whose audio'
```

Expected: the line `Titles whose audio is in the tuning dataset ...`, naming under `missing` `SLUG_A/eng-only` (or `eng-only-2`), and `SLUG_D/eng-only` when Step 7 ran. A rip that MakeMKV made with its default selection after rejecting the profile is still a rip: add it too. For each SLUG/LABEL named, run Task 11 Step 11 from `audio_dataset.py args` onwards, with its exit codes and its Expected. The eng-only rip of SLUG_A is the title Task 11 ripped as `all-tracks`. Its streams have the same packets (the `md5=` of its `stream` lines equals those of `corpus/owner/SLUG_A/all-tracks/audio-levels-20ms-*/manifest.txt`). It needs no new labels: its one stream is the title's reference, so the harness lists the title as in the dataset without a label line (a label is asked for candidate streams only) and adds nothing to experiment 13's counts. A packet MD5 that differs from the all-tracks rip's for the same stream is a finding for EXP-11: note it.

In Step 10 (Commit), `git status --short` also lists the new level folders' `streams.json`, `proxy-check.json` and `audio-proxy.json`, and `corpus/owner/audio-dataset.json`.

- [ ] **Step 10: Commit**

Which disc is in the drive: does not matter. Disc A should be back in for Task 14 (Step 7e; when Step 7 was skipped, it never left the drive).

```bash
find corpus/owner -iname '*unit_key*'
grep -rIl 'eyJhbGciOi' corpus/owner
python3 -m unittest discover -s scripts/phase0 -p 'test_*.py' 2>&1 | tail -1
git add scripts/phase0/exp11_tracks.py scripts/phase0/test_exp11_tracks.py corpus/owner docs/phase0/experiments
git status --short
git commit -m "Run experiment 11: selection by language" -m "Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

Expected:
- `find` and `grep` print nothing: no AACS unit-key file, and no JSON Web Token in any recording (the TMDb read access token is one, and it starts with that text). If either prints a path, stop and do not commit.
- The tests end `OK`.
- `git status --short` lists:
  - the two new scripts;
  - the new files under `corpus/owner/SLUG_A/eng-only/`, plus `eng-only-2/` or `eng-only-failed-<n>/` if they exist;
  - the files under `corpus/owner/SLUG_D/`, if Step 7 ran;
  - `docs/phase0/experiments/EXP-11-language-selection.md`.

  `compare_tracks.py` and `test_decisions.py` are not listed: the controller committed them as prerequisites.
- The commit succeeds.

---

### Task 14: Whole-disc copy and eject (experiments 3 and 7)

**Files:**
- Create: `scripts/phase0/backup-disc.ps1` (Step 1), `scripts/phase0/extract-from-copy.ps1` (Step 2), `scripts/phase0/second-drive.ps1` (Step 3).
- Create (pulled from the mini PC): `corpus/owner/SLUG_A/backup/`, `corpus/owner/SLUG_A/extract/`, `corpus/owner/SLUG_A/drive2/`, and any `backup-failed-N/` or `extract-failed-N/` copy that "When -Force is right" calls for.
- Create: `docs/phase0/experiments/EXP-03-whole-disc-copy.md`, `docs/phase0/experiments/EXP-07-eject.md`.
- Modify: `docs/phase0/experiments/EXP-05-offline-and-unit-key.md` (the second drive), `docs/phase0/environment.md` (the optical drives).
- Outside the repository, and only if the attached drive's letter changes (Step 10): `C:\lazymkv\phase0\config.ps1` on the mini PC.

**Interfaces:**
- Consumes:
  - From `env.ps1`: `$P0`, `$Recordings`, `$Staging`, `$CAP`, `$MK`, `$MKVMERGE`, `$AllTracksProfile`, `Get-MkFlags`, `Get-DiscIndex`, `Assert-Slug`, `Assert-DiscIs`, `Assert-EmptyDir` (decision D1). `$Drive` is used through Assert-DiscIs and Get-DiscIndex.
  - From the capture tool:
    - `lazymkv-capture record --out <file> -- <command...>`: writes `<file>` (the command's standard output), `<file>.timing.jsonl`, `<file>.stderr` and, when the command has ended, `<file>.exit.json` (`exit_code`, `elapsed_ms`, `lines`).
    - `lazymkv-capture disc --root <path> --label <text> --out <dir>`: writes `meta.json`, `files.json` and `nav\`; exit 0, or 2 on failure.
    - `lazymkv-capture eject --drive <letter>`: prints `lock=..., dismount=..., allow-removal=..., eject=...` (each `ok` or `error N`) and exits 0 when the eject step succeeded. Otherwise it prints that report, or `cannot open drive X: (error N)`, or `X: is not an optical drive`, to standard error and exits 2.
  - Scripts: `scripts/box.sh`, `scripts/phase0/job.sh`, `scripts/phase0/wait.sh`, `scripts/phase0/pull-recording.sh <slug>` (Task 8: never deletes on the Mac), `scripts/phase0/setup-check.ps1` (Task 7), `scripts/phase0/compare_scans.py <scan A> <scan B>` (exits 1 when a title is in only one scan or a scan is empty), `scripts/phase0/compare_tracks.py <scan> <title> <mkvmerge json>` (exits 1 when any verdict is no), `parse` from `scripts/phase0/robot.py`.
  - Disc A's recording: `meta.json`, `files.json` and `scan.txt` from Task 9, and `all-tracks/mkvmerge.json` (the direct rip of title N) from Task 11. It is at `C:\lazymkv\phase0\recordings\SLUG_A\` on the mini PC and at `corpus/owner/SLUG_A/` on the Mac.
- Produces:
  - `scripts/phase0/backup-disc.ps1 -Slug <slug> [-Force]`. Run it as a job: `scripts/phase0/job.sh backup-disc.ps1 <log> -Slug <slug> [-Force]`.
    - It refuses before anything is created or deleted (only Assert-DiscIs's own `$P0\tmp\disc-check` is written), when:
      - `-Slug` fails Assert-Slug;
      - `$CAP` or the all-tracks profile is missing;
      - a MakeMKV process (`makemkvcon64`, `makemkvcon` or the GUI `makemkv`) is running;
      - `$Staging\<slug>\backup\` or `$Recordings\<slug>\backup\` is not empty and `-Force` was not given;
      - Assert-DiscIs rejects the disc in the drive, or MakeMKV lists no drive at `$Drive`.
    - Then, with `-Force`, it deletes both folders. In every case it creates both empty with Assert-EmptyDir, the staging folder first, so an unreachable share stops it before anything is created on C:.
    - It runs `lazymkv-capture record --out <rec>\backup.txt -- makemkvcon64 -r --messages=-stdout --progress=-same --cache=1024 backup --decrypt disc:<index> <staging folder>`. `<rec>` is `$Recordings\<slug>\backup`, which is `C:\lazymkv\phase0\recordings\<slug>\backup`.
    - It finds the copy's root by listing the staging folder: the folder itself when it holds `BDMV` or `VIDEO_TS`, or else the one folder below it that does. It writes `<rec>\copy-files.txt` (UTF-8 with a BOM): one line per file of the copy, its size, a space, and its path below the copy's root with `/` separators.
    - After a good copy it scans the copy: `lazymkv-capture record --out <rec>\scan-file.txt -- makemkvcon64 <Get-MkFlags> info file:<copy root>`, the same flags as Disc A's `scan.txt`.
    - It writes `<rec>\backup-summary.json` (UTF-8 with a BOM) whenever the folders were prepared, for a success and for a failure. Its fields are `ok`, `result`, `slug`, `dest`, `started` (the mini PC's clock, `yyyy-MM-dd HH:mm:ss.fff`), `elapsed_s` (the backup call), `exit_code`, `copy_message` (the last `MSG:5070` or `5079` line), `fail_codes` (`5069` if seen), `read_errors` (count of `2003`), `write_errors` (`2019`), `top_level` (names in the staging folder), `copy_root`, `files`, `bytes`, `mb_per_s`, `scan_ok` and `scan_titles`.
    - The copy succeeded (spec 6.3) only if `backup.txt.exit.json` was written, `MSG:5070` or `5079` was seen, `5069` was not, and the copy's root was found and holds data. `5079` (completed with hash failures) is a success with a note. The scan succeeded only if `scan-file.txt.exit.json` was written, the `MSG:1005` banner is there and `TCOUNT` is above 0. MakeMKV's exit code is recorded, never judged.
    - Output lines: `copy: <slug> from disc:<index> to <staging folder>, started <time>`, `copy: finished after <n> s; message: <line>; top level of <staging folder>: <names>; copy root: <path>`, `scan: scan-file.txt exit.json=<json or MISSING> version_banner=<True|False> titles=<n or NO-TCOUNT-LINE>` (only after a good copy), the summary JSON, then `RESULT ok` (with a note after `5079`) or `RESULT FAILED: <reason>`. A reason that begins `the copy succeeded, but` means the copy is good and only its scan failed. After a failure the script throws, so wait.sh exits 1.
  - `scripts/phase0/extract-from-copy.ps1 -Slug <slug> -Title <n> [-Force]`. Run it as a job: `scripts/phase0/job.sh extract-from-copy.ps1 <log> -Slug <slug> -Title <n> [-Force]`. The disc is not used.
    - It refuses before anything is created or deleted when:
      - `-Slug` fails Assert-Slug;
      - `$CAP`, the all-tracks profile or mkvmerge is missing;
      - `$Recordings\<slug>\backup\backup-summary.json` is missing or does not say `"ok": true`;
      - `scan-file.txt` is missing or lists no title `<n>`;
      - a MakeMKV process is running;
      - `$Staging\<slug>\extract\` or `$Recordings\<slug>\extract\` is not empty and `-Force` was not given.
    - Then, with `-Force`, it deletes both folders, creates both empty with Assert-EmptyDir (staging first), and refuses if the copy is no longer at `copy_root`.
    - It runs `lazymkv-capture record --out <rec>\extract.txt -- makemkvcon64 <Get-MkFlags> mkv file:<copy root> <n> <staging folder>`, with `<rec>` = `$Recordings\<slug>\extract`. Then `lazymkv-capture record --out <rec>\mkvmerge.json -- mkvmerge -J <file>`.
    - It writes `<rec>\extract-summary.json` (UTF-8 with a BOM) whenever the folders were prepared. Its fields are `ok`, `result`, `slug`, `title`, `copy_root`, `dest`, `started`, `elapsed_s`, `exit_code`, `files`, `bytes`, `mb_per_s`, `success_message`, `fail_codes`, `profile_errors` (count of `1009`), `read_errors`, `write_errors`, `scan_source_file` (`scan-file.txt`'s `TINFO:<n>,16`), `rip_source_file` (this run's `MSG:3307` for title `<n>`), `mkvmerge_exit` and `tracks`.
    - The extraction succeeded (spec 9.4) only if a success summary (`MSG:5005` or `5036`) for one title was seen, none of `5003`, `5004`, `5037` was seen, there is exactly one non-empty `.mkv` in the staging folder, `rip_source_file` equals `scan_source_file` when both are known, and `mkvmerge -J` exits 0 or 1 and lists at least one track.
    - Output lines: `extract: ...` (start), `extract: <file name> (<bytes> bytes), <n> tracks` (after a success), the summary JSON, then `RESULT ok` (with a note when `MSG:1009` was seen) or `RESULT FAILED: <reason>`. After a failure the script throws.
  - `scripts/phase0/second-drive.ps1 -Slug <slug> -Letter <letter> [-Force]`. Run it over plain ssh: it needs C: and the drive, never the share.
    - It refuses before anything is written when `-Slug` fails Assert-Slug, `-Letter` is not one letter or not an optical drive, `$CAP` is missing, `$Recordings\<slug>\meta.json` is missing, or `$Recordings\<slug>\drive2\` exists and `-Force` was not given.
    - It waits up to 120 s for a readable disc in `<letter>:`. It records MakeMKV's drive listing and runs `lazymkv-capture disc` for `<letter>:\` with the volume label from Windows, all in `$P0\tmp\drive2-<slug>`.
    - Only if MakeMKV lists the drive with state 2 and the disc's fingerprint equals the recording's `meta.json` fingerprint does it move that folder to `$Recordings\<slug>\drive2\`: `drives.txt` with `.timing.jsonl`, `.stderr` and `.exit.json`, and `snapshot\meta.json`, `snapshot\files.json`, `snapshot\nav\`. A failed run leaves the recording unchanged. A disc ID that cannot be read is a finding, not a failure.
    - Output lines: `drive: <letter>: is MakeMKV index <n>, state <s>, name '<drive name>', disc label '<label>'`, `snapshot: label=... fingerprint=... disc_id=... files=...`, `first drive (meta.json): fingerprint=... disc_id=...`, `disc_id: second drive <read|NOT read>; same as the first drive: <True|False|not compared>`, `saved: <folder>`, then `RESULT ok` or `RESULT FAILED: <reason>`. After a failure the script throws, so the ssh command exits 1.
  - The reports `EXP-03-whole-disc-copy.md` and `EXP-07-eject.md`, the second-drive section of `EXP-05-offline-and-unit-key.md`, both drives in `docs/phase0/environment.md`, and the recordings listed under Files.
  - `$Drive` in `C:\lazymkv\phase0\config.ps1` matching the drive left attached for Task 15, confirmed by a setup check.

Placeholders:
- SLUG_A: Disc A's slug, chosen in Task 9 and written in the `Disc(s):` line of EXP-05 (Task 11 calls it SLUG).
- N: Disc A's main-feature title number in `scan.txt`, chosen in Task 11 Step 4 and named in EXP-01's "What was run".
- K: the title number, in the copy's scan, of N's title. The matching script in Step 6 prints it.
- LETTER2: the second drive's letter (one capital letter, no colon), read in Step 9 from MakeMKV's `DRV` line.
- DRIVE2_NAME: the second drive's name as MakeMKV lists it: the `name '...'` part of second-drive.ps1's `drive:` line in Step 9.
- EJECT1 and EJECT2: for drive 1 (Step 8) and drive 2 (Step 9), the eject report line and the `exit=` value.
- TRAY1 and TRAY2: the owner's answer, for each drive, to "did the tray open?".
- DISCID2_STATE: whether Disc A's disc ID was read in drive 2: `read (40 hex characters)`, `not read: Unit_Key_RO.inf listed but unreadable` or `not read: Unit_Key_RO.inf not listed` (Step 11, from Step 9's comparison and its `grep -A1 unit_key`).
- SAME_ID: whether drive 2's disc ID equals drive 1's: `yes`, `no (drive 1 <id>, drive 2 <id>)` or `not compared` when either disc ID is missing (Step 11, from Step 9's comparison).
- KEEP and KEEP_LETTER: the drive that stays attached after this task (`1` or `2`) and its letter, from Step 10.
- SCRATCH: the session's scratch directory, as given in the agent's system prompt; never `/tmp` and never inside the repository.
- Disc A: the UHD film of Task 9. It is in drive 1 (`D:`) from Step 4 until Step 8 ejects it, and in drive 2 in Step 9. In Step 10 the owner takes it out and keeps it at hand until the commit in Step 13; from then on the attached drive is empty. backup-disc.ps1 checks it with Assert-DiscIs before it copies, and second-drive.ps1 checks its fingerprint before it keeps anything.

When `-Force` is right (decision D3):
- `backup-disc.ps1 ... -Force`: only to replace a copy that failed or was interrupted, once its cause has been dealt with and the owner has agreed to another one to two hours of drive time. Such a copy has a `backup-summary.json` that is missing, or that says `"ok": false` with a `result` beginning `FAILED: the copy failed`. Check with `ssh USER@makemkv-box "type C:\lazymkv\phase0\recordings\SLUG_A\backup\backup-summary.json"`.
  - Never use it when the `result` begins `FAILED: the copy succeeded, but`. The copy is good and `-Force` would delete it; stop and report instead.
  - Never use it over a summary that says `"ok": true`.
  - Before using it, pull the recording (`scripts/phase0/pull-recording.sh SLUG_A`) and keep the failed attempt on the Mac with `cp -R corpus/owner/SLUG_A/backup corpus/owner/SLUG_A/backup-failed-1` (`-failed-2` if `-failed-1` already exists, and so on). The copy is committed as evidence of the failure.
- `extract-from-copy.ps1 ... -Force`: only to replace an extraction whose `extract-summary.json` is missing or says `"ok": false`, once its cause has been dealt with. Keep it first the same way, as `corpus/owner/SLUG_A/extract-failed-1`. Never over `"ok": true`.
- `second-drive.ps1 ... -Force`: never in this task. A failed run leaves nothing behind, so a repeat needs no `-Force`.

- [ ] **Step 1: Write the copy script**

Which disc is in the drive: none needed.

`scripts/phase0/backup-disc.ps1`:

```powershell
# Experiment 3 (Task 14): a decrypted whole-disc copy of the disc recorded under -Slug, written to the staging
# share, and MakeMKV's scan of that copy (spec 6.3 steps 6 and 8).
# The copy goes to $Staging\<Slug>\backup\. The evidence goes to $Recordings\<Slug>\backup\
# (C:\lazymkv\phase0\recordings\<Slug>\backup\):
#   backup.txt with .timing.jsonl, .stderr and .exit.json      MakeMKV's backup run, recorded by lazymkv-capture
#   copy-files.txt       one line per file of the copy: its size, a space, its path below the copy's root
#   scan-file.txt with .timing.jsonl, .stderr and .exit.json   MakeMKV's scan of the copy, with Get-MkFlags
#   backup-summary.json  timing, size, messages, the copy's layout and the verdict
# If the staging folder or the evidence folder is not empty the run is refused, unless -Force is given:
# -Force deletes both, but only after Assert-DiscIs has accepted the disc.
# The copy succeeded (spec 6.3) when MakeMKV reported MSG:5070, or 5079 (completed with hash failures), and no
# 5069, and the copy holds a BDMV or VIDEO_TS folder with data in it. The copy's root is found by listing the
# folder that was empty before the call: the folder itself, or the one folder below it that holds BDMV or VIDEO_TS.
# The scan succeeded when it finished, printed the MSG:1005 banner and lists at least one title.
# MakeMKV's exit code is recorded, never judged (spec 9.2). MakeMKV text never builds a path.
# The last line is "RESULT ok" or "RESULT FAILED: <reason>"; after a failure the script throws.
param(
    [Parameter(Mandatory)][string]$Slug,
    [switch]$Force
)
. C:\lazymkv\src\scripts\phase0\env.ps1

$failure = $null
$note = ''
$summary = $null
$rec = $null
try {
    Assert-Slug $Slug
    if (-not (Test-Path -LiteralPath $CAP)) { throw "missing $CAP; build it with: scripts/box.sh cargo build --release -p lazymkv-capture" }
    if (-not (Test-Path -LiteralPath $AllTracksProfile)) { throw "missing job profile $AllTracksProfile" }
    $rec = "$Recordings\$Slug\backup"
    $dest = "$Staging\$Slug\backup"

    # Only one MakeMKV may use the drive (spec 10.4); the GUI holds it too.
    $running = @(Get-Process -Name makemkvcon64, makemkvcon, makemkv -ErrorAction SilentlyContinue)
    if ($running.Count -gt 0) { throw "MakeMKV is still running ($($running[0].Name).exe, process id $($running[0].Id)); it must end before the copy starts" }

    # A copy and its evidence are never replaced without -Force.
    foreach ($dir in $dest, $rec) {
        if (-not $Force -and (Test-Path -LiteralPath $dir) -and @(Get-ChildItem -LiteralPath $dir -Force -ErrorAction Stop).Count -gt 0) {
            throw "$dir is not empty: a copy of $Slug was made before and is kept; use -Force only where the plan says so"
        }
    }
    # The disc must be the one recorded under this slug; checked before anything is deleted.
    Assert-DiscIs $Slug
    $index = Get-DiscIndex
    # The staging folder first: if the share cannot be reached, nothing is created on C:.
    foreach ($dir in $dest, $rec) {
        if ($Force -and (Test-Path -LiteralPath $dir)) { Remove-Item -LiteralPath $dir -Recurse -Force -ErrorAction Stop }
        Assert-EmptyDir $dir
    }

    $summary = [ordered]@{
        ok = $false; result = $null; slug = $Slug; dest = $dest; started = $null; elapsed_s = $null; exit_code = $null
        copy_message = $null; fail_codes = @(); read_errors = $null; write_errors = $null
        top_level = @(); copy_root = $null; files = $null; bytes = $null; mb_per_s = $null
        scan_ok = $false; scan_titles = $null
    }
    $start = Get-Date
    $summary['started'] = $start.ToString('yyyy-MM-dd HH:mm:ss.fff')
    "copy: $Slug from disc:$index to $dest, started $($summary['started'])"
    & $CAP record --out "$rec\backup.txt" -- $MK -r --messages=-stdout --progress=-same --cache=1024 backup --decrypt "disc:$index" $dest | Out-Null
    $summary['exit_code'] = $LASTEXITCODE
    $elapsed = ((Get-Date) - $start).TotalSeconds
    $summary['elapsed_s'] = [int]$elapsed

    # The copy's verdict from MakeMKV's messages (spec 6.3).
    $codes = @()
    $doneLine = $null
    if (Test-Path -LiteralPath "$rec\backup.txt") {
        foreach ($line in [IO.File]::ReadAllText("$rec\backup.txt").Split("`n")) {
            if (-not ($line -match '^MSG:(\d+),')) { continue }
            $code = [int]$Matches[1]
            $codes += $code
            if ($code -eq 5070 -or $code -eq 5079) { $doneLine = $line.Trim() }
        }
    }
    $summary['copy_message'] = $doneLine
    $summary['fail_codes'] = @($codes | Where-Object { $_ -eq 5069 } | Sort-Object -Unique)
    $summary['read_errors'] = @($codes | Where-Object { $_ -eq 2003 }).Count
    $summary['write_errors'] = @($codes | Where-Object { $_ -eq 2019 }).Count

    # The copy's root is found by listing the folder that was empty before the call.
    $top = @(Get-ChildItem -LiteralPath $dest -Force -ErrorAction SilentlyContinue)
    $summary['top_level'] = @($top | ForEach-Object { $_.Name })
    $root = $null
    if ((Test-Path -LiteralPath "$dest\BDMV") -or (Test-Path -LiteralPath "$dest\VIDEO_TS")) {
        $root = $dest
    } else {
        $inner = @($top | Where-Object { $_.PSIsContainer -and ((Test-Path -LiteralPath "$($_.FullName)\BDMV") -or (Test-Path -LiteralPath "$($_.FullName)\VIDEO_TS")) })
        if ($inner.Count -eq 1) { $root = $inner[0].FullName }
    }
    $summary['copy_root'] = $root
    "copy: finished after $($summary['elapsed_s']) s; message: $doneLine; top level of ${dest}: $($summary['top_level'] -join ', '); copy root: $root"

    if ($root) {
        # No -ErrorAction Stop on the listing: a hiccup after a long copy must not hide the verdict below.
        $base = $root.TrimEnd('\')
        $files = @(Get-ChildItem -LiteralPath $root -Recurse -File -Force -ErrorAction SilentlyContinue | Sort-Object -Property FullName)
        $lines = @($files | ForEach-Object { '{0} {1}' -f $_.Length, $_.FullName.Substring($base.Length).TrimStart('\').Replace('\', '/') })
        if ($lines.Count -gt 0) { Set-Content -LiteralPath "$rec\copy-files.txt" -Value $lines -Encoding UTF8 -ErrorAction Stop }
        $bytes = [int64](($files | Measure-Object -Property Length -Sum).Sum)
        $summary['files'] = $files.Count
        $summary['bytes'] = $bytes
        if ($elapsed -gt 0 -and $bytes -gt 0) { $summary['mb_per_s'] = [math]::Round($bytes / 1MB / $elapsed, 1) }
    }

    $problems = @()
    if (-not (Test-Path -LiteralPath "$rec\backup.txt.exit.json")) { $problems += 'lazymkv-capture wrote no backup.txt.exit.json' }
    if (-not $doneLine) { $problems += 'no completion message (MSG:5070 or 5079)' }
    if ($summary['fail_codes'].Count -gt 0) { $problems += 'MSG:5069 (backup failed)' }
    if (-not $root) { $problems += "no BDMV or VIDEO_TS folder in $dest, nor in exactly one folder below it" } elseif (-not ($summary['bytes'] -gt 0)) { $problems += "the copy in $root holds no data" }
    if ($problems.Count -gt 0) { throw "the copy failed: $($problems -join '; ')" }
    if ($codes -contains 5079) { $note = ' (MSG:5079: the copy completed with hash failures)' }

    # The copy's titles, with the same flags as the disc-time scan (spec 9.2).
    $flags = Get-MkFlags
    & $CAP record --out "$rec\scan-file.txt" -- $MK @flags info "file:$root" | Out-Null
    $exitJson = [string](Get-Content -LiteralPath "$rec\scan-file.txt.exit.json" -Raw -ErrorAction SilentlyContinue)
    $raw = [string](Get-Content -LiteralPath "$rec\scan-file.txt" -Raw -ErrorAction SilentlyContinue)
    $banner = [regex]::IsMatch($raw, '(?m)^MSG:1005,')
    $count = [regex]::Match($raw, '(?m)^TCOUNT:(\d+)')
    $titles = if ($count.Success) { $count.Groups[1].Value } else { 'NO-TCOUNT-LINE' }
    if ($count.Success) { $summary['scan_titles'] = [int]$titles }
    "scan: scan-file.txt exit.json=$(if ($exitJson) { $exitJson.Trim() } else { 'MISSING' }) version_banner=$banner titles=$titles"
    if (-not $exitJson) { throw 'the copy succeeded, but its scan did not finish: scan-file.txt.exit.json was not written' }
    if (-not $banner) { throw 'the copy succeeded, but MakeMKV did not run its scan: no MSG:1005 version banner in scan-file.txt' }
    if (-not ($count.Success -and [int]$titles -gt 0)) { throw "the copy succeeded, but its scan lists no titles (titles=$titles)" }
    $summary['scan_ok'] = $true
} catch {
    $failure = $_.Exception.Message
}

if ($null -ne $summary) {
    $summary['ok'] = -not $failure
    $summary['result'] = 'ok'
    if ($failure) { $summary['result'] = "FAILED: $failure" }
    $json = $summary | ConvertTo-Json
    try { Set-Content -LiteralPath "$rec\backup-summary.json" -Value $json -Encoding UTF8 -ErrorAction Stop } catch { if (-not $failure) { $failure = "cannot write backup-summary.json: $($_.Exception.Message)" } }
    $json
}
if ($failure) {
    "RESULT FAILED: $failure"
    throw "backup-disc.ps1: $failure"
}
"RESULT ok$note"
```

The script does not set `$ErrorActionPreference = 'Stop'`: inside a job, that would turn any line a native program writes to standard error into a failure. Each cmdlet that must not fail silently has `-ErrorAction Stop` instead.

Run: `LC_ALL=C grep -n '[^ -~]' scripts/phase0/backup-disc.ps1`
Expected: no output. The file must be plain ASCII, because PowerShell 5.1 reads a file without a BOM in the ANSI code page.

- [ ] **Step 2: Write the extraction script**

Which disc is in the drive: none needed.

`scripts/phase0/extract-from-copy.ps1`:

```powershell
# Experiment 3 (Task 14): extracts one title from the whole-disc copy that backup-disc.ps1 made and judged good
# (spec 6.3 step 8), and identifies the file. The disc is not used.
# The copy is read at the copy_root recorded in $Recordings\<Slug>\backup\backup-summary.json, which
# backup-disc.ps1 found by listing the copy. -Title is a title number of the copy's scan,
# $Recordings\<Slug>\backup\scan-file.txt. The file goes to $Staging\<Slug>\extract\; the evidence goes to
# $Recordings\<Slug>\extract\ (C:\lazymkv\phase0\recordings\<Slug>\extract\):
#   extract.txt with .timing.jsonl, .stderr and .exit.json     MakeMKV's run: mkv file:<copy root> <Title> <dir>
#   mkvmerge.json with .timing.jsonl, .stderr and .exit.json   mkvmerge -J of the extracted file
#   extract-summary.json   timing, size, messages and the verdict
# If either folder is not empty the run is refused, unless -Force is given, which deletes both first.
# Success (spec 9.4): a success summary (MSG:5005 or 5036) for one title; none of 5003, 5004 or 5037; exactly one
# non-empty .mkv in the staging folder; where both name one, the same source file for the title in this run
# (MSG:3307) as in scan-file.txt (TINFO attribute 16); and mkvmerge -J exits 0 or 1 and lists at least one track.
# MakeMKV's exit code is recorded, never judged (spec 9.2). MakeMKV text never builds a path.
# The last line is "RESULT ok" or "RESULT FAILED: <reason>"; after a failure the script throws.
param(
    [Parameter(Mandatory)][string]$Slug,
    [Parameter(Mandatory)][int]$Title,
    [switch]$Force
)
. C:\lazymkv\src\scripts\phase0\env.ps1

$failure = $null
$note = ''
$summary = $null
$rec = $null
try {
    Assert-Slug $Slug
    if (-not (Test-Path -LiteralPath $CAP)) { throw "missing $CAP; build it with: scripts/box.sh cargo build --release -p lazymkv-capture" }
    if (-not (Test-Path -LiteralPath $AllTracksProfile)) { throw "missing job profile $AllTracksProfile" }
    if (-not $MKVMERGE) { throw 'mkvmerge.exe not found' }
    $backup = "$Recordings\$Slug\backup"
    $rec = "$Recordings\$Slug\extract"
    $dest = "$Staging\$Slug\extract"

    # Only a copy that backup-disc.ps1 judged good is used, at the root it found by listing the copy.
    $copyFile = "$backup\backup-summary.json"
    if (-not (Test-Path -LiteralPath $copyFile)) { throw "no $copyFile; make the copy with backup-disc.ps1 first" }
    $copy = Get-Content -LiteralPath $copyFile -Raw -Encoding UTF8 -ErrorAction Stop | ConvertFrom-Json
    if ($copy.ok -ne $true -or -not $copy.copy_root) { throw "the copy of $Slug did not succeed ($($copy.result)); only a copy whose backup-summary.json says ok is used" }
    $root = [string]$copy.copy_root

    # The title must be in the copy's scan, made with the same flags; its source file is compared below.
    $scanFile = "$backup\scan-file.txt"
    if (-not (Test-Path -LiteralPath $scanFile)) { throw "no $scanFile" }
    $scanText = [IO.File]::ReadAllText($scanFile)
    if (-not [regex]::IsMatch($scanText, '(?m)^TINFO:' + $Title + ',')) { throw "scan-file.txt lists no title $Title" }
    $found = [regex]::Match($scanText, '(?m)^TINFO:' + $Title + ',16,\d+,"(.*)"\r?$')
    $scanSource = $null
    if ($found.Success) { $scanSource = $found.Groups[1].Value }

    # Only one MakeMKV at a time (spec 10.4).
    $running = @(Get-Process -Name makemkvcon64, makemkvcon, makemkv -ErrorAction SilentlyContinue)
    if ($running.Count -gt 0) { throw "MakeMKV is still running ($($running[0].Name).exe, process id $($running[0].Id)); it must end before the extraction starts" }

    # An extraction and its evidence are never replaced without -Force.
    foreach ($dir in $dest, $rec) {
        if (-not $Force -and (Test-Path -LiteralPath $dir) -and @(Get-ChildItem -LiteralPath $dir -Force -ErrorAction Stop).Count -gt 0) {
            throw "$dir is not empty: an extraction was made before and is kept; use -Force only where the plan says so"
        }
    }
    # The staging folder first: if the share cannot be reached, nothing is created on C:.
    foreach ($dir in $dest, $rec) {
        if ($Force -and (Test-Path -LiteralPath $dir)) { Remove-Item -LiteralPath $dir -Recurse -Force -ErrorAction Stop }
        Assert-EmptyDir $dir
    }
    if (-not (Test-Path -LiteralPath $root)) { throw "the copy is no longer at $root" }

    $summary = [ordered]@{
        ok = $false; result = $null; slug = $Slug; title = $Title; copy_root = $root; dest = $dest
        started = $null; elapsed_s = $null; exit_code = $null; files = @(); bytes = $null; mb_per_s = $null
        success_message = $null; fail_codes = @(); profile_errors = $null; read_errors = $null; write_errors = $null
        scan_source_file = $scanSource; rip_source_file = $null; mkvmerge_exit = $null; tracks = $null
    }
    $flags = Get-MkFlags
    $start = Get-Date
    $summary['started'] = $start.ToString('yyyy-MM-dd HH:mm:ss.fff')
    "extract: $Slug title $Title (source file in scan-file.txt: $scanSource) from $root to $dest, started $($summary['started'])"
    & $CAP record --out "$rec\extract.txt" -- $MK @flags mkv "file:$root" $Title $dest | Out-Null
    $summary['exit_code'] = $LASTEXITCODE
    $elapsed = ((Get-Date) - $start).TotalSeconds
    $summary['elapsed_s'] = [int]$elapsed

    $codes = @()
    $successLine = $null
    $savedCount = $null
    if (Test-Path -LiteralPath "$rec\extract.txt") {
        foreach ($line in [IO.File]::ReadAllText("$rec\extract.txt").Split("`n")) {
            if (-not ($line -match '^MSG:(\d+),')) { continue }
            $code = [int]$Matches[1]
            $codes += $code
            if ($code -eq 5005 -or $code -eq 5036) {
                $successLine = $line.Trim()
                $savedCount = $null
                if ($line -match ',"(\d+)"\s*$') { $savedCount = [int]$Matches[1] }
            }
            if ($code -eq 3307 -and $line -match ',"([^"]*)","(\d+)"\s*$' -and [int]$Matches[2] -eq $Title) {
                $summary['rip_source_file'] = $Matches[1]
            }
        }
    }
    $failCodes = @($codes | Where-Object { $_ -eq 5003 -or $_ -eq 5004 -or $_ -eq 5037 } | Sort-Object -Unique)
    $summary['fail_codes'] = $failCodes
    $summary['success_message'] = $successLine
    $summary['profile_errors'] = @($codes | Where-Object { $_ -eq 1009 }).Count
    $summary['read_errors'] = @($codes | Where-Object { $_ -eq 2003 }).Count
    $summary['write_errors'] = @($codes | Where-Object { $_ -eq 2019 }).Count

    # The output is found by listing the folder that was empty before the call.
    $files = @(Get-ChildItem -LiteralPath $dest -File -Force -ErrorAction SilentlyContinue)
    $mkvs = @($files | Where-Object { $_.Extension -eq '.mkv' })
    $bytes = [int64](($mkvs | Measure-Object -Property Length -Sum).Sum)
    $summary['files'] = @($files | ForEach-Object { $_.Name })
    $summary['bytes'] = $bytes
    if ($elapsed -gt 0 -and $bytes -gt 0) { $summary['mb_per_s'] = [math]::Round($bytes / 1MB / $elapsed, 1) }

    $problems = @()
    if (-not (Test-Path -LiteralPath "$rec\extract.txt.exit.json")) { $problems += 'lazymkv-capture wrote no extract.txt.exit.json' }
    if (-not $successLine) { $problems += 'no success summary (MSG:5005 or 5036)' } elseif ($null -ne $savedCount -and $savedCount -ne 1) { $problems += "the success summary reports $savedCount titles" }
    if ($failCodes.Count -gt 0) { $problems += "failure messages $($failCodes -join ',')" }
    if ($mkvs.Count -ne 1) { $problems += "$($mkvs.Count) .mkv files in $dest" } elseif ($mkvs[0].Length -eq 0) { $problems += "$($mkvs[0].Name) is empty" }
    $ripSource = $summary['rip_source_file']
    if ($scanSource -and $ripSource -and $ripSource -ne $scanSource) { $problems += "title $Title is $ripSource in this run but $scanSource in scan-file.txt" }
    if ($problems.Count -gt 0) { throw "the extraction failed: $($problems -join '; ')" }
    if ($summary['profile_errors'] -gt 0) { $note = " (MSG:1009 seen $($summary['profile_errors']) times: MakeMKV rejected the profile; see extract.txt)" }

    # Identification: a header read. mkvmerge exits 1 for warnings only.
    & $CAP record --out "$rec\mkvmerge.json" -- $MKVMERGE -J $mkvs[0].FullName | Out-Null
    $mkvmergeExit = $LASTEXITCODE
    $summary['mkvmerge_exit'] = $mkvmergeExit
    if ($mkvmergeExit -gt 1) { throw "mkvmerge -J exited $mkvmergeExit; see mkvmerge.json" }
    $ident = Get-Content -LiteralPath "$rec\mkvmerge.json" -Raw -Encoding UTF8 -ErrorAction Stop | ConvertFrom-Json
    $summary['tracks'] = @($ident.tracks | Where-Object { $_ }).Count
    if ($summary['tracks'] -eq 0) { throw 'mkvmerge -J lists no tracks; see mkvmerge.json' }
    "extract: $($mkvs[0].Name) ($bytes bytes), $($summary['tracks']) tracks"
} catch {
    $failure = $_.Exception.Message
}

if ($null -ne $summary) {
    $summary['ok'] = -not $failure
    $summary['result'] = 'ok'
    if ($failure) { $summary['result'] = "FAILED: $failure" }
    $json = $summary | ConvertTo-Json
    try { Set-Content -LiteralPath "$rec\extract-summary.json" -Value $json -Encoding UTF8 -ErrorAction Stop } catch { if (-not $failure) { $failure = "cannot write extract-summary.json: $($_.Exception.Message)" } }
    $json
}
if ($failure) {
    "RESULT FAILED: $failure"
    throw "extract-from-copy.ps1: $failure"
}
"RESULT ok$note"
```

Run: `LC_ALL=C grep -n '[^ -~]' scripts/phase0/extract-from-copy.ps1`
Expected: no output.

- [ ] **Step 3: Write the second-drive script**

Which disc is in the drive: none needed.

`scripts/phase0/second-drive.ps1`:

```powershell
# Experiments 5 and 7 (Task 14): the disc recorded under -Slug, read in another drive (the second drive) at
# -Letter. Run it over plain ssh: it needs C: and the drive, never the share. It writes
# $Recordings\<Slug>\drive2\ (C:\lazymkv\phase0\recordings\<Slug>\drive2\):
#   drives.txt with .timing.jsonl, .stderr and .exit.json   MakeMKV's drive listing (info disc:9999)
#   snapshot\meta.json, snapshot\files.json, snapshot\nav\  lazymkv-capture disc for <Letter>:\ (fingerprint, disc ID)
# Both are made in $P0\tmp\drive2-<Slug> first and moved into the recording only when the disc in <Letter>: has
# the recording's fingerprint, so a failed run leaves the recording unchanged. Assert-DiscIs is not used because
# it reads the drive in $Drive, not <Letter>:; the same fingerprint check is made here. A disc ID that cannot be
# read is a finding, not a failure. An existing drive2 folder is refused unless -Force is given.
# The last line is "RESULT ok" or "RESULT FAILED: <reason>"; after a failure the script throws.
param(
    [Parameter(Mandatory)][string]$Slug,
    [Parameter(Mandatory)][string]$Letter,
    [switch]$Force
)
. C:\lazymkv\src\scripts\phase0\env.ps1

try {
    Assert-Slug $Slug
    if ($Letter -notmatch '^[A-Za-z]$') { throw "bad drive letter: '$Letter' (one letter, without a colon)" }
    $Letter = $Letter.ToUpper()
    $type = $null
    try { $type = [string]([System.IO.DriveInfo]::new($Letter).DriveType) } catch { }
    if ($type -ne 'CDRom') { throw "${Letter}: is not an optical drive (drive type: $type)" }
    if (-not (Test-Path -LiteralPath $CAP)) { throw "missing $CAP; build it with: scripts/box.sh cargo build --release -p lazymkv-capture" }
    $metaFile = "$Recordings\$Slug\meta.json"
    if (-not (Test-Path -LiteralPath $metaFile)) { throw "no $metaFile; the disc must be recorded first" }
    $first = Get-Content -LiteralPath $metaFile -Raw -Encoding UTF8 -ErrorAction Stop | ConvertFrom-Json
    if (-not $first.fingerprint) { throw "$metaFile has no fingerprint" }
    $out = "$Recordings\$Slug\drive2"
    if ((Test-Path -LiteralPath $out) -and -not $Force) { throw "$out already exists and is kept as evidence; use -Force only where the plan says so" }
    $stage = "$P0\tmp\drive2-$Slug"
    if (Test-Path -LiteralPath $stage) { Remove-Item -LiteralPath $stage -Recurse -Force -ErrorAction Stop }
    Assert-EmptyDir $stage

    # The disc may still be spinning up: wait up to two minutes until its root can be listed.
    $ready = $false
    for ($i = 0; $i -lt 24 -and -not $ready; $i++) {
        if (@(Get-ChildItem -LiteralPath "${Letter}:\" -ErrorAction SilentlyContinue).Count -gt 0) { $ready = $true } else { Start-Sleep -Seconds 5 }
    }
    if (-not $ready) { throw "no readable disc in ${Letter}: after 120 s" }

    # MakeMKV's view of the drive: its index, its name (the model) and its state.
    & $CAP record --out "$stage\drives.txt" -- $MK -r --cache=1 info disc:9999 | Out-Null
    $raw = [string](Get-Content -LiteralPath "$stage\drives.txt" -Raw -ErrorAction SilentlyContinue)
    $drv = [regex]::Match($raw, '(?m)^DRV:(\d+),(\d+),\d+,\d+,"([^"]*)","([^"]*)","' + [regex]::Escape("${Letter}:") + '"\s*$')
    if (-not $drv.Success) { throw "MakeMKV lists no drive at ${Letter}: (see $stage\drives.txt)" }
    "drive: ${Letter}: is MakeMKV index $($drv.Groups[1].Value), state $($drv.Groups[2].Value), name '$($drv.Groups[3].Value)', disc label '$($drv.Groups[4].Value)'"
    if ($drv.Groups[2].Value -ne '2') { throw "MakeMKV does not list a disc in ${Letter}: (DRV state $($drv.Groups[2].Value); 2 means a disc is inserted)" }

    # The snapshot: the fingerprint and the disc ID as this drive presents the disc.
    $label = $null
    try { $label = ([System.IO.DriveInfo]::new($Letter)).VolumeLabel } catch { }
    if (-not $label) { $label = '(none)' }
    & $CAP disc --root "${Letter}:\" --label $label --out "$stage\snapshot" | Out-Null
    $capExit = $LASTEXITCODE
    if ($capExit -ne 0 -or -not (Test-Path -LiteralPath "$stage\snapshot\meta.json")) { throw "lazymkv-capture disc failed on ${Letter}: (exit $capExit)" }
    $meta = Get-Content -LiteralPath "$stage\snapshot\meta.json" -Raw -Encoding UTF8 -ErrorAction Stop | ConvertFrom-Json
    "snapshot: label=$label fingerprint=$($meta.fingerprint) disc_id=$($meta.disc_id) files=$($meta.file_count)"
    "first drive (meta.json): fingerprint=$($first.fingerprint) disc_id=$($first.disc_id)"
    if (-not $meta.fingerprint -or $meta.fingerprint -ne $first.fingerprint) {
        throw "the disc in ${Letter}: is not the disc of recording $Slug (fingerprint $($meta.fingerprint), recorded $($first.fingerprint))"
    }
    $sameId = 'not compared'
    if ($meta.disc_id -and $first.disc_id) { $sameId = [string]($meta.disc_id -eq $first.disc_id) }
    "disc_id: second drive $(if ($meta.disc_id) { 'read' } else { 'NOT read' }); same as the first drive: $sameId"

    if (Test-Path -LiteralPath $out) { Remove-Item -LiteralPath $out -Recurse -Force -ErrorAction Stop }
    Move-Item -LiteralPath $stage -Destination $out -ErrorAction Stop
    "saved: $out"
    'RESULT ok'
} catch {
    "RESULT FAILED: $($_.Exception.Message)"
    throw
}
```

Run: `LC_ALL=C grep -n '[^ -~]' scripts/phase0/second-drive.ps1`
Expected: no output.

- [ ] **Step 4: Check the inputs, sync and try the scripts without copying**

Which disc is in the drive: Disc A (SLUG_A) in drive 1 (`D:`), as Task 13 left it. If it is not in, ask the owner to insert it and to say when the drive's light has stopped blinking. The `backup-disc.ps1` run below reads its fingerprint through Assert-DiscIs. Nothing is copied in this step.

```bash
ls corpus/owner/SLUG_A/meta.json corpus/owner/SLUG_A/files.json corpus/owner/SLUG_A/scan.txt corpus/owner/SLUG_A/all-tracks/mkvmerge.json
grep -E '^(function (Assert-Slug|Assert-DiscIs|Assert-EmptyDir|Get-InsertedDisc)|\$Recordings)' scripts/phase0/env.ps1
LC_ALL=C grep -n '[^ -~]' scripts/phase0/backup-disc.ps1 scripts/phase0/extract-from-copy.ps1 scripts/phase0/second-drive.ps1
scripts/box.sh
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -Command \"(Get-Command C:\lazymkv\src\scripts\phase0\backup-disc.ps1).Parameters.Keys\""
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -Command \"(Get-Command C:\lazymkv\src\scripts\phase0\extract-from-copy.ps1).Parameters.Keys\""
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -Command \"(Get-Command C:\lazymkv\src\scripts\phase0\second-drive.ps1).Parameters.Keys\""
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\backup-disc.ps1 -Slug Bad_Slug"; echo "exit=$?"
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\extract-from-copy.ps1 -Slug Bad_Slug -Title 0"; echo "exit=$?"
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\second-drive.ps1 -Slug Bad_Slug -Letter D"; echo "exit=$?"
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\second-drive.ps1 -Slug SLUG_A -Letter C"; echo "exit=$?"
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\extract-from-copy.ps1 -Slug SLUG_A -Title 0"; echo "exit=$?"
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\backup-disc.ps1 -Slug SLUG_A"; echo "exit=$?"
ssh USER@makemkv-box "if exist C:\lazymkv\phase0\recordings\SLUG_A\backup (echo CREATED) else (echo absent)"
ssh USER@makemkv-box "tasklist | findstr /i makemkv"
```

Expected, in order:
- `ls` lists the four files. A missing one means Task 9 or Task 11 is not finished: stop and report.
- `grep` prints five lines: the `$Recordings` assignment and the four D1 helper functions. With fewer, the env.ps1 prerequisites are missing: stop and report.
- The ASCII check prints nothing.
- `box.sh` syncs the tree and exits 0.
- The three parameter lists begin `Slug`, `Force`; `Slug`, `Title`, `Force`; and `Slug`, `Letter`, `Force`. PowerShell's common parameters follow. A parse error is a defect in Steps 1 to 3: fix it before going on.
- The three `Bad_Slug` runs each print `RESULT FAILED:` with Assert-Slug's rejection of `Bad_Slug` within seconds, then `exit=1`. If a message says that `Assert-Slug` is not recognized, the env.ps1 prerequisites are missing.
- The `-Letter C` run prints `RESULT FAILED: C: is not an optical drive (drive type: Fixed)` and `exit=1`. Nothing was written.
- The `extract-from-copy.ps1` run prints `RESULT FAILED: no C:\lazymkv\phase0\recordings\SLUG_A\backup\backup-summary.json; make the copy with backup-disc.ps1 first` and `exit=1`.
- The `backup-disc.ps1 -Slug SLUG_A` run prints `RESULT FAILED:` within a minute, then `exit=1`. Its message is the error from creating the staging folder under `\\unas-pro\media\lazymkv-staging\SLUG_A`, typically `Access is denied` or `Access to the path ... is denied`. Over plain ssh the share gives "Access is denied" (it is reachable only from a job, as `docs/phase0/environment.md` notes), so this proves three things: the script parses, Assert-DiscIs accepts Disc A, and an unreachable share stops the script before MakeMKV starts. Other results:
  - Assert-DiscIs's message: Disc A is not in the drive. Ask the owner to insert it, then run the line again.
  - `MakeMKV is still running`: ask the owner to close the MakeMKV window, or end the process with `ssh USER@makemkv-box "taskkill /im makemkvcon64.exe /f"`, then run the line again.
  - `is not empty`: a copy was started before. Stop and report.
  - Anything else, or a run that goes on for minutes (a copy started): stop and report.
- The `if exist` line prints `absent`: nothing was created on C:.
- `tasklist | findstr` prints nothing.

- [ ] **Step 5: Copy Disc A to the share and scan the copy**

Which disc is in the drive: Disc A (SLUG_A) in drive 1 (`D:`). backup-disc.ps1 checks this itself with Assert-DiscIs before MakeMKV starts. Disc A stays in until Step 8, in case the copy has to be repeated.

Ask the owner: "Please make sure Disc A is in the drive and that you stay logged in at the mini PC. The copy reads the whole disc and takes one to two hours." Then check that the share is reachable and has room for the copy and, later, the extracted feature:

```bash
python3 -c "import json,sys; m=json.load(open('corpus/owner/'+sys.argv[1]+'/meta.json', encoding='utf-8-sig')); print('disc total_bytes', m['total_bytes'], '=', round(m['total_bytes'] / 2**30, 1), 'GB as setup-check counts it')" SLUG_A
scripts/phase0/job.sh setup-check.ps1 setup-check-task14
scripts/phase0/wait.sh setup-check-task14
```

What wait.sh's exit codes mean for the setup-check job:
- **0**: read the log. It must contain `staging    : \\unas-pro\media\lazymkv-staging : write, rename and delete OK`. Its `staging free:` value must be at least twice the disc's size printed above. If the staging line says `FAILED`, or there is too little room, tell the owner, and start no copy until a new run of this job (same command) passes.
- **1**: setup-check.ps1 threw (env.ps1 or config.ps1 is broken). Stop and report with the `ERROR:` line.
- **3**: still running (it normally takes under a minute). Run wait.sh again.
- **4**: the job is not running and wrote no DONE line. Stop and report.
- **5**: the mini PC cannot be reached. Run wait.sh again once it can.

Then start the copy:

```bash
scripts/phase0/job.sh backup-disc.ps1 backup-copy -Slug SLUG_A
scripts/phase0/wait.sh backup-copy
```

job.sh prints `started backup-disc.ps1; log: C:\lazymkv\phase0\logs\backup-copy.txt`. If it exits 1 because a phase 0 job is still running, wait for that job with its own wait.sh line first.

To see progress at any time (the recording is on C:, so plain ssh can read it):

```bash
ssh USER@makemkv-box "powershell -NoProfile -Command \"Get-Date -Format 'yyyy-MM-dd HH:mm:ss.fff'; Get-Content C:\lazymkv\phase0\recordings\SLUG_A\backup\backup.txt -Tail 2; Get-Content C:\lazymkv\phase0\recordings\SLUG_A\backup\scan-file.txt -Tail 2 -ErrorAction SilentlyContinue\""
```

It prints the mini PC's time and the last two lines of the copy (`PRGV:current,total,max` while MakeMKV works). Once the copy has finished, the last two lines of the copy's scan follow.

What wait.sh's exit codes mean for this job:
- **0**: check the log against the Expected below.
- **1**: read the `RESULT FAILED:` line.
  - A reason from the checks before MakeMKV starts means nothing was copied. These are: Assert-Slug, a missing `lazymkv-capture.exe` or job profile, `MakeMKV is still running`, `is not empty`, Assert-DiscIs, `MakeMKV lists no drive`, or an error creating a folder. Fix the cause as in Step 4, or run the setup-check job again for a folder on the share. Then start the job again under the log name `backup-copy-b`.
  - `the copy failed: ...`: the copy ran and failed. Pull the recording (`scripts/phase0/pull-recording.sh SLUG_A`), read `grep -E '^MSG:' corpus/owner/SLUG_A/backup/backup.txt | cut -c1-160`, and stop and report, with the `read_errors` and `write_errors` counts from the summary. If the copy failed within a few minutes with a message about the destination folder, say so: whether `backup` accepts an existing empty folder was not known beforehand. A repeat is decided with the owner, for example after cleaning the disc or after the share check passes again. It is the same job command with `-Force` added, under the log name `backup-copy-b`, after the failed attempt has been kept as "When -Force is right" says. If no repeat is made, experiment 3 is answered no: skip Step 6 and go to Step 7.
  - `the copy succeeded, but ...`: the copy is good and only its scan failed. Pull the recording and read `grep -E '^MSG:' corpus/owner/SLUG_A/backup/scan-file.txt | cut -c1-160`. Stop and report. Do not use `-Force`: it would delete the copy.
- **3**: still running. This is normal for one to two hours, plus several minutes for the scan of the copy. Run `scripts/phase0/wait.sh backup-copy` again, and start nothing else meanwhile.
  - Run the progress command now and then. MakeMKV is stuck if two progress checks 15 minutes apart show the same last lines, or if four hours pass without a DONE line.
  - Then tell the owner and end MakeMKV, not the job: `ssh USER@makemkv-box "taskkill /im makemkvcon64.exe /f"`. The script then writes its summary and fails, and wait.sh exits 1. Stop and report.
  - Do not use Stop-ScheduledTask here: it would lose the summary.
- **4**: the job ended without a DONE line (it never started, or was killed). Check `ssh USER@makemkv-box "tasklist | findstr /i makemkvcon"`, then stop and report.
- **5**: the mini PC cannot be reached. The job keeps running there; run wait.sh again once it can be reached.

Expected (exit 0), in the log, in this order (lines printed by the env.ps1 helpers may come first):

```text
copy: SLUG_A from disc:<index> to \\unas-pro\media\lazymkv-staging\SLUG_A\backup, started <yyyy-MM-dd HH:mm:ss.fff>
copy: finished after <n> s; message: MSG:5070,...; top level of \\unas-pro\media\lazymkv-staging\SLUG_A\backup: <names>; copy root: <path>
scan: scan-file.txt exit.json={"exit_code":<n>,"elapsed_ms":<n>,"lines":<n>} version_banner=True titles=<n above 0>
<the summary JSON>
RESULT ok
PHASE0-DONE status=ok exit=<n>
```

`exit=<n>` on the DONE line is only the last native command's exit code, not a verdict. In the JSON:
- `ok` is true, and `scan_ok` is true with `scan_titles` above 0;
- `copy_message` begins `MSG:5070` (or `MSG:5079`, which `RESULT ok (MSG:5079: ...)` also says), and `fail_codes` is an empty list;
- `top_level` is a JSON list (`[ ... ]`) and `copy_root` is a path. `copy_root` equal to `dest` means `backup` writes `BDMV` straight into the target folder; a path one level below it means `backup` made its own folder;
- `files` and `bytes` are above 0.

If `top_level` appears as an object with `value` and `Count` fields, the script has a defect: stop and report.

Then copy everything to the Mac and read it. Run the commands one at a time, not joined with `&&` and not under `set -e`. compare_scans.py exits 1 when its answer is "different", and `grep -c` exits 1 when it counts 0. Both are findings, not failures. A Python traceback is an error: stop and report.

```bash
scripts/phase0/pull-recording.sh SLUG_A
cat corpus/owner/SLUG_A/backup/backup-summary.json
grep -E '^MSG:(5069|5070|5079|5010|2003|2019)' corpus/owner/SLUG_A/backup/backup.txt | cut -c1-160
grep -E '^MSG:' corpus/owner/SLUG_A/backup/backup.txt | cut -d, -f1,4 | sort | uniq -c
python3 - SLUG_A <<'EOF'
import json, pathlib, sys
d = pathlib.Path("corpus/owner") / sys.argv[1]
disc = {e["path"].upper(): e["size"] for e in json.loads((d / "files.json").read_text(encoding="utf-8-sig"))}
copy = {}
for line in (d / "backup" / "copy-files.txt").read_text(encoding="utf-8-sig").splitlines():
    size, _, path = line.partition(" ")
    if path:
        copy[path.upper()] = int(size)
print(f"disc: {len(disc)} files, {sum(disc.values())} bytes; copy: {len(copy)} files, {sum(copy.values())} bytes")
groups = (
    ("only on the disc", sorted(set(disc) - set(copy))),
    ("only in the copy", sorted(set(copy) - set(disc))),
    ("size differs", sorted(p for p in set(disc) & set(copy) if disc[p] != copy[p])),
)
for title, paths in groups:
    print(f"{title}: {len(paths)}")
    for p in paths[:30]:
        print("  ", p, disc.get(p, "-"), copy.get(p, "-"))
    if len(paths) > 30:
        print("   ...")
stream = [p for p in disc if p.startswith("BDMV/STREAM/")]
bad = [p for p in stream if copy.get(p) != disc[p]]
print(f"BDMV/STREAM: {len(stream)} files on the disc, {len(bad)} missing from the copy or of another size")
EOF
python3 scripts/phase0/compare_scans.py corpus/owner/SLUG_A/scan.txt corpus/owner/SLUG_A/backup/scan-file.txt; echo "compare_scans exit=$?"
grep -ci '"BDMV/JAR/' corpus/owner/SLUG_A/files.json
python3 - SLUG_A <<'EOF'
import pathlib, sys
sys.path.insert(0, "scripts/phase0")
from robot import parse
d = pathlib.Path("corpus/owner") / sys.argv[1]
for name in ("scan.txt", "backup/scan-file.txt"):
    scan = parse((d / name).read_text(encoding="utf-8-sig", errors="replace"))
    java = any(code == 3344 for code, _, _ in scan.messages)
    labelled = [(t, a.get(16), a.get(49)) for t, a in sorted(scan.titles.items()) if a.get(49)]
    print(f"{name}: titles={len(scan.titles)} java_in_use(3344)={java} labelled titles={len(labelled)}")
    for row in labelled:
        print("  ", row)
EOF
```

Expected:
- `pulled corpus/owner/SLUG_A (N files copied, N files there now)`. If it fails, nothing on the Mac changed; run it again.
- The summary as in the log.
- The message lines: one `MSG:5070` (or `5079`), no `5069`, and the counts of `2003` (read errors) and `2019` (write errors), if any. The second grep lists every distinct message of the copy with its count.
- The file comparison: the disc's and the copy's file and byte counts, the files only on one side and the files whose size differs (the first 30 of each), then the `BDMV/STREAM` line. `0 missing from the copy or of another size` means that every stream file arrived complete. Files only on the disc, such as `AACS/...`, are expected from a decrypted copy; list them in the report.
- compare_scans prints `titles: A=<disc scan> B=<copy scan> common=<n>`, then the `only in A`, `only in B` and `comment differs` lists. A title is keyed by source file, segment map and duration. It exits 0 only when no title is in only one scan. It prints `EMPTY SCAN` if either scan has no titles.
- The `grep -ci` count: the number of `BDMV/JAR/` entries in the disc's file list. A count above 0 means Disc A has Java menus. `0` means it has none: `MSG:3344` shows only that MakeMKV used a Java runtime, not that the disc has Java menus.
- For each scan, the title count, whether `MSG:3344` (Java in use) appears, and every title with a comment (attribute 49), with its source file.

- [ ] **Step 6: Extract the main feature from the copy**

Which disc is in the drive: Disc A stays in drive 1 (`D:`). It is not used: the extraction reads only the copy on the share.

Skip this step if Step 5 did not end with `RESULT ok`. Experiment 3 is then answered from Step 5 alone.

Choose K: the title of the copy's scan that matches N by source file, segment map and duration (spec 6.3 step 8). SLUG_A and N are command-line arguments here, so nothing inside the heredoc needs editing:

```bash
python3 - SLUG_A N <<'EOF'
import pathlib, sys
sys.path.insert(0, "scripts/phase0")
from robot import parse
d = pathlib.Path("corpus/owner") / sys.argv[1]
n = int(sys.argv[2])
disc = parse((d / "scan.txt").read_text(encoding="utf-8-sig", errors="replace"))
copy = parse((d / "backup" / "scan-file.txt").read_text(encoding="utf-8-sig", errors="replace"))
def key(attrs):
    """Spec 6.3 matches titles by source file, segment map and duration."""
    return (attrs.get(16), attrs.get(26), attrs.get(9))
if n not in disc.titles:
    sys.exit(f"scan.txt lists no title {n}")
want = key(disc.titles[n])
on_disc = [t for t, a in sorted(disc.titles.items()) if key(a) == want]
in_copy = [t for t, a in sorted(copy.titles.items()) if key(a) == want]
print(f"disc title {n}: source {want[0]}, duration {want[2]}, segments {(want[1] or '')[:60]}, comment {disc.titles[n].get(49, '')}")
print(f"disc titles with this key: {on_disc}; copy titles with this key: {in_copy}")
for t, a in sorted(copy.titles.items()):
    mark = "<== same key" if t in in_copy else ""
    print(f"  copy title {t:>3}  {a.get(16, ''):<14} {a.get(9, ''):>8}  {(a.get(26) or '')[:40]:<40}  {a.get(49, '')}  {mark}")
if in_copy and len(in_copy) == len(on_disc):
    print("K =", in_copy[on_disc.index(n)])
else:
    print(f"K cannot be chosen: {len(in_copy)} copy titles and {len(on_disc)} disc titles have this key")
EOF
```

Expected: N's source file, duration, segment map and comment; the titles with that key in each scan; every title of the copy's scan, with the matches marked; and a last line `K = <number>`. When several titles share the key, as angles can, K is the one in the same position among them as N is on the disc. If the last line says `K cannot be chosen`, skip the rest of this step. Experiment 3's title question is then answered no for the main feature; record the printed lines in EXP-03.

Then extract title K:

```bash
scripts/phase0/job.sh extract-from-copy.ps1 extract-copy -Slug SLUG_A -Title K
scripts/phase0/wait.sh extract-copy
```

MakeMKV scans the copy again before it writes; then it reads the title from the share and writes it back to the share. To see progress:

```bash
ssh USER@makemkv-box "powershell -NoProfile -Command \"Get-Date -Format 'yyyy-MM-dd HH:mm:ss.fff'; Get-Content C:\lazymkv\phase0\recordings\SLUG_A\extract\extract.txt -Tail 2\""
```

What wait.sh's exit codes mean for this job:
- **0**: check the log against the Expected below.
- **1**: read the `RESULT FAILED:` line.
  - A reason from the checks before MakeMKV starts means nothing was extracted. These are: Assert-Slug, a missing tool or profile, `the copy of SLUG_A did not succeed`, `scan-file.txt lists no title`, `MakeMKV is still running`, `is not empty`, an error creating a folder, or `the copy is no longer at`. Fix the cause (run the setup-check job again for the share), then start the job again under the log name `extract-copy-b`.
  - `the extraction failed: ...`, `mkvmerge -J exited ...` or `mkvmerge -J lists no tracks`: the extraction ran and was judged a failure. Pull the recording, read `grep -E '^MSG:' corpus/owner/SLUG_A/extract/extract.txt | cut -c1-160`, and stop and report. A repeat after the fix is the same job command with `-Force` added, under the log name `extract-copy-b`, after the failed attempt has been kept as "When -Force is right" says.
- **3**: still running. This is normal: the scan of the copy, then a full read and a full write of the feature over the network, can take an hour or more. Run `scripts/phase0/wait.sh extract-copy` again.
  - MakeMKV is stuck if two progress checks 15 minutes apart show the same last lines, or if three hours pass without a DONE line.
  - Then end MakeMKV, not the job: `ssh USER@makemkv-box "taskkill /im makemkvcon64.exe /f"`. The script writes its summary and fails, and wait.sh exits 1. Stop and report.
- **4**: the job ended without a DONE line. Check `ssh USER@makemkv-box "tasklist | findstr /i makemkvcon"`, then stop and report.
- **5**: the mini PC cannot be reached. Run wait.sh again once it can.

Expected (exit 0), in the log: `extract: SLUG_A title K (source file in scan-file.txt: <file>) from <copy root> to \\unas-pro\media\lazymkv-staging\SLUG_A\extract, started <time>`, then `extract: <name>.mkv (<bytes> bytes), <n> tracks`, the summary JSON, `RESULT ok` and `PHASE0-DONE status=ok exit=<n>`. In the JSON:
- `ok` is true;
- `files` is a JSON list holding one `.mkv` name, and `bytes` is above 0;
- `fail_codes` is an empty list and `profile_errors` is 0;
- `rip_source_file` equals `scan_source_file`. A null `rip_source_file` means this run printed no `MSG:3307` for title K; note it for EXP-03;
- `mkvmerge_exit` is 0 or 1, and `tracks` is above 0.

Then copy the evidence to the Mac and compare. Run the commands one at a time, not joined with `&&` and not under `set -e`: compare_tracks.py exits 1 when any verdict is no, which is a finding here.

```bash
scripts/phase0/pull-recording.sh SLUG_A
cat corpus/owner/SLUG_A/extract/extract-summary.json
grep -E '^MSG:(3307|3308|5003|5004|5005|5036|5037|1009|2003|2019)' corpus/owner/SLUG_A/extract/extract.txt | cut -c1-160
python3 scripts/phase0/compare_tracks.py corpus/owner/SLUG_A/backup/scan-file.txt K corpus/owner/SLUG_A/extract/mkvmerge.json; echo "compare_tracks exit=$?"
python3 scripts/phase0/compare_tracks.py corpus/owner/SLUG_A/scan.txt N corpus/owner/SLUG_A/extract/mkvmerge.json; echo "compare_tracks exit=$?"
python3 - corpus/owner/SLUG_A/all-tracks/mkvmerge.json corpus/owner/SLUG_A/extract/mkvmerge.json <<'EOF'
import json, sys
def load(path):
    data = json.load(open(path, encoding="utf-8-sig"))
    rows = [(t["type"], t["codec"], t["properties"].get("language", "und"), t["properties"].get("audio_channels", ""))
            for t in data["tracks"]]
    seconds = data.get("container", {}).get("properties", {}).get("duration", 0) / 1e9
    chapters = sum(c.get("num_entries", 0) for c in data.get("chapters", []))
    return rows, seconds, chapters
direct, direct_s, direct_ch = load(sys.argv[1])
extracted, extracted_s, extracted_ch = load(sys.argv[2])
for i in range(max(len(direct), len(extracted))):
    print(i, direct[i] if i < len(direct) else "-", "|", extracted[i] if i < len(extracted) else "-")
print(f"direct rip: {len(direct)} tracks, {direct_ch} chapters, {direct_s:.1f} s; extracted: {len(extracted)} tracks, {extracted_ch} chapters, {extracted_s:.1f} s")
print("SAME TRACKS", "yes" if direct == extracted else "no")
print("SAME CHAPTERS", "yes" if direct_ch == extracted_ch else "no")
print("DURATION WITHIN 2 S", "yes" if abs(direct_s - extracted_s) <= 2 else "no")
EOF
```

Expected:
- `pulled corpus/owner/SLUG_A (...)`, and the summary as in the log.
- The message lines: one success summary (`MSG:5005` or `5036`) for one title, none of `5003`, `5004`, `5037` or `1009`, and a `MSG:3307` line that maps title K to N's source file.
- Each compare_tracks run prints the side-by-side table, the lines `TRACK COUNT`, `ORDER MATCH`, `CHAPTERS`, `DURATION`, `VIDEO TRACKS`, `STATISTICS TAGS`, then `VERDICT count=... order=... video=... chapters=... duration=...` and the codec reminder. `compare_tracks exit=0` means every verdict is yes. The first run checks the file against the copy's scan (title K); the second checks it against the disc-time scan (title N), which spec 6.3 keeps as the reference.
- The last command prints the direct rip's tracks (Task 11) beside the extracted file's, one row per position (type, codec, language, channels). It then prints the counts and three lines: `SAME TRACKS`, `SAME CHAPTERS` and `DURATION WITHIN 2 S`, each `yes` or `no`.

- [ ] **Step 7: Write the experiment 3 report**

Which disc is in the drive: Disc A stays in drive 1 (not used).

Create `docs/phase0/experiments/EXP-03-whole-disc-copy.md` from `docs/phase0/experiments/TEMPLATE.md`. Fill in the header:
- `Disc(s): SLUG_A, UHD (meta.json format bluray), drive 1 (D:)`;
- the MakeMKV version from the `MSG:1005` line of `backup.txt`;
- `Spec reference: section 17, row 3`.

Under "Question", give section 17 row 3's question.

Under "What was run", record:
- the copy: `backup --decrypt disc:<index>` to `\\unas-pro\media\lazymkv-staging\SLUG_A\backup` with `--cache=1024`, by `backup-disc.ps1`;
- the scan of the copy: `info file:<copy root>` with the same flags as the disc-time scan;
- the extraction of title K by `extract-from-copy.ps1` (`mkv file:<copy root> K ...`), and how K was chosen (Step 6's printed lines);
- any repeat with `-Force`, and the `backup-failed-N` or `extract-failed-N` folders it left.

Under "What was observed", record:
- The copy: `started`, `elapsed_s` in minutes, `bytes` in GB, `mb_per_s`, and the disc's `total_bytes` for comparison; `copy_message` and any `5069`; `read_errors` and `write_errors`; every distinct message of `backup.txt` with its count.
- The copy's layout: `top_level` and `copy_root`, and therefore whether `backup` writes `BDMV` straight into the target folder and whether it accepted an existing empty folder. Then the file comparison with the disc's `files.json`: files only on the disc, only in the copy, sizes that differ, and the `BDMV/STREAM` line.
- The scan comparison: compare_scans' `titles:` line, its `only in A`, `only in B` and `comment differs` lists, and its exit code.
- Java: the `BDMV/JAR/` count from `files.json`, whether each scan shows `MSG:3344`, and the labelled titles in each scan.
- The extraction, if Step 6 ran: from `extract-summary.json`, `elapsed_s`, `bytes`, `success_message`, `scan_source_file` and `rip_source_file`; the `VERDICT` line of both compare_tracks runs; the three lines of the direct-rip comparison; and the extracted file's `bytes` against the direct rip's (`corpus/owner/SLUG_A/all-tracks/rip-summary.json`). If Step 6 was skipped, say why.

Under "Answer":
1. First state whether Disc A has Java menus: it has them when `files.json` lists `BDMV/JAR/` entries. `MSG:3344` shows only that MakeMKV used a Java runtime.
2. Answer yes only if all three of these hold:
   - the copy completed: `backup-copy` ended `RESULT ok` (`MSG:5070`, or `5079`, and no `5069`);
   - every disc-scan title is in the copy's scan by source file, segment map and duration: compare_scans printed `only in A: 0`. List any titles only in the copy's scan (`only in B`) as an observation; they do not change the answer;
   - the extracted feature matches the direct rip: `SAME TRACKS yes`, `SAME CHAPTERS yes` and `DURATION WITHIN 2 S yes`.
   Otherwise the answer is no, naming which of the three failed. A copy that completed with `5079` is a yes with the note "completed with hash failures".
3. If Disc A has no Java menus, the question's condition ("a UHD with Java menus") is not met. The answer then covers the network copy of a UHD without Java menus; say that the Java-menu case is not yet answered. Ask the owner whether they have a UHD with Java menus and want to give one to two more hours of drive time for a repeat, and record the answer. If yes, tell the controller. The repeat needs that disc recorded with a scan first (Task 15 Step 1), then Steps 5 to 7 of this task with its slug and its main-feature title number. With no direct rip of that disc, the compare_tracks verdict against its disc scan replaces the direct-rip comparison.
4. Note separately whether main-feature labels survived in the copy's scan, but only when `scan.txt` has at least one labelled title: compare its labelled titles with the copy's, and quote compare_scans' `comment differs` list. When `scan.txt` has no labelled title, write "there were no labels to compare"; `comment differs: 0` alone does not show that labels survived.

Under "Consequence for the spec":
- If yes, spec 6.3 stands. Record the copy's layout (where `BDMV` lands, so that `info file:` and `mkv file:` are pointed at the right folder) and the copy's throughput, for spec section 18's network-cost estimate.
- If no, spec 6.3 is replaced by its fallback (spec section 17 row 3): title-unsure discs rip every title that passes the candidate filter (8.2) in Direct mode, most likely first, waiting in `AwaitingSpace` if they do not fit.
- If labels did not survive in the copy, no design changes: spec 6.3 already keeps the disc-time scan, with its labels, as the reference.

- [ ] **Step 8: Test eject on drive 1**

Which disc is in the drive: Disc A in drive 1 (`D:`). After this step the tray is open, with Disc A in it.

Ask the owner to watch drive 1's tray. Then:

```bash
ssh USER@makemkv-box "tasklist | findstr /i makemkv"
ssh USER@makemkv-box "C:\lazymkv\target\release\lazymkv-capture.exe eject --drive D"; echo "exit=$?"
```

Expected:
- `tasklist | findstr` prints nothing: no MakeMKV program is running. If it prints a row, ask the owner to close the MakeMKV window, or end `makemkvcon64.exe` with `ssh USER@makemkv-box "taskkill /im makemkvcon64.exe /f"`, and check again before the eject.
- `lock=ok, dismount=ok, allow-removal=ok, eject=ok`, then `exit=0`. The ssh session is elevated (checked by the controller before the disc sessions), as opening `\\.\D:` for writing and locking the volume require.
- `exit=` is ssh's exit status, which is the remote program's exit code; bash expands `$?` on the Mac, so cmd never sees it. A failed eject prints its report ending `eject=error N` (or `cannot open drive D: (error N)`) and gives `exit=2`. `exit=255` means ssh itself failed: run the line again.
- A line such as `lock=error 5, dismount=ok, allow-removal=ok, eject=ok` with `exit=0` is a success with a failed step; record it as printed.

Ask the owner: "Did drive 1's tray open?" Write down EJECT1 (the printed line and `exit=`) and TRAY1 (the owner's answer).

- [ ] **Step 9: Test the second drive**

Which disc is in the drive: Disc A, moved by the owner from drive 1 to drive 2 in this step. second-drive.ps1 checks that the disc in drive 2 has Disc A's fingerprint before it keeps anything.

Experiment 5 asks whether `AACS/Unit_Key_RO.inf` can be read from a mounted UHD in both drives, and Disc A's first-drive disc ID is already in `corpus/owner/SLUG_A/meta.json`. So the second drive is tested with Disc A. Ask the owner: "Please take Disc A out of drive 1's tray, unplug drive 1, plug in the second drive, put Disc A in it, and tell me when its light has stopped blinking." If the owner cannot attach the second drive now, record drive 2 as not run, with the reason, in EXP-05 and EXP-07, skip the rest of this step, and go to Step 10.

Find the second drive's letter and name as MakeMKV lists them:

```bash
ssh USER@makemkv-box "\"C:\Program Files (x86)\MakeMKV\makemkvcon64.exe\" -r --cache=1 info disc:9999" | grep '^DRV:' | grep -v '^DRV:[0-9]*,256,'
```

A remote command line that begins with the quoted program path works (checked by the controller before the disc sessions). The second grep keeps every attached drive, whatever its index; state 256 means no drive at that index.

Expected: exactly one line, `DRV:<index>,2,999,<flags>,"<drive name>","<Disc A's label>","<letter>:"`. LETTER2 is the letter in the last field, without the colon. Other results:
- two lines: drive 1 is still attached. Ask the owner to unplug it, then run the line again;
- state `3` (loading) or `0` (no disc): wait half a minute and run it again;
- no line: MakeMKV does not see the drive. Ask the owner to check the cable and run it again. If it still prints nothing, record drive 2 as not seen by MakeMKV in EXP-05 and EXP-07, and go to Step 10.

Then record Disc A in this drive:

```bash
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\second-drive.ps1 -Slug SLUG_A -Letter LETTER2"; echo "exit=$?"
```

Expected:

```text
drive: LETTER2: is MakeMKV index <n>, state 2, name '<drive name>', disc label '<label>'
snapshot: label=<label> fingerprint=<the same 32 hex characters as meta.json> disc_id=<40 hex characters, or empty> files=<N>
first drive (meta.json): fingerprint=<32 hex characters> disc_id=<40 hex characters, or empty>
disc_id: second drive <read|NOT read>; same as the first drive: <True|False|not compared>
saved: C:\lazymkv\phase0\recordings\SLUG_A\drive2
RESULT ok
exit=0
```

Write down DRIVE2_NAME (the `name '...'` text). Other results (a failed run leaves the recording unchanged, so the same line can simply be run again):
- `RESULT FAILED: no readable disc in LETTER2: after 120 s`: ask the owner whether Disc A is in and the light has stopped, then run the line again.
- `RESULT FAILED: ... is not an optical drive`: LETTER2 is wrong. Read the DRV line again.
- `RESULT FAILED: MakeMKV lists no drive at ...` or `... does not list a disc in ...`: run the DRV line again, then this line.
- `RESULT FAILED: the disc in LETTER2: is not the disc of recording SLUG_A ...`: another disc is in the drive. Ask the owner for Disc A, then run the line again.
- `RESULT FAILED: lazymkv-capture disc failed ...`: stop and report, with the error text printed above the RESULT line.
- `RESULT FAILED: ... already exists and is kept as evidence ...`: a drive-2 recording was already made. Stop and report; do not use `-Force`.
- `exit=255`: ssh could not reach the mini PC. Run the line again once it can.

Then test eject on this drive. Ask the owner to watch drive 2's tray.

```bash
ssh USER@makemkv-box "tasklist | findstr /i makemkv"
ssh USER@makemkv-box "C:\lazymkv\target\release\lazymkv-capture.exe eject --drive LETTER2"; echo "exit=$?"
```

Expected as in Step 8, for `LETTER2:`. Ask the owner: "Did drive 2's tray open?" Write down EJECT2 and TRAY2.

Copy the recording to the Mac and compare the two drives. Run the commands one at a time; `grep` exits 1 when it finds nothing, which is a finding here.

```bash
scripts/phase0/pull-recording.sh SLUG_A
python3 - SLUG_A <<'EOF'
import json, pathlib, sys
d = pathlib.Path("corpus/owner") / sys.argv[1]
one = json.loads((d / "meta.json").read_text(encoding="utf-8-sig"))
two = json.loads((d / "drive2" / "snapshot" / "meta.json").read_text(encoding="utf-8-sig"))
print("same fingerprint:", one["fingerprint"] == two["fingerprint"])
same_id = (one["disc_id"] == two["disc_id"]) if one["disc_id"] and two["disc_id"] else "not compared"
print("disc_id drive 1:", one["disc_id"], "| drive 2:", two["disc_id"], "| equal:", same_id)
print("volume label drive 1:", one["volume_label"], "| drive 2:", two["volume_label"])
print("files drive 1:", one["file_count"], "| drive 2:", two["file_count"])
EOF
grep -i -A1 unit_key corpus/owner/SLUG_A/drive2/snapshot/files.json
grep '^DRV:' corpus/owner/SLUG_A/drive2/drives.txt | grep -v '^DRV:[0-9]*,256,'
```

Expected:
- `pulled corpus/owner/SLUG_A (...)`.
- `same fingerprint: True`, the two disc IDs and whether they are equal, and the volume labels and file counts of both drives.
- `grep -A1 unit_key` prints the `"path": ".../Unit_Key_RO.inf"` line and the `"size": <n>` line below it when drive 2 lists the file, and nothing when it does not. With a disc ID of None, the first means the file is listed but could not be read in this drive; the second means the drive does not expose it.
- The recorded `DRV` line for drive 2.

The `drive2` folder is part of Disc A's recording. `hit_rate.py` counts only top-level folders with a `meta.json`, so experiment 10 does not count Disc A twice.

- [ ] **Step 10: Leave the right drive attached**

Which disc is in the drive: none needed. Disc A is in drive 2 if the owner moved it there in Step 9 (in an open tray if Step 9's eject ran), otherwise in drive 1's open tray (Step 8). The owner takes it out now, before any drive is unplugged, and keeps it at hand until the commit in Step 13, in case a step has to be repeated. This step ends with the attached drive empty and its tray closed, which is how Task 15 starts.

Ask the owner: "Please take Disc A out of the drive it is in (if that tray is closed, press the drive's eject button first), keep it at hand until I say Task 14 is committed, and close that tray with no disc in it. Which drive should stay attached for the rest of phase 0 (Task 15 records about 20 discs)? If it is drive 1, please unplug drive 2 and plug drive 1 back in, and leave its tray closed with no disc in it." Write down KEEP (`1` or `2`). Then check which letter Windows gave it, from a job, as every later script sees it:

```bash
scripts/phase0/job.sh setup-check.ps1 setup-check-after-drives
scripts/phase0/wait.sh setup-check-after-drives
```

What wait.sh's exit codes mean for this job:
- **0**: the log contains `drive      : D: is MakeMKV disc index <n>` (with the letter from `config.ps1`'s `$Drive`). The attached drive has that letter: KEEP_LETTER is it, and `config.ps1` stays as it is. The `staging` line must say `write, rename and delete OK`.
- **1**: with `ERROR: MakeMKV lists no drive at D:`, the attached drive has another letter, or MakeMKV does not name the letter of an empty drive. Run the DRV line of Step 9.
  - If it shows the attached drive with its letter in the last field, that letter is KEEP_LETTER: change `config.ps1` as below.
  - If the last field is empty, ask the owner to put any disc in that drive, and run the DRV line again; the letter it now shows is KEEP_LETTER. That disc stays in until a setup check has ended with exit 0; the last paragraph of this step has it taken out. If KEEP_LETTER is the letter in `$Drive`, `config.ps1` stays as it is: run the setup check once more, with that disc in, as `scripts/phase0/job.sh setup-check.ps1 setup-check-after-config` and then `scripts/phase0/wait.sh setup-check-after-config`, and read its exit codes as here. Otherwise change `config.ps1` as below.
  - Any other `ERROR:` line: stop and report.
- **3**: still running (it normally takes under a minute). Run wait.sh again.
- **4**: the job is not running and wrote no DONE line. Stop and report.
- **5**: the mini PC cannot be reached. Run wait.sh again once it can.

Only if KEEP_LETTER differs from `$Drive`, replace the settings file as a whole: write it on the Mac and copy it over, rather than editing it through ssh's cmd.exe. Copy it to the scratch directory and look at it:

```bash
scp -q USER@makemkv-box:C:/lazymkv/phase0/config.ps1 SCRATCH/config-before.ps1
cat SCRATCH/config-before.ps1
```

Write `SCRATCH/config.ps1` with the Write tool. It must hold exactly the lines of `config-before.ps1`, except the `$Drive = '...'` line, which becomes `$Drive = 'KEEP_LETTER'`. Then:

```bash
tr -d '\r' < SCRATCH/config-before.ps1 | diff - SCRATCH/config.ps1
LC_ALL=C grep -n '[^ -~]' SCRATCH/config.ps1
scp -q SCRATCH/config.ps1 USER@makemkv-box:C:/lazymkv/phase0/config.ps1
ssh USER@makemkv-box "type C:\lazymkv\phase0\config.ps1"
scripts/phase0/job.sh setup-check.ps1 setup-check-after-config
scripts/phase0/wait.sh setup-check-after-config
```

Expected: `diff` shows only the `$Drive` line, old and new; the ASCII check prints nothing; `type` prints the new file. The setup-check job's exit codes read as above, and with exit 0 its log contains `drive      : KEEP_LETTER: is MakeMKV disc index <n>` and the staging OK line. If `diff` shows any other line, fix `SCRATCH/config.ps1` before copying it.

Last, once a setup check has ended with exit 0: if a disc was put in the drive for the DRV line, ask the owner: "Please take that disc out and close the empty tray." The attached drive is now empty with its tray closed, and Disc A is with the owner.

- [ ] **Step 11: Write the experiment 7 report and complete experiment 5**

Which disc is in the drive: none; Disc A is with the owner. The drive is not used.

Create `docs/phase0/experiments/EXP-07-eject.md` from `docs/phase0/experiments/TEMPLATE.md`, with `Disc(s): SLUG_A, UHD, in drive 1 (D:) and drive 2 (LETTER2:)`, the MakeMKV version and `Spec reference: section 17, row 7`.
- Under "What was run": `lazymkv-capture eject --drive <letter>` over ssh, once per drive, each after a check that no MakeMKV program was running.
- Under "What was observed", for each drive:
  - its name as MakeMKV lists it (drive 1: `BD-RE HL-DT-ST BD-RE BU40N 1.03 SERIAL`, from `docs/phase0/environment.md`; drive 2: DRIVE2_NAME) and its letter;
  - EJECT1 or EJECT2, as printed, including any step that reported `error N`;
  - TRAY1 or TRAY2.
  - Name any drive that was not run, with the reason.
- Answer yes only if, on both drives, the eject exited 0 and the owner saw the tray open. If only one drive was tested, answer for that drive and say the other was not run.
- Consequence if no: the `eject_failed` notification asks the owner to remove the disc (spec section 17 row 7). Also record, for spec 4.3's Ejector, any step that failed while the eject still worked (for example the volume lock).

Complete `docs/phase0/experiments/EXP-05-offline-and-unit-key.md`:
- Change the header's `Disc(s):` line to add `and drive 2 (LETTER2:, DRIVE2_NAME)`.
- Under "What was observed", replace the sentence saying that only drive 1 was tested with:

```markdown
Second drive (Task 14): Disc A in drive 2 (`LETTER2:`, `DRIVE2_NAME`), snapshot in
`corpus/owner/SLUG_A/drive2/snapshot/meta.json`, made by `scripts/phase0/second-drive.ps1`.
Fingerprint equal to drive 1's: yes. Disc ID in drive 2: DISCID2_STATE. Equal to drive 1's: SAME_ID.
```

  - DISCID2_STATE and SAME_ID are filled in as the Placeholders list defines them.
  - If drive 2 was not run, write that instead, with the reason.
- Under "Answer", restate the second question for both drives: `AACS/Unit_Key_RO.inf` was readable in both drives, in drive 1 only, in drive 2 only, or in neither.
- Under "Consequence for the spec": if the file was unreadable in either drive, lookups on that drive use the fingerprint only (identity 0.95 at best from TheDiscDB), as row 5's fallback says.

In `docs/phase0/environment.md`, under `## Optical drives`, replace these lines:

```markdown
- Drive 1 (attached, `D:`): `BD-RE HL-DT-ST BD-RE BU40N 1.03 SERIAL`
- Drive 2: not attached when this was recorded. Its `DRV` line is added in Task 14, when the
  owner swaps drives.
```

with:

```markdown
- Drive 1 (`D:` in Tasks 7 to 14): `BD-RE HL-DT-ST BD-RE BU40N 1.03 SERIAL`
- Drive 2 (`LETTER2:` when attached in Task 14): `DRIVE2_NAME`
- Attached after Task 14: drive KEEP, at `KEEP_LETTER:` (`$Drive` in `C:\lazymkv\phase0\config.ps1`,
  confirmed by the setup check `setup-check-after-drives` or `setup-check-after-config`).
```

If drive 2 was not run, its line reads `- Drive 2: not tested in Task 14 (<reason>).`

- [ ] **Step 12: Add the extracted title to the tuning dataset (decision D16)**

Which disc is in the drive: none (Disc A is with the owner). The job reads only the share.

Skip this step if Step 6 did not end `RESULT ok`. The extraction is a rip of title N made from the disc copy. Its recording has `mkvmerge.json` but no `ffprobe-streams.json`, so `args` reads the `ffprobe-streams.json` of a rip of the same title with the same size if there is one, and otherwise mkvmerge.json's track IDs, which for a Matroska file are ffprobe's stream numbers:

```bash
python3 scripts/phase0/audio_dataset.py args corpus/owner/SLUG_A extract
```

Expected: `SLUG_A/extract: <n> audio streams (from <source>); original language spa (TMDb es)`, the stream lines, the line `  programme reference: stream <M> (...), placed first in the original language (spec 8.6 steps 3 and 4); candidates: ...`, possibly `  note: ...` lines (name each in the report), and the job lines. Run them, then the pull, `auto` and the harness, as in Task 11 Step 11, with its exit codes and its Expected. When `<source>` is `mkvmerge.json (track IDs: ...)`, check that the job's `stream <n>:` lines show the codec and channels that `args` listed for the same numbers; if they do not, stop and report (the harness would also report `wrong reference` for that folder). Then compare the packets with the direct rip:

```bash
grep -h '^\(main\|cand\)=' corpus/owner/SLUG_A/all-tracks/audio-levels-20ms-*/manifest.txt corpus/owner/SLUG_A/extract/audio-levels-20ms-*/manifest.txt | sed 's/ frames=.*md5=/ md5=/' | sort | uniq -c
```

Expected: every `main=`/`cand=` line appears twice (count `2`), once from each rip. The extracted title's audio is then bit-identical to the direct rip's, which is evidence for EXP-03. A line with count `1` names a stream whose packets differ: record it in EXP-03.

In Step 13 (Commit), add `corpus/owner/audio-dataset.json` to the `git add` line, after `corpus/owner/SLUG_A`; `git status --short` then also lists the extract's level folder files (`streams.json`, `proxy-check.json`, `audio-proxy.json`), `corpus/owner/SLUG_A/audio-labels.json` when labels were added, and `corpus/owner/audio-dataset.json`.

- [ ] **Step 13: Commit**

Which disc is in the drive: none; Disc A is with the owner. Disc A's part in phase 0 ends here.

```bash
find corpus/owner -iname '*unit_key*'
grep -rIl 'eyJhbGciOi' corpus/owner
git add scripts/phase0/backup-disc.ps1 scripts/phase0/extract-from-copy.ps1 scripts/phase0/second-drive.ps1 corpus/owner/SLUG_A docs/phase0 corpus/owner/audio-dataset.json
git status --short
git commit -m "Run experiments 3 and 7: whole-disc copy and eject" -m "Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

Expected:
- `find` and `grep` print nothing: no AACS unit-key file and no JSON Web Token in any recording. `copy-files.txt` and `files.json` may name `AACS/Unit_Key_RO.inf` as a path, which is not its content. If either command prints a path, stop and do not commit.
- `git status --short` lists:
  - the three new scripts;
  - new files under `corpus/owner/SLUG_A/`: `backup/`, `extract/` (if Step 6 ran), `drive2/` (if Step 9 ran), and any `backup-failed-N/` or `extract-failed-N/`;
  - the new `EXP-03-whole-disc-copy.md` and `EXP-07-eject.md`, and the changed `EXP-05-offline-and-unit-key.md` and `environment.md`.
  - `config.ps1` is not listed: it is not in the repository.
- The commit succeeds.

Then tell the owner: "Task 14 is committed. You can put Disc A away; phase 0 does not need it again. Please leave the attached drive empty with its tray closed."

---

### Task 15: The remaining discs (experiments 8 and 10, and repeats)

**Files:**
- Create: `corpus/owner/<slug>/` for every disc recorded in Steps 1 and 5 that has no recording yet, and `corpus/owner/EXP2_SLUG/exp02/` (Step 2).
- Create or append to (on the Mac only; a pull never removes it): `corpus/owner/<slug>/notes.md` for every disc of Step 1 (section `## Ground truth (Task 15)`) and for DVD_SLUG (section `## DVD flags and closed captions (Task 15)`, Step 3).
- Create: `docs/phase0/experiments/EXP-08-encoding.md`, `docs/phase0/experiments/EXP-10-thediscdb-hit-rate.md`.
- Modify: `docs/phase0/experiments/EXP-02-index-guard.md` (Step 2), and `docs/phase0/experiments/EXP-12-java-labels.md` (Step 6, only when Disc A did not answer it).
- No script is created or changed.

**Interfaces:**
- Consumes:
  - From `env.ps1` (decision D1): `Get-InsertedDisc`, called directly over plain ssh to read the fingerprint of the disc in the drive. Through the scripts below also `$Recordings`, `Assert-Slug`, `Assert-DiscIs`, `Assert-EmptyDir`, `Get-DiscIndex` and `Get-MkFlags`.
  - Task 8's final `record-disc.ps1 -Slug <slug> [-NoScan] [-Force]`:
    - with a scan it runs as a job (`scripts/phase0/job.sh record-disc.ps1 <log> -Slug <slug>`); with `-NoScan` it runs over plain ssh;
    - it waits up to 120 s for a readable disc; a new slug gets a snapshot (`meta.json`, `files.json`, `nav\`) and lookups (`thediscdb-hash.json`, `thediscdb-discid.json` when the disc has a disc ID, `lookup-status.txt` with lines `dischash <code>` and `discid <code>`); a slug already recorded is checked with Assert-DiscIs and keeps its snapshot and lookups, unless `-Force` replaces them with new ones of the same disc;
    - output lines `disc: ...`, `lookups: ...`, with a scan also `scan: scan.txt exit.json=... version_banner=... titles=...`, then `RESULT ok` or `RESULT FAILED: <reason>`; after a failure it throws, so wait.sh exits 1 and an ssh run exits 1.
  - Task 8's `scripts/phase0/pull-recording.sh <slug>`: copies the recording over `corpus/owner/<slug>/`, keeps files that exist only on the Mac (such as `notes.md`), never deletes anything, and changes nothing when the copy fails.
  - Task 10's `exp02-index-guard.ps1 -Slug <slug> [-MaxTitles <n>] [-Redo <n,n,...>] [-Plan]` and Task 10 Steps 3 to 6, which run it, pull the results and check them with `scripts/phase0/exp02_check.py <recording dir>`.
  - On the Mac: `scripts/phase0/summarize_scan.py <recording dir>`; `scripts/phase0/check_encoding.py` (its `main(path)`); `scripts/phase0/hit_rate.py <folder>` (one row per subfolder with a `meta.json`; exits 1 when a fingerprint lookup got no answer or a disc-ID lookup failed) and its functions `statuses(dir)` and `lookup(path, status)`; `parse` from `scripts/phase0/robot.py`.
  - `scripts/box.sh`, `scripts/phase0/job.sh`, `scripts/phase0/wait.sh`.
  - The recordings of Tasks 8 to 14 in `corpus/owner/`, Disc A's (SLUG_A) among them, and the reports `EXP-02-index-guard.md` (Task 10), `EXP-03-whole-disc-copy.md` (Task 14) and `EXP-12-java-labels.md` (Task 9).
- Produces:
  - Full recordings (snapshot, lookups, `scan.txt`) of the discs asked for in Step 1, each with the owner's ground truth in `notes.md` under `## Ground truth (Task 15)`, committed one disc at a time.
  - Fingerprint-only recordings (snapshot and lookups) of further discs, until about 20 distinct discs are recorded in all (Step 5).
  - Experiment 2 on a second disc, in `corpus/owner/EXP2_SLUG/exp02/`, and the final answer to experiment 2 in `EXP-02-index-guard.md`.
  - The DVD flag and closed-caption findings for spec 8.7, in `corpus/owner/DVD_SLUG/notes.md` under `## DVD flags and closed captions (Task 15)`. Task 17 reads them when it updates spec 8.7.
  - `EXP-08-encoding.md` and `EXP-10-thediscdb-hit-rate.md`; `EXP-12-java-labels.md` updated when Disc A did not answer it.

Placeholders:
- SLUG_A: Disc A's slug, chosen in Task 9 and written in the `Disc(s):` line of EXP-05.
- SLUG: in Steps 1 and 5, the disc being handled. The fingerprint check (Step 1, part 1c) decides it:
  - for a disc recorded before, SLUG is the slug S that the check prints;
  - for a new disc, SLUG is a new slug made from the title, year and format the owner gives. It has lowercase letters, digits and hyphens only, is not shown by `ls corpus/owner`, and ends in `-uhd`, `-bluray` or `-dvd`, for example `heat-1995-bluray`. A UHD combo pack's Blu-ray disc ends in `-bluray`.
- EXP2_SLUG: the disc that experiment 2 is repeated on, chosen in Step 1, part 1g. It is never SLUG_A, and never a recording that already has an `exp02` folder.
- DVD_SLUG: the DVD of kind (d) in Step 1.
- NONASCII_SLUG: the disc of kind (f) in Step 1, if the owner has one.
- In Step 2, K, PLANNED, SCAN_3308, SCAN_3309, LIMIT_S, ESTIMATE, LIST, T, PID and WRITTEN_MAX are defined as in Task 10, for EXP2_SLUG's run.
- DRIVE: the attached drive's letter, KEEP_LETTER from Task 14 Step 10 (`D` unless Task 14 changed `$Drive`). `grep 'Attached after Task 14' docs/phase0/environment.md` shows it. Where Task 10 shows `D:` in an expected message or a report header, read it as DRIVE:; this task writes DRIVE: itself. The recordings of Tasks 8 to 14 were made at `D:`, so the EXP-08 and EXP-10 headers, which cover them too, name both drives when DRIVE is not `D`.
- `FP` and `HR` are not placeholders: they are shell variables that a command sets and the next command reads. Run those commands together in one shell (one Bash call), as printed.
- Disc A: not needed in this task. Task 14 left the drive empty with its tray closed.

When `-Force` is right (decision D3):
- `record-disc.ps1 -Slug SLUG -NoScan -Force` over ssh: only to repeat lookups that got no answer, that is, a `dischash` or `discid` code other than `200` or `404`, with the same disc still in the drive (Steps 1 and 5). It replaces that recording's snapshot and lookups with new ones of the same disc. Assert-DiscIs refuses any other disc, and `-NoScan` leaves the scan files untouched.
- Never on a record job with a scan. `exp02-index-guard.ps1` has no `-Force`. No other use of `-Force` is right in this task.

- [ ] **Step 1: Record these discs with a full scan**

Which disc is in the drive: none for part 1a. After that, each disc of part 1b in turn, inserted by the owner. record-disc.ps1 checks a disc that is already recorded with Assert-DiscIs before it scans.

**1a. Check the tools and the corpus (once).**

```bash
grep -E '^(function (Assert-Slug|Assert-DiscIs|Assert-EmptyDir|Get-InsertedDisc)|\$Recordings)' scripts/phase0/env.ps1
grep -n 'switch]\$ScanMayFail' scripts/phase0/record-disc.ps1
grep -n 'ever deleted' scripts/phase0/pull-recording.sh
git ls-files scripts/phase0/exp02-index-guard.ps1
python3 -m unittest discover -s scripts/phase0 -p 'test_*.py'
scripts/box.sh cargo build --release -p lazymkv-capture
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -Command \"(Get-Command C:\lazymkv\src\scripts\phase0\record-disc.ps1).Parameters.Keys\""
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -Command \"(Get-Command C:\lazymkv\src\scripts\phase0\exp02-index-guard.ps1).Parameters.Keys\""
ssh USER@makemkv-box "tasklist | findstr /i makemkv"
```

Expected:
- The first `grep` prints five lines: the `$Recordings` assignment and the four D1 helper functions.
- The second prints `[switch]$ScanMayFail,` (Task 8's final record-disc.ps1). The third prints `# Nothing in corpus/owner is ever deleted: ...` (Task 8's pull-recording.sh, which keeps `notes.md` on every pull).
- `git ls-files` prints `scripts/phase0/exp02-index-guard.ps1` (Task 10).
- The unit tests end `Ran 34 tests` (or more, if other tasks added some) and `OK`.
- `box.sh` syncs the tree and cargo ends with ``Finished `release` profile [optimized] target(s) in ...``.
- The parameter lists begin `Slug`, `NoScan`, `ScanName`, `ScanMayFail`, `Force` and `Slug`, `MaxTitles`, `Redo`, `Plan`, with no parse error.
- `tasklist | findstr` prints nothing: no MakeMKV program is running. If it prints a row, ask the owner to close the MakeMKV window, and check again.

Anything else: stop and report. The prerequisites of Tasks 8 and 10 are missing.

Then look at what is recorded already, and at the answers that decide parts of this task:

```bash
python3 - <<'EOF'
import json, pathlib
for meta_path in sorted(pathlib.Path("corpus/owner").glob("*/meta.json")):
    d = meta_path.parent
    meta = json.loads(meta_path.read_text(encoding="utf-8-sig"))
    notes = d / "notes.md"
    text = notes.read_text(encoding="utf-8-sig", errors="replace") if notes.exists() else ""
    heads = [line[3:].strip() for line in text.splitlines() if line.startswith("## ")]
    print(f"{d.name:<36} {str(meta.get('format')):<6} fingerprint={str(meta.get('fingerprint'))[:8]} "
          f"scan={'yes' if (d / 'scan.txt').exists() else 'no'} notes: {'; '.join(heads) or '-'}")
EOF
for f in EXP-02-index-guard EXP-03-whole-disc-copy EXP-12-java-labels; do echo "== $f"; sed -n '/^## Answer/,/^## Consequence/p' "docs/phase0/experiments/$f.md"; done
```

Expected:
- One line per recording: Disc A (SLUG_A), the discs of Task 8, and any added in Tasks 12 and 13. Each line shows the slug, the format, the start of the fingerprint, whether it has a scan, and its `notes.md` sections (for example `Experiment 6 (Task 12)`).
- The three `## Answer` sections in full, each under its `== <report>` line and ending at that report's `## Consequence for the spec` heading. Write down:
  - whether EXP-12's answer is yes (Disc A answered experiment 12). If not, part 1b asks for kind (h);
  - whether EXP-03 says that the Java-menu case is not yet answered. If so, part 1b asks for kind (h) too.

**1b. The discs to ask for.**

Ask the owner for these discs, in this order. One disc may cover several kinds.
- (a) a Blu-ray or UHD with decoy playlists. The owner may not know which discs have them; the scan shows it, because many titles then share the feature's duration.
- (b) a two-cut disc, for example theatrical and extended cuts on one disc.
- (c) a TV disc.
- (d) a DVD with a commentary track, closed captions and forced subtitles. If no DVD has all three, the DVD that has the most of them.
- (e) a non-English film.
- (f) a disc whose title has non-ASCII characters, for example an accented letter.
- (g) a UHD combo pack's Blu-ray disc.
- (h) only if part 1a found that EXP-12's answer is not yes, or that EXP-03 says the Java-menu case is not yet answered: a UHD with Java menus. Its recording shows them (part 1e). If EXP-03 asked for a repeat on such a disc, tell the controller this disc's slug: Task 14's repeat starts from this recording.

Never record a disc a second time under a new slug. Show the owner the slugs from part 1a, and ask whether one of those discs covers a kind:
- A recorded disc whose line says `scan=yes` covers its kinds without being inserted again. Go straight to part 1e with its slug as SLUG; the pull there only refreshes the Mac's copy. There is one exception: if it becomes EXP2_SLUG (part 1g), it must be inserted for Step 2.
- A recorded disc whose line says `scan=no` goes through parts 1c to 1f like a new disc. Its record job keeps its snapshot and adds the scan.

When the owner has no disc of a kind, write down `none available` for it. The reports need kinds (a), (d) and (f).

Do not ask for the next disc until the current one has been through part 1f, and, if it became EXP2_SLUG, through Step 2.

**1c. Identify the disc.**

Which disc is in the drive: the disc the owner has just inserted.

Ask the owner to insert the disc and to tell you what it is:
- the title and year, or for a show the season and the episodes on this disc;
- the format: UHD, Blu-ray or DVD;
- its cuts, and which audio and subtitle tracks are commentary;
- anything else the owner knows: forced subtitles, closed captions, the original language, and the title as printed, with any accented letters.

Ask the owner to say when the drive's light has stopped blinking. Then run these two commands together, in one shell:

```bash
FP="$(ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -Command \". C:\lazymkv\src\scripts\phase0\env.ps1; (Get-InsertedDisc).fingerprint\"" | tr -d '\r' | tail -1)"
python3 - "$FP" <<'EOF'
import json, pathlib, re, sys
fp = sys.argv[1].strip().upper()
if not re.fullmatch(r"[0-9A-F]{32}", fp):
    sys.exit(f"NO FINGERPRINT: the mini PC printed {sys.argv[1]!r}; its error, if any, is above")
hits = [p.parent for p in sorted(pathlib.Path("corpus/owner").glob("*/meta.json"))
        if (json.loads(p.read_text(encoding="utf-8-sig")).get("fingerprint") or "").upper() == fp]
print("fingerprint", fp)
if not hits:
    print("not recorded yet: a new disc")
for d in hits:
    print(f"already recorded as {d.name}; scan.txt {'present' if (d / 'scan.txt').exists() else 'absent'}")
EOF
```

What it prints, and what to do:
- `NO FINGERPRINT: ...`: Get-InsertedDisc failed, and its error is printed above that line.
  - If the error says that no disc is mounted, or that the device is not ready, the disc is still spinning up. Wait half a minute and run both commands again.
  - If it says that `Get-InsertedDisc` is not recognized, the D1 helpers are missing: stop.
  - Any other error: stop and report.
- `fingerprint <32 hex characters>`, then `not recorded yet: a new disc`: choose SLUG as Placeholders says. Go to part 1d.
- `already recorded as S; scan.txt present`: SLUG is S, and the disc has a scan already. Skip part 1d and go to part 1e.
- `already recorded as S; scan.txt absent`: SLUG is S. Part 1d adds the scan and keeps the snapshot.
- Two `already recorded` lines: an earlier task recorded this disc twice. SLUG is the one with `scan.txt present`, or the first one if neither has a scan. Step 5 counts the disc once.

**1d. Record the disc with a scan.**

Which disc is in the drive: SLUG's disc. If SLUG is already recorded, record-disc.ps1 checks the disc with Assert-DiscIs before MakeMKV starts.

```bash
scripts/phase0/job.sh record-disc.ps1 t15-record-SLUG -Slug SLUG
scripts/phase0/wait.sh t15-record-SLUG
```

job.sh prints `started record-disc.ps1; log: C:\lazymkv\phase0\logs\t15-record-SLUG.txt`. If job.sh exits 1 because a phase 0 job is still running, first wait for that job with its own wait.sh line. A job started again for the same disc gets `-2` on its log name (then `-3`, and so on), so that no log is replaced.

What wait.sh's exit codes mean for this job:
- **0**: the job finished. Check the log against the Expected below.
- **1**: the script threw. Read its `RESULT FAILED:` line:
  - Assert-Slug's rejection: SLUG is malformed. Choose a valid slug and start the job again; the log name follows the slug.
  - `no readable disc in DRIVE: after 120 s`: nothing was written. Ask the owner whether the disc is in and the light has stopped, then start the job again as `t15-record-SLUG-2`.
  - `is not the disc of recording SLUG` (Assert-DiscIs): the disc in the drive is not the one part 1c identified, or the mini PC holds a recording of another disc under this slug that was never pulled. Run part 1c again. If it says `not recorded yet`, choose another new slug.
  - `... already exists and is kept as evidence ...`: the mini PC already holds a scan for SLUG that the Mac has not got. Do not use `-Force`. Go to part 1e: its pull fetches that scan.
  - `the scan lists no titles (...)`: MakeMKV ran but listed no title. Pull the recording with `scripts/phase0/pull-recording.sh SLUG`, then run `grep '^MSG:' corpus/owner/SLUG/scan.txt | cut -c1-160`.
    - If the messages show `MSG:5010` (failed to open the disc) or `MSG:3303`, MakeMKV cannot open this disc, for example because it does not know the disc's key. That is a result for this disc, not a defect. Write it in the disc's notes (part 1e), commit it (part 1f), and ask the owner for another disc of the same kind.
    - Otherwise stop and report.
  - `missing ...`, `lazymkv-capture disc failed`, `the disc has no fingerprint`, `the scan did not finish` or `MakeMKV did not run`: stop and report with the log. Any other `RESULT FAILED:` reason, for example `MakeMKV lists no drive at DRIVE:`: stop and report with the log.
- **3**: still running. This is normal: a DVD scan takes a minute or two, a Blu-ray a few minutes, and a UHD with Java menus ten minutes or more. A disc with decoy playlists can take much longer, because MakeMKV examines every playlist. Run `scripts/phase0/wait.sh t15-record-SLUG` again, and start nothing else meanwhile.
  - If 60 minutes pass without a PHASE0-DONE line, tell the owner, and end the job with `ssh USER@makemkv-box "powershell -NoProfile -Command Stop-ScheduledTask -TaskName lazymkv-phase0"`.
  - Check that `ssh USER@makemkv-box "tasklist | findstr /i makemkvcon"` then prints nothing, and stop and report.
- **4**: the job is not running and wrote no PHASE0-DONE line (it never started, or was killed). Check that the owner is logged in at the mini PC's console, then stop and report.
- **5**: the mini PC cannot be reached. The job keeps running there; run wait.sh again once it can be reached.

Expected (exit 0), in the log:
- for a new slug: `disc: label=<label or (none)> format=<bluray|dvd> fingerprint=<the same 32 hex characters as part 1c> disc_id=<40 hex characters, or empty> files=<N>`, then `lookups: dischash <code>[; discid <code>][; tmdb ...]`;
- for a slug recorded before without a scan: `disc: kept from the earlier recording of this same disc: ...`, then `lookups: kept: ...`;
- in both cases, then `scan: scan.txt exit.json={"exit_code":<n>,"elapsed_ms":<n>,"lines":<n>} version_banner=True titles=<n above 0>`, `RESULT ok` and `PHASE0-DONE status=ok exit=<n>`. `exit=<n>` is only the last native command's exit code, not a verdict.

Then read the `lookups:` line. Every `dischash` and `discid` code on it must be `200` (an answer) or `404` (unknown to TheDiscDB). Another code means the lookup got no answer: `000` is no answer at all, `429` a rate limit, `5xx` a server error. Then make the lookups again now, while the disc is still in the drive:

```bash
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\record-disc.ps1 -Slug SLUG -NoScan -Force"; echo "exit=$?"
```

Expected: `disc: label=... fingerprint=<the same as before> ...`, `lookups: ...`, `RESULT ok` and `exit=0`. The scan is not touched. Other results:
- If a code is still not `200` or `404`, leave it: Step 5 tries once more.
- `exit=1` with Assert-DiscIs's message: the disc was changed. Ask the owner for SLUG's disc, then run the line again.
- `exit=255`: ssh could not reach the mini PC. Run the line again once it can.

**1e. Copy the recording to the Mac, look at it, and write down what the disc is.**

Which disc is in the drive: SLUG's disc, if part 1c had it inserted. It stays in until part 1g has decided whether Step 2 needs it. A recorded disc that part 1b reuses without inserting it is not needed here.

```bash
scripts/phase0/pull-recording.sh SLUG
python3 scripts/phase0/summarize_scan.py corpus/owner/SLUG
python3 - SLUG <<'EOF'
import collections, json, pathlib, sys
sys.path.insert(0, "scripts/phase0")
from robot import parse
d = pathlib.Path("corpus/owner") / sys.argv[1]
meta = json.loads((d / "meta.json").read_text(encoding="utf-8-sig"))
scan = parse((d / "scan.txt").read_text(encoding="utf-8-sig", errors="replace"))
codes = collections.Counter(code for code, _, _ in scan.messages)
durations = collections.Counter(a.get(9) for a in scan.titles.values())
common, shared = durations.most_common(1)[0] if durations else ("-", 0)
print(f"{d.name}: format={meta.get('format')} titles={len(scan.titles)} MSG:3308 lines={codes[3308]} "
      f"MSG:3309 lines={codes[3309]}; the most common duration, {common}, is shared by {shared} titles")
EOF
```

SLUG is a command-line argument of the heredoc, so nothing inside it needs editing.

Expected:
- `pulled corpus/owner/SLUG (<n> files copied, <m> files there now)`. If the pull fails, nothing on the Mac changed: run it again.
- The summary starts `MakeMKV: MakeMKV v2.0.0 win(x64-release)` and lists the titles.
- The last line gives the format, the number of titles, the counts of `MSG:3308` (angle) and `MSG:3309` (skipped duplicate) lines, and how many titles share the most common duration.
  - For kind (a), a disc with decoy playlists normally shows many titles sharing the feature's duration, or `MSG:3309` lines. If it shows neither, tell the owner, who may have another candidate.

For kind (h), also run `grep -ci '"BDMV/JAR/' corpus/owner/SLUG/files.json`. A count above 0 means the disc has Java menus. `0` means it has none: ask the owner for another UHD, if there is one.

Then append this section to `corpus/owner/SLUG/notes.md`. Create the file if it is missing, and never replace what is in it; a disc measured in Task 12 already has a section there.

```markdown
## Ground truth (Task 15)

Covers: <the kinds of Task 15 Step 1 it covers, for example "(a) decoy playlists; (b) two cuts">
What it is (the owner's words): <film and year; or show, season and the episodes on this disc>
Format (the owner's words): <UHD | Blu-ray | DVD>
Cuts: <for example "theatrical and extended, both on this disc", or "one">
Commentary tracks: <which audio and subtitle tracks are commentary, as the owner knows them, or "none known">
Other: <forced subtitles, closed captions, original language, the title as printed with its accented letters, anything else>
Scan: <n> titles, <n> MSG:3308 lines, <n> MSG:3309 lines (Task 15 Step 1e); or "MakeMKV could not list it: <the MSG lines>"
```

**1f. Commit the disc at once.**

Which disc is in the drive: unchanged from part 1e. The commit does not use the drive.

A pull never deletes `notes.md` (Task 8's pull-recording.sh), and committing straight away also protects it from a mistake.

```bash
find corpus/owner/SLUG -iname '*unit_key*'
grep -rIl 'eyJhbGciOi' corpus/owner/SLUG
git add corpus/owner/SLUG
git status --short
git commit -m "Record SLUG for Task 15" -m "Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

Expected:
- `find` and `grep` print nothing: no AACS unit-key file, and no JSON Web Token in the recording (the TMDb read access token is one, and it starts with that text). If either prints a path, stop and do not commit.
- `git status --short` shows staged lines (a letter in the first column) only under `corpus/owner/SLUG/`:
  - `A ` for new files;
  - `M ` for `notes.md` and for re-pulled files of an earlier recording.
- The commit succeeds.

**1g. Is this the disc for experiment 2?**

Which disc is in the drive: unchanged from part 1e.

EXP2_SLUG is chosen once, by these rules:
- If EXP2_SLUG is not chosen yet, SLUG is not SLUG_A, `corpus/owner/SLUG/exp02` does not exist, SLUG's format is `bluray` (a Blu-ray or a UHD), and part 1e counted a `MSG:3308` or `MSG:3309` line, then EXP2_SLUG is SLUG. Do Step 2 now, while the disc is in the drive; a recorded disc that was reused without inserting it must be inserted first. Then come back here for the next disc of part 1b.
- A DVD is never EXP2_SLUG: a DVD scan has no source file names (attribute 16) for exp02_check.py to compare, so every title would read `match=NO`.
- Disc A (SLUG_A) is never EXP2_SLUG, and neither is any recording that has an `exp02` folder: experiment 2 already ran on it in Task 10, and a second run would count the same disc twice. If Disc A showed a `MSG:3308` or `MSG:3309` line, the condition is already met on it, and Step 2's Answer rules use that.
- When every disc of part 1b has been handled and no EXP2_SLUG was chosen, EXP2_SLUG is the disc of kind (a), if it is a Blu-ray or a UHD, is not SLUG_A and has no `exp02` folder. Step 2 then runs on it; the owner puts it back in the drive, and Step 2 checks that it is the right disc. If there is no such disc (the owner has none, or the disc of kind (a) is Disc A), Step 2 records that experiment 2 was not repeated.

Then ask the owner for the next disc of part 1b, and start again at part 1c. When every kind has been handled, go on to Step 2 (if it has not run yet) or Step 3.

- [ ] **Step 2: Repeat experiment 2 on a disc with angles or duplicate titles**

Which disc is in the drive: EXP2_SLUG's disc, from the start of this step to the end of Task 10 Step 5. Task 10's script checks it with Assert-DiscIs in the `-Plan` run of Task 10 Step 3, and again in the job before its first MakeMKV call. A run against another disc is refused (`is not the disc of recording EXP2_SLUG`) before any MakeMKV call. If the owner has changed discs since EXP2_SLUG was recorded, ask the owner to put it back first.

Run Task 10 Steps 3, 4, 5 and 6 as written there, with SLUG = EXP2_SLUG. In this task:
- Task 10 Steps 4 and 5 say, after each `wait.sh` call, what each exit code means and what to do. Exit 3 is normal there: run the same `wait.sh` line again, because the run takes about ESTIMATE minutes.
- Their log names are `exp02-EXP2_SLUG`, `exp02-EXP2_SLUG-b` and `exp02-EXP2_SLUG-redoK`. EXP2_SLUG is never SLUG_A, so none is the name of a log of Disc A's run, and no log is replaced.
- Where Task 10 says to take SLUG from the EXP-05 report, EXP2_SLUG comes from Step 1 of this task.
- Where Task 10 says "note it for Step 7" or "Step 7 reports it", note it for the EXP-02 addition below.
- Task 10 Steps 7 and 8 are not run. The report is extended below and committed at the end of this step.
- A disc with decoy playlists scans slowly, and each tested title costs one scan, so tell the owner the estimate that Task 10 Step 3 prints (ESTIMATE).

Then extend `docs/phase0/experiments/EXP-02-index-guard.md`. Keep everything Task 10 wrote about Disc A.
- Header: add `; EXP2_SLUG, <UHD or Blu-ray, as the owner said in its notes.md>, drive DRIVE: (Task 15)` to the `Disc(s):` line, and the date of this run to the `Date:` line.
- Under `## What was run`, add a paragraph that starts `**Second disc (Task 15): EXP2_SLUG.**` It gives:
  - why the disc was chosen: its scan's SCAN_3308 and SCAN_3309 counts, or "the decoy-playlist disc; no disc of Task 15 Step 1 other than Disc A showed a 3308 or 3309 line";
  - the flags in the `command` field of its run JSON;
  - the titles tested: PLANNED, which is n of M, sampled evenly with `-MaxTitles 12`. Say plainly that this is a sample, not every title index;
  - each redo, with its titles and the reason from Task 10 Step 5;
  - every title set aside in Task 10 Step 5, with its last messages.
- Under `## What was observed`, add a paragraph that starts `**Second disc (Task 15): EXP2_SLUG.**` It gives the same items that Task 10 Step 7 lists for Disc A:
  - the checker's output, pasted in full in a `text` block;
  - SCAN_3308 and SCAN_3309, and the checker's `3308-lines` and `3309-lines` columns;
  - WRITTEN_MAX;
  - any `MSG:1009`;
  - the `MSG:3308` parameter layout from Task 10 Step 6's probe, if there were such lines;
  - any `MSG:5010` straight after a stopped call.
- Replace the text under `## Answer` with an answer from both discs:
  - The experiment's condition, "a disc with angles or duplicate titles", is met when either disc shows a `3308` or `3309` line, in its scan or in the checker's columns. Say which disc meets it.
  - The answer is `no` when any title of either disc that reached `MSG:5014` has `match=NO` or `added-before-5014=NO`. Name the disc and the titles. A no stands whether or not the condition is met.
  - It is `yes` when the condition is met and every title of both discs that reached `MSG:5014` has `match=yes` and `added-before-5014=yes`.
  - When neither disc shows a `3308` or `3309` line and there is no mismatch, write exactly `yes on discs without angles or duplicate titles; the experiment's condition was not met; not verified`. Never write a plain yes in that case.
  - Name every title set aside on either disc. The answer covers only the titles that reached `MSG:5014`.
- Replace the text under `## Consequence for the spec`:
  - no: spec 6.2 step 6 drops the pre-read guard, as the fallback in section 17 row 2 says. Drift is then caught by verification after the read.
  - yes: the guard stays, and section 17 row 2 is marked answered.
  - not verified: the guard stays, and section 17 row 2 is marked "not yet verified: no disc with angles or duplicate titles was available".

If part 1g found no disc for experiment 2 (the owner has no Blu-ray or UHD other than Disc A with decoy playlists, angles or duplicate titles), do not run Task 10's steps. Instead:
- add the line `Task 15: no Blu-ray or UHD other than Disc A with decoy playlists, angles or duplicate titles was available; experiment 2 was not repeated.` under `## What was run`;
- apply the Answer and Consequence rules above to Disc A alone.

Then commit:

```bash
find corpus/owner/EXP2_SLUG -iname '*unit_key*'
grep -rIl 'eyJhbGciOi' corpus/owner/EXP2_SLUG
git add corpus/owner/EXP2_SLUG docs/phase0/experiments/EXP-02-index-guard.md
git status --short
git commit -m "Repeat experiment 2 on EXP2_SLUG" -m "Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

When experiment 2 was not repeated, skip `find` and `grep`, stage only the report, and commit it:

```bash
git add docs/phase0/experiments/EXP-02-index-guard.md
git status --short
git commit -m "Record that experiment 2 could not be repeated" -m "Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

Expected:
- `find` and `grep` print nothing. If either prints a path, stop and do not commit.
- `git status --short` shows as staged (when experiment 2 was not repeated, only the changed report):
  - the new files under `corpus/owner/EXP2_SLUG/exp02/`: the `title-<n>.txt` files, any `title-<n>.txt.stderr`, the `run-<time>.json` files and any `superseded/` files;
  - the changed report.
- The commit succeeds.

Tell the owner that the experiment is finished and the disc may come out. Then go back to Step 1, part 1g, for the next disc, or go on to Step 3 when Step 1 is complete.

- [ ] **Step 3: Check the DVD's flags**

Which disc is in the drive: does not matter. This step reads DVD_SLUG's recording on the Mac.

DVD_SLUG is the DVD of kind (d) from Step 1. If the owner had no DVD at all, skip this step, and tell the controller that the two DVD items in spec 8.7 stay unverified.

```bash
python3 - DVD_SLUG <<'EOF'
import collections, pathlib, sys
sys.path.insert(0, "scripts/phase0")
from robot import parse
d = pathlib.Path("corpus/owner") / sys.argv[1]
scan = parse((d / "scan.txt").read_text(encoding="utf-8-sig", errors="replace"))
print(f"{d.name}: {scan.disc.get(1)}, {len(scan.titles)} titles")
print("audio and subtitle streams with flags, or with a conversion other than lossless:")
kinds = collections.Counter()
shown = 0
for (t, s), a in sorted(scan.streams.items()):
    if a.get(1) not in ("Audio", "Subtitles"):
        continue
    flags = int(a.get(22, "0") or 0)
    conversion = a.get(42, "")
    kinds[(a.get(1), a.get(6, ""), a.get(7, ""), conversion)] += 1
    if flags or (conversion and "lossless" not in conversion.lower()):
        bits = [b for b in (1, 2, 4, 256, 512, 1024, 2048, 4096) if flags & b]
        print(f"  title {t} stream {s}: {a.get(1)} lang={a.get(3, '')} codec={a.get(6, '')} / {a.get(7, '')} "
              f"flags={flags} {bits} conversion={conversion} tree={a.get(30, '')}")
        shown += 1
if not shown:
    print("  none")
print("all audio and subtitle streams, by type, codec and conversion:")
for (kind, short, full, conversion), n in sorted(kinds.items()):
    print(f"  {n:>4} x {kind} codec={short} / {full} conversion={conversion}")
EOF
```

DVD_SLUG is a command-line argument, so nothing inside the heredoc needs editing.

Expected:
- A first line with the disc type and the title count.
- One line per audio or subtitle stream that carries flags (attribute 22, with the set bits in brackets), or whose output conversion (attribute 42) is not lossless. Each line shows the stream's language, codec (short and long names) and tree text. In the pinned public DVD logs every attribute-42 value is `( Lossless conversion )` except on the closed-caption streams of `corpus/thediscdb/series/Firefly (2002)/2003-dvd-the-complete-series/disc01.txt` and `disc02.txt`, which read `( Lossy conversion )`. Spec 8.7 expects the closed-caption stream to be the one marked lossy.
- A summary of every audio and subtitle stream by type, codec and conversion. The closed-caption stream shows here even if its conversion text turns out to be lossless after all. Look for a codec that names closed captions, for example `CC` or `EIA-608`. In the pinned Firefly logs the closed-caption stream is a `Subtitles` stream with language `eng`, codec ID (attribute 5) `S_CC608/DVD`, short name `CC`, long name `Closed Captions`, flags 0 and conversion `( Lossy conversion )`; its tree text starts with `CC` and ends `English ( Lossy conversion )`. Those logs also show forced-only DVD subtitle streams with flags 4096 and tree text `(forced only)`. Look for the same in this disc's scan: whether MakeMKV 2.0.0 names and flags these streams the same way is what this scan checks.

Compare the flagged streams with the owner's account in the disc's `## Ground truth (Task 15)` section: are the flag 1 or 2 streams the commentary tracks the owner named? Is the flag 4096 stream the forced track? Then append this section to `corpus/owner/DVD_SLUG/notes.md`, below what is there:

```markdown
## DVD flags and closed captions (Task 15)

From scan.txt (attribute 22 flags, attribute 42 output conversion), checked against the owner's account above:
- Streams with flags: title <t> stream <s>: <Audio | Subtitles> <language> <codec> flags=<n> (<bits>): <what the owner says it is>; or "no audio or subtitle stream carries a flag"
- Director's comments (flag 1 or 2): <the streams, and whether they are the commentary tracks the owner named>; or "no stream carries flag 1 or 2"
- Forced subtitles (flag 4096 without 2048): <the streams, and whether the owner confirms them>; or "no stream carries flag 4096 without 2048"
- Closed-caption stream: title <t> stream <s>, codec <short> / <long>, conversion <attribute 42 text>, language <code>; or "no closed-caption stream appears in the scan"
```

These answer the two items that spec 8.7 marks as unverified on DVD: whether MakeMKV sets the forced flag (4096 without 2048), and how the closed-caption stream appears. Task 17 reads this section when it updates spec 8.7.

- [ ] **Step 4: Experiment 8, output encoding**

Which disc is in the drive: does not matter. This step reads the pulled recordings on the Mac.

Every `scan*.txt` file in a recording is `lazymkv-capture record`'s byte-for-byte copy of MakeMKV's standard output, so the bytes are MakeMKV's own. Job logs and terminal output are not: PowerShell decodes them with the OEM code page. Use only the pulled scan files. This check runs over every scan recorded, NONASCII_SLUG's among them:

```bash
python3 - <<'EOF'
import pathlib, re, sys
sys.path.insert(0, "scripts/phase0")
import check_encoding
owner = pathlib.Path("corpus/owner")
paths = sorted(set(owner.glob("*/scan*.txt")) | set(owner.glob("*/*/scan*.txt")))
QUOTES = {
    "backslash-escaped": r'\\"',
    "doubled": r'(?<=[^,:"])""(?=[^,"\r\n])',
    "bare, unescaped": r'(?<=[^,:"\\])"(?=[^,"\r\n])',
}
for path in paths:
    print("==", path)
    check_encoding.main(path)
    lines = path.read_bytes().decode("utf-8", "replace").splitlines()
    for kind, pattern in QUOTES.items():
        hits = [line for line in lines if re.search(pattern, line)]
        print(f"{kind} quotes inside a field: {len(hits)}")
        for line in hits[:2]:
            print("  ", line[:160])
print(len(paths), "scan files checked")
EOF
```

Expected, for each scan file (`scan.txt`, Disc A's `scan-offline.txt`, and the scan of Task 14's whole-disc copy):
- `== <path>`;
- check_encoding.py's lines: `valid UTF-8: yes` or `valid UTF-8: NO (...)`; then, for up to five lines with non-ASCII bytes, the line decoded as UTF-8 (`utf-8 :`) and as Windows-1252 (`cp1252:`); then `lines with non-ASCII bytes shown: <n>`, and its own count of backslash-escaped quotes;
- three counts of quotes inside a field, each with up to two example lines. A quote that opens or closes a field (after `,` or `:`, before `,` or the end of the line) is not counted.

The last line is `<n> scan files checked`.

If NONASCII_SLUG exists, also look at how the disc names itself, in the scan and in its own metadata file (a Blu-ray only):

```bash
grep '^CINFO:' corpus/owner/NONASCII_SLUG/scan.txt
grep -h -o '<di:name>[^<]*</di:name>' corpus/owner/NONASCII_SLUG/nav/BDMV/META/DL/*.xml
```

Expected: the disc-level lines of the scan (attribute 2 is the name MakeMKV gives the disc), and the title the disc's `bdmt_<language>.xml` files carry. A DVD has no such file, and grep then reports `No such file or directory`.

Create `docs/phase0/experiments/EXP-08-encoding.md` from `docs/phase0/experiments/TEMPLATE.md`:
- Header:
  - `Disc(s): NONASCII_SLUG (<format>), and every other recorded scan (<n> files), drive D:`, or `drives D: and DRIVE:` if DRIVE is not `D`. If the owner had no disc with a non-ASCII title, write `no disc with a non-ASCII title was available; every recorded scan (<n> files)`.
  - The MakeMKV version from the `MSG:1005` line.
  - `Spec reference: section 17, row 8`.
- Under "Question": row 8's question.
- Under "What was run": the check above, over the pulled `scan*.txt` files only, which are lazymkv-capture's byte-for-byte copies of MakeMKV's output. No job log and no terminal output was used.
- Under "What was observed":
  - In full, in a `text` block, the part of the output for every scan that has a non-ASCII byte or a quote inside a field.
  - For the other scans, their file names with "valid UTF-8, no non-ASCII bytes, no quotes inside a field".
  - For NONASCII_SLUG: its `CINFO` lines and its `bdmt` title. Say where the non-ASCII text appears (which line kinds and attributes), or that MakeMKV printed the title without it.
- Under "Answer". For the encoding:
  - Answer `robot output is UTF-8` (yes) only if some scan shows `valid UTF-8: yes` with `lines with non-ASCII bytes shown` of 1 or more, and on those lines the `utf-8 :` rendering reads correctly while the `cp1252:` rendering is garbled (two odd characters where the UTF-8 line shows one accented letter).
  - If a scan is not valid UTF-8, the answer is no. Name the encoding whose rendering reads correctly, usually the `cp1252:` line, and the scan it was seen in.
  - If no scan contains a non-ASCII byte, report the experiment as not run, even if a disc with a non-ASCII title was scanned. Say whether one was, and how MakeMKV printed its title.

  For the quotes:
  - Write which forms were seen: backslash-escaped, doubled or bare. Give an example line for each.
  - If none was seen, write `no quote inside a field was seen; escaping not observed`. Never write "no escaping".
- Under "Consequence for the spec":
  - yes: spec 9.4 states that robot output is UTF-8 (verified on MakeMKV 2.0.0), and names the quote forms seen. The parser keeps tolerating both backslash-escaped and doubled quotes.
  - no: spec 9.4 decodes robot output in the encoding observed.
  - not run: row 8's fallback stays ("decode as UTF-8, replacing invalid bytes; MakeMKV text is display-only already"), marked "not yet verified".
  - A bare, unescaped quote inside a field breaks the field splitting of spec 9.4 and of `robot.py`'s `split_fields`. If one was seen, say so and tell the controller.

- [ ] **Step 5: Experiment 10, TheDiscDB hit rate**

Which disc is in the drive: each further disc in turn, inserted by the owner. For a repeated lookup, the disc concerned.

First count the distinct discs recorded so far:

```bash
python3 -c "import json,pathlib; fps={(json.loads(p.read_text(encoding='utf-8-sig')).get('fingerprint') or '').upper() for p in pathlib.Path('corpus/owner').glob('*/meta.json')}; fps.discard(''); print(len(fps), 'distinct discs recorded')"
```

Expected: `<n> distinct discs recorded`. Ask the owner for 20 minus n further discs (more is fine). They should be a sample of the owner's collection, not chosen by whether TheDiscDB might know them, with UHDs, Blu-rays and DVDs as the collection has them. If n is already 20 or more, go straight to the count below.

For each further disc (about a minute each):
1. Ask the owner to insert it, to say its title, year and format, and to say when the light has stopped blinking.
2. Run the fingerprint check of Step 1 part 1c (both commands together, in one shell).
   - `already recorded as S`: the disc is counted already. Ask for another disc.
   - `not recorded yet`: choose SLUG, as Placeholders says.
   - `NO FINGERPRINT`: as in part 1c.
3. Record its fingerprint and lookups, without a scan:

```bash
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\record-disc.ps1 -Slug SLUG -NoScan"; echo "exit=$?"
```

Expected:

```text
disc: label=<volume label or (none)> format=<bluray|dvd> fingerprint=<the fingerprint part 1c printed> disc_id=<40 hex characters, or empty> files=<N>
lookups: dischash <code>[; discid <code>][; tmdb <movie|tv>/<id> <code>][; tmdb not tried: <reason>]
RESULT ok
exit=0
```

record-disc.ps1 waits up to two minutes for a freshly inserted disc to become readable before it reads anything, so a disc that is still spinning up is not misread. Other results:
- `exit=1` with `RESULT FAILED: no readable disc in DRIVE: after 120 s`: nothing was written. Ask the owner whether the disc is in and the light has stopped, then run the same line again. Note for the report how often this happened.
- `exit=1` with `lazymkv-capture disc failed (exit N)` or `the disc has no fingerprint ...`: nothing was kept in the recording. Ask the owner to clean the disc and insert it again, then run the same line once more. If it fails again, leave the disc out, note its title for the report, and take the next one.
- `exit=1` with Assert-Slug's rejection: SLUG is malformed. Choose a valid slug.
- `exit=1` with `is not the disc of recording SLUG` (Assert-DiscIs): the mini PC already holds another disc under this slug. Choose another new slug.
- `disc: kept from the earlier recording of this same disc: ...` and `RESULT ok`: the mini PC holds this disc under SLUG already, but it was never pulled. Go on with the pull.
- `exit=255`: ssh could not reach the mini PC. Run the line again once it can.
- Anything else: stop and report.

4. If the `lookups:` line shows a `dischash` or `discid` code other than `200` or `404`, run the same line with `-Force` added, once, now, while the disc is still in (see "When -Force is right"). The expected output is the same.
5. Copy the recording to the Mac:

```bash
scripts/phase0/pull-recording.sh SLUG
```

Expected: `pulled corpus/owner/SLUG (<n> files copied, <n> files there now)`. If it fails, nothing on the Mac changed: run it again.

When the discs are recorded, count them and look them up. A disc recorded under two slugs (by different tasks) is counted once: the count below links one recording per distinct fingerprint into a fresh temporary folder and runs `hit_rate.py` on that folder. No recording is deleted for this. Run these commands together, in one shell:

```bash
HR="$(mktemp -d "${TMPDIR:-/tmp}/lazymkv-hit-rate.XXXXXX")"
python3 - "$HR" <<'EOF'
import collections, json, os, pathlib, sys
sys.path.insert(0, "scripts/phase0")
from hit_rate import lookup, statuses

out = pathlib.Path(sys.argv[1])
SUFFIX = {"uhd": "UHD", "bluray": "Blu-ray", "dvd": "DVD"}

def kind(d, meta):
    """UHD, Blu-ray or DVD, and what says so."""
    if meta.get("format") == "dvd":
        return "DVD", "meta.json"
    nav = d / "nav"
    for p in (nav.rglob("*") if nav.is_dir() else []):
        if p.name.lower() == "index.bdmv" and p.parent.name.upper() == "BDMV":
            version = p.read_bytes()[4:8].decode("ascii", "replace")
            if version == "0300":
                return "UHD", "index.bdmv 0300"
            if version in ("0100", "0200", "0240"):
                return "Blu-ray", "index.bdmv " + version
    suffix = d.name.rsplit("-", 1)[-1]
    if suffix in SUFFIX:
        return SUFFIX[suffix], "slug suffix"
    return "unknown", "no index.bdmv and no format suffix"

by_fp = collections.defaultdict(list)
for meta_path in sorted(pathlib.Path("corpus/owner").glob("*/meta.json")):
    meta = json.loads(meta_path.read_text(encoding="utf-8-sig"))
    by_fp[(meta.get("fingerprint") or "").upper()].append((meta_path.parent, meta))

rows = []
for fp, recs in sorted(by_fp.items(), key=lambda item: item[1][0][0].name):
    if not fp:
        print("NO FINGERPRINT, not counted:", [d.name for d, _ in recs])
        continue
    answered = [r for r in recs if statuses(r[0]).get("dischash") in ("200", "404")]
    d, meta = (answered or recs)[0]
    if len(recs) > 1:
        print("one disc under several slugs:", [r[0].name for r in recs], "- counted once, as", d.name)
    os.symlink(d.resolve(), out / d.name)
    k, source = kind(d, meta)
    suffix = d.name.rsplit("-", 1)[-1]
    if suffix in SUFFIX and SUFFIX[suffix] != k:
        print(f"CHECK: {d.name} ends in -{suffix}, but {source} says {k}: ask the owner")
    status = statuses(d)
    rows.append((d.name, k, source, lookup(d / "thediscdb-hash.json", status.get("dischash")),
                 lookup(d / "thediscdb-discid.json", status.get("discid"))))
print()
for name, k, source, by_hash, by_id in rows:
    print(f"{name:<40} {k:<8} ({source}) hash={by_hash:<6} discid={by_id}")
print()
print(len(rows), "distinct discs")
for k in ("UHD", "Blu-ray", "DVD", "unknown"):
    mine = [r for r in rows if r[1] == k]
    if mine:
        answered = [r for r in mine if r[3] in ("hit", "miss")]
        hits = sum(1 for r in answered if r[3] == "hit")
        either = sum(1 for r in mine if "hit" in (r[3], r[4]))
        print(f"{k}: {len(mine)} discs; known by fingerprint: {hits} of {len(answered)} answered; "
              f"by fingerprint or disc ID: {either} of {len(mine)}")
EOF
python3 scripts/phase0/hit_rate.py "$HR"; echo "hit_rate exit=$?"
```

Expected from the first command:
- `one disc under several slugs: [...] - counted once, as <slug>` for each disc recorded twice. The slug counted is one whose fingerprint lookup got an answer.
- `NO FINGERPRINT, not counted: [...]` should not appear: record-disc.ps1 never keeps a recording without a fingerprint. If it does, stop and report.
- One row per distinct disc, with its kind and what says so. `meta.json` cannot tell a UHD from a Blu-ray, so the kind comes from:
  - the version in the disc's own `BDMV/index.bdmv` (kept in the recording's `nav` folder): `0300` is a UHD, and `0100`, `0200` or `0240` a Blu-ray;
  - otherwise from the slug's suffix.
- A `CHECK:` line means that the suffix and `index.bdmv` disagree. Ask the owner what the disc is, use the answer in the split, and say so in the report.
- `<n> distinct discs`, then one line per kind: how many discs, how many of the answered fingerprint lookups were hits, and how many discs are known by fingerprint or disc ID.

Expected from hit_rate.py:
- one row per distinct disc (`<slug> <format> hash=<hit|miss|failed|n/a> discid=<hit|miss|failed|n/a>`);
- `TheDiscDB knows <x> of <y> discs by fingerprint` and `by fingerprint or disc ID: <z> of <n> discs`, with a note when lookups failed;
- `hit_rate exit=0` or `hit_rate exit=1`.

`hit_rate exit=1` is not a command failure. It means that a row shows `hash=failed`, `hash=n/a` or `discid=failed`. `discid=n/a` is normal: no disc-ID lookup was made, as on a DVD, and it does not cause exit 1. For each disc with such a row, once:
1. Ask the owner to insert it again.
2. Run the fingerprint check of part 1c. One of its `already recorded as` lines must name that row's slug; otherwise the wrong disc is in the drive.
3. Run the record line above for that row's slug with `-Force` added.
4. Pull it.

Then run the count again (all three commands, so that a new temporary folder is made). Report the share from a run that exited 0. If a lookup still gets no answer, report how many stayed unanswered, and say that the share is computed without them.

Create `docs/phase0/experiments/EXP-10-thediscdb-hit-rate.md` from `docs/phase0/experiments/TEMPLATE.md`:
- Header:
  - `Disc(s): <n> distinct discs of the owner's, listed below, drive D:`, or `drives D: and DRIVE:` if DRIVE is not `D`.
  - `MakeMKV version: not used (the fingerprint comes from lazymkv-capture's file listing)`.
  - `Spec reference: section 17, row 10`.
- Under "Question": row 10's question.
- Under "What was run":
  - `record-disc.ps1 -NoScan` over ssh for the discs of this step, and the recordings of Tasks 8 to 14 and of Step 1 of this task;
  - the lookups, which record-disc.ps1 makes against TheDiscDB's dischash and discid endpoints;
  - the de-duplicating count and `hit_rate.py`;
  - every repeat with `-Force`, with its slug;
  - discs left out because they could not be read, with their titles, and how often a disc was not yet readable when the script started.
- Under "What was observed":
  - the first command's output (duplicates, `CHECK:` lines, the table with kinds and the split by kind);
  - hit_rate.py's table, its summary lines and its exit code;
  - the lookups that stayed unanswered.
- Under "Answer":
  - `TheDiscDB knows <x> of <y> of the owner's discs by fingerprint (<x/y as a percentage>)`, from the de-duplicated run. Also give the figure by fingerprint or disc ID, and the split by UHD, Blu-ray and DVD.
  - State the number of distinct discs. If it is below 20, write `only <n> discs were available, fewer than the 20 the experiment asks for`.
  - If lookups stayed unanswered, say how many, and that the share is computed without them.
- Under "Consequence for the spec": no design change (section 17 row 10). Section 18's risk line "TheDiscDB coverage is small and mostly US releases" is replaced by the measured share, by kind. A low share makes the TMDb-plus-structure route the main one and sets test priorities, as row 10 says.

- [ ] **Step 6: Update experiment 12 if a later disc answered it**

Which disc is in the drive: does not matter. This step reads the recordings on the Mac.

If part 1a found that EXP-12's answer is already yes (Disc A answered it), skip this step.

Otherwise list every Blu-ray or UHD recording that has a scan:

```bash
python3 - <<'EOF'
import json, pathlib, sys
sys.path.insert(0, "scripts/phase0")
from robot import parse
for scan_path in sorted(pathlib.Path("corpus/owner").glob("*/scan.txt")):
    d = scan_path.parent
    if json.loads((d / "meta.json").read_text(encoding="utf-8-sig")).get("format") != "bluray":
        continue
    files = json.loads((d / "files.json").read_text(encoding="utf-8-sig"))
    jar = sum(1 for f in files if f["path"].upper().startswith("BDMV/JAR/"))
    version = "none"
    nav = d / "nav"
    for p in (nav.rglob("*") if nav.is_dir() else []):
        if p.name.lower() == "index.bdmv" and p.parent.name.upper() == "BDMV":
            version = p.read_bytes()[4:8].decode("ascii", "replace")
    scan = parse(scan_path.read_text(encoding="utf-8-sig", errors="replace"))
    java = any(code == 3344 for code, _, _ in scan.messages)
    labelled = [(t, a[49]) for t, a in sorted(scan.titles.items()) if a.get(49)]
    main = [t for t, comment in labelled if comment.startswith("FPL_MainFeature")]
    print(f"{d.name:<36} index.bdmv={version} BDMV/JAR files={jar} java_in_use(3344)={java} "
          f"labelled titles={len(labelled)} FPL_MainFeature titles={main}")
EOF
```

Expected: one line per Blu-ray or UHD recording with a scan, Disc A's among them. Each line gives:
- the `index.bdmv` version: `0300` is a UHD;
- the number of `BDMV/JAR/` files on the disc: above 0 means the disc has Java menus;
- whether the scan shows `MSG:3344`, the Java runtime in use;
- how many titles carry a comment (attribute 49);
- which titles' comments start `FPL_MainFeature`.

`MSG:3344` alone shows only that MakeMKV used a Java runtime, not that the disc has Java menus.

Add to `docs/phase0/experiments/EXP-12-java-labels.md`:
- in the header's `Disc(s):` line, the slugs of the UHDs listed;
- under "What was observed", a paragraph that starts `**Later discs (Task 15).**`, with the listing in a `text` block.

Then replace the text under "Answer". Only UHDs (`index.bdmv=0300`, or the owner says so in `notes.md`) with Java menus (`BDMV/JAR files` above 0) can answer the question:
- yes, `yes, on <slug>: MSG:3344, and title <t>'s comment starts FPL_MainFeature`, when such a UHD shows both;
- no, naming the discs and what their scans show, when at least one such UHD was scanned and none shows both;
- `yes on <slugs>, no on <slugs>` when both kinds were seen;
- `not answered: no UHD with Java menus was scanned; the condition was not met`, when the listing has no such UHD.

Blu-ray lines with labels or `MSG:3344` are observations. Report them, but they do not decide the answer.

Replace the text under "Consequence for the spec":
- yes: label rule 8.3.2 stands, with labels trusted only when `MSG:3344` is seen (spec 9.4);
- no: label rule 8.3.2 is unavailable, and titles are chosen by database and structure only (section 17 row 12);
- yes on some discs and no on others: rule 8.3.2 applies only when labels are present and `MSG:3344` is seen;
- not answered: the spec keeps rule 8.3.2, marked "not yet verified".

- [ ] **Step 7: Commit**

Which disc is in the drive: does not matter. Tell the owner that phase 0 needs no more discs; Tasks 16 and 17 use none.

```bash
find corpus/owner -iname '*unit_key*'
grep -rIl 'eyJhbGciOi' corpus/owner
git add corpus/owner docs/phase0/experiments
git status --short
git commit -m "Record the remaining discs; run experiments 8 and 10" -m "Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

Expected:
- `find` and `grep` print nothing: no AACS unit-key file and no JSON Web Token in any recording. If either prints a path, stop and do not commit.
- `git status --short` lists as staged:
  - the new fingerprint-only recordings of Step 5 (`A `);
  - re-pulled files of recordings whose lookups were repeated with `-Force` (`M `);
  - `corpus/owner/DVD_SLUG/notes.md` (`M `, from Step 3);
  - the new `EXP-08-encoding.md` and `EXP-10-thediscdb-hit-rate.md`;
  - `EXP-12-java-labels.md` if Step 6 changed it.

  The Step 1 recordings and the EXP-02 changes were committed in Steps 1 and 2.
- The commit succeeds.

---

### Task 16: Naming on the media servers (experiment 9)

**Files:**
- Create: `scripts/phase0/make-test-library.ps1`, `docs/phase0/experiments/EXP-09-naming.md`
- On the mini PC only, not in the repository: `C:\lazymkv\phase0\config.ps1` (its `$TestLibrary` line, Step 1), and the script's summaries `C:\lazymkv\phase0\exp09\summary-<run>.json` (a few hundred bytes each; the same text is printed in the job log).
- Prerequisite, applied and committed by the controller before this task: the env.ps1 helpers of decision D1. This task uses `Assert-Slug`.

**Interfaces:**
- Consumes:
  - From `env.ps1` (which loads `config.ps1`): `$P0`, `$FFMPEG`, `$TestLibrary`, `Assert-Slug`.
  - Scripts: `scripts/box.sh`, `scripts/phase0/job.sh <script.ps1> <log name> [arguments without quotes]`, `scripts/phase0/wait.sh <log name>`.
- Produces:
  - `scripts/phase0/make-test-library.ps1 [-Style combined|plex|jellyfin] [-Compat] [-Force]`. Run it as a job: `scripts/phase0/job.sh make-test-library.ps1 exp09-<run> [-Style <style>] [-Compat] [-Force]`. The share is reachable only from a job (fact 5).
    - It writes five short generated video files (8 seconds each, 2160p or 1080p) into `$TestLibrary`, named as spec 8.9 prescribes for the style:
      - `combined` (the default): `Alien`, `Inception` (2160p and 1080p) and `Blade Runner` (two cuts in one folder, `{edition-...}` in the file names);
      - `plex`: Blade Runner's cuts in one folder per edition;
      - `jellyfin`: Blade Runner's cuts in one folder, the cut name after the resolution without the `{edition-...}` tag;
      - `-Compat` adds the ` [tmdbid-ID]` tag of Jellyfin 10.x after every `{tmdb-ID}` (spec 8.9, `jellyfin10_compat`).
    - It refuses before anything is created, deleted or copied when:
      - `-Style` is not one of the three (compared case-sensitively);
      - `$TestLibrary` is empty;
      - `$TestLibrary` is not written as `\\unas-pro\media\<folder>[\<folder>...]` with names of letters, digits, `-` and `_` only (so another name for the NAS, such as an IP address, `unas-pro.local` or a mapped drive letter, is refused), or it is `\\unas-pro\media\Movies` or `\\unas-pro\media\Shows` or lies inside one of them;
      - `$TestLibrary` does not exist (the owner creates the folder; the script never does);
      - `$TestLibrary` holds anything other than the entries of its ignore list (`.DS_Store`, `Thumbs.db`, `desktop.ini`, `@eaDir`, `#recycle`, `$RECYCLE.BIN`, `System Volume Information`);
      - the run's summary already exists and `-Force` was not given.
    - It never overwrites or deletes a file in `$TestLibrary`. Both clips are generated on C: before the first copy, so a failed encode leaves the library untouched. The clips are removed from C: when the script ends.
    - Output lines: `library: ...` (what the folder held, hidden entries included), five lines `<full path>  <bytes> bytes`, the summary JSON, then `RESULT ok` or `RESULT FAILED: <reason>`. After a failure the script throws, so wait.sh exits 1.
    - Summary: `C:\lazymkv\phase0\exp09\summary-<run>.json` (UTF-8 with a BOM), with `ok`, `result`, `run`, `style`, `compat`, `library`, `started` and `files` (a list of `path` relative to the library and `bytes`). It is written whenever the library checks passed, for a success and for a failure.
  - `docs/phase0/experiments/EXP-09-naming.md`: the experiment 9 report.

Placeholders:
- SCRATCH: the session's scratch directory on the Mac (never `/tmp`). Only Step 1 uses it. A shell variable does not survive from one command to the next, so begin every command block of Step 1 that mentions `$SCRATCH` with the line `SCRATCH=<that directory>` (the blocks below show only the commands).
- TEST_LIBRARY: the full path the owner gives for the empty test folder in Step 1, in the form `\\unas-pro\media\<folder>`, for example `\\unas-pro\media\lazymkv-test-library`.
- PLEX_VERSION, PLEX_PASS (`yes` or `no`) and JELLYFIN_VERSION: the owner's answers in Step 1.
- STYLE: the layout of a run. It is `combined` in Step 4. In Step 6 it is `plex` when PLEX_PASS is `yes` and `jellyfin` when it is `no`, as spec 8.9 says. In Step 7 it is the layout the report settles on.
- RUN: the name of one run of the script: STYLE, with `-compat` added when `-Compat` is used (`combined`, `plex`, `combined-compat`, and so on). The job's log is `exp09-RUN`, and its summary is `C:\lazymkv\phase0\exp09\summary-RUN.json`.
- No step of this task needs a disc or uses the drive. Whatever is in the drive stays there.

When `-Force` is right (decision D3):
- `make-test-library.ps1 ... -Force`: only to repeat a run whose summary `C:\lazymkv\phase0\exp09\summary-RUN.json` says `"ok": false`, which means the run failed after the library checks passed. It is used once the cause has been dealt with and the owner has emptied the test folder (the script refuses a folder that is not empty). Check with `ssh USER@makemkv-box "type C:\lazymkv\phase0\exp09\summary-RUN.json"`.
- Never over a summary that says `"ok": true`, and in no other case. A finished run is not repeated; if it must be, stop and report.

- [ ] **Step 1: Fix the test library folder and ask for the server versions**

Which disc is in the drive: none needed.

Ask the owner, in chat:
1. "Please create an empty folder on the NAS for the naming test. It must not be inside `\\unas-pro\media\Movies` or `\\unas-pro\media\Shows`, and nothing else may be in it. What is its full path, written as `\\unas-pro\media\<folder>` (not by IP address, `unas-pro.local` or a drive letter), with names made of letters, digits, `-` and `_` only?" (Task 7 suggested `\\unas-pro\media\lazymkv-test-library`.) The owner creates the folder: the script refuses a folder that is missing, not empty, inside Movies or Shows, or written in any other form, and never creates one.
2. "Which Plex Media Server version is running, does your Plex account have Plex Pass, and which Jellyfin version is running (Dashboard, then About)?"

If the path in answer 1 is not of the form `\\unas-pro\media\<folder>[\<folder>...]` with such names, or is inside Movies or Shows, ask again: the script would refuse it (another name for the NAS could lead into Movies or Shows past its check, which the owner's rules forbid). Write down TEST_LIBRARY, PLEX_VERSION, PLEX_PASS and JELLYFIN_VERSION. Then show the current config file:

```bash
ssh USER@makemkv-box "type C:\lazymkv\phase0\config.ps1"
```

Expected: three lines, `$Staging = '...'`, `$Drive = '...'` and `$TestLibrary = '...'`. The third may be empty (`''`) or an example path. If it already equals TEST_LIBRARY exactly, skip the rest of this step.

Otherwise change only the `$TestLibrary` line. Do not retype the other two lines (Task 14 may have changed `$Drive`) and do not edit the file on the mini PC any other way: every script dot-sources it, and a broken file stops every later job. The new file is built on the Mac from the old one and copied over:

```bash
ssh USER@makemkv-box "type C:\lazymkv\phase0\config.ps1" | tr -d '\r' > "$SCRATCH/config-old.ps1"
grep -c -E '^\$(Staging|Drive) = ' "$SCRATCH/config-old.ps1"
```

Expected: `2`. Anything else means the file is not what Task 7 wrote, or the ssh call failed: stop and report. Then build the new file. Replace TEST_LIBRARY in the heredoc line with the owner's path, with its backslashes as they are (the heredoc is quoted, so bash leaves them alone):

```bash
{ grep -v 'TestLibrary' "$SCRATCH/config-old.ps1"; cat <<'LINE'
$TestLibrary = 'TEST_LIBRARY'
LINE
} > "$SCRATCH/config.ps1"
cat "$SCRATCH/config.ps1"
```

Expected: three lines: `$Staging`, `$Drive` exactly as shown before, and `$TestLibrary = '<the owner's path>'`. Then copy it over and read it back:

```bash
scp -q "$SCRATCH/config.ps1" USER@makemkv-box:C:/lazymkv/phase0/config.ps1
ssh USER@makemkv-box "type C:\lazymkv\phase0\config.ps1"
```

Expected: the same three lines. If `scp` fails, the old file is unchanged: run it again.

- [ ] **Step 2: Write the script**

Which disc is in the drive: none needed.

`scripts/phase0/make-test-library.ps1`:

```powershell
# Experiment 9: generated files named as spec 8.9 prescribes, for the owner to add to Plex and Jellyfin.
# Run as a job (the share is reachable only from one):
#   scripts/phase0/job.sh make-test-library.ps1 exp09-<run> [-Style combined|plex|jellyfin] [-Compat] [-Force]
# <run> is the style, with "-compat" added when -Compat is given; it names the log and the summary.
# -Compat adds the " [tmdbid-ID]" tag that Jellyfin 10.x needs (spec 8.9, jellyfin10_compat).
# $TestLibrary must be an existing, empty folder that the owner made for this test, written as
# \\unas-pro\media\<folder>[\<folder>...] and outside \\unas-pro\media\Movies and \\unas-pro\media\Shows.
# The script never creates it, never overwrites a file and never deletes anything in it. Both clips are made
# on C: before the first copy, so a failed encode leaves the library untouched; the clips are removed when the
# script ends.
# The summary $P0\exp09\summary-<run>.json is evidence: an existing one is refused unless -Force is given.
# The last line is "RESULT ok" or "RESULT FAILED: <reason>"; after a failure the script throws.
param(
    [string]$Style = 'combined',
    [switch]$Compat,
    [switch]$Force
)
. C:\lazymkv\src\scripts\phase0\env.ps1

# One 8-second clip of the given size. ffmpeg's exit code and the output file are both checked.
function New-Clip([string]$Size, [string]$Out) {
    & $FFMPEG -nostdin -v error -y -f lavfi -i "testsrc2=size=${Size}:rate=24" -f lavfi -i 'sine=frequency=440:sample_rate=48000' -t 8 -c:v libx264 -preset ultrafast -crf 30 -pix_fmt yuv420p -c:a aac $Out | Out-Null
    $code = $LASTEXITCODE
    if ($code -ne 0 -or -not (Test-Path -LiteralPath $Out) -or (Get-Item -LiteralPath $Out).Length -eq 0) { throw "ffmpeg did not write $Out (exit $code)" }
}

# .NET calls: a bracket in a name ([tmdbid-...] with -Compat) is never read as a wildcard, File.Copy never
# overwrites, and a failure is always a terminating error.
function Place([string]$Source, [string]$Relative) {
    $destination = [System.IO.Path]::Combine($root, $Relative)
    [System.IO.Directory]::CreateDirectory([System.IO.Path]::GetDirectoryName($destination)) | Out-Null
    [System.IO.File]::Copy($Source, $destination, $false)
    $length = (Get-Item -LiteralPath $destination -ErrorAction Stop).Length
    if ($length -ne (Get-Item -LiteralPath $Source -ErrorAction Stop).Length) { throw "short copy: $destination" }
    [void]$placed.Add([pscustomobject]@{ path = $destination; bytes = $length })
}

function Tagged([string]$Title, [int]$TmdbId) {
    if ($Compat) { "$Title {tmdb-$TmdbId} [tmdbid-$TmdbId]" } else { "$Title {tmdb-$TmdbId}" }
}

$failure = $null
$summary = $null
$placed = New-Object System.Collections.ArrayList
$tmp = "$P0\tmp\exp09"
try {
    if (@('combined', 'plex', 'jellyfin') -cnotcontains $Style) { throw "-Style must be combined, plex or jellyfin, not '$Style'" }
    $run = $Style
    if ($Compat) { $run = "$Style-compat" }
    Assert-Slug $run
    if (-not $FFMPEG) { throw 'ffmpeg.exe not found' }

    # The library is checked before anything is created, copied or deleted.
    if (-not $TestLibrary) { throw 'Set $TestLibrary in C:\lazymkv\phase0\config.ps1 first.' }
    $root = $TestLibrary.TrimEnd('\')
    # Nothing may be written into Movies or Shows (owner's rule). A plain string comparison misses another name for the
    # same place (an IP address, unas-pro.local, a mapped drive letter, \\?\UNC\..., a "/", a "." or ".." segment,
    # a name Windows trims), so only the one form this check can judge is accepted. -match ignores case.
    if ($root -notmatch '^\\\\unas-pro\\media(\\[A-Za-z0-9_-]+)+$' -or $root -match '^\\\\unas-pro\\media\\(Movies|Shows)(\\|$)') {
        throw "$root is not a folder of the form \\unas-pro\media\<folder> outside Movies and Shows (names of letters, digits, - and _ only); give the test folder by that path"
    }
    if (-not (Test-Path -LiteralPath $root -PathType Container)) { throw "$root does not exist or cannot be reached; the owner creates the empty test folder first" }
    $entries = @(Get-ChildItem -LiteralPath $root -Force -ErrorAction Stop)
    $seen = 'none'
    if ($entries.Count -gt 0) { $seen = ($entries | Select-Object -First 10 | ForEach-Object { $_.Name }) -join ', ' }
    "library: $root ($($entries.Count) entries, hidden ones included): $seen"
    $ignore = '.DS_Store', 'Thumbs.db', 'desktop.ini', '@eaDir', '#recycle', '$RECYCLE.BIN', 'System Volume Information'
    $present = @($entries | Where-Object { $ignore -notcontains $_.Name } | ForEach-Object { $_.Name })
    if ($present.Count -gt 0) {
        $shown = ($present | Select-Object -First 5) -join ', '
        throw "$root is not empty (found: $shown); it must be the empty folder the owner made for this test"
    }
    $expDir = "$P0\exp09"
    $summaryFile = "$expDir\summary-$run.json"
    if ((Test-Path -LiteralPath $summaryFile) -and -not $Force) {
        throw "$summaryFile already exists and is kept as evidence; use -Force only where the plan says so"
    }

    New-Item -ItemType Directory -Force -Path $expDir -ErrorAction Stop | Out-Null
    New-Item -ItemType Directory -Force -Path $tmp -ErrorAction Stop | Out-Null
    $summary = [ordered]@{
        ok = $false; result = $null; run = $run; style = $Style; compat = [bool]$Compat; library = $root
        started = (Get-Date).ToString('yyyy-MM-dd HH:mm:ss'); files = @()
    }

    # Both clips first: nothing is copied to the share unless both exist.
    New-Clip '3840x2160' "$tmp\2160.mkv"
    New-Clip '1920x1080' "$tmp\1080.mkv"

    $alien = Tagged 'Alien (1979)' 348
    $inception = Tagged 'Inception (2010)' 27205
    $blade = Tagged 'Blade Runner (1982)' 78

    # A lone file with a resolution label.
    Place "$tmp\2160.mkv" "$alien\$alien - 2160p.mkv"
    # Two resolutions of one film: versions (D11).
    Place "$tmp\2160.mkv" "$inception\$inception - 2160p.mkv"
    Place "$tmp\1080.mkv" "$inception\$inception - 1080p.mkv"
    # Two cuts in the chosen edition style (spec 8.9).
    foreach ($cut in 'Theatrical Cut', 'Final Cut') {
        switch ($Style) {
            'combined' { Place "$tmp\2160.mkv" "$blade\$blade - 2160p $cut {edition-$cut}.mkv" }
            'plex'     { Place "$tmp\2160.mkv" "$blade {edition-$cut}\$blade {edition-$cut} - 2160p.mkv" }
            'jellyfin' { Place "$tmp\2160.mkv" "$blade\$blade - 2160p $cut.mkv" }
        }
    }
} catch {
    $failure = $_.Exception.Message
}

Remove-Item -LiteralPath "$tmp\2160.mkv", "$tmp\1080.mkv" -Force -ErrorAction SilentlyContinue
if ($null -ne $summary) {
    try {
        foreach ($file in $placed) { '{0}  {1:N0} bytes' -f $file.path, $file.bytes }
        $summary['files'] = @($placed | ForEach-Object { [ordered]@{ path = $_.path.Substring($root.Length + 1); bytes = $_.bytes } })
        if (-not $failure -and $placed.Count -ne 5) { $failure = "expected 5 files, placed $($placed.Count)" }
    } catch {
        if (-not $failure) { $failure = "listing the files failed: $($_.Exception.Message)" }
    }
    $summary['ok'] = -not $failure
    $summary['result'] = if ($failure) { "FAILED: $failure" } else { 'ok' }
    $json = $summary | ConvertTo-Json -Depth 4
    try { Set-Content -LiteralPath $summaryFile -Value $json -Encoding UTF8 -ErrorAction Stop } catch { if (-not $failure) { $failure = "cannot write ${summaryFile}: $($_.Exception.Message)" } }
    $json
}
if ($failure) {
    "RESULT FAILED: $failure"
    throw "make-test-library.ps1: $failure"
}
'RESULT ok'
```

The script does not set `$ErrorActionPreference = 'Stop'`: inside a job, that would turn any line a native program writes to standard error into a failure. Each cmdlet that must not fail silently has `-ErrorAction Stop`, the copies are .NET calls that throw by themselves, and ffmpeg's exit code and output file are checked. `ffmpeg -v error` writes nothing on a clean run (probed).

- [ ] **Step 3: Sync and check the script without writing to the library**

Which disc is in the drive: none needed.

The Mac cannot run PowerShell, so the script's first parse happens on the mini PC. Nothing in this step reaches the share: the bad `-Style` is refused before the library is looked at.

```bash
grep -n 'function Assert-Slug' scripts/phase0/env.ps1
scripts/box.sh
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -Command \"(Get-Command C:\lazymkv\src\scripts\phase0\make-test-library.ps1).Parameters.Keys\""
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\make-test-library.ps1 -Style bogus"; echo "exit=$?"
ssh USER@makemkv-box "if exist C:\lazymkv\phase0\exp09 (echo CREATED) else (echo absent)"
```

Expected, in order:
- `grep` prints one line. With none, the env.ps1 prerequisites are missing: stop and report.
- `box.sh` syncs the tree and exits 0.
- The parameter list is `Style`, `Compat`, `Force` (PowerShell's common parameters such as `Verbose` may also be listed; they do not matter). An error message instead of the list is a parse error, which is a defect in Step 2: fix it before going on.
- The `bogus` run prints `RESULT FAILED: -Style must be combined, plex or jellyfin, not 'bogus'`, then PowerShell's error text, then `exit=1`. If the message says that `Assert-Slug` is not recognized, the env.ps1 prerequisites are missing. If env.ps1 itself fails, `config.ps1` is broken: go back to Step 1.
- The last command prints `absent` (this is before the first real run of this task). Nothing was created.

- [ ] **Step 4: Generate the library in the combined layout**

Which disc is in the drive: none needed.

The owner's test folder must exist and be empty (Step 1). Run the job and wait for it:

```bash
scripts/phase0/job.sh make-test-library.ps1 exp09-combined
scripts/phase0/wait.sh exp09-combined
```

job.sh prints `started make-test-library.ps1; log: C:\lazymkv\phase0\logs\exp09-combined.txt`. If job.sh exits 1 because a phase 0 job is still running, wait for that job with its own wait.sh line, then start this one. The owner must be logged in at the mini PC's console (global constraints). The job encodes two clips (a 2160p clip takes a minute or two on this CPU) and copies five small files.

What wait.sh's exit codes mean for this job:
- **0**: the script did not throw. Check the log against the Expected below.
- **1**: the script threw. Read the `RESULT FAILED:` line. Show nothing to the owner until the cause is fixed.
  - `Set $TestLibrary`, `is not a folder of the form`, `does not exist or cannot be reached`: nothing was written. Go back to Step 1 (for the last one, also ask the owner whether the folder exists and whether the NAS is up), then start the same job again.
  - `is not empty (found: ...)`: nothing was written. Read the names. If the owner put them there, the owner empties the folder or chooses another one (the agent deletes nothing in it), then the same job is started again. If every name is something the NAS keeps by itself in any folder, add those names to `$ignore` in the script, with the owner's agreement. Run `scripts/box.sh`, start the same job again, and note the names for the report.
  - `already exists and is kept as evidence`: a summary of this run exists. Read "When `-Force` is right" above; do not use `-Force` otherwise.
  - `ffmpeg.exe not found`, or `ffmpeg did not write ...`: the library was not touched. Read ffmpeg's error text above the RESULT line, then stop and report.
  - Anything else (a copy error such as `Access ... denied` or `The network name is no longer available`, `short copy`, `expected 5 files`): the library may hold some of the five files, and the summary says `"ok": false`. Stop and report with the log. A repeat needs the owner to empty the test folder first, and the same command with `-Force` added.
- **3**: the job is still running. This is normal if the encode is slow: run `scripts/phase0/wait.sh exp09-combined` again and start nothing else meanwhile. If 30 minutes pass without a DONE line, end the job with `ssh USER@makemkv-box "powershell -NoProfile -Command Stop-ScheduledTask -TaskName lazymkv-phase0"`, check that `ssh USER@makemkv-box "tasklist | findstr /i ffmpeg"` prints nothing, and stop and report.
- **4**: the job is not running and wrote no DONE line (it never started, or was killed). Stop and report.
- **5**: the mini PC cannot be reached. Run wait.sh again once it can be.

Expected (exit 0), in the log, in this order:
- `library: TEST_LIBRARY (0 entries, hidden ones included): none`. Entries from the ignore list may be named instead of `none`.
- Five lines `<full path>  <bytes> bytes`, in any order, none of them `0 bytes`:
  - `TEST_LIBRARY\Alien (1979) {tmdb-348}\Alien (1979) {tmdb-348} - 2160p.mkv`;
  - `TEST_LIBRARY\Inception (2010) {tmdb-27205}\Inception (2010) {tmdb-27205} - 2160p.mkv`;
  - `TEST_LIBRARY\Inception (2010) {tmdb-27205}\Inception (2010) {tmdb-27205} - 1080p.mkv`;
  - `TEST_LIBRARY\Blade Runner (1982) {tmdb-78}\Blade Runner (1982) {tmdb-78} - 2160p Theatrical Cut {edition-Theatrical Cut}.mkv`;
  - `TEST_LIBRARY\Blade Runner (1982) {tmdb-78}\Blade Runner (1982) {tmdb-78} - 2160p Final Cut {edition-Final Cut}.mkv`.
- The summary JSON, with `"ok": true`, `"run": "combined"` and five entries under `files`.
- `RESULT ok`, then `PHASE0-DONE status=ok exit=<n>`. `exit=<n>` is only the last native command's exit code, not a verdict.

Keep the five `<full path>  <bytes> bytes` lines for the report.

- [ ] **Step 5: Ask the owner to check both servers**

Which disc is in the drive: none needed.

Only after Step 4 ended with `RESULT ok`. Ask the owner to add TEST_LIBRARY as a movie library in Plex and in Jellyfin (a Movies library in each, by the path each server sees the folder under), to let both scan it, and to answer for each server:

1. Does "Alien" appear once, with the right poster and title?
2. Does "Inception" appear once, with two versions to choose from (2160p and 1080p)?
3. Does "Blade Runner" appear once with two editions or versions, named "Theatrical Cut" and "Final Cut"? If not, how does it appear (for example as two films, as one film with unnamed versions, or not at all)?
4. Is any of the three films missing altogether, or has the server said why (for example that it skipped a very short file)? The clips are 8 seconds long.

The versions and the Plex Pass answer were asked in Step 1; if the owner now corrects them, use the new values. Write down the answers per server in the owner's words.

If the owner says a film is missing and no reason can be found, answer question 1 or 2 with no, and note in the report that the 8-second length was not ruled out as the cause. Tell the owner that the test library can stay until Task 17's clean-up, where they decide what to remove.

What happens next:
- 1 or 2 is no on either server: skip Steps 6 and 7 and go to Step 8. After the commit, stop and return to the owner, as spec section 17 row 9 says.
- 1 and 2 are yes on both servers and 3 is no on either: Step 6, then Step 7 if JELLYFIN_VERSION is 10.x.
- 1 to 3 are all yes on both servers: skip Step 6. Step 7 follows only if JELLYFIN_VERSION is 10.x.

- [ ] **Step 6: If question 3 was no, confirm with the layout the spec names**

Which disc is in the drive: none needed.

Spec 8.9 says the default becomes `plex` when the owner has Plex Pass, else `jellyfin`. STYLE is that layout. Ask the owner to:
- empty the test folder (all five film folders; the agent deletes nothing there);
- empty the Plex trash for the library and scan it again;
- scan the Jellyfin library again, so that the old items disappear.

Then run the script with that layout. The script refuses a folder that is not empty. The run has its own log and summary names, so the first run's evidence is kept (no `-Force` is needed):

```bash
scripts/phase0/job.sh make-test-library.ps1 exp09-STYLE -Style STYLE
scripts/phase0/wait.sh exp09-STYLE
```

What wait.sh's exit codes mean for this job:
- **0**: the same Expected as Step 4, except that the Blade Runner lines follow STYLE's layout (spec 8.9): for `plex`, a folder per edition, `TEST_LIBRARY\Blade Runner (1982) {tmdb-78} {edition-Final Cut}\Blade Runner (1982) {tmdb-78} {edition-Final Cut} - 2160p.mkv` and the same for Theatrical Cut; for `jellyfin`, one folder, `...\Blade Runner (1982) {tmdb-78}\Blade Runner (1982) {tmdb-78} - 2160p Final Cut.mkv` and the same for Theatrical Cut. The summary says `"run": "STYLE"`.
- **1**: read the `RESULT FAILED:` line as in Step 4. `is not empty` here means the owner has not emptied the folder yet: ask again, then start the same job again.
- **3**: still running. Run `scripts/phase0/wait.sh exp09-STYLE` again; the 30-minute rule of Step 4 applies.
- **4**: stop and report.
- **5**: run wait.sh again once the mini PC can be reached.

After exit 0, have the owner check question 3 only, on both servers, and write down the answers. If question 3 is still no on either server with this layout, record that and stop and report to the owner: neither edition layout has then been shown to work, and the choice is theirs.

- [ ] **Step 7: If Jellyfin is 10.x, optionally check the names it needs**

Which disc is in the drive: none needed.

Only if JELLYFIN_VERSION is 10.x and Steps 5 and 6 did not end in a stop. Jellyfin 10.x does not read the `{tmdb-ID}` tag, and spec 8.9's `jellyfin10_compat` adds `[tmdbid-ID]` after it. This run shows how both servers treat those names. It is extra evidence and does not change the answer of the plain layout. Ask the owner first, since it means emptying the folder and rescanning once more. If the owner declines, write "not run: owner declined" in the report.

STYLE is the layout the report settles on: `combined`, or the layout confirmed in Step 6. Ask the owner to empty the test folder, empty the Plex trash and scan again, and to scan Jellyfin again, as in Step 6. Then:

```bash
scripts/phase0/job.sh make-test-library.ps1 exp09-STYLE-compat -Style STYLE -Compat
scripts/phase0/wait.sh exp09-STYLE-compat
```

What wait.sh's exit codes mean for this job:
- **0**: as Step 4's Expected, with ` [tmdbid-348]`, ` [tmdbid-27205]` and ` [tmdbid-78]` after each `{tmdb-ID}` in the folder and file names, and `"compat": true` in the summary.
- **1**: read the `RESULT FAILED:` line as in Step 4. `is not empty` means the owner has not emptied the folder yet.
- **3**: still running. Run `scripts/phase0/wait.sh exp09-STYLE-compat` again; the 30-minute rule of Step 4 applies.
- **4**: stop and report.
- **5**: run wait.sh again once the mini PC can be reached.

After exit 0, have the owner answer questions 1 to 3 again on both servers, and write the answers down as the compat run's. A no here is reported to the owner in the report's consequence section.

- [ ] **Step 8: Write the report**

Which disc is in the drive: none needed.

Create `docs/phase0/experiments/EXP-09-naming.md` from `docs/phase0/experiments/TEMPLATE.md`. Under "Disc(s)" write `none: generated files`, and under "MakeMKV version" `not used`. It must state:

- What was run:
  - `make-test-library.ps1` with its arguments for each run (`exp09-combined`, and `exp09-STYLE` and `exp09-STYLE-compat` when they ran), the log names and the summaries' `ok` values;
  - the five `<full path>  <bytes> bytes` lines of the combined run, from Step 4;
  - the layout of each later run, if any;
  - the clip facts: 8 seconds, 3840x2160 and 1920x1080, H.264 and AAC in Matroska.
- What was observed:
  - the owner's answers to questions 1 to 4 per server, in the owner's words, for the combined run;
  - for Step 6, STYLE and the answers to question 3 on both servers;
  - for Step 7, the answers to questions 1 to 3, or "not run: owner declined" or "not run: Jellyfin is not 10.x";
  - PLEX_VERSION, PLEX_PASS and JELLYFIN_VERSION;
  - any names the NAS keeps in an empty folder that were added to the script's ignore list;
  - whether the 8-second length was ruled out as a cause where a film was missing.

Answer yes only if 1 to 3 hold on both servers for the combined run. Consequences:
- On a no for 3, the default `edition_style` becomes `plex` or `jellyfin` as spec 8.9 says, and Step 6's confirmation run states whether that layout shows correctly.
- On a no for 1 or 2, stop and return to the owner, as spec section 17 row 9 says: D11 and spec 8.9 are revisited before phase 2.
- Record PLEX_VERSION, PLEX_PASS and JELLYFIN_VERSION as the value of spec section 19's open item ("Which media servers the owner runs and their versions, and whether Plex Pass is available"). Task 17 writes them into the spec.
- If Step 7 ran with a no on either server, say which names misbehaved. That bears on `jellyfin10_compat`.

- [ ] **Step 9: Commit**

Which disc is in the drive: does not matter.

```bash
git add scripts/phase0/make-test-library.ps1 docs/phase0/experiments/EXP-09-naming.md
git status --short
git commit -m "Run experiment 9: naming on the media servers" -m "Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

Expected:
- `git status --short` lists only the two files above (the script is new; if `$ignore` was extended in Step 4, it is the extended version).
- The commit succeeds. If Step 5 ended with a no for 1 or 2, the commit is still made, because the report is the result; then stop and return to the owner before Task 17.

---

### Task 17: Replay check, results into the spec, and clean-up

**Files:**
- Create: `corpus/scenarios/disc-a-scan/makemkvcon64.json`, `corpus/scenarios/disc-a-scan/scan.txt`, `corpus/scenarios/disc-a-scan/scan.txt.timing.jsonl` (Step 1)
- Create: `scripts/phase0/list-leftovers.ps1` (Step 5), `docs/phase0/REPORT.md` (Step 3; Step 7 adds its last section)
- Modify: `docs/superpowers/specs/2026-10-04-lazymkv-design.md` (Step 2)
- On the mini PC only, not in the repository: `C:\lazymkv\phase0\tmp\makemkvcon64.exe` (a copy of the stand-in) and `C:\lazymkv\phase0\tmp\replayed.txt` (Step 1), and the job logs `final-list` and `final-list-2` (Steps 5 and 6).
- Prerequisite, applied and committed by the controller before this task: the env.ps1 variables of decision D1. This task uses `$P0`, `$Recordings`, `$Staging`, `$TestLibrary` and `$TuningData` only.

**Interfaces:**
- Consumes:
  - The twelve experiment reports in `docs/phase0/experiments/`: `EXP-01-profile-and-order.md`, `EXP-02-index-guard.md`, `EXP-03-whole-disc-copy.md`, `EXP-04-rip-to-share.md`, `EXP-05-offline-and-unit-key.md`, `EXP-06-audio-profiles-and-subtitles.md`, `EXP-07-eject.md`, `EXP-08-encoding.md`, `EXP-09-naming.md`, `EXP-10-thediscdb-hit-rate.md`, `EXP-11-language-selection.md`, `EXP-12-java-labels.md`. Each follows `TEMPLATE.md`, so each has a `## Answer` and a `## Consequence for the spec`.
  - `docs/phase0/environment.md` (its `## Installed for phase 0`, `## Optical drives` and `## Fingerprint check (Task 8)` sections) and `corpus/owner/<slug>/notes.md` (written in Tasks 12 and 15).
  - Disc A's recording from Task 9: `corpus/owner/SLUG_A/scan.txt` and `scan.txt.timing.jsonl` (the online scan, made with `Get-MkFlags`), and on the mini PC the original `C:\lazymkv\phase0\recordings\SLUG_A\scan.txt`.
  - The stand-in: `cargo build --release -p fake-tools` writes `C:\lazymkv\target\release\fake-tool.exe`. It takes the tool's name from its own file name (`makemkvcon64.exe` reads `makemkvcon64.json` in the folder named by the environment variable `LAZYMKV_FAKE_DIR`), prints the file named by the first response whose `args_contain` strings all occur in its arguments, and, when `LAZYMKV_FAKE_SPEED` is above 0, waits that fraction of each line's recorded arrival time before printing it.
  - From `env.ps1`: `$P0`, `$Recordings`, `$Staging`, `$TestLibrary`, `$TuningData`.
  - Scripts: `scripts/box.sh [command...]`, `scripts/phase0/job.sh <script.ps1> <log name>`, `scripts/phase0/wait.sh <log name>`, `scripts/phase0/summarize_scan.py <recording dir>`.
  - `scripts/phase0/audio_dataset.py` (its last list: the titles not yet in the tuning dataset).
- Produces:
  - `corpus/scenarios/disc-a-scan/`: a stand-in scenario made from a real scan: `makemkvcon64.json` (one response: arguments containing `info` and `disc:0` print `scan.txt` with the timing of `scan.txt.timing.jsonl`), `scan.txt` and `scan.txt.timing.jsonl`. Phase 1's tests point `LAZYMKV_FAKE_DIR` at this folder.
  - `scripts/phase0/list-leftovers.ps1` (no parameters; read-only). Run it as a job: `scripts/phase0/job.sh list-leftovers.ps1 <log name>`. It prints:
    - `staging: <path>`, then one line for each folder directly under the staging folder (its size in GB), one line for each item directly inside it (a folder with its size in GB, or a file with its size in bytes), and `staging holds <n> folder(s)`;
    - `test library: <path>` and one line for each file in it (`<full path>  <bytes> bytes`), or `test library: (not set in config.ps1)` or `test library: <path> (does not exist)`;
    - `tuning dataset: <path>: <n> GB in <k> title folder(s)` (`$TuningData`; `(does not exist)` when missing);
    - one line each for `C:\lazymkv\phase0\tmp`, `C:\lazymkv\phase0\exp09`, `C:\lazymkv\phase0\recordings` and `C:\lazymkv\phase0\logs` (`<path>: <n> MB`, or `<path>: absent`);
    - `firewall rule lazymkv-exp05 (0 = gone): <n>`;
    - last `RESULT ok` or `RESULT FAILED: <reason>`. After a failure it throws, so wait.sh exits 1.
  - `docs/superpowers/specs/2026-10-04-lazymkv-design.md` stating measured facts where it stated assumptions: the spec phase 1's plan is written against.
  - `docs/phase0/REPORT.md`, the phase summary, with a record of what the experiments left behind and what the owner decided about it.
  - The owner's decisions on the clean-up, carried out by the owner.

Placeholders:
- SLUG_A: Disc A's slug, chosen in Task 9 and written in the `Disc(s):` line at the top of the EXP-05 and EXP-12 reports (`ls corpus/owner` shows it). Step 1 uses its online scan `scan.txt`, never `scan-offline.txt`. Every command that names SLUG_A is run with the real slug.
- SCRATCH: the session's scratch directory on the Mac, as the agent's system prompt names it (never `/tmp`, never inside the repository). Only Step 4 uses it. A shell variable does not survive from one command to the next, so begin the command block of Step 4 with the line `SCRATCH=<that directory>`.
- NN: an experiment's number as two digits (`01` to `12`), as in its report's file name `EXP-NN-<name>.md`.
- The capitalised fill-ins of the REPORT.md skeleton (Step 3: DATE, ANSWER_1 to ANSWER_12, SECTIONS, DISC_SLUG, DISC_FORMAT, DISC_COVERS, NOT_DONE, INSTALLS) and of its last section (Step 7: REPLAY_RESULT, RUST_RESULT, PY_RESULT, TASK_STATE, FIREWALL_STATE) are each defined next to the skeleton they fill.
- STAGING, FOLDER, LABEL, TEST_LIBRARY (Steps 5 and 6): STAGING is the path printed after `staging: ` on the first line of the `final-list` log, FOLDER a folder name printed directly under it, LABEL a folder name printed under that FOLDER, TEST_LIBRARY the path printed after `test library: `. The agent copies them from the log, never from memory.
- No step of this task needs a disc or uses the drive. Whatever is in the drive stays there.

Decision D3 does not need a `-Force` here: the one script of this task writes nothing, and the scenario folder holds copies of evidence whose originals stay in `corpus/owner/SLUG_A/`.

- [ ] **Step 1: Replay a real recording through the stand-in**

Which disc is in the drive: none needed. The check runs the stand-in, not MakeMKV, and does not touch the drive. No job is running at this point (the last one ended in Task 16), so `scripts/box.sh` may replace `C:\lazymkv\src`.

Create the scenario folder with copies of Disc A's online scan and its timing. SLUG_A is Disc A's slug from Task 9, and the file is `scan.txt`, not `scan-offline.txt`:

```bash
ls corpus/owner/SLUG_A/scan.txt corpus/owner/SLUG_A/scan.txt.timing.jsonl
mkdir -p corpus/scenarios/disc-a-scan
cp corpus/owner/SLUG_A/scan.txt corpus/scenarios/disc-a-scan/scan.txt
cp corpus/owner/SLUG_A/scan.txt.timing.jsonl corpus/scenarios/disc-a-scan/scan.txt.timing.jsonl
cat > corpus/scenarios/disc-a-scan/makemkvcon64.json <<'EOF'
{"responses": [
  {"args_contain": ["info", "disc:0"], "stdout": "scan.txt", "timing": "scan.txt.timing.jsonl"}
]}
EOF
python3 scripts/phase0/summarize_scan.py corpus/scenarios/disc-a-scan
python3 - <<'EOF'
import json, pathlib
folder = pathlib.Path("corpus/scenarios/disc-a-scan")
scan = (folder / "scan.txt").read_bytes()
lines = scan.count(b"\n") + (0 if scan.endswith(b"\n") or not scan else 1)
records = [json.loads(line) for line in (folder / "scan.txt.timing.jsonl").read_text(encoding="utf-8-sig").splitlines() if line.strip()]
print("scan lines:", lines, "timing records:", len(records), "last arrival (ms):", records[-1]["t_ms"] if records else "none")
EOF
```

Expected:
- `ls` lists both files. If one is missing, the recording was not pulled: run `scripts/phase0/pull-recording.sh SLUG_A` and start again.
- The summary starts `MakeMKV: MakeMKV v2.0.0 win(x64-release)` and has a title table with at least one row. A scan with no titles is the wrong file (the offline scan): check that SLUG_A's `scan.txt` is the online one before going on.
- The last command prints equal numbers for `scan lines:` and `timing records:`. The stand-in pairs the n-th line with the n-th timing record, so unequal numbers would misalign the replay: stop and report.

Now build the stand-in and replay the scan on the mini PC. `scripts/box.sh` copies the scenario folder too, because it syncs untracked files. `LAZYMKV_FAKE_SPEED=0.001` makes the stand-in wait 0.1% of each line's recorded time, so a ten-minute scan replays in under a second while the timing file is read and the waits run. The output is compared with the original recording on the mini PC, not with the Mac's copy, so the comparison also shows that the copy survived the trip through scp, git and box.sh:

```bash
scripts/box.sh cargo build --release -p fake-tools
ssh USER@makemkv-box "(if not exist C:\lazymkv\phase0\tmp mkdir C:\lazymkv\phase0\tmp) & copy /y C:\lazymkv\target\release\fake-tool.exe C:\lazymkv\phase0\tmp\makemkvcon64.exe >nul & set LAZYMKV_FAKE_DIR=C:\lazymkv\src\corpus\scenarios\disc-a-scan& set LAZYMKV_FAKE_SPEED=0.001& C:\lazymkv\phase0\tmp\makemkvcon64.exe -r info disc:0 > C:\lazymkv\phase0\tmp\replayed.txt & fc /b C:\lazymkv\phase0\tmp\replayed.txt C:\lazymkv\phase0\recordings\SLUG_A\scan.txt"; echo "exit=$?"
```

Expected:
- `box.sh` syncs the tree, then cargo ends with ``Finished `release` profile [optimized] target(s) in ...`` and box.sh exits 0.
- The ssh command prints `Comparing files ...` and `FC: no differences encountered`, then `exit=0`. `exit=` is the exit code of the last command in the chain, `fc`, so it is the verdict. The stand-in reproduces a real scan byte for byte. The check does not measure the delays; it shows that the timing path runs without error.

What the other results mean:
- A line printed before `Comparing files` came from the stand-in's standard error, for example `LAZYMKV_FAKE_DIR is not set` or `...makemkvcon64.json: The system cannot find the file specified`. Run `scripts/box.sh` again and repeat the ssh command. If it persists, stop and report.
- `exit=1` with differing bytes, or a line saying that one file is longer than the other: the stand-in or a copy is wrong. Stop and report, and do not edit the scan copy.
- `exit=2` with `cannot open`: a file is missing. The usual cause is a wrong SLUG_A in the last path. Check it and run the ssh command again.
- `exit=255`: ssh could not reach the mini PC. Run the line again once it can.

- [ ] **Step 2: Update the spec from the reports**

Which disc is in the drive: none needed. Nothing in this step touches the mini PC.

First read what the reports say. This prints, for each report that exists, its header lines, its `## Answer` and its `## Consequence for the spec`:

```bash
python3 - <<'EOF'
import pathlib
for path in sorted(pathlib.Path("docs/phase0/experiments").glob("EXP-[0-9][0-9]-*.md")):
    sections = {"header": []}
    name = "header"
    for line in path.read_text(encoding="utf-8-sig").split("\n"):
        if line.startswith("## "):
            name = line[3:].strip()
            sections[name] = []
        else:
            sections[name].append(line)
    print("=" * 72)
    print(path.name)
    for line in sections["header"]:
        if line.startswith("- "):
            print(line)
    for name in ("Answer", "Consequence for the spec"):
        print("## " + name)
        print("\n".join(sections.get(name, ["(missing)"])).strip())
EOF
```

Expected: one block for each of the twelve reports. A report that does not exist means that its experiment was never written up: treat the experiment as not run and tell the owner. `(missing)` under a heading means the report does not follow the template: read the report itself.

Then list every assumption mark the spec still carries, so that none is overlooked:

```bash
grep -n -i -e 'provisional' -e 'unverified' -e 'not yet' -e 'unmeasured' -e 'phase 0' docs/superpowers/specs/2026-10-04-lazymkv-design.md
```

Go through every line it prints. Either a report settled the point (change the line as below) or none did (the line stays).

The rules for editing `docs/superpowers/specs/2026-10-04-lazymkv-design.md`:
- Change only what a report supports. Where an experiment was not run, or its answer says "not answered", the spec keeps its assumption and says `Not yet verified (EXP-NN: <reason>)`.
- Where a report settled a point, say so in the spec's own words and name the report: `Verified in phase 0 (EXP-NN-<name>.md): <the fact>.` In a table cell, `(EXP-NN)` is enough.
- Where a report's answer is "no", apply the fallback that the experiment's row in section 17 names, everywhere the spec depends on the assumption. Find those places with `grep -n -i '<term>' docs/superpowers/specs/2026-10-04-lazymkv-design.md`.
- Put no token, password or `Unit_Key_RO.inf` content in the spec.

Two commands collect facts that sections 8.1 and 9.4 need. Run both now and keep their output.

The first prints every field path of TheDiscDB's pinned answers and of the owner's recorded answers, and the values of the fields that name a type or a format (for section 8.1):

```bash
python3 - <<'EOF'
import glob, json
paths, values = {}, {}
def walk(node, prefix):
    if isinstance(node, dict):
        for key, value in node.items():
            walk(value, prefix + "." + key)
    elif isinstance(node, list):
        for value in node:
            walk(value, prefix + "[]")
    else:
        name = prefix.lstrip("[]")
        paths[name] = paths.get(name, 0) + 1
        if name.rsplit(".", 1)[-1] in ("type", "audio_type", "format", "locale"):
            values.setdefault(name, set()).add(str(node))
files = sorted(glob.glob("corpus/thediscdb-api/*.json") + glob.glob("corpus/owner/*/thediscdb-*.json"))
used = 0
for name in files:
    try:
        data = json.load(open(name, encoding="utf-8-sig"))
    except ValueError:
        print("skipped (not JSON):", name)
        continue
    used += 1
    walk(data, "")
print(used, "answers read")
for name in sorted(paths):
    print(f"{paths[name]:6d} {name}")
for name in sorted(values):
    print("values of", name, sorted(values[name]))
EOF
```

The second checks the value formats of spec 9.4 against every MakeMKV output in the owner's recordings, and prints the message codes that the 9.4 table lacks (for section 9.4; the message-code lines include noise that is not acted on):

```bash
python3 - corpus/owner <<'EOF'
import pathlib, re, sys
sys.path.insert(0, "scripts/phase0")
from robot import parse, split_fields

# The value formats of spec 9.4, as regular expressions, and the message codes its table lists.
TITLE_FORMATS = {
    9: ("duration H:MM:SS", r"\d+:\d\d:\d\d"),
    11: ("size, decimal bytes", r"\d+"),
    26: ("segment map, comma-separated clip numbers", r"[\d,\-]+(\.\.\.)?"),
}
STREAM_FORMATS = {
    13: ("bitrate, a number with a unit", r"[\d.]+ \S+"),
    19: ("video size WxH", r"\d+x\d+"),
}
TABLE = {1005, 1009, 2003, 2019, 3307, 3308, 3309, 3344, 5003, 5004, 5037, 5005, 5036, 5010, 5014, 5015,
         5021, 5050, 5052, 5055, 5095, 5069, 5070, 5079}
seen, extra, versions = {}, {}, set()
for path in sorted(pathlib.Path(sys.argv[1]).rglob("*.txt")):
    text = path.read_text(encoding="utf-8-sig", errors="replace")
    if "MSG:" not in text:
        continue
    for raw in text.split("\n"):
        if raw.startswith("MSG:"):
            fields = split_fields(raw[4:].rstrip("\r"))
            try:
                code = int(fields[0])
            except (ValueError, IndexError):
                continue
            if code == 1005:
                versions.add(fields[3])
            elif code not in TABLE and code != 1003:
                extra.setdefault(code, (path, fields[3] if len(fields) > 3 else ""))
    scan = parse(text)
    values = [("TINFO", a, v) for attrs in scan.titles.values() for a, v in attrs.items()]
    values += [("SINFO", a, v) for attrs in scan.streams.values() for a, v in attrs.items()]
    for kind, attribute, value in values:
        formats = TITLE_FORMATS if kind == "TINFO" else STREAM_FORMATS
        if attribute in formats:
            row = seen.setdefault((kind, attribute), [0, 0, []])
            row[0] += 1
            if re.fullmatch(formats[attribute][1], value):
                row[1] += 1
            elif len(row[2]) < 3:
                row[2].append(value)
print("MakeMKV versions in these files:", sorted(versions))
for (kind, attribute), (total, good, bad) in sorted(seen.items()):
    formats = TITLE_FORMATS if kind == "TINFO" else STREAM_FORMATS
    print(f"{kind}:{attribute} {formats[attribute][0]}: {total} values, {good} match; examples that do not: {bad}")
for code in sorted(extra):
    print(f"code {code} is not in the 9.4 table: {extra[code][1][:70]!r} (first seen in {extra[code][0]})")
EOF
```

The changes, by section. These are the minimum. After them come the consequences that the reports themselves name and that are not listed here.

Facts about the environment:
- **Section 3.** The MakeMKV sentence (`MakeMKV 1.18.3 ... it is upgraded to 2.0.0 before recordings are captured (section 19)`) becomes the installed version: `MakeMKV 2.0.0` at the same path, upgraded from 1.18.3 on 2026-10-04. The drive sentence (`One is an HL-DT-ST BU40N reporting firmware 1.03`) becomes both drives as MakeMKV lists them, from `docs/phase0/environment.md` `## Optical drives`: drive 1 `BD-RE HL-DT-ST BD-RE BU40N 1.03 SERIAL`, and drive 2 as Task 14 recorded it. If drive 2 was not tested, say that.
- **Section 4.4.** Add a fourth column, `Installed (phase 0)`, to the table, with a cell in every row: MakeMKV 2.0.0; mkvmerge v100.0 (MKVToolNix 100.0.0); ffprobe 9.0.2; Java: the version in `environment.md` (Java 8, 1.8.0_471), with EXP-12's answer on whether MakeMKV used it. If EXP-06 made MediaInfo a detector, add a row for it (`mediainfo.exe`, what it detects, minimum version 26.05, installed 26.05). Otherwise leave it out.
- **Section 5.** In the Fingerprint bullet, replace `phase 0 checks it against known discs` with the `Outcome:` sentence of `docs/phase0/environment.md` `## Fingerprint check (Task 8)`. In the Disc ID bullet, add that the DVD disc ID is not yet implemented: phase 0's capture tool computes only the AACS disc ID, so a DVD is looked up by its fingerprint alone until it is.
- **Section 19.** In the first bullet say that the installs were made in phase 0, with the versions of 4.4. Replace the bullet `Which media servers the owner runs and their versions, and whether Plex Pass is available (for 8.9)` with the values from EXP-09's report: Plex Media Server version, Plex Pass yes or no, Jellyfin version.

Rules decided by experiments:
- **Section 6.2 step 6 and section 17 row 2** (EXP-02). One of:
  - The answer is yes and the report says the experiment's condition (a disc with angles or duplicate titles) was met: the guard stays. Add `Verified in phase 0 (EXP-02-index-guard.md)` with the disc, the number of titles tested and that angles or duplicates were present.
  - The answer is yes but the condition was not met on any disc: the guard stays, and step 6 gets `Not yet verified on a disc with angles or duplicate titles (EXP-02-index-guard.md).`
  - The answer is no: apply row 2's fallback. In step 6 remove the sentences from `While the call performs its own disc scan` through `retry, at most twice`, and say that a title that drifted is caught by the verification in step 7.
  - Not run or not answered: the guard stays, marked `Not yet verified`.
- **Section 6.3 and section 17 row 3** (EXP-03).
  - Yes: 6.3 stands. Add where `BDMV` lands in the copy (so that `info file:` and `mkv file:` are pointed at the right folder), the copy's throughput, and, if the report says Disc A had no Java menus, `Not yet verified on a UHD with Java menus`.
  - No: replace 6.3 by row 3's fallback, and search the spec for every other use of whole-disc copy mode (`grep -n -i -e 'whole-disc' -e 'backup' docs/superpowers/specs/2026-10-04-lazymkv-design.md`): sections 6.1, 8.3 to 8.8, 11, 16 (phase 3) and 18 all change.
- **Section 6.2 step 7** (EXP-01). State what the report found: the duration within 2 s, the chapter count against attribute 8 (see 9.3 below), and whether the output codec text agrees with attributes 41 and 42, including whether 41 appears only on converted streams.
- **Section 6.4 step 9** (EXP-06, question 1). The detector for Atmos and DTS:X: `ffprobe`, or MediaInfo for the format where only MediaInfo tells them apart, or neither (the report says which). Add the profile strings that identify each format, and say which format was not tested, if one was not.
- **Section 6.4 step 10** (EXP-01 and EXP-06). Where the subtitle counts come from: the header's statistics tags if EXP-01 shows that MakeMKV writes them, otherwise the evidence extraction as the step already says.
- **Section 8.1, last bullet.** Replace the sentence about TheDiscDB's fields being `mapped from responses saved in phase 0` with the actual JSON field names, from the first command's output. The pinned answers give this baseline; start from it and correct it with anything the owner's recordings add or contradict. A `dischash` answer is one object and a `discid` answer an array of such objects. Each carries `content_hash`, `global_disc_id`, `format` (`UHD`, `Blu-ray` or `Blu-Ray`, `DVD`), `media` (`type` is `Movie` or `Series`; `title`, `full_title`, `year`, `external_ids.tmdb`, `external_ids.imdb`), `release` (`title`, `year`, `slug`, `locale`, `region_code`, `upc`), `disc` (`index`, `name`, `slug`) and `titles[]`. Each title has `index`, `source_file`, `segment_map`, `duration` (`H:MM:SS`), `size` (bytes), `display_size`, `resolution`, `chapters[]` (`index`, `title`), and, only when TheDiscDB has described it, `file_name` and `item` (`title`, `type`, `season`, `episode`; `type` is `MainMovie`, `Episode`, `Extra`, `Featurette`, `DeletedScene`, `Trailer` or `Other`). Each track has `index`, `type` (`Video`, `Audio` or `Subtitles`), `name` (the codec name, for example `Dolby Digital`), and, where present, `audio_type` (the channel layout: `Stereo`, `Surround 5.1`, `Surround 7.1`), `language_code` (three letters), `language`, `description` (the text that spec 8.1's commentary, audio-description and SDH rules read), `resolution` and `aspect_ratio`. The main movie is the title whose `item.type` is `MainMovie`. Also add what EXP-05 found about `Unit_Key_RO.inf` here: if it was unreadable on either drive, a disc on that drive can only reach step 3 of the identification table (a fingerprint match, 0.95), never step 2.
- **Section 8.7 and section 14** (EXP-06, EXP-01 and the DVD notes).
  - Replace the provisional defaults `subs_same_min` (0.70), `subs_diff_max` (0.30) and `sub_clear_max_bytes` (64) with the measured values, in 8.7 and in the two rows of section 14 that carry them (`subs_same_min`, `subs_diff_max` share one row), and drop the word `provisional` and the phrase `Experiment 6 replaces the provisional defaults`. If the report answers `not answered` or `no clean separation`, the values stay provisional and the text says so; on no clean separation, 8.7's ambiguity rule is the report's fallback (any title with two or more full English tracks raises `subtitle_ambiguous`).
  - The parent of a forced-only stream (`confirmed in experiment 1`): keep the sentence if EXP-01's item 5 shows every forced-only stream directly follows a non-derived subtitle stream of its language; otherwise correct the rule to what was seen.
  - The DVD stream flagged forced (`whether MakeMKV sets this is checked on the DVD acceptance disc`) and the DVD closed-caption stream (`output conversion marked lossy, attribute 42`): replace them with what the DVD's notes say, from Task 15 Step 3 (`grep -n -i -e 'closed' -e 'flags' corpus/owner/*/notes.md`) and from EXP-06: which streams carried flags 1, 2, 4 or 4096, and how MakeMKV 2.0.0 shows the closed-caption stream (its codec and conversion text), and whether it aligned with the dialogue tracks. If no DVD with closed captions was recorded, write `Not yet verified`.
- **Section 8.9** (EXP-09).
  - Yes: keep `combined` as the default, and replace `unverified until experiment 9` in the table by the servers and versions it was verified on.
  - No on edition names: the default becomes `plex` when the owner has Plex Pass and `jellyfin` when not. Change the table's `(default)` mark, the sentence `If experiment 9 shows combined misbehaving ...`, and the `edition_style` default in section 14. State whether the confirmation run of that layout showed correctly.
  - Add what the `-Compat` run showed about `jellyfin10_compat`, or `Not yet verified (owner declined the run)`.
- **Section 8.10** (EXP-01 item 6). How a Dolby Vision title's video tracks appear in the output: one video track or two. Replace `experiment 1 records whether that is one video track or two` by the answer.
- **Section 9.3** (EXP-01). The job profile as verified: honoured or not (a no switches 9.3 to its fallback), and the chapter-count behaviour: MakeMKV's default profile has `insertFirstChapter00IfMissing="true"`, so say whether the output's chapter count equalled attribute 8 and what spec 6.2 step 7's check should therefore accept.
- **Section 9.4** (EXP-08 and the recordings).
  - State the output encoding and the quote escapes from EXP-08 (`robot output is UTF-8`, or the encoding seen), and which quote escapes appeared.
  - Confirm the value formats on MakeMKV 2.0.0 from the owner's recordings, with the second command's output. When every value matches and every version is 2.0.0, change `as seen in public logs, confirmed against 2.0.0 recordings in phase 0` to a statement that they were confirmed, with the number of recordings. A value that does not match is quoted in the spec as a correction. Add a row to the message table for each code that an experiment report gave a use (for example the codes of `NeedsNetwork` and of a share drop), and one sentence listing the other new codes as seen but not acted on. In 9.1, change `phase 0 re-verifies them against 2.0.0 recordings` to say that it did, where this output supports it.
- **Section 10.2** (EXP-05, EXP-04 and the others). Replace `Experiment 5 captures NeedsNetwork` with the message codes and texts EXP-05 found (or `NeedsNetwork stays Unknown`, as its consequence says). Put the message codes a share drop produced (EXP-04) into the `Output` row. Add the codes of any other class an experiment exercised: for example `MSG:2003` read errors seen in a rip, or the key and licence codes that EXP-05 met. A code that spec 9.4 already gives another meaning is not a trigger.
- **Section 14, `cache_mb`** (EXP-04). The working value: 1024 on yes, otherwise the largest value whose repeat answered yes. Say it was measured.
- **Section 17** (every report). Add a fourth column to the table: the header line becomes `| # | Question | If the answer is no | Result (phase 0) |` and the separator `|---|---|---|---|`. Each of the twelve rows gets a last cell with the result word (`yes`, `no`, `not answered` or `not run`), the report's one-sentence answer and the report's file name, for example `yes on both drives (EXP-07-eject.md)`.
- **Section 18** (EXP-04, EXP-03, EXP-10). Replace the unmeasured estimates by measurements: in `Network cost`, the measured file-phase time per title (EXP-04's evidence read, and the remux if it ran) instead of `my unmeasured estimate is 25 to 40 minutes per title`; in `Whole-disc copy`, the result of experiment 3 and the copy's measured throughput instead of `unverified on this setup until experiment 3`; in `TheDiscDB coverage`, EXP-10's share of discs known and the number of discs. A measurement that was not made stays an estimate and says `not yet verified`.

Other consequences that the reports name. Read each report's `## Consequence for the spec` once more and make sure each is applied somewhere:
- **Section 4.3, `Ejector`** (EXP-07): any step that failed while the eject still worked, such as the volume lock.
- **Section 8.3 rule 2, section 6.1 step 4 and the `ToolMissing` row of 10.2** (EXP-12): whether MakeMKV used the installed Java (`3344`) and whether the main-feature labels appeared. On no, label rule 8.3.2 is unavailable and titles are chosen by database and structure only.
- **Section 10.4** (EXP-04, EXP-05): whether `makemkvcon` exits after a share drop (for `exit_wait_min`), and, if the offline UHD scan ran for a long time, what that says about `scan_timeout_min`.
- **Section 16, item 1** (EXP-11): the rip-time selection by language stays a later feature with its facts, or is marked not buildable with this selection string, or `Not yet verified` with the parts not exercised.
- **Section 8.6** (EXP-06): the profile strings for the object-audio criterion.

Then check the result:

```bash
python3 - <<'EOF'
import pathlib, re
spec = pathlib.Path("docs/superpowers/specs/2026-10-04-lazymkv-design.md").read_text(encoding="utf-8")
reports = sorted(p.name for p in pathlib.Path("docs/phase0/experiments").glob("EXP-[0-9][0-9]-*.md"))
section = spec.split("## 17. Experiments (phase 0)", 1)[1].split("\n## ", 1)[0]
table = [line for line in section.split("\n") if line.startswith("|")]
odd = [line[:40] for line in table if len(line.strip().strip("|").split("|")) != 4]
print(len(table), "table lines (14 expected: header, separator, 12 rows); lines without 4 cells:", odd)
for line in table:
    match = re.match(r"\| (\d+) \|", line)
    if match:
        number = int(match.group(1))
        named = [name for name in reports if name.startswith("EXP-%02d-" % number) and name in line]
        print("row %d: report file %s" % (number, "named" if named else "NOT NAMED"))
EOF
grep -n -e 'MakeMKV 1.18.3 at' -e 'mapped from responses saved in phase 0' -e 'Experiment 5 captures' -e 'unmeasured estimate' -e 'unverified on this setup until experiment 3' docs/superpowers/specs/2026-10-04-lazymkv-design.md; echo "grep=$?"
```

Expected:
- `14 table lines ...; lines without 4 cells: []`, and twelve lines `row N: report file named`. A row that is `NOT NAMED` is missing its result cell or the right report file name.
- The `grep` prints nothing and `grep=1`. A line it prints is a stale assumption: change it. The one exception is a line that belongs to an experiment that was not run (for example the network-cost estimate when experiment 4 was not run): it may stay if it now says `not yet verified`.

- [ ] **Step 3: Write the phase report**

Which disc is in the drive: none needed. Nothing in this step touches the mini PC.

Create `docs/phase0/REPORT.md` from this skeleton. Every capitalised fill-in is replaced as described below the skeleton, and nothing else in the skeleton changes:

```markdown
# Phase 0 report

Date: DATE. The design spec is `docs/superpowers/specs/2026-10-04-lazymkv-design.md`; its section 17
lists the experiments. Each experiment has its own report in `docs/phase0/experiments/`; this page
summarises them.

## The twelve experiments

| # | Question | Answer | Report |
|---|---|---|---|
| 1 | Is the job profile honoured in robot mode: track order, output codecs, chapter count, forced-stream parent, Dolby Vision, statistics tags? | ANSWER_1 | `EXP-01-profile-and-order.md` |
| 2 | Do the title numbers MakeMKV prints while a rip scans the disc equal the index ripped, and come before saving starts? | ANSWER_2 | `EXP-02-index-guard.md` |
| 3 | Does `backup --decrypt` to a network path work on a UHD, and does `info file:` on the copy list the same titles? | ANSWER_3 | `EXP-03-whole-disc-copy.md` |
| 4 | Does a UHD rip straight to the share keep free RAM above 1 GB with no slow-write warning at `cache_mb` 1024; what are the throughput, the share-drop messages and the remux time? | ANSWER_4 | `EXP-04-rip-to-share.md` |
| 5 | What does MakeMKV report with its network blocked for a UHD it has never scanned, and can `Unit_Key_RO.inf` be read in both drives? | ANSWER_5 | `EXP-05-offline-and-unit-key.md` |
| 6 | Does `ffprobe` tell Atmos and DTS:X apart, and do the subtitle alignment values separate dialogue from commentary? | ANSWER_6 | `EXP-06-audio-profiles-and-subtitles.md` |
| 7 | Does programmatic eject work on both drives? | ANSWER_7 | `EXP-07-eject.md` |
| 8 | What encoding and quote escaping does robot output use for non-ASCII titles and quotes? | ANSWER_8 | `EXP-08-encoding.md` |
| 9 | Do the spec 8.9 file names show correctly on the owner's Plex and Jellyfin? | ANSWER_9 | `EXP-09-naming.md` |
| 10 | What share of about 20 of the owner's discs does TheDiscDB know? | ANSWER_10 | `EXP-10-thediscdb-hit-rate.md` |
| 11 | Does a profile that selects by language rip exactly the streams predicted from the scan, in scan order? | ANSWER_11 | `EXP-11-language-selection.md` |
| 12 | On a UHD with Java menus, does the scan report a Java runtime (`3344`) and main-feature labels? | ANSWER_12 | `EXP-12-java-labels.md` |

## Spec sections changed

SECTIONS

## Discs recorded

| Slug | Format | What it covers |
|---|---|---|
| DISC_SLUG | DISC_FORMAT | DISC_COVERS |

## What could not be done, and why

NOT_DONE

## Installs and other changes on the mini PC

INSTALLS
```

Three commands collect what the fill-ins need. Run them in this order, after Step 2's edits and before the commit, and keep their output.

The first compares the spec on disk with the committed one and prints the section headings the changes touched (for SECTIONS):

```bash
python3 - <<'EOF'
import difflib, pathlib, subprocess
path = "docs/superpowers/specs/2026-10-04-lazymkv-design.md"
old = subprocess.run(["git", "show", "HEAD:" + path], capture_output=True, text=True, check=True).stdout.split("\n")
new = pathlib.Path(path).read_text(encoding="utf-8").split("\n")
heading_of, current = [], "(before the first heading)"
for line in new:
    if line.startswith("#"):
        current = line
    heading_of.append(current)
changed = []
for tag, i1, i2, j1, j2 in difflib.SequenceMatcher(None, old, new, autojunk=False).get_opcodes():
    if tag == "equal":
        continue
    for j in range(j1, max(j2, j1 + 1)):
        heading = heading_of[min(j, len(new) - 1)]
        if heading not in changed:
            changed.append(heading)
print("\n".join(changed))
EOF
```

The second prints what each recording holds (for DISC_SLUG, DISC_FORMAT and DISC_COVERS):

```bash
python3 - <<'EOF'
import json, pathlib, re
for meta_path in sorted(pathlib.Path("corpus/owner").glob("*/meta.json")):
    folder = meta_path.parent
    meta = json.loads(meta_path.read_text(encoding="utf-8-sig"))
    scan = folder / "scan.txt"
    titles = "no scan"
    if scan.exists():
        found = re.search(r"(?m)^TCOUNT:(\d+)", scan.read_text(encoding="utf-8-sig", errors="replace"))
        titles = found.group(1) if found else "no TCOUNT line"
    subfolders = sorted(p.name for p in folder.iterdir() if p.is_dir() and p.name != "nav")
    print(f"{folder.name} | {meta.get('format')} | label {meta.get('volume_label')} | titles {titles} | folders: {' '.join(subfolders) or '-'} | notes.md: {'yes' if (folder / 'notes.md').exists() else 'no'}")
EOF
```

The third lists every place a report says that something was not done (for NOT_DONE):

```bash
grep -n -i -e 'not run' -e 'not answered' -e 'not tested' -e 'not yet verified' -e 'declined' -e 'could not' docs/phase0/experiments/EXP-*.md
```

The fill-ins:
- **DATE**: today's date, from `date +%Y-%m-%d`.
- **ANSWER_1 to ANSWER_12**: each report's answer, from Step 2's printout: the first sentence of its `## Answer`, starting with its yes, no, not answered or not run. Keep `not answered` and `not run` as the report words them, with the reason. A report that does not exist gives `not run: no report was written`.
- **SECTIONS**: one bullet per heading the first command printed, as `- <heading without the leading #s>` (for example `- 6.2 Direct mode`).
- **DISC_SLUG, DISC_FORMAT, DISC_COVERS**: one row for each line the second command printed. DISC_SLUG is the folder name and DISC_FORMAT the `format`. DISC_COVERS is what the disc served, from its folders (`scan-offline.txt` and `drives-offline.txt` are experiment 5; `exp02` is 2; `rehearsal` and `all-tracks` are 1, 4 and 6; `share-drop*` and `all-tracks-512` or `-256` are 4; `eng-only` is 11; `backup` and `extract` are 3; `drive2` is 5 and 7; `thediscdb-*.json` files are 10; a `scan.txt` with Java labels is 12) and from the owner's words in its `notes.md`, for example `commentary subtitles, DVD closed captions`. A recording with no scan is a fingerprint-only recording for experiment 10.
- **NOT_DONE**: what could not be done and why, one bullet each, naming the experiment, the step and the reason, taken from the reports and from the third command's output. Write `nothing` if the command printed nothing.
- **INSTALLS**: the installs made on the mini PC. Copy the table under `## Installed for phase 0` in `docs/phase0/environment.md`, with any row added to it since. Then add these bullets: the scheduled task `lazymkv-phase0` (registered by `scripts/phase0/run-interactive.ps1` for every job); the firewall rule `lazymkv-exp05` (added and removed by the owner in Task 9); the folders `C:\lazymkv\src`, `C:\lazymkv\target` and `C:\lazymkv\phase0` with `config.ps1` and the job profiles; the staging folder on the share; and the test library folder the owner made for experiment 9.

- [ ] **Step 4: Run every test once more**

Which disc is in the drive: does not matter. No test touches the drive. No job is running, so `scripts/box.sh` may replace `C:\lazymkv\src`.

```bash
SCRATCH=SCRATCH   # replace the value with the scratch directory (see Placeholders)
scripts/box.sh cargo test --workspace > "$SCRATCH/cargo-test.txt" 2>&1; echo "cargo test exit=$?"
grep -e 'Running' -e 'Doc-tests' -e 'test result' -e 'FAILED' -e 'panicked' "$SCRATCH/cargo-test.txt"
python3 -m unittest discover -s scripts/phase0 -p 'test_*.py' 2>&1 | tail -4
```

Expected:
- `cargo test exit=0`.
- `Running` lines for six test programs, each followed by a `test result: ok. <n> passed; 0 failed`, and one `Doc-tests` run with `0 passed`:
  - `lazymkv-core`: 8 unit tests (`src\lib.rs`: 5 in `fingerprint.rs`, 3 in `profile.rs`) and 1 corpus test (`tests\thediscdb_corpus.rs`);
  - `lazymkv-capture`: 7 unit tests (`src\main.rs`, which includes the Windows-only eject test) and 1 integration test (`tests\exit_code.rs`);
  - `fake-tools`: 2 unit tests (`src\main.rs`) and 1 integration test (`tests\replay.rs`).

  That is 20 tests, none failed. A later prerequisite may have added tests, so a higher count is fine. A lower count, a `FAILED` line or a `panicked` line is not: stop and report.
- unittest prints `Ran 40 tests in ...s` and `OK`. The 40 are the 29 of Task 7, 5 added by Task 12's `subtitle_alignment.py` prerequisite, 1 added by Task 13's `compare_tracks.py` prerequisite and 5 in `test_exp11_tracks.py`. A later prerequisite may have added more. Fewer than 40, or any failure: stop and report. Write the numbers down: they are RUST_RESULT and PY_RESULT in Step 7.

- [ ] **Step 5: Write the leftovers script and list what the experiments left**

Which disc is in the drive: does not matter. The script does not use the drive.

The experiments left rips, a whole-disc copy, extractions and a test library on the share, and small files on C:. The share cannot be listed from an ssh session (it gives `Access is denied`), so the listing runs as a job. The script is read-only.

`scripts/phase0/list-leftovers.ps1`:

```powershell
# Read-only: what the experiments left on the share and on the mini PC (Task 17). Run it as a job, because the
# share is reachable only from one:  scripts/phase0/job.sh list-leftovers.ps1 final-list
# It creates, changes and deletes nothing. A size is the sum of every file below the folder.
# The last line is "RESULT ok" or "RESULT FAILED: <reason>"; after a failure the script throws.
[CmdletBinding()]
param()
. C:\lazymkv\src\scripts\phase0\env.ps1

# The sum of the sizes of every file below a folder, in bytes (0 for an empty folder).
function Get-FolderBytes([string]$Path) {
    $sum = (Get-ChildItem -LiteralPath $Path -Recurse -File -Force -ErrorAction Stop | Measure-Object -Property Length -Sum).Sum
    if ($null -eq $sum) { return [long]0 }
    return [long]$sum
}

try {
    "staging: $Staging"
    $folders = 0
    foreach ($entry in @(Get-ChildItem -LiteralPath $Staging -Force -ErrorAction Stop | Sort-Object Name)) {
        if ($entry.PSIsContainer) {
            $folders++
            '  folder {0,-46} {1,8:N1} GB' -f $entry.Name, ((Get-FolderBytes $entry.FullName) / 1GB)
            foreach ($item in @(Get-ChildItem -LiteralPath $entry.FullName -Force -ErrorAction Stop | Sort-Object Name)) {
                if ($item.PSIsContainer) {
                    '      folder {0,-40} {1,8:N1} GB' -f $item.Name, ((Get-FolderBytes $item.FullName) / 1GB)
                } else {
                    '      file   {0,-40} {1,12:N0} bytes' -f $item.Name, $item.Length
                }
            }
        } else {
            '  file   {0,-46} {1,12:N0} bytes' -f $entry.Name, $entry.Length
        }
    }
    "staging holds $folders folder(s)"

    if (-not $TestLibrary) {
        'test library: (not set in config.ps1)'
    } elseif (-not (Test-Path -LiteralPath $TestLibrary -PathType Container)) {
        "test library: $TestLibrary (does not exist)"
    } else {
        "test library: $TestLibrary"
        foreach ($file in @(Get-ChildItem -LiteralPath $TestLibrary -Recurse -File -Force -ErrorAction Stop)) {
            '  {0}  {1:N0} bytes' -f $file.FullName, $file.Length
        }
    }

    if (Test-Path -LiteralPath $TuningData -PathType Container) {
        $titles = @(Get-ChildItem -LiteralPath $TuningData -Directory -Force -ErrorAction Stop | ForEach-Object { Get-ChildItem -LiteralPath $_.FullName -Directory -Force -ErrorAction Stop })
        'tuning dataset: {0}: {1:N1} GB in {2} title folder(s)' -f $TuningData, ((Get-FolderBytes $TuningData) / 1GB), $titles.Count
    } else {
        "tuning dataset: $TuningData (does not exist)"
    }

    foreach ($place in "$P0\tmp", "$P0\exp09", $Recordings, "$P0\logs") {
        if (Test-Path -LiteralPath $place) {
            '{0}: {1:N1} MB' -f $place, ((Get-FolderBytes $place) / 1MB)
        } else {
            "${place}: absent"
        }
    }

    $rules = @(Get-NetFirewallRule -DisplayName 'lazymkv-exp05' -ErrorAction SilentlyContinue)
    "firewall rule lazymkv-exp05 (0 = gone): $($rules.Count)"
    'RESULT ok'
} catch {
    "RESULT FAILED: $($_.Exception.Message)"
    throw
}
```

The script sets no `$ErrorActionPreference`: inside a job, `'Stop'` would turn any line a native program writes to standard error into a failure. Each listing that must not fail silently carries `-ErrorAction Stop`, so an unreachable share ends the job with `RESULT FAILED`. It registers nothing: it cannot report on the scheduled task, because every job runs through that task and `run-interactive.ps1` registers it again for each job.

Check that the file is plain ASCII, sync it, and check that it parses on the mini PC:

```bash
LC_ALL=C grep -n '[^ -~]' scripts/phase0/list-leftovers.ps1; echo "grep=$?"
scripts/box.sh
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -Command \"(Get-Command C:\lazymkv\src\scripts\phase0\list-leftovers.ps1).Parameters.Keys\""
```

Expected: `grep` prints nothing and `grep=1` (the file has no tab and no character outside printable ASCII: PowerShell 5.1 reads a file without a BOM in the ANSI code page). `box.sh` exits 0. The last command prints PowerShell's common parameters (`Verbose`, `Debug`, `ErrorAction`, ...) and no parse error. A parse error is a defect in this step's file: fix it and run the block again.

Now start the listing and read it:

```bash
scripts/phase0/job.sh list-leftovers.ps1 final-list
scripts/phase0/wait.sh final-list
ssh USER@makemkv-box "dir /s /b C:\lazymkv\phase0\tmp & schtasks /query /tn lazymkv-phase0 /fo list | findstr TaskName"
```

job.sh prints `started list-leftovers.ps1; log: C:\lazymkv\phase0\logs\final-list.txt`. If job.sh fails with `no such script`, run `scripts/box.sh` and start the job again. If it exits 1 because a phase 0 job is still running, wait for that job with its own wait.sh line first. The owner must be logged in at the mini PC's console (global constraints).

What wait.sh's exit codes mean for this job:
- **0**: the script did not throw. Check the log against the Expected below. `exit=<n>` on the `PHASE0-DONE` line is only the last native command's exit code, not a verdict.
- **1**: the script threw. Read the `RESULT FAILED:` line. A message about `Access is denied` or a path that cannot be found on the staging share means the share is not reachable from the owner's desktop session: ask the owner to open the share once in Explorer on the mini PC, then start the job again as `final-list-b`. Anything else: stop and report.
- **3**: no `PHASE0-DONE` line yet. This is normal: the script reads the size of every file on the share, which can take several minutes. Run `scripts/phase0/wait.sh final-list` again; start nothing else meanwhile. If 30 minutes pass without a DONE line, tell the owner and end the job with `ssh USER@makemkv-box "powershell -NoProfile -Command Stop-ScheduledTask -TaskName lazymkv-phase0"`, then stop and report.
- **4**: the job is not running and wrote no `PHASE0-DONE` line (it never started, or was killed). Check that the owner is logged in at the console, then stop and report.
- **5**: the mini PC cannot be reached. Run wait.sh again once it can be.

Expected (exit 0), in the log, in this order:

```text
staging: <STAGING>
  folder <slug>                                      <n> GB
      folder <label>                                 <n> GB
      file   <name>                                  <n> bytes
staging holds <n> folder(s)
test library: <TEST_LIBRARY>
  <full path>  <bytes> bytes
tuning dataset: <the value of $TuningData in config.ps1>: <n> GB in <k> title folder(s)
C:\lazymkv\phase0\tmp: <n> MB
C:\lazymkv\phase0\exp09: <n> MB
C:\lazymkv\phase0\recordings: <n> MB
C:\lazymkv\phase0\logs: <n> MB
firewall rule lazymkv-exp05 (0 = gone): 0
RESULT ok
PHASE0-DONE status=ok exit=<n>
```

The last line may read `PHASE0-DONE status=ok exit=` with nothing after the `=`, because this script runs no native program; `status=ok` and the `RESULT ok` line above it are what count.

- There is one `folder` line for each slug that a rip, copy or extraction used, with one indented line for each label folder (`rehearsal`, `all-tracks`, `share-drop`, `eng-only`, `backup`, `extract`, `share-drop-2`, `all-tracks-512` and so on) beneath it. Files inside a label folder, such as the ripped `.mkv` and `evidence.mks`, are counted in that folder's size and are not listed. A repeat with `-Force` reused its label folder on the share, so the `-failed-N` folders kept on the Mac have no line here. A folder that holds `exp02-tmp` means experiment 2's cleanup did not finish; note it.
- The `test library` line may read `(not set in config.ps1)` or `(does not exist)`; then there are no file lines. Otherwise there are five file lines: the files of the last Task 16 run (Task 16 Steps 6 and 7 have the owner empty the folder before each later run, so earlier runs' files are gone; their logs and summaries on the mini PC still list them). There is also a line for each file the NAS keeps in the folder by itself: `.DS_Store`, `Thumbs.db` or `desktop.ini`, and each file inside a folder such as `@eaDir` or `#recycle` (the listing includes hidden files and lists files only, not folders). If the owner has emptied the folder since, or did not run Task 16 at all, there are no file lines.
- A local folder that was never created reads `<path>: absent`.
- The `C:\lazymkv\phase0\tmp` line also counts a `tuning-fetch` folder that a fetch left behind (`tuning-fetch.ps1`; the Mac should have deleted it right after its scp); name it if `dir /s /b` shows one.
- If the `firewall` line is not `0`, the rule that blocks MakeMKV's network access is still there. Task 9 asked the owner to remove it. Ask the owner to remove it now, as Step 6 says.
- The ssh line prints the file names under `C:\lazymkv\phase0\tmp` (at least `makemkvcon64.exe` and `replayed.txt` from Step 1) and a line `TaskName:   \lazymkv-phase0`: the job that just ran registered the task.

- [ ] **Step 6: Clean up, with the owner's agreement**

Which disc is in the drive: does not matter.

The staging folders `STAGING\<slug>\` (rips, the whole-disc copy, extractions, evidence files), the test library folder, the scheduled task `lazymkv-phase0` and `C:\lazymkv\phase0\tmp` are all created by this plan. The rips may be wanted as inputs for phase 2, and a whole-disc copy of a UHD can be tens of gigabytes. The decision is the owner's, and so is the deletion: the agent deletes nothing (the owner's rules reserve permanent deletion to the owner, and changing the firewall or other system settings too). No `Remove-Item`, `rmdir`, `del` or firewall command is run by the agent here.

Before offering any staging folder for deletion, run `python3 scripts/phase0/audio_dataset.py | grep '^Titles whose audio'`. A rip it names under `missing` is not offered: its audio is not in the tuning dataset yet (decision D16); add it first with Task 11 Step 11, or ask the owner to keep it. Never write a command for `$TuningData` or anything under it: the dataset is kept until the owner decides otherwise, separately from this clean-up.

Show the owner the `final-list` log and ask, item by item, what to keep and what to delete. Group the questions by what the data is good for, and give the sizes from the log. For each item the owner agrees to delete, write out the exact command below, with the real names copied from the log, and ask the owner to run it in an administrator PowerShell at the mini PC's desktop and to say when it is done. One named folder per command. Before sending a command, check that its path appears, character for character, as a folder line in the log:

```powershell
Remove-Item -LiteralPath 'STAGING\FOLDER\LABEL' -Recurse -Force
Remove-Item -LiteralPath 'STAGING\FOLDER' -Recurse -Force
Remove-Item -LiteralPath 'TEST_LIBRARY' -Recurse -Force
Remove-Item -LiteralPath 'C:\lazymkv\phase0\tmp' -Recurse -Force
```

- Never write a command for STAGING itself, for an empty FOLDER, for `$TuningData` or anything under it, or for anything under `\\unas-pro\media\Movies` or `\\unas-pro\media\Shows`.
- The first command removes one label folder, the second a whole slug folder. The third removes the test library folder, which the owner made; if the owner wants to keep the empty folder, they empty it by hand in Explorer instead. Before it goes, ask the owner to remove that library from Plex and from Jellyfin and to empty the Plex trash.
- The fourth removes the stand-in copy and the replayed scan from Step 1 and everything else the scripts left in `tmp`.
- Leave `C:\lazymkv\phase0\recordings` and `logs` alone unless the owner asks: they are small, and the recordings are also in `corpus/owner`.

When the owner says the deletions are done, list again, to show on the share what is gone and what is kept:

```bash
scripts/phase0/job.sh list-leftovers.ps1 final-list-2
scripts/phase0/wait.sh final-list-2
```

What wait.sh's exit codes mean for this job: the same as in Step 5, with `final-list-2` in place of `final-list` (a start again after exit 1 uses `final-list-2b`). Expected (exit 0): the same listing as `final-list`, without the folders the owner deleted. If a deleted folder is still listed, tell the owner and ask them to run the command again; a folder open in Explorer or a program can keep a delete from finishing.

The scheduled task goes last, because every job runs through it and `job.sh` registers it again. Ask the owner, when nothing else needs a job, to run this in the same administrator PowerShell:

```powershell
Unregister-ScheduledTask -TaskName lazymkv-phase0 -Confirm:$false
```

Firewall: the `firewall` line of the listing must read `0`. If it does not, ask the owner to run `Remove-NetFirewallRule -DisplayName 'lazymkv-exp05'` in an administrator PowerShell. The agent never changes the rule.

Afterwards confirm only over ssh, never with `job.sh` (it would register the task again):

```bash
ssh USER@makemkv-box "dir /s /b C:\lazymkv\phase0\tmp & schtasks /query /tn lazymkv-phase0 /fo list | findstr TaskName"
ssh USER@makemkv-box "netsh advfirewall firewall show rule name=lazymkv-exp05"
```

Expected:
- First command, with the items the owner agreed to delete gone: `File Not Found` if `tmp` was deleted (otherwise its file names), and no `TaskName:` line if the task was unregistered. For a missing task `schtasks` prints an error such as `ERROR: The system cannot find the file specified.` (the exact words may differ); what matters is that no `TaskName:` line appears. A `TaskName:` line means the task is still registered, which is correct only if the owner chose to keep it.
- Second command: `No rules match the specified criteria.` If a rule is listed, ask the owner to remove it as above and run the command again.

If the owner keeps everything, the second listing and the unregistering are skipped, and the `final-list` log is the record.

- [ ] **Step 7: Record the closing checks in the report**

Which disc is in the drive: does not matter.

Append this section to the end of `docs/phase0/REPORT.md`:

```markdown
## Closing checks, and what was left behind

- Stand-in replay (Task 17 Step 1): `fake-tool` replayed Disc A's online scan (`corpus/scenarios/disc-a-scan/`) with `LAZYMKV_FAKE_SPEED=0.001`; `fc /b` against the original recording on the mini PC: REPLAY_RESULT.
- Tests (Task 17 Step 4): Rust: RUST_RESULT. Python: PY_RESULT.
- What the experiments left on the share and on the mini PC, as `scripts/phase0/list-leftovers.ps1` listed it (job log `final-list`), and what the owner decided:

| Where | Item | Size | Decision |
|---|---|---|---|
| WHERE | ITEM | SIZE | DECISION |

- Scheduled task `lazymkv-phase0`: TASK_STATE.
- Firewall rule `lazymkv-exp05`: FIREWALL_STATE.
```

The fill-ins:
- **REPLAY_RESULT**: `FC: no differences encountered`, as Step 1 printed it (or what it printed instead, and what was done).
- **RUST_RESULT**: `20 tests passed, 0 failed (core 8 + 1, capture 7 + 1, fake-tools 2 + 1)`, or the counts Step 4 printed.
- **PY_RESULT**: the `Ran <n> tests ... OK` line of Step 4.
- **WHERE, ITEM, SIZE, DECISION**: one row for each folder or file the `final-list` log printed directly under `staging:` (WHERE `share`; a folder with its labels beneath it gets one row for each label the owner decided about separately), one row for the test library (`share`), and one for each local folder (`mini PC`: `tmp`, `exp09`, `recordings`, `logs`). SIZE is the log's size. DECISION is `kept` or `deleted by the owner`, as the owner decided and as `final-list-2` and the ssh checks showed.
- **TASK_STATE**: `unregistered by the owner` or `kept`, as the ssh check in Step 6 showed. Note that `scripts/phase0/run-interactive.ps1` registers it again for any later job.
- **FIREWALL_STATE**: `no rule (netsh: "No rules match the specified criteria.")`, or what is true.

- [ ] **Step 8: Commit**

Which disc is in the drive: does not matter.

```bash
find corpus/owner corpus/scenarios -iname '*unit_key*'
grep -rIl 'eyJhbGciOi' corpus docs/phase0 docs/superpowers/specs
git add corpus/scenarios scripts/phase0/list-leftovers.ps1 docs/phase0/REPORT.md docs/superpowers/specs/2026-10-04-lazymkv-design.md
git status --short
git commit -m "Fold phase 0 results into the spec; add the phase report" -m "Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

Expected:
- `find` and `grep` print nothing: no AACS unit-key file, and no JSON Web Token (the TMDb read access token is one, and it starts with that text) in the corpus, the reports or the spec. If either prints a path, stop and do not commit.
- `git status --short` lists as staged only these: `corpus/scenarios/disc-a-scan/makemkvcon64.json`, `scan.txt` and `scan.txt.timing.jsonl`, `scripts/phase0/list-leftovers.ps1`, `docs/phase0/REPORT.md` and the spec (`M`). Anything else listed (for example a `Cargo.lock` that `box.sh` brought back) was not changed by this task: do not add it, and report it.
- The commit succeeds. Phase 0 is finished: phase 1's plan is written against the updated spec.
