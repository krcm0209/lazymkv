//! `lazymkv-dev titles [--evidence DIR] <slug|dir>`: which titles of a recorded scan have two or more audio streams
//! in one language, and which were ripped. Exit 0; 1 no scan.txt or a usage error (the Python's sys.exit); 2 a file
//! that could not be read (where the Python crashed); 3 no evidence clone. Py: audio_similarity.main's --titles.

use std::io::Write;

use lazymkv_core::audio::titles::titles_report;
use lazymkv_core::recording::Tree;

use crate::evidence::{resolve_recording, Ctx, Refused};
use crate::{usage_error, Args};

/// The synopsis after "usage: lazymkv-dev ".
const USAGE: &str = "titles [--evidence DIR] <slug|dir>";
/// What follows the recording's path when it has no scan.txt, verbatim.
const NO_SCAN: &str = "no scan.txt (record and pull the disc first)";

pub fn run(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let mut parsed = Args::new(args, &["--evidence"]);
    let evidence = match parsed.flag("--evidence") {
        Ok(value) => value,
        Err(message) => return usage(err, &message),
    };
    let words = match parsed.positionals() {
        Ok(words) => words,
        Err(message) => return usage(err, &message),
    };
    match words.len() {
        0 => return usage(err, "the following arguments are required: slug|dir"),
        1 => {}
        _ => return usage(err, &format!("unrecognized arguments: {}", words[1..].join(" "))),
    }
    let rec = match resolve_recording(ctx, evidence.as_deref(), &words[0]) {
        Ok(rec) => rec,
        Err(Refused { code, line }) => {
            let _ = writeln!(err, "{line}");
            return code;
        }
    };
    if !rec.tree.is_file(&rec.dir.join("scan.txt")) {
        let _ = writeln!(err, "{}: {}", rec.tree.display(&rec.dir), NO_SCAN);
        return 1;
    }
    match titles_report(&rec.tree, &rec.dir) {
        Ok(lines) => {
            for line in lines {
                let _ = writeln!(out, "{line}");
            }
            0
        }
        Err(message) => {
            let _ = writeln!(err, "{message}");
            2
        }
    }
}

/// argparse's two lines, with the Python's exit 1 (its `--titles` form exited through sys.exit("usage: ...")).
fn usage(err: &mut dyn Write, message: &str) -> i32 {
    usage_error(err, "titles", USAGE, message);
    1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::{repo_root, skip, Ctx};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A made-up recording's scan: one title with two English streams.
    const SCAN: &str = concat!(
        "TINFO:0,9,0,\"0:21:49\"\n",
        "TINFO:0,26,0,\"7\"\n",
        "SINFO:0,1,1,0,\"Audio\"\n",
        "SINFO:0,1,3,0,\"eng\"\n",
        "SINFO:0,2,1,0,\"Audio\"\n",
        "SINFO:0,2,3,0,\"eng\"\n",
    );

    /// The Python's `audio_similarity.py --titles some-disc` for SCAN with e16/rip-summary.json
    /// {"ok": true, "title": 0}.
    const SOME_DISC: &[&str] = &[
        "some-disc: 1 titles, longest first; TMDb original language: unknown",
        "  audio: scan stream numbers by language, leaving out streams MakeMKV derived from another (flag 2048)",
        "  title   0   0:21:49  segments 7         audio eng 1,2   <- eng x2; ripped as e16",
    ];

    /// A fresh folder under the system's temporary directory, removed when dropped.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(tag: &str) -> Scratch {
            static COUNT: AtomicUsize = AtomicUsize::new(0);
            let n = COUNT.fetch_add(1, Ordering::SeqCst);
            let dir = std::env::temp_dir().join(format!("lazymkv-test-{tag}-{}-{n}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Scratch(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// Runs the command; (exit code, stdout, stderr).
    fn call(ctx: &Ctx, args: &[&str]) -> (i32, String, String) {
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run(ctx, &args, &mut out, &mut err);
        (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
    }

    #[test]
    fn the_short_circuit_2_dvd_shares_no_segment() {
        let repo = repo_root(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("the workspace root");
        if !repo.join("corpus/owner/short-circuit-2-1988-dvd/scan.txt").is_file() {
            skip("the evidence repository is not cloned at corpus/owner");
            return;
        }
        let (code, out, err) = call(&Ctx::for_test(&repo), &["short-circuit-2-1988-dvd"]);
        assert_eq!(code, 0, "{err}");
        let lines: Vec<&str> = out.lines().collect();
        assert!(lines[2].contains("so shared segments are not looked for"), "{out}");
        assert!(lines.iter().all(|line| !line.contains("shares segments")), "{out}");
    }

    #[test]
    fn titles_prints_the_listing_of_a_recording_directory() {
        let tmp = Scratch::new("titles");
        let d = tmp.0.join("some-disc");
        fs::create_dir_all(d.join("e16")).unwrap();
        fs::write(d.join("scan.txt"), SCAN).unwrap();
        fs::write(d.join("e16").join("rip-summary.json"), r#"{"ok": true, "title": 0}"#).unwrap();
        let (code, out, err) = call(&Ctx::for_test(&tmp.0), &["some-disc"]);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(out, SOME_DISC.join("\n") + "\n");
    }

    #[test]
    fn a_recording_without_scan_txt_exits_1() {
        let tmp = Scratch::new("titles");
        fs::create_dir_all(tmp.0.join("empty-disc")).unwrap();
        let (code, out, err) = call(&Ctx::for_test(&tmp.0), &["empty-disc"]);
        assert_eq!((code, out.as_str()), (1, ""));
        assert_eq!(err, "empty-disc: no scan.txt (record and pull the disc first)\n");
    }

    #[test]
    fn a_slug_without_the_evidence_clone_exits_3() {
        let tmp = Scratch::new("titles");
        let mut ctx = Ctx::for_test(&tmp.0);
        ctx.env.insert("LAZYMKV_EVIDENCE".to_string(), tmp.0.join("no-evidence").display().to_string());
        let (code, out, err) = call(&ctx, &["some-slug"]);
        assert_eq!((code, out.as_str()), (3, ""));
        assert!(err.contains("no such folder; the evidence repository lazymkv-evidence is not cloned there"), "{err}");
    }

    #[test]
    fn a_wrong_argument_count_prints_the_usage_and_exits_1() {
        let tmp = Scratch::new("titles");
        let ctx = Ctx::for_test(&tmp.0);
        for args in [&[][..], &["some-disc", "other-disc"][..]] {
            let (code, out, err) = call(&ctx, args);
            assert_eq!((code, out.as_str()), (1, ""), "{args:?}");
            assert!(err.starts_with("usage: lazymkv-dev titles [--evidence DIR] <slug|dir>\n"), "{err}");
        }
    }

    #[test]
    fn titles_is_reached_through_its_row_of_the_command_table() {
        let row = crate::COMMANDS
            .iter()
            .find(|(name, _, _)| *name == "titles")
            .expect("a titles row in COMMANDS");
        assert_eq!(row.2, "[--evidence DIR] <slug|dir>");
        let tmp = Scratch::new("titles");
        fs::create_dir_all(tmp.0.join("empty-disc")).unwrap();
        let argv: Vec<String> = ["titles", "empty-disc"].iter().map(|a| a.to_string()).collect();
        let (mut out, mut err): (Vec<u8>, Vec<u8>) = (Vec::new(), Vec::new());
        assert_eq!(crate::dispatch(&Ctx::for_test(&tmp.0), &argv, &mut out, &mut err), 1);
        assert!(out.is_empty());
        assert_eq!(String::from_utf8(err).unwrap(), "empty-disc: no scan.txt (record and pull the disc first)\n");
    }
}
