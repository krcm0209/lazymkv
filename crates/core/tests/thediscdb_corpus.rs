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
