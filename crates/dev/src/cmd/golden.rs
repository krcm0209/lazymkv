//! The goldens of the Rust port (port spec section 5; the schemas, the provenance and the re-recording rule are in the
//! evidence repository's golden/README.md). write_goldens.py, beside that README, wrote every golden once from the
//! phase 0 Python. `golden check` recomputes each with the Rust and compares: JSON structurally within the tolerance of
//! spec section 4 (`structural_diff`, `Tolerance::spec`), text line by line, the retune report by `retune_diff`, a
//! rebuilt proxy folder byte for byte, and the harness's snapshot against the committed baseline by `snapshot_diff`
//! (the round trip). Every comparison ends pass, fail or skipped with its reason; a golden that is not there is "not
//! covered", a skip. `golden write` writes the same files from the Rust, for a re-recording commit only.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};

use lazymkv_core::audio::dataset::{
    find_folders, harness, read_snapshot, result_line, snapshot_line, HarnessError, HarnessInput, Overrides,
    BASELINE_FILE, LEVELS_NOT_HERE,
};
use lazymkv_core::audio::labels::{exp13_requirements, is_slug, labels_json, load_fitted, Requirements};
use lazymkv_core::audio::measures::Constants;
use lazymkv_core::audio::report::{folder_json, report_lines};
use lazymkv_core::audio::rule::{results, Plain, Rule};
use lazymkv_core::audio::titles::titles_report;
use lazymkv_core::reconcile::{reconcile, RipLog, RIP_LOG_NAMES};
use lazymkv_core::recording::files::{rip_labels, MkvmergeIdent, RipLabel};
use lazymkv_core::recording::{py_repr_str, read_json, read_text, sha1_hex, Encoding, Errors, PathKey, Tree};
use lazymkv_core::robot::{parse, scan_json};
use lazymkv_core::subtitle::align::align_subs_dir;
use lazymkv_core::subtitle::labels::TEXT_HINT;
use lazymkv_core::subtitle::text::{subs_json, subs_run, Rule as TextRule};
use serde_json::Value;

use crate::cmd::levels_from_proxy::build;
use crate::evidence::{default_spec, ocr_text_dir, resolve_evidence, Ctx, DiskTree, EVIDENCE_SHOWN};
use crate::pyjson::{dumps, structural_diff, Style, Tolerance};
use crate::{usage_error, Args};

/// The folder name reserved for goldens. Every walker skips it (Tree::list and Tree::glob, Task 1), so the goldens
/// are found and read here through the file system.
pub const GOLDEN_DIR: &str = "golden";
/// The OCR reading the goldens use: the folder tuning-fetch.ps1 -Text copies, <TuningData>\<slug>\<label>\subs-ocr.
const READING: &str = "subs-ocr";
/// subtitle_alignment's defaults (--clear-max-bytes 64, --tolerance-ms 300), which align.json is written with.
const ALIGN_CLEAR_MAX_BYTES: i64 = 64;
const ALIGN_TOLERANCE_MS: i64 = 300;
/// The retune report's snapshot line names this path: a check writes no snapshot, and the line is not compared.
const SNAPSHOT_SHOWN: &str = "$TMPDIR/lazymkv-golden-retune.json";
/// A proxy golden's own file, and the files of a rebuilt level folder that are neither kept nor compared.
const PROXY_GOLDEN: &str = "golden.json";
const PROXY_NOT_COMPARED: [&str; 2] = ["proxy.txt", "ffmpeg.txt"];
/// A Python golden kept beside its re-recorded successor (README "Re-recording a golden"): never read.
const PYTHON_SUFFIX: &str = ".python";
/// golden/README.md and the fence of its written_by block.
const README: &str = "README.md";
const WRITTEN_BY_FENCE: &str = "```json written_by";
/// The usage line's arguments, as argparse would print them.
const USAGE: &str = "golden write|check [--evidence DIR] [--text DIR]";
/// How many differences a fail line shows.
const SHOWN_DIFFERENCES: usize = 3;
/// write_goldens.py, beside README.md: neither is a golden.
const WRITER: &str = "write_goldens.py";
/// The comparison that says the golden table's proxy row is not covered (no <slug>/golden/proxy-* folder).
const NO_PROXY: &str = "*/golden/proxy-*";

/// The README's written_by block: provenance of every golden.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WrittenBy {
    /// The lazymkv pull request and the branch commit write_goldens.py ran beside.
    pub pull_request: u32,
    pub branch_commit: String,
    /// `git rev-parse <branch_commit>:scripts/phase0`.
    pub scripts_phase0_tree: String,
    /// The evidence repository's commit the inputs are at.
    pub evidence_commit: String,
    /// "3.9.6", and `ffmpeg -version`'s first line.
    pub python: String,
    pub ffmpeg: String,
    /// Input path relative to corpus/owner -> SHA-1 of its bytes (level series and OCR text included).
    pub inputs: BTreeMap<String, String>,
}

/// The written_by block of golden/README.md: the JSON between the line "```json written_by" and the next "```" line.
pub fn read_written_by(readme: &str) -> Result<WrittenBy, String> {
    let mut lines = readme.lines();
    if !lines.any(|line| line.trim_end() == WRITTEN_BY_FENCE) {
        return Err(format!("no {WRITTEN_BY_FENCE} block"));
    }
    let mut body = Vec::new();
    for line in lines {
        if line.trim_end() == "```" {
            return serde_json::from_str(&body.join("\n"))
                .map_err(|error| format!("the written_by block is not WrittenBy: {error}"));
        }
        body.push(line);
    }
    Err(format!("the {WRITTEN_BY_FENCE} block is not closed"))
}

/// proxy-<folder>/golden.json: what was rebuilt and with which ffmpeg.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProxyGolden {
    pub slug: String,
    pub label: String,
    /// The level folder the proxy rebuilt (proxy-<folder>'s <folder>).
    pub folder: String,
    pub main: i64,
    pub cands: Vec<i64>,
    /// `ffmpeg -version`'s first line when the golden was written; another version skips the comparison.
    pub ffmpeg_version: String,
    /// SHA-1 of the audio-proxy.mka fetched from the tuning dataset (<DIR>/<slug>/<label>/audio-proxy.mka).
    pub proxy_sha1: String,
}

/// How one comparison ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Pass,
    Fail(String),
    Skipped(String),
}

/// "pass", "fail: <first differences>", "skipped: <reason>".
impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Outcome::Pass => write!(f, "pass"),
            Outcome::Fail(differences) => write!(f, "fail: {differences}"),
            Outcome::Skipped(reason) => write!(f, "skipped: {reason}"),
        }
    }
}

/// One comparison: its name is the golden's path below corpus/owner ("<slug>/golden/<label>/subs.json").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Comparison {
    pub name: String,
    pub outcome: Outcome,
}

/// What the Rust computes for one golden.
enum Produced {
    /// A JSON golden: compared structurally; written as json.dumps(indent=1) and a newline.
    Json(Value),
    /// A text golden: compared line by line; written with a newline after every line.
    Lines(Vec<String>),
    /// retune-report.txt: compared by retune_diff; written as Lines.
    Retune(Vec<String>),
    /// A rebuilt proxy folder: its files but proxy.txt and ffmpeg.txt, by name, and its golden.json.
    Proxy { files: Vec<(String, Vec<u8>)>, golden: Value },
    /// The harness's snapshot, compared with the committed baseline by snapshot_diff; never written.
    RoundTrip(Value),
}

/// Why a golden was not computed: an input is absent (a skip, with the reason) or the computation failed (a fail).
enum NotProduced {
    Skipped(String),
    Failed(String),
}

type Production = Result<Produced, NotProduced>;

/// Goldens computed together (one measurement gives audio-<folder>.json and .txt) and how: a check computes nothing
/// for goldens that are not there.
struct Unit<'a> {
    names: Vec<String>,
    produce: Box<dyn FnOnce() -> Vec<Production> + 'a>,
}

impl<'a> Unit<'a> {
    fn one(name: String, produce: impl FnOnce() -> Production + 'a) -> Unit<'a> {
        Unit { names: vec![name], produce: Box::new(move || vec![produce()]) }
    }

    fn pair(names: [String; 2], produce: impl FnOnce() -> [Production; 2] + 'a) -> Unit<'a> {
        Unit { names: Vec::from(names), produce: Box::new(move || produce().into_iter().collect()) }
    }
}

/// The same reason for both goldens of a pair.
fn both(reason: NotProduced) -> [Production; 2] {
    match reason {
        NotProduced::Skipped(text) => [Err(NotProduced::Skipped(text.clone())), Err(NotProduced::Skipped(text))],
        NotProduced::Failed(text) => [Err(NotProduced::Failed(text.clone())), Err(NotProduced::Failed(text))],
    }
}

fn to_json<T: serde::Serialize>(value: &T) -> Production {
    serde_json::to_value(value)
        .map(Produced::Json)
        .map_err(|error| NotProduced::Failed(format!("not JSON: {error}")))
}

/// The recording folders of the evidence, in PathKey order (Tree::list leaves golden/ out).
fn recordings(tree: &dyn Tree) -> Vec<String> {
    tree.list(&PathKey::root())
        .into_iter()
        .filter(|path| tree.is_dir(path) && is_slug(path.name()))
        .map(|path| path.name().to_string())
        .collect()
}

/// A rip label's audio-levels-20ms-* folders, in PathKey order (the first measurement's audio-levels-<main>-<cand>
/// folders are not level folders).
fn level_folders(tree: &dyn Tree, label_dir: &PathKey) -> Vec<PathKey> {
    tree.glob(label_dir, "audio-levels-20ms-*").into_iter().filter(|path| tree.is_dir(path)).collect()
}

/// The proxy goldens of a recording: its golden/proxy-* folders (a kept .python folder is not one), by name.
fn proxy_folders(evidence: &Path, slug: &str) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(evidence.join(slug).join(GOLDEN_DIR))
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| entry.path().is_dir())
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .filter(|name| name.starts_with("proxy-") && !name.ends_with(PYTHON_SUFFIX))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// Every golden of one recording, in the order write_goldens.py writes them.
fn recording_units<'a>(
    ctx: &'a Ctx,
    tree: &'a DiskTree,
    evidence: &'a Path,
    slug: &str,
    text: Option<&'a Path>,
    writing: bool,
) -> Vec<Unit<'a>> {
    let dir = PathKey::new(slug);
    let gold = format!("{slug}/{GOLDEN_DIR}");
    let mut units = Vec::new();
    let scan = dir.join("scan.txt");
    if tree.is_file(&scan) {
        units.push(Unit::one(format!("{gold}/scan.json"), move || scan_golden(tree, &scan)));
        let rec = dir.clone();
        units.push(Unit::one(format!("{gold}/titles.txt"), move || {
            titles_report(tree, &rec).map(Produced::Lines).map_err(NotProduced::Failed)
        }));
    }
    let rec = dir.clone();
    units.push(Unit::one(format!("{gold}/labels.json"), move || labels_golden(ctx, tree, &rec)));
    for rip in rip_labels(tree, &dir) {
        let label_dir = dir.join(&rip.label);
        let label_gold = format!("{gold}/{}", rip.label);
        let rec = dir.clone();
        let this = rip.clone();
        units.push(Unit::one(format!("{label_gold}/reconcile.json"), move || reconcile_golden(tree, &rec, &this)));
        for folder in level_folders(tree, &label_dir) {
            let name = folder.name().to_string();
            let (owned_slug, label) = (slug.to_string(), rip.label.clone());
            units.push(Unit::pair(
                [format!("{label_gold}/audio-{name}.json"), format!("{label_gold}/audio-{name}.txt")],
                move || audio_goldens(tree, &owned_slug, &label, &folder),
            ));
        }
        if tree.is_file(&label_dir.join("subs/index.json")) && tree.is_dir(&label_dir.join(READING)) {
            let (owned_slug, label) = (slug.to_string(), rip.label.clone());
            units.push(Unit::pair(
                [format!("{label_gold}/subs.json"), format!("{label_gold}/subs.txt")],
                move || subs_goldens(tree, &owned_slug, &label, text, writing),
            ));
        }
        let subs = label_dir.join("subs");
        if tree.is_dir(&subs) {
            units.push(Unit::one(format!("{label_gold}/align.json"), move || align_golden(tree, &subs)));
        }
    }
    for name in proxy_folders(evidence, slug) {
        let folder = evidence.join(slug).join(GOLDEN_DIR).join(&name);
        units.push(Unit::one(format!("{gold}/{name}"), move || proxy_golden(ctx, &folder, text, writing)));
    }
    units
}

/// The goldens of the whole evidence: the retune report, and the round trip against audio-dataset.json.
fn corpus_units<'a>(ctx: &'a Ctx, tree: &'a DiskTree) -> Vec<Unit<'a>> {
    vec![Unit::pair([format!("{GOLDEN_DIR}/retune-report.txt"), BASELINE_FILE.to_string()], move || {
        retune_goldens(ctx, tree)
    })]
}

/// scan.json: the parsed scan without drives and messages; scan.txt read as UTF-8 with replacement.
fn scan_golden(tree: &dyn Tree, scan: &PathKey) -> Production {
    let text = read_text(tree, scan, Encoding::Utf8, Errors::Replace).map_err(NotProduced::Failed)?;
    to_json(&scan_json(&parse(&text)))
}

/// Row 13 of the spec that --spec defaults to (one source for the numbers, as the Python's).
fn spec_requirements(ctx: &Ctx) -> Result<Requirements, String> {
    let path = default_spec(ctx)
        .ok_or_else(|| "the spec cannot be found: no repository root above the current folder".to_string())?;
    let text = std::fs::read_to_string(&path).map_err(|error| format!("cannot read the spec {}: {error}", path.display()))?;
    exp13_requirements(text.trim_start_matches('\u{feff}'))
}

/// labels.json, with the fitted set of row 13 and the evidence's audio-fitted.json.
fn labels_golden(ctx: &Ctx, tree: &dyn Tree, rec: &PathKey) -> Production {
    let fitting = spec_requirements(ctx).map_err(NotProduced::Failed)?.fitting_slug;
    let fitted = load_fitted(tree, &PathKey::root(), Some(fitting.as_str())).map_err(NotProduced::Failed)?;
    to_json(&labels_json(tree, rec, &fitted))
}

/// reconcile.json: the rip against the scan, its selection and title from the summary, its rip log (rip.txt, else
/// extract.txt) found beside mkvmerge.json, as compare_tracks.main finds it.
fn reconcile_golden(tree: &dyn Tree, rec: &PathKey, rip: &RipLabel) -> Production {
    let text = read_text(tree, &rec.join("scan.txt"), Encoding::Utf8, Errors::Replace).map_err(NotProduced::Failed)?;
    let scan = parse(&text);
    let label_dir = rec.join(&rip.label);
    let ident: MkvmergeIdent = read_json(tree, &label_dir.join("mkvmerge.json")).map_err(NotProduced::Failed)?;
    let log = RIP_LOG_NAMES.iter().map(|name| label_dir.join(name)).find(|path| tree.exists(path));
    let log_text = match &log {
        Some(path) => Some(read_text(tree, path, Encoding::Utf8, Errors::Replace).map_err(NotProduced::Failed)?),
        None => None,
    };
    let rip_log = match (&log, &log_text) {
        (Some(path), Some(text)) => Some(RipLog { shown: tree.display(path), text: text.as_str() }),
        _ => None,
    };
    let title = u32::try_from(rip.title).map_err(|_| {
        NotProduced::Failed(format!("{}: title {} is not a scan title number", rip.label, rip.title))
    })?;
    to_json(&reconcile(&scan, title, &ident, rip.selection, rip_log).map_err(NotProduced::Failed)?)
}

/// The Python's line for a level folder that git keeps without its level files (audio_dataset.measure), or None.
fn series_absent(tree: &dyn Tree, slug: &str, label: &str, folder: &PathKey) -> Option<String> {
    if tree.is_file(&folder.join("manifest.txt")) || !tree.is_file(&folder.join("streams.json")) {
        return None;
    }
    Some(format!("{slug}/{label}/{}: {LEVELS_NOT_HERE}", folder.name()))
}

/// audio-<folder>.json and .txt: one measurement with the default rule and constants and no cache.
fn audio_goldens(tree: &dyn Tree, slug: &str, label: &str, folder: &PathKey) -> [Production; 2] {
    if let Some(reason) = series_absent(tree, slug, label, folder) {
        return both(NotProduced::Skipped(reason));
    }
    let rule = Rule::default();
    let consts = Constants::default();
    match results(tree, folder, &rule, &consts, &mut Plain { consts: &consts }) {
        Ok(res) => [to_json(&folder_json(&res, folder.name(), &rule)), Ok(Produced::Lines(report_lines(tree, folder, &res)))],
        Err(message) => both(NotProduced::Failed(message)),
    }
}

/// Whether a folder holds a sub-<n>.ocr.jsonl.
fn holds_ocr_text(folder: &Path) -> bool {
    std::fs::read_dir(folder)
        .map(|entries| {
            entries.flatten().any(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                name.starts_with("sub-") && name.ends_with(".ocr.jsonl")
            })
        })
        .unwrap_or(false)
}

/// subs.json and subs.txt: the subs run over <text>/<slug>/<label>/subs-ocr, the original language from tmdb.json. The
/// text folder is shown as TEXT/<slug>/<label>/subs-ocr (write_goldens.py's name for it), so no absolute path reaches
/// a golden or a line. A run that does not exit 0 produces no golden: `golden write` fails, as write_goldens.py's
/// fail() stopped, and `golden check` skips with the run's lines, as there is nothing complete to compare.
fn subs_goldens(tree: &dyn Tree, slug: &str, label: &str, text: Option<&Path>, writing: bool) -> [Production; 2] {
    let Some(root) = text else {
        return both(NotProduced::Skipped(format!("no OCR text: {slug}/{label}/{READING} {TEXT_HINT}")));
    };
    let label_text = root.join(slug).join(label);
    let reading = label_text.join(READING);
    if !holds_ocr_text(&reading) {
        return both(NotProduced::Skipped(format!("no OCR text: {} {TEXT_HINT}", reading.display())));
    }
    let shown = format!("TEXT/{slug}/{label}");
    let text_tree = match DiskTree::open_sub(&label_text, &shown, READING) {
        Ok(text_tree) => text_tree,
        Err(error) => return both(NotProduced::Failed(format!("{shown}/{READING}: {error}"))),
    };
    let rule = TextRule::default();
    let rec = PathKey::new(&format!("{slug}/{label}"));
    let reading_key = PathKey::new(READING);
    let run = subs_run(tree, &rec, &text_tree, &reading_key, None, rule.CLEAR_MAX_BYTES, &rule);
    if run.exit_code != 0 {
        let reason = format!("the subs run exited {}: {}", run.exit_code, run.stderr.join("; "));
        return both(if writing { NotProduced::Failed(reason) } else { NotProduced::Skipped(reason) });
    }
    let lines = run.stdout.iter().chain(run.stderr.iter()).cloned().collect();
    [to_json(&subs_json(&run, READING, &text_tree, &reading_key)), Ok(Produced::Lines(lines))]
}

/// align.json: the align table over the rip's subs/ folder with subtitle_alignment's defaults.
fn align_golden(tree: &dyn Tree, subs: &PathKey) -> Production {
    to_json(&align_subs_dir(tree, subs, ALIGN_CLEAR_MAX_BYTES, ALIGN_TOLERANCE_MS).json)
}

/// The first line `<program> -version` prints; a skip when the program is not on PATH.
fn tool_version(ctx: &Ctx, program: &str) -> Result<String, NotProduced> {
    match ctx.runner.run(program, &["-version".to_string()], None) {
        Ok(output) => Ok(output.stdout.lines().next().unwrap_or("").to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(NotProduced::Skipped(format!("{program} is not on PATH")))
        }
        Err(error) => Err(NotProduced::Failed(format!("{program} -version: {error}"))),
    }
}

/// The files of a folder (but proxy.txt and ffmpeg.txt), by name, with their bytes.
fn folder_files(folder: &Path) -> Result<Vec<(String, Vec<u8>)>, String> {
    let entries = std::fs::read_dir(folder).map_err(|error| format!("{}: {error}", folder.display()))?;
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("{}: {error}", folder.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.path().is_file() && !PROXY_NOT_COMPARED.contains(&name.as_str()) {
            let bytes = std::fs::read(entry.path()).map_err(|error| format!("{name}: {error}"))?;
            files.push((name, bytes));
        }
    }
    files.sort();
    Ok(files)
}

/// A proxy golden: the level folder levels-from-proxy rebuilds, into a temporary folder, from the fetched proxy its
/// golden.json names. In a check another ffmpeg or another proxy is a skip; `golden write` takes them and records them.
fn proxy_golden(ctx: &Ctx, folder: &Path, text: Option<&Path>, writing: bool) -> Production {
    let golden: ProxyGolden = std::fs::read(folder.join(PROXY_GOLDEN))
        .map_err(|error| error.to_string())
        .and_then(|bytes| serde_json::from_slice(&bytes).map_err(|error| format!("not a proxy golden ({error})")))
        .map_err(|error| NotProduced::Failed(format!("{PROXY_GOLDEN}: {error}")))?;
    let version = tool_version(ctx, "ffmpeg")?;
    if version != golden.ffmpeg_version && !writing {
        return Err(NotProduced::Skipped(format!(
            "ffmpeg is {version}, the golden was written by {}",
            golden.ffmpeg_version
        )));
    }
    tool_version(ctx, "ffprobe")?;
    let Some(root) = text else {
        return Err(NotProduced::Skipped(format!(
            "the proxy is not fetched: {}/{}/audio-proxy.mka",
            golden.slug, golden.label
        )));
    };
    let mka = root.join(&golden.slug).join(&golden.label).join("audio-proxy.mka");
    if !mka.is_file() {
        return Err(NotProduced::Skipped(format!("the proxy is not fetched: {}", mka.display())));
    }
    let sha = std::fs::read(&mka)
        .map(|bytes| sha1_hex(&bytes))
        .map_err(|error| NotProduced::Failed(format!("{}: {error}", mka.display())))?;
    if sha != golden.proxy_sha1 && !writing {
        return Err(NotProduced::Skipped(format!(
            "the proxy {} has SHA-1 {sha}, the golden was written from {}",
            mka.display(),
            golden.proxy_sha1
        )));
    }
    let out = ctx.temp_dir().join(format!("lazymkv-golden-proxy-{}-{}", std::process::id(), golden.folder));
    if out.exists() {
        std::fs::remove_dir_all(&out).map_err(|error| NotProduced::Failed(format!("{}: {error}", out.display())))?;
    }
    let built = build(ctx, &mka, &mka.display().to_string(), &out, golden.main, &golden.cands, None)
        .map_err(NotProduced::Failed)
        .and_then(|_| folder_files(&out).map_err(NotProduced::Failed));
    let _ = std::fs::remove_dir_all(&out);
    let files = built?;
    let golden = ProxyGolden { ffmpeg_version: version, proxy_sha1: sha, ..golden };
    let golden = serde_json::to_value(&golden).map_err(|error| NotProduced::Failed(format!("not JSON: {error}")))?;
    Ok(Produced::Proxy { files, golden })
}

/// retune-report.txt and the round trip: the harness over the whole evidence with no override, the baseline as the
/// previous snapshot, the snapshot line spelt as the goldens spell it (nothing is written) and the RESULT line.
fn retune_goldens(ctx: &Ctx, tree: &dyn Tree) -> [Production; 2] {
    for (slug, label, folder) in find_folders(tree) {
        if let Some(reason) = series_absent(tree, &slug, &label, &folder) {
            return both(NotProduced::Skipped(reason));
        }
    }
    let req = match spec_requirements(ctx) {
        Ok(req) => req,
        Err(message) => return both(NotProduced::Failed(message)),
    };
    let baseline = read_text(tree, &PathKey::new(BASELINE_FILE), Encoding::Utf8, Errors::Strict).ok();
    let previous = baseline.as_deref().and_then(read_snapshot);
    let overrides = Overrides::default();
    let created = ctx.now();
    let input = HarnessInput {
        tree,
        req: &req,
        overrides: &overrides,
        previous: previous.as_ref(),
        compare: None,
        use_cache: true,
        created: created.as_str(),
    };
    let run = match harness(&input) {
        Ok(run) => run,
        Err(HarnessError::Labels(lines)) => {
            return both(NotProduced::Failed(format!("a labels file does not validate: {}", lines.join("; "))));
        }
    };
    let mut lines = run.lines.clone();
    lines.push(String::new());
    lines.push(snapshot_line(SNAPSHOT_SHOWN, "", true));
    lines.push(result_line(&run.failures, &run.unchecked, false).0);
    let snapshot = serde_json::to_value(&run.snapshot)
        .map(Produced::RoundTrip)
        .map_err(|error| NotProduced::Failed(format!("not JSON: {error}")));
    [Ok(Produced::Retune(lines)), snapshot]
}

/// The lines of a golden text file: every line ends with "\n"; an empty file holds none.
fn golden_lines(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    text.strip_suffix('\n').unwrap_or(text).split('\n').map(str::to_string).collect()
}

/// The text of a golden text file: every line followed by "\n".
fn lines_text(lines: &[String]) -> String {
    lines.iter().map(|line| format!("{line}\n")).collect()
}

fn read_golden_lines(path: &Path, name: &str) -> Result<Vec<String>, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("{EVIDENCE_SHOWN}/{name}: {error}"))?;
    let text = String::from_utf8(bytes).map_err(|_| format!("{EVIDENCE_SHOWN}/{name}: not UTF-8"))?;
    Ok(golden_lines(&text))
}

fn read_golden_json(path: &Path, name: &str) -> Result<Value, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("{EVIDENCE_SHOWN}/{name}: {error}"))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("{EVIDENCE_SHOWN}/{name}: not valid JSON ({error})"))
}

/// A line as a difference shows it: Python's repr, or "(absent)".
fn shown_line(line: Option<&String>) -> String {
    line.map_or_else(|| "(absent)".to_string(), |line| py_repr_str(line))
}

/// Every line that differs, "line <n>: <golden> != <ours>".
fn line_diff(golden: &[String], ours: &[String]) -> Vec<String> {
    (0..golden.len().max(ours.len()))
        .filter(|&at| golden.get(at) != ours.get(at))
        .map(|at| format!("line {}: {} != {}", at + 1, shown_line(golden.get(at)), shown_line(ours.get(at))))
        .collect()
}

/// Two lines that state the measure cache or the baseline rather than the rule: both "measures: ...", both
/// "snapshot: ...", or both "Changes since the previous snapshot (...):".
fn state_lines(golden: &str, ours: &str) -> bool {
    const MEASURES: &str = "measures: ";
    const SNAPSHOT: &str = "snapshot: ";
    const CHANGES: &str = "Changes since the previous snapshot (";
    let changes = |line: &str| line.starts_with(CHANGES) && line.ends_with("):");
    (golden.starts_with(MEASURES) && ours.starts_with(MEASURES))
        || (golden.starts_with(SNAPSHOT) && ours.starts_with(SNAPSHOT))
        || (changes(golden) && changes(ours))
}

/// retune-report.txt against the Rust's lines: byte for byte, except the lines whose content depends on the cache
/// and the baseline's state rather than on the rule: "measures: ...", "snapshot: ...", and the parenthesis of
/// "Changes since the previous snapshot (...):" (02-interfaces.md, open point 1).
pub fn retune_diff(golden: &[String], ours: &[String]) -> Vec<String> {
    (0..golden.len().max(ours.len()))
        .filter(|&at| match (golden.get(at), ours.get(at)) {
            (Some(g), Some(o)) => g != o && !state_lines(g, o),
            _ => true,
        })
        .map(|at| format!("line {}: {} != {}", at + 1, shown_line(golden.get(at)), shown_line(ours.get(at))))
        .collect()
}

/// The round trip of spec section 5: the committed baseline against the first v3 snapshot, structurally, with
/// "/format", "/created", "/code", "/tracks/*/key" and "/tracks/*/peer/key" ignored and "measures_cache" compared as a
/// multiset of values (its keys changed with the formula).
pub fn snapshot_diff(baseline: &Value, ours: &Value) -> Vec<String> {
    let mut tolerance = Tolerance::spec();
    tolerance.ignore = ["/format", "/created", "/code", "/tracks/*/key", "/tracks/*/peer/key", "/measures_cache"]
        .iter()
        .map(|pointer| pointer.to_string())
        .collect();
    let mut out = structural_diff(baseline, ours, &tolerance);
    let cache = |document: &Value| -> Vec<Value> {
        document
            .get("measures_cache")
            .and_then(Value::as_object)
            .map(|entries| entries.values().cloned().collect())
            .unwrap_or_default()
    };
    let (expected, actual) = (cache(baseline), cache(ours));
    let mut matched = vec![false; actual.len()];
    let mut missing = 0;
    for value in &expected {
        let found = (0..actual.len())
            .find(|&at| !matched[at] && structural_diff(value, &actual[at], &Tolerance::spec()).is_empty());
        match found {
            Some(at) => matched[at] = true,
            None => missing += 1,
        }
    }
    let extra = matched.iter().filter(|seen| !**seen).count();
    if missing > 0 || extra > 0 {
        out.push(format!(
            "/measures_cache: {missing} measure(s) of the baseline have no equal here, {extra} measure(s) here have \
             none in the baseline"
        ));
    }
    out
}

/// Where two byte strings first differ (the shorter one's length when one is a prefix of the other).
fn first_difference(a: &[u8], b: &[u8]) -> usize {
    a.iter().zip(b).position(|(x, y)| x != y).unwrap_or_else(|| a.len().min(b.len()))
}

/// A rebuilt proxy folder against its golden folder, file by file, byte for byte (golden.json and kept .python files
/// are not compared).
fn proxy_diff(folder: &Path, files: &[(String, Vec<u8>)]) -> Vec<String> {
    let golden: Vec<(String, Vec<u8>)> = match folder_files(folder) {
        Ok(golden) => golden
            .into_iter()
            .filter(|(name, _)| name != PROXY_GOLDEN && !name.ends_with(PYTHON_SUFFIX))
            .collect(),
        Err(message) => return vec![message],
    };
    let mut out = Vec::new();
    for (name, bytes) in files {
        match golden.iter().find(|(golden_name, _)| golden_name == name) {
            None => out.push(format!("{name}: rebuilt, but not in the golden")),
            Some((_, expected)) if expected != bytes => out.push(format!(
                "{name}: {} bytes in the golden, {} rebuilt, first difference at byte {}",
                expected.len(),
                bytes.len(),
                first_difference(expected, bytes)
            )),
            Some(_) => {}
        }
    }
    for (name, _) in &golden {
        if !files.iter().any(|(rebuilt, _)| rebuilt == name) {
            out.push(format!("{name}: in the golden, not rebuilt"));
        }
    }
    out
}

/// The first differences of a fail line, "; "-joined, and how many more there are.
fn first_differences(differences: &[String]) -> String {
    let mut text = differences.iter().take(SHOWN_DIFFERENCES).cloned().collect::<Vec<_>>().join("; ");
    if differences.len() > SHOWN_DIFFERENCES {
        text.push_str(&format!("; and {} more", differences.len() - SHOWN_DIFFERENCES));
    }
    text
}

/// One golden against what the Rust computed for it.
fn compare(evidence: &Path, name: &str, production: Production) -> Outcome {
    let produced = match production {
        Ok(produced) => produced,
        Err(NotProduced::Skipped(reason)) => return Outcome::Skipped(reason),
        Err(NotProduced::Failed(message)) => return Outcome::Fail(message),
    };
    let path = evidence.join(name);
    let differences = match &produced {
        Produced::Json(ours) => match read_golden_json(&path, name) {
            Ok(golden) => structural_diff(&golden, ours, &Tolerance::spec()),
            Err(message) => return Outcome::Fail(message),
        },
        Produced::Lines(ours) => match read_golden_lines(&path, name) {
            Ok(golden) => line_diff(&golden, ours),
            Err(message) => return Outcome::Fail(message),
        },
        Produced::Retune(ours) => match read_golden_lines(&path, name) {
            Ok(golden) => retune_diff(&golden, ours),
            Err(message) => return Outcome::Fail(message),
        },
        Produced::Proxy { files, .. } => proxy_diff(&path, files),
        Produced::RoundTrip(ours) => match read_golden_json(&path, name) {
            Ok(baseline) => snapshot_diff(&baseline, ours),
            Err(message) => return Outcome::Fail(message),
        },
    };
    if differences.is_empty() {
        Outcome::Pass
    } else {
        Outcome::Fail(first_differences(&differences))
    }
}

fn skipped(name: &str, reason: String) -> Comparison {
    Comparison { name: name.to_string(), outcome: Outcome::Skipped(reason) }
}

fn not_covered(name: &str) -> Comparison {
    skipped(name, format!("not covered: {name} is not in the goldens"))
}

/// The comparisons of one unit: nothing is computed when none of its goldens is there.
fn compare_unit(evidence: &Path, unit: Unit<'_>, out: &mut Vec<Comparison>) {
    let Unit { names, produce } = unit;
    let present: Vec<bool> = names.iter().map(|name| evidence.join(name).exists()).collect();
    if !present.contains(&true) {
        out.extend(names.iter().map(|name| not_covered(name)));
        return;
    }
    for ((name, present), production) in names.iter().zip(present).zip(produce()) {
        if present {
            out.push(Comparison { name: name.clone(), outcome: compare(evidence, name, production) });
        } else {
            out.push(not_covered(name));
        }
    }
}

/// golden/README.md: its written_by block parses (pass) or not (fail); None when there is no README.
fn readme_comparison(evidence: &Path) -> Option<Comparison> {
    let path = evidence.join(GOLDEN_DIR).join(README);
    if !path.is_file() {
        return None;
    }
    let name = format!("{GOLDEN_DIR}/{README}");
    let outcome = match std::fs::read_to_string(&path).map_err(|error| error.to_string()).and_then(|text| read_written_by(&text)) {
        Ok(_) => Outcome::Pass,
        Err(message) => Outcome::Fail(format!("{EVIDENCE_SHOWN}/{name}: {message}")),
    };
    Some(Comparison { name, outcome })
}

/// The files of a golden folder, recursively, as names below the evidence (`prefix` is the folder's own name): README.md,
/// write_goldens.py and kept .python files are not goldens; a proxy-* folder is one golden, its contents are not
/// walked. Adds to `files`; true when a proxy-* folder (not a kept proxy-*.python one) was seen.
fn walk_goldens(folder: &Path, prefix: &str, files: &mut Vec<String>) -> bool {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return false;
    };
    let mut proxy = false;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        if path.is_dir() {
            if name.starts_with("proxy-") {
                if !name.ends_with(PYTHON_SUFFIX) {
                    proxy = true;
                    files.push(format!("{prefix}/{name}"));
                }
            } else {
                proxy |= walk_goldens(&path, &format!("{prefix}/{name}"), files);
            }
        } else if name != README && name != WRITER && !name.ends_with(PYTHON_SUFFIX) {
            files.push(format!("{prefix}/{name}"));
        }
    }
    proxy
}

/// Every golden file of the evidence: below golden/ and below the golden/ folder of every folder of the evidence
/// (a recording or not), as `walk_goldens` names them; and whether any proxy-* folder is among them.
fn golden_files(evidence: &Path) -> (Vec<String>, bool) {
    let mut files = Vec::new();
    let mut proxy = walk_goldens(&evidence.join(GOLDEN_DIR), GOLDEN_DIR, &mut files);
    if let Ok(entries) = std::fs::read_dir(evidence) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let gold = entry.path().join(GOLDEN_DIR);
            if name != GOLDEN_DIR && gold.is_dir() {
                proxy |= walk_goldens(&gold, &format!("{name}/{GOLDEN_DIR}"), &mut files);
            }
        }
    }
    (files, proxy)
}

/// Every comparison of the golden table for every recording with a golden/ folder, in PathKey order of their names,
/// plus one skip per recording without a golden/ folder ("no golden/ folder for <slug>") and per rip label without a
/// golden folder ("not covered: no golden for the rip label <slug>/<label>"), the retune report, the round trip
/// against audio-dataset.json and the README's written_by block; one fail per golden file no computation produces
/// (its input is gone: a removed rip label, level folder or recording), and one skip when there is no proxy golden.
pub fn check(ctx: &Ctx, evidence: &Path, text: Option<&Path>) -> Vec<Comparison> {
    if !evidence.is_dir() {
        return vec![skipped("evidence", format!("no evidence clone at {}", evidence.display()))];
    }
    let tree = match DiskTree::open(evidence, EVIDENCE_SHOWN) {
        Ok(tree) => tree,
        Err(error) => {
            return vec![Comparison {
                name: "evidence".to_string(),
                outcome: Outcome::Fail(format!("the evidence clone cannot be read: {error}")),
            }];
        }
    };
    let mut out = Vec::new();
    let mut units = Vec::new();
    for slug in recordings(&tree) {
        let gold = evidence.join(&slug).join(GOLDEN_DIR);
        if !gold.is_dir() {
            out.push(skipped(&format!("{slug}/{GOLDEN_DIR}"), format!("no golden/ folder for {slug}")));
            continue;
        }
        let mut uncovered = Vec::new();
        for rip in rip_labels(&tree, &PathKey::new(&slug)) {
            if !gold.join(&rip.label).is_dir() {
                let name = format!("{slug}/{GOLDEN_DIR}/{}", rip.label);
                out.push(skipped(&name, format!("not covered: no golden for the rip label {slug}/{}", rip.label)));
                uncovered.push(format!("{name}/"));
            }
        }
        for unit in recording_units(ctx, &tree, evidence, &slug, text, false) {
            if !unit.names.iter().any(|name| uncovered.iter().any(|prefix| name.starts_with(prefix.as_str()))) {
                units.push(unit);
            }
        }
    }
    units.extend(corpus_units(ctx, &tree));
    let named: BTreeSet<String> = units.iter().flat_map(|unit| unit.names.iter().cloned()).collect();
    let (files, proxy) = golden_files(evidence);
    for name in files.into_iter().filter(|name| !named.contains(name)) {
        let outcome = Outcome::Fail(format!("{name}: no computation produces this golden (its input is gone)"));
        out.push(Comparison { name, outcome });
    }
    if !proxy {
        out.push(skipped(NO_PROXY, "not covered: no proxy golden".to_string()));
    }
    for unit in units {
        compare_unit(evidence, unit, &mut out);
    }
    out.extend(readme_comparison(evidence));
    out.sort_by(|a, b| PathKey::new(&a.name).cmp(&PathKey::new(&b.name)));
    out
}

/// The summary line ("golden check: <p> pass, <f> fail, <s> skipped") and the exit code: 1 on any fail, 3 when
/// nothing could run (no pass and no fail), 0 otherwise.
pub fn summary(comparisons: &[Comparison]) -> (String, i32) {
    let pass = comparisons.iter().filter(|c| c.outcome == Outcome::Pass).count();
    let fail = comparisons.iter().filter(|c| matches!(c.outcome, Outcome::Fail(_))).count();
    let skipped = comparisons.len() - pass - fail;
    let code = if fail > 0 {
        1
    } else if pass == 0 {
        3
    } else {
        0
    };
    (format!("golden check: {pass} pass, {fail} fail, {skipped} skipped"), code)
}

/// A golden file as write_goldens.py writes it.
fn write_file(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, bytes)
}

fn write_produced(evidence: &Path, name: &str, produced: &Produced) -> std::io::Result<()> {
    let path = evidence.join(name);
    match produced {
        Produced::Json(value) => write_file(&path, format!("{}\n", dumps(value, Style::INDENT1_ASCII)).as_bytes()),
        Produced::Lines(lines) | Produced::Retune(lines) => write_file(&path, lines_text(lines).as_bytes()),
        Produced::Proxy { files, golden } => {
            for (file, bytes) in files {
                write_file(&path.join(file), bytes)?;
            }
            write_file(&path.join(PROXY_GOLDEN), format!("{}\n", dumps(golden, Style::INDENT1_ASCII)).as_bytes())
        }
        Produced::RoundTrip(_) => Ok(()),
    }
}

/// `golden write`: the same files from the Rust (for a named re-recording commit only), every golden of every
/// recording and the retune report, in write_goldens.py's order; a proxy golden only where a golden.json names its
/// proxy. One line per golden: "wrote <name>", "skipped <name>: <reason>" or "failed <name>: <message>"; Err with
/// all the lines when one failed (the others are written). README.md and write_goldens.py are never touched, nor is
/// audio-dataset.json.
pub fn write(ctx: &Ctx, evidence: &Path, text: Option<&Path>) -> Result<Vec<String>, String> {
    if !evidence.is_dir() {
        return Err(format!("no evidence clone at {}", evidence.display()));
    }
    let tree =
        DiskTree::open(evidence, EVIDENCE_SHOWN).map_err(|error| format!("the evidence clone cannot be read: {error}"))?;
    let mut units = Vec::new();
    for slug in recordings(&tree) {
        units.extend(recording_units(ctx, &tree, evidence, &slug, text, true));
    }
    units.extend(corpus_units(ctx, &tree));
    let mut lines = Vec::new();
    let mut failed = false;
    for Unit { names, produce } in units {
        for (name, production) in names.iter().zip(produce()) {
            if name == BASELINE_FILE {
                continue;
            }
            match production {
                Ok(produced) => match write_produced(evidence, name, &produced) {
                    Ok(()) => lines.push(format!("wrote {name}")),
                    Err(error) => {
                        failed = true;
                        lines.push(format!("failed {name}: {error}"));
                    }
                },
                Err(NotProduced::Skipped(reason)) => lines.push(format!("skipped {name}: {reason}")),
                Err(NotProduced::Failed(message)) => {
                    failed = true;
                    lines.push(format!("failed {name}: {message}"));
                }
            }
        }
    }
    if failed {
        Err(lines.join("\n"))
    } else {
        Ok(lines)
    }
}

/// The comparison lines and the summary line; returns the summary's exit code.
fn print_check(out: &mut dyn Write, comparisons: &[Comparison]) -> i32 {
    for comparison in comparisons {
        let _ = writeln!(out, "{}: {}", comparison.name, comparison.outcome);
    }
    let (line, code) = summary(comparisons);
    let _ = writeln!(out, "{line}");
    code
}

/// `golden write|check [--evidence DIR] [--text DIR]`. check: one line per comparison, the summary line, exit 0, 1 or
/// 3 (no evidence clone: its exit-3 line on stderr, then one skip). write: one line per golden, exit 0; 1 when a
/// golden failed; 3 when nothing was written or there is no evidence clone. 2 for an argument error.
pub fn run(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let mut parsed = Args::new(args, &["--evidence", "--text"]);
    let evidence_flag = match parsed.flag("--evidence") {
        Ok(value) => value,
        Err(message) => return usage_error(err, "golden", USAGE, &message),
    };
    let text_flag = match parsed.flag("--text") {
        Ok(value) => value,
        Err(message) => return usage_error(err, "golden", USAGE, &message),
    };
    let words = match parsed.positionals() {
        Ok(words) => words,
        Err(message) => return usage_error(err, "golden", USAGE, &message),
    };
    let checking = match words.as_slice() {
        [] => return usage_error(err, "golden", USAGE, "the following arguments are required: action"),
        [word] if word == "check" => true,
        [word] if word == "write" => false,
        [word] => {
            let message = format!("argument action: invalid choice: {} (choose from 'write', 'check')", py_repr_str(word));
            return usage_error(err, "golden", USAGE, &message);
        }
        [_, rest @ ..] => {
            return usage_error(err, "golden", USAGE, &format!("unrecognized arguments: {}", rest.join(" ")));
        }
    };
    let text: Option<PathBuf> = ocr_text_dir(ctx, text_flag.as_deref());
    let evidence = match resolve_evidence(ctx, evidence_flag.as_deref()) {
        Ok(evidence) => evidence,
        Err(refused) => {
            let _ = writeln!(err, "{}", refused.line);
            if !checking || refused.code != 3 {
                return refused.code;
            }
            let shown = refused.line.split(": no such folder").next().unwrap_or_default();
            return print_check(out, &[skipped("evidence", format!("no evidence clone at {shown}"))]);
        }
    };
    if checking {
        return print_check(out, &check(ctx, &evidence, text.as_deref()));
    }
    match write(ctx, &evidence, text.as_deref()) {
        Ok(lines) => {
            for line in &lines {
                let _ = writeln!(out, "{line}");
            }
            if lines.iter().any(|line| line.starts_with("wrote ")) {
                0
            } else {
                3
            }
        }
        Err(report) => {
            let _ = writeln!(err, "{report}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    //! The parts of golden check and golden write that need no evidence clone. The clone-shaped tests (a fresh clone,
    //! the proxy skips, write then check, the real clone) are in crates/dev/tests/golden.rs.
    use super::*;
    use serde_json::json;

    fn words(text: &[&str]) -> Vec<String> {
        text.iter().map(|word| word.to_string()).collect()
    }

    #[test]
    fn outcomes_print_as_the_check_prints_them() {
        assert_eq!(Outcome::Pass.to_string(), "pass");
        assert_eq!(Outcome::Fail("/a: 1 != 2".to_string()).to_string(), "fail: /a: 1 != 2");
        assert_eq!(Outcome::Skipped("ffmpeg is not on PATH".to_string()).to_string(), "skipped: ffmpeg is not on PATH");
    }

    #[test]
    fn the_summary_counts_each_outcome_and_gives_the_exit_code() {
        let c = |outcome: Outcome| Comparison { name: "x".to_string(), outcome };
        let all = [c(Outcome::Pass), c(Outcome::Fail("d".to_string())), c(Outcome::Skipped("r".to_string()))];
        assert_eq!(summary(&all), ("golden check: 1 pass, 1 fail, 1 skipped".to_string(), 1));
        let some = [c(Outcome::Pass), c(Outcome::Skipped("r".to_string()))];
        assert_eq!(summary(&some), ("golden check: 1 pass, 0 fail, 1 skipped".to_string(), 0));
        assert_eq!(summary(&[c(Outcome::Skipped("r".to_string()))]), ("golden check: 0 pass, 0 fail, 1 skipped".to_string(), 3));
        assert_eq!(summary(&[]), ("golden check: 0 pass, 0 fail, 0 skipped".to_string(), 3));
        assert_eq!(first_differences(&words(&["a", "b", "c", "d", "e"])), "a; b; c; and 2 more");
        assert_eq!(first_differences(&words(&["a"])), "a");
    }

    #[test]
    fn retune_diff_exempts_the_three_state_lines_and_nothing_else() {
        let golden = words(&[
            "audio dataset: corpus/owner: 1 disc, 1 title, 1 level folder",
            "rule: version 1, thresholds and constants as in the code",
            "measures: 0 analysed now, 9 taken from the cache (the snapshot, or this run's baseline)",
            "Changes since the previous snapshot (2026-10-13T14:16:49, code 3b6278d937d9):",
            "  none",
            "",
            "snapshot: $TMPDIR/lazymkv-golden-retune.json",
            "RESULT ok",
        ]);
        let mut ours = golden.clone();
        ours[2] = "measures: 9 analysed now, 0 taken from the cache (the snapshot, or this run's baseline)".to_string();
        ours[3] = "Changes since the previous snapshot (2026-10-14T09:00:00, rule version 1):".to_string();
        ours[6] = "snapshot: /elsewhere/audio-dataset.json (unchanged but for its time: not rewritten)".to_string();
        assert_eq!(retune_diff(&golden, &ours), Vec::<String>::new());
        ours[1] = "rule: version 2, thresholds and constants as in the code".to_string();
        ours[4] = "  gone: 1 track(s) not measured now: x".to_string();
        ours.push("extra".to_string());
        assert_eq!(
            retune_diff(&golden, &ours),
            words(&[
                "line 2: 'rule: version 1, thresholds and constants as in the code' != 'rule: version 2, thresholds and constants as in the code'",
                "line 5: '  none' != '  gone: 1 track(s) not measured now: x'",
                "line 9: (absent) != 'extra'",
            ])
        );
        // A state line against another kind of line is a difference.
        let mut swapped = golden.clone();
        swapped[2] = "RESULT ok".to_string();
        assert_eq!(retune_diff(&golden, &swapped).len(), 1);
    }

    #[test]
    fn snapshot_diff_ignores_what_the_key_formula_changed_and_compares_the_cache_as_a_multiset() {
        let baseline = json!({
            "format": "lazymkv audio dataset snapshot v2", "created": "2026-10-13T14:16:49",
            "code": {"audio_similarity_sha1": "3b6278d937d9"},
            "tracks": [{"stream": 2, "verdict": "keep", "key": "aaa", "peer": {"stream": 3, "key": "bbb"},
                        "measures": {"smi": 0.5, "dc": 0.25}}],
            "measures_cache": {"aaa": {"dc": 0.25, "smi": 0.5}, "bbb": {"dc": null, "smi": 0.75}}
        });
        let ours = json!({
            "format": "lazymkv audio dataset snapshot v3", "created": "2026-10-14T09:00:00",
            "code": {"rule_version": 1},
            "tracks": [{"stream": 2, "verdict": "keep", "key": "ccc", "peer": {"stream": 3, "key": "ddd"},
                        "measures": {"smi": 0.5000004, "dc": 0.25}}],
            "measures_cache": {"ddd": {"dc": null, "smi": 0.75}, "ccc": {"dc": 0.25, "smi": 0.5}}
        });
        assert_eq!(snapshot_diff(&baseline, &ours), Vec::<String>::new());
        let mut changed = ours.clone();
        changed["tracks"][0]["verdict"] = json!("commentary");
        changed["measures_cache"]["ccc"]["dc"] = json!(0.26);
        assert_eq!(
            snapshot_diff(&baseline, &changed),
            words(&[
                "/tracks/0/verdict: \"keep\" != \"commentary\"",
                "/measures_cache: 1 measure(s) of the baseline have no equal here, 1 measure(s) here have none in the baseline",
            ])
        );
    }

    #[test]
    fn the_written_by_block_is_read_from_the_readme() {
        let readme = concat!(
            "# Goldens\n\nSome text.\n\n```json written_by\n{\n \"pull_request\": 2,\n",
            " \"branch_commit\": \"0123456789abcdef0123456789abcdef01234567\",\n",
            " \"scripts_phase0_tree\": \"89abcdef0123456789abcdef0123456789abcdef\",\n",
            " \"evidence_commit\": \"fedcba9876543210fedcba9876543210fedcba98\",\n",
            " \"python\": \"3.9.6\",\n",
            " \"ffmpeg\": \"ffmpeg version 0.0-test\",\n",
            " \"inputs\": {\n  \"disc-a/scan.txt\": \"da39a3ee5e6b4b0d3255bfef95601890afd80709\"\n }\n}\n```\n\nMore text.\n",
        );
        let written = read_written_by(readme).unwrap();
        assert_eq!(written.pull_request, 2);
        assert_eq!(written.python, "3.9.6");
        assert_eq!(written.ffmpeg, "ffmpeg version 0.0-test");
        assert_eq!(
            written.inputs.get("disc-a/scan.txt").map(String::as_str),
            Some("da39a3ee5e6b4b0d3255bfef95601890afd80709")
        );
        assert_eq!(read_written_by("# Goldens\n"), Err("no ```json written_by block".to_string()));
        assert_eq!(read_written_by("```json written_by\n{}\n"), Err("the ```json written_by block is not closed".to_string()));
        let error = read_written_by("```json written_by\n{}\n```\n").unwrap_err();
        assert!(error.starts_with("the written_by block is not WrittenBy: missing field `pull_request`"), "{error}");
    }

    #[test]
    fn golden_text_files_hold_every_line_with_a_newline() {
        for case in [vec![], vec![""], vec!["a", "", "b"], vec!["  veto: none (row 0, D21: x)"]] {
            let lines = words(&case);
            assert_eq!(golden_lines(&lines_text(&lines)), lines);
        }
        assert_eq!(lines_text(&words(&["a", "b"])), "a\nb\n");
        assert_eq!(
            line_diff(&words(&["a", "b"]), &words(&["a", "c", "d"])),
            words(&["line 2: 'b' != 'c'", "line 3: (absent) != 'd'"])
        );
    }

    #[test]
    fn a_rebuilt_proxy_folder_is_compared_byte_for_byte() {
        let dir = std::env::temp_dir().join(format!("lazymkv-test-golden-proxy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("golden.json"), "{}\n").unwrap();
        std::fs::write(dir.join("main-mix.txt"), "# header\r\n-23.5\r\n").unwrap();
        std::fs::write(dir.join("main-mix.txt.python"), "kept, never compared\n").unwrap();
        std::fs::write(dir.join("manifest.txt"), "# lazymkv audio-levels v2 (experiment 13)\n").unwrap();
        let same = vec![
            ("main-mix.txt".to_string(), b"# header\r\n-23.5\r\n".to_vec()),
            ("manifest.txt".to_string(), b"# lazymkv audio-levels v2 (experiment 13)\n".to_vec()),
        ];
        assert_eq!(proxy_diff(&dir, &same), Vec::<String>::new());
        let other = vec![
            ("cand-2.txt".to_string(), b"# header\r\n".to_vec()),
            ("main-mix.txt".to_string(), b"# header\r\n-23.4\r\n".to_vec()),
        ];
        assert_eq!(
            proxy_diff(&dir, &other),
            words(&[
                "cand-2.txt: rebuilt, but not in the golden",
                "main-mix.txt: 17 bytes in the golden, 17 rebuilt, first difference at byte 14",
                "manifest.txt: in the golden, not rebuilt",
            ])
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_command_takes_write_or_check_and_nothing_else() {
        let ctx = Ctx::for_test(Path::new("."));
        let cases: [(&[&str], &str); 5] = [
            (&[], "the following arguments are required: action"),
            (&["verify"], "argument action: invalid choice: 'verify' (choose from 'write', 'check')"),
            (&["check", "extra"], "unrecognized arguments: extra"),
            (&["check", "--bogus"], "unrecognized arguments: --bogus"),
            (&["check", "--text"], "argument --text: expected one argument"),
        ];
        for (args, message) in cases {
            let (mut out, mut err): (Vec<u8>, Vec<u8>) = (Vec::new(), Vec::new());
            assert_eq!(run(&ctx, &words(args), &mut out, &mut err), 2, "{args:?}");
            assert_eq!(
                String::from_utf8(err).unwrap(),
                format!(
                    "usage: lazymkv-dev golden write|check [--evidence DIR] [--text DIR]\nlazymkv-dev golden: error: {message}\n"
                )
            );
            assert!(out.is_empty());
        }
    }

    #[test]
    fn golden_check_without_the_clone_is_one_skip_and_exit_3() {
        let ctx = Ctx::for_test(Path::new("."));
        let missing = std::env::temp_dir().join(format!("lazymkv-test-golden-no-clone-{}", std::process::id()));
        let shown = missing.display().to_string();
        let (mut out, mut err): (Vec<u8>, Vec<u8>) = (Vec::new(), Vec::new());
        assert_eq!(run(&ctx, &words(&["check", "--evidence", shown.as_str()]), &mut out, &mut err), 3);
        assert_eq!(
            String::from_utf8(out).unwrap(),
            format!("evidence: skipped: no evidence clone at {shown}\ngolden check: 0 pass, 0 fail, 1 skipped\n")
        );
        assert_eq!(
            String::from_utf8(err).unwrap(),
            format!(
                "{shown}: no such folder; the evidence repository lazymkv-evidence is not cloned there \
                 (git clone https://github.com/krcm0209/lazymkv-evidence {shown})\n"
            )
        );
        let (mut out, mut err): (Vec<u8>, Vec<u8>) = (Vec::new(), Vec::new());
        assert_eq!(run(&ctx, &words(&["write", "--evidence", shown.as_str()]), &mut out, &mut err), 3);
        assert!(out.is_empty());
        assert_eq!(
            check(&ctx, &missing, None),
            vec![Comparison {
                name: "evidence".to_string(),
                outcome: Outcome::Skipped(format!("no evidence clone at {shown}")),
            }]
        );
        assert_eq!(write(&ctx, &missing, None), Err(format!("no evidence clone at {shown}")));
    }

    #[test]
    fn a_golden_no_computation_produces_fails_with_its_name() {
        use crate::testutil::{Scratch, ScratchExt};
        let dir = Scratch::new("golden-orphan");
        dir.write("disc-z/notes.txt", "a recording folder without a rip label\n");
        dir.write("disc-z/golden/all-tracks/reconcile.json", "{}\n");
        dir.write("golden/stale.txt", "x\n");
        dir.write("golden/README.md", "not a golden\n");
        dir.write("golden/write_goldens.py", "# not a golden\n");
        dir.write("golden/retune-report.txt.python", "kept, never read\n");
        let ctx = Ctx::for_test(Path::new("."));
        let gone = |name: &str| Comparison {
            name: name.to_string(),
            outcome: Outcome::Fail(format!("{name}: no computation produces this golden (its input is gone)")),
        };
        let not_covered = |name: &str| Comparison {
            name: name.to_string(),
            outcome: Outcome::Skipped(format!("not covered: {name} is not in the goldens")),
        };
        let comparisons = check(&ctx, &dir.0, None);
        assert_eq!(
            comparisons,
            vec![
                Comparison {
                    name: NO_PROXY.to_string(),
                    outcome: Outcome::Skipped("not covered: no proxy golden".to_string()),
                },
                not_covered("audio-dataset.json"),
                gone("disc-z/golden/all-tracks/reconcile.json"),
                not_covered("disc-z/golden/labels.json"),
                Comparison {
                    name: "golden/README.md".to_string(),
                    outcome: Outcome::Fail(
                        "corpus/owner/golden/README.md: no ```json written_by block".to_string()
                    ),
                },
                not_covered("golden/retune-report.txt"),
                gone("golden/stale.txt"),
            ]
        );
        assert_eq!(summary(&comparisons), ("golden check: 0 pass, 3 fail, 4 skipped".to_string(), 1));
    }

    #[test]
    fn without_a_proxy_golden_one_line_says_the_proxy_row_is_not_covered() {
        use crate::testutil::{Scratch, ScratchExt};
        let ctx = Ctx::for_test(Path::new("."));
        let proxy_line = |root: &Path| -> Vec<Comparison> {
            check(&ctx, root, None).into_iter().filter(|c| c.name == NO_PROXY).collect()
        };
        let dir = Scratch::new("golden-no-proxy");
        dir.write("disc-z/notes.txt", "x\n");
        dir.write("disc-z/golden/proxy-audio-levels-20ms-1.python/main-mix.txt", "kept, never read\n");
        assert_eq!(
            proxy_line(&dir.0),
            vec![Comparison {
                name: "*/golden/proxy-*".to_string(),
                outcome: Outcome::Skipped("not covered: no proxy golden".to_string()),
            }]
        );
        dir.write(
            "disc-z/golden/proxy-audio-levels-20ms-1/golden.json",
            r#"{"slug": "disc-z", "label": "all-tracks", "folder": "audio-levels-20ms-1", "main": 1, "cands": [],
 "ffmpeg_version": "ffmpeg version 0.0-test", "proxy_sha1": "0000000000000000000000000000000000000000"}"#,
        );
        assert_eq!(proxy_line(&dir.0), Vec::<Comparison>::new());
        let (files, proxy) = golden_files(&dir.0);
        assert!(proxy);
        assert_eq!(files, vec!["disc-z/golden/proxy-audio-levels-20ms-1".to_string()]);
    }
}
