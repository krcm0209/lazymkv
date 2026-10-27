//! Ruling RP21 pinned: no file under `crates/` holds a value copied from the private evidence clone (`corpus/owner`)
//! that a reader could not derive from the public spec or the pinned MIT fixtures. The values checked are the kinds the
//! final review found copied into tests: every quoted CINFO 2 or 32 value (the disc's name and its volume label) and
//! every TINFO 11 size of `corpus/owner/<slug>/scan.txt`, and every TheDiscDB item title of
//! `corpus/owner/<slug>/thediscdb-hash.json`. A value the public tree already publishes may stay (the design and port
//! specs, `docs/phase0`, the pinned fixtures under `corpus/thediscdb`, `corpus/thediscdb-api` and
//! `corpus/scenarios/disc-a-scan`, and the READMEs): RP21 allows what the spec or the pinned fixtures publish. The
//! `.rs` and `.json` files under `crates/` are checked, and so is every other scenario under `corpus/scenarios/`, whose
//! values must be synthetic or come from `corpus/thediscdb-api` and `core::testsupport` (Task 23). The clone is read
//! when the test runs, so the values stay out of this file too. Skips without the evidence clone
//! (`lazymkv_dev::evidence::skip`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use lazymkv_core::recording::{decode_text, Encoding, Errors};
use lazymkv_core::robot::parse;
use lazymkv_dev::evidence::{resolve_evidence, skip, Ctx};

/// The repository root (crates/dev is two levels down).
fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The public sources a reader has besides the code: the files under these folders of the repository, and these files.
const PUBLIC_FOLDERS: [&str; 5] = [
    "docs/phase0",
    "docs/superpowers/specs",
    "corpus/thediscdb",
    "corpus/thediscdb-api",
    "corpus/scenarios/disc-a-scan",
];
const PUBLIC_FILES: [&str; 2] = ["README.md", "corpus/README.md"];

/// Every file under `dir`, recursively, in no particular order.
fn files_under(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files_under(&path, found);
        } else {
            found.push(path);
        }
    }
}

/// A file's text, invalid bytes replaced.
fn text_of(path: &Path) -> String {
    let bytes = std::fs::read(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    String::from_utf8_lossy(&bytes).into_owned()
}

/// `text` with the separators a number may carry between its digits removed: `_` (a Rust literal) and `,` (prose).
fn plain_digits(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (at, &c) in chars.iter().enumerate() {
        let between = at > 0
            && at + 1 < chars.len()
            && chars[at - 1].is_ascii_digit()
            && chars[at + 1].is_ascii_digit();
        if (c == '_' || c == ',') && between {
            continue;
        }
        out.push(c);
    }
    out
}

/// Whether `digits` appears in `text` as a whole number (no digit right before or after it).
fn holds_number(text: &str, digits: &str) -> bool {
    text.match_indices(digits).any(|(at, _)| {
        let before = text[..at].chars().next_back().is_some_and(|c| c.is_ascii_digit());
        let after = text[at + digits.len()..].chars().next().is_some_and(|c| c.is_ascii_digit());
        !before && !after
    })
}

/// One value of the clone and how a source may hold it.
enum CloneValue {
    /// A CINFO 2 or 32 value: held when a source quotes it whole ("<value>").
    Quoted(String),
    /// A TheDiscDB item title: held when a source contains it anywhere (a file name built from it holds it too).
    Text(String),
    /// A TINFO 11 size: held when a source has it as a whole number, `_` and `,` separators ignored.
    Number(String),
}

/// The clone's values, each with where it was read ("<slug> CINFO 2", "<slug> title 3 size", "<slug> item title").
fn clone_values(evidence: &Path) -> BTreeMap<String, (String, CloneValue)> {
    let mut values = BTreeMap::new();
    let mut slugs: Vec<PathBuf> = std::fs::read_dir(evidence)
        .unwrap_or_else(|error| panic!("{}: {error}", evidence.display()))
        .flatten()
        .map(|entry| entry.path())
        .filter(|dir| dir.join("scan.txt").is_file())
        .collect();
    slugs.sort();
    for dir in slugs {
        let slug = dir.file_name().unwrap().to_string_lossy().into_owned();
        let bytes = std::fs::read(dir.join("scan.txt")).unwrap();
        let scan = parse(&decode_text(&bytes, Encoding::Utf8Sig, Errors::Replace).unwrap());
        for id in [2, 32] {
            if let Some(value) = scan.disc.get(&id).filter(|value| !value.trim().is_empty()) {
                let key = format!("quoted {value}");
                values.entry(key).or_insert((format!("{slug} CINFO {id}"), CloneValue::Quoted(value.clone())));
            }
        }
        for (title, attrs) in &scan.titles {
            if let Some(size) = attrs.get(&11).filter(|size| !size.is_empty() && size.bytes().all(|b| b.is_ascii_digit())) {
                let key = format!("number {size}");
                values.entry(key).or_insert((format!("{slug} title {title} size"), CloneValue::Number(size.clone())));
            }
        }
        let answer = dir.join("thediscdb-hash.json");
        let Ok(text) = std::fs::read_to_string(&answer) else { continue };
        let Ok(record) = serde_json::from_str::<serde_json::Value>(text.trim_start_matches('\u{feff}')) else { continue };
        for title in record["titles"].as_array().into_iter().flatten() {
            if let Some(name) = title["item"]["title"].as_str().filter(|name| !name.trim().is_empty()) {
                let key = format!("text {name}");
                values.entry(key).or_insert((format!("{slug} item title"), CloneValue::Text(name.to_string())));
            }
        }
    }
    values
}

/// Where the clone's `values` are held among `texts` (path, text, `plain_digits` of the text), one line per holder
/// naming the file and where the value was read, never the value itself; a value `public` already publishes is skipped.
fn holders(
    root: &Path,
    public: &str,
    values: &BTreeMap<String, (String, CloneValue)>,
    texts: &[(PathBuf, String, String)],
) -> Vec<String> {
    let mut found = Vec::new();
    for (whence, value) in values.values() {
        let (published, held): (bool, Box<dyn Fn(&str, &str) -> bool>) = match value {
            CloneValue::Quoted(text) => {
                let quoted = format!("\"{text}\"");
                (public.contains(text.as_str()), Box::new(move |source: &str, _: &str| source.contains(&quoted)))
            }
            CloneValue::Text(text) => {
                let text = text.clone();
                (public.contains(text.as_str()), Box::new(move |source: &str, _: &str| source.contains(&text)))
            }
            CloneValue::Number(digits) => {
                let digits = digits.clone();
                (holds_number(public, &digits), Box::new(move |_: &str, plain: &str| holds_number(plain, &digits)))
            }
        };
        if published {
            continue;
        }
        for (path, source, plain) in texts {
            if held(source, plain) {
                let shown = path.strip_prefix(root).unwrap_or(path).display().to_string();
                // The value itself is not printed (it is the clone's): where it was read and where it was found.
                found.push(format!("{shown}: holds the value of {whence}"));
            }
        }
    }
    found
}

#[test]
fn no_file_in_crates_holds_a_value_only_the_evidence_clone_holds() {
    let mut ctx = Ctx::for_test(&std::env::current_dir().expect("a current directory"));
    if let Ok(value) = std::env::var("LAZYMKV_EVIDENCE") {
        ctx.env.insert("LAZYMKV_EVIDENCE".to_string(), value);
    }
    let evidence = match resolve_evidence(&ctx, None) {
        Ok(dir) => dir,
        Err(refused) => return skip(&refused.line),
    };
    let root = repository();
    let mut public_files: Vec<PathBuf> = PUBLIC_FILES.iter().map(|file| root.join(file)).collect();
    for folder in PUBLIC_FOLDERS {
        files_under(&root.join(folder), &mut public_files);
    }
    let public: String = public_files.iter().filter(|path| path.is_file()).map(|path| plain_digits(&text_of(path))).collect();
    let mut sources = Vec::new();
    files_under(&root.join("crates"), &mut sources);
    sources.retain(|path| path.extension().is_some_and(|extension| extension == "rs" || extension == "json"));
    sources.sort();
    assert!(!sources.is_empty(), "no source under {}", root.join("crates").display());
    let texts: Vec<(PathBuf, String, String)> =
        sources.into_iter().map(|path| (path.clone(), text_of(&path), plain_digits(&text_of(&path)))).collect();

    let values = clone_values(&evidence);
    assert!(!values.is_empty(), "the clone at {} gave no value to look for", evidence.display());
    let found = holders(&root, &public, &values, &texts);
    assert!(
        found.is_empty(),
        "ruling RP21: values of the private evidence clone are written in crates/ (read them from the clone when the \
         test runs, or use made-up values of the same shape):\n{}",
        found.join("\n")
    );
}

#[test]
fn no_scenario_holds_a_clone_value() {
    let mut ctx = Ctx::for_test(&std::env::current_dir().expect("a current directory"));
    if let Ok(value) = std::env::var("LAZYMKV_EVIDENCE") {
        ctx.env.insert("LAZYMKV_EVIDENCE".to_string(), value);
    }
    let evidence = match resolve_evidence(&ctx, None) {
        Ok(dir) => dir,
        Err(refused) => return skip(&refused.line),
    };
    let root = repository();
    let mut public_files: Vec<PathBuf> = PUBLIC_FILES.iter().map(|file| root.join(file)).collect();
    for folder in PUBLIC_FOLDERS {
        files_under(&root.join(folder), &mut public_files);
    }
    let public: String = public_files.iter().filter(|p| p.is_file()).map(|p| plain_digits(&text_of(p))).collect();
    let mut sources = Vec::new();
    files_under(&root.join("corpus/scenarios"), &mut sources);
    sources.retain(|path| !path.starts_with(root.join("corpus/scenarios/disc-a-scan")));
    sources.sort();
    assert!(!sources.is_empty(), "no scenario under corpus/scenarios besides disc-a-scan");
    let texts: Vec<(PathBuf, String, String)> = sources.into_iter().map(|p| (p.clone(), text_of(&p), plain_digits(&text_of(&p)))).collect();
    let found = holders(&root, &public, &clone_values(&evidence), &texts);
    assert!(found.is_empty(), "ruling RP21: a scenario holds values of the private evidence clone:\n{}", found.join("\n"));
}

#[test]
fn numbers_are_found_whole_and_their_separators_are_ignored() {
    assert!(holds_number("size 4096 bytes", "4096"));
    assert!(!holds_number("size 40960 bytes", "4096"));
    assert!(!holds_number("size 14096", "4096"));
    assert_eq!(plain_digits("55,613,952,000 and 1_000_000, then a, b"), "55613952000 and 1000000, then a, b");
}
