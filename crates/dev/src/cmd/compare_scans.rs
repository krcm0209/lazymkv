//! `lazymkv-dev compare-scans <A> <B>`: experiments 3 and 5, the title lists of two recorded scans compared by source
//! file, segment map and duration. Py: compare_scans, its lines verbatim but for one deliberate change (spec 3.4): a
//! title key is printed as `(a, b, c)` with each part written by the JSON writer ("00800.mpls", null for an absent
//! attribute), not as Python's tuple repr; the lists keep the Python's order, sorted by that repr. Exit 0 when no
//! title is on one side only; 1 when one is, or when a scan has no title; 2 where the Python crashed (fewer than two
//! arguments, a scan unreadable).

use crate::evidence::{pathlib_str, read_scan_file, Ctx};
use crate::pyjson::{dumps, Style};
use crate::usage_error;
use lazymkv_core::recording::{py_repr_str, Encoding, Errors};
use lazymkv_core::robot::Scan;
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::Write;

/// The command's name as typed.
const NAME: &str = "compare-scans";
/// Its usage after "usage: lazymkv-dev ". The Python read sys.argv[1] and [2] and ignored any further word.
const USAGE: &str = "compare-scans <A> <B>";
/// The two arguments, as the usage names them.
const POSITIONALS: [&str; 2] = ["A", "B"];
/// The line a scan without titles adds.
const EMPTY_SCAN: &str = "EMPTY SCAN: a scan with no titles cannot be compared";

/// A title's key: its attributes 16 (source file), 26 (segment map) and 9 (duration), None when absent.
pub type Key = [Option<String>; 3];

/// One (key, comment) pair per title, the comment being attribute 49 ("" when absent). Py: compare_scans.titles.
pub fn titles(scan: &Scan) -> Vec<(Key, String)> {
    scan.titles
        .values()
        .map(|a| ([a.get(&16).cloned(), a.get(&26).cloned(), a.get(&9).cloned()], a.get(&49).cloned().unwrap_or_default()))
        .collect()
}

/// What compare found. Py: compare's dict.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Comparison {
    /// Keys more often in A than in B, once per surplus title, in the Python's order.
    pub only_a: Vec<Key>,
    /// Keys more often in B than in A.
    pub only_b: Vec<Key>,
    /// Keys on both sides whose sorted comment lists differ.
    pub comment_differs: Vec<Key>,
    /// Titles on both sides, each key counted as often as its smaller side has it.
    pub common: usize,
}

/// How often each key occurs. Py: Counter(key for key, _ in side).
fn counts(side: &[(Key, String)]) -> BTreeMap<Key, usize> {
    let mut counts = BTreeMap::new();
    for (key, _) in side {
        *counts.entry(key.clone()).or_insert(0) += 1;
    }
    counts
}

/// Each key's comments in order. Py: comments.setdefault(key, []).append(comment).
fn comments(side: &[(Key, String)]) -> BTreeMap<Key, Vec<String>> {
    let mut comments: BTreeMap<Key, Vec<String>> = BTreeMap::new();
    for (key, comment) in side {
        comments.entry(key.clone()).or_default().push(comment.clone());
    }
    comments
}

/// Every key as often as `mine` has it more than `theirs`. Py: (count_a - count_b).elements().
fn surplus(mine: &BTreeMap<Key, usize>, theirs: &BTreeMap<Key, usize>) -> Vec<Key> {
    let mut keys = Vec::new();
    for (key, n) in mine {
        let m = theirs.get(key).copied().unwrap_or(0);
        for _ in m..*n {
            keys.push(key.clone());
        }
    }
    keys
}

/// Titles are compared as multisets, so a title listed twice on one side is a difference. Py: compare_scans.compare.
pub fn compare(a: &[(Key, String)], b: &[(Key, String)]) -> Comparison {
    let (count_a, count_b) = (counts(a), counts(b));
    let (comments_a, comments_b) = (comments(a), comments(b));
    let mut only_a = surplus(&count_a, &count_b);
    let mut only_b = surplus(&count_b, &count_a);
    let mut comment_differs = Vec::new();
    for (key, mine) in &comments_a {
        if let Some(theirs) = comments_b.get(key) {
            let (mut mine, mut theirs) = (mine.clone(), theirs.clone());
            mine.sort();
            theirs.sort();
            if mine != theirs {
                comment_differs.push(key.clone());
            }
        }
    }
    let common: usize = count_a.iter().map(|(key, n)| (*n).min(count_b.get(key).copied().unwrap_or(0))).sum();
    for keys in [&mut only_a, &mut only_b, &mut comment_differs] {
        keys.sort_by_cached_key(python_repr);
    }
    Comparison { only_a, only_b, comment_differs, common }
}

/// str(key) in Python: "('00800.mpls', '1,2', '1:59:10')", None for an absent part. The order of the lists.
pub fn python_repr(key: &Key) -> String {
    let parts: Vec<String> = key.iter().map(|part| part.as_deref().map_or_else(|| "None".to_string(), py_repr_str)).collect();
    format!("({})", parts.join(", "))
}

/// A key as compare-scans prints it: "(\"00800.mpls\", \"1,2\", \"1:59:10\")", null for an absent part (spec 3.4).
pub fn shown(key: &Key) -> String {
    let parts: Vec<String> = key
        .iter()
        .map(|part| match part {
            Some(text) => dumps(&Value::String(text.clone()), Style::PY_DEFAULT),
            None => "null".to_string(),
        })
        .collect();
    format!("({})", parts.join(", "))
}

/// compare_scans.main's lines for two scans' titles, and its exit code. Py: compare_scans.main.
pub fn report_lines(a: &[(Key, String)], b: &[(Key, String)]) -> (Vec<String>, i32) {
    let result = compare(a, b);
    let mut lines = vec![format!("titles: A={} B={} common={}", a.len(), b.len(), result.common)];
    for (name, keys) in [("only in A", &result.only_a), ("only in B", &result.only_b), ("comment differs", &result.comment_differs)] {
        lines.push(format!("{name}: {}", keys.len()));
        for key in keys.iter() {
            lines.push(format!("  {}", shown(key)));
        }
    }
    if a.is_empty() || b.is_empty() {
        lines.push(EMPTY_SCAN.to_string());
        return (lines, 1);
    }
    let code = if result.only_a.is_empty() && result.only_b.is_empty() { 0 } else { 1 };
    (lines, code)
}

/// A scan's titles, read as Path.read_text(encoding="utf-8", errors="replace"). Py: compare_scans.titles.
fn read_titles(ctx: &Ctx, arg: &str) -> Result<Vec<(Key, String)>, String> {
    Ok(titles(&read_scan_file(&ctx.abs(arg), &pathlib_str(arg), Encoding::Utf8, Errors::Replace)?))
}

/// `lazymkv-dev compare-scans <A> <B>`. Py: compare_scans' `sys.exit(main(sys.argv[1], sys.argv[2]))`.
pub fn run(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    if args.first().is_some_and(|arg| arg == "-h" || arg == "--help") {
        let _ = writeln!(out, "usage: lazymkv-dev {USAGE}");
        return 0;
    }
    if args.len() < POSITIONALS.len() {
        let message = format!("the following arguments are required: {}", POSITIONALS[args.len()..].join(", "));
        return usage_error(err, NAME, USAGE, &message);
    }
    let mut sides = Vec::new();
    for arg in &args[..2] {
        match read_titles(ctx, arg) {
            Ok(titles) => sides.push(titles),
            Err(message) => {
                let _ = writeln!(err, "lazymkv-dev {NAME}: error: {message}");
                return 2;
            }
        }
    }
    let (lines, code) = report_lines(&sides[0], &sides[1]);
    for line in lines {
        let _ = writeln!(out, "{line}");
    }
    code
}

#[cfg(test)]
mod tests {
    use super::run;
    use crate::evidence::Ctx;
    use crate::testutil::{printed, Scratch as TempDir, ScratchExt};
    use std::path::{Path, PathBuf};

    /// The pinned John Wick UHD scan. Py: JOHN_WICK.
    fn john_wick() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../corpus/thediscdb/movie")
            .join("John Wick (2014)")
            .join("4k-ultra-hd--blu-ray")
            .join("disc01.txt")
    }

    /// The command over scans `a` and `b`: (exit code, stdout, stderr). Py: quiet(compare_scans.main, a, b).
    fn quiet(cwd: &Path, a: &Path, b: &Path) -> (i32, String, String) {
        let ctx = Ctx::for_test(cwd);
        let args = vec![a.to_string_lossy().into_owned(), b.to_string_lossy().into_owned()];
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run(&ctx, &args, &mut out, &mut err);
        (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
    }

    /// Py: the `one` template of test_a_title_listed_twice_on_one_side_is_a_difference.
    fn one(n: u32) -> String {
        format!("TINFO:{n},16,0,\"00001.mpls\"\nTINFO:{n},26,0,\"1\"\nTINFO:{n},9,0,\"1:00:00\"\n")
    }

    #[test]
    fn compare_scans_a_scan_equals_itself() {
        let (code, out, _) = quiet(Path::new("."), &john_wick(), &john_wick());
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("only in A: 0"), "{out}");
        assert!(out.contains("only in B: 0"), "{out}");
    }

    #[test]
    fn compare_scans_empty_scans_do_not_compare_equal() {
        let tmp = TempDir::new("empty");
        let (a, b) = (tmp.write_path("a.txt", ""), tmp.write_path("b.txt", "garbage\n"));
        let (code, out, _) = quiet(&tmp.0, &a, &b);
        assert_eq!(code, 1, "{out}");
        assert!(out.contains("EMPTY SCAN"), "{out}");
    }

    #[test]
    fn compare_scans_a_title_listed_twice_on_one_side_is_a_difference() {
        let tmp = TempDir::new("twice");
        let a = tmp.write_path("a.txt", &format!("{}{}", one(0), one(1)));
        let b = tmp.write_path("b.txt", &one(0));
        let (code, out, _) = quiet(&tmp.0, &a, &b);
        assert_eq!(code, 1, "{out}");
        assert!(out.contains("titles: A=2 B=1 common=1"), "{out}");
        assert!(out.contains("only in A: 1"), "{out}");
    }

    #[test]
    fn compare_scans_prints_the_pythons_lines_for_a_doubled_title() {
        // The doubled title's comments are ["", ""] against [""]: a comment difference too.
        let tmp = TempDir::new("doubled");
        let a = tmp.write_path("a.txt", &format!("{}{}", one(0), one(1)));
        let b = tmp.write_path("b.txt", &one(0));
        let (code, out, err) = quiet(&tmp.0, &a, &b);
        assert_eq!((code, err.as_str()), (1, ""));
        assert_eq!(
            out,
            printed(&[
                "titles: A=2 B=1 common=1",
                "only in A: 1",
                "  (\"00001.mpls\", \"1\", \"1:00:00\")",
                "only in B: 0",
                "comment differs: 1",
                "  (\"00001.mpls\", \"1\", \"1:00:00\")",
            ])
        );
    }

    #[test]
    fn compare_scans_prints_one_side_titles_as_json_tuples() {
        let tmp = TempDir::new("tuples");
        let a = tmp.write_path(
            "x.txt",
            "TINFO:0,26,0,\"1\"\nTINFO:0,9,0,\"0:30:00\"\nTINFO:0,49,0,\"one\"\n\
             TINFO:1,16,0,\"00002.mpls\"\nTINFO:1,9,0,\"0:10:00\"\nTINFO:1,49,0,\"two\"\n",
        );
        let b = tmp.write_path(
            "y.txt",
            "TINFO:0,16,0,\"00002.mpls\"\nTINFO:0,9,0,\"0:10:00\"\nTINFO:0,49,0,\"zwei\"\n\
             TINFO:1,16,0,\"00003.mpls\"\nTINFO:1,9,0,\"0:20:00\"\n",
        );
        let (code, out, _) = quiet(&tmp.0, &a, &b);
        assert_eq!(code, 1, "{out}");
        assert_eq!(
            out,
            printed(&[
                "titles: A=2 B=2 common=1",
                "only in A: 1",
                "  (null, \"1\", \"0:30:00\")",
                "only in B: 1",
                "  (\"00003.mpls\", null, \"0:20:00\")",
                "comment differs: 1",
                "  (\"00002.mpls\", null, \"0:10:00\")",
            ])
        );
    }

    #[test]
    fn compare_scans_keeps_the_pythons_order_of_reprs() {
        // Python sorted str(key): ("it's.mpls", None, None) before ('a.mpls', None, None), as '"' < "'". B's line
        // ends with a lone "\r", which Python's text-mode read made a line end.
        let tmp = TempDir::new("repr-order");
        let a = tmp.write_path(
            "a.txt",
            "TINFO:0,16,0,\"a.mpls\"\nTINFO:1,16,0,\"it's.mpls\"\nTINFO:2,16,0,\"b.mpls\"\nTINFO:2,9,0,\"0:01:00\"\n",
        );
        let b = tmp.write_path("b.txt", "TINFO:0,16,0,\"b.mpls\"\nTINFO:0,9,0,\"0:01:00\"\r");
        let (code, out, _) = quiet(&tmp.0, &a, &b);
        assert_eq!(code, 1, "{out}");
        assert_eq!(
            out,
            printed(&[
                "titles: A=3 B=1 common=1",
                "only in A: 2",
                "  (\"it's.mpls\", null, null)",
                "  (\"a.mpls\", null, null)",
                "only in B: 0",
                "comment differs: 0",
            ])
        );
    }

    #[test]
    fn compare_scans_reads_a_lone_cr_as_a_line_end() {
        let tmp = TempDir::new("cr");
        let a = tmp.write_path("c.txt", "TINFO:0,16,0,\"00001.mpls\"\rTINFO:0,9,0,\"1:00:00\"\r");
        let b = tmp.write_path("d.txt", "TINFO:0,16,0,\"00001.mpls\"\nTINFO:0,9,0,\"1:00:00\"\n");
        let (code, out, _) = quiet(&tmp.0, &a, &b);
        assert_eq!(code, 0, "{out}");
        assert_eq!(out, printed(&["titles: A=1 B=1 common=1", "only in A: 0", "only in B: 0", "comment differs: 0"]));
    }

    #[test]
    fn compare_scans_without_two_readable_scans_exits_2() {
        let ctx = Ctx::for_test(Path::new("."));
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(run(&ctx, &["one".to_string()], &mut out, &mut err), 2);
        assert!(out.is_empty());
        assert_eq!(
            String::from_utf8(err).unwrap(),
            printed(&[
                "usage: lazymkv-dev compare-scans <A> <B>",
                "lazymkv-dev compare-scans: error: the following arguments are required: B",
            ])
        );
        let tmp = TempDir::new("missing");
        let a = tmp.write_path("a.txt", "");
        let (code, out, err) = quiet(&tmp.0, &a, &tmp.0.join("missing.txt"));
        assert_eq!((code, out.as_str()), (2, ""));
        assert!(err.starts_with("lazymkv-dev compare-scans: error: "), "{err}");
    }

    #[test]
    fn compare_scans_names_a_missing_scan_as_python_does() {
        // FileNotFoundError names str(Path(arg)): "./nope//b.txt" is shown as "nope/b.txt".
        let tmp = TempDir::new("missing-text");
        let a = tmp.write_path("a.txt", "");
        let (code, out, err) = quiet(&tmp.0, &a, Path::new("./nope//b.txt"));
        assert_eq!((code, out.as_str()), (2, ""));
        assert_eq!(err, "lazymkv-dev compare-scans: error: [Errno 2] No such file or directory: 'nope/b.txt'\n");
    }
}
