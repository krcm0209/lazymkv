//! Snapshot of a disc's file system: listing, fingerprint, navigation-file copies.

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
