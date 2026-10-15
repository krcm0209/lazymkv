//! `docs/phase1/rust-port-tests.md` maps the 213 Python tests of `scripts/phase0/test_*.py`, deleted by the Rust
//! port, to the Rust tests that port them one for one (port spec section 5, "Ported tests"). These tests check the
//! table so a reviewer can trust it: 213 rows numbered 1 to 213, exactly the Python tests in their files' order
//! (pinned by a SHA-1), the count per file of spec section 2, every Rust test named once and defined in the file its
//! row names, a task of the port that ports tests, and no placeholder left.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use lazymkv_core::recording::sha1_hex;
use lazymkv_dev::evidence::{repo_root, skip};

/// The table, from the repository root.
const TABLE: &str = "docs/phase1/rust-port-tests.md";
/// SHA-1 (hex) of the 213 names "<file>::<class>::<method>", each followed by "\n", in the order of the files by name
/// and of their lines: the list Task 15 Step 7 printed from the Python of the last commit that had it.
const PYTHON_NAMES_SHA1: &str = "6f6bbb40160c0bce4002ba9ea5913e2d0cab2e00";
/// The number of Python tests.
const PYTHON_TESTS: usize = 213;
/// `def test_` per Python file (port spec section 2).
const PER_FILE: [(&str, usize); 7] = [
    ("test_analysis.py", 13),
    ("test_audio_dataset.py", 64),
    ("test_audio_similarity.py", 36),
    ("test_decisions.py", 26),
    ("test_exp11_tracks.py", 5),
    ("test_ocr_compare.py", 20),
    ("test_subtitle_text.py", 49),
];
/// The tasks of the port's plan that port Python tests.
const PORTING_TASKS: std::ops::RangeInclusive<u32> = 2..=12;
/// The placeholders Task 15 replaces before the table is committed.
const PLACEHOLDERS: [&str; 2] = ["PY_COMMIT", "PR_NUMBER"];

/// One row: "| n | `python` | `rust` | `file` | task |".
struct Row {
    number: usize,
    python: String,
    rust: String,
    file: String,
    task: String,
}

/// A table row, or None for any other line (the header row's "#" and the rule's "---" are not numbers).
fn row(line: &str) -> Option<Row> {
    let inner = line.trim().strip_prefix('|')?.strip_suffix('|')?;
    let cells: Vec<&str> = inner.split('|').map(str::trim).collect();
    if cells.len() != 5 {
        return None;
    }
    let number = cells[0].parse().ok()?;
    let code = |cell: &str| cell.trim_matches('`').to_string();
    Some(Row { number, python: code(cells[1]), rust: code(cells[2]), file: code(cells[3]), task: cells[4].to_string() })
}

/// The repository root and the table's text; None (after a skip line) when there is no repository around the crate.
fn table_text() -> Option<(PathBuf, String)> {
    let Some(root) = repo_root(Path::new(env!("CARGO_MANIFEST_DIR"))) else {
        skip("no repository root (a Cargo.toml with [workspace]) above crates/dev");
        return None;
    };
    let text = std::fs::read_to_string(root.join(TABLE)).unwrap_or_else(|e| panic!("{TABLE}: {e}"));
    Some((root, text))
}

fn rows(text: &str) -> Vec<Row> {
    text.lines().filter_map(row).collect()
}

#[test]
fn the_table_lists_the_213_python_tests_in_their_files_order() {
    let Some((_, text)) = table_text() else { return };
    let rows = rows(&text);
    assert_eq!(rows.len(), PYTHON_TESTS, "{TABLE}: rows");
    for (i, r) in rows.iter().enumerate() {
        assert_eq!(r.number, i + 1, "{TABLE}: row {} is numbered {}", i + 1, r.number);
    }
    let names: String = rows.iter().map(|r| format!("{}\n", r.python)).collect();
    assert_eq!(
        sha1_hex(names.as_bytes()),
        PYTHON_NAMES_SHA1,
        "{TABLE}: the Python column is not the list of the Python tests in order"
    );
    for (file, count) in PER_FILE {
        let prefix = format!("{file}::");
        assert_eq!(rows.iter().filter(|r| r.python.starts_with(&prefix)).count(), count, "{TABLE}: {file}");
    }
}

#[test]
fn every_rust_test_is_named_once_and_defined_in_its_file() {
    let Some((root, text)) = table_text() else { return };
    let mut seen = BTreeSet::new();
    for r in rows(&text) {
        assert!(seen.insert(r.rust.clone()), "{TABLE}: row {}: {} is named twice", r.number, r.rust);
        let name = r.rust.rsplit("::").next().unwrap_or_default();
        assert!(!name.is_empty(), "{TABLE}: row {}: no Rust test", r.number);
        let source = std::fs::read_to_string(root.join(&r.file))
            .unwrap_or_else(|e| panic!("{TABLE}: row {}: {}: {e}", r.number, r.file));
        assert!(source.contains(&format!("fn {name}(")), "{TABLE}: row {}: no `fn {name}(` in {}", r.number, r.file);
        let task: u32 = r.task.parse().unwrap_or_else(|_| panic!("{TABLE}: row {}: task {:?}", r.number, r.task));
        assert!(PORTING_TASKS.contains(&task), "{TABLE}: row {}: task {task} ports no Python test", r.number);
    }
}

#[test]
fn no_placeholder_is_left() {
    let Some((_, text)) = table_text() else { return };
    for placeholder in PLACEHOLDERS {
        assert!(!text.contains(placeholder), "{TABLE} still holds {placeholder}");
    }
}
