//! `lazymkv-dev reconcile <scan> <title> <mkvmerge_json> [--selection all|eng] [--rip-log FILE]`:
//! `scripts/phase0/compare_tracks.py`'s command line (port spec 3.4). The files are read here; the comparison and
//! the report lines are `lazymkv_core::reconcile`'s. Exit 0 when every verdict is yes, 1 when one is no; 2 for an
//! argument error (argparse's) and where the Python crashed (deliberate: its traceback exited 1), with one line on
//! stderr and nothing on stdout.

use std::io::Write;
use std::path::Path;

use lazymkv_core::recording::files::{MkvmergeIdent, Selection};
use lazymkv_core::recording::{
    pathlib_str, pathlib_str_beside, py_int, py_repr_str, text_mode, Encoding, Errors,
};
use lazymkv_core::reconcile::{reconcile, report_lines, RipLog, RIP_LOG_NAMES};

use crate::evidence::{read_file_text, read_scan_file, Ctx};
use crate::{usage_error, Args};

/// The command's name as typed.
const NAME: &str = "reconcile";
/// argparse's usage line of compare_tracks.py, under the command's name.
const USAGE: &str = "reconcile [-h] [--selection {all,eng}] [--rip-log RIP_LOG] scan title mkvmerge_json";
/// What -h prints after the usage line: argparse's help of compare_tracks.py.
const HELP: &str = "
positional arguments:
  scan
  title
  mkvmerge_json

optional arguments:
  -h, --help            show this help message and exit
  --selection {all,eng}
  --rip-log RIP_LOG     MakeMKV's log of the rip; default: rip.txt or extract.txt beside the mkvmerge json
";
/// The positional arguments in order, as argparse names them.
const POSITIONALS: [&str; 3] = ["scan", "title", "mkvmerge_json"];
// argparse's error sentences, verbatim.
const REQUIRED: &str = "the following arguments are required: ";
const INVALID_INT: &str = "argument title: invalid int value: ";
const INVALID_CHOICE: &str = "argument --selection: invalid choice: ";
const CHOICES: &str = " (choose from 'all', 'eng')";
const UNRECOGNIZED: &str = "unrecognized arguments: ";
/// A title no scan lists (a negative one): core::reconcile's sentence for the Python's KeyError.
const NO_SUCH_TITLE: &str = "the scan lists no title ";

/// `lazymkv-dev reconcile`. Py: compare_tracks.main.
pub fn run(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        let _ = writeln!(out, "usage: lazymkv-dev {USAGE}");
        let _ = write!(out, "{HELP}");
        return 0;
    }
    let mut parsed = Args::new(args, &["--selection", "--rip-log"]);
    let selection = match parsed.flag("--selection") {
        Ok(value) => value,
        Err(message) => return usage_error(err, NAME, USAGE, &message),
    };
    let rip_log = match parsed.flag("--rip-log") {
        Ok(value) => value,
        Err(message) => return usage_error(err, NAME, USAGE, &message),
    };
    let words = match parsed.positionals() {
        Ok(words) => words,
        Err(message) => return usage_error(err, NAME, USAGE, &message),
    };
    let selection = match selection.as_deref().map(|text| (text, Selection::parse(text))) {
        None => Selection::All,
        Some((_, Some(selection))) => selection,
        Some((text, None)) => {
            let message = format!("{INVALID_CHOICE}{}{CHOICES}", py_repr_str(text));
            return usage_error(err, NAME, USAGE, &message);
        }
    };
    // argparse converts the title as it takes it, so a bad title is reported before a missing argument.
    let title = match words.get(1).map(|word| (word, py_int(word))) {
        Some((word, None)) => {
            let message = format!("{INVALID_INT}{}", py_repr_str(word));
            return usage_error(err, NAME, USAGE, &message);
        }
        Some((_, Some(title))) => title,
        // Fewer than two words: reported as missing just below.
        None => 0,
    };
    if words.len() < POSITIONALS.len() {
        let message = format!("{REQUIRED}{}", POSITIONALS[words.len()..].join(", "));
        return usage_error(err, NAME, USAGE, &message);
    }
    if words.len() > POSITIONALS.len() {
        let message = format!("{UNRECOGNIZED}{}", words[POSITIONALS.len()..].join(" "));
        return usage_error(err, NAME, USAGE, &message);
    }
    match compare(ctx, &words[0], title, &words[2], rip_log.as_deref(), selection) {
        Ok((lines, code)) => {
            for line in lines {
                let _ = writeln!(out, "{line}");
            }
            code
        }
        Err(message) => {
            let _ = writeln!(err, "lazymkv-dev {NAME}: error: {message}");
            2
        }
    }
}

/// Reads the scan, mkvmerge.json and the rip log as compare_tracks.main does, and returns the report's lines and its
/// exit code; Err where the Python crashed.
fn compare(
    ctx: &Ctx,
    scan_arg: &str,
    title: i64,
    json_arg: &str,
    rip_log: Option<&str>,
    selection: Selection,
) -> Result<(Vec<String>, i32), String> {
    let scan_shown = shown(scan_arg);
    let scan = read_scan_file(&ctx.abs(&scan_shown), &scan_shown, Encoding::Utf8, Errors::Replace)?;
    let json_shown = shown(json_arg);
    let json_path = ctx.abs(&json_shown);
    let json_text = read_file_text(&json_path, &json_shown, Encoding::Utf8Sig, Errors::Strict)?;
    let ident: MkvmergeIdent = serde_json::from_str(&json_text).map_err(|e| format!("{json_shown}: {e}"))?;
    // `Path(args.rip_log) if args.rip_log else next(...)`: an empty --rip-log is falsy, so the log is looked for
    // beside mkvmerge.json, rip.txt first; Path.exists is true for a file or a folder.
    let rip_found = match rip_log.filter(|given| !given.is_empty()) {
        Some(given) => {
            let given_shown = shown(given);
            Some((ctx.abs(&given_shown), given_shown))
        }
        None => {
            let folder = json_path.parent().map_or_else(|| ctx.abs("."), Path::to_path_buf);
            RIP_LOG_NAMES
                .iter()
                .map(|name| (folder.join(name), shown_beside(json_arg, name)))
                .find(|(path, _)| path.exists())
        }
    };
    let rip_text = match &rip_found {
        Some((path, path_shown)) => {
            Some(text_mode(&read_file_text(path, path_shown, Encoding::Utf8, Errors::Replace)?))
        }
        None => None,
    };
    // A negative title (int() accepts one) is in no scan: the Python's KeyError.
    let title = u32::try_from(title).map_err(|_| format!("{NO_SUCH_TITLE}{title}"))?;
    let rip = rip_found.zip(rip_text.as_deref()).map(|((_, path_shown), text)| RipLog { shown: path_shown, text });
    let report = reconcile(&scan, title, &ident, selection, rip)?;
    Ok((report_lines(&report), report.exit_code))
}

/// str(Path(text)): how compare_tracks printed (and opened) a path it was given.
fn shown(text: &str) -> String {
    pathlib_str(text)
}

/// str(Path(text).parent / name): `name` beside the file `text` names.
fn shown_beside(text: &str, name: &str) -> String {
    pathlib_str_beside(text, name)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::evidence::{repo_root, skip};
    use crate::testutil::{printed, sha1_hex};

    /// One title: video, then two audio tracks of the same language with 8 and 2 channels. Py: TRACK_SCAN.
    const TRACK_SCAN: &str = r#"TINFO:0,8,0,"2"
TINFO:0,9,0,"0:10:00"
SINFO:0,0,1,6201,"Video"
SINFO:0,0,6,0,"MpegH"
SINFO:0,1,1,6202,"Audio"
SINFO:0,1,3,0,"{lang}"
SINFO:0,1,6,0,"TrueHD"
SINFO:0,1,14,0,"8"
SINFO:0,2,1,6202,"Audio"
SINFO:0,2,3,0,"{lang}"
SINFO:0,2,6,0,"DD"
SINFO:0,2,14,0,"2"
"#;

    /// json.dumps(ident([("video", "und", None), ("audio", "eng", 8), ("audio", "eng", 2)])) of test_decisions.py.
    const TRACK_IDENT: &str = r#"{"tracks": [{"id": 0, "type": "video", "codec": "x", "properties": {"language": "und"}}, {"id": 1, "type": "audio", "codec": "x", "properties": {"language": "eng", "audio_channels": 8}}, {"id": 2, "type": "audio", "codec": "x", "properties": {"language": "eng", "audio_channels": 2}}], "chapters": [{"num_entries": 2}], "container": {"properties": {"duration": 600000000000}}}"#;

    /// Title 0 of a synthetic UHD-like scan (video with attribute 28 and no attribute 3, English and French audio,
    /// two English subtitle tracks, the first forced-only), and a title 1 that must be ignored. New in the port.
    const SCAN_A: &str = r#"TINFO:0,8,0,"2"
TINFO:0,9,0,"0:10:00"
SINFO:0,0,1,6201,"Video"
SINFO:0,0,6,0,"MpegH"
SINFO:0,0,28,0,"eng"
SINFO:0,0,41,0,"V_MPEGH/ISO/HEVC"
SINFO:0,1,1,6202,"Audio"
SINFO:0,1,3,0,"eng"
SINFO:0,1,6,0,"TrueHD"
SINFO:0,1,14,0,"8"
SINFO:0,1,41,0,"A_TRUEHD"
SINFO:0,2,1,6202,"Audio"
SINFO:0,2,3,0,"fre"
SINFO:0,2,6,0,"DD"
SINFO:0,2,14,0,"2"
SINFO:0,2,41,0,"A_AC3"
SINFO:0,3,1,6203,"Subtitles"
SINFO:0,3,3,0,"eng"
SINFO:0,3,6,0,"PGS"
SINFO:0,3,22,0,"6144"
SINFO:0,3,41,0,"S_HDMV/PGS"
SINFO:0,4,1,6203,"Subtitles"
SINFO:0,4,3,0,"eng"
SINFO:0,4,6,0,"PGS"
SINFO:0,4,41,0,"S_HDMV/PGS"
TINFO:1,9,0,"0:01:00"
SINFO:1,0,1,6201,"Video"
"#;

    /// The rip's MakeMKV log for SCAN_A: an audio gap (not a removal), output tracks 3 and 7 removed, the saving
    /// message. Synthetic.
    const RIP_A: &str = r#"MSG:4047,16,3,"AV sync issue in stream 1 at 0:01:00 : audio gap","AV sync issue in stream %1 at %2 : %3","1","0:01:00","audio gap"
MSG:4047,16,3,"AV sync issue in stream 3 at 0:00:00 : Forced subtitles track #3 turned out to be empty and was removed from output file","AV sync issue in stream %1 at %2 : %3","3","0:00:00","Forced subtitles track #3 turned out to be empty and was removed from output file"
MSG:4047,16,3,"AV sync issue in stream 7 at 0:00:00 : Forced subtitles track #7 turned out to be empty and was removed from output file","AV sync issue in stream %1 at %2 : %3","7","0:00:00","Forced subtitles track #7 turned out to be empty and was removed from output file"
MSG:5014,0,2,"Saving 1 titles into directory x","Saving %1 titles into directory %2","1","x"
"#;

    /// The file of SCAN_A's rip: video with two statistics tags, English 8-channel and French 2-channel audio, three
    /// chapters, 600.25 seconds. Synthetic.
    const IDENT_A: &str = r#"{"tracks": [{"id": 0, "type": "video", "codec": "MPEG-H/HEVC/H.265", "properties": {"language": "und", "tag_duration": "00:10:00", "tag_bps": "1000"}}, {"id": 1, "type": "audio", "codec": "TrueHD Atmos", "properties": {"language": "eng", "audio_channels": 8}}, {"id": 2, "type": "audio", "codec": "AC-3", "properties": {"language": "fre", "audio_channels": 2}}], "chapters": [{"num_entries": 2}, {"num_entries": 1}], "container": {"properties": {"duration": 600250000000}}}"#;

    /// What compare_tracks.py printed (exit 1) for `scan.txt 0 mkvmerge.json`, run once while the port was planned in
    /// a folder holding SCAN_A as scan.txt, IDENT_A as mkvmerge.json and RIP_A as rip.txt.
    const OUTPUT_A: [&str; 17] = [
        "scan                                                   | file",
        " 0 video     eng  MpegH        out=V_MPEGH/ISO/HEVC ch=   f=0 |  0 video     und  MPEG-H/HEVC/H.265 ch=",
        " 1 audio     eng  TrueHD       out=A_TRUEHD ch=8  f=0  |  1 audio     eng  TrueHD Atmos ch=8",
        " 2 audio     fre  DD           out=A_AC3    ch=2  f=0  |  2 audio     fre  AC-3 ch=2",
        " 4 subtitles eng  PGS          out=S_HDMV/PGS ch=   f=0 | ",
        "",
        "RIP LOG          rip.txt",
        "REMOVED          scan stream 3 (subtitles eng PGS f=6144): MakeMKV left it out of the file (MSG:4047), so it is not expected",
        "REMOVED          MSG:4047 names output track 7, which the prediction does not have",
        "TRACK COUNT      scan=4 file=3",
        "ORDER MATCH      no (type, language except on video, audio channels; position by position)",
        "CHAPTERS         scan=2 file=3",
        "DURATION         scan=600s file=600.2s",
        "VIDEO TRACKS     1",
        "STATISTICS TAGS  present",
        "VERDICT          count=no order=no video=yes chapters=no duration=yes",
        "                 codecs are not compared: read the scan's out= column against the file's codec column",
    ];

    /// What compare_tracks.py printed for one rip label of the evidence clone.
    struct Recorded {
        slug: &'static str,
        label: &'static str,
        /// The title its rip-summary.json names, and the selection its profile's file name gives.
        title: &'static str,
        selection: &'static str,
        /// SHA-1 of scan.txt, <label>/mkvmerge.json and <label>/rip.txt, concatenated in that order.
        inputs: &'static str,
        /// SHA-1 of the Python's stdout.
        stdout: &'static str,
        verdict: &'static str,
        exit: i32,
    }

    /// Every rip label of the evidence clone at its commit 41c5c51 (a folder with an ok rip-summary.json and an
    /// mkvmerge.json), run once while the port was planned, from the repository root, as
    /// `python3 scripts/phase0/compare_tracks.py corpus/owner/<slug>/scan.txt <title>
    /// corpus/owner/<slug>/<label>/mkvmerge.json --selection <selection>`.
    const EVIDENCE: [Recorded; 9] = [
        Recorded {
            slug: "pans-labyrinth-2006-uhd",
            label: "all-tracks",
            title: "0",
            selection: "all",
            inputs: "b772449537057aeb5ad95c1d865bc5ea3844bf38",
            stdout: "8cf48795af943fee8b13c0e99ec233881294c6a6",
            verdict: "VERDICT          count=yes order=yes video=yes chapters=yes duration=yes",
            exit: 0,
        },
        Recorded {
            slug: "pans-labyrinth-2006-uhd",
            label: "eng-only",
            title: "0",
            selection: "eng",
            inputs: "9fe8af391d88b1056c3415764420db1b4ee7540a",
            stdout: "c7782b4403d6d2c539bfed56488c6f7aff67822d",
            verdict: "VERDICT          count=yes order=yes video=yes chapters=yes duration=yes",
            exit: 0,
        },
        Recorded {
            slug: "pans-labyrinth-2006-uhd",
            label: "rehearsal",
            title: "1",
            selection: "all",
            inputs: "ba13b2b161e23f6bd83bb6a30814c1865f333085",
            stdout: "0e308a529e61e9aae65438c48edfbded2dd891f3",
            verdict: "VERDICT          count=yes order=yes video=yes chapters=yes duration=yes",
            exit: 0,
        },
        Recorded {
            slug: "short-circuit-1986-dvd",
            label: "all-tracks",
            title: "0",
            selection: "all",
            inputs: "485fff3f99618a2f63d0bfd5033714934386e846",
            stdout: "4dceadeded2fe250ad32591bc0eda8575031a2ae",
            verdict: "VERDICT          count=yes order=yes video=yes chapters=yes duration=no",
            exit: 1,
        },
        Recorded {
            slug: "short-circuit-2-1988-dvd",
            label: "all-tracks-2",
            title: "0",
            selection: "all",
            inputs: "677f25b3e9d5199fbb32796d764d6936c62b2244",
            stdout: "72b57ad0269c6d06acd951598e83110fd10431d2",
            verdict: "VERDICT          count=yes order=no video=yes chapters=yes duration=no",
            exit: 1,
        },
        Recorded {
            slug: "short-circuit-2-1988-dvd",
            label: "eng-only",
            title: "0",
            selection: "eng",
            inputs: "8fa56c49b7d32460305af2ac41a48c18c1976103",
            stdout: "36ca466323400e8b4eb9bb2a002e8c2de72b271a",
            verdict: "VERDICT          count=yes order=yes video=yes chapters=yes duration=no",
            exit: 1,
        },
        Recorded {
            slug: "the-office-2005-s3d3-bluray",
            label: "e16-stereo",
            title: "0",
            selection: "all",
            inputs: "4343e897e0715d581fc95449f8325e9c0588be54",
            stdout: "8e6d778454c791c5558929be1c88fde26b5d33ba",
            verdict: "VERDICT          count=yes order=yes video=yes chapters=yes duration=yes",
            exit: 0,
        },
        Recorded {
            slug: "the-office-2005-s3d3-bluray",
            label: "rehearsal",
            title: "1",
            selection: "all",
            inputs: "55bb846b05644016b13ffab8bbc118eb827d455b",
            stdout: "b4274dfd9411bc203bc9927dbfc406eb2602e7a3",
            verdict: "VERDICT          count=yes order=yes video=yes chapters=yes duration=yes",
            exit: 0,
        },
        Recorded {
            slug: "the-office-2005-s3d3-bluray",
            label: "t3-all-tracks",
            title: "3",
            selection: "all",
            inputs: "8fe14db6be94b602c22142c50ff35d1ff389695d",
            stdout: "db9afefd69ad57e32267380b9397184d461877e0",
            verdict: "VERDICT          count=yes order=yes video=yes chapters=yes duration=yes",
            exit: 0,
        },
    ];

    /// A folder under the system's temporary folder, removed when dropped. Py: TempDir (tempfile, "lazymkv-test-").
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> TempDir {
            let dir = std::env::temp_dir().join(format!("lazymkv-test-reconcile-{}-{name}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
        }

        fn write(&self, name: &str, text: &str) -> PathBuf {
            let path = self.0.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, text).unwrap();
            path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// The command run with `cwd` as the current directory: (exit code, stdout, stderr).
    fn run_in(cwd: &Path, args: &[&str]) -> (i32, String, String) {
        let ctx = Ctx::for_test(cwd);
        let args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run(&ctx, &args, &mut out, &mut err);
        (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
    }

    /// The RIP LOG line of a run over fixture A, which exits 1 wherever its rip log is found.
    fn rip_line(cwd: &Path, args: &[&str]) -> String {
        let (code, out, err) = run_in(cwd, args);
        assert_eq!((code, err.as_str()), (1, ""), "{args:?}");
        out.lines().find(|line| line.starts_with("RIP LOG")).unwrap_or_default().to_string()
    }

    #[test]
    fn a_4047_index_beyond_the_selection_is_reported_not_fatal() {
        let dir = TempDir::new("4047-beyond");
        let scan = dir.write("scan.txt", &TRACK_SCAN.replace("{lang}", "eng"));
        let json = dir.write("mkvmerge.json", TRACK_IDENT);
        let removed = "Forced subtitles track #9 turned out to be empty and was removed from output file";
        dir.write(
            "rip.txt",
            &format!(
                "MSG:4047,16,3,\"AV sync issue in stream 9 at 0:00:00 : {removed}\",\"AV sync issue in stream %1 at %2 : %3\",\"9\",\"0:00:00\",\"{removed}\"\n"
            ),
        );
        let (code, out, _) = run_in(&dir.0, &[scan.to_str().unwrap(), "0", json.to_str().unwrap()]);
        assert!(out.contains("MSG:4047 names output track 9, which the prediction does not have"), "{out}");
        assert!(out.contains("CHAPTERS"), "{out}");
        assert_eq!(code, 0);
    }

    // New in the port.

    #[test]
    fn the_output_is_the_pythons_byte_for_byte() {
        let dir = TempDir::new("python-output");
        dir.write("scan.txt", SCAN_A);
        dir.write("mkvmerge.json", IDENT_A);
        dir.write("rip.txt", RIP_A);
        let (code, out, err) = run_in(&dir.0, &["scan.txt", "0", "mkvmerge.json"]);
        assert_eq!(out, printed(&OUTPUT_A));
        assert_eq!(err, "");
        assert_eq!(code, 1);
    }

    #[test]
    fn line_ends_are_read_as_python_text_mode_reads_them() {
        // Path.read_text turns "\r\n" and a lone "\r" into "\n" before robot.parse splits on "\n": the Python printed
        // OUTPUT_A for SCAN_A written with lone "\r" line ends and RIP_A with "\r\n" (checked while planning).
        let dir = TempDir::new("line-ends");
        dir.write("scan.txt", &SCAN_A.replace('\n', "\r"));
        dir.write("mkvmerge.json", IDENT_A);
        dir.write("rip.txt", &RIP_A.replace('\n', "\r\n"));
        let (code, out, err) = run_in(&dir.0, &["scan.txt", "0", "mkvmerge.json"]);
        assert_eq!(out, printed(&OUTPUT_A));
        assert_eq!(err, "");
        assert_eq!(code, 1);
    }

    #[test]
    fn the_rip_log_is_looked_for_and_printed_as_pathlib_prints_it() {
        let dir = TempDir::new("rip-log");
        dir.write("scan.txt", SCAN_A);
        dir.write("sub/mkvmerge.json", IDENT_A);
        dir.write("sub/extract.txt", RIP_A);
        dir.write("logs/rip.txt", RIP_A);
        assert_eq!(rip_line(&dir.0, &["scan.txt", "0", "sub/mkvmerge.json"]), "RIP LOG          sub/extract.txt");
        assert_eq!(
            rip_line(&dir.0, &["scan.txt", "0", "sub/mkvmerge.json", "--rip-log", "./logs//rip.txt"]),
            "RIP LOG          logs/rip.txt"
        );
        // An empty --rip-log is falsy in the Python, so the log is looked for beside mkvmerge.json.
        assert_eq!(
            rip_line(&dir.0, &["scan.txt", "0", "sub/mkvmerge.json", "--rip-log", ""]),
            "RIP LOG          sub/extract.txt"
        );
        // rip.txt is looked for before extract.txt.
        dir.write("sub/rip.txt", RIP_A);
        assert_eq!(rip_line(&dir.0, &["scan.txt", "0", "./sub//mkvmerge.json"]), "RIP LOG          sub/rip.txt");
    }

    #[test]
    fn unreadable_inputs_exit_2() {
        // The Python crashed with a traceback (exit 1) on each of these; exit 2 is the deliberate change (spec 3.4).
        let dir = TempDir::new("unreadable");
        dir.write("scan.txt", SCAN_A);
        dir.write("mkvmerge.json", IDENT_A);
        dir.write("broken.json", "{\"tracks\": [");
        let cases: [(&[&str], &str); 5] = [
            (&["nothere.txt", "0", "mkvmerge.json"], "[Errno 2] No such file or directory: 'nothere.txt'"),
            (&["scan.txt", "0", "broken.json"], "broken.json: "),
            (
                &["scan.txt", "0", "mkvmerge.json", "--rip-log", "nothere.txt"],
                "[Errno 2] No such file or directory: 'nothere.txt'",
            ),
            (&["scan.txt", "5", "mkvmerge.json"], "the scan lists no title 5"),
            (&["scan.txt", "-1", "mkvmerge.json"], "the scan lists no title -1"),
        ];
        for (args, message) in cases {
            let (code, out, err) = run_in(&dir.0, args);
            assert_eq!(code, 2, "{args:?}");
            assert_eq!(out, "", "{args:?}");
            assert!(err.contains(message), "{args:?}: {err}");
        }
    }

    #[test]
    fn argument_errors_exit_2_with_argparses_messages() {
        let dir = TempDir::new("arguments");
        let cases: [(&[&str], &str); 4] = [
            (&["scan.txt"], "the following arguments are required: title, mkvmerge_json"),
            (&["scan.txt", "x", "y"], "argument title: invalid int value: 'x'"),
            (
                &["scan.txt", "0", "y", "--selection", "fr"],
                "argument --selection: invalid choice: 'fr' (choose from 'all', 'eng')",
            ),
            (&["scan.txt", "0", "y", "z", "w"], "unrecognized arguments: z w"),
        ];
        for (args, message) in cases {
            let (code, out, err) = run_in(&dir.0, args);
            assert_eq!(code, 2, "{args:?}");
            assert_eq!(out, "", "{args:?}");
            assert!(err.contains(message), "{args:?}: {err}");
        }
    }

    #[test]
    fn paths_are_shown_as_pathlib_shows_them() {
        // str(PurePosixPath(p)) and str(PurePosixPath(p).parent / "rip.txt") as Python 3.9 printed them.
        let cases = [
            ("mkvmerge.json", "mkvmerge.json", "rip.txt"),
            ("a//b/./mkvmerge.json", "a/b/mkvmerge.json", "a/b/rip.txt"),
            ("/x/mkvmerge.json", "/x/mkvmerge.json", "/x/rip.txt"),
            ("//x/m.json", "//x/m.json", "//x/rip.txt"),
            ("///x/m.json", "/x/m.json", "/x/rip.txt"),
            ("../m.json", "../m.json", "../rip.txt"),
            ("a/", "a", "rip.txt"),
            ("./logs//extract.txt", "logs/extract.txt", "logs/rip.txt"),
            ("", ".", "rip.txt"),
            ("/", "/", "/rip.txt"),
            (".", ".", "rip.txt"),
            ("a/..", "a/..", "a/rip.txt"),
        ];
        for (given, path, beside) in cases {
            assert_eq!(shown(given), path, "{given:?}");
            assert_eq!(shown_beside(given, "rip.txt"), beside, "{given:?}");
        }
    }

    #[test]
    fn every_evidence_rip_label_gives_the_pythons_output() {
        let Some(root) = repo_root(Path::new(env!("CARGO_MANIFEST_DIR"))) else {
            skip("no repository root (a Cargo.toml with [workspace]) above crates/dev");
            return;
        };
        if !root.join("corpus").join("owner").is_dir() {
            skip("no evidence clone at corpus/owner");
            return;
        }
        for r in &EVIDENCE {
            let scan = format!("corpus/owner/{}/scan.txt", r.slug);
            let json = format!("corpus/owner/{}/{}/mkvmerge.json", r.slug, r.label);
            let rip = format!("corpus/owner/{}/{}/rip.txt", r.slug, r.label);
            let mut inputs = Vec::new();
            let mut absent = None;
            for path in [&scan, &json, &rip] {
                match std::fs::read(root.join(path)) {
                    Ok(bytes) => inputs.extend(bytes),
                    Err(_) => {
                        absent = Some(path.clone());
                        break;
                    }
                }
            }
            if let Some(path) = absent {
                skip(&format!("{path} is not in the evidence clone"));
                continue;
            }
            if sha1_hex(&inputs) != r.inputs {
                skip(&format!("{}/{}: its inputs changed since the Python's output was recorded", r.slug, r.label));
                continue;
            }
            let (code, out, err) =
                run_in(&root, &[scan.as_str(), r.title, json.as_str(), "--selection", r.selection]);
            let context = format!(
                "{}/{}: the expected output is what compare_tracks printed, at the last commit with the Python, for \
                 {scan} {} {json} --selection {}\n{out}{err}",
                r.slug, r.label, r.title, r.selection
            );
            assert_eq!(out.lines().find(|line| line.starts_with("VERDICT")), Some(r.verdict), "{context}");
            assert_eq!(sha1_hex(out.as_bytes()), r.stdout, "{context}");
            assert_eq!(code, r.exit, "{context}");
        }
    }
}
