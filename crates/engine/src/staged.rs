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

    /// T5-R55: the data folder, staging and tuning_root are different volumes, so the unit moves a file between them
    /// only through `copy_checked` and `move_checked`, which this file holds; no other source of the unit renames or
    /// copies.
    #[test]
    fn the_unit_moves_files_across_volumes_only_through_the_checked_copy() {
        let sources = [("step10.rs", include_str!("step10.rs"))];
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
}
