//! `lazymkv-dev levels-args [--evidence DIR] <slug|dir> <label>`: the audio-levels.ps1 job lines that add a ripped
//! title to the tuning dataset (EXP-13 addendum 9.1): its programme reference against every other stream in a wanted
//! language, a dataset-only job per unwanted language with two or more streams, the notes. A slug is read under the
//! evidence clone, a directory as given. Exit 0; 2 a usage error or a title that cannot be measured as it is (no
//! probe, no audio stream, a file that cannot be read); 3 a slug without the evidence clone.
//! Py: audio_dataset.py args.

use std::io::Write;

use lazymkv_core::audio::dataset::levels_args;

use crate::evidence::{resolve_recording, Ctx, Refused};
use crate::{usage_error, Args};

/// The synopsis after "usage: lazymkv-dev ".
const USAGE: &str = "levels-args [--evidence DIR] <slug|dir> <label>";

pub fn run(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let mut parsed = Args::new(args, &["--evidence"]);
    let evidence = match parsed.flag("--evidence") {
        Ok(value) => value,
        Err(message) => return usage_error(err, "levels-args", USAGE, &message),
    };
    let words = match parsed.positionals() {
        Ok(words) => words,
        Err(message) => return usage_error(err, "levels-args", USAGE, &message),
    };
    match words.len() {
        0 => return usage_error(err, "levels-args", USAGE, "the following arguments are required: slug|dir, label"),
        1 => return usage_error(err, "levels-args", USAGE, "the following arguments are required: label"),
        2 => {}
        _ => {
            let extra = format!("unrecognized arguments: {}", words[2..].join(" "));
            return usage_error(err, "levels-args", USAGE, &extra);
        }
    }
    let rec = match resolve_recording(ctx, evidence.as_deref(), &words[0]) {
        Ok(rec) => rec,
        Err(Refused { code, line }) => {
            let _ = writeln!(err, "{line}");
            return code;
        }
    };
    match levels_args(&rec.tree, &rec.dir, &words[1]) {
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use serde_json::json;

    use super::run;
    use crate::evidence::Ctx;
    use crate::testutil::{self, Scratch as TempDir};

    /// What `audio_dataset.py args some-film commentary` printed, tmdb.json saying "es".
    const COMMENTARY: &str = concat!(
        "some-film/commentary: 3 audio streams (from ffprobe-streams.json); original language spa (TMDb es)\n",
        "  stream 1: dts DTS-HD MA 6 ch lang=spa\n",
        "  stream 2: dts DTS 6 ch lang=spa\n",
        "  stream 3: ac3 2 ch lang=eng\n",
        "  programme reference: stream 1 (dts DTS-HD MA 6 ch), placed first in the original language (spec 8.6 steps 3 and 4); candidates: every other stream in eng, spa or with no language, whatever its language\n",
        "scripts/phase0/job.sh audio-levels.ps1 audio-levels-some-film-commentary -Slug some-film -Label commentary -Main 1 -Cand 2,3\n",
        "scripts/phase0/wait.sh audio-levels-some-film-commentary\n",
    );

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    /// The Python test's probe_file(): video, then the given audio streams (codec, profile, channels, language).
    fn probe_file(path: &Path, streams: &[(&str, Option<&str>, i64, &str)]) {
        let mut list = vec![json!({"index": 0, "codec_type": "video"})];
        for (k, (codec, profile, channels, language)) in streams.iter().enumerate() {
            list.push(json!({"index": k + 1, "codec_type": "audio", "codec_name": codec, "profile": profile,
                             "channels": channels, "tags": {"language": language}}));
        }
        write(path, &serde_json::to_string(&json!({"streams": list})).unwrap());
    }

    /// Runs the command with `dir` as the current directory: (exit code, stdout, stderr).
    fn call(dir: &Path, args: &[&str]) -> (i32, String, String) {
        testutil::call(run, &Ctx::for_test(dir), args)
    }

    #[test]
    fn prints_the_job_lines_of_a_recording_directory() {
        let tmp = TempDir::new("lines");
        probe_file(&tmp.0.join("some-film/commentary/ffprobe-streams.json"),
                   &[("dts", Some("DTS-HD MA"), 6, "spa"), ("dts", Some("DTS"), 6, "spa"), ("ac3", None, 2, "eng")]);
        write(&tmp.0.join("some-film/tmdb.json"), r#"{"original_language": "es"}"#);
        let (code, out, err) = call(&tmp.0, &["some-film", "commentary"]);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(out, COMMENTARY);
    }

    #[test]
    fn a_rip_without_a_probe_exits_2_with_the_pythons_message() {
        let tmp = TempDir::new("no-probe");
        write(&tmp.0.join("some-film/share-drop/rip-summary.json"), r#"{"ok": true, "title": 5, "bytes": 300}"#);
        let (code, out, err) = call(&tmp.0, &["some-film", "share-drop"]);
        assert_eq!((code, out.as_str()), (2, ""));
        assert!(err.ends_with(concat!(
            "share-drop: no ffprobe-streams.json or mkvmerge.json, and no other rip of the same title with the same size has one. Probe the rip first, then run args again:\n",
            "scripts/phase0/job.sh probe-title.ps1 probe-share-drop -Slug some-film -Label share-drop -SkipRemux\n",
            "scripts/phase0/wait.sh probe-share-drop\n",
        )), "{err}");
        probe_file(&tmp.0.join("some-film/silent/ffprobe-streams.json"), &[]);
        let (code, _, err) = call(&tmp.0, &["some-film", "silent"]);
        assert_eq!(code, 2);
        assert!(err.ends_with("silent: the file has no audio stream\n"), "{err}");
    }

    #[test]
    fn a_slug_without_the_evidence_clone_exits_3() {
        let tmp = TempDir::new("no-clone");
        let missing = tmp.0.join("no-evidence").display().to_string();
        let (code, out, err) = call(&tmp.0, &["--evidence", missing.as_str(), "some-slug", "all-tracks"]);
        assert_eq!((code, out.as_str()), (3, ""));
        assert!(err.contains("no such folder; the evidence repository lazymkv-evidence is not cloned there"), "{err}");
    }

    #[test]
    fn a_wrong_argument_count_is_a_usage_error() {
        let tmp = TempDir::new("usage");
        for (args, message) in [
            (&[][..], "the following arguments are required: slug|dir, label"),
            (&["some-film"][..], "the following arguments are required: label"),
            (&["some-film", "all-tracks", "extra"][..], "unrecognized arguments: extra"),
        ] {
            let (code, out, err) = call(&tmp.0, args);
            assert_eq!((code, out.as_str()), (2, ""), "{args:?}");
            assert!(err.starts_with("usage: lazymkv-dev levels-args [--evidence DIR] <slug|dir> <label>\n"), "{err}");
            assert!(err.contains(&format!("lazymkv-dev levels-args: error: {message}")), "{err}");
        }
    }

    #[test]
    fn levels_args_is_in_the_command_table_between_reconcile_and_titles() {
        let names: Vec<&str> = crate::COMMANDS.iter().map(|(name, _, _)| *name).collect();
        let at = |name: &str| names.iter().position(|n| *n == name).unwrap_or_else(|| panic!("{name} in {names:?}"));
        assert!(at("reconcile") < at("levels-args") && at("levels-args") < at("titles"), "{names:?}");
    }
}
