//! Capture's `disc` output, pinned byte for byte before its snapshot moves into core and tools, and checked against
//! core's pure part (R-B2; design part 4.1, "unchanged output"). The disc is a made-up Blu-ray folder written here;
//! nothing is read from corpus/.

use lazymkv_core::fingerprint::DiscFile;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// The made-up disc: each file's path under the root and its bytes.
fn disc_files() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("BDMV/index.bdmv", b"index".to_vec()),
        ("BDMV/MovieObject.bdmv", b"movieobject".to_vec()),
        ("BDMV/PLAYLIST/00800.mpls", b"mpls".to_vec()),
        ("BDMV/META/DL/bdmt_eng.xml", b"<di:name>Synthetic</di:name>".to_vec()),
        ("BDMV/CLIPINF/00000.clpi", b"clpi".to_vec()),
        ("BDMV/STREAM/00000.m2ts", vec![0u8; 100]),
        ("BDMV/STREAM/00001.m2ts", vec![0u8; 300]),
        ("AACS/Unit_Key_RO.inf", b"abc".to_vec()),
    ]
}

/// files.json for `disc_files`: sorted by path as bytes, two-space indent, no final newline.
const FILES_JSON: &[&str] = &[
    "[",
    "  {", "    \"path\": \"AACS/Unit_Key_RO.inf\",", "    \"size\": 3", "  },",
    "  {", "    \"path\": \"BDMV/CLIPINF/00000.clpi\",", "    \"size\": 4", "  },",
    "  {", "    \"path\": \"BDMV/META/DL/bdmt_eng.xml\",", "    \"size\": 28", "  },",
    "  {", "    \"path\": \"BDMV/MovieObject.bdmv\",", "    \"size\": 11", "  },",
    "  {", "    \"path\": \"BDMV/PLAYLIST/00800.mpls\",", "    \"size\": 4", "  },",
    "  {", "    \"path\": \"BDMV/STREAM/00000.m2ts\",", "    \"size\": 100", "  },",
    "  {", "    \"path\": \"BDMV/STREAM/00001.m2ts\",", "    \"size\": 300", "  },",
    "  {", "    \"path\": \"BDMV/index.bdmv\",", "    \"size\": 5", "  }",
    "]",
];

/// meta.json for `disc_files` with `captured_unix` masked to 0. The fingerprint is the MD5 of the sizes 100 and 300 as
/// little-endian 64-bit integers; the disc ID is the SHA-1 of "abc".
const META_JSON: &[&str] = &[
    "{",
    "  \"volume_label\": \"SYNTHETIC\",",
    "  \"format\": \"bluray\",",
    "  \"fingerprint\": \"881150E67DD260B285FD74856BFEEBE2\",",
    "  \"disc_id\": \"A9993E364706816ABA3E25717850C26C9CD0D89D\",",
    "  \"file_count\": 8,",
    "  \"total_bytes\": 455,",
    "  \"captured_unix\": 0",
    "}",
];

/// The navigation files copied under `nav/`, sorted.
const NAV_FILES: [&str; 4] =
    ["BDMV/META/DL/bdmt_eng.xml", "BDMV/MovieObject.bdmv", "BDMV/PLAYLIST/00800.mpls", "BDMV/index.bdmv"];

/// A fresh, empty folder under the system's temporary directory.
fn scratch(tag: &str) -> PathBuf {
    static COUNT: AtomicUsize = AtomicUsize::new(0);
    let n = COUNT.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("lazymkv-capture-{tag}-{}-{n}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// The made-up disc written under a fresh root.
fn disc_root() -> PathBuf {
    let root = scratch("disc");
    for (path, bytes) in disc_files() {
        let file = root.join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, bytes).unwrap();
    }
    root
}

/// `lazymkv-capture disc --root <root> --label SYNTHETIC --out <out>`.
fn capture_disc(root: &Path, out: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lazymkv-capture"))
        .arg("disc")
        .arg("--root")
        .arg(root)
        .args(["--label", "SYNTHETIC", "--out"])
        .arg(out)
        .output()
        .unwrap()
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

fn text(path: &Path) -> String {
    String::from_utf8(fs::read(path).unwrap()).unwrap()
}

/// `text` with the digits after `"captured_unix": ` replaced by 0, and the value they held.
fn mask_captured_unix(text: &str) -> (String, u64) {
    let key = "\"captured_unix\": ";
    let start = text.find(key).expect("captured_unix is written") + key.len();
    let digits = text[start..].bytes().take_while(u8::is_ascii_digit).count();
    let value = text[start..start + digits].parse().unwrap();
    (format!("{}0{}", &text[..start], &text[start + digits..]), value)
}

/// Every file under `dir`, as `/`-joined paths relative to it, sorted.
fn relative_files(dir: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, found: &mut Vec<String>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, found);
            } else {
                let parts: Vec<String> = path
                    .strip_prefix(root)
                    .unwrap()
                    .components()
                    .map(|part| part.as_os_str().to_string_lossy().into_owned())
                    .collect();
                found.push(parts.join("/"));
            }
        }
    }
    let mut found = Vec::new();
    walk(dir, dir, &mut found);
    found.sort();
    found
}

#[test]
fn snapshot_output_is_pinned_byte_for_byte() {
    let root = disc_root();
    let out = scratch("out").join("snapshot"); // not yet created: capture makes it
    let before = unix_now();
    let output = capture_disc(&root, &out);
    let after = unix_now();
    assert_eq!(output.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    assert_eq!(text(&out.join("files.json")), FILES_JSON.join("\n"));
    let (meta, captured) = mask_captured_unix(&text(&out.join("meta.json")));
    assert_eq!(meta, META_JSON.join("\n"));
    assert!((before..=after).contains(&captured), "captured_unix {captured} not in {before}..={after}");
    let (stdout, printed) = mask_captured_unix(&String::from_utf8(output.stdout).unwrap());
    assert_eq!(stdout, META_JSON.join("\n") + "\n");
    assert_eq!(printed, captured);

    let mut top: Vec<String> =
        fs::read_dir(&out).unwrap().map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned()).collect();
    top.sort();
    assert_eq!(top, ["files.json", "meta.json", "nav"]);
    assert_eq!(relative_files(&out.join("nav")), NAV_FILES);
    for path in NAV_FILES {
        assert_eq!(fs::read(out.join("nav").join(path)).unwrap(), fs::read(root.join(path)).unwrap(), "{path}");
    }
}

#[test]
fn core_snapshot_functions_agree_with_captures_output() {
    use lazymkv_core::snapshot::{disc_meta, is_nav_file, unit_key_file};
    let root = disc_root();
    let out = scratch("out").join("snapshot");
    assert_eq!(capture_disc(&root, &out).status.code(), Some(0));

    let mut files: Vec<DiscFile> = disc_files()
        .into_iter()
        .map(|(path, bytes)| DiscFile { path: path.to_string(), size: bytes.len() as u64 })
        .collect();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let written = fs::read(out.join("meta.json")).unwrap();
    let (_, captured) = mask_captured_unix(&String::from_utf8(written.clone()).unwrap());

    let key = unit_key_file(&files).expect("the disc has a unit key");
    assert_eq!(key.path, "AACS/Unit_Key_RO.inf");
    let key_bytes = fs::read(root.join(&key.path)).unwrap();
    let meta = disc_meta(&files, "SYNTHETIC", Some(key_bytes.as_slice()), captured);
    assert_eq!(serde_json::to_vec_pretty(&meta).unwrap(), written);

    let nav: Vec<&str> = files.iter().filter(|file| is_nav_file(&file.path)).map(|file| file.path.as_str()).collect();
    assert_eq!(nav, relative_files(&out.join("nav")));
}
