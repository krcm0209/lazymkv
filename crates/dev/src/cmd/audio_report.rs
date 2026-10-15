//! `lazymkv-dev audio-report <folder> ...`: the measures and the verdict of every candidate of each level folder
//! written by audio-levels.ps1, line for line as `audio_similarity.py <folder> ...` printed them. Every folder is
//! checked before anything is printed. Exit 0; 1 a refusal, or no folder named (the Python's sys.exit); 2 a file
//! that could not be read once the folders passed the check (where the Python crashed).
//! Py: audio_similarity.main (the folder form), report.

use std::io::Write;
use std::path::Path;

use lazymkv_core::audio::levels::refusal;
use lazymkv_core::audio::report::report;
use lazymkv_core::recording::{MemTree, PathKey, Tree};

use crate::evidence::{Ctx, DiskTree};

/// Printed with exit 1 when no folder is named, verbatim.
const USAGE: &str = "usage: lazymkv-dev audio-report <folder> ...";

/// A level folder named on the command line: a tree that holds it below its parent, and its key there.
struct Folder {
    tree: Box<dyn Tree>,
    key: PathKey,
}

/// `str(Path(arg).parent)` and `Path(arg).name`: separators collapsed, "." components dropped, ".." kept, "." for
/// no parent ("/" for the root of an absolute path).
fn parent_and_name(arg: &str) -> (String, String) {
    let absolute = arg.starts_with(std::path::is_separator);
    let parts: Vec<&str> = arg.split(std::path::is_separator).filter(|p| !p.is_empty() && *p != ".").collect();
    let (name, parent) = match parts.split_last() {
        Some((last, rest)) => (last.to_string(), rest.join("/")),
        None => (String::new(), String::new()),
    };
    let parent = match (absolute, parent.is_empty()) {
        (true, true) => "/".to_string(),
        (true, false) => format!("/{parent}"),
        (false, true) => ".".to_string(),
        (false, false) => parent,
    };
    (parent, name)
}

/// The tree for one folder argument. A folder that is not there (or not a folder) gets an empty tree printed as
/// given. Otherwise the tree is rooted at the folder's resolved parent and printed as the argument's parent; when
/// the argument's last component is not the folder's own name ("." or "..", or a link to a folder of another name),
/// the resolved parent is printed instead (a deviation for such arguments only: no golden uses one).
fn open_folder(ctx: &Ctx, arg: &str) -> Result<Folder, String> {
    let (parent_shown, name) = parent_and_name(arg);
    let path = ctx.abs(arg);
    if !path.is_dir() {
        return Ok(Folder { tree: Box::new(MemTree::new(&parent_shown, "")), key: PathKey::new(&name) });
    }
    let resolved = std::fs::canonicalize(&path).map_err(|e| format!("{arg}: {e}"))?;
    let resolved_name = resolved.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let parent = resolved.parent().map(Path::to_path_buf).unwrap_or_else(|| resolved.clone());
    let (shown, key) =
        if name == resolved_name { (parent_shown, name) } else { (parent.display().to_string(), resolved_name) };
    let tree = DiskTree::open(&parent, &shown).map_err(|e| format!("{}: {e}", parent.display()))?;
    Ok(Folder { tree: Box::new(tree), key: PathKey::new(&key) })
}

pub fn run(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    if args.is_empty() {
        let _ = writeln!(err, "{USAGE}");
        return 1;
    }
    let mut folders = Vec::new();
    for arg in args {
        match open_folder(ctx, arg) {
            Ok(folder) => folders.push(folder),
            Err(message) => {
                let _ = writeln!(err, "{message}");
                return 2;
            }
        }
    }
    // every folder is checked before anything is printed
    for folder in &folders {
        match refusal(&*folder.tree, &folder.key) {
            Ok(why) if why.is_empty() => {}
            Ok(why) => {
                let _ = writeln!(err, "{why}");
                return 1;
            }
            Err(message) => {
                let _ = writeln!(err, "{message}");
                return 2;
            }
        }
    }
    for folder in &folders {
        match report(&*folder.tree, &folder.key) {
            Ok(lines) => {
                for line in lines {
                    let _ = writeln!(out, "{line}");
                }
                let _ = out.flush();
            }
            Err(message) => {
                let _ = writeln!(err, "{message}");
                return 2;
            }
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use lazymkv_core::testsupport::{commentary, level_file_text, programme, raw_file_text};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// The Python's refusal of a folder named "nope" that does not exist.
    const NOPE_REFUSAL: &str = "nope: no manifest.txt, so it was not written by the new audio-levels.ps1 (the first measurement's folders, audio-levels-<main>-<cand>, hold 100 ms full-band levels); this folder cannot be analysed: measure the title again with the new audio-levels.ps1, which writes audio-levels-20ms-<main>-<candidates>";

    /// The Python's `audio_similarity.py lv/audio-levels-20ms-1-3/` for a duplicate folder (MIX[:2000] twice).
    const LV_DUPLICATE: &[&str] = &[
        "lv/audio-levels-20ms-1-3 (x.mkv, measured in 1 s)",
        "  main stream 1: dts 6 ch 5.1(side) eng; dialogue reference: its mono mixdown (no centre channel); 2000 frames (0.7 min)",
        "  veto: not checked: the folder has no streams.json (row 0, D21)",
        "  stream 3: ac3 2 ch stereo eng",
        "    verdict: duplicate of stream 1 (exact duplicate (same packets): keep the first copy, drop this one; no audio analysis needed)",
        "    row: | lv/x.mkv | 3 | same packets as stream 1 | duplicate |",
    ];

    /// MANIFEST of test_audio_similarity.py: main stream 1 (DTS-HD MA 5.1, English) and candidate 3 (AC-3 stereo).
    fn manifest(fc: &str, main: &str, cand: &str) -> String {
        let mut text = String::from("# lazymkv audio-levels v2 (experiment 13)\n");
        text.push_str("file=x.mkv bytes=1 seconds=1 frame_ms=20 band_hz=300-3400\n");
        text.push_str(&format!(
            "main=1 codec=dts profile=DTS-HD_MA channels=6 layout=5.1(side) language=eng fc={fc} frames=1 md5={main}\n"
        ));
        text.push_str(&format!("cand=3 codec=ac3 profile=- channels=2 layout=stereo language=eng frames=1 md5={cand}\n"));
        text
    }

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

    /// <tmp>/lv/audio-levels-20ms-1-3: MIX[:2000] as main and as candidate 3 with one md5 (nothing to measure).
    fn duplicate_folder(tmp: &Path) {
        let (mix, _) = programme(420, 1);
        let d = tmp.join("lv").join("audio-levels-20ms-1-3");
        fs::create_dir_all(&d).unwrap();
        for name in ["main-mix.txt", "cand-3.txt"] {
            fs::write(d.join(name), level_file_text(&mix[..2000])).unwrap();
        }
        fs::write(d.join("manifest.txt"), manifest("no", "43ae", "43ae")).unwrap();
    }

    #[test]
    fn a_folder_written_by_the_new_script_is_analysed() {
        let tmp = Scratch::new("audio-report");
        let d = tmp.0.join("audio-levels-20ms-1-3");
        fs::create_dir(&d).unwrap();
        let (mix, centre) = programme(420, 1);
        fs::write(d.join("main-mix.txt"), level_file_text(&mix)).unwrap();
        fs::write(d.join("main-fc.txt"), level_file_text(&centre)).unwrap();
        // raw ametadata output at 20 ms is read as well
        fs::write(d.join("cand-3.txt"), raw_file_text(&commentary(&mix, 7), 0.02)).unwrap();
        fs::write(d.join("manifest.txt"), manifest("yes", "aa", "bb")).unwrap();
        let tree = DiskTree::open(&tmp.0, &tmp.0.display().to_string()).unwrap();
        assert_eq!(refusal(&tree, &PathKey::new("audio-levels-20ms-1-3")).unwrap(), "");
        let shown = d.display().to_string();
        let (code, out, err) = call(&Ctx::for_test(&tmp.0), &[shown.as_str()]);
        assert_eq!(code, 0, "{err}");
        assert!(out.contains("dialogue reference: its centre channel"), "{out}");
        assert!(out.contains("verdict: commentary"), "{out}");
    }

    #[test]
    fn a_folder_of_the_first_measurement_is_refused_before_any_number() {
        let tmp = Scratch::new("audio-report");
        let d = tmp.0.join("audio-levels-1-3"); // 100 ms frames, full band, no manifest
        fs::create_dir(&d).unwrap();
        let (mix, _) = programme(420, 1);
        let coarse: Vec<f64> = mix[..3000].iter().step_by(5).copied().collect(); // MIX[:3000:5]
        for name in ["main-mix.txt", "main-fc.txt", "cand-mix.txt", "cand-mid.txt", "cand-side.txt"] {
            fs::write(d.join(name), raw_file_text(&coarse, 0.1)).unwrap();
        }
        let shown = d.display().to_string();
        let (code, out, err) = call(&Ctx::for_test(&tmp.0), &[shown.as_str()]);
        assert_eq!(code, 1);
        assert!(err.contains("no manifest.txt"), "{err}");
        assert!(err.contains("measure the title again"), "{err}");
        assert_eq!(out, "");
        let tree = DiskTree::open(&tmp.0, &tmp.0.display().to_string()).unwrap();
        assert!(report(&tree, &PathKey::new("audio-levels-1-3")).is_err());
    }

    #[test]
    fn every_folder_is_checked_before_anything_is_printed() {
        let tmp = Scratch::new("audio-report");
        duplicate_folder(&tmp.0);
        let (code, out, err) = call(&Ctx::for_test(&tmp.0), &["lv/audio-levels-20ms-1-3", "nope"]);
        assert_eq!((code, out.as_str()), (1, ""));
        assert_eq!(err, format!("{NOPE_REFUSAL}\n"));
    }

    #[test]
    fn the_report_prints_the_folder_as_given_and_labels_it_by_its_parent() {
        let tmp = Scratch::new("audio-report");
        duplicate_folder(&tmp.0);
        // a trailing separator, as a shell completes a folder name, is dropped as pathlib drops it
        let (code, out, err) = call(&Ctx::for_test(&tmp.0), &["lv/audio-levels-20ms-1-3/"]);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(out, LV_DUPLICATE.join("\n") + "\n");
    }

    #[test]
    fn no_folder_prints_the_usage_and_exits_1() {
        let tmp = Scratch::new("audio-report");
        let (code, out, err) = call(&Ctx::for_test(&tmp.0), &[]);
        assert_eq!((code, out.as_str()), (1, ""));
        assert_eq!(err, "usage: lazymkv-dev audio-report <folder> ...\n");
    }

    #[test]
    fn audio_report_is_reached_through_its_row_of_the_command_table() {
        let row = crate::COMMANDS
            .iter()
            .find(|(name, _, _)| *name == "audio-report")
            .expect("an audio-report row in COMMANDS");
        assert_eq!(row.2, "<folder> ...");
        let tmp = Scratch::new("audio-report");
        let argv = vec!["audio-report".to_string()];
        let (mut out, mut err): (Vec<u8>, Vec<u8>) = (Vec::new(), Vec::new());
        assert_eq!(crate::dispatch(&Ctx::for_test(&tmp.0), &argv, &mut out, &mut err), 1);
        assert!(out.is_empty());
        assert_eq!(String::from_utf8(err).unwrap(), "usage: lazymkv-dev audio-report <folder> ...\n");
    }

    #[test]
    fn a_folder_argument_is_split_as_pathlib_prints_it() {
        let cases = [
            ("lv/x/", "lv", "x"),
            ("lv/x", "lv", "x"),
            ("x", ".", "x"),
            ("./x", ".", "x"),
            ("a//b", "a", "b"),
            ("a/../b", "a/..", "b"),
            ("/a/b", "/a", "b"),
            ("/a", "/", "a"),
            (".", ".", ""),
        ];
        for (arg, parent, name) in cases {
            assert_eq!(parent_and_name(arg), (parent.to_string(), name.to_string()), "{arg}");
        }
    }
}
