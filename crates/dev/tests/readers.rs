//! The six readers (port spec 3.4): their registration, and their output on the evidence clone (corpus/owner) against
//! the Python's, byte for byte by SHA-1. Every comparison whose input is absent or changed is skipped with its
//! reason; without the clone every evidence test skips.

use lazymkv_core::testsupport::sha1_hex;
use lazymkv_dev::evidence::{repo_root, skip, Ctx};
use std::path::{Path, PathBuf};

/// One run of a reader as the Python made it: the arguments (from the repository root), the inputs (paths below
/// corpus/owner) and the SHA-1 of their bytes, the SHA-1 of stdout and the exit code.
struct Recorded {
    args: &'static [&'static str],
    inputs: &'static [&'static str],
    inputs_sha1: &'static str,
    stdout_sha1: &'static str,
    exit: i32,
}

/// Python 3.9.6, 2026-10-13.
const RECORDED: &[Recorded] = &[
    Recorded { args: &["exp02", "corpus/owner/pans-labyrinth-2006-uhd"], inputs: &["pans-labyrinth-2006-uhd/scan.txt", "pans-labyrinth-2006-uhd/exp02/title-0.txt", "pans-labyrinth-2006-uhd/exp02/title-1.txt"], inputs_sha1: "3e2e30f0f192397879bd49489c44ff12391a07e2", stdout_sha1: "06725ab459692a709e6c5ca46cbb3a8d607985e4", exit: 0 },
    Recorded { args: &["encoding", "corpus/owner/pans-labyrinth-2006-uhd/scan.txt"], inputs: &["pans-labyrinth-2006-uhd/scan.txt"], inputs_sha1: "1f595955154dc29997750bce759721fb6fef269f", stdout_sha1: "a900415f008db3aea5c4f4890c2735fd0b822342", exit: 0 },
    Recorded { args: &["encoding", "corpus/owner/pans-labyrinth-2006-uhd/scan-offline.txt"], inputs: &["pans-labyrinth-2006-uhd/scan-offline.txt"], inputs_sha1: "bce602ebc5bc62909cac5c0d809049ee7872e9ce", stdout_sha1: "a900415f008db3aea5c4f4890c2735fd0b822342", exit: 0 },
    Recorded { args: &["encoding", "corpus/owner/short-circuit-1986-dvd/scan.txt"], inputs: &["short-circuit-1986-dvd/scan.txt"], inputs_sha1: "bea2a4a6a3bf50162be4e884c1270f53245ceb53", stdout_sha1: "a900415f008db3aea5c4f4890c2735fd0b822342", exit: 0 },
    Recorded { args: &["encoding", "corpus/owner/short-circuit-2-1988-dvd/scan.txt"], inputs: &["short-circuit-2-1988-dvd/scan.txt"], inputs_sha1: "a765be8a61d23394afce4ac271ea20936cdaf2a4", stdout_sha1: "1b55ac92233e210759a95604521f37e180ae5bb4", exit: 0 },
    Recorded { args: &["encoding", "corpus/owner/the-office-2005-s3d3-bluray/scan.txt"], inputs: &["the-office-2005-s3d3-bluray/scan.txt"], inputs_sha1: "3fbf1539f965b106ddc9f7aa5778a5f8de3ab576", stdout_sha1: "a900415f008db3aea5c4f4890c2735fd0b822342", exit: 0 },
    Recorded { args: &["compare-scans", "corpus/owner/pans-labyrinth-2006-uhd/scan.txt", "corpus/owner/pans-labyrinth-2006-uhd/scan-offline.txt"], inputs: &["pans-labyrinth-2006-uhd/scan.txt", "pans-labyrinth-2006-uhd/scan-offline.txt"], inputs_sha1: "89ac1f2400c795ff6cb0717a5e5f289ae1775b37", stdout_sha1: "60b7cb1240ef87e3ba9d455e3c6ea8e24290effb", exit: 0 },
    Recorded { args: &["exp11", "coverage", "corpus/owner/pans-labyrinth-2006-uhd"], inputs: &["pans-labyrinth-2006-uhd/scan.txt"], inputs_sha1: "1f595955154dc29997750bce759721fb6fef269f", stdout_sha1: "42bf1258d71e6cd8a4778aa21cb604cfc8c6c160", exit: 0 },
    Recorded { args: &["exp11", "coverage", "corpus/owner/short-circuit-1986-dvd"], inputs: &["short-circuit-1986-dvd/scan.txt"], inputs_sha1: "bea2a4a6a3bf50162be4e884c1270f53245ceb53", stdout_sha1: "ddd5627d8867bba5e019672ce980adec2ffa611c", exit: 0 },
    Recorded { args: &["exp11", "coverage", "corpus/owner/short-circuit-2-1988-dvd"], inputs: &["short-circuit-2-1988-dvd/scan.txt"], inputs_sha1: "a765be8a61d23394afce4ac271ea20936cdaf2a4", stdout_sha1: "670424614a5392ecd844e9757c42c98f4aa34ac3", exit: 0 },
    Recorded { args: &["exp11", "coverage", "corpus/owner/the-office-2005-s3d3-bluray"], inputs: &["the-office-2005-s3d3-bluray/scan.txt"], inputs_sha1: "3fbf1539f965b106ddc9f7aa5778a5f8de3ab576", stdout_sha1: "735f8e0a0629a7b154b006da0977977053fc4fdd", exit: 0 },
    Recorded { args: &["exp11", "coverage", "corpus/owner/pans-labyrinth-2006-uhd@0"], inputs: &["pans-labyrinth-2006-uhd/scan.txt"], inputs_sha1: "1f595955154dc29997750bce759721fb6fef269f", stdout_sha1: "15d66cbfa478b060bc94203d1c8ba71b9f1ebfe4", exit: 0 },
    Recorded { args: &["exp11", "coverage", "corpus/owner/pans-labyrinth-2006-uhd@1"], inputs: &["pans-labyrinth-2006-uhd/scan.txt"], inputs_sha1: "1f595955154dc29997750bce759721fb6fef269f", stdout_sha1: "a5727345a65eafe369c227cf4e02cf37491548ab", exit: 0 },
    Recorded { args: &["exp11", "coverage", "corpus/owner/short-circuit-1986-dvd@0"], inputs: &["short-circuit-1986-dvd/scan.txt"], inputs_sha1: "bea2a4a6a3bf50162be4e884c1270f53245ceb53", stdout_sha1: "abb3a9055827db2ce34843c925dda6a712f6b884", exit: 0 },
    Recorded { args: &["exp11", "coverage", "corpus/owner/short-circuit-2-1988-dvd@0"], inputs: &["short-circuit-2-1988-dvd/scan.txt"], inputs_sha1: "a765be8a61d23394afce4ac271ea20936cdaf2a4", stdout_sha1: "a8d5961514cf7a2be881fa34ffe770817a14a73b", exit: 0 },
    Recorded { args: &["exp11", "coverage", "corpus/owner/the-office-2005-s3d3-bluray@0"], inputs: &["the-office-2005-s3d3-bluray/scan.txt"], inputs_sha1: "3fbf1539f965b106ddc9f7aa5778a5f8de3ab576", stdout_sha1: "714842f09f56ee64ef30dd9ea86696c37b0c5c54", exit: 0 },
    Recorded { args: &["exp11", "coverage", "corpus/owner/the-office-2005-s3d3-bluray@1"], inputs: &["the-office-2005-s3d3-bluray/scan.txt"], inputs_sha1: "3fbf1539f965b106ddc9f7aa5778a5f8de3ab576", stdout_sha1: "c646cc437d29e92632be8ad073128e91cde77e0b", exit: 0 },
    Recorded { args: &["exp11", "coverage", "corpus/owner/the-office-2005-s3d3-bluray@3"], inputs: &["the-office-2005-s3d3-bluray/scan.txt"], inputs_sha1: "3fbf1539f965b106ddc9f7aa5778a5f8de3ab576", stdout_sha1: "b18d3be7236af2afeb2a657ae1ed5abbbc4d19ac", exit: 0 },
    Recorded { args: &["exp11", "match", "corpus/owner/pans-labyrinth-2006-uhd", "0", "all-tracks"], inputs: &["pans-labyrinth-2006-uhd/scan.txt", "pans-labyrinth-2006-uhd/all-tracks/mkvmerge.json", "pans-labyrinth-2006-uhd/all-tracks/rip-summary.json"], inputs_sha1: "03d4a96a83c248a4a8346fecb5d35416ca671a19", stdout_sha1: "04a841db3aa3bcb3dcf2d1a99fdeafbd06dd9faf", exit: 0 },
    Recorded { args: &["exp11", "match", "corpus/owner/pans-labyrinth-2006-uhd", "0", "eng-only"], inputs: &["pans-labyrinth-2006-uhd/scan.txt", "pans-labyrinth-2006-uhd/eng-only/mkvmerge.json", "pans-labyrinth-2006-uhd/all-tracks/mkvmerge.json", "pans-labyrinth-2006-uhd/all-tracks/rip-summary.json"], inputs_sha1: "a2f82c11f08c6ab5b48f25b96b541d71c98b13ad", stdout_sha1: "9952d4cf9c6974681b17494c40a95b6a1fe11a55", exit: 0 },
    Recorded { args: &["exp11", "match", "corpus/owner/pans-labyrinth-2006-uhd", "1", "rehearsal"], inputs: &["pans-labyrinth-2006-uhd/scan.txt", "pans-labyrinth-2006-uhd/rehearsal/mkvmerge.json", "pans-labyrinth-2006-uhd/all-tracks/mkvmerge.json", "pans-labyrinth-2006-uhd/all-tracks/rip-summary.json"], inputs_sha1: "79bf8e9d9b35ea5175e243521f05dd1d201a1787", stdout_sha1: "53ef18ca38f35440b72c4eba7fa47a41cd0d4436", exit: 0 },
    Recorded { args: &["exp11", "match", "corpus/owner/short-circuit-1986-dvd", "0", "all-tracks"], inputs: &["short-circuit-1986-dvd/scan.txt", "short-circuit-1986-dvd/all-tracks/mkvmerge.json", "short-circuit-1986-dvd/all-tracks/rip-summary.json"], inputs_sha1: "7819ec049ab613af841f960d1b2d8c1c0accf8ab", stdout_sha1: "f8cb014778e0a2c53eb912b49e957682888fdb45", exit: 0 },
    Recorded { args: &["exp11", "match", "corpus/owner/short-circuit-2-1988-dvd", "0", "all-tracks-2"], inputs: &["short-circuit-2-1988-dvd/scan.txt", "short-circuit-2-1988-dvd/all-tracks-2/mkvmerge.json", "short-circuit-2-1988-dvd/all-tracks/rip-summary.json"], inputs_sha1: "2bd1ea40c7c5b9f9e8028027d6080cc46b6f3955", stdout_sha1: "ea8a15c3d0f454ce980803c5d5cb615f6e486725", exit: 0 },
    Recorded { args: &["exp11", "match", "corpus/owner/short-circuit-2-1988-dvd", "0", "eng-only"], inputs: &["short-circuit-2-1988-dvd/scan.txt", "short-circuit-2-1988-dvd/eng-only/mkvmerge.json", "short-circuit-2-1988-dvd/all-tracks/rip-summary.json"], inputs_sha1: "d440592af81e6c7fd32f2fd27d7012f372dd3f65", stdout_sha1: "384720117e0f4749504a99d7a6079ef27f84feca", exit: 0 },
    Recorded { args: &["exp11", "match", "corpus/owner/the-office-2005-s3d3-bluray", "0", "e16-stereo"], inputs: &["the-office-2005-s3d3-bluray/scan.txt", "the-office-2005-s3d3-bluray/e16-stereo/mkvmerge.json"], inputs_sha1: "e3fc1705e9415c70dd25f914d16b93ab29c5c20c", stdout_sha1: "18f15910e06a07fca1bc89e3c7e0772c07be13dd", exit: 0 },
    Recorded { args: &["exp11", "match", "corpus/owner/the-office-2005-s3d3-bluray", "1", "rehearsal"], inputs: &["the-office-2005-s3d3-bluray/scan.txt", "the-office-2005-s3d3-bluray/rehearsal/mkvmerge.json"], inputs_sha1: "84e3b3c5c86437fb4b78b2e52282555e39d01bed", stdout_sha1: "c74cc3076d080f345a39f30735b66de0489ecebd", exit: 0 },
    Recorded { args: &["exp11", "match", "corpus/owner/the-office-2005-s3d3-bluray", "3", "t3-all-tracks"], inputs: &["the-office-2005-s3d3-bluray/scan.txt", "the-office-2005-s3d3-bluray/t3-all-tracks/mkvmerge.json"], inputs_sha1: "4702c1470f80910d980ae328b6ec42261ece6fc2", stdout_sha1: "288bc7124479212679423478d6e1d79893251e0e", exit: 0 },
];

/// A recording's line of `hit-rate corpus/owner` as the Python printed it, and the inputs it rests on (below the
/// recording's folder) with the SHA-1 of their bytes. The whole output is not compared: a new recording changes the
/// totals, not these lines.
struct HitLine {
    slug: &'static str,
    inputs: &'static [&'static str],
    inputs_sha1: &'static str,
    line: &'static str,
}

/// Python 3.9.6, 2026-10-13.
const HIT_LINES: &[HitLine] = &[
    HitLine { slug: "pans-labyrinth-2006-uhd", inputs: &["meta.json", "lookup-status.txt", "thediscdb-hash.json", "thediscdb-discid.json"], inputs_sha1: "276cd9e6eb6f0e62ffbf5a8c5649c62dd1d38de9", line: "pans-labyrinth-2006-uhd                  bluray  hash=hit    discid=miss" },
    HitLine { slug: "short-circuit-1986-dvd", inputs: &["meta.json", "lookup-status.txt", "thediscdb-hash.json"], inputs_sha1: "82b83590b332f593793bf24de724453df7378c84", line: "short-circuit-1986-dvd                   dvd     hash=miss   discid=n/a" },
    HitLine { slug: "short-circuit-2-1988-dvd", inputs: &["meta.json", "lookup-status.txt", "thediscdb-hash.json"], inputs_sha1: "dac242ea4976990391ff5c0a5216837e6888ccbf", line: "short-circuit-2-1988-dvd                 dvd     hash=miss   discid=n/a" },
    HitLine { slug: "the-office-2005-s3d3-bluray", inputs: &["meta.json", "lookup-status.txt", "thediscdb-hash.json", "thediscdb-discid.json"], inputs_sha1: "d932d5ef92e4357a409241f63b994b865c93bad5", line: "the-office-2005-s3d3-bluray              bluray  hash=hit    discid=miss" },
];

/// The repository root and the evidence clone under it, or why the evidence tests skip.
fn evidence() -> Result<(PathBuf, PathBuf), String> {
    let root = repo_root(Path::new(env!("CARGO_MANIFEST_DIR")))
        .ok_or_else(|| "no repository root (a Cargo.toml with [workspace]) above crates/dev".to_string())?;
    let owner = root.join("corpus").join("owner");
    if !owner.is_dir() {
        return Err("no evidence clone at corpus/owner".to_string());
    }
    Ok((root, owner))
}

/// The bytes of `inputs` (paths below `base`) one after the other, or the first path that is absent.
fn inputs_of(base: &Path, inputs: &[&str]) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    for input in inputs {
        match std::fs::read(base.join(input)) {
            Ok(content) => bytes.extend(content),
            Err(_) => return Err(input.to_string()),
        }
    }
    Ok(bytes)
}

/// lazymkv-dev with `args`, run from `cwd`: (exit code, stdout bytes, stderr).
fn run(cwd: &Path, args: &[&str]) -> (i32, Vec<u8>, String) {
    let ctx = Ctx::for_test(cwd);
    let argv: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = lazymkv_dev::dispatch(&ctx, &argv, &mut out, &mut err);
    (code, out, String::from_utf8_lossy(&err).into_owned())
}

#[test]
fn the_six_readers_are_registered_in_spec_order() {
    let rows: Vec<(&str, &str)> = lazymkv_dev::COMMANDS.iter().map(|(name, _, synopsis)| (*name, *synopsis)).collect();
    let readers = [
        ("exp02", "<dir>"),
        ("encoding", "<file>"),
        ("hit-rate", "<dir>"),
        ("exp11", "coverage <where>... | match <folder> <title> <label>"),
        ("compare-scans", "<A> <B>"),
        ("ocr-compare", "<tess> <ocrs> [--rec DIR] [--tag CODE]"),
    ];
    let Some(first) = rows.iter().position(|(name, _)| *name == "exp02") else {
        panic!("exp02 is not registered: {rows:?}");
    };
    assert!(rows.len() >= first + readers.len(), "{rows:?}");
    assert_eq!(rows[first..first + readers.len()], readers, "{rows:?}");
    if let Some(proxy) = rows.iter().position(|(name, _)| *name == "levels-from-proxy") {
        assert_eq!(first, proxy + 1, "the readers follow levels-from-proxy: {rows:?}");
    }
}

#[test]
fn every_recorded_reader_run_gives_the_pythons_output() {
    let (root, owner) = match evidence() {
        Ok(found) => found,
        Err(reason) => {
            skip(&reason);
            return;
        }
    };
    for r in RECORDED {
        let bytes = match inputs_of(&owner, r.inputs) {
            Ok(bytes) => bytes,
            Err(path) => {
                skip(&format!("corpus/owner/{path} is not in the evidence clone"));
                continue;
            }
        };
        if sha1_hex(&bytes) != r.inputs_sha1 {
            skip(&format!("lazymkv-dev {}: its inputs changed since the Python's output was recorded", r.args.join(" ")));
            continue;
        }
        let (code, out, err) = run(&root, r.args);
        let context = format!("lazymkv-dev {}\nstdout:\n{}stderr:\n{err}", r.args.join(" "), String::from_utf8_lossy(&out));
        assert_eq!(code, r.exit, "{context}");
        assert_eq!(sha1_hex(&out), r.stdout_sha1, "{context}");
        assert!(err.is_empty(), "{context}");
    }
}

#[test]
fn hit_rate_lists_every_recording_as_the_python_did() {
    let (root, owner) = match evidence() {
        Ok(found) => found,
        Err(reason) => {
            skip(&reason);
            return;
        }
    };
    let (code, out, err) = run(&root, &["hit-rate", "corpus/owner"]);
    let out = String::from_utf8(out).unwrap();
    // 0 or 1 by the lookups of every recording there (a new one may have failed lookups); 2 would be a crash.
    assert!(code == 0 || code == 1, "exit {code}\n{out}{err}");
    for h in HIT_LINES {
        let bytes = match inputs_of(&owner.join(h.slug), h.inputs) {
            Ok(bytes) => bytes,
            Err(path) => {
                skip(&format!("corpus/owner/{}/{path} is not in the evidence clone", h.slug));
                continue;
            }
        };
        if sha1_hex(&bytes) != h.inputs_sha1 {
            skip(&format!("{}: its lookups changed since the Python's output was recorded", h.slug));
            continue;
        }
        assert!(out.lines().any(|line| line == h.line), "{}\n{out}", h.line);
    }
}

/// Every sub-<n>.ocr.jsonl below `dir`.
fn ocr_texts(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            ocr_texts(&path, found);
        } else if name.starts_with("sub-") && name.ends_with(".ocr.jsonl") {
            found.push(path);
        }
    }
}

#[test]
fn ocr_compare_reads_a_reading_the_same_as_itself() {
    let Some(text) = std::env::var_os("LAZYMKV_OCR_TEXT").map(PathBuf::from).filter(|dir| dir.is_dir()) else {
        skip("no OCR text: give LAZYMKV_OCR_TEXT, a folder holding sub-<n>.ocr.jsonl files (fetched with tuning-fetch.ps1 -Text, deleted after use)");
        return;
    };
    let mut files = Vec::new();
    ocr_texts(&text, &mut files);
    files.sort();
    let Some(file) = files.into_iter().find(|file| std::fs::metadata(file).map_or(false, |meta| meta.len() > 0)) else {
        skip("no sub-<n>.ocr.jsonl with a record under LAZYMKV_OCR_TEXT");
        return;
    };
    let arg = file.to_string_lossy().into_owned();
    let (code, out, _) = run(&text, &["ocr-compare", arg.as_str(), arg.as_str()]);
    if code == 2 {
        skip("the first OCR text file holds no frame record");
        return;
    }
    // No assertion message below prints the output: if the command printed a line of the text, the message would.
    assert_eq!(code, 0, "ocr-compare of a reading with itself did not exit 0");
    let out = String::from_utf8(out).unwrap();
    let exact = out.lines().find(|line| line.starts_with("exact match after folding: ")).unwrap_or_default();
    let words: Vec<&str> = exact.split(' ').collect();
    assert!(words.len() > 6 && words[4] == words[6], "not every frame with text matched itself");
    assert!(out.contains(" mean 0.000, median 0.000 over "), "the character error rate is not 0");
    assert!(out.lines().any(|line| line == "  none: every frame with text reads the same"), "a difference was listed");
    // The reading's own lines (12 characters or more) appear nowhere in the output.
    let content = std::fs::read_to_string(&file).unwrap_or_default();
    for record in content.lines().filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok()) {
        let mut pieces: Vec<String> = Vec::new();
        if let Some(serde_json::Value::Array(items)) = record.get("lines") {
            pieces.extend(items.iter().filter_map(|item| item.as_str()).map(str::to_string));
        }
        if let Some(joined) = record.get("text").and_then(|value| value.as_str()) {
            pieces.push(joined.to_string());
        }
        for piece in pieces.iter().map(|piece| piece.trim()).filter(|piece| piece.chars().count() >= 12) {
            assert!(!out.contains(piece), "ocr-compare printed a line of subtitle text");
        }
    }
}
