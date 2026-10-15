//! `lazymkv-dev align [name=path ...] [--subs-dir DIR] [--clear-max-bytes N] [--tolerance-ms N]`: the command line of
//! the phase 0 Python `subtitle_alignment` (port spec 3.4). The files are read here (`name=path` inputs from the
//! file system, a subs folder through a `DiskTree`); the counts, the table and the pairs are
//! `lazymkv_core::subtitle::align`'s. Exit 0; 2 for argparse's errors (the usage line, then the message, on
//! stderr) and where the Python crashed (deliberate: its traceback exited 1), in which case the lines it printed
//! before the crash are on stdout and one error line is on stderr.

use std::io::Write;

use lazymkv_core::recording::files::Packet;
use lazymkv_core::recording::{decode_text, py_int, py_repr_str, Encoding, Errors, PathKey};
use lazymkv_core::subtitle::align::{
    align_named, align_subs_dir, index_streams, load_packets, no_index_error, no_stream_error, AlignRun,
};

use crate::evidence::{os_error_text, pathlib_str, Ctx, DiskTree};
use crate::{usage_error, Args};

/// The command's name as typed.
const NAME: &str = "align";
/// argparse's usage line of the Python (3.9.6, which wraps it at 80 columns), on one line, under the command's name.
const USAGE: &str =
    "align [-h] [--subs-dir SUBS_DIR] [--clear-max-bytes CLEAR_MAX_BYTES] [--tolerance-ms TOLERANCE_MS] [inputs ...]";
/// What -h prints after the usage line: argparse's help of the Python, verbatim (80 columns).
const HELP: &str = "
positional arguments:
  inputs                name=path to a packets JSON file

optional arguments:
  -h, --help            show this help message and exit
  --subs-dir SUBS_DIR   a subs folder written by probe-title.ps1 (index.json,
                        sub-<n>.packets.json)
  --clear-max-bytes CLEAR_MAX_BYTES
                        packets this small are ignored; with --subs-dir, on
                        image streams only
  --tolerance-ms TOLERANCE_MS
";
/// main's own ap.error when both or neither of the two input forms are given, verbatim.
const GIVE_EITHER: &str = "give either name=path inputs or --subs-dir";
/// argparse's type=int error, after "argument <flag>" and before the repr of the value.
const INVALID_INT: &str = ": invalid int value: ";
/// argparse's defaults.
const DEFAULT_CLEAR_MAX_BYTES: i64 = 64;
const DEFAULT_TOLERANCE_MS: i64 = 300;

/// `lazymkv-dev align`. Py: subtitle_alignment.main.
pub fn run(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        let _ = writeln!(out, "usage: lazymkv-dev {USAGE}");
        let _ = write!(out, "{HELP}");
        return 0;
    }
    let mut parsed = Args::new(args, &["--subs-dir", "--clear-max-bytes", "--tolerance-ms"]);
    let subs_dir = match parsed.flag("--subs-dir") {
        Ok(value) => value,
        Err(message) => return usage_error(err, NAME, USAGE, &message),
    };
    let clear_text = match parsed.flag("--clear-max-bytes") {
        Ok(value) => value,
        Err(message) => return usage_error(err, NAME, USAGE, &message),
    };
    let tolerance_text = match parsed.flag("--tolerance-ms") {
        Ok(value) => value,
        Err(message) => return usage_error(err, NAME, USAGE, &message),
    };
    let words = parsed.positionals();
    // argparse converts each value as it reads it: a bad int is reported before unrecognized arguments, and of two
    // bad ints the one first on the command line.
    let clear = int_flag("--clear-max-bytes", clear_text.as_deref(), DEFAULT_CLEAR_MAX_BYTES);
    let tolerance = int_flag("--tolerance-ms", tolerance_text.as_deref(), DEFAULT_TOLERANCE_MS);
    let (clear_max_bytes, tolerance_ms) = match (clear, tolerance) {
        (Ok(clear), Ok(tolerance)) => (clear, tolerance),
        (Err(clear), Err(tolerance)) => {
            let first = if first_word(args, "--t") < first_word(args, "--c") { tolerance } else { clear };
            return usage_error(err, NAME, USAGE, &first);
        }
        (Err(message), Ok(_)) | (Ok(_), Err(message)) => return usage_error(err, NAME, USAGE, &message),
    };
    let inputs = match words {
        Ok(words) => words,
        Err(message) => return usage_error(err, NAME, USAGE, &message),
    };
    // bool(args.inputs) == bool(args.subs_dir): an empty --subs-dir is falsy, like none.
    match (inputs.is_empty(), subs_dir.filter(|dir| !dir.is_empty())) {
        (false, None) => named(ctx, &inputs, clear_max_bytes, tolerance_ms, out, err),
        (true, Some(dir)) => subs(ctx, &dir, clear_max_bytes, tolerance_ms, out, err),
        _ => usage_error(err, NAME, USAGE, GIVE_EITHER),
    }
}

/// argparse's `type=int`: Python's int() of the value, the default when the flag is absent.
fn int_flag(flag: &str, value: Option<&str>, default: i64) -> Result<i64, String> {
    match value {
        None => Ok(default),
        Some(text) => py_int(text).ok_or_else(|| format!("argument {flag}{INVALID_INT}{}", py_repr_str(text))),
    }
}

/// Where the first word starting with `prefix` is (args.len() when none). With the declared flags, "--c" begins
/// only --clear-max-bytes or a prefix of it, and "--t" only --tolerance-ms.
fn first_word(args: &[String], prefix: &str) -> usize {
    args.iter().position(|arg| arg.starts_with(prefix)).unwrap_or(args.len())
}

/// The `name=path` form. Py: main's inputs branch, which reads one file at a time.
fn named(
    ctx: &Ctx,
    inputs: &[String],
    clear_max_bytes: i64,
    tolerance_ms: i64,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let mut loaded: Vec<(String, Vec<Packet>)> = Vec::new();
    for item in inputs {
        // item.partition("="): the name before the first "=", the path after it ("" when there is no "=").
        let (name, path) = item.split_once('=').unwrap_or((item.as_str(), ""));
        match read_packets(ctx, path) {
            Ok(packets) => loaded.push((name.to_string(), packets)),
            Err(message) => {
                // The Python printed one line per input before this one, then crashed; an input before this one
                // whose sizes or times did not convert crashed it earlier, before this file was opened.
                let before = align_named(&loaded, clear_max_bytes, tolerance_ms);
                if before.error.is_some() {
                    return finish(before, out, err);
                }
                print_lines(out, &before.lines[..loaded.len()]);
                return crash(err, &message);
            }
        }
    }
    finish(align_named(&loaded, clear_max_bytes, tolerance_ms), out, err)
}

/// The `--subs-dir DIR` form. Py: main's subs-dir branch, whose two ap.error sentences name DIR as typed.
fn subs(
    ctx: &Ctx,
    given: &str,
    clear_max_bytes: i64,
    tolerance_ms: i64,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let folder = ctx.abs(given);
    // (Path(DIR) / "index.json").exists(): a file or a folder of that name.
    if !folder.join("index.json").exists() {
        return usage_error(err, NAME, USAGE, &no_index_error(given));
    }
    // Paths inside the folder print as str(Path(DIR) / name) did.
    let given_shown = pathlib_str(given);
    let tree = match DiskTree::open(&folder, &given_shown) {
        Ok(tree) => tree,
        Err(e) => return crash(err, &os_error_text(&e, &given_shown)),
    };
    let root = PathKey::root();
    match index_streams(&tree, &root) {
        Err(message) => return crash(err, &message),
        Ok(streams) if streams.is_empty() => return usage_error(err, NAME, USAGE, &no_stream_error(given)),
        Ok(_) => {}
    }
    finish(align_subs_dir(&tree, &root, clear_max_bytes, tolerance_ms), out, err)
}

/// load_packets(path): `Path(path).read_text(encoding="utf-8-sig")` (strict) and the packets in it.
fn read_packets(ctx: &Ctx, path: &str) -> Result<Vec<Packet>, String> {
    let path_shown = pathlib_str(path);
    let bytes = std::fs::read(ctx.abs(&path_shown)).map_err(|e| os_error_text(&e, &path_shown))?;
    let text = decode_text(&bytes, Encoding::Utf8Sig, Errors::Strict)?;
    load_packets(&text, &path_shown)
}

/// Prints a run: its lines on stdout, then its error, if any, as a crash line (the argparse errors were checked
/// before the run); returns the exit code.
fn finish(run: AlignRun, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    print_lines(out, &run.lines);
    match &run.error {
        Some(message) => crash(err, message),
        None => run.exit_code,
    }
}

fn print_lines(out: &mut dyn Write, lines: &[String]) {
    for line in lines {
        let _ = writeln!(out, "{line}");
    }
}

/// One error line where the Python crashed with a traceback; exit 2 (deliberate, spec 3.4: the Python exited 1).
fn crash(err: &mut dyn Write, message: &str) -> i32 {
    let _ = writeln!(err, "lazymkv-dev {NAME}: error: {message}");
    2
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use lazymkv_core::recording::PathKey;
    use lazymkv_core::subtitle::align::align_subs_dir;
    use serde_json::{json, Value};

    use super::*;
    use crate::evidence::{repo_root, skip, DiskTree, EVIDENCE_SHOWN};
    use crate::pyjson::{dumps, Style};
    use crate::testutil::{call, evidence_inputs, printed, sha1_hex, Scratch as TempDir, ScratchExt};

    /// The byte-order mark probe-title.ps1 writes before index.json.
    const BOM: &str = "\u{feff}";

    /// Two made-up packets files: one.json (sizes on both sides of 64; written with a byte-order mark) and the
    /// file the input `bee=dir/x=y.json` names (a path holding "=", which str.partition keeps).
    const ONE: &str = r#"{"packets": [{"pts_time": "1.000000", "size": "900"}, {"pts_time": "2.000000", "size": "64"}, {"pts_time": "3.000000", "size": "65"}, {"pts_time": "4.000000", "size": "900"}]}"#;
    const TWO: &str = r#"{"packets": [{"pts_time": "1.200000", "size": "900"}, {"pts_time": "3.400000", "size": "900"}, {"pts_time": "9.000000", "size": "900"}]}"#;

    /// What the Python printed (exit 0) for `a=one.json bee=dir/x=y.json`.
    const OUTPUT_D: [&str; 13] = [
        "a: packets=4 sized<=32/64/128/256: [0, 1, 2, 2]",
        "bee: packets=3 sized<=32/64/128/256: [0, 0, 0, 0]",
        "",
        "a: events=3",
        "bee: events=3",
        "",
        "aligned(row, column); packets of 64 bytes or less are ignored; tolerance 300 ms:",
        "               a     bee",
        "a           1.00    0.33",
        "bee         0.33    1.00",
        "",
        "pairs, with the larger of aligned(A, B) and aligned(B, A):",
        "a       bee     0.33",
    ];

    /// What it printed (exit 0) for `a=one.json bee=dir/x=y.json --clear=65 --tol 100` (argparse's abbreviations
    /// and its --flag=value form).
    const OUTPUT_D2: [&str; 13] = [
        "a: packets=4 sized<=32/64/128/256: [0, 1, 2, 2]",
        "bee: packets=3 sized<=32/64/128/256: [0, 0, 0, 0]",
        "",
        "a: events=2",
        "bee: events=3",
        "",
        "aligned(row, column); packets of 65 bytes or less are ignored; tolerance 100 ms:",
        "               a     bee",
        "a           1.00    0.00",
        "bee         0.00    1.00",
        "",
        "pairs, with the larger of aligned(A, B) and aligned(B, A):",
        "a       bee     0.00",
    ];

    /// A subs folder that lists two English streams and has only the first one's packets file (made up).
    const CRASH_INDEX: &str = r#"[{"n": 0, "language": "eng", "codec": "VobSub", "track_name": null, "forced": false}, {"n": 1, "language": "eng", "codec": "SubRip/SRT", "track_name": "CC", "forced": false}]"#;
    const CRASH_SUB_0: &str = r#"{"packets": [{"pts_time": "1.000", "size": "5000"}, {"pts_time": "3.000", "size": "30"}]}"#;
    /// The line the Python printed for `--subs-dir ./crash/` before its FileNotFoundError.
    const CRASH_LINE: &str =
        "s0-eng-image: codec=VobSub track_name=None packets=2 sized<=32/64/128/256: [1, 1, 1, 1]";

    /// An index.json listing only a French stream (made up).
    const FRENCH_INDEX: &str =
        r#"[{"n": 0, "language": "fre", "codec": "VobSub", "track_name": null, "forced": false}]"#;

    /// What the Python printed for one subs folder of the evidence clone.
    struct Recorded {
        slug: &'static str,
        label: &'static str,
        /// SHA-1 of index.json, then every `sub-<n>.packets.json` of the folder in name order, concatenated.
        inputs: &'static str,
        /// SHA-1 of the Python's stdout (exit 0, nothing on stderr).
        stdout: &'static str,
        /// Its `<stream>: events=<n>` lines.
        events: &'static [&'static str],
    }

    /// Every subs folder of the evidence clone at its commit 41c5c51.
    const EVIDENCE: [Recorded; 7] = [
        Recorded {
            slug: "pans-labyrinth-2006-uhd",
            label: "all-tracks",
            inputs: "507ce3a7330c7b7e083abe6598c8ae59c5ed8498",
            stdout: "7af63f12f4c96ce26eb949528e7a84a92c1a2d47",
            events: &["s0-eng-image: events=887"],
        },
        Recorded {
            slug: "pans-labyrinth-2006-uhd",
            label: "eng-only",
            inputs: "0205f792c464e6556264dc284616c903f5d7c8fc",
            stdout: "7af63f12f4c96ce26eb949528e7a84a92c1a2d47",
            events: &["s0-eng-image: events=887"],
        },
        Recorded {
            slug: "short-circuit-2-1988-dvd",
            label: "all-tracks-2",
            inputs: "ae3380d877ba49f9caa699584311b9d884518579",
            stdout: "c1dd2a1b8ac869089f204e33279b490485c91492",
            events: &["s0-eng-text: events=1671", "s1-eng-image: events=1742"],
        },
        Recorded {
            slug: "short-circuit-2-1988-dvd",
            label: "eng-only",
            inputs: "862f404d9a99877e8eab951eb714e2a7dc68b9fb",
            stdout: "c1dd2a1b8ac869089f204e33279b490485c91492",
            events: &["s0-eng-text: events=1671", "s1-eng-image: events=1742"],
        },
        Recorded {
            slug: "the-office-2005-s3d3-bluray",
            label: "e16-stereo",
            inputs: "995c9e24b9c250ac8e7d99522af8f63ca58b0a76",
            stdout: "934942d7d3639a06ccf9c0a53a6fd42e99cfb406",
            events: &["s0-eng-image: events=345"],
        },
        Recorded {
            slug: "the-office-2005-s3d3-bluray",
            label: "rehearsal",
            inputs: "c27dbe6fb2147590c05c62206b90700944d3fc2b",
            stdout: "5521e14fe4844c36d7f5dd31c4007ddd80e27ea9",
            events: &["s0-eng-image: events=363"],
        },
        Recorded {
            slug: "the-office-2005-s3d3-bluray",
            label: "t3-all-tracks",
            inputs: "5886e52d1111b1e2a6c653c3f5d4b5ecb6dea473",
            stdout: "5a88f6e98e5bc74a0ff7b6dd7658b4ca542fe636",
            events: &["s0-eng-image: events=710"],
        },
    ];

    /// The command run with `cwd` as the current directory: (exit code, stdout, stderr).
    fn run_in(cwd: &Path, args: &[&str]) -> (i32, String, String) {
        call(run, &Ctx::for_test(cwd), args)
    }

    /// index.json as probe-title.ps1 writes it (UTF-8 with a byte-order mark), and one packets file per stream.
    /// Py: SubsDir.write_subs, whose json.dumps is pyjson::dumps in Python's default style.
    fn write_subs(dir: &TempDir, index: &Value, packets: &[(i64, Vec<Value>)]) {
        dir.write("index.json", format!("{BOM}{}", dumps(index, Style::PY_DEFAULT)).as_bytes());
        for (n, items) in packets {
            dir.write(&format!("sub-{n}.packets.json"), &dumps(&json!({ "packets": items }), Style::PY_DEFAULT));
        }
    }

    // Ported from test_analysis, class SubsDir (the two tests that run main()).

    #[test]
    fn text_streams_count_every_packet_and_image_streams_drop_clear_packets() {
        let dir = TempDir::new("text-and-image");
        let starts: Vec<i64> = (0..20).map(|i| 10 + 4 * i).collect();
        let mut image = Vec::new();
        for s in &starts {
            // f"{s:.3f}" of whole seconds.
            image.push(json!({"pts_time": format!("{s}.000"), "size": "5000"}));
            image.push(json!({"pts_time": format!("{}.000", s + 2), "size": "30"}));
        }
        let text: Vec<Value> = starts
            .iter()
            .enumerate()
            .map(|(i, s)| json!({"pts_time": format!("{s}.000"), "size": (20 + (i % 3) * 20).to_string()}))
            .collect();
        write_subs(
            &dir,
            &json!([
                {"n": 0, "language": "eng", "codec": "VobSub", "track_name": null, "forced": false},
                {"n": 1, "language": "eng", "codec": "SubRip/SRT", "track_name": "CC", "forced": false}
            ]),
            &[(0, image), (1, text)],
        );
        let (code, out, err) = run_in(&dir.0, &["--subs-dir", dir.0.to_str().unwrap(), "--clear-max-bytes", "64"]);
        assert!(out.contains("s0-eng-image: events=20"), "{out}");
        assert!(out.contains("s1-eng-text: events=20"), "{out}");
        let pairs: Vec<Vec<&str>> = out
            .lines()
            .filter(|line| line.starts_with("s0-eng-image") && line.contains("s1-eng-text"))
            .map(|line| line.split_whitespace().collect())
            .collect();
        assert_eq!(pairs, vec![vec!["s0-eng-image", "s1-eng-text", "1.00"]]);
        assert_eq!((code, err.as_str()), (0, ""));
    }

    #[test]
    fn inputs_and_a_subs_dir_are_not_mixed() {
        let dir = TempDir::new("not-mixed");
        let given = dir.0.to_str().unwrap();
        let cases: [&[&str]; 2] = [&[], &["a=x.json", "--subs-dir", given]];
        for args in cases {
            let (code, out, err) = run_in(&dir.0, args);
            assert_eq!((code, out.as_str()), (2, ""), "{args:?}");
            assert!(err.contains("give either name=path inputs or --subs-dir"), "{args:?}: {err}");
        }
    }

    // New in the port.

    #[test]
    fn name_path_inputs_print_what_the_python_printed() {
        let dir = TempDir::new("named");
        dir.write("one.json", format!("{BOM}{ONE}").as_bytes());
        dir.write("dir/x=y.json", TWO);
        let (code, out, err) = run_in(&dir.0, &["a=one.json", "bee=dir/x=y.json"]);
        assert_eq!(out, printed(&OUTPUT_D));
        assert_eq!((code, err.as_str()), (0, ""));
        let (code, out, err) = run_in(&dir.0, &["a=one.json", "bee=dir/x=y.json", "--clear=65", "--tol", "100"]);
        assert_eq!(out, printed(&OUTPUT_D2));
        assert_eq!((code, err.as_str()), (0, ""));
    }

    #[test]
    fn argument_errors_exit_2_with_argparses_messages() {
        let dir = TempDir::new("arguments");
        dir.write("noeng/index.json", format!("{BOM}{FRENCH_INDEX}").as_bytes());
        let cases: [(&[&str], &str); 9] = [
            (&["--clear-max-bytes", "x", "a=b"], "argument --clear-max-bytes: invalid int value: 'x'"),
            (&["--tolerance-ms", "1.5", "a=b"], "argument --tolerance-ms: invalid int value: '1.5'"),
            // Of two bad values argparse reports the first on the command line.
            (&["--tolerance-ms", "y", "--clear-max-bytes", "x", "a=b"], "argument --tolerance-ms: invalid int value: 'y'"),
            (&["--clear-max-bytes", "x", "--tolerance-ms", "y", "a=b"], "argument --clear-max-bytes: invalid int value: 'x'"),
            (&["--bogus", "a=b"], "unrecognized arguments: --bogus"),
            (&["a=b", "--subs-dir"], "argument --subs-dir: expected one argument"),
            // An empty --subs-dir is falsy, like none.
            (&["--subs-dir", ""], "give either name=path inputs or --subs-dir"),
            // The folder as typed, so its trailing "/" doubles.
            (
                &["--subs-dir", "nothere/"],
                "no nothere//index.json: the title has no subtitle track, or the recording was not pulled",
            ),
            (&["--subs-dir", "noeng"], "noeng/index.json lists no English or no-language subtitle stream"),
        ];
        for (args, message) in cases {
            let (code, out, err) = run_in(&dir.0, args);
            assert_eq!(code, 2, "{args:?}");
            assert_eq!(out, "", "{args:?}");
            assert!(err.starts_with("usage: lazymkv-dev align "), "{args:?}: {err}");
            assert!(err.contains(&format!("lazymkv-dev align: error: {message}")), "{args:?}: {err}");
        }
    }

    #[test]
    fn unreadable_inputs_exit_2_after_the_lines_printed_before() {
        // The Python crashed with a traceback (exit 1) on each of these after printing the lines shown; exit 2 is the
        // deliberate change (spec 3.4). The message is the last traceback line without the exception's name.
        let dir = TempDir::new("unreadable");
        dir.write("ok.json", r#"{"packets": [{"pts_time": "1.0", "size": 900}]}"#);
        dir.write("badsize.json", r#"{"packets": [{"pts_time": "1.000000", "size": "abc"}]}"#);
        dir.write("badpts.json", r#"{"packets": [{"pts_time": "x", "size": "900"}]}"#);
        dir.write("list.json", r#"[{"pts_time": "1.0", "size": "900"}]"#);
        dir.write("crash/index.json", format!("{BOM}{CRASH_INDEX}").as_bytes());
        dir.write("crash/sub-0.packets.json", CRASH_SUB_0);
        let a_line = "a: packets=1 sized<=32/64/128/256: [0, 0, 0, 0]\n".to_string();
        let cases: [(&[&str], String, &str); 6] = [
            (&["a=ok.json", "b=nothere.json"], a_line.clone(), "[Errno 2] No such file or directory: 'nothere.json'"),
            (&["a=ok.json", "b=./x//nothere.json"], a_line.clone(), "[Errno 2] No such file or directory: 'x/nothere.json'"),
            (&["a=ok.json", "b=badsize.json"], a_line.clone(), "invalid literal for int() with base 10: 'abc'"),
            (&["a=badpts.json"], a_line.clone(), "could not convert string to float: 'x'"),
            (&["a=list.json"], String::new(), "'list' object has no attribute 'get'"),
            (
                &["--subs-dir", "./crash/"],
                format!("{CRASH_LINE}\n"),
                "[Errno 2] No such file or directory: 'crash/sub-1.packets.json'",
            ),
        ];
        for (args, stdout, message) in cases {
            let (code, out, err) = run_in(&dir.0, args);
            assert_eq!(code, 2, "{args:?}");
            assert_eq!(out, stdout, "{args:?}");
            assert_eq!(err, format!("lazymkv-dev align: error: {message}\n"), "{args:?}");
        }
        // No "=": the path is "", and Path("") is ".", a folder (IsADirectoryError on macOS and Linux).
        let (code, out, err) = run_in(&dir.0, &["a.json"]);
        assert_eq!((code, out.as_str()), (2, ""));
        assert!(err.starts_with("lazymkv-dev align: error: "), "{err}");
        if cfg!(unix) {
            assert_eq!(err, "lazymkv-dev align: error: [Errno 21] Is a directory: '.'\n");
        }
    }

    #[test]
    fn help_prints_argparses_help() {
        let dir = TempDir::new("help");
        for flag in ["-h", "--help"] {
            let (code, out, err) = run_in(&dir.0, &["a=b", flag]);
            assert_eq!(out, format!("usage: lazymkv-dev {USAGE}\n{HELP}"));
            assert_eq!((code, err.as_str()), (0, ""));
        }
        assert!(HELP.ends_with("  --tolerance-ms TOLERANCE_MS\n"));
    }

    #[test]
    fn the_command_is_registered() {
        let row = crate::COMMANDS.iter().find(|(name, _, _)| *name == "align");
        assert_eq!(
            row.map(|(_, _, synopsis)| *synopsis),
            Some("[name=path ...] [--subs-dir DIR] [--clear-max-bytes N] [--tolerance-ms N]")
        );
    }

    #[test]
    fn every_evidence_subs_folder_gives_the_pythons_output() {
        let Some(root) = repo_root(Path::new(env!("CARGO_MANIFEST_DIR"))) else {
            skip("no repository root (a Cargo.toml with [workspace]) above crates/dev");
            return;
        };
        let evidence = root.join("corpus").join("owner");
        if !evidence.is_dir() {
            skip("no evidence clone at corpus/owner");
            return;
        }
        let tree = DiskTree::open(&evidence, EVIDENCE_SHOWN).expect("the evidence clone opens as a tree");
        for r in &EVIDENCE {
            let subs = format!("corpus/owner/{}/{}/subs", r.slug, r.label);
            let folder = root.join(&subs);
            if !folder.join("index.json").is_file() {
                skip(&format!("{subs}/index.json is not in the evidence clone"));
                continue;
            }
            if sha1_hex(&evidence_inputs(&folder)) != r.inputs {
                skip(&format!("{subs}: its inputs changed since the Python's output was recorded"));
                continue;
            }
            let (code, out, err) = run_in(&root, &["--subs-dir", subs.as_str(), "--clear-max-bytes", "64"]);
            let context = format!("{subs}: recorded from the Python at evidence commit 41c5c51\n{out}{err}");
            let events: Vec<&str> = out.lines().filter(|line| line.contains(": events=")).collect();
            assert_eq!(events, r.events, "{context}");
            assert_eq!(sha1_hex(out.as_bytes()), r.stdout, "{context}");
            assert_eq!((code, err.as_str()), (0, ""), "{context}");
            // Task 14's golden path: align_subs_dir over the evidence tree, 64 bytes and 300 ms, gives the same lines.
            let run = align_subs_dir(&tree, &PathKey::new(&format!("{}/{}/subs", r.slug, r.label)), 64, 300);
            let lines: String = run.lines.iter().map(|line| format!("{line}\n")).collect();
            assert_eq!(lines, out, "{context}");
            assert_eq!((run.json.exit_code, run.json.streams.len()), (0, r.events.len()), "{context}");
        }
    }
}
