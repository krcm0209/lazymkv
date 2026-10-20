//! `lazymkv-dev subs (--rec DIR | <slug|dir> <label>) [--text DIR] [--orig-lang CODE] [--clear-max-bytes N]
//! [--evidence DIR]` (port spec 3.4): the English subtitle text rule over one rip label, line for line as the Python
//! `subtitle_text` printed it (numbers only, no subtitle text). The OCR text folder (`--text`, else
//! LAZYMKV_OCR_TEXT) holds sub-<n>.ocr.jsonl as the Python read it. Exit 0; 2 for an argument error, unreadable
//! input, no OCR text, or an English or no-language bitmap stream without OCR text; 3 when a slug is given and the
//! evidence clone is absent.

use std::io::Write;
use std::path::{Path, PathBuf};

use lazymkv_core::recording::{py_int, py_repr_str, MemTree, PathKey, Tree};
use lazymkv_core::subtitle::labels::READING;
use lazymkv_core::subtitle::text::{subs_run, Rule};

use crate::evidence::{insert_disk_file, ocr_text_dir, pathlib_split, resolve_recording, Ctx};
use crate::{usage_error, Args};

/// The command's name as typed.
const NAME: &str = "subs";
/// The usage line, after "usage: lazymkv-dev " (argparse's form: the command's name first).
pub const USAGE: &str =
    "subs [--evidence DIR] (--rec DIR | <slug|dir> <label>) [--text DIR] [--orig-lang CODE] [--clear-max-bytes N]";
/// The command table's synopsis: spec 3.4's arguments after the name.
pub const SYNOPSIS: &str =
    "(--rec DIR | <slug|dir> <label>) [--text DIR] [--orig-lang CODE] [--clear-max-bytes N] [--evidence DIR]";

/// The command, registered in COMMANDS as "subs".
pub fn run(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        let _ = writeln!(out, "usage: lazymkv-dev {USAGE}");
        return 0;
    }
    let mut parsed = Args::new(args, &["--rec", "--text", "--orig-lang", "--clear-max-bytes", "--evidence"]);
    let rec_flag = match parsed.flag("--rec") {
        Ok(value) => value,
        Err(message) => return usage_error(err, NAME, USAGE, &message),
    };
    let text_flag = match parsed.flag("--text") {
        Ok(value) => value,
        Err(message) => return usage_error(err, NAME, USAGE, &message),
    };
    let orig_lang = match parsed.flag("--orig-lang") {
        Ok(value) => value,
        Err(message) => return usage_error(err, NAME, USAGE, &message),
    };
    let clear_flag = match parsed.flag("--clear-max-bytes") {
        Ok(value) => value,
        Err(message) => return usage_error(err, NAME, USAGE, &message),
    };
    let evidence = match parsed.flag("--evidence") {
        Ok(value) => value,
        Err(message) => return usage_error(err, NAME, USAGE, &message),
    };
    let words = match parsed.positionals() {
        Ok(words) => words,
        Err(message) => return usage_error(err, NAME, USAGE, &message),
    };
    // argparse's type=int, with RULE['CLEAR_MAX_BYTES'] as the default.
    let clear_max_bytes = match clear_flag {
        None => Rule::default().CLEAR_MAX_BYTES,
        Some(text) => match py_int(&text) {
            Some(number) => number,
            None => {
                let message = format!("argument --clear-max-bytes: invalid int value: {}", py_repr_str(&text));
                return usage_error(err, NAME, USAGE, &message);
            }
        },
    };
    if words.len() > 2 {
        return usage_error(err, NAME, USAGE, &format!("unrecognized arguments: {}", words[2..].join(" ")));
    }
    if rec_flag.is_some() && !words.is_empty() {
        return usage_error(err, NAME, USAGE, "argument --rec: not allowed with <slug|dir> <label>");
    }
    if rec_flag.is_none() && words.is_empty() {
        return usage_error(err, NAME, USAGE, "one of the arguments --rec or <slug|dir> <label> is required");
    }
    if rec_flag.is_none() && words.len() == 1 {
        return usage_error(err, NAME, USAGE, "the following arguments are required: label");
    }
    let Some(text_dir) = ocr_text_dir(ctx, text_flag.as_deref()) else {
        return usage_error(err, NAME, USAGE, "the following arguments are required: --text");
    };
    // The folder as the user wrote it (flag or environment), printed as pathlib prints it.
    let text_given = text_flag.or_else(|| ctx.env.get("LAZYMKV_OCR_TEXT").cloned()).unwrap_or_default();
    let (rec_tree, rec): (Box<dyn Tree>, PathKey) = match rec_flag {
        Some(dir) => rec_folder_tree(ctx, &dir),
        None => match resolve_recording(ctx, evidence.as_deref(), &words[0]) {
            Ok(recording) => {
                let rec = recording.dir.join(&words[1]);
                (Box::new(recording.tree) as Box<dyn Tree>, rec)
            }
            Err(refused) => {
                let _ = writeln!(err, "{}", refused.line);
                return refused.code;
            }
        },
    };
    let mut text_tree = MemTree::new(&pathlib_split(&text_given).0, "");
    add_folder_files(&mut text_tree, &PathKey::root(), &text_dir);
    let run = subs_run(
        rec_tree.as_ref(),
        &rec,
        &text_tree,
        &PathKey::root(),
        orig_lang.as_deref(),
        clear_max_bytes,
        &Rule::default(),
    );
    for line in &run.stdout {
        let _ = writeln!(out, "{line}");
    }
    for line in &run.stderr {
        let _ = writeln!(err, "{line}");
    }
    run.exit_code
}

/// The --rec folder as the rule reads it: a tree printed as pathlib prints the argument's parent, holding the
/// parent's tmdb.json and the files of the folder's subs/ and subs-ocr/ (the only files the rule reads), so that the
/// rest of the parent is not walked; and the folder's key in it. For ".", "/" and the like (no name), the folder is
/// its own parent, as Path('.').parent is Path('.').
fn rec_folder_tree(ctx: &Ctx, arg: &str) -> (Box<dyn Tree>, PathKey) {
    let (text, parent, name) = pathlib_split(arg);
    let name = if name == "." { String::new() } else { name };
    let rec_dir = ctx.abs(arg);
    let (display, rec, parent_dir): (String, PathKey, PathBuf) = if name.is_empty() {
        (text, PathKey::root(), rec_dir.clone())
    } else {
        let parent_dir = rec_dir.parent().map(Path::to_path_buf).unwrap_or_else(|| rec_dir.clone());
        (parent, PathKey::new(&name), parent_dir)
    };
    let mut tree = MemTree::new(&display, &name);
    let tmdb = parent_dir.join("tmdb.json");
    if tmdb.is_file() {
        insert_disk_file(&mut tree, &rec.parent().join("tmdb.json"), &tmdb);
    }
    for folder in ["subs", READING] {
        add_folder_files(&mut tree, &rec.join(folder), &rec_dir.join(folder));
    }
    (Box::new(tree), rec)
}

/// Adds every file directly in `dir` under `key`; nothing when `dir` cannot be listed (its files are then missing,
/// as they were for the Python).
fn add_folder_files(tree: &mut MemTree, key: &PathKey, dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    tree.insert_dir(&key.to_string());
    let mut names: Vec<String> =
        entries.filter_map(|entry| entry.ok()).filter_map(|entry| entry.file_name().into_string().ok()).collect();
    names.sort();
    for name in names {
        let path = dir.join(&name);
        if path.is_file() {
            insert_disk_file(tree, &key.join(&name), &path);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use lazymkv_core::recording::{py_fixed, py_float_repr, MemTree, PathKey};
    use lazymkv_core::subtitle::text::load_title;
    use serde_json::{json, Value};

    use super::{run, SYNOPSIS};
    use crate::evidence::{pathlib_split, repo_root, resolve_recording, skip, Ctx};
    use crate::testutil::{call, evidence_inputs, sha1_hex, Scratch as TempDir, ScratchExt};

    /// Made-up words: no real subtitle text.
    const WORDS: [&str; 11] = ["babin", "dagon", "gikol", "kubal", "lodin", "bekan", "dulog", "gamin", "kodel", "labun", "bodig"];

    /// The OCR lines of event k: a made-up sentence, and a sound cue as event 5.
    fn lines_of(k: usize) -> Vec<String> {
        if k == 5 {
            return vec!["(DOOR SLAMS)".to_string()];
        }
        vec![format!("We {} the {} and the {}.", WORDS[k % 11], WORDS[(3 * k + 1) % 11], WORDS[(7 * k + 2) % 11])]
    }

    /// A rip label's subs/ under `rec` and its OCR text under `text`, as the mini PC writes them: index.json with a
    /// byte-order mark and CRLF line ends (probe-title.ps1), the jsonl with CRLF. s0 English PGS with 30 events and
    /// text, s1 SubRip closed captions (3 packets) without text, s2 French PGS without text.
    fn write_label(tmp: &TempDir, rec: &str, text: &str) {
        let mut packets = Vec::new();
        for k in 0..30 {
            let start = 10.0 + 4.0 * k as f64;
            packets.push(json!({"pts_time": py_fixed(start, 3), "duration_time": "2.500", "size": "4000"}));
            packets.push(json!({"pts_time": py_fixed(start + 2.5, 3), "duration_time": "0.021", "size": "30"}));
        }
        let cc: Vec<Value> = (0..3)
            .map(|i| json!({"pts_time": py_fixed(20.0 + 3.0 * i as f64, 3), "duration_time": "2.000", "size": "40"}))
            .collect();
        let index = json!([
            {"n": 0, "track_id": 3, "language": "eng", "codec": "HDMV PGS", "track_name": null, "forced": false, "default": false},
            {"n": 1, "track_id": 4, "language": "eng", "codec": "SubRip/SRT", "track_name": null, "forced": false, "default": false},
            {"n": 2, "track_id": 5, "language": "fre", "codec": "HDMV PGS", "track_name": null, "forced": false, "default": false}
        ]);
        let mut bytes = b"\xef\xbb\xbf".to_vec();
        bytes.extend(serde_json::to_string_pretty(&index).unwrap().replace('\n', "\r\n").into_bytes());
        tmp.write(&format!("{rec}/subs/index.json"), &bytes);
        for (n, list) in [(0, &packets), (1, &cc), (2, &packets)] {
            tmp.write(&format!("{rec}/subs/sub-{n}.packets.json"), json!({"packets": list}).to_string().as_bytes());
        }
        let recs: Vec<String> =
            (0..30).map(|k| json!({"frame": k, "t": 4.0 * k as f64, "lines": lines_of(k)}).to_string()).collect();
        tmp.write(&format!("{text}/sub-0.ocr.jsonl"), format!("{}\r\n", recs.join("\r\n")).as_bytes());
    }

    /// What the Python printed for `--rec rec --text text --orig-lang en` over write_label (Python 3.9.6, 2026-10-13).
    const REC_EN: &str = concat!(
        "rec: 1 subtitle streams with OCR text, 1 not read (another language), 1 set aside (closed captions); rule thresholds are provisional\n",
        "stream s0 eng: events=30 lines=30 words=203 letters=1.000 wordlike=1.000 readable=yes lang=en (en=0.571 es=0.000 fr=0.000 de=0.000) CUE=1 LABEL=0 CHEV=0 M=1 rate=0.0333 SO=1 CS=1 caps_share=0.000 CAPS_CUE=0 NOTE=0 NOTE_LOOSE=0 ODD=0 class=unclear  [text by order, 30 frames with text, 0 unmatched]\n",
        "stream s1 eng: set aside (closed captions: SubRip/SRT is a text stream; 8.7 sets it aside), 3 events\n",
        "stream s2 fre: not read (no OCR text), 30 events, used for timing only\n",
        "original language: en\n",
        "| Stream | Tag | Events | Class | Rule | Review | Note | 8.7 today | Before yes |\n",
        "|---|---|---|---|---|---|---|---|---|\n",
        "| s0 | eng | 30 | unclear | regular | subtitle_ambiguous | some SDH markers (1 events, 1 sound-only, 1 naming a sound) | regular | regular (review: subtitle_text_disagrees) |\n",
        "| s1 | eng | 3 | set aside | set aside | - | closed captions, a text stream (8.7: set aside first) | set aside | set aside |\n",
        "| s2 | fre | 30 | not read | drop | - | not English (tag fre) | drop | drop |\n",
        "row: | rec/evidence.mks | s0: HDMV PGS eng, 30 events | M 1, SO 1, CS 1, lang en, unclear: regular (review: some SDH markers (1 events, 1 sound-only, 1 naming a sound)) | |\n",
        "row: | rec/evidence.mks | s1: SubRip/SRT eng, 3 events | set aside (closed captions): set aside | |\n",
        "row: | rec/evidence.mks | s2: HDMV PGS fre, 30 events | not read (timing only): drop | |\n",
    );

    /// What the Python printed for `--rec corpus/owner/test-disc-2000-bluray/all-tracks --text text`, the original
    /// language "es" read from the recording's tmdb.json.
    const SLUG_ES: &str = concat!(
        "corpus/owner/test-disc-2000-bluray/all-tracks: 1 subtitle streams with OCR text, 1 not read (another language), 1 set aside (closed captions); rule thresholds are provisional\n",
        "stream s0 eng: events=30 lines=30 words=203 letters=1.000 wordlike=1.000 readable=yes lang=en (en=0.571 es=0.000 fr=0.000 de=0.000) CUE=1 LABEL=0 CHEV=0 M=1 rate=0.0333 SO=1 CS=1 caps_share=0.000 CAPS_CUE=0 NOTE=0 NOTE_LOOSE=0 ODD=0 class=unclear  [text by order, 30 frames with text, 0 unmatched]\n",
        "stream s1 eng: set aside (closed captions: SubRip/SRT is a text stream; 8.7 sets it aside), 3 events\n",
        "stream s2 fre: not read (no OCR text), 30 events, used for timing only\n",
        "original language: es\n",
        "| Stream | Tag | Events | Class | Rule | Review | Note | 8.7 today | Before yes |\n",
        "|---|---|---|---|---|---|---|---|---|\n",
        "| s0 | eng | 30 | unclear | regular | subtitle_ambiguous | some SDH markers (1 events, 1 sound-only, 1 naming a sound) | regular | regular (review: subtitle_text_disagrees) |\n",
        "| s1 | eng | 3 | set aside | set aside | - | closed captions, a text stream (8.7: set aside first) | set aside | set aside |\n",
        "| s2 | fre | 30 | not read | drop | - | not English (tag fre) | drop | drop |\n",
        "row: | all-tracks/evidence.mks | s0: HDMV PGS eng, 30 events | M 1, SO 1, CS 1, lang en, unclear: regular (review: some SDH markers (1 events, 1 sound-only, 1 naming a sound)) | |\n",
        "row: | all-tracks/evidence.mks | s1: SubRip/SRT eng, 3 events | set aside (closed captions): set aside | |\n",
        "row: | all-tracks/evidence.mks | s2: HDMV PGS fre, 30 events | not read (timing only): drop | |\n",
    );

    /// `run` with `ctx`: (exit code, stdout, stderr).
    fn run_with(ctx: &Ctx, args: &[&str]) -> (i32, String, String) {
        call(run, ctx, args)
    }

    /// `run` with `dir` as the current directory.
    fn run_in(dir: &Path, args: &[&str]) -> (i32, String, String) {
        run_with(&Ctx::for_test(dir), args)
    }

    #[test]
    fn run_prints_the_pythons_lines_from_disk() {
        let tmp = TempDir::new("rec-form");
        write_label(&tmp, "rec", "text");
        let (code, out, err) = run_in(&tmp.0, &["--rec", "rec", "--text", "text", "--orig-lang", "en"]);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(out, REC_EN);
        // A trailing slash and the "=" spelling change nothing (pathlib drops the slash).
        let (code, out, _) = run_in(&tmp.0, &["--rec=rec/", "--text=text", "--orig-lang=en"]);
        assert_eq!((code, out.as_str()), (0, REC_EN));
    }

    #[test]
    fn run_reads_a_slug_a_recording_path_and_a_label_folder_alike() {
        let tmp = TempDir::new("three-forms");
        write_label(&tmp, "corpus/owner/test-disc-2000-bluray/all-tracks", "text");
        let tmdb = r#"{"original_language": "es", "fetched_at": "2026-10-13"}"#;
        tmp.write("corpus/owner/test-disc-2000-bluray/tmdb.json", tmdb.as_bytes());
        // The slug under --evidence (printed as corpus/owner whatever --evidence says).
        let evidence = tmp.0.join("corpus/owner");
        let (code, out, err) = run_in(
            &tmp.0,
            &["test-disc-2000-bluray", "all-tracks", "--text", "text", "--evidence", evidence.to_str().unwrap()],
        );
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(out, SLUG_ES);
        // The recording as a path, and the label folder as --rec: the paths as given.
        let (code, out, _) = run_in(&tmp.0, &["corpus/owner/test-disc-2000-bluray", "all-tracks", "--text", "text"]);
        assert_eq!((code, out.as_str()), (0, SLUG_ES));
        let (code, out, _) = run_in(&tmp.0, &["--rec", "corpus/owner/test-disc-2000-bluray/all-tracks", "--text", "text"]);
        assert_eq!((code, out.as_str()), (0, SLUG_ES));
    }

    #[test]
    fn run_reports_input_errors_as_the_python() {
        let tmp = TempDir::new("errors");
        write_label(&tmp, "rec", "text");
        // What the Python printed for the same folders, 2026-10-13.
        let (code, out, err) = run_in(&tmp.0, &["--rec", "missing", "--text", "text"]);
        assert_eq!((code, out.as_str()), (2, ""));
        assert_eq!(err, "cannot read the input: [Errno 2] No such file or directory: 'missing/subs/index.json'\n");
        let (code, out, err) = run_in(&tmp.0, &["--rec", "rec", "--text", "no-text"]);
        assert_eq!((code, out.as_str()), (2, ""));
        assert_eq!(err, "no sub-<n>.ocr.jsonl in no-text for the streams of rec/subs/index.json\n");
        // An English bitmap stream without OCR text.
        let index = json!([
            {"n": 0, "language": "eng", "codec": "HDMV PGS", "forced": false},
            {"n": 1, "language": "eng", "codec": "SubRip/SRT", "forced": false},
            {"n": 2, "language": "fre", "codec": "HDMV PGS", "forced": false},
            {"n": 3, "language": "eng", "codec": "HDMV PGS", "forced": false}
        ]);
        tmp.write("rec/subs/index.json", index.to_string().as_bytes());
        let packets = std::fs::read(tmp.0.join("rec/subs/sub-0.packets.json")).unwrap();
        tmp.write("rec/subs/sub-3.packets.json", &packets);
        let (code, out, err) = run_in(&tmp.0, &["--rec", "rec", "--text", "text"]);
        assert_eq!((code, out.as_str()), (2, ""));
        assert_eq!(err, "stream s3 eng: no OCR text (no sub-3.ocr.jsonl in text); read it with ocr-subs.ps1\n");
    }

    #[test]
    fn run_takes_the_text_folder_from_the_environment() {
        let tmp = TempDir::new("env");
        write_label(&tmp, "rec", "text");
        let mut ctx = Ctx::for_test(&tmp.0);
        ctx.env.insert("LAZYMKV_OCR_TEXT".to_string(), "text".to_string());
        let (code, out, err) = run_with(&ctx, &["--rec", "rec", "--orig-lang", "en"]);
        assert_eq!((code, out.as_str(), err.as_str()), (0, REC_EN, ""));
        // --text wins over the environment.
        ctx.env.insert("LAZYMKV_OCR_TEXT".to_string(), "elsewhere".to_string());
        let (code, out, _) = run_with(&ctx, &["--rec", "rec", "--text", "text", "--orig-lang", "en"]);
        assert_eq!((code, out.as_str()), (0, REC_EN));
    }

    #[test]
    fn run_refuses_without_the_evidence_clone() {
        let tmp = TempDir::new("no-clone");
        let (code, out, err) =
            run_in(&tmp.0, &["some-slug", "all-tracks", "--text", "text", "--evidence", "absent"]);
        assert_eq!((code, out.as_str()), (3, ""));
        assert!(err.contains("the evidence repository lazymkv-evidence is not cloned there"), "{err}");
    }

    #[test]
    fn run_reports_usage_errors() {
        let tmp = TempDir::new("usage");
        let cases: [(&[&str], &str); 7] = [
            (&[], "one of the arguments --rec or <slug|dir> <label> is required"),
            (&["--rec", "rec"], "the following arguments are required: --text"),
            (&["--rec", "rec", "slug", "label", "--text", "t"], "argument --rec: not allowed with <slug|dir> <label>"),
            (&["slug", "--text", "t"], "the following arguments are required: label"),
            (&["a", "b", "c", "--text", "t"], "unrecognized arguments: c"),
            (&["--rec", "rec", "--text", "t", "--clear-max-bytes", "x"], "argument --clear-max-bytes: invalid int value: 'x'"),
            (&["--rec", "rec", "--text", "t", "--bogus"], "unrecognized arguments: --bogus"),
        ];
        for (args, message) in cases {
            let (code, out, err) = run_in(&tmp.0, args);
            assert_eq!((code, out.as_str()), (2, ""), "{args:?}");
            assert_eq!(
                err,
                format!("usage: lazymkv-dev {}\nlazymkv-dev subs: error: {message}\n", super::USAGE),
                "{args:?}"
            );
        }
        let (code, out, _) = run_in(&tmp.0, &["--help"]);
        assert_eq!((code, out), (0, format!("usage: lazymkv-dev {}\n", super::USAGE)));
    }

    #[test]
    fn subs_is_in_the_command_table() {
        let row = crate::COMMANDS.iter().find(|(name, _, _)| *name == "subs").map(|(_, _, synopsis)| *synopsis);
        assert_eq!(row, Some(SYNOPSIS));
        let at = |name: &str| crate::COMMANDS.iter().position(|(command, _, _)| *command == name);
        // Spec 3.4's order: labels, subs, align (when those rows are there).
        if let (Some(labels), Some(subs), Some(align)) = (at("labels"), at("subs"), at("align")) {
            assert!(labels < subs && subs < align, "labels {labels}, subs {subs}, align {align}");
        }
    }

    /// One rip label of the evidence clone as the Python loaded it at evidence commit 41c5c51 (2026-10-13), with no
    /// OCR text: the SHA-1 of its subs/index.json and sub-*.packets.json (Task 9's evidence_inputs), and per stream
    /// (n, tag, codec, set aside or not read, events, SHA-1 of the lines "repr(start) repr(dur)" joined by "\n").
    struct Recorded {
        slug: &'static str,
        label: &'static str,
        inputs: &'static str,
        streams: &'static [(i64, &'static str, &'static str, &'static str, usize, &'static str)],
    }

    const EVIDENCE: [Recorded; 7] = [
        Recorded {
            slug: "pans-labyrinth-2006-uhd",
            label: "all-tracks",
            inputs: "507ce3a7330c7b7e083abe6598c8ae59c5ed8498",
            streams: &[
                (0, "eng", "HDMV PGS", "not read", 887, "0787fbaa4f8d107794a61c9ca26188801f3dc713"),
                (1, "spa", "HDMV PGS", "not read", 884, "6284d8de87784cc6f7225d7264c79683970c0e17"),
            ],
        },
        Recorded {
            slug: "pans-labyrinth-2006-uhd",
            label: "eng-only",
            inputs: "0205f792c464e6556264dc284616c903f5d7c8fc",
            streams: &[
                (0, "eng", "HDMV PGS", "not read", 887, "0787fbaa4f8d107794a61c9ca26188801f3dc713"),
            ],
        },
        Recorded {
            slug: "short-circuit-2-1988-dvd",
            label: "all-tracks-2",
            inputs: "ae3380d877ba49f9caa699584311b9d884518579",
            streams: &[
                (0, "eng", "SubRip/SRT", "set aside", 1671, "1bac7f302a4aed1ccf80ab05f16513574a271824"),
                (1, "eng", "VobSub", "not read", 1742, "cdbd987a1c18052fa4ad2b30c97efcee94031864"),
                (2, "fre", "VobSub", "not read", 1678, "ac620f772c9ab7d0b5923c67307c7d9f892427e2"),
                (3, "spa", "VobSub", "not read", 1736, "c02a446ce1f3e364b5e7a7e33c05e8f770714efa"),
                (4, "por", "VobSub", "not read", 1246, "a89866fc8418c48c9f40254f6231107da7a12a95"),
                (5, "chi", "VobSub", "not read", 1774, "dac35fe556d2db5f308e1c9a9f5c46fc855cce69"),
                (6, "kor", "VobSub", "not read", 1774, "b571fdb74b0f9028e3d3c9006bafd4687b2ccd37"),
                (7, "tha", "VobSub", "not read", 1775, "80d96a351b56a61b3e02f1b897b65951581cd5a8"),
            ],
        },
        Recorded {
            slug: "short-circuit-2-1988-dvd",
            label: "eng-only",
            inputs: "862f404d9a99877e8eab951eb714e2a7dc68b9fb",
            streams: &[
                (0, "eng", "SubRip/SRT", "set aside", 1671, "1bac7f302a4aed1ccf80ab05f16513574a271824"),
                (1, "eng", "VobSub", "not read", 1742, "cdbd987a1c18052fa4ad2b30c97efcee94031864"),
            ],
        },
        Recorded {
            slug: "the-office-2005-s3d3-bluray",
            label: "e16-stereo",
            inputs: "995c9e24b9c250ac8e7d99522af8f63ca58b0a76",
            streams: &[
                (0, "eng", "HDMV PGS", "not read", 345, "b71ead71a024abe97a8d2e479a4a846a9626bf1b"),
            ],
        },
        Recorded {
            slug: "the-office-2005-s3d3-bluray",
            label: "rehearsal",
            inputs: "c27dbe6fb2147590c05c62206b90700944d3fc2b",
            streams: &[
                (0, "eng", "HDMV PGS", "not read", 363, "6ea7fed826bf77de75c3e750706a42fb13e19224"),
            ],
        },
        Recorded {
            slug: "the-office-2005-s3d3-bluray",
            label: "t3-all-tracks",
            inputs: "5886e52d1111b1e2a6c653c3f5d4b5ecb6dea473",
            streams: &[
                (0, "eng", "HDMV PGS", "not read", 710, "bbb97bba984bd1717d8419f7861f3f7b66e00c36"),
            ],
        },
    ];

    #[test]
    fn every_evidence_subs_index_loads_as_the_python_loaded_it() {
        let Some(root) = repo_root(Path::new(env!("CARGO_MANIFEST_DIR"))) else {
            skip("no repository root (a Cargo.toml with [workspace]) above crates/dev");
            return;
        };
        let evidence = root.join("corpus").join("owner");
        if !evidence.is_dir() {
            skip("no evidence clone at corpus/owner");
            return;
        }
        let empty = TempDir::new("evidence-no-text");
        let empty_shown = pathlib_split(empty.0.to_str().unwrap()).0;
        let ctx = Ctx::for_test(&root);
        for r in &EVIDENCE {
            let subs = evidence.join(r.slug).join(r.label).join("subs");
            let shown = format!("corpus/owner/{}/{}/subs", r.slug, r.label);
            if !subs.join("index.json").is_file() {
                skip(&format!("{shown}/index.json is not in the evidence clone"));
                continue;
            }
            if sha1_hex(&evidence_inputs(&subs)) != r.inputs {
                skip(&format!("{shown}: its inputs changed since the Python's output was recorded"));
                continue;
            }
            let recording = resolve_recording(&ctx, Some(evidence.to_str().unwrap()), r.slug)
                .unwrap_or_else(|refused| panic!("{}", refused.line));
            let rec = recording.dir.join(r.label);
            let tracks = load_title(&recording.tree, &rec, &MemTree::new("no-text", ""), &PathKey::root(), 64)
                .unwrap_or_else(|error| panic!("{shown}: {error}"));
            let got: Vec<(i64, String, String, &str, usize, String)> = tracks
                .iter()
                .map(|t| {
                    let times: Vec<String> =
                        t.events.iter().map(|e| format!("{} {}", py_float_repr(e.start), py_float_repr(e.dur))).collect();
                    let status = if t.set_aside { "set aside" } else { "not read" };
                    let codec = t.codec.clone().unwrap_or_default();
                    (t.n, t.tag.clone(), codec, status, t.events.len(), sha1_hex(times.join("\n").as_bytes()))
                })
                .collect();
            let want: Vec<(i64, String, String, &str, usize, String)> = r
                .streams
                .iter()
                .map(|&(n, tag, codec, status, events, hash)| {
                    (n, tag.to_string(), codec.to_string(), status, events, hash.to_string())
                })
                .collect();
            assert_eq!(got, want, "{shown}: recorded from the Python at evidence commit 41c5c51");
            // The command over the same label without OCR text: the Python's line and exit 2.
            let args = ["--evidence", evidence.to_str().unwrap(), r.slug, r.label, "--text", empty.0.to_str().unwrap()];
            let (code, out, err) = run_in(&root, &args);
            assert_eq!((code, out.as_str()), (2, ""), "{shown}: {err}");
            assert_eq!(err, format!("no sub-<n>.ocr.jsonl in {empty_shown} for the streams of {shown}/index.json\n"));
        }
    }
}
