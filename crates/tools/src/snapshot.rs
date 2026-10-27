//! The disc snapshot's file I/O, moved here from capture's disc.rs (R-B2, design parts 4.1 and 5.4 step 2): the walk,
//! `files.json`, the `nav/` copies and `meta.json`. The pure part (which files are navigation files, which is the unit
//! key, meta.json's values) is `lazymkv_core::snapshot`. This file reads no clock: `captured_unix` is a parameter.

use lazymkv_core::fingerprint::DiscFile;
use lazymkv_core::snapshot::{disc_meta, is_nav_file, unit_key_file, DiscMeta};
use serde::Serialize;
use std::fs;
use std::io;
use std::path::Path;

/// One entry of `files.json`, keys in this order.
#[derive(Debug, Serialize)]
struct FileEntry<'a> {
    path: &'a str,
    size: u64,
}

/// Every file under `root`, with `/`-separated paths relative to it, sorted by path (R-B2; design part 5.4 step 2).
/// The sort compares the path strings byte by byte. Def: T12. Use: T22, T23.
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

/// Writes `files.json`, `nav/` and `meta.json` under `out` for the disc mounted at `root`, in that order, and returns
/// the `meta.json` it wrote (R-B2; design part 5.4 step 2). `nav/` holds only the files `is_nav_file` accepts, at their
/// relative paths; the AACS unit key is read for `disc_id` but never copied. A unit key file that is listed but cannot be
/// read gives `disc_id: None` without an error. `captured_unix` is the caller's clock. `out` may already hold an earlier
/// snapshot of the same disc (S29: a restart in Detected, a Retry or a return records the disc again into the job
/// folder), whose files are overwritten; the nav copies are always writable by their owner, whatever the disc's
/// attributes (review round 1, V10). Def: T12. Use: T22, T23.
pub fn snapshot(root: &Path, volume_label: &str, out: &Path, captured_unix: u64) -> io::Result<DiscMeta> {
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
        copy_nav_file(&root.join(&file.path), &destination)?;
    }

    let unit_key = unit_key_file(&files).and_then(|file| fs::read(root.join(&file.path)).ok());
    let meta = disc_meta(&files, volume_label, unit_key.as_deref(), captured_unix);
    fs::write(out.join("meta.json"), serde_json::to_vec_pretty(&meta)?)?;
    Ok(meta)
}

/// Copies one navigation file so that a later snapshot into the same `out` can replace it (S29). `fs::copy` keeps the
/// disc's read-only attribute (phase 0 saw it, docs/phase0/environment.md), and a read-only copy cannot be overwritten,
/// so the copy is made writable; a copy an earlier snapshot left read-only (one written before review round 1's V10, or
/// a kill between the copy and that change) is made writable first. A metadata error other than NotFound is returned.
fn copy_nav_file(from: &Path, to: &Path) -> io::Result<()> {
    match fs::symlink_metadata(to) {
        Ok(existing) if existing.is_file() => make_writable(to)?,
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    fs::copy(from, to)?;
    make_writable(to)
}

/// Gives the owner write permission when it is missing, and changes no other bit: never the group's or others' write
/// bit (RR1-11).
#[cfg(unix)]
fn make_writable(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mode = fs::metadata(path)?.permissions().mode();
    if mode & 0o200 == 0 {
        fs::set_permissions(path, fs::Permissions::from_mode(mode | 0o200))?;
    }
    Ok(())
}

/// Clears the read-only attribute when it is set. On Windows `set_readonly(false)` clears only
/// FILE_ATTRIBUTE_READONLY, so the lint's concern, write access for every user, applies only to Unix modes.
#[cfg(not(unix))]
#[allow(clippy::permissions_set_readonly_false)]
fn make_writable(path: &Path) -> io::Result<()> {
    let mut permissions = fs::metadata(path)?.permissions();
    if permissions.readonly() {
        permissions.set_readonly(false);
        fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::temp_dir;
    use lazymkv_core::fingerprint::content_hash;

    fn write(root: &Path, rel: &str, bytes: &[u8]) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn snapshot_lists_fingerprints_and_copies_navigation_files() {
        let (root, out) = (temp_dir("disc"), temp_dir("out"));
        write(&root, "BDMV/STREAM/00001.m2ts", &[0u8; 300]);
        write(&root, "BDMV/STREAM/00000.m2ts", &[0u8; 100]);
        write(&root, "BDMV/PLAYLIST/00001.mpls", b"mpls");
        write(&root, "BDMV/index.bdmv", b"index");
        write(&root, "AACS/Unit_Key_RO.inf", b"abc");

        let meta = snapshot(&root, "MY_DISC", &out, 7).unwrap();

        assert_eq!(meta.volume_label, "MY_DISC");
        assert_eq!(meta.format.as_deref(), Some("bluray"));
        assert_eq!(meta.file_count, 5);
        assert_eq!(meta.total_bytes, 300 + 100 + 4 + 5 + 3);
        assert_eq!(meta.captured_unix, 7);
        let expected = content_hash(&[
            DiscFile { path: "BDMV/STREAM/00000.m2ts".into(), size: 100 },
            DiscFile { path: "BDMV/STREAM/00001.m2ts".into(), size: 300 },
        ]);
        assert_eq!(meta.fingerprint, expected);
        assert_eq!(meta.disc_id.as_deref(), Some("A9993E364706816ABA3E25717850C26C9CD0D89D"));
        assert!(out.join("nav/BDMV/PLAYLIST/00001.mpls").exists());
        assert!(out.join("nav/BDMV/index.bdmv").exists());
        assert!(!out.join("nav/BDMV/STREAM/00001.m2ts").exists());
        let written: serde_json::Value = serde_json::from_slice(&fs::read(out.join("meta.json")).unwrap()).unwrap();
        assert_eq!(written["captured_unix"], 7);
        let listed: serde_json::Value = serde_json::from_slice(&fs::read(out.join("files.json")).unwrap()).unwrap();
        assert_eq!(listed.as_array().unwrap().len(), 5);
        assert_eq!(listed[0], serde_json::json!({"path": "AACS/Unit_Key_RO.inf", "size": 3}));
    }

    #[test]
    fn the_snapshot_never_copies_unit_key_ro() {
        let (root, out) = (temp_dir("disc-key"), temp_dir("out-key"));
        write(&root, "AACS/Unit_Key_RO.inf", b"abc");
        write(&root, "AACS/DUPLICATE/Unit_Key_RO.inf", b"abc");
        write(&root, "BDMV/index.bdmv", b"index");

        let meta = snapshot(&root, "KEY", &out, 7).unwrap();

        assert!(!out.join("nav/AACS").exists());
        assert_eq!(meta.disc_id.as_deref(), Some("A9993E364706816ABA3E25717850C26C9CD0D89D"));
        let listed = fs::read_to_string(out.join("files.json")).unwrap();
        assert!(listed.contains("AACS/DUPLICATE/Unit_Key_RO.inf"), "{listed}");
    }

    #[test]
    fn nav_files_keep_their_relative_paths() {
        let (root, out) = (temp_dir("disc-nav"), temp_dir("out-nav"));
        write(&root, "BDMV/META/DL/bdmt_eng.xml", b"<di:name>Synthetic Movie</di:name>");
        write(&root, "BDMV/PLAYLIST/00800.mpls", b"mpls");
        write(&root, "BDMV/STREAM/00001.m2ts", &[0u8; 64]);

        snapshot(&root, "SYNTHETIC_MOVIE", &out, 7).unwrap();

        assert_eq!(fs::read(out.join("nav/BDMV/META/DL/bdmt_eng.xml")).unwrap(), b"<di:name>Synthetic Movie</di:name>");
        assert_eq!(fs::read(out.join("nav/BDMV/PLAYLIST/00800.mpls")).unwrap(), b"mpls");
        assert!(!out.join("nav/BDMV/STREAM").exists());
    }

    /// Makes a file read-only, as a disc's files are (docs/phase0/environment.md), or writable again for the clean-up:
    /// mode 0o444 or 0o644 on Unix.
    #[cfg(unix)]
    fn set_read_only(path: &Path, read_only: bool) {
        use std::os::unix::fs::PermissionsExt;
        let mode = if read_only { 0o444 } else { 0o644 };
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    }

    /// Sets or clears the read-only attribute (Windows: only FILE_ATTRIBUTE_READONLY).
    #[cfg(not(unix))]
    #[allow(clippy::permissions_set_readonly_false)]
    fn set_read_only(path: &Path, read_only: bool) {
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_readonly(read_only);
        fs::set_permissions(path, permissions).unwrap();
    }

    /// Makes every file under each folder writable again and removes the folders.
    fn restore_and_remove(dirs: &[&Path]) {
        for dir in dirs {
            for file in list_files(dir).unwrap() {
                set_read_only(&dir.join(&file.path), false);
            }
            let _ = fs::remove_dir_all(dir);
        }
    }

    /// A disc whose files are all read-only: three navigation files and one stream.
    fn read_only_disc(tag: &str) -> std::path::PathBuf {
        let root = temp_dir(tag);
        write(&root, "BDMV/index.bdmv", b"index");
        write(&root, "BDMV/PLAYLIST/00001.mpls", b"mpls");
        write(&root, "BDMV/META/DL/bdmt_eng.xml", b"<di:name>Synthetic Movie</di:name>");
        write(&root, "BDMV/STREAM/00001.m2ts", &[0u8; 64]);
        for file in list_files(&root).unwrap() {
            set_read_only(&root.join(&file.path), true);
        }
        root
    }

    /// Review round 1, V10 (S29): a restart in Detected, a Retry or a return records the disc again into the job
    /// folder, so a second snapshot must replace the first one's nav copies even when the disc's files are read-only.
    #[test]
    fn a_second_snapshot_into_the_same_folder_replaces_read_only_nav_copies() {
        let (root, out) = (read_only_disc("disc-ro-twice"), temp_dir("out-ro-twice"));

        snapshot(&root, "RO", &out, 7).unwrap();
        let meta = snapshot(&root, "RO", &out, 8).unwrap();

        assert_eq!(meta.captured_unix, 8);
        let written: serde_json::Value = serde_json::from_slice(&fs::read(out.join("meta.json")).unwrap()).unwrap();
        assert_eq!(written["captured_unix"], 8);
        for rel in ["BDMV/index.bdmv", "BDMV/PLAYLIST/00001.mpls", "BDMV/META/DL/bdmt_eng.xml"] {
            assert_eq!(fs::read(out.join("nav").join(rel)).unwrap(), fs::read(root.join(rel)).unwrap(), "{rel}");
        }
        assert!(!out.join("nav/BDMV/STREAM").exists());
        restore_and_remove(&[&root, &out]);
    }

    /// Review round 1, V10 (RR1-11): a nav copy never keeps the disc's read-only attribute; on Unix only the owner's
    /// write bit is added, never the group's or others'.
    #[test]
    fn nav_copies_are_writable_when_the_disc_files_are_read_only() {
        let (root, out) = (read_only_disc("disc-ro-writable"), temp_dir("out-ro-writable"));

        snapshot(&root, "RO", &out, 7).unwrap();

        let copy = out.join("nav/BDMV/index.bdmv");
        drop(fs::OpenOptions::new().write(true).open(&copy).expect("the nav copy opens for writing"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&copy).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o644, "{mode:o}");
        }
        assert_eq!(fs::read(&copy).unwrap(), b"index");
        restore_and_remove(&[&root, &out]);
    }

    /// Review round 1, V10: a nav copy an earlier snapshot left read-only (one written before the fix, or a kill
    /// between the copy and making it writable) is replaced.
    #[test]
    fn a_nav_copy_an_earlier_snapshot_left_read_only_is_replaced() {
        let (root, out) = (temp_dir("disc-ro-left"), temp_dir("out-ro-left"));
        write(&root, "BDMV/index.bdmv", b"index");
        write(&out, "nav/BDMV/index.bdmv", b"an older copy");
        set_read_only(&out.join("nav/BDMV/index.bdmv"), true);

        snapshot(&root, "RO", &out, 7).unwrap();

        assert_eq!(fs::read(out.join("nav/BDMV/index.bdmv")).unwrap(), b"index");
        restore_and_remove(&[&root, &out]);
    }

    #[test]
    fn list_files_gives_slash_paths_sorted_by_path() {
        let root = temp_dir("list");
        write(&root, "b/2.txt", b"22");
        write(&root, "a.txt", b"1");
        write(&root, "b/1.txt", b"333");

        assert_eq!(
            list_files(&root).unwrap(),
            vec![
                DiscFile { path: "a.txt".into(), size: 1 },
                DiscFile { path: "b/1.txt".into(), size: 3 },
                DiscFile { path: "b/2.txt".into(), size: 2 },
            ]
        );
    }
}
