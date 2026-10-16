//! `lazymkv-dev exp02 <dir>`: experiment 2, do the 3307/3308 title numbers of a rip call equal the index ripped?
//! Py: exp02_check, its lines verbatim. Exit 0 when every run says yes; 1 when a run says no or there is no run;
//! 2 where the Python crashed (no argument; the folder, its scan.txt or a run unreadable).

use crate::evidence::{read_scan_in, run_on_folder, Ctx};
use lazymkv_core::recording::{py_int, py_isdigit, Encoding, Errors, PathKey, Tree};
use lazymkv_core::robot::Scan;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;

/// The command's name as typed.
const NAME: &str = "exp02";
/// Its usage after "usage: lazymkv-dev ". The Python read sys.argv[1] and ignored any further word; so does this.
const USAGE: &str = "exp02 <dir>";
/// The runs below <dir>/exp02, as the Python's glob names them.
const RUN_PATTERN: &str = "title-[0-9]*.txt";

/// The facts for one rip call of a title that was stopped after the saving message. Py: check_run's dict.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunFacts {
    /// The scan's attribute 16 (source file) of the title; None when the scan does not list it.
    pub expected: Option<String>,
    /// Every file this run added under the title's number, sorted and joined with "/"; None when none.
    pub added: Option<String>,
    /// The run added the scan's source file, and no other file, under the title's number.
    pub matched: bool,
    /// The run printed the saving message: MSG:5014, or MSG:5015 when a profile is in use (MakeMKV 2.0.0).
    pub saw_5014: bool,
    /// The run has a saving message and a 3307/3308 line, and every 3307/3308 line comes before the first saving
    /// message.
    pub before: bool,
    /// How many MSG:3308 lines the run has.
    pub n3308: usize,
    /// How many MSG:3309 lines the run has.
    pub n3309: usize,
}

/// The facts for the run `log` of `title` against `scan`. The printed labels keep the name 5014 and are true for
/// either saving code. Nothing passes by default: a title the scan does not list, a run with no 3307/3308 line, and
/// a title number added for two different files are all a "NO". Py: exp02_check.check_run.
pub fn check_run(scan: &Scan, title: i64, log: &Scan) -> RunFacts {
    let codes: Vec<u32> = log.messages.iter().map(|message| message.code).collect();
    // title number -> every file added under that number in this run
    let mut added: BTreeMap<i64, BTreeSet<String>> = BTreeMap::new();
    for message in &log.messages {
        let params = &message.params;
        if (message.code == 3307 || message.code == 3308) && params.len() >= 2 {
            let last = params[params.len() - 1].as_str();
            if py_isdigit(last) {
                if let Some(number) = py_int(last) {
                    added.entry(number).or_default().insert(params[0].clone());
                }
            }
        }
    }
    let added_at = positions(&codes, &[3307, 3308]);
    let saving_at = positions(&codes, &[5014, 5015]);
    let expected = u32::try_from(title)
        .ok()
        .and_then(|number| scan.titles.get(&number))
        .and_then(|attrs| attrs.get(&16))
        .cloned();
    let saw_5014 = !saving_at.is_empty();
    let before = saw_5014 && !added_at.is_empty() && added_at.iter().all(|at| *at < saving_at[0]);
    let files = added.get(&title);
    let matched = match (&expected, files) {
        (Some(source), Some(files)) => files.len() == 1 && files.contains(source),
        _ => false,
    };
    // "/".join(sorted(...)) or None: an empty join is None too.
    let joined = files
        .map(|files| files.iter().map(String::as_str).collect::<Vec<_>>().join("/"))
        .filter(|text| !text.is_empty());
    RunFacts {
        expected,
        added: joined,
        matched,
        saw_5014,
        before,
        n3308: positions(&codes, &[3308]).len(),
        n3309: positions(&codes, &[3309]).len(),
    }
}

/// The places in `codes` that hold one of `wanted`, in order.
fn positions(codes: &[u32], wanted: &[u32]) -> Vec<usize> {
    codes.iter().enumerate().filter(|(_, code)| wanted.contains(*code)).map(|(at, _)| at).collect()
}

/// "yes" or "NO", as the run lines spell them.
fn yes_or_no(ok: bool) -> &'static str {
    if ok {
        "yes"
    } else {
        "NO"
    }
}

/// str() of an optional string: the string, or "None".
fn or_none(text: &Option<String>) -> &str {
    text.as_deref().unwrap_or("None")
}

/// The title number of a run file: the first run of digits in its stem (the name without its last suffix, as
/// pathlib's stem). Err when there is none or it overflows i64 (the glob guarantees one; Python's int has no limit).
fn run_number(name: &str) -> Result<i64, String> {
    let stem = match name.rfind('.') {
        Some(at) if at > 0 && at < name.len() - 1 => &name[..at],
        _ => name,
    };
    let Some(start) = stem.find(|c: char| c.is_ascii_digit()) else {
        return Err(format!("{name}: no title number in the name"));
    };
    let digits: String = stem[start..].chars().take_while(char::is_ascii_digit).collect();
    py_int(&digits).ok_or_else(|| format!("{name}: the title number {digits} is too large"))
}

/// Writes exp02_check.main's lines for the recording `rec` of `tree` as it goes, and returns the exit code (0 when
/// experiment 2 says yes, else 1); Err where the Python crashed, after the lines written so far. Py: exp02_check.main.
pub fn report(tree: &dyn Tree, rec: &PathKey, out: &mut dyn Write) -> Result<i32, String> {
    let scan = read_scan_in(tree, &rec.join("scan.txt"), Encoding::Utf8, Errors::Replace)?;
    let mut runs: Vec<(i64, PathKey)> = Vec::new();
    for path in tree.glob(&rec.join("exp02"), RUN_PATTERN) {
        runs.push((run_number(path.name())?, path));
    }
    // sorted(..., key=title number) is stable: two names with one number keep the glob's order.
    runs.sort_by_key(|(number, _)| *number);
    let mut all_ok = !runs.is_empty();
    for (title, path) in &runs {
        let log = read_scan_in(tree, path, Encoding::Utf8Sig, Errors::Replace)?;
        let r = check_run(&scan, *title, &log);
        all_ok = all_ok && r.saw_5014 && r.before && r.matched;
        let _ = writeln!(
            out,
            "title {:>3}: scan source={} rip #{}={} match={} 5014={} added-before-5014={} 3308-lines={} 3309-lines={}",
            title,
            or_none(&r.expected),
            title,
            or_none(&r.added),
            yes_or_no(r.matched),
            yes_or_no(r.saw_5014),
            yes_or_no(r.before),
            r.n3308,
            r.n3309
        );
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "EXPERIMENT 2: {} ({} titles)", if all_ok { "yes" } else { "no" }, runs.len());
    Ok(if all_ok { 0 } else { 1 })
}

/// `lazymkv-dev exp02 <dir>`. Py: exp02_check's `sys.exit(0 if main(sys.argv[1]) else 1)`.
pub fn run(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    run_on_folder(ctx, args, out, err, NAME, USAGE, report)
}

#[cfg(test)]
mod tests {
    use super::run;
    use crate::evidence::Ctx;
    use crate::testutil::{printed, Scratch as TempDir, ScratchExt};
    use std::path::Path;

    /// Py: SAVING.
    const SAVING: &str = "MSG:5014,0,2,\"Saving 1 titles into directory x\",\"Saving %1 titles into directory %2\",\"1\",\"x\"\n";
    /// Py: SCAN.
    const SCAN: &str = "TINFO:0,16,0,\"00001.mpls\"\nTINFO:1,16,0,\"00002.mpls\"\n";

    /// Py: ADDED.format(f=file, n=number).
    fn added(file: &str, number: u32) -> String {
        format!("MSG:3307,0,2,\"File {file} was added as title #{number}\",\"File %1 was added as title #%2\",\"{file}\",\"{number}\"\n")
    }

    /// Py: BOTH_ADDED.
    fn both_added() -> String {
        format!("{}{}{SAVING}", added("00001.mpls", 0), added("00002.mpls", 1))
    }

    /// The command over `dir`: (exit code, stdout, stderr). Py: quiet(exp02_check.main, dir).
    fn quiet(dir: &Path) -> (i32, String, String) {
        let ctx = Ctx::for_test(dir);
        let args = vec![dir.to_string_lossy().into_owned()];
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run(&ctx, &args, &mut out, &mut err);
        (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
    }

    #[test]
    fn exp02_yes_when_every_run_maps_its_title_to_the_scans_source() {
        let tmp = TempDir::new("yes");
        tmp.write("scan.txt", SCAN);
        tmp.write("exp02/title-0.txt", &both_added());
        tmp.write("exp02/title-1.txt", &both_added());
        let (code, out, _) = quiet(&tmp.0);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("EXPERIMENT 2: yes (2 titles)"), "{out}");
    }

    #[test]
    fn exp02_the_saving_message_of_a_profile_rip_counts() {
        // With --profile, MakeMKV 2.0.0 prints MSG:5015 where a plain rip prints MSG:5014.
        let tmp = TempDir::new("profile");
        tmp.write("scan.txt", SCAN);
        tmp.write("exp02/title-0.txt", &both_added().replace("MSG:5014,", "MSG:5015,"));
        let (code, out, _) = quiet(&tmp.0);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("5014=yes added-before-5014=yes"), "{out}");
    }

    #[test]
    fn exp02_no_when_the_scan_does_not_list_the_title() {
        let tmp = TempDir::new("unlisted");
        tmp.write("scan.txt", "");
        tmp.write("exp02/title-3.txt", SAVING);
        let (code, out, _) = quiet(&tmp.0);
        assert_eq!(code, 1, "{out}");
        assert!(out.contains("match=NO"), "{out}");
        assert!(out.contains("added-before-5014=NO"), "{out}");
        assert!(out.contains("EXPERIMENT 2: no (1 titles)"), "{out}");
    }

    #[test]
    fn exp02_no_when_the_rip_maps_the_title_to_another_file() {
        let tmp = TempDir::new("another");
        tmp.write("scan.txt", SCAN);
        tmp.write("exp02/title-0.txt", &format!("{}{SAVING}", added("00002.mpls", 0)));
        let (code, out, _) = quiet(&tmp.0);
        assert_eq!(code, 1, "{out}");
        assert!(out.contains("match=NO"), "{out}");
    }

    #[test]
    fn exp02_no_when_a_title_is_added_after_5014() {
        let tmp = TempDir::new("after");
        tmp.write("scan.txt", SCAN);
        tmp.write("exp02/title-0.txt", &format!("{SAVING}{}", added("00001.mpls", 0)));
        let (code, out, _) = quiet(&tmp.0);
        assert_eq!(code, 1, "{out}");
        assert!(out.contains("match=yes"), "{out}");
        assert!(out.contains("added-before-5014=NO"), "{out}");
    }

    #[test]
    fn exp02_no_when_a_title_number_is_added_for_two_files() {
        let tmp = TempDir::new("two-files");
        tmp.write("scan.txt", SCAN);
        tmp.write("exp02/title-0.txt", &format!("{}{}{SAVING}", added("00002.mpls", 0), added("00001.mpls", 0)));
        let (code, out, _) = quiet(&tmp.0);
        assert_eq!(code, 1, "{out}");
        assert!(out.contains("rip #0=00001.mpls/00002.mpls match=NO"), "{out}");
    }

    #[test]
    fn exp02_no_without_runs() {
        let tmp = TempDir::new("no-runs");
        tmp.write("scan.txt", SCAN);
        let (code, out, _) = quiet(&tmp.0);
        assert_eq!(code, 1, "{out}");
        assert!(out.contains("EXPERIMENT 2: no (0 titles)"), "{out}");
    }

    #[test]
    fn exp02_prints_the_pythons_lines() {
        let tmp = TempDir::new("lines");
        tmp.write("scan.txt", SCAN);
        tmp.write("exp02/title-0.txt", &both_added());
        tmp.write("exp02/title-1.txt", &both_added());
        let (code, out, err) = quiet(&tmp.0);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(
            out,
            printed(&[
                "title   0: scan source=00001.mpls rip #0=00001.mpls match=yes 5014=yes added-before-5014=yes 3308-lines=0 3309-lines=0",
                "title   1: scan source=00002.mpls rip #1=00002.mpls match=yes 5014=yes added-before-5014=yes 3308-lines=0 3309-lines=0",
                "",
                "EXPERIMENT 2: yes (2 titles)",
            ])
        );
    }

    #[test]
    fn exp02_takes_runs_in_title_order_and_only_numbered_names() {
        // title-x.txt does not match title-[0-9]*.txt; 2 sorts before 10 as a number.
        let tmp = TempDir::new("order");
        tmp.write("scan.txt", SCAN);
        for name in ["title-10.txt", "title-2.txt", "title-x.txt"] {
            tmp.write(&format!("exp02/{name}"), &both_added());
        }
        let (code, out, _) = quiet(&tmp.0);
        assert_eq!(code, 1, "{out}");
        assert_eq!(
            out,
            printed(&[
                "title   2: scan source=None rip #2=None match=NO 5014=yes added-before-5014=yes 3308-lines=0 3309-lines=0",
                "title  10: scan source=None rip #10=None match=NO 5014=yes added-before-5014=yes 3308-lines=0 3309-lines=0",
                "",
                "EXPERIMENT 2: no (2 titles)",
            ])
        );
    }

    #[test]
    fn exp02_reads_3308_and_3309_a_byte_order_mark_and_both_line_ends() {
        // The scan's lines end with a lone "\r", the run's with "\r\n" after a byte-order mark: Python's text-mode
        // read made plain lines of both.
        let tmp = TempDir::new("line-ends");
        tmp.write("scan.txt", "TINFO:0,16,0,\"00001.mpls\"\rTINFO:1,16,0,\"00002.mpls\"\r");
        let log = format!(
            "\u{feff}{}MSG:3309,0,1,\"x\",\"x\",\"y\"\n{SAVING}",
            added("00001.mpls", 0).replace("MSG:3307", "MSG:3308")
        );
        tmp.write("exp02/title-0.txt", &log.replace('\n', "\r\n"));
        let (code, out, _) = quiet(&tmp.0);
        assert_eq!(code, 0, "{out}");
        assert_eq!(
            out,
            printed(&[
                "title   0: scan source=00001.mpls rip #0=00001.mpls match=yes 5014=yes added-before-5014=yes 3308-lines=1 3309-lines=1",
                "",
                "EXPERIMENT 2: yes (1 titles)",
            ])
        );
    }

    #[test]
    fn exp02_without_an_argument_or_a_scan_exits_2() {
        let ctx = Ctx::for_test(Path::new("."));
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(run(&ctx, &[], &mut out, &mut err), 2);
        assert!(out.is_empty());
        assert_eq!(
            String::from_utf8(err).unwrap(),
            printed(&[
                "usage: lazymkv-dev exp02 <dir>",
                "lazymkv-dev exp02: error: the following arguments are required: dir",
            ])
        );
        let tmp = TempDir::new("no-scan");
        tmp.write("exp02/title-0.txt", &both_added());
        let (code, out, err) = quiet(&tmp.0);
        assert_eq!((code, out.as_str()), (2, ""));
        assert!(err.starts_with("lazymkv-dev exp02: error: [Errno 2] ") && err.contains("scan.txt"), "{err}");
    }
}
