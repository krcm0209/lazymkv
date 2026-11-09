//! Staged, evidence and work paths, the one checked copy across volumes, and the unit's two-way error (design parts 7.1, 7.5; B1 (the worker's folder); T5-R4, T5-R39, T5-R55, T5-R56).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The data folder's work folder, `<data>\work` (B1 (the worker's folder), T5-R4, T5-R39). Def: T5-14. Use: T5-14, T5-15.
pub(crate) const WORK: &str = "work";

/// The suffix of a checked copy before its rename (T5-R55). Def: T5-14. Use: T5-14, T5-19.
pub(crate) const PARTIAL: &str = ".partial";

/// `<staging>\<job id>\tuning\<key>` (design part 7.5). Def: T5-14. Use: T5-15, T5-17, T5-18.
pub(crate) fn tuning_dir(staging_root: &str, job_id: &str, key: &str) -> PathBuf {
    Path::new(staging_root).join(job_id).join("tuning").join(key)
}

/// `<staging>\<job id>\evidence\<key>.mks` (spec 6.4 step 10, design part 10.2 step 1). Def: T5-14. Use: T5-14, T5-15,
/// T5-19.
pub(crate) fn evidence_file(staging_root: &str, job_id: &str, key: &str) -> PathBuf {
    Path::new(staging_root).join(job_id).join("evidence").join(format!("{key}.mks"))
}

/// `<work_root>\<kind>-<job id>-<key>`, kind `ocr` or `levels` (T5-R4, T5-R39). Def: T5-14. Use: T5-15, T5-17.
pub(crate) fn work_dir(work_root: &Path, kind: &str, job_id: &str, key: &str) -> PathBuf {
    work_root.join(format!("{kind}-{job_id}-{key}"))
}

/// The checked copy with its copy step given, the fault seam of the tests (T5-R55). Def: T5-14. Use: T5-14, T5-19.
pub(crate) fn copy_checked_with(from: &Path, to: &Path, copy: &dyn Fn(&Path, &Path) -> io::Result<u64>) -> io::Result<()> {
    let want = fs::metadata(from)?.len();
    let mut partial = to.as_os_str().to_os_string();
    partial.push(PARTIAL);
    let partial = PathBuf::from(partial);
    match fs::remove_file(&partial) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    copy(from, &partial)?;
    // On Windows FlushFileBuffers needs write access; a file renamed into place is then whole on the disk (C3).
    fs::OpenOptions::new().write(true).open(&partial)?.sync_all()?;
    let got = fs::metadata(&partial)?.len();
    if got != want {
        let _ = fs::remove_file(&partial);
        return Err(io::Error::other(format!(
            "{}: {got} bytes after the copy, not the {want} bytes of {}",
            partial.display(),
            from.display()
        )));
    }
    fs::rename(&partial, to)
}

/// `copy_checked_with` over `fs::copy`: `<to>.partial` flushed with `sync_all`, its size against `from`'s, the rename (T5-R55).
/// Def: T5-14. Use: T5-15, T5-17, T5-19.
pub(crate) fn copy_checked(from: &Path, to: &Path) -> io::Result<()> {
    copy_checked_with(from, to, &|from, to| fs::copy(from, to))
}

/// `bytes` written to `<to>.partial`, a stale `<to>.partial` removed first, its size checked against `bytes.len()`,
/// then renamed to `to` (T5-R55, T5-R60: an orphan's TMDb copy fetched again). Def: T5-22. Use: T5-22.
pub(crate) fn write_checked(to: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut partial = to.as_os_str().to_os_string();
    partial.push(PARTIAL);
    let partial = PathBuf::from(partial);
    match fs::remove_file(&partial) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    fs::write(&partial, bytes)?;
    let written = fs::metadata(&partial)?.len();
    if written != bytes.len() as u64 {
        let _ = fs::remove_file(&partial);
        return Err(io::Error::other(format!("{}: {written} bytes written of {}", partial.display(), bytes.len())));
    }
    fs::rename(&partial, to)
}

/// B1 (the worker's folder)'s move: `copy_checked`, then `from` deleted (T5-R55). Def: T5-14. Use: T5-15.
pub(crate) fn move_checked(from: &Path, to: &Path) -> io::Result<()> {
    copy_checked(from, to)?;
    fs::remove_file(from)
}

/// Every file under `from` into `to` through `copy_checked`, each folder's entries in name order (T5-R55). Def: T5-14.
/// Use: T5-15, T5-19.
pub(crate) fn copy_tree_checked(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    let mut entries = Vec::new();
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        entries.push((entry.file_name(), entry.path()));
    }
    entries.sort();
    for (name, path) in entries {
        if path.is_dir() {
            copy_tree_checked(&path, &to.join(&name))?;
        } else {
            copy_checked(&path, &to.join(&name))?;
        }
    }
    Ok(())
}

/// The job.log sentence of a title whose staged tuning folder is complete on a re-run of the unit (notes C3), after
/// `<key>: `. Def: T5-18. Use: T5-18.
pub(crate) const SKIPPED_COMPLETE: &str = "steps 10 and 10a skipped: the staged tuning folder is complete";

/// The job.log sentence of a title whose staged tuning folder was incomplete on a re-run of the unit and was deleted
/// (notes C3), after `<key>: `. Def: T5-18. Use: T5-18.
pub(crate) const CLEARED_INCOMPLETE: &str =
    "the staged tuning folder was incomplete, so it was deleted and steps 10 and 10a run again";

/// The name prefix of a level folder, `audio-levels-20ms-<main>[-<cand>...]` (design part 7.5).
const LEVEL_PREFIX: &str = "audio-levels-20ms-";

/// What a re-run of the unit finds in a title's staged tuning folder (notes C3): `Complete` skips steps 10 and 10a,
/// `Incomplete` is deleted and both run again, `Absent` (no folder) runs both. Def: T5-18. Use: T5-18.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Staged {
    /// Every marker is there: steps 10 and 10a are skipped.
    Complete,
    /// The folder is there without a marker: it is deleted and both steps run again.
    Incomplete,
    /// No folder: both steps run.
    Absent,
}

/// Whether a title's staged tuning folder is complete (notes C3): the staged level folder's own `audio-proxy.json`,
/// written last there (T5-R13), or with `keep_tuning_evidence` off `streamhash.txt`; and `subs-ocr\summary.json`,
/// written last into the staged `subs-ocr\` (T5-R12), when step 10 runs for the title. Def: T5-18. Use: T5-18.
pub(crate) fn staged_state(tuning: &Path, keep_tuning_evidence: bool, step10: bool) -> Staged {
    if !tuning.is_dir() {
        return Staged::Absent;
    }
    let audio = if keep_tuning_evidence {
        match fs::read_dir(tuning) {
            Ok(entries) => entries.filter_map(Result::ok).any(|entry| {
                let path = entry.path();
                entry.file_name().to_str().is_some_and(|name| name.starts_with(LEVEL_PREFIX))
                    && path.is_dir()
                    && path.join("audio-proxy.json").is_file()
            }),
            Err(_) => false,
        }
    } else {
        tuning.join("streamhash.txt").is_file()
    };
    let subs = !step10 || tuning.join("subs-ocr").join("summary.json").is_file();
    if audio && subs {
        Staged::Complete
    } else {
        Staged::Incomplete
    }
}

/// `remove` with a path already gone read as done.
fn removed(result: io::Result<()>) -> io::Result<()> {
    match result {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// Deletes an incomplete staged tuning folder and the job folder's outputs of steps 10 and 10a, `subs-ocr\`, the
/// level folders, `streamhash.txt` and the streamhash pass's capture (notes C3; T5-R57), so a re-run never mixes two
/// runs' files; a path already gone is not an error. Def: T5-18. Use: T5-18.
pub(crate) fn clear_incomplete(tuning: &Path, label_dir: &Path) -> io::Result<()> {
    removed(fs::remove_dir_all(tuning))?;
    removed(fs::remove_dir_all(label_dir.join("subs-ocr")))?;
    let entries = match fs::read_dir(label_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let mut levels = Vec::new();
    for entry in entries {
        let entry = entry?;
        if entry.file_name().to_str().is_some_and(|name| name.starts_with(LEVEL_PREFIX)) && entry.path().is_dir() {
            levels.push(entry.path());
        }
    }
    levels.sort();
    for level in levels {
        removed(fs::remove_dir_all(level))?;
    }
    for file in STREAMHASH_FILES {
        removed(fs::remove_file(label_dir.join(file)))?;
    }
    Ok(())
}

/// Whether the job folder's title folder `label_dir` holds any output of steps 10 and 10a that `clear_incomplete`
/// deletes: `subs-ocr\`, a level folder, `streamhash.txt` or the streamhash pass's capture (design part 7.5; notes C3;
/// T5-R57), so an unreachable staging share read as an absent staged folder deletes none of them. Def: T5-18. Use:
/// T5-18.
pub(crate) fn unit_outputs_left(label_dir: &Path) -> bool {
    let levels = fs::read_dir(label_dir).is_ok_and(|entries| {
        entries.filter_map(Result::ok).any(|entry| {
            entry.file_name().to_str().is_some_and(|name| name.starts_with(LEVEL_PREFIX)) && entry.path().is_dir()
        })
    });
    levels || label_dir.join("subs-ocr").exists() || STREAMHASH_FILES.iter().any(|file| label_dir.join(file).exists())
}

/// The job folder's streamhash files that `clear_incomplete` deletes: `streamhash.txt` and the pass's capture.
const STREAMHASH_FILES: [&str; 5] = [
    "streamhash.txt",
    "streamhash-ffmpeg.txt",
    "streamhash-ffmpeg.txt.stderr",
    "streamhash-ffmpeg.txt.timing.jsonl",
    "streamhash-ffmpeg.txt.exit.json",
];

/// A copy into the job folder (R5-code-30, design part 7.4): `from`'s bytes, their count checked against `from`'s
/// size, written through `lazymkv_platform::write_atomic`, so a job-folder file is flushed and whole or absent; it
/// creates no folder. Def: T5-17. Use: T5-17, T5-19.
pub(crate) fn copy_into_job(from: &Path, to: &Path) -> io::Result<()> {
    let want = fs::metadata(from)?.len();
    let bytes = fs::read(from)?;
    if bytes.len() as u64 != want {
        return Err(io::Error::other(format!(
            "{}: {} bytes read, not the {want} bytes of its size",
            from.display(),
            bytes.len()
        )));
    }
    lazymkv_platform::write_atomic(to, &bytes)
}

/// How a step of the unit (steps 10, 10a and 11) failed (T5-R56): `Failed` is a tool, decode or argument failure, fed
/// as `DecisionFailed { Unknown }` with its sentence; `Staging` is a write error on the staging share, classed by
/// `staging_write_failure` over the share check. Def: T5-14. Use: T5-14, T5-15, T5-17, T5-18.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum UnitError {
    /// A tool, decode or argument failure, with its sentence.
    Failed(String),
    /// A write error on the staging share, with its sentence.
    Staging(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use lazymkv_core::testsupport::{Scratch, ScratchExt};
    use std::fs;

    /// Every file under `dir`, relative, with '/' separators.
    fn files(dir: &Path) -> Vec<String> {
        fn walk(root: &Path, dir: &Path, found: &mut Vec<String>) {
            for entry in fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    walk(root, &path, found);
                } else {
                    found.push(path.strip_prefix(root).unwrap().to_str().unwrap().replace('\\', "/"));
                }
            }
        }
        let mut found = Vec::new();
        walk(dir, dir, &mut found);
        found.sort();
        found
    }

    #[test]
    fn copy_checked_lands_a_file_across_two_roots_through_partial() {
        let (a, b) = (Scratch::new("staged-from"), Scratch::new("staged-to"));
        let from = a.write_path("evidence.mks", b"lazymkv-test-mks");
        let to = b.0.join("evidence.mks");
        fs::write(b.0.join("evidence.mks.partial"), b"stale").unwrap();
        copy_checked(&from, &to).unwrap();
        assert_eq!(fs::read(&to).unwrap(), b"lazymkv-test-mks");
        assert_eq!(fs::read(&from).unwrap(), b"lazymkv-test-mks");
        assert_eq!(files(&b.0), ["evidence.mks"]);
        let missing = b.0.join("no-folder").join("evidence.mks");
        assert_eq!(copy_checked(&from, &missing).unwrap_err().kind(), io::ErrorKind::NotFound);
        assert!(!b.0.join("no-folder").exists(), "copy_checked creates no folder");
    }

    #[test]
    fn a_copy_whose_size_differs_leaves_no_final_file_and_says_so() {
        let (a, b) = (Scratch::new("staged-from"), Scratch::new("staged-to"));
        let from = a.write_path("sub-0.ocr.jsonl", b"0123456789");
        let to = b.0.join("sub-0.ocr.jsonl");
        let short = |from: &Path, to: &Path| -> io::Result<u64> {
            fs::write(to, &fs::read(from)?[..7])?;
            Ok(10)
        };
        let error = copy_checked_with(&from, &to, &short).unwrap_err();
        let partial = b.0.join("sub-0.ocr.jsonl.partial");
        assert_eq!(
            error.to_string(),
            format!("{}: 7 bytes after the copy, not the 10 bytes of {}", partial.display(), from.display())
        );
        assert!(files(&b.0).is_empty(), "neither the file nor its .partial is left");
    }

    #[test]
    fn move_checked_deletes_the_source_only_after_the_copy_landed() {
        let (a, b) = (Scratch::new("staged-from"), Scratch::new("staged-to"));
        let from = a.write_path("s0/f-000001000.png", b"png-bytes");
        assert!(move_checked(&from, &b.0.join("no-folder").join("f-000001000.png")).is_err());
        assert!(from.is_file(), "a copy that failed keeps its source");
        let to = b.0.join("f-000001000.png");
        move_checked(&from, &to).unwrap();
        assert!(!from.exists());
        assert_eq!(fs::read(&to).unwrap(), b"png-bytes");
    }

    #[test]
    fn copy_tree_checked_copies_every_file_of_every_folder() {
        let (a, b) = (Scratch::new("staged-from"), Scratch::new("staged-to"));
        a.write("subs-ocr/sub-0.ocr.jsonl", b"{}\n");
        a.write("subs-ocr/s0/f-000001000.png", b"png");
        copy_tree_checked(&a.0.join("subs-ocr"), &b.0.join("subs-ocr")).unwrap();
        assert_eq!(files(&b.0), ["subs-ocr/s0/f-000001000.png", "subs-ocr/sub-0.ocr.jsonl"]);
        assert_eq!(fs::read(b.0.join("subs-ocr").join("s0").join("f-000001000.png")).unwrap(), b"png");
        assert_eq!(files(&a.0), ["subs-ocr/s0/f-000001000.png", "subs-ocr/sub-0.ocr.jsonl"], "a copy keeps its source");
    }

    /// R5-code-30: a copy into the job folder is the job folder's own write, `write_atomic`, after a size check.
    #[test]
    fn copy_into_job_writes_the_bytes_through_write_atomic_and_leaves_no_temporary_file() {
        let (a, b) = (Scratch::new("staged-from"), Scratch::new("staged-job"));
        let from = a.write_path("streamhash.txt", b"0,a,MD5=0f343b0931126a20f133d67c2b018a3b\n");
        let to = b.0.join("streamhash.txt");
        fs::write(&to, b"old").unwrap();
        copy_into_job(&from, &to).unwrap();
        assert_eq!(fs::read(&to).unwrap(), fs::read(&from).unwrap());
        assert_eq!(files(&b.0), ["streamhash.txt"], "no .partial and no .lazymkv-tmp is left");
        let missing = b.0.join("no-folder").join("streamhash.txt");
        assert_eq!(copy_into_job(&from, &missing).unwrap_err().kind(), io::ErrorKind::NotFound);
        assert!(!b.0.join("no-folder").exists(), "copy_into_job creates no folder");
    }

    /// T5-R55: the data folder, staging and tuning_root are different volumes, so the unit moves a file between them
    /// only through `copy_checked` and `move_checked`, which this file holds; no other source of the unit renames or
    /// copies.
    #[test]
    fn the_unit_moves_files_across_volumes_only_through_the_checked_copy() {
        let sources = [
            ("step10.rs", include_str!("step10.rs")),
            ("step10a.rs", include_str!("step10a.rs")),
            ("tuning.rs", include_str!("tuning.rs")),
        ];
        for (name, text) in sources {
            for needle in ["fs::rename", "fs::copy", "rename_no_replace"] {
                assert!(!text.contains(needle), "{name} holds {needle}");
            }
        }
    }

    #[test]
    fn work_dir_names_kind_job_and_key_under_the_work_root() {
        let root = Path::new("data").join(WORK);
        assert_eq!(work_dir(&root, "ocr", "abc-1", "t00-00800"), Path::new("data").join("work").join("ocr-abc-1-t00-00800"));
        assert_eq!(work_dir(&root, "levels", "abc-1", "t00-00800"), root.join("levels-abc-1-t00-00800"));
    }

    #[test]
    fn the_staged_paths_are_under_the_jobs_staging_folder() {
        let job = Path::new("S").join("abc-1");
        assert_eq!(evidence_file("S", "abc-1", "t00-00800"), job.join("evidence").join("t00-00800.mks"));
        assert_eq!(tuning_dir("S", "abc-1", "t00-00800"), job.join("tuning").join("t00-00800"));
    }

    /// A staged title folder in `s`: each of `files` written as "x" under `tuning/`.
    fn staged(s: &Scratch, files: &[&str]) -> std::path::PathBuf {
        for file in files {
            s.write(&format!("tuning/{file}"), "x");
        }
        s.0.join("tuning")
    }

    #[test]
    fn a_missing_staged_folder_is_absent() {
        let s = Scratch::new("staged-absent");
        assert_eq!(staged_state(&s.0.join("tuning"), true, true), Staged::Absent);
    }

    #[test]
    fn the_level_folders_audio_proxy_json_and_summary_json_make_the_folder_complete() {
        let s = Scratch::new("staged-complete");
        let tuning = staged(&s, &[
            "audio-proxy.mka",
            "audio-proxy.json",
            "audio-levels-20ms-1-2/main-mix.txt",
            "audio-levels-20ms-1-2/audio-proxy.json",
            "subs-ocr/sub-0.ocr.jsonl",
            "subs-ocr/summary.json",
        ]);
        assert_eq!(staged_state(&tuning, true, true), Staged::Complete);
    }

    #[test]
    fn the_audio_proxy_json_beside_the_proxy_does_not_make_the_folder_complete() {
        let s = Scratch::new("staged-no-level-marker");
        let tuning = staged(&s, &[
            "audio-proxy.mka",
            "audio-proxy.json",
            "audio-levels-20ms-1-2/main-mix.txt",
            "subs-ocr/summary.json",
        ]);
        assert_eq!(staged_state(&tuning, true, true), Staged::Incomplete);
    }

    #[test]
    fn a_staged_folder_without_summary_json_where_step_10_runs_is_incomplete() {
        let s = Scratch::new("staged-no-summary");
        let tuning = staged(&s, &["audio-levels-20ms-1/audio-proxy.json", "subs-ocr/sub-0.ocr.jsonl"]);
        assert_eq!(staged_state(&tuning, true, true), Staged::Incomplete);
    }

    #[test]
    fn without_step_10_summary_json_is_not_asked_for() {
        let s = Scratch::new("staged-no-step-10");
        let tuning = staged(&s, &["audio-levels-20ms-1/audio-proxy.json"]);
        assert_eq!(staged_state(&tuning, true, false), Staged::Complete);
    }

    #[test]
    fn with_the_setting_off_streamhash_txt_is_the_audio_marker() {
        let s = Scratch::new("staged-off");
        let tuning = staged(&s, &["streamhash.txt", "subs-ocr/summary.json"]);
        assert_eq!(staged_state(&tuning, false, true), Staged::Complete);
        let s = Scratch::new("staged-off-level-only");
        let tuning = staged(&s, &["audio-levels-20ms-1/audio-proxy.json", "subs-ocr/summary.json"]);
        assert_eq!(staged_state(&tuning, false, true), Staged::Incomplete);
        let s = Scratch::new("staged-on-streamhash-only");
        let tuning = staged(&s, &["streamhash.txt", "subs-ocr/summary.json"]);
        assert_eq!(staged_state(&tuning, true, true), Staged::Incomplete);
    }

    #[test]
    fn clear_incomplete_deletes_the_staged_folder_and_only_the_units_files_of_the_job_folder() {
        let s = Scratch::new("staged-clear");
        let tuning = staged(&s, &["audio-proxy.mka", "subs-ocr/sub-0.ocr.jsonl"]);
        for file in [
            "job/t00-00800/subs-ocr/summary.json",
            "job/t00-00800/subs-ocr/sub-0.frames.json",
            "job/t00-00800/audio-levels-20ms-1-2/main-mix.txt",
            "job/t00-00800/streamhash.txt",
            "job/t00-00800/streamhash-ffmpeg.txt",
            "job/t00-00800/streamhash-ffmpeg.txt.stderr",
            "job/t00-00800/streamhash-ffmpeg.txt.timing.jsonl",
            "job/t00-00800/streamhash-ffmpeg.txt.exit.json",
            "job/t00-00800/subs/index.json",
            "job/t00-00800/subs/sub-0.packets.json",
            "job/t00-00800/probe-summary.json",
            "job/t00-00800/evidence-mkvmerge.txt",
            "job/t00-00800/mkvmerge.json",
            "job/t00-00800/rip.txt",
        ] {
            s.write(file, "x");
        }
        let label_dir = s.0.join("job").join("t00-00800");
        clear_incomplete(&tuning, &label_dir).unwrap();
        assert!(!tuning.exists());
        let mut left: Vec<String> =
            fs::read_dir(&label_dir).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
        left.sort();
        assert_eq!(left, ["evidence-mkvmerge.txt", "mkvmerge.json", "probe-summary.json", "rip.txt", "subs"]);
        assert!(label_dir.join("subs").join("sub-0.packets.json").is_file());
    }

    /// T5-R60: an orphan's TMDb copy lands through `<to>.partial`, replacing a torn one of another size.
    #[test]
    fn write_checked_lands_the_bytes_through_partial_and_leaves_no_partial() {
        let s = Scratch::new("staged-write-checked");
        let to = s.0.join("tmdb.json");
        fs::write(s.0.join("tmdb.json.partial"), "a torn write of another size").unwrap();
        write_checked(&to, b"{\n  \"id\": 1\n}\n").unwrap();
        assert_eq!(fs::read(&to).unwrap(), b"{\n  \"id\": 1\n}\n");
        assert!(!s.0.join("tmdb.json.partial").exists());
    }

    /// H1: a link left at `<to>.partial` is removed, not written through, so its target keeps its bytes and `to` is a
    /// regular file.
    #[cfg(unix)]
    #[test]
    fn write_checked_removes_a_link_at_partial_and_never_writes_through_it() {
        let s = Scratch::new("staged-write-checked-link");
        let victim = s.write_path("victim.json", b"the victim's bytes");
        let to = s.0.join("tmdb.json");
        std::os::unix::fs::symlink(&victim, s.0.join("tmdb.json.partial")).unwrap();
        write_checked(&to, b"{\n  \"id\": 1\n}\n").unwrap();
        assert_eq!(fs::read(&victim).unwrap(), b"the victim's bytes");
        assert!(fs::symlink_metadata(&to).unwrap().file_type().is_file(), "to is a regular file, not the link");
        assert_eq!(fs::read(&to).unwrap(), b"{\n  \"id\": 1\n}\n");
        assert!(fs::symlink_metadata(s.0.join("tmdb.json.partial")).is_err(), "no .partial is left");
    }

    /// Notes C3, Task 18's error path: a plain file at the staged tuning folder's path is not a folder already gone,
    /// so `clear_incomplete` gives the error and deletes nothing of the job folder.
    #[test]
    fn clear_incomplete_gives_the_error_when_the_staged_tuning_path_is_a_file() {
        let s = Scratch::new("staged-clear-file");
        let tuning = s.write_path("tuning/t00-00800", "a file where the folder goes");
        s.write("job/t00-00800/subs-ocr/summary.json", "x");
        let label_dir = s.0.join("job").join("t00-00800");
        let error = clear_incomplete(&tuning, &label_dir).unwrap_err();
        assert_ne!(error.kind(), io::ErrorKind::NotFound);
        assert!(tuning.is_file());
        assert!(label_dir.join("subs-ocr").join("summary.json").is_file());
    }

    #[test]
    fn unit_outputs_left_finds_only_what_clear_incomplete_deletes() {
        let s = Scratch::new("staged-outputs-left");
        let label_dir = s.0.join("job").join("t00-00800");
        assert!(!unit_outputs_left(&label_dir), "no title folder");
        s.write("job/t00-00800/mkvmerge.json", "x");
        s.write("job/t00-00800/subs/index.json", "x");
        assert!(!unit_outputs_left(&label_dir));
        for (scratch, file) in [
            ("staged-outputs-left-subs", "subs-ocr/summary.json"),
            ("staged-outputs-left-levels", "audio-levels-20ms-1/main-mix.txt"),
            ("staged-outputs-left-hash", "streamhash.txt"),
            ("staged-outputs-left-capture", "streamhash-ffmpeg.txt.stderr"),
        ] {
            let s = Scratch::new(scratch);
            s.write(&format!("job/t00-00800/{file}"), "x");
            assert!(unit_outputs_left(&s.0.join("job").join("t00-00800")), "{file}");
        }
    }

    #[test]
    fn clear_incomplete_of_a_title_with_nothing_to_delete_is_ok() {
        let s = Scratch::new("staged-clear-nothing");
        s.write("job/t00-00800/mkvmerge.json", "x");
        let label_dir = s.0.join("job").join("t00-00800");
        assert!(clear_incomplete(&s.0.join("tuning"), &label_dir).is_ok());
        assert!(label_dir.join("mkvmerge.json").is_file());
    }
}
