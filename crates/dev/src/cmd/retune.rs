//! `lazymkv-dev retune [--evidence DIR[=TEXTDIR] ...] [--set NAME=VALUE ...] [--snapshot FILE] [--compare FILE]
//! [--spec FILE] [--allow-unmeasured]`: the tuning harness of the audio rule over the evidence clone (spec 3.4; spec
//! 15's `retune` until lazymkvctl exists). Exit 0; 1 a FAIL; 2 an argument, the spec's row 13, a --set value, a labels
//! or fitted file, or a --compare file that is not a snapshot; 3 no evidence clone, or something could not be checked
//! and --allow-unmeasured was not given. Py: audio_dataset.main (its positional corpus path is --evidence).
//!
//! `lazymkv-dev retune --subtitles [--evidence DIR] [--spec FILE] [--text DIR]` (RP15, phase 1 Task 18) runs the
//! subtitle harness instead (`lazymkv_core::subtitle::labels::harness`, spec 15 "Rules against the stored evidence"):
//! every labelled English or no-language subtitle track of the evidence, judged by 8.7 and by the text rule over the
//! OCR text of --text DIR (else LAZYMKV_OCR_TEXT), counted against spec section 17 row 14 read from the spec, with
//! spec 14's default settings. Exit 0; 1 a costly verdict; 2 an argument, the spec's row 14, a subtitle labels file
//! that does not validate, or an OCR text folder that is not there; 3 no evidence clone, no subtitle label, or a
//! labelled track that could not be judged.
//!
//! `--evidence` may be given more than once, each as `DIR` or `DIR=TEXTDIR`, split at the first `=` (design part 7.7;
//! notes D1, D2, call 12). One `--evidence` reads as before, its TEXTDIR half standing for `--text` with `--subtitles`
//! (and `--text` then refused beside it) and ignored without it. Two or more print one section per folder in flag
//! order, each headed `evidence folder <k> of <m>: <DIR>`; they write no snapshot (`--snapshot`, `--compare` and
//! `--text` exit 2), read each folder's own `audio-dataset.json` for the changes since it, report a folder without
//! labels as `no labelled track`, and exit with the worst code over the folders with labels, 3 above 1 above 0, and 3
//! when no folder has labels; a labels file that does not validate exits 2 at once.
//!
//! Before either harness reads the evidence, each recording folder under it (every child folder but `.git`, in name
//! order) passes the refetch gate of `crate::stale`, which prints each copy it fetched again on stderr: exit 3 when a
//! TMDb copy of the folder is more than six months old and cannot be fetched again (TMDB_TOKEN unset, or the fetch
//! failed; design part 8.4, K15); an engine job folder is read as it is. A job folder whose TMDb copy is stale or
//! missing is left out and named unmeasured on stderr, in every run (Ruling T5-R40).

use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::Value;

use lazymkv_core::audio::dataset::{
    destination, harness, parse_sets, read_snapshot, result_line, snapshot_line, why_phrase, Destination,
    HarnessError, HarnessInput, PreviousSnapshot, BASELINE_FILE, SNAPSHOT_FORMAT, SNAPSHOT_FORMAT_V2, TRIAL_FILE,
    UNCHECKED_FILE,
};
use lazymkv_core::audio::dataset::Overrides;
use lazymkv_core::audio::labels::{exp13_requirements, Requirements};
use lazymkv_core::recording::{decode_text, Encoding, Errors, PathKey, Tree};
use lazymkv_core::settings::Settings;
use lazymkv_core::subtitle::labels::{exp14_requirements, harness as subtitle_harness, Row14, TEXT_HINT};

use crate::evidence::{
    default_spec, ocr_text_dir, os_error_text, resolve_evidence, Ctx, DiskTree, Refused, EVIDENCE_SHOWN,
};
use crate::pyjson::{dumps, write_snapshot, Style};
use crate::stale::{fetched_line, refresh_copies, stale_job_folders, unmeasured_line};
use crate::{usage_error, Args};

/// The synopsis after "usage: lazymkv-dev ".
const USAGE: &str = "retune [--evidence DIR[=TEXTDIR] ...] [--set NAME=VALUE] [--snapshot FILE] [--compare FILE] [--spec FILE] [--allow-unmeasured] [--subtitles] [--text DIR]";
const FLAGS: [&str; 8] =
    ["--evidence", "--set", "--snapshot", "--compare", "--spec", "--allow-unmeasured", "--subtitles", "--text"];
/// --subtitles runs the subtitle harness, which takes none of the audio harness's flags (RP15).
const SUBTITLES_ALONE: &str =
    "argument --subtitles: not allowed with --set, --snapshot, --compare or --allow-unmeasured";
/// --text names the OCR text, which only the subtitle harness reads.
const TEXT_NEEDS_SUBTITLES: &str = "argument --text: only with --subtitles";

/// One `--evidence` value (design part 7.7; notes D1, call 12): the evidence folder and, in the form `DIR=TEXTDIR`,
/// the OCR text folder `--subtitles` reads for it (Ruling T5-R53). Def: T5-21. Use: T5-21.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EvidenceArg {
    /// The evidence folder, as given.
    pub(crate) dir: String,
    /// The OCR text folder after the first `=`, as given; None without one.
    pub(crate) text: Option<String>,
}

/// `value` split at its first `=` into the evidence folder and the OCR text folder (design part 7.7; call 12); a
/// value without `=` is the folder alone, its text None. Def: T5-21. Use: T5-21.
pub(crate) fn split_evidence(value: &str) -> EvidenceArg {
    match value.split_once('=') {
        Some((dir, text)) => EvidenceArg { dir: dir.to_string(), text: Some(text.to_string()) },
        None => EvidenceArg { dir: value.to_string(), text: None },
    }
}

/// The exit code of a several-folder run over the codes of its folders with labels (design part 7.7; notes D1, D2):
/// 3 when any is 3, else 1 when any is 1, else 0; 3 for an empty list, a run in which no folder has labels.
/// Def: T5-21. Use: T5-21.
pub(crate) fn worst(codes: &[i32]) -> i32 {
    if codes.is_empty() || codes.contains(&3) {
        3
    } else if codes.contains(&1) {
        1
    } else {
        0
    }
}

/// The section body of a folder with no labels file of the harness run (design part 7.7; D2). Def: T5-21. Use: T5-21.
pub const NO_LABELLED_TRACK: &str = "no labelled track";

/// The argument error of a flag a several-folder run does not take (design part 7.7; D1; Ruling T5-R29): `argument
/// <flag>: only with a single --evidence`, and for `--text` that text and `; give DIR=TEXTDIR`. Def: T5-21.
/// Use: T5-21.
pub(crate) fn single_only(flag: &str) -> String {
    let line = format!("argument {flag}: only with a single --evidence");
    if flag == "--text" {
        format!("{line}; give DIR=TEXTDIR")
    } else {
        line
    }
}

/// `--text` beside an `--evidence DIR=TEXTDIR`, refused before the single-folder rule (design part 7.7; call 12;
/// Ruling T5-R53). Def: T5-21. Use: T5-21, T5-22.
pub(crate) const TEXT_WITH_TEXTDIR: &str =
    "argument --text: not with --evidence DIR=TEXTDIR, which names the text already";

/// The header of folder `k` of `m` in a several-folder run (design part 7.7, D1): `evidence folder <k> of <m>:
/// <shown>`, `shown` the folder's DIR half as given. Def: T5-21. Use: T5-21.
pub(crate) fn section_header(k: usize, m: usize, shown: &str) -> String {
    format!("evidence folder {k} of {m}: {shown}")
}

pub fn run(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let mut parsed = Args::new(args, &FLAGS);
    let allow_unmeasured = parsed.switch("--allow-unmeasured");
    let subtitles = parsed.switch("--subtitles");
    let text_flag = match parsed.flag("--text") {
        Ok(value) => value,
        Err(message) => return usage_error(err, "retune", USAGE, &message),
    };
    let sets = match parsed.flags("--set") {
        Ok(values) => values,
        Err(message) => return usage_error(err, "retune", USAGE, &message),
    };
    let evidence_args: Vec<EvidenceArg> = match parsed.flags("--evidence") {
        Ok(values) => values.iter().map(|value| split_evidence(value)).collect(),
        Err(message) => return usage_error(err, "retune", USAGE, &message),
    };
    let several = evidence_args.len() > 1;
    let snapshot_flag = match parsed.flag("--snapshot") {
        Ok(value) => value,
        Err(message) => return usage_error(err, "retune", USAGE, &message),
    };
    let compare_flag = match parsed.flag("--compare") {
        Ok(value) => value,
        Err(message) => return usage_error(err, "retune", USAGE, &message),
    };
    let spec_flag = match parsed.flag("--spec") {
        Ok(value) => value,
        Err(message) => return usage_error(err, "retune", USAGE, &message),
    };
    let words = match parsed.positionals() {
        Ok(words) => words,
        Err(message) => return usage_error(err, "retune", USAGE, &message),
    };
    if !words.is_empty() {
        return usage_error(err, "retune", USAGE, &format!("unrecognized arguments: {}", words.join(" ")));
    }
    let (spec_path, spec_shown) = match spec_file(ctx, spec_flag) {
        Some(found) => found,
        None => return usage_error(err, "retune", USAGE, "the following arguments are required: --spec"),
    };
    if subtitles {
        if !sets.is_empty() || snapshot_flag.is_some() || compare_flag.is_some() || allow_unmeasured {
            return usage_error(err, "retune", USAGE, SUBTITLES_ALONE);
        }
        if text_flag.is_some() {
            if evidence_args.iter().any(|arg| arg.text.is_some()) {
                return usage_error(err, "retune", USAGE, TEXT_WITH_TEXTDIR);
            }
            if several {
                return usage_error(err, "retune", USAGE, &single_only("--text"));
            }
        }
        if several {
            return run_subtitles_several(ctx, &evidence_args, &spec_path, &spec_shown, out, err);
        }
        return run_subtitles(ctx, evidence_args.first(), &spec_path, &spec_shown, text_flag.as_deref(), out, err);
    }
    if text_flag.is_some() {
        return usage_error(err, "retune", USAGE, TEXT_NEEDS_SUBTITLES);
    }
    if several {
        if snapshot_flag.is_some() {
            return usage_error(err, "retune", USAGE, &single_only("--snapshot"));
        }
        if compare_flag.is_some() {
            return usage_error(err, "retune", USAGE, &single_only("--compare"));
        }
    }
    let req = match read_text_file(&spec_path, &spec_shown, Encoding::Utf8Sig).and_then(|text| exp13_requirements(&text)) {
        Ok(req) => req,
        Err(message) => return fail(err, &message, 2),
    };
    let overrides = match parse_sets(&sets) {
        Ok(overrides) => overrides,
        Err(message) => return fail(err, &message, 2),
    };
    if several {
        return run_several(ctx, &evidence_args, &req, &overrides, allow_unmeasured, out, err);
    }
    let evidence = match resolve_evidence(ctx, evidence_args.first().map(|arg| arg.dir.as_str())) {
        Ok(dir) => dir,
        Err(Refused { code, line }) => return fail(err, &line, code),
    };
    if let Err(code) = refresh_evidence(ctx, &evidence, EVIDENCE_SHOWN, err) {
        return code;
    }
    let left_out = unmeasured_job_folders(ctx, &evidence, EVIDENCE_SHOWN, err);
    let compare = match &compare_flag {
        None => None,
        Some(given) => match read_snapshot_file(&ctx.abs(given)) {
            Some(snapshot) => Some((given.clone(), snapshot)),
            None => {
                let line = format!(
                    "{given}: not a snapshot written by lazymkv-dev retune ({SNAPSHOT_FORMAT}) or by audio_dataset.py ({SNAPSHOT_FORMAT_V2})"
                );
                return fail(err, &line, 2);
            }
        },
    };
    let tree = match DiskTree::open_without(&evidence, EVIDENCE_SHOWN, &left_out) {
        Ok(tree) => tree,
        Err(e) => return fail(err, &os_error_text(&e, &evidence.display().to_string()), 3),
    };
    let previous = read_snapshot_file(&evidence.join(BASELINE_FILE));
    let created = ctx.now();
    let input = HarnessInput {
        tree: &tree,
        req: &req,
        overrides: &overrides,
        previous: previous.as_ref(),
        compare: compare.as_ref().map(|(given, snapshot)| (given.as_str(), snapshot)),
        use_cache: true,
        created: &created,
    };
    let h = match harness(&input) {
        Ok(h) => h,
        Err(HarnessError::Labels(lines)) => {
            for line in lines {
                let _ = writeln!(out, "{line}");
            }
            return 2;
        }
    };
    let baseline_shown = format!("{EVIDENCE_SHOWN}/{BASELINE_FILE}");
    let dest = destination(snapshot_flag.is_some(), !overrides.is_empty(), &h.unchecked);
    let (path, shown) = match (dest, &snapshot_flag) {
        (Destination::Given, Some(given)) => (ctx.abs(given), given.clone()),
        (Destination::Trial, _) => {
            let path = ctx.temp_dir().join(TRIAL_FILE);
            let shown = path.display().to_string();
            (path, shown)
        }
        (Destination::Unchecked, _) => {
            let path = ctx.temp_dir().join(UNCHECKED_FILE);
            let shown = path.display().to_string();
            (path, shown)
        }
        _ => (evidence.join(BASELINE_FILE), baseline_shown.clone()),
    };
    let why = why_phrase(dest, &baseline_shown, &h.unchecked);
    let value = match serde_json::to_value(&h.snapshot) {
        Ok(value) => value,
        Err(e) => return fail(err, &e.to_string(), 2),
    };
    let written = match write_unless_unchanged(&path, &value) {
        Ok(written) => written,
        Err(e) => return fail(err, &os_error_text(&e, &shown), 2),
    };
    let (result, code) = result_line(&h.failures, &h.unchecked, allow_unmeasured);
    for line in &h.lines {
        let _ = writeln!(out, "{line}");
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "{}", snapshot_line(&shown, &why, written));
    let _ = writeln!(out, "{result}");
    code
}

/// The audio harness over two or more evidence folders (design part 7.7; notes D1, D2; Rulings T5-R30, T5-R31):
/// every folder resolved, then every folder through the refetch gate, shown by its DIR half as given (the first
/// refusal returns its code); then one section per folder in flag order, each headed by `section_header` after a
/// blank line (none before the first), reading that folder's own baseline for the changes since it. A folder with no
/// `*/audio-labels.json` reads `NO_LABELLED_TRACK` and gives no code; labels that do not validate print their lines
/// and return 2 at once. Nothing is written; the exit code is `worst` over the folders' codes.
fn run_several(
    ctx: &Ctx,
    args: &[EvidenceArg],
    req: &Requirements,
    overrides: &Overrides,
    allow_unmeasured: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let folders = match resolve_all(ctx, args, err) {
        Ok(folders) => folders,
        Err(code) => return code,
    };
    let created = ctx.now();
    let mut codes = Vec::new();
    for (k, (arg, folder)) in args.iter().zip(&folders).enumerate() {
        let tree = match open_section(ctx, k, args.len(), arg, folder, out, err) {
            Ok(tree) => tree,
            Err(code) => return code,
        };
        if tree.glob(&PathKey::root(), "*/audio-labels.json").is_empty() {
            let _ = writeln!(out, "{NO_LABELLED_TRACK}");
            continue;
        }
        let previous = read_snapshot_file(&folder.join(BASELINE_FILE));
        let input = HarnessInput {
            tree: &tree,
            req,
            overrides,
            previous: previous.as_ref(),
            compare: None,
            use_cache: true,
            created: &created,
        };
        let h = match harness(&input) {
            Ok(h) => h,
            Err(HarnessError::Labels(lines)) => {
                for line in lines {
                    let _ = writeln!(out, "{line}");
                }
                return 2;
            }
        };
        let (result, code) = result_line(&h.failures, &h.unchecked, allow_unmeasured);
        for line in &h.lines {
            let _ = writeln!(out, "{line}");
        }
        let _ = writeln!(out);
        let _ = writeln!(out, "{result}");
        codes.push(code);
    }
    worst(&codes)
}

/// `retune --subtitles` over two or more evidence folders (design part 7.7; notes D1, D2; Ruling T5-R51): row 14 from
/// the spec (exit 2 when it cannot be read), every folder resolved and through the refetch gate as in `run_several`,
/// every TEXTDIR half checked (exit 2 for a folder that is not there; a folder without one has no OCR text, and
/// LAZYMKV_OCR_TEXT is not read), then one section per folder: `NO_LABELLED_TRACK` for a folder with no
/// `*/subtitle-labels.json`, else the subtitle harness's report, its exit code 2 returned at once. The exit code is
/// `worst` over the folders' codes.
fn run_subtitles_several(
    ctx: &Ctx,
    args: &[EvidenceArg],
    spec_path: &Path,
    spec_shown: &str,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let row14 = match read_row14(spec_path, spec_shown) {
        Ok(row14) => row14,
        Err(message) => return fail(err, &message, 2),
    };
    let folders = match resolve_all(ctx, args, err) {
        Ok(folders) => folders,
        Err(code) => return code,
    };
    let mut texts = Vec::new();
    for arg in args {
        match open_text(arg.text.as_ref().map(|text| ctx.abs(text))) {
            Ok(text) => texts.push(text),
            Err(line) => return fail(err, &line, 2),
        }
    }
    let mut codes = Vec::new();
    for (k, ((arg, folder), text)) in args.iter().zip(&folders).zip(&texts).enumerate() {
        let tree = match open_section(ctx, k, args.len(), arg, folder, out, err) {
            Ok(tree) => tree,
            Err(code) => return code,
        };
        if tree.glob(&PathKey::root(), "*/subtitle-labels.json").is_empty() {
            let _ = writeln!(out, "{NO_LABELLED_TRACK}");
            continue;
        }
        let h = subtitle_harness(&tree, text.as_ref().map(|tree| tree as &dyn Tree), &row14, &Settings::default());
        for line in &h.lines {
            let _ = writeln!(out, "{line}");
        }
        if h.exit_code == 2 {
            return 2;
        }
        codes.push(h.exit_code);
    }
    worst(&codes)
}

/// Every DIR half through `resolve_evidence`, then every folder through the refetch gate, each in flag order and shown
/// by its DIR half as given (design part 7.7; R5-code-63); Err holds the first refusal's code, after its line.
fn resolve_all(ctx: &Ctx, args: &[EvidenceArg], err: &mut dyn Write) -> Result<Vec<PathBuf>, i32> {
    let mut folders = Vec::new();
    for arg in args {
        match resolve_evidence(ctx, Some(&arg.dir)) {
            Ok(dir) => folders.push(dir),
            Err(Refused { code, line }) => return Err(fail(err, &line, code)),
        }
    }
    for (arg, folder) in args.iter().zip(&folders) {
        refresh_evidence(ctx, folder, &arg.dir, err)?;
    }
    Ok(folders)
}

/// Folder `k` (from 0) of `m` in a several-folder run: its unmeasured job folders on stderr, a blank line before every
/// section but the first, `section_header` on stdout, then the folder's tree printed as `EVIDENCE_SHOWN`, the job
/// folders left out; Err(3) after the OS error's text when it cannot be read.
fn open_section(
    ctx: &Ctx,
    k: usize,
    m: usize,
    arg: &EvidenceArg,
    folder: &Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<DiskTree, i32> {
    let left_out = unmeasured_job_folders(ctx, folder, &arg.dir, err);
    if k > 0 {
        let _ = writeln!(out);
    }
    let _ = writeln!(out, "{}", section_header(k + 1, m, &arg.dir));
    DiskTree::open_without(folder, EVIDENCE_SHOWN, &left_out)
        .map_err(|e| fail(err, &os_error_text(&e, &folder.display().to_string()), 3))
}

/// The job folders of `evidence` left out of the reading (design part 7.7; Ruling T5-R40, call 25): each that
/// `stale_job_folders` names printed to stderr as `unmeasured_line`, shown `<shown_root>/<name>`; their names.
fn unmeasured_job_folders(ctx: &Ctx, evidence: &Path, shown_root: &str, err: &mut dyn Write) -> Vec<String> {
    let mut names = Vec::new();
    for (name, copy) in stale_job_folders(ctx, evidence) {
        let _ = writeln!(err, "{}", unmeasured_line(&format!("{shown_root}/{name}"), &copy));
        names.push(name);
    }
    names
}

/// The refetch gate over the evidence (design part 8.4, K15; Ruling T4-R10): `refresh_copies` over each child folder
/// of `evidence` but `.git`, in name order, shown as `<shown_root>/<name>` (`shown_root` is `EVIDENCE_SHOWN` in a
/// single-folder run, the DIR half as given in a several-folder one; design part 7.7, R5-code-63); each copy fetched
/// again printed as `fetched_line`. Err holds the exit code after the refusal's line, printed bare. A folder that
/// cannot be listed is left to `DiskTree::open`.
fn refresh_evidence(ctx: &Ctx, evidence: &Path, shown_root: &str, err: &mut dyn Write) -> Result<(), i32> {
    let Ok(entries) = std::fs::read_dir(evidence) else {
        return Ok(());
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name != ".git")
        .collect();
    names.sort();
    for name in names {
        let shown = format!("{shown_root}/{name}");
        match refresh_copies(ctx, &evidence.join(&name), &shown) {
            Ok(fetched) => {
                for file in fetched {
                    let _ = writeln!(err, "{}", fetched_line("retune", &shown, &file));
                }
            }
            Err(Refused { code, line }) => return Err(fail(err, &line, code)),
        }
    }
    Ok(())
}

/// Prints `line` to stderr and returns `code`.
fn fail(err: &mut dyn Write, line: &str, code: i32) -> i32 {
    let _ = writeln!(err, "{line}");
    code
}

/// The spec file and how it is shown: --spec as given (taken against cwd), else the design spec under the repository
/// root; None when neither names one.
fn spec_file(ctx: &Ctx, flag: Option<String>) -> Option<(PathBuf, String)> {
    match flag {
        Some(given) => Some((ctx.abs(&given), given)),
        None => default_spec(ctx).map(|path| {
            let shown = path.display().to_string();
            (path, shown)
        }),
    }
}

/// `retune --subtitles`: row 14 from the spec (exit 2 when it cannot be read), the evidence (resolve_evidence's code
/// over the DIR half), the OCR text (the TEXTDIR half, else --text DIR, else LAZYMKV_OCR_TEXT, else none; exit 2 for a
/// folder that is not there), then the subtitle harness with spec 14's default settings; its report on stdout and its
/// exit code. RP15, spec 15; design part 7.7, Ruling T5-R53.
fn run_subtitles(
    ctx: &Ctx,
    evidence_arg: Option<&EvidenceArg>,
    spec_path: &Path,
    spec_shown: &str,
    text_flag: Option<&str>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let row14 = match read_row14(spec_path, spec_shown) {
        Ok(row14) => row14,
        Err(message) => return fail(err, &message, 2),
    };
    let evidence = match resolve_evidence(ctx, evidence_arg.map(|arg| arg.dir.as_str())) {
        Ok(dir) => dir,
        Err(Refused { code, line }) => return fail(err, &line, code),
    };
    if let Err(code) = refresh_evidence(ctx, &evidence, EVIDENCE_SHOWN, err) {
        return code;
    }
    let left_out = unmeasured_job_folders(ctx, &evidence, EVIDENCE_SHOWN, err);
    let tree = match DiskTree::open_without(&evidence, EVIDENCE_SHOWN, &left_out) {
        Ok(tree) => tree,
        Err(e) => return fail(err, &os_error_text(&e, &evidence.display().to_string()), 3),
    };
    let text_dir = match evidence_arg.and_then(|arg| arg.text.as_ref()) {
        Some(text) => Some(ctx.abs(text)),
        None => ocr_text_dir(ctx, text_flag),
    };
    let text = match open_text(text_dir) {
        Ok(text) => text,
        Err(line) => return fail(err, &line, 2),
    };
    let h = subtitle_harness(&tree, text.as_ref().map(|tree| tree as &dyn Tree), &row14, &Settings::default());
    for line in &h.lines {
        let _ = writeln!(out, "{line}");
    }
    h.exit_code
}

/// Row 14 of the spec file, read as UTF-8 with an optional byte order mark; Err with the reading's or the row's text.
fn read_row14(spec_path: &Path, spec_shown: &str) -> Result<Row14, String> {
    read_text_file(spec_path, spec_shown, Encoding::Utf8Sig).and_then(|text| exp14_requirements(&text))
}

/// The OCR text folder as a tree printed as its path, None for none; Err with the line of exit 2 when it is not a
/// folder (`the OCR text folder <shown> does not exist <TEXT_HINT>`) or cannot be read.
fn open_text(dir: Option<PathBuf>) -> Result<Option<DiskTree>, String> {
    let Some(dir) = dir else {
        return Ok(None);
    };
    let shown = dir.display().to_string();
    if !dir.is_dir() {
        return Err(format!("the OCR text folder {shown} does not exist {TEXT_HINT}"));
    }
    DiskTree::open(&dir, &shown).map(Some).map_err(|e| os_error_text(&e, &shown))
}

/// Path.read_text(encoding) with strict errors; Err with the OSError or UnicodeDecodeError text.
fn read_text_file(path: &Path, shown: &str, encoding: Encoding) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| os_error_text(&e, shown))?;
    decode_text(&bytes, encoding, Errors::Strict)
}

/// load_snapshot(path): the file read as plain "utf-8"; None when it is missing, not text, not JSON or not a
/// snapshot.
fn read_snapshot_file(path: &Path) -> Option<PreviousSnapshot> {
    read_snapshot(&read_text_file(path, "", Encoding::Utf8).ok()?)
}

/// The document as text with "created" set to null in place (`dict(d, created=None)`), so NaN-free documents compare
/// as the Python's `timeless` did.
fn timeless(value: &Value) -> String {
    let mut v = value.clone();
    if let Some(object) = v.as_object_mut() {
        object.insert("created".to_string(), Value::Null);
    }
    dumps(&v, Style::INDENT1_ASCII)
}

/// Writes the snapshot unless `path` already holds it with only another "created"; true when written.
/// Py: write_snapshot's comparison and write.
fn write_unless_unchanged(path: &Path, value: &Value) -> std::io::Result<bool> {
    if let Some(old) = read_snapshot_file(path) {
        if timeless(&old.raw) == timeless(value) {
            return Ok(false);
        }
    }
    write_snapshot(path, value)?;
    Ok(true)
}

#[cfg(test)]
mod tests;
