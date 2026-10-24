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
/// read gives `disc_id: None` without an error. `captured_unix` is the caller's clock. Def: T12. Use: T22, T23.
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
        fs::copy(root.join(&file.path), destination)?;
    }

    let unit_key = unit_key_file(&files).and_then(|file| fs::read(root.join(&file.path)).ok());
    let meta = disc_meta(&files, volume_label, unit_key.as_deref(), captured_unix);
    fs::write(out.join("meta.json"), serde_json::to_vec_pretty(&meta)?)?;
    Ok(meta)
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
