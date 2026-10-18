//! `lazymkv-dev decide (<slug> <label> | --rec DIR) [--text DIR] [--selection all|eng] [--settings FILE]
//! [--evidence DIR]` (port spec 3.4; `--settings FILE`, spec 14's settings for the plan and decision sections, is phase
//! 1's): the first form of the design spec's phase 1 command that reads a recording and prints its decisions.
//! For one rip label it prints one JSON document, `DecideDoc`: `reconcile` (the scan's streams against
//! mkvmerge.json, spec 3.3), `audio` (the content rule over every audio-levels-20ms-* folder of the label, or why a
//! folder was skipped), `subtitles` (the text rule over the title's OCR text, or why it was skipped) and `reviews`
//! (the review reasons of the audio verdicts and the subtitle decisions; the reconciler raises none). No Python had
//! this command: each section is a function an earlier task ported (reconcile; results and folder_json; subs_run
//! and subs_json), run on the inputs spec 3.3's `recording` derives (the title and the selection from the rip's
//! summary, the selection from the profile's file name split on '\'). Exit 0; 1 when a reconcile verdict is no; 2
//! for an argument error, an unreadable scan.txt or mkvmerge.json, or a label that is not a rip label; 3 when a
//! slug is given and the evidence clone is absent. A level folder or OCR text that is absent never changes the exit
//! code: its section says why it was skipped.
//!
//! Phase 1 (RP14, design spec 6.4 step 11) appends two sections after `reviews` and leaves the six above, and their
//! goldens, byte-identical: `plan`, the document `lazymkv-dev plan` prints for the recording (`cmd::plan::plan_doc`
//! with the settings of `--settings FILE`, spec 14's defaults without it), and `decision`, the file phase of the rip
//! label's title (`decision_doc`): the rip
//! verified against the scan (`verify::verify_rip`, spec 6.2 step 7), the facts read from the label's files and from
//! the audio and subtitles sections above, and `decision::decide`'s FileDecision (tracks, layout, final path, the three
//! confidences, review reasons, notes and disposition). Either is skipped with its reason, and neither changes the exit
//! code or `reviews`.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};

use lazymkv_core::audio::dataset::LEVELS_NOT_HERE;
use lazymkv_core::audio::levels::{parse_proxy, read_manifest};
use lazymkv_core::audio::measures::Constants;
use lazymkv_core::audio::report::{folder_json, FolderJson};
use lazymkv_core::audio::rule::{results, Plain, Rule as AudioRule, Verdict};
use lazymkv_core::decision::{decide as file_decision, same_path_reviews, DecisionInputs, FileDecision};
use lazymkv_core::disc::{self, Disc};
use lazymkv_core::lookup::discdb::{match_titles, DiscDbRecord, DiscDbTitle};
use lazymkv_core::lookup::tmdb::{TmdbIdentity, TmdbRecord};
use lazymkv_core::plan::movie::LibraryFacts;
use lazymkv_core::plan::{Plan, PlannedTitle};
use lazymkv_core::recording::files::{rip_label, FfprobeStreams, MkvmergeIdent, Selection};
use lazymkv_core::recording::{py_repr_str, read_json, read_text, text_mode, Encoding, Errors, PathKey, Tree};
use lazymkv_core::reconcile::{reconcile, Report, RipLog, RIP_LOG_NAMES};
use lazymkv_core::settings::Settings;
use lazymkv_core::subtitle::labels::TEXT_HINT;
use lazymkv_core::subtitle::text::{subs_json, subs_run, Decision as TextDecision, Rule as TextRule, SubsJson};
use lazymkv_core::tracks::audio::AudioFacts;
use lazymkv_core::tracks::subtitle::{step10_runs, SubtitleFacts};
use lazymkv_core::verify::{
    by_scan_stream, failure_reviews, rip_title, selected_streams, stream_of_track, subs_index, subs_streams,
    subtitle_events, verify_rip, VerifyReport,
};

use crate::cmd::plan::{plan_doc, recorded_answers, PlanDoc, PlanOptions, PlanSource};
use crate::evidence::{ocr_text_dir, pathlib_split, read_scan_in, resolve_recording, Ctx, DiskTree, Recording, Refused};
use crate::pyjson::{dumps, Style};
use crate::{usage_error, Args};

/// The command's name as typed.
const NAME: &str = "decide";
/// The usage line, under the command's name: spec 3.4's synopsis with the --evidence flag every slug command takes.
const USAGE: &str = "decide (<slug> <label> | --rec DIR) [--text DIR] [--selection all|eng] [--settings FILE] [--evidence DIR]";
/// What -h prints after the usage line.
const HELP: &str = "
One JSON document for one rip label (port spec 3.4): reconcile (the scan's streams against mkvmerge.json, with
the title and the selection from the rip's summary), audio (the content rule over every audio-levels-20ms-*
folder of the label, or why a folder was skipped), subtitles (the text rule over the title's OCR text, or why it
was skipped), reviews (the review reasons they raise), plan (lazymkv-dev plan's document for the recording) and
decision (the file phase of the label's title: tracks, layout, final path, confidences and disposition); either of
the last two says why it was skipped when it was.

arguments:
  slug                  a recording: a folder under the evidence clone, or a recording folder's path
  label                 a rip label of the recording: a folder with an ok rip-summary.json or
                        extract-summary.json and an mkvmerge.json
  --rec DIR             the rip label folder itself, instead of slug and label (no evidence check)
  --text DIR            the OCR text: DIR/<slug>/<label>/subs-ocr/sub-<n>.ocr.jsonl, else DIR/sub-<n>.ocr.jsonl
                        (default: LAZYMKV_OCR_TEXT)
  --selection {all,eng} the rip's track selection (default: eng for the profile eng-only.mmcp.xml, else all)
  --settings FILE       spec 14's settings for the plan and decision sections: a JSON file naming the keys it
                        changes, such as movies_root and tv_root (default: spec 14's defaults, which set no library
                        root, so the decision has no final path); a file it refuses skips both sections
  --evidence DIR        the evidence clone (default: LAZYMKV_EVIDENCE, else corpus/owner)

exit status: 0; 1 when a reconcile verdict is no; 2 for an argument error, an unreadable scan.txt or
mkvmerge.json, or a label that is not a rip label; 3 when the evidence clone is not there.
";
/// The level folders of a label: the folders the tuning harness measures. Py: audio_dataset.find_folders' glob.
const LEVEL_FOLDERS: &str = "audio-levels-20ms-*";
/// The reading folder decide looks for first under --text DIR, in the goldens' layout DIR/<slug>/<label>/<reading>/
/// (port spec 5): the one ocr-subs.ps1 writes.
const READING: &str = "subs-ocr";
/// The review reasons decide raises (design spec 8.6, 8.7 and 8.8).
const AUDIO_REFERENCE_UNCLEAR: &str = "audio_reference_unclear";
const COMMENTARY_DETECTED: &str = "commentary_detected";
const SUBTITLE_AMBIGUOUS: &str = "subtitle_ambiguous";
/// What a subtitle outcome that carries a review holds: spec87's "(review)", before_yes's "(review: ...)".
const REVIEW_MARK: &str = "(review";
/// The scan's sentence for a title it does not list (core::reconcile's), also used for a negative summary title.
const NO_SUCH_TITLE: &str = "the scan lists no title ";
/// Before the plan section's reason when the decision is skipped because the plan was (RP14).
const NO_PLAN: &str = "no plan: ";
/// The decision section while decide_doc builds the document; decide_doc replaces it before it returns.
const UNDECIDED: &str = "not decided yet";

/// decide's JSON document (spec 3.4). Field order is the output's key order.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DecideDoc {
    pub slug: String,
    pub label: String,
    pub reconcile: Report,
    /// One entry per level folder of the label, in PathKey order.
    pub audio: Vec<AudioEntry>,
    pub subtitles: SubtitleEntry,
    /// Sorted and unique: see audio_reviews and subtitle_reviews. The plan and decision sections add none here: their
    /// review reasons are in their own documents (RP14).
    pub reviews: Vec<String>,
    /// `lazymkv-dev plan`'s document for the recording, or why it was skipped (RP14).
    pub plan: PlanEntry,
    /// The file phase of the label's title, or why it was skipped (RP14, spec 6.4 step 11).
    pub decision: DecisionEntry,
}

/// A level folder's results (the same document as its audio-<folder>.json golden), or why it was skipped (the
/// refusal message, or the Python's "its level files are not here ..." line).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum AudioEntry {
    Results(FolderJson),
    Skipped { folder: String, skipped: String },
}

/// The title's subtitle outcome (the same document as its subs.json golden), or why it was skipped.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum SubtitleEntry {
    Results(SubsJson),
    Skipped { skipped: String },
}

/// The plan section (RP14): the document `lazymkv-dev plan` prints for the recording (`cmd::plan::plan_doc` with
/// decide's settings file, spec 14's defaults without one), or why it was skipped (plan_doc's refusal line, such as a
/// lookup file that exists and does not parse, or a settings file it refuses). A section that can be skipped, as
/// AudioEntry and SubtitleEntry are.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum PlanEntry {
    Results(PlanDoc),
    Skipped { skipped: String },
}

/// The decision section (RP14, spec 6.4 step 11): the file phase of the rip label's title, or why it was skipped (the
/// plan was skipped, the plan does not include the title, an input of the file phase cannot be read, or
/// `decision::decide` refused).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum DecisionEntry {
    Results(DecisionDoc),
    Skipped { skipped: String },
}

/// The file phase for one rip label (RP14, spec 6.4 step 11): the verify report that maps the file onto the scan (spec
/// 6.2 step 7), which scan streams each fact covered, and the FileDecision of `decision::decide`.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DecisionDoc {
    /// The planned title's key (spec 5), `PlannedTitle.key`.
    pub title_key: String,
    /// `verify::verify_rip` of the label's mkvmerge.json against the scan's title, with the rip log.
    pub verify: VerifyReport,
    pub facts: FactsSummary,
    pub decision: FileDecision,
}

/// Which scan streams each fact of the file phase covered, each list sorted.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FactsSummary {
    /// Streams with a packet MD5 from a level folder's manifest.txt or audio-proxy.json (6.4 step 10a, D13).
    pub md5_streams: Vec<u32>,
    /// Audio streams with an ffprobe profile from ffprobe-streams.json (6.4 step 9).
    pub profile_streams: Vec<u32>,
    /// Subtitle streams with event start times from subs/ (6.4 step 10, 8.7 "Events").
    pub event_streams: Vec<u32>,
    /// Streams with a content-rule verdict from the audio section (8.6 "Content", RP7).
    pub verdict_streams: Vec<u32>,
    /// The stream those verdicts were measured against (the level folder's `main=`).
    pub verdict_reference: Option<u32>,
    /// Streams with a text-rule decision from the subtitles section that the decision used (8.7 "Text"): empty where 6.4
    /// step 10 does not run (`tracks::subtitle::step10_runs`: fewer than two English or no-language subtitle streams in
    /// the file), since `decide_subtitles` then leaves the text out.
    pub text_streams: Vec<u32>,
    /// Selected streams MakeMKV removed from the file (`MSG:4047`, 6.2 step 7).
    pub removed: Vec<u32>,
}

/// Review reasons of one folder, sorted and unique: "audio_reference_unclear" for a reference-unclear verdict (the
/// veto's and the reverse run's, MEANING), "commentary_detected" for commentary and commentary-cross-language
/// (MEANING's "flagged for review"); notes (possible_commentary, possible_audio_description) are not review
/// reasons, and nor are duplicate, keep and not-judged.
pub fn audio_reviews(folder: &FolderJson) -> Vec<&'static str> {
    let mut out = BTreeSet::new();
    for candidate in &folder.candidates {
        match candidate.verdict {
            Verdict::ReferenceUnclear => {
                out.insert(AUDIO_REFERENCE_UNCLEAR);
            }
            Verdict::Commentary | Verdict::CommentaryCrossLanguage => {
                out.insert(COMMENTARY_DETECTED);
            }
            _ => {}
        }
    }
    out.into_iter().collect()
}

/// "subtitle_ambiguous" when a decision has a review or a before_yes outcome holds "(review".
pub fn subtitle_reviews(subs: &SubsJson) -> Vec<&'static str> {
    let raised = subs.streams.iter().any(|stream| {
        stream.decide.as_ref().map_or(false, |decision| decision.review.is_some())
            || stream.before_yes.as_deref().map_or(false, |outcome| outcome.contains(REVIEW_MARK))
    });
    if raised {
        vec![SUBTITLE_AMBIGUOUS]
    } else {
        Vec::new()
    }
}

/// Everything decide prints for one rip label. The rip label (rip_label: Err code 2 with its reason, which also
/// refuses a folder named golden) gives the title and the selection (its title_of and selection_of); `selection`
/// overrides the profile's. Then <slug>/scan.txt, <label>/mkvmerge.json and the first of RIP_LOG_NAMES beside it,
/// read as compare_tracks.main reads them, go to reconcile (code 2 where it fails: a title the scan lacks); then
/// the audio and subtitle sections, which never fail. `text` is taken against ctx.cwd when relative. Last the plan
/// section (`plan_doc` with `settings`, decide's --settings FILE, which plan_doc takes against ctx.cwd; spec 14's
/// defaults without it) and the decision section (`decision_doc` over the document built so far), which never fail
/// either: each is skipped with its reason (RP14), a refused settings file included.
pub fn decide_doc(
    ctx: &Ctx,
    rec: &Recording,
    label: &str,
    text: Option<&Path>,
    selection: Option<Selection>,
    settings: Option<&Path>,
) -> Result<DecideDoc, Refused> {
    let refused = |line: String| Refused { code: 2, line };
    let tree: &dyn Tree = &rec.tree;
    let rip = rip_label(tree, &rec.dir, label).map_err(refused)?;
    let label_dir = rec.dir.join(label);
    let selection = selection.unwrap_or(rip.selection);
    let title = u32::try_from(rip.title).map_err(|_| refused(format!("{}{}", NO_SUCH_TITLE, rip.title)))?;
    let scan = read_scan_in(tree, &rec.dir.join("scan.txt"), Encoding::Utf8, Errors::Replace).map_err(refused)?;
    let ident = read_ident(tree, &label_dir.join("mkvmerge.json")).map_err(refused)?;
    let rip = read_rip_log(tree, &label_dir).map_err(refused)?;
    let rip_log = rip.as_ref().map(|(path, text)| RipLog { shown: tree.display(path), text: text.as_str() });
    let report = reconcile(&scan, title, &ident, selection, rip_log).map_err(refused)?;
    let audio = audio_section(rec, label);
    let text = text.map(|dir| ctx.cwd.join(dir));
    let subtitles = subtitle_section(rec, label, text.as_deref());
    let mut reviews: BTreeSet<&'static str> = BTreeSet::new();
    for entry in &audio {
        if let AudioEntry::Results(folder) = entry {
            reviews.extend(audio_reviews(folder));
        }
    }
    if let SubtitleEntry::Results(subs) = &subtitles {
        reviews.extend(subtitle_reviews(subs));
    }
    let options = PlanOptions { settings: settings.map(Path::to_path_buf), ..PlanOptions::default() };
    let plan = match plan_doc(ctx, &PlanSource::Recording(rec), &options) {
        Ok(found) => PlanEntry::Results(found),
        Err(refused) => PlanEntry::Skipped { skipped: refused.line },
    };
    let mut doc = DecideDoc {
        slug: rec.slug.clone(),
        label: label.to_string(),
        reconcile: report,
        audio,
        subtitles,
        reviews: reviews.into_iter().map(str::to_string).collect(),
        plan,
        decision: DecisionEntry::Skipped { skipped: UNDECIDED.to_string() },
    };
    let decision = match &doc.plan {
        PlanEntry::Results(found) => match decision_doc(ctx, rec, label, found, &doc) {
            Ok(decided) => DecisionEntry::Results(decided),
            Err(why) => DecisionEntry::Skipped { skipped: why },
        },
        PlanEntry::Skipped { skipped } => DecisionEntry::Skipped { skipped: format!("{}{}", NO_PLAN, skipped) },
    };
    doc.decision = decision;
    Ok(doc)
}

/// mkvmerge.json: Err is the read or decode text, or "<shown>: <serde_json's message>" when it is not the JSON
/// compare_tracks reads (as Task 3's command words it).
fn read_ident(tree: &dyn Tree, path: &PathKey) -> Result<MkvmergeIdent, String> {
    let text = read_text(tree, path, Encoding::Utf8Sig, Errors::Strict)?;
    serde_json::from_str(&text).map_err(|error| format!("{}: {}", tree.display(path), error))
}

/// Every audio-levels-20ms-* folder of the label in PathKey order, as audio_dataset.measure reads them: a folder
/// without manifest.txt but with streams.json is skipped with the "level files are not here" line; any other goes
/// through results with the default rule and constants and no cache (Plain: every measure is computed), and its
/// refusal or read error is its skip reason.
fn audio_section(rec: &Recording, label: &str) -> Vec<AudioEntry> {
    let tree: &dyn Tree = &rec.tree;
    let rule = AudioRule::default();
    let consts = Constants::default();
    let mut out = Vec::new();
    for folder in tree.glob(&rec.dir.join(label), LEVEL_FOLDERS) {
        if !tree.is_dir(&folder) {
            continue;
        }
        let name = folder.name().to_string();
        if !tree.is_file(&folder.join("manifest.txt")) && tree.is_file(&folder.join("streams.json")) {
            let skipped = format!("{}/{}/{}: {}", rec.slug, label, name, LEVELS_NOT_HERE);
            out.push(AudioEntry::Skipped { folder: name, skipped });
            continue;
        }
        match results(tree, &folder, &rule, &consts, &mut Plain { consts: &consts }) {
            Ok(res) => out.push(AudioEntry::Results(folder_json(&res, &name, &rule))),
            Err(why) => out.push(AudioEntry::Skipped { folder: name, skipped: why }),
        }
    }
    out
}

/// Where the OCR text of <slug>/<label> is, and the reading's name: <text>/<slug>/<label>/subs-ocr/ when it is a
/// folder (the goldens' layout), else <text> itself when it is one (subtitle_text.py's --text, the folder holding
/// sub-<n>.ocr.jsonl), named by its last component. None when neither is a folder.
fn reading_folder(text: &Path, slug: &str, label: &str) -> Option<(PathBuf, String)> {
    let nested = text.join(slug).join(label).join(READING);
    if nested.is_dir() {
        return Some((nested, READING.to_string()));
    }
    if text.is_dir() {
        let name = text.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
        return Some((text.to_path_buf(), name));
    }
    None
}

/// The title's subtitle outcome: subs_run over the label folder with the OCR text reading_folder finds, as SubsJson
/// when it exits 0; otherwise skipped with subtitle_text.py's stderr lines joined by "; " (no subs/index.json, no
/// sub-<n>.ocr.jsonl for any stream, or an English or no-language bitmap stream without one). Skipped too without
/// `text`, or when no folder holds the text.
fn subtitle_section(rec: &Recording, label: &str, text: Option<&Path>) -> SubtitleEntry {
    let text = match text {
        Some(text) => text,
        None => return SubtitleEntry::Skipped { skipped: format!("no OCR text {}", TEXT_HINT) },
    };
    let (dir, reading) = match reading_folder(text, &rec.slug, label) {
        Some(found) => found,
        None => return SubtitleEntry::Skipped { skipped: format!("no OCR text: {} {}", text.display(), TEXT_HINT) },
    };
    let shown = dir.display().to_string();
    let text_tree = match DiskTree::open(&dir, &shown) {
        Ok(tree) => tree,
        Err(error) => return SubtitleEntry::Skipped { skipped: format!("{}: {}", shown, error) },
    };
    let rule = TextRule::default();
    let root = PathKey::root();
    let outcome = subs_run(&rec.tree, &rec.dir.join(label), &text_tree, &root, None, rule.CLEAR_MAX_BYTES, &rule);
    if outcome.exit_code == 0 {
        SubtitleEntry::Results(subs_json(&outcome, &reading, &text_tree, &root))
    } else {
        SubtitleEntry::Skipped { skipped: outcome.stderr.join("; ") }
    }
}

/// `--rec DIR`: the rip label folder, as subtitle_text.py's --rec. Its parent is the recording, named as a path (so
/// no evidence check, spec 3.4), and its last component the label, both as pathlib splits the path as given
/// (`pathlib_split`); a DIR whose name is empty, "." or ".." is resolved first. Err (exit 2) when DIR is not a folder.
fn rec_folder(ctx: &Ctx, dir: &str) -> Result<(Recording, String), Refused> {
    if !ctx.abs(dir).is_dir() {
        return Err(Refused { code: 2, line: format!("--rec {}: no such folder", dir) });
    }
    let (_, head, name) = pathlib_split(dir);
    let (parent, label) = if name.is_empty() || name == "." || name == ".." {
        let canonical = std::fs::canonicalize(ctx.abs(dir))
            .map_err(|error| Refused { code: 2, line: format!("--rec {}: {}", dir, error) })?;
        let label = canonical.file_name().map(|name| name.to_string_lossy().into_owned());
        let parent = canonical.parent().map(|path| path.display().to_string());
        match (parent, label) {
            (Some(parent), Some(label)) => (parent, label),
            _ => return Err(Refused { code: 2, line: format!("--rec {}: not a rip label folder", dir) }),
        }
    } else {
        (head, name)
    };
    let rec = resolve_recording(ctx, None, &parent)?;
    Ok((rec, label))
}

/// `lazymkv-dev decide`: the arguments, the recording, the document on stdout, the exit code.
pub fn run(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        let _ = writeln!(out, "usage: lazymkv-dev {}", USAGE);
        let _ = write!(out, "{}", HELP);
        return 0;
    }
    let mut parsed = Args::new(args, &["--rec", "--text", "--selection", "--settings", "--evidence"]);
    let rec_dir = match parsed.flag("--rec") {
        Ok(value) => value,
        Err(message) => return usage_error(err, NAME, USAGE, &message),
    };
    let text = match parsed.flag("--text") {
        Ok(value) => value,
        Err(message) => return usage_error(err, NAME, USAGE, &message),
    };
    let selection = match parsed.flag("--selection") {
        Ok(value) => value,
        Err(message) => return usage_error(err, NAME, USAGE, &message),
    };
    let settings = match parsed.flag("--settings") {
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
    let selection = match selection.as_deref() {
        None => None,
        Some(given) => match Selection::parse(given) {
            Some(selection) => Some(selection),
            None => {
                let message =
                    format!("argument --selection: invalid choice: {} (choose from 'all', 'eng')", py_repr_str(given));
                return usage_error(err, NAME, USAGE, &message);
            }
        },
    };
    let (rec, label) = match (rec_dir, words.as_slice()) {
        (Some(_), [_, ..]) => return usage_error(err, NAME, USAGE, "argument --rec: not allowed with argument slug"),
        (Some(dir), []) => match rec_folder(ctx, &dir) {
            Ok(found) => found,
            Err(refused) => {
                let _ = writeln!(err, "lazymkv-dev {}: error: {}", NAME, refused.line);
                return refused.code;
            }
        },
        (None, []) => return usage_error(err, NAME, USAGE, "the following arguments are required: slug, label"),
        (None, [_]) => return usage_error(err, NAME, USAGE, "the following arguments are required: label"),
        (None, [slug, label]) => match resolve_recording(ctx, evidence.as_deref(), slug) {
            Ok(rec) => (rec, label.clone()),
            Err(refused) => {
                let _ = writeln!(err, "{}", refused.line);
                return refused.code;
            }
        },
        (None, [_, _, rest @ ..]) => {
            return usage_error(err, NAME, USAGE, &format!("unrecognized arguments: {}", rest.join(" ")))
        }
    };
    let text = ocr_text_dir(ctx, text.as_deref());
    let doc = match decide_doc(ctx, &rec, &label, text.as_deref(), selection, settings.as_deref().map(Path::new)) {
        Ok(doc) => doc,
        Err(refused) => {
            let _ = writeln!(err, "lazymkv-dev {}: error: {}", NAME, refused.line);
            return refused.code;
        }
    };
    let value = match serde_json::to_value(&doc) {
        Ok(value) => value,
        Err(error) => {
            let _ = writeln!(err, "lazymkv-dev {}: error: {}", NAME, error);
            return 2;
        }
    };
    let _ = writeln!(out, "{}", dumps(&value, Style::INDENT1_ASCII));
    if doc.reconcile.exit_code == 0 {
        0
    } else {
        1
    }
}

/// The content rule's verdicts as the decision takes them (spec 8.6 "Content", RP7): per scan stream, the stream they
/// were measured against, and whether D21's veto held for it.
#[derive(Clone, Debug, Default, PartialEq)]
struct ContentVerdicts {
    by_stream: BTreeMap<u32, Verdict>,
    reference: Option<u32>,
    veto_holds: bool,
}

/// The file phase of one rip label (RP14, spec 6.4 step 11), as phase 2's engine will run it after a Direct rip. The
/// label's title (`doc.reconcile.title`, with the selection decide used) must be one of `plan`'s titles (Err "the plan
/// does not include title <n> (planned: <titles>)"). <slug>/scan.txt is typed (`disc::disc`) and the rip's
/// mkvmerge.json is verified against the scan's title with the rip log (`verify::verify_rip`, spec 6.2 step 7), whose
/// pairs map every file track onto its scan stream. The title is narrowed to the selected streams (`verify::rip_title`,
/// the one copy `retune --subtitles` shares), so a rip with the eng selection is decided over the streams it holds. The facts: every level folder's manifest.txt and
/// audio-proxy.json give the packet MD5s (6.4 step 10a, D13), ffprobe-streams.json the audio profiles (6.4 step 9),
/// subs/ the event start times at `plan.settings.sub_clear_max_bytes` (`verify::subtitle_events`, 8.7), decide's
/// reconcile section the streams MakeMKV removed (its `dropped` rows, `MSG:4047`), decide's audio section the content
/// rule's verdicts against their `main=` stream (RP7), decide's subtitles section the text rule's decisions (8.7
/// "Text", passed through unchanged: `decide_subtitles` uses them only where 6.4 step 10 runs), and the lookup answers as the plan section parsed them (`cmd::plan::recorded_answers`) TheDiscDB's matched
/// title (`lookup::discdb::match_titles`) and the TMDb record that names the file when it is the plan's identity
/// (8.9); the rip's read errors and `verify::failure_reviews(&report, !report.ok)` are carried in (the recorded rip is
/// the last attempt there will be, so a rip that fails verification raises verify_failed, as 6.2 step 7's second
/// failure does). The settings are the plan's; `LibraryFacts` are the defaults (phase 2 reads the library, RP18).
/// `decision::decide` then gives the FileDecision, and `decision::same_path_reviews` runs over the job's decisions as
/// phase 2's engine runs it (decide holds one title, so it adds nothing here). Err with the sentence of an input that
/// cannot be read, or decide's.
/// `_ctx` is not read: every input is under the recording's tree.
pub fn decision_doc(
    _ctx: &Ctx,
    rec: &Recording,
    label: &str,
    plan: &PlanDoc,
    doc: &DecideDoc,
) -> Result<DecisionDoc, String> {
    let tree: &dyn Tree = &rec.tree;
    let label_dir = rec.dir.join(label);
    let number = doc.reconcile.title;
    let planned = planned_title(&plan.plan, number)?;
    let scan = read_scan_in(tree, &rec.dir.join("scan.txt"), Encoding::Utf8, Errors::Replace)?;
    let typed = disc::disc(&scan)?;
    let scanned = typed.title(number)?;
    let selection = selected_streams(&scan, number, doc.reconcile.selection)?;
    let ident = read_ident(tree, &label_dir.join("mkvmerge.json"))?;
    let rip_text = read_rip_log(tree, &label_dir)?.map(|(_, text)| text);
    let report = verify_rip(scanned, &ident, &selection, rip_text.as_deref(), typed.is_dvd);
    let title = rip_title(scanned, &selection);
    let settings = &plan.settings;
    // MSG:4047's removals as decide's own reconcile section counted them (6.2 step 7): not computed a second time.
    let removed: BTreeSet<u32> = doc.reconcile.dropped.iter().map(|row| row.stream).collect();
    let md5 = level_md5s(tree, &label_dir, &report)?;
    let profiles = ffprobe_profiles(tree, &label_dir, &report)?;
    let events = subtitle_starts(tree, &label_dir, &report, settings.sub_clear_max_bytes)?;
    let verdicts = content_verdicts(&doc.audio, &report);
    let text = text_decisions(&doc.subtitles, tree, &label_dir, &report, settings)?;
    // The lookup answers exactly as the plan section parsed them (a non-200 answer is left unparsed there).
    let answers = recorded_answers(rec).map_err(|refused| refused.line)?;
    let record = record_for_naming(plan.plan.identity.tmdb, answers.recorded.clone());
    let (rip_reviews, read_errors) = (failure_reviews(&report, !report.ok), report.read_errors.clone());
    // TheDiscDB's record as plan_doc chose it: the fingerprint's, else the first disc-ID record with titles.
    let discdb = answers
        .by_fingerprint
        .as_ref()
        .or_else(|| answers.by_disc_id.iter().find(|record| !record.titles.is_empty()))
        .and_then(|record| discdb_title(record, &typed, number));
    let facts = FactsSummary {
        md5_streams: md5.keys().copied().collect(),
        profile_streams: profiles.keys().copied().collect(),
        event_streams: events.keys().copied().collect(),
        verdict_streams: verdicts.by_stream.keys().copied().collect(),
        verdict_reference: verdicts.reference,
        text_streams: if step10_runs(&title, &removed) { text.keys().copied().collect() } else { Vec::new() },
        removed: removed.iter().copied().collect(),
    };
    let library = LibraryFacts::default();
    let inputs = DecisionInputs {
        plan: &plan.plan,
        disc: &typed,
        planned,
        title: &title,
        audio: AudioFacts {
            md5,
            ffprobe_profile: profiles,
            discdb,
            verdicts: verdicts.by_stream,
            veto_holds: verdicts.veto_holds,
            verdict_reference: verdicts.reference,
            remembered: None,
            reviewed: None,
        },
        subtitles: SubtitleFacts {
            events,
            removed,
            discdb,
            text: (!text.is_empty()).then_some(&text),
            remembered: None,
            reviewed: None,
        },
        record: record.as_ref(),
        library: &library,
        rip_reviews,
        read_errors,
        overrides: None,
        settings,
    };
    // The job-level step after every title is decided (spec 7, 8.9): decide holds one title, so it never raises here.
    let mut decisions = [file_decision(&inputs)?];
    same_path_reviews(&mut decisions);
    let [decision] = decisions;
    Ok(DecisionDoc { title_key: planned.key.clone(), verify: report, facts, decision })
}

/// Spec 8.9 "Title and year always come from the same TMDb record as the ID": the record names the file only when it
/// is the record of the plan's identity; a record of another identity, or one without an identity in hand, names
/// nothing, so the decision is made without a path (`decision::decide` refuses a record of another identity).
fn record_for_naming(identity: Option<TmdbIdentity>, record: Option<TmdbRecord>) -> Option<TmdbRecord> {
    record.filter(|found| identity == Some(found.identity))
}

/// The text rule's decisions of decide's subtitles section (8.7 "Text"), keyed by scan stream: each stream's `decide`
/// by its subs/index.json `n`, mapped through the index's track ids and the verify report's pairs
/// (`verify::subs_streams`, `verify::by_scan_stream`); a stream without a decision is left out. Empty when the section
/// was skipped (no OCR text), and when `settings.text_rule()` is not `text::Rule::default()`, the rule the section ran
/// with, since its decisions answer other thresholds (spec 14's subtitle settings, RP15). subs/index.json is read by
/// `verify::subs_index`, the reading `verify::subtitle_events` shares. Err when it cannot be read or does not parse.
fn text_decisions(
    subtitles: &SubtitleEntry,
    tree: &dyn Tree,
    label_dir: &PathKey,
    report: &VerifyReport,
    settings: &Settings,
) -> Result<BTreeMap<u32, TextDecision>, String> {
    let SubtitleEntry::Results(subs) = subtitles else {
        return Ok(BTreeMap::new());
    };
    if settings.text_rule() != TextRule::default() {
        return Ok(BTreeMap::new());
    }
    let by_n: BTreeMap<i64, TextDecision> =
        subs.streams.iter().filter_map(|stream| stream.decide.clone().map(|decided| (stream.n, decided))).collect();
    // Read as verify::subtitle_events reads it (invalid bytes replaced), so an index that gave the events never turns
    // the text into an Err that skips the decision.
    let index = subs_index(tree, &label_dir.join("subs"))?;
    Ok(by_scan_stream(&by_n, &subs_streams(&index, report)))
}

/// The content rule's verdicts of decide's audio section, as 8.6 "Content" takes them (RP7): each measured level
/// folder's `main=` stream and candidates (file track numbers, which ffprobe and mkvmerge share) mapped onto scan
/// streams through the verify report's pairs. The first folder whose main stream maps gives the reference; a folder
/// measured against another stream is left out, with its verdicts and its veto, since they answer another reference;
/// within the reference's folders the first verdict of a stream wins, and D21's veto holds when one of them says it
/// holds. A skipped folder, and a folder with no mapped candidate, gives nothing (it does not fix the reference). When
/// no verdict was taken (only main-only folders), there is no reference and no veto: AudioFacts.verdict_reference is
/// None when there are no verdicts, so nothing measured is not judged.
fn content_verdicts(audio: &[AudioEntry], report: &VerifyReport) -> ContentVerdicts {
    let mut found = ContentVerdicts::default();
    for entry in audio {
        let AudioEntry::Results(folder) = entry else {
            continue;
        };
        let Some(main) = folder.main.get("main").and_then(|text| track_stream(text, report)) else {
            continue;
        };
        let mapped: Vec<(u32, Verdict)> = folder
            .candidates
            .iter()
            .filter_map(|candidate| track_stream(&candidate.stream, report).map(|stream| (stream, candidate.verdict)))
            .collect();
        if mapped.is_empty() {
            continue;
        }
        if *found.reference.get_or_insert(main) != main {
            continue;
        }
        found.veto_holds |= folder.veto.holds == Some(true);
        for (stream, verdict) in mapped {
            found.by_stream.entry(stream).or_insert(verdict);
        }
    }
    if found.by_stream.is_empty() {
        found.reference = None;
        found.veto_holds = false;
    }
    found
}

/// The plan's title `number` (Plan.titles, spec 6.1 step 5); Err "the plan does not include title <n> (planned: <the
/// planned titles in plan order, or none>)" for a title the plan left out (a Direct-mode job holds only its planned
/// titles, 8.8).
fn planned_title(plan: &Plan, number: u32) -> Result<&PlannedTitle, String> {
    plan.titles.iter().find(|planned| planned.title == number).ok_or_else(|| {
        let planned: Vec<String> = plan.titles.iter().map(|planned| planned.title.to_string()).collect();
        let list = if planned.is_empty() { "none".to_string() } else { planned.join(", ") };
        format!("the plan does not include title {number} (planned: {list})")
    })
}

/// The rip's MakeMKV log and its path: the first of RIP_LOG_NAMES beside mkvmerge.json, read as compare_tracks.main
/// reads it (UTF-8, invalid bytes replaced, newlines as text mode gives them); None when the label has neither.
fn read_rip_log(tree: &dyn Tree, label_dir: &PathKey) -> Result<Option<(PathKey, String)>, String> {
    match RIP_LOG_NAMES.iter().map(|name| label_dir.join(name)).find(|path| tree.exists(path)) {
        Some(path) => {
            let text = text_mode(&read_text(tree, &path, Encoding::Utf8, Errors::Replace)?);
            Ok(Some((path, text)))
        }
        None => Ok(None),
    }
}

/// A file track number as text ("1", the spelling of manifest.txt and of the audio section) to its scan stream through
/// the verify report's pairs (an ffprobe index of an MKV equals its mkvmerge track id).
fn track_stream(text: &str, report: &VerifyReport) -> Option<u32> {
    text.trim().parse::<i64>().ok().and_then(|track| stream_of_track(report, track))
}

/// The packet MD5 of every audio stream the label's level folders hold (6.4 step 10a, D13), keyed by scan stream: over
/// every audio-levels-20ms-* folder in PathKey order, first the `md5=` of each manifest.txt line with `main=` or
/// `cand=`, then the `source.md5` of each audio-proxy.json track by its `source_stream` (both ffprobe stream numbers of
/// the file, mapped through the verify pairs); the first MD5 of a stream wins; `-` or empty is no MD5. The manifests are
/// git-ignored side files, like the level series, but audio-proxy.json is tracked and holds the same MD5s for every
/// audio stream of the file, so a clone without the level files keeps them. Err when a manifest cannot be read or an
/// audio-proxy.json does not parse.
fn level_md5s(tree: &dyn Tree, label_dir: &PathKey, report: &VerifyReport) -> Result<BTreeMap<u32, String>, String> {
    let mut md5 = BTreeMap::new();
    for folder in tree.glob(label_dir, LEVEL_FOLDERS) {
        if !tree.is_dir(&folder) {
            continue;
        }
        let manifest = read_manifest(tree, &folder)?;
        let main = manifest.main.iter().map(|fields| (fields, "main"));
        let lines = main.chain(manifest.cands.iter().map(|fields| (fields, "cand")));
        for (fields, key) in lines {
            let stream = fields.get(key).and_then(|text| track_stream(text, report));
            first_md5(&mut md5, stream, fields.get("md5").map(|text| text.trim()));
        }
        let proxy_path = folder.join("audio-proxy.json");
        if tree.is_file(&proxy_path) {
            let text = read_text(tree, &proxy_path, Encoding::Utf8Sig, Errors::Strict)?;
            let proxy = parse_proxy(&text).map_err(|error| format!("{}: {}", tree.display(&proxy_path), error))?;
            for track in &proxy.tracks {
                let stream = track.source_stream.as_i64().and_then(|index| stream_of_track(report, index));
                let digest = track.source.as_ref().and_then(|source| source.get("md5")).and_then(|value| value.as_str());
                first_md5(&mut md5, stream, digest.map(str::trim));
            }
        }
    }
    Ok(md5)
}

/// Records `digest` as the MD5 of `stream` unless the stream already has one; a missing stream, a missing digest, an
/// empty one or `-` records nothing.
fn first_md5(md5: &mut BTreeMap<u32, String>, stream: Option<u32>, digest: Option<&str>) {
    let digest = digest.filter(|text| !text.is_empty() && *text != "-");
    if let (Some(stream), Some(digest)) = (stream, digest) {
        md5.entry(stream).or_insert_with(|| digest.to_string());
    }
}

/// ffprobe's profile of every audio stream of the file (6.4 step 9: "DTS-HD MA", "DTS"), from
/// <label>/ffprobe-streams.json, keyed by scan stream; streams without a profile are left out. Empty when the file is
/// absent; Err "<path>: <serde's message>" when it does not parse.
fn ffprobe_profiles(
    tree: &dyn Tree,
    label_dir: &PathKey,
    report: &VerifyReport,
) -> Result<BTreeMap<u32, String>, String> {
    let path = label_dir.join("ffprobe-streams.json");
    if !tree.is_file(&path) {
        return Ok(BTreeMap::new());
    }
    let probe: FfprobeStreams = read_json(tree, &path).map_err(|error| format!("{}: {}", tree.display(&path), error))?;
    Ok(probe
        .streams
        .iter()
        .filter(|stream| stream.codec_type.as_deref() == Some("audio"))
        .filter_map(|stream| {
            let profile = stream.profile.as_deref().filter(|profile| !profile.is_empty())?;
            Some((stream_of_track(report, stream.index)?, profile.to_string()))
        })
        .collect())
}

/// The event start times of every subtitle stream the label's subs/ folder holds (6.4 step 10, 8.7 "Events"), keyed by
/// scan stream: `verify::subtitle_events` at `clear_max_bytes` (spec 14 `sub_clear_max_bytes`). Empty when the label
/// has no subs/index.json (6.4 step 10 skipped, or a title without subtitles).
fn subtitle_starts(
    tree: &dyn Tree,
    label_dir: &PathKey,
    report: &VerifyReport,
    clear_max_bytes: i64,
) -> Result<BTreeMap<u32, Vec<f64>>, String> {
    let subs = label_dir.join("subs");
    if !tree.is_file(&subs.join("index.json")) {
        return Ok(BTreeMap::new());
    }
    subtitle_events(tree, &subs, report, clear_max_bytes)
}

/// The database title matched to scan title `number` (spec 8.3 "Database titles are matched to scan titles",
/// `lookup::discdb::match_titles`), whose track descriptions 8.6 step 1 and 8.7 use; None when none matches.
fn discdb_title<'r>(record: &'r DiscDbRecord, disc: &Disc, number: u32) -> Option<&'r DiscDbTitle> {
    let index = match_titles(record, disc).into_iter().find(|(_, scan)| *scan == number).map(|(index, _)| index)?;
    record.titles.iter().find(|title| title.index == index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    use lazymkv_core::audio::report::CandidateJson;
    use lazymkv_core::audio::rule::{Dialogue, Veto};
    use lazymkv_core::decision::{Confidences, Disposition};
    use lazymkv_core::lookup::tmdb::{parse_recorded, TmdbIdentity, TmdbKind};
    use lazymkv_core::lookup::{DescriptionKind, IdentityStep, Kind};
    use lazymkv_core::naming::NamedPath;
    use lazymkv_core::plan::{DiscDbMatch, Mode};
    use lazymkv_core::recording::Ordered;
    use lazymkv_core::reconcile::Verdicts;
    use lazymkv_core::review::{Note, ReviewReason};
    use lazymkv_core::robot::parse;
    use lazymkv_core::settings::Settings;
    use lazymkv_core::subtitle::text::{Class, Decision, Role, SubsStreamJson};
    use lazymkv_core::testsupport::{commentary, level_file_text, programme, raw_file_text, scan_text};
    use lazymkv_core::tracks::audio::{Alignment, AudioDrop};
    use lazymkv_core::tracks::subtitle::SubtitleDrop;
    use lazymkv_core::tracks::SubtitleRole;
    use lazymkv_core::verify::ReadError;
    use crate::testutil::{sha1_hex, write, Scratch};
    use serde_json::{json, Value};

    /// Title 0 of a made-up Blu-ray scan: video, English 6-channel and stereo audio, Spanish stereo audio, English
    /// PGS subtitles. An eng-only rip keeps streams 0, 1, 2 and 4; the all selection expects stream 3 too.
    const SCAN: &str = r#"TINFO:0,8,0,"2"
TINFO:0,9,0,"0:10:00"
SINFO:0,0,1,6201,"Video"
SINFO:0,0,6,0,"Mpeg4"
SINFO:0,1,1,6202,"Audio"
SINFO:0,1,3,0,"eng"
SINFO:0,1,6,0,"DTS-HD MA"
SINFO:0,1,14,0,"6"
SINFO:0,2,1,6202,"Audio"
SINFO:0,2,3,0,"eng"
SINFO:0,2,6,0,"DD"
SINFO:0,2,14,0,"2"
SINFO:0,3,1,6202,"Audio"
SINFO:0,3,3,0,"spa"
SINFO:0,3,6,0,"DD"
SINFO:0,3,14,0,"2"
SINFO:0,4,1,6203,"Subtitles"
SINFO:0,4,3,0,"eng"
SINFO:0,4,6,0,"PGS"
"#;

    /// MakeMKV's log of the rip: one made-up message, no MSG:4047 removal.
    const RIP: &str = "MSG:5011,0,0,\"made-up line\",\"made-up line\"\n";

    /// The manifest of test_audio_similarity.py's Report tests (MANIFEST with fc=yes, main=aa, cand=bb): main stream
    /// 1 with a centre channel, candidate 3, different packets.
    const MANIFEST: &str = "# lazymkv audio-levels v2 (experiment 13)\n\
        file=x.mkv bytes=1 seconds=1 frame_ms=20 band_hz=300-3400\n\
        main=1 codec=dts profile=DTS-HD_MA channels=6 layout=5.1(side) language=eng fc=yes frames=1 md5=aa\n\
        cand=3 codec=ac3 profile=- channels=2 layout=stereo language=eng frames=1 md5=bb\n";

    /// Made-up English lines for the OCR fixture (no line of any real subtitle).
    const LINES: [&str; 6] = [
        "The cat and the dog went to the shop.",
        "You know that is not what I said.",
        "We have to go back to the house now.",
        "It was the best day of the year.",
        "He said that you would be there.",
        "What do you want me to do with this.",
    ];
    /// A made-up sound cue, put before every third line of stream 1.
    const CUE: &str = "[DOOR CLOSES]";

    /// mkvmerge -J of the eng-only rip of SCAN: video, the two English audio tracks, the English subtitles; two
    /// chapters, 600 s.
    fn ident_json() -> Value {
        json!({
            "tracks": [
                {"id": 0, "type": "video", "codec": "AVC", "properties": {"language": "und"}},
                {"id": 1, "type": "audio", "codec": "DTS-HD MA", "properties": {"language": "eng", "audio_channels": 6}},
                {"id": 2, "type": "audio", "codec": "AC-3", "properties": {"language": "eng", "audio_channels": 2}},
                {"id": 3, "type": "subtitles", "codec": "HDMV PGS", "properties": {"language": "eng"}}
            ],
            "chapters": [{"num_entries": 2}],
            "container": {"properties": {"duration": 600_000_000_000_i64}}
        })
    }

    /// recs/disc-a: scan.txt, and the rip label eng-only (an ok rip-summary.json naming title 0 and a Windows path to
    /// the eng-only profile, mkvmerge.json, rip.txt). Returns the label folder.
    fn recording(root: &Path) -> PathBuf {
        let rec = root.join("recs/disc-a");
        write(&rec.join("scan.txt"), SCAN);
        let label = rec.join("eng-only");
        let summary = json!({"ok": true, "title": 0, "profile": "C:\\made-up\\eng-only.mmcp.xml"});
        write(&label.join("rip-summary.json"), summary.to_string());
        write(&label.join("mkvmerge.json"), ident_json().to_string());
        write(&label.join("rip.txt"), RIP);
        label
    }

    /// The reconcile report decide must give for the recording: reconcile() itself over the same inputs.
    fn expected_reconcile(rip_shown: &str, selection: Selection) -> Report {
        let ident: MkvmergeIdent = serde_json::from_value(ident_json()).unwrap();
        let rip = RipLog { shown: rip_shown.to_string(), text: RIP };
        reconcile(&parse(SCAN), 0, &ident, selection, Some(rip)).unwrap()
    }

    /// Three folders: audio-levels-20ms-1-3 as audio-levels.ps1 writes it (programme(420, 1) as main-mix.txt and
    /// main-fc.txt, commentary(mix, 7) as ffmpeg's raw output for stream 3), audio-levels-20ms-1-3-4 as git keeps
    /// it (streams.json only), and a first-form folder audio-levels-1-3, which is not a level folder of decide.
    fn level_folders(label: &Path) {
        let (mix, centre) = programme(420, 1);
        let full = label.join("audio-levels-20ms-1-3");
        write(&full.join("manifest.txt"), MANIFEST);
        write(&full.join("main-mix.txt"), level_file_text(&mix));
        write(&full.join("main-fc.txt"), level_file_text(&centre));
        write(&full.join("cand-3.txt"), raw_file_text(&commentary(&mix, 7), 0.02));
        write(&label.join("audio-levels-20ms-1-3-4/streams.json"), "{\"streams\": []}\n");
        write(&label.join("audio-levels-1-3/main-mix.txt"), "-20.0\n");
    }

    /// Two English PGS streams of 40 events 3 s apart (a 3000-byte packet, then a 30-byte clear packet) carrying
    /// LINES in turn, stream 1 with CUE before every third line; their OCR text goes to `text` when given, except
    /// for the streams in `missing`. Returns (file name, bytes) of every sub-<n>.ocr.jsonl written.
    fn subtitles(label: &Path, text: Option<&Path>, missing: &[i64]) -> Vec<(String, Vec<u8>)> {
        let index: Vec<Value> = (0..2_i64)
            .map(|n| {
                json!({"n": n, "track_id": 3 + n, "language": "eng", "codec": "HDMV PGS", "track_name": null,
                       "forced": false, "default": n == 0})
            })
            .collect();
        write(&label.join("subs/index.json"), Value::Array(index).to_string());
        let mut written = Vec::new();
        for n in 0..2_i64 {
            let mut packets = Vec::new();
            let mut jsonl = String::new();
            for k in 0..40_i64 {
                let t = 10.0 + 3.0 * k as f64;
                packets.push(json!({"pts_time": format!("{:.6}", t), "duration_time": "2.000000", "size": "3000"}));
                packets.push(json!({"pts_time": format!("{:.6}", t + 2.0), "duration_time": "N/A", "size": "30"}));
                let mut lines = vec![LINES[(k % 6) as usize]];
                if n == 1 && k % 3 == 0 {
                    lines.insert(0, CUE);
                }
                jsonl.push_str(&json!({"frame": k, "t": t, "pts": t, "lines": lines}).to_string());
                jsonl.push('\n');
            }
            write(&label.join(format!("subs/sub-{}.packets.json", n)), json!({"packets": packets}).to_string());
            if let Some(text) = text {
                if !missing.contains(&n) {
                    let name = format!("sub-{}.ocr.jsonl", n);
                    write(&text.join(&name), &jsonl);
                    written.push((name, jsonl.into_bytes()));
                }
            }
        }
        written
    }

    /// decide_doc over recs/disc-a (named by its path, so printed as recs/disc-a/...) with cwd `root` and no settings
    /// file.
    fn doc_of(root: &Path, label: &str, text: Option<&str>, selection: Option<Selection>) -> Result<DecideDoc, Refused> {
        let ctx = Ctx::for_test(root);
        let rec = resolve_recording(&ctx, None, "recs/disc-a").unwrap();
        decide_doc(&ctx, &rec, label, text.map(Path::new), selection, None)
    }

    /// The command with cwd `root` and no environment: (exit code, stdout, stderr).
    fn run_in(root: &Path, args: &[&str]) -> (i32, String, String) {
        let ctx = Ctx::for_test(root);
        let args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
        let (mut out, mut err): (Vec<u8>, Vec<u8>) = (Vec::new(), Vec::new());
        let code = run(&ctx, &args, &mut out, &mut err);
        (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
    }

    fn candidate(stream: &str, verdict: Verdict) -> CandidateJson {
        CandidateJson {
            stream: stream.to_string(),
            duplicate_of: None,
            verdict,
            not_judged: String::new(),
            cross_language: String::new(),
            before_veto: None,
            measures: None,
            reverse: None,
            swapped: None,
            swapped_verdict: None,
            peer: None,
        }
    }

    /// A level folder's document whose candidates 2, 3, ... have these verdicts.
    fn folder(verdicts: &[Verdict]) -> FolderJson {
        FolderJson {
            folder: "audio-levels-20ms-1-2".to_string(),
            main: Ordered::new(),
            frames: 0,
            dialogue: Dialogue::Mix,
            veto: Veto { holds: Some(false), why: "made-up".to_string() },
            veto_line: String::new(),
            candidates: verdicts.iter().enumerate().map(|(k, v)| candidate(&(k + 2).to_string(), *v)).collect(),
        }
    }

    fn stream(n: i64, decide: Option<Decision>, before_yes: Option<&str>) -> SubsStreamJson {
        SubsStreamJson {
            n,
            tag: "eng".to_string(),
            codec: Some("HDMV PGS".to_string()),
            forced: false,
            events: 40,
            status: "read".to_string(),
            pairing: None,
            m: None,
            decide,
            spec87: None,
            before_yes: before_yes.map(str::to_string),
        }
    }

    fn subs(streams: Vec<SubsStreamJson>) -> SubsJson {
        SubsJson {
            reading: "subs-ocr".to_string(),
            exit_code: 0,
            orig_lang: None,
            streams,
            pairs: Vec::new(),
            text_sha1: Ordered::new(),
        }
    }

    fn decision(role: Role, review: Option<&str>) -> Decision {
        Decision { role, review: review.map(str::to_string), note: None }
    }

    #[test]
    fn audio_reviews_name_reference_unclear_and_commentary_only() {
        assert_eq!(audio_reviews(&folder(&Verdict::ALL)), vec!["audio_reference_unclear", "commentary_detected"]);
        assert_eq!(audio_reviews(&folder(&[Verdict::Commentary])), vec!["commentary_detected"]);
        assert_eq!(audio_reviews(&folder(&[Verdict::CommentaryCrossLanguage])), vec!["commentary_detected"]);
        assert_eq!(
            audio_reviews(&folder(&[Verdict::ReferenceUnclear, Verdict::ReferenceUnclear])),
            vec!["audio_reference_unclear"]
        );
        let notes = [
            Verdict::Duplicate,
            Verdict::AdPattern,
            Verdict::NoteCommentary,
            Verdict::NoteAd,
            Verdict::Keep,
            Verdict::NotJudged,
        ];
        assert!(audio_reviews(&folder(&notes)).is_empty());
        assert!(audio_reviews(&folder(&[])).is_empty());
    }

    #[test]
    fn subtitle_reviews_come_from_a_decision_review_or_a_before_yes_review() {
        assert!(subtitle_reviews(&subs(vec![])).is_empty());
        assert!(subtitle_reviews(&subs(vec![stream(0, None, None)])).is_empty());
        let plain = stream(0, Some(decision(Role::Regular, None)), Some("regular"));
        assert!(subtitle_reviews(&subs(vec![plain.clone()])).is_empty());
        let reviewed = stream(1, Some(decision(Role::Keep, Some("subtitle_ambiguous"))), Some("sdh"));
        assert_eq!(subtitle_reviews(&subs(vec![plain, reviewed])), vec!["subtitle_ambiguous"]);
        let today = stream(0, Some(decision(Role::Keep, None)), Some("keep (review)"));
        assert_eq!(subtitle_reviews(&subs(vec![today])), vec!["subtitle_ambiguous"]);
        let disagrees = stream(0, Some(decision(Role::Sdh, None)), Some("regular (review: subtitle_text_disagrees)"));
        assert_eq!(subtitle_reviews(&subs(vec![disagrees])), vec!["subtitle_ambiguous"]);
    }

    #[test]
    fn the_text_folder_is_the_reading_under_slug_and_label_first_then_the_folder_itself() {
        let tmp = Scratch::new("reading");
        let root = tmp.0.clone();
        fs::create_dir_all(root.join("text/disc-a/eng-only/subs-ocr")).unwrap();
        fs::create_dir_all(root.join("ocr-copy")).unwrap();
        assert_eq!(
            reading_folder(&root.join("text"), "disc-a", "eng-only"),
            Some((root.join("text/disc-a/eng-only/subs-ocr"), "subs-ocr".to_string()))
        );
        assert_eq!(
            reading_folder(&root.join("text"), "disc-a", "all-tracks"),
            Some((root.join("text"), "text".to_string()))
        );
        assert_eq!(
            reading_folder(&root.join("ocr-copy"), "disc-a", "eng-only"),
            Some((root.join("ocr-copy"), "ocr-copy".to_string()))
        );
        assert_eq!(reading_folder(&root.join("absent"), "disc-a", "eng-only"), None);
    }

    #[test]
    fn a_rip_label_gives_reconcile_audio_subtitles_and_reviews_in_one_document() {
        let tmp = Scratch::new("full");
        let root = tmp.0.clone();
        let label = recording(&root);
        level_folders(&label);
        let written = subtitles(&label, Some(&root.join("text/disc-a/eng-only/subs-ocr")), &[]);
        let doc = doc_of(&root, "eng-only", Some("text"), None).unwrap();
        assert_eq!((doc.slug.as_str(), doc.label.as_str()), ("disc-a", "eng-only"));
        // Title 0 and the eng selection come from the summary; rip.txt is the log beside mkvmerge.json.
        assert_eq!(doc.reconcile, expected_reconcile("recs/disc-a/eng-only/rip.txt", Selection::Eng));
        assert_eq!(
            doc.reconcile.verdicts,
            Verdicts { count: true, order: true, video: true, chapters: true, duration: true }
        );
        assert_eq!(doc.reconcile.exit_code, 0);
        // The two audio-levels-20ms-* folders in PathKey order; the first-form folder is not one.
        assert_eq!(doc.audio.len(), 2);
        match &doc.audio[0] {
            AudioEntry::Results(folder) => {
                assert_eq!(folder.folder, "audio-levels-20ms-1-3");
                assert_eq!(folder.frames, 21000);
                assert_eq!(folder.dialogue, Dialogue::Fc);
                assert_eq!(
                    folder.veto,
                    Veto { holds: None, why: "not checked: the folder has no streams.json".to_string() }
                );
                let verdicts: Vec<(&str, Verdict)> =
                    folder.candidates.iter().map(|c| (c.stream.as_str(), c.verdict)).collect();
                assert_eq!(verdicts, [("3", Verdict::Commentary)]);
            }
            other => panic!("audio-levels-20ms-1-3 was not analysed: {:?}", other),
        }
        assert_eq!(
            doc.audio[1],
            AudioEntry::Skipped {
                folder: "audio-levels-20ms-1-3-4".to_string(),
                skipped: "disc-a/eng-only/audio-levels-20ms-1-3-4: its level files are not here (git keeps only \
                          streams.json): pull the recording or copy the folder from the dataset share"
                    .to_string(),
            }
        );
        // The subtitles, read from <text>/disc-a/eng-only/subs-ocr/.
        let subs = match &doc.subtitles {
            SubtitleEntry::Results(subs) => subs,
            other => panic!("the subtitles were not read: {:?}", other),
        };
        assert_eq!((subs.reading.as_str(), subs.exit_code, subs.orig_lang.as_deref()), ("subs-ocr", 0, None));
        let both = Decision {
            role: Role::Keep,
            review: Some("subtitle_ambiguous".to_string()),
            note: Some("some SDH markers (14 events, 0 sound-only, 14 naming a sound)".to_string()),
        };
        let pairing = Some("text by order, 40 frames with text, 0 unmatched");
        let rows: Vec<_> = subs
            .streams
            .iter()
            .map(|s| {
                (
                    s.n,
                    s.status.as_str(),
                    s.pairing.as_deref(),
                    s.m.as_ref().map(|m| m.class),
                    s.decide.clone(),
                    s.spec87.as_deref(),
                    s.before_yes.as_deref(),
                )
            })
            .collect();
        assert_eq!(
            rows,
            vec![
                (0, "read", pairing, Some(Class::Regular), Some(both.clone()), Some("sdh"),
                 Some("sdh (review: subtitle_text_disagrees)")),
                (1, "read", pairing, Some(Class::Unclear), Some(both), Some("regular"),
                 Some("regular (review: subtitle_text_disagrees)")),
            ]
        );
        assert_eq!(subs.pairs.len(), 1);
        let hashes: Vec<(String, String)> = subs.text_sha1.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
        let expected: Vec<(String, String)> =
            written.iter().map(|(name, bytes)| (name.clone(), sha1_hex(bytes))).collect();
        assert_eq!(hashes, expected);
        // commentary from the audio, subtitle_ambiguous from the subtitles.
        assert_eq!(doc.reviews, ["commentary_detected", "subtitle_ambiguous"]);
    }

    #[test]
    fn the_document_is_printed_as_json_with_indent_1_and_the_key_order_of_decide_doc() {
        let tmp = Scratch::new("printed");
        let root = tmp.0.clone();
        let label = recording(&root);
        write(&label.join("audio-levels-20ms-1-3-4/streams.json"), "{\"streams\": []}\n");
        let (code, out, err) = run_in(&root, &["recs/disc-a", "eng-only"]);
        assert_eq!((code, err.as_str()), (0, ""));
        assert!(
            out.starts_with(concat!(
                "{\n",
                " \"slug\": \"disc-a\",\n",
                " \"label\": \"eng-only\",\n",
                " \"reconcile\": {\n",
                "  \"title\": 0,\n",
                "  \"selection\": \"eng\",\n",
                "  \"rip_log\": \"recs/disc-a/eng-only/rip.txt\",\n",
                "  \"want\": [\n",
                "   {\n",
                "    \"stream\": 0,\n",
                "    \"type\": \"video\",\n"
            )),
            "{}",
            out
        );
        // decide's own sections are printed as they always were; the plan and decision sections follow (RP14).
        assert!(
            out.contains(concat!(
                " \"audio\": [\n",
                "  {\n",
                "   \"folder\": \"audio-levels-20ms-1-3-4\",\n",
                "   \"skipped\": \"disc-a/eng-only/audio-levels-20ms-1-3-4: its level files are not here (git keeps ",
                "only streams.json): pull the recording or copy the folder from the dataset share\"\n",
                "  }\n",
                " ],\n",
                " \"subtitles\": {\n",
                "  \"skipped\": \"no OCR text (give --text DIR or LAZYMKV_OCR_TEXT)\"\n",
                " },\n",
                " \"reviews\": [],\n",
                " \"plan\": {\n"
            )),
            "{}",
            out
        );
        assert!(out.ends_with("\n }\n}\n"), "{}", out);
        let value: Value = serde_json::from_str(&out).unwrap();
        let keys: Vec<&str> = value.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys, ["slug", "label", "reconcile", "audio", "subtitles", "reviews", "plan", "decision"]);
        let expected = expected_reconcile("recs/disc-a/eng-only/rip.txt", Selection::Eng);
        assert_eq!(value["reconcile"], serde_json::to_value(&expected).unwrap());
        let back: DecideDoc = serde_json::from_str(&out).unwrap();
        assert_eq!(back.reconcile, expected);
        assert_eq!(back.subtitles, SubtitleEntry::Skipped { skipped: "no OCR text (give --text DIR or LAZYMKV_OCR_TEXT)".to_string() });
    }

    #[test]
    fn a_level_folder_without_its_series_or_with_a_wrong_manifest_is_skipped_with_the_reason() {
        let tmp = Scratch::new("audio-skips");
        let root = tmp.0.clone();
        let label = recording(&root);
        write(&label.join("audio-levels-20ms-1-2/manifest.txt"), "# made-up first line\nframe_ms=20\n");
        write(&label.join("audio-levels-20ms-1-2-3/streams.json"), "{\"streams\": []}\n");
        write(&label.join("audio-levels-20ms-1-2-3-4/command.txt"), "made-up\n");
        let doc = doc_of(&root, "eng-only", None, None).unwrap();
        let again = "this folder cannot be analysed: measure the title again with the new audio-levels.ps1, which \
                     writes audio-levels-20ms-<main>-<candidates>";
        assert_eq!(
            doc.audio,
            vec![
                AudioEntry::Skipped {
                    folder: "audio-levels-20ms-1-2".to_string(),
                    skipped: format!(
                        "recs/disc-a/eng-only/audio-levels-20ms-1-2: manifest.txt does not begin with \
                         '# lazymkv audio-levels v2'; {}",
                        again
                    ),
                },
                AudioEntry::Skipped {
                    folder: "audio-levels-20ms-1-2-3".to_string(),
                    skipped: "disc-a/eng-only/audio-levels-20ms-1-2-3: its level files are not here (git keeps \
                              only streams.json): pull the recording or copy the folder from the dataset share"
                        .to_string(),
                },
                AudioEntry::Skipped {
                    folder: "audio-levels-20ms-1-2-3-4".to_string(),
                    skipped: format!(
                        "recs/disc-a/eng-only/audio-levels-20ms-1-2-3-4: no manifest.txt, so it was not written by \
                         the new audio-levels.ps1 (the first measurement's folders, audio-levels-<main>-<cand>, \
                         hold 100 ms full-band levels); {}",
                        again
                    ),
                },
            ]
        );
        assert!(doc.reviews.is_empty());
        assert_eq!(doc.reconcile.exit_code, 0);
    }

    #[test]
    fn without_ocr_text_for_the_title_the_subtitles_are_skipped_with_the_reason() {
        let tmp = Scratch::new("subtitle-skips");
        let root = tmp.0.clone();
        let label = recording(&root);
        // No --text and no LAZYMKV_OCR_TEXT.
        let doc = doc_of(&root, "eng-only", None, None).unwrap();
        assert_eq!(
            doc.subtitles,
            SubtitleEntry::Skipped { skipped: "no OCR text (give --text DIR or LAZYMKV_OCR_TEXT)".to_string() }
        );
        // --text names no folder.
        let doc = doc_of(&root, "eng-only", Some("absent"), None).unwrap();
        let skipped = format!("no OCR text: {} (give --text DIR or LAZYMKV_OCR_TEXT)", root.join("absent").display());
        assert_eq!(doc.subtitles, SubtitleEntry::Skipped { skipped });
        // A text folder, but the label has no subs/index.json: subtitle_text.py's "cannot read the input".
        fs::create_dir_all(root.join("ocr-copy")).unwrap();
        let doc = doc_of(&root, "eng-only", Some("ocr-copy"), None).unwrap();
        assert_eq!(
            doc.subtitles,
            SubtitleEntry::Skipped {
                skipped: "cannot read the input: [Errno 2] No such file or directory: \
                          'recs/disc-a/eng-only/subs/index.json'"
                    .to_string(),
            }
        );
        // The streams are probed, but the text folder holds no sub-<n>.ocr.jsonl of them.
        subtitles(&label, Some(&root.join("elsewhere")), &[]);
        let doc = doc_of(&root, "eng-only", Some("ocr-copy"), None).unwrap();
        let skipped = format!(
            "no sub-<n>.ocr.jsonl in {} for the streams of recs/disc-a/eng-only/subs/index.json",
            root.join("ocr-copy").display()
        );
        assert_eq!(doc.subtitles, SubtitleEntry::Skipped { skipped });
        // An English bitmap stream without its text: subtitle_text.py's input error, one line per stream.
        subtitles(&label, Some(&root.join("ocr-copy")), &[1]);
        let doc = doc_of(&root, "eng-only", Some("ocr-copy"), None).unwrap();
        let skipped = format!(
            "stream s1 eng: no OCR text (no sub-1.ocr.jsonl in {}); read it with ocr-subs.ps1",
            root.join("ocr-copy").display()
        );
        assert_eq!(doc.subtitles, SubtitleEntry::Skipped { skipped });
        assert!(doc.reviews.is_empty());
        assert_eq!(doc.reconcile.exit_code, 0);
    }

    #[test]
    fn a_reconcile_verdict_no_exits_1_and_the_document_is_still_printed() {
        let tmp = Scratch::new("verdict-no");
        let root = tmp.0.clone();
        recording(&root);
        // --selection all overrides the eng-only profile: the scan's Spanish stream is then expected.
        let (code, out, err) = run_in(&root, &["recs/disc-a", "eng-only", "--selection", "all"]);
        assert_eq!((code, err.as_str()), (1, ""));
        let doc: DecideDoc = serde_json::from_str(&out).unwrap();
        assert_eq!(doc.reconcile, expected_reconcile("recs/disc-a/eng-only/rip.txt", Selection::All));
        assert_eq!(
            doc.reconcile.verdicts,
            Verdicts { count: false, order: false, video: true, chapters: true, duration: true }
        );
        assert_eq!(doc.reconcile.exit_code, 1);
    }

    #[test]
    fn a_folder_that_is_not_a_rip_label_exits_2_with_its_reason() {
        let tmp = Scratch::new("not-a-rip-label");
        let root = tmp.0.clone();
        let label = recording(&root);
        // A folder named golden holding what a rip label holds is still not one (Review Focus 2).
        for name in ["rip-summary.json", "mkvmerge.json", "rip.txt"] {
            write(&root.join("recs/disc-a/golden").join(name), fs::read(label.join(name)).unwrap());
        }
        write(&root.join("recs/disc-a/no-file/rip-summary.json"), "{\"ok\": true, \"title\": 0}");
        write(&root.join("recs/disc-a/failed/rip-summary.json"), "{\"ok\": false, \"title\": 0}");
        write(&root.join("recs/disc-a/failed/mkvmerge.json"), ident_json().to_string());
        let cases = [
            ("golden", "golden: the folder name golden is reserved for goldens; it is not a rip label"),
            ("no-file", "no-file: no mkvmerge.json"),
            ("failed", "failed: no rip-summary.json or extract-summary.json saying ok"),
            ("nowhere", "nowhere: no rip-summary.json or extract-summary.json saying ok"),
        ];
        for (name, why) in cases {
            let (code, out, err) = run_in(&root, &["recs/disc-a", name]);
            assert_eq!(
                (code, out.as_str(), err),
                (2, "", format!("lazymkv-dev decide: error: {}\n", why)),
                "{}",
                name
            );
        }
    }

    #[test]
    fn an_unreadable_scan_or_mkvmerge_json_or_a_title_the_scan_lacks_exits_2() {
        let tmp = Scratch::new("unreadable");
        let root = tmp.0.clone();
        let label = recording(&root);
        write(&label.join("mkvmerge.json"), "{");
        let (code, out, err) = run_in(&root, &["recs/disc-a", "eng-only"]);
        assert_eq!(
            (code, out.as_str(), err.as_str()),
            (
                2,
                "",
                "lazymkv-dev decide: error: recs/disc-a/eng-only/mkvmerge.json: EOF while parsing an object at line 1 \
                 column 1\n"
            )
        );
        write(&label.join("mkvmerge.json"), ident_json().to_string());
        write(&label.join("rip-summary.json"), "{\"ok\": true, \"title\": 7}");
        let (code, _, err) = run_in(&root, &["recs/disc-a", "eng-only"]);
        assert_eq!((code, err.as_str()), (2, "lazymkv-dev decide: error: the scan lists no title 7\n"));
        write(&label.join("rip-summary.json"), "{\"ok\": true, \"title\": -1}");
        let (code, _, err) = run_in(&root, &["recs/disc-a", "eng-only"]);
        assert_eq!((code, err.as_str()), (2, "lazymkv-dev decide: error: the scan lists no title -1\n"));
        write(&label.join("rip-summary.json"), "{\"ok\": true, \"title\": 0}");
        fs::remove_file(root.join("recs/disc-a/scan.txt")).unwrap();
        let (code, _, err) = run_in(&root, &["recs/disc-a", "eng-only"]);
        assert_eq!(
            (code, err.as_str()),
            (2, "lazymkv-dev decide: error: [Errno 2] No such file or directory: 'recs/disc-a/scan.txt'\n")
        );
    }

    #[test]
    fn rec_names_the_rip_label_folder_and_gives_the_same_document() {
        let tmp = Scratch::new("rec");
        let root = tmp.0.clone();
        recording(&root);
        let (code, by_slug, err) = run_in(&root, &["recs/disc-a", "eng-only"]);
        assert_eq!((code, err.as_str()), (0, ""));
        for rec in ["recs/disc-a/eng-only", "recs/disc-a/eng-only/", "./recs//disc-a/eng-only"] {
            let (code, out, err) = run_in(&root, &["--rec", rec]);
            assert_eq!((code, out.as_str(), err.as_str()), (0, by_slug.as_str(), ""), "{}", rec);
        }
        let (code, out, err) = run_in(&root, &["--rec", "recs/disc-a/nope"]);
        assert_eq!(
            (code, out.as_str(), err.as_str()),
            (2, "", "lazymkv-dev decide: error: --rec recs/disc-a/nope: no such folder\n")
        );
    }

    #[test]
    fn argument_errors_are_reported_as_argparse_reports_them() {
        let tmp = Scratch::new("arguments");
        let root = tmp.0.clone();
        let usage = "usage: lazymkv-dev decide (<slug> <label> | --rec DIR) [--text DIR] [--selection all|eng] \
                     [--settings FILE] [--evidence DIR]\n";
        let cases: [(&[&str], &str); 8] = [
            (&[], "the following arguments are required: slug, label"),
            (&["recs/disc-a"], "the following arguments are required: label"),
            (&["recs/disc-a", "eng-only", "extra"], "unrecognized arguments: extra"),
            (&["recs/disc-a", "eng-only", "--bogus"], "unrecognized arguments: --bogus"),
            (&["--rec", "recs/disc-a/eng-only", "recs/disc-a"], "argument --rec: not allowed with argument slug"),
            (
                &["recs/disc-a", "eng-only", "--selection", "both"],
                "argument --selection: invalid choice: 'both' (choose from 'all', 'eng')",
            ),
            (&["recs/disc-a", "eng-only", "--text"], "argument --text: expected one argument"),
            (&["recs/disc-a", "eng-only", "--settings"], "argument --settings: expected one argument"),
        ];
        for (args, message) in cases {
            let (code, out, err) = run_in(&root, args);
            assert_eq!(
                (code, out.as_str(), err),
                (2, "", format!("{}lazymkv-dev decide: error: {}\n", usage, message)),
                "{:?}",
                args
            );
        }
        let (code, out, err) = run_in(&root, &["-h"]);
        assert_eq!((code, err.as_str()), (0, ""));
        assert!(out.starts_with(usage), "{}", out);
        assert!(out.contains("  --rec DIR "), "{}", out);
    }

    #[test]
    fn a_slug_is_read_under_the_evidence_clone_or_exits_3_without_it() {
        let tmp = Scratch::new("slug");
        let root = tmp.0.clone();
        write(&root.join("Cargo.toml"), "[workspace]\nmembers = []\n");
        let (code, out, err) = run_in(&root, &["disc-a", "eng-only"]);
        assert_eq!(
            (code, out.as_str(), err.as_str()),
            (
                3,
                "",
                "corpus/owner: no such folder; the evidence repository lazymkv-evidence is not cloned there \
                 (git clone https://github.com/krcm0209/lazymkv-evidence corpus/owner)\n"
            )
        );
        let (code, _, err) = run_in(&root, &["disc-a", "eng-only", "--evidence", "elsewhere"]);
        assert_eq!(
            (code, err.as_str()),
            (
                3,
                "elsewhere: no such folder; the evidence repository lazymkv-evidence is not cloned there \
                 (git clone https://github.com/krcm0209/lazymkv-evidence elsewhere)\n"
            )
        );
        // With the clone, a slug is a folder under it and every path prints under corpus/owner.
        recording(&root);
        fs::create_dir_all(root.join("corpus/owner")).unwrap();
        fs::rename(root.join("recs/disc-a"), root.join("corpus/owner/disc-a")).unwrap();
        let (code, out, err) = run_in(&root, &["disc-a", "eng-only"]);
        assert_eq!((code, err.as_str()), (0, ""));
        let doc: DecideDoc = serde_json::from_str(&out).unwrap();
        assert_eq!(doc.reconcile.rip_log.as_deref(), Some("corpus/owner/disc-a/eng-only/rip.txt"));
        let (code, _, err) = run_in(&root, &["disc-z", "eng-only"]);
        assert_eq!(
            (code, err.as_str()),
            (2, "corpus/owner/disc-z: no such recording (neither a recording folder nor a slug under corpus/owner)\n")
        );
    }

    /// recs/disc-b's streams as (attribute, value) pairs (spec 9.4: 1 the kind, 2 the name, 3 the language, 5 to 7 the
    /// codec, 13 the bit rate, 14 the channels, 17 and 18 the sample rate and size, 19 the video size, 22 the flags, 30
    /// the tree text, 40 the layout), with the values of the Office's title 1 in
    /// corpus/owner/the-office-2005-s3d3-bluray/scan.txt.
    const B_VIDEO: &[(u32, &str)] =
        &[(1, "Video"), (5, "V_MPEG4/ISO/AVC"), (6, "Mpeg4"), (7, "Mpeg4 AVC High@L4.1"), (19, "1920x1080"), (22, "0")];
    const B_HD_MA: &[(u32, &str)] = &[
        (1, "Audio"),
        (2, "Surround 5.1"),
        (3, "eng"),
        (5, "A_DTS"),
        (6, "DTS-HD MA"),
        (7, "DTS-HD Master Audio"),
        (14, "6"),
        (17, "48000"),
        (18, "24"),
        (22, "1024"),
        (40, "5.1(side)"),
    ];
    const B_CORE: &[(u32, &str)] = &[
        (1, "Audio"),
        (2, "Surround 5.1"),
        (3, "eng"),
        (5, "A_DTS"),
        (6, "DTS"),
        (7, "DTS"),
        (13, "1.5 Mb/s"),
        (14, "6"),
        (17, "48000"),
        (22, "2304"),
        (40, "5.1(side)"),
    ];
    const B_PGS: &[(u32, &str)] =
        &[(1, "Subtitles"), (3, "eng"), (5, "S_HDMV/PGS"), (6, "PGS"), (7, "HDMV PGS Subtitles"), (22, "0")];
    const B_FORCED_ONLY: &[(u32, &str)] = &[
        (1, "Subtitles"),
        (3, "eng"),
        (5, "S_HDMV/PGS"),
        (6, "PGS"),
        (7, "HDMV PGS Subtitles"),
        (22, "6144"),
        (30, "PGS English  (forced only)"),
    ];
    const B_STEREO: &[(u32, &str)] = &[
        (1, "Audio"),
        (2, "Stereo"),
        (3, "eng"),
        (5, "A_AC3"),
        (6, "DD"),
        (7, "Dolby Digital"),
        (13, "192 Kb/s"),
        (14, "2"),
        (17, "48000"),
        (22, "0"),
        (40, "stereo"),
    ];
    /// Title 0: 00800.mpls, 50 minutes, 5 chapters, one segment; title 1: 00801.mpls, 3 minutes, 1 chapter.
    const B_TITLE_0: &[(u32, &str)] =
        &[(2, "Made Up Film"), (8, "5"), (9, "0:50:00"), (11, "6000000000"), (16, "00800.mpls"), (25, "1"), (26, "1")];
    const B_TITLE_1: &[(u32, &str)] =
        &[(8, "1"), (9, "0:03:00"), (11, "300000000"), (16, "00801.mpls"), (25, "1"), (26, "2")];
    const B_STREAMS_0: &[(u32, &[(u32, &str)])] = &[
        (0, B_VIDEO),
        (1, B_HD_MA),
        (2, B_CORE),
        (3, B_HD_MA),
        (4, B_CORE),
        (5, B_PGS),
        (6, B_FORCED_ONLY),
    ];
    const B_STREAMS_1: &[(u32, &[(u32, &str)])] = &[(0, B_VIDEO), (1, B_STEREO)];
    /// Two made-up packet MD5s (ruling RP21): TWIN_A for the twin tracks 1 and 3, TWIN_B for their cores 2 and 4.
    const TWIN_A: &str = "0123456789abcdef0123456789abcdef";
    const TWIN_B: &str = "fedcba9876543210fedcba9876543210";

    /// recs/disc-b's scan.txt: CINFO 1 (a Blu-ray), 2 and 32 (the name and the volume label), then the two titles.
    fn disc_b_scan() -> String {
        scan_text(
            &[(1, "Blu-ray disc"), (2, "Made Up Film"), (32, "MADE_UP_FILM")],
            &[(0, B_TITLE_0, B_STREAMS_0), (1, B_TITLE_1, B_STREAMS_1)],
        )
    }

    /// mkvmerge -J of a rip of recs/disc-b: `tracks` as (type, codec, audio channels) with ids 0, 1, 2 and on, every
    /// track English but the video ("und"); one chapter edition of `chapters` entries; the container's duration.
    fn ident_b(tracks: &[(&str, &str, Option<i64>)], chapters: i64, duration_ns: i64) -> String {
        let tracks: Vec<Value> = tracks
            .iter()
            .enumerate()
            .map(|(id, (kind, codec, channels))| {
                let mut properties = json!({"language": if *kind == "video" { "und" } else { "eng" }});
                if let Some(channels) = channels {
                    properties["audio_channels"] = json!(channels);
                }
                json!({"id": id, "type": kind, "codec": codec, "properties": properties})
            })
            .collect();
        json!({
            "tracks": tracks,
            "chapters": [{"num_entries": chapters}],
            "container": {"properties": {"duration": duration_ns}}
        })
        .to_string()
    }

    /// ffprobe-streams.json of a rip of recs/disc-b: the video (index 0), one DTS stream per profile, then `subtitles`
    /// PGS streams, indexed in file order.
    fn ffprobe_b(profiles: &[&str], subtitles: usize) -> String {
        let mut streams = vec![json!({"index": 0, "codec_type": "video", "codec_name": "h264", "profile": "High"})];
        for (k, profile) in profiles.iter().enumerate() {
            streams.push(
                json!({"index": k + 1, "codec_type": "audio", "codec_name": "dts", "profile": profile, "channels": 6}),
            );
        }
        for k in 0..subtitles {
            let index = profiles.len() + 1 + k;
            streams.push(json!({"index": index, "codec_type": "subtitle", "codec_name": "hdmv_pgs_subtitle"}));
        }
        json!({"streams": streams}).to_string()
    }

    /// A level folder's manifest.txt as audio-levels.ps1 writes it: the format line, the file line, then one line per
    /// (role "main" or "cand", ffprobe stream, packet MD5).
    fn manifest_b(lines: &[(&str, u32, &str)]) -> String {
        let mut text = String::from(
            "# lazymkv audio-levels v2 (experiment 13)\nfile=title_t00.mkv bytes=1 seconds=1 frame_ms=20 band_hz=300-3400\n",
        );
        for (role, stream, md5) in lines {
            text.push_str(&format!(
                "{role}={stream} codec=dts channels=6 layout=5.1(side) language=eng frames=1 md5={md5}\n"
            ));
        }
        text
    }

    /// MakeMKV's line for an empty forced-only track it removed, at output index `index` (spec 6.2 step 7, 9.4).
    fn removal(index: u32) -> String {
        format!(
            "MSG:4047,16,3,\"AV sync issue in stream {index} at 0:00:00 : Forced subtitles track #{index} turned out to \
             be empty and was removed from output file\",\"AV sync issue in stream %1 at %2 : %3\",\"{index}\",\
             \"0:00:00\",\"Forced subtitles track #{index} turned out to be empty and was removed from output file\"\n"
        )
    }

    /// The label's subs/: index.json with one English PGS stream per file track of `track_ids`, n 0, 1 and on in that
    /// order, and each stream's 40 events 3 s apart, each a 3000-byte packet followed by a 30-byte clear packet.
    fn subs_b(label: &Path, track_ids: &[i64]) {
        let index: Vec<Value> = track_ids
            .iter()
            .enumerate()
            .map(|(n, track_id)| {
                json!({"n": n, "track_id": track_id, "language": "eng", "codec": "HDMV PGS", "track_name": null,
                       "forced": false, "default": false})
            })
            .collect();
        write(&label.join("subs/index.json"), Value::Array(index).to_string());
        let mut packets = Vec::new();
        for k in 0..40_i64 {
            let t = 10.0 + 3.0 * k as f64;
            packets.push(json!({"pts_time": format!("{:.6}", t), "duration_time": "2.000000", "size": "3000"}));
            packets.push(json!({"pts_time": format!("{:.6}", t + 2.0), "duration_time": "N/A", "size": "30"}));
        }
        for n in 0..track_ids.len() {
            write(&label.join(format!("subs/sub-{n}.packets.json")), json!({"packets": packets}).to_string());
        }
    }

    /// recs/disc-b under `root`: scan.txt and the rip labels all-tracks (title 0, every stream), eng-only (title 0, the
    /// eng profile) and extra (title 1).
    fn disc_b(root: &Path) {
        let rec = root.join("recs/disc-b");
        write(&rec.join("scan.txt"), disc_b_scan());
        let summary = |title: i64, profile: &str| {
            json!({"ok": true, "title": title, "profile": format!("C:\\made-up\\{profile}")}).to_string()
        };
        let video = ("video", "AVC/H.264/MPEG-4p10", None);
        let hd_ma = ("audio", "DTS-HD Master Audio", Some(6));
        let core = ("audio", "DTS", Some(6));
        let pgs = ("subtitles", "HDMV PGS", None);

        let all = rec.join("all-tracks");
        write(&all.join("rip-summary.json"), summary(0, "all-tracks.mmcp.xml"));
        write(&all.join("mkvmerge.json"), ident_b(&[video, hd_ma, core, hd_ma, core, pgs], 5, 3_000_500_000_000));
        write(&all.join("rip.txt"), removal(6));
        write(&all.join("ffprobe-streams.json"), ffprobe_b(&["DTS-HD MA", "DTS", "DTS-HD MA", "DTS"], 1));
        write(
            &all.join("audio-levels-20ms-1-2-3-4/manifest.txt"),
            manifest_b(&[("main", 1, TWIN_A), ("cand", 2, TWIN_B), ("cand", 3, TWIN_A), ("cand", 4, TWIN_B)]),
        );
        subs_b(&all, &[5]);

        let eng = rec.join("eng-only");
        write(&eng.join("rip-summary.json"), summary(0, "eng-only.mmcp.xml"));
        write(&eng.join("mkvmerge.json"), ident_b(&[video, hd_ma, hd_ma, pgs], 5, 3_000_500_000_000));
        write(&eng.join("rip.txt"), removal(4));
        write(&eng.join("ffprobe-streams.json"), ffprobe_b(&["DTS-HD MA", "DTS-HD MA"], 1));
        write(&eng.join("audio-levels-20ms-1-2/manifest.txt"), manifest_b(&[("main", 1, TWIN_A), ("cand", 2, TWIN_A)]));
        subs_b(&eng, &[3]);

        let extra = rec.join("extra");
        write(&extra.join("rip-summary.json"), summary(1, "all-tracks.mmcp.xml"));
        write(&extra.join("mkvmerge.json"), ident_b(&[video, ("audio", "AC-3", Some(2))], 1, 180_200_000_000));
        write(&extra.join("rip.txt"), RIP);
    }

    /// recs/disc-b's rip label forced-kept under `root` (title 0, every stream): MakeMKV kept the derived forced-only
    /// stream 6, so the file holds two English subtitle streams (file tracks 5 and 6, both with events) and 6.4 step 10
    /// runs (`step10_runs`).
    fn forced_kept(root: &Path) {
        let label = root.join("recs/disc-b/forced-kept");
        let summary = json!({"ok": true, "title": 0, "profile": "C:\\made-up\\all-tracks.mmcp.xml"}).to_string();
        write(&label.join("rip-summary.json"), summary);
        let tracks = [
            ("video", "AVC/H.264/MPEG-4p10", None),
            ("audio", "DTS-HD Master Audio", Some(6)),
            ("audio", "DTS", Some(6)),
            ("audio", "DTS-HD Master Audio", Some(6)),
            ("audio", "DTS", Some(6)),
            ("subtitles", "HDMV PGS", None),
            ("subtitles", "HDMV PGS", None),
        ];
        write(&label.join("mkvmerge.json"), ident_b(&tracks, 5, 3_000_500_000_000));
        write(&label.join("rip.txt"), RIP);
        write(&label.join("ffprobe-streams.json"), ffprobe_b(&["DTS-HD MA", "DTS", "DTS-HD MA", "DTS"], 2));
        write(
            &label.join("audio-levels-20ms-1-2-3-4/manifest.txt"),
            manifest_b(&[("main", 1, TWIN_A), ("cand", 2, TWIN_B), ("cand", 3, TWIN_A), ("cand", 4, TWIN_B)]),
        );
        subs_b(&label, &[5, 6]);
    }

    /// decide_doc over recs/disc-b (named by its path) with cwd `root`, no OCR text, the profile's selection and no
    /// settings file (spec 14's defaults).
    fn doc_b(root: &Path, label: &str) -> DecideDoc {
        let ctx = Ctx::for_test(root);
        let rec = resolve_recording(&ctx, None, "recs/disc-b").unwrap();
        decide_doc(&ctx, &rec, label, None, None, None).unwrap()
    }

    /// The decision section's document; panics with the reason when it was skipped.
    fn results_of(entry: &DecisionEntry) -> &DecisionDoc {
        match entry {
            DecisionEntry::Results(found) => found,
            DecisionEntry::Skipped { skipped } => panic!("the decision was skipped: {skipped}"),
        }
    }

    /// The kept subtitle tracks as (scan stream, role, events).
    fn kept_subtitles(decision: &FileDecision) -> Vec<(u32, SubtitleRole, Option<usize>)> {
        decision.subtitles.kept.iter().map(|track| (track.stream, track.role, track.events)).collect()
    }

    /// The scan streams of the final file's tracks, in order (8.10).
    fn layout_streams(decision: &FileDecision) -> Vec<u32> {
        decision.layout.tracks.iter().map(|track| track.stream).collect()
    }

    #[test]
    fn the_plan_section_is_lazymkv_dev_plans_document_for_the_recording() {
        let tmp = Scratch::new("plan-section");
        let root = tmp.0.clone();
        disc_b(&root);
        let ctx = Ctx::for_test(&root);
        let rec = resolve_recording(&ctx, None, "recs/disc-b").unwrap();
        let doc = decide_doc(&ctx, &rec, "all-tracks", None, None, None).unwrap();
        let expected = plan_doc(&ctx, &PlanSource::Recording(&rec), &PlanOptions::default()).unwrap();
        assert_eq!(doc.plan, PlanEntry::Results(expected.clone()));
        // decide plans with spec 14's defaults.
        assert_eq!(expected.settings, Settings::default());
        // Spec 8.1 step 6 (no lookup recorded), 8.2 (a movie by default), 8.3 rule 3.5 (one rival at 0.85), 6.1 step 5's
        // mode at 0.85, spec 5's title key.
        let plan = &expected.plan;
        assert_eq!((plan.identity.step, plan.identity_confidence, plan.identity.tmdb), (IdentityStep::Nothing, 0.0, None));
        assert_eq!(plan.kind, Kind::Movie);
        let titles: Vec<(u32, &str)> = plan.titles.iter().map(|planned| (planned.title, planned.key.as_str())).collect();
        assert_eq!(titles, [(0, "t00-00800")]);
        assert_eq!((plan.title_confidence, plan.mode), (0.85, Mode::Direct));
        assert_eq!(plan.reviews, [ReviewReason::IdentityUnknown]);
    }

    #[test]
    fn the_decision_maps_the_rips_files_onto_the_scan_streams() {
        let tmp = Scratch::new("decision-facts");
        let root = tmp.0.clone();
        disc_b(&root);
        let doc = doc_b(&root, "all-tracks");
        let all = results_of(&doc.decision);
        assert_eq!(all.title_key, "t00-00800");
        assert!(all.verify.ok, "{:?}", all.verify.mismatches);
        // Every selected stream but the removed forced-only one is in the file, track id = position.
        let pairs: Vec<(u32, Option<i64>)> = (0..6).map(|stream| (stream, Some(i64::from(stream)))).collect();
        assert_eq!(all.verify.pairs, pairs);
        assert_eq!(
            all.facts,
            FactsSummary {
                md5_streams: vec![1, 2, 3, 4],
                profile_streams: vec![1, 2, 3, 4],
                event_streams: vec![5],
                verdict_streams: Vec::new(),
                verdict_reference: None,
                text_streams: Vec::new(),
                removed: vec![6],
            }
        );
        // The eng profile left the cores out: file tracks 1, 2 and 3 are scan streams 1, 3 and 5, and MSG:4047's index
        // 4 counts the selection [0, 1, 3, 5, 6], so it is stream 6.
        let doc = doc_b(&root, "eng-only");
        let eng = results_of(&doc.decision);
        assert!(eng.verify.ok, "{:?}", eng.verify.mismatches);
        assert_eq!(eng.verify.pairs, vec![(0, Some(0)), (1, Some(1)), (3, Some(2)), (5, Some(3))]);
        assert_eq!(
            eng.facts,
            FactsSummary {
                md5_streams: vec![1, 3],
                profile_streams: vec![1, 3],
                event_streams: vec![5],
                verdict_streams: Vec::new(),
                verdict_reference: None,
                text_streams: Vec::new(),
                removed: vec![6],
            }
        );
    }

    #[test]
    fn the_twin_tracks_of_a_rip_are_dropped_as_duplicates_by_their_md5() {
        let tmp = Scratch::new("decision-twins");
        let root = tmp.0.clone();
        disc_b(&root);
        let doc = doc_b(&root, "all-tracks");
        let decided = results_of(&doc.decision);
        let decision = &decided.decision;
        assert_eq!(decision.title_key, "t00-00800");
        // 8.6 step 1: the cores are derived (2048); stream 3's packets equal stream 1's (D13): one copy is kept.
        let kept: Vec<(u32, &str)> = decision.audio.kept.iter().map(|track| (track.stream, track.name.as_str())).collect();
        assert_eq!(kept, [(1, "DTS-HD MA 5.1")]);
        assert_eq!(
            decision.audio.dropped,
            vec![(2, AudioDrop::Derived), (3, AudioDrop::Duplicate { of: 1 }), (4, AudioDrop::Derived)]
        );
        assert_eq!(decision.audio.confidence, 0.95);
        // 8.7: the forced-only stream is set aside and MakeMKV removed it as empty; the one full track is "English".
        assert_eq!(kept_subtitles(decision), [(5, SubtitleRole::Regular, Some(40))]);
        assert_eq!(decision.subtitles.dropped, vec![(6, SubtitleDrop::DerivedForcedOnlyEmpty)]);
        assert_eq!(decision.subtitles.confidence, 0.90);
        // 8.8: identity and title from the plan, track the lower of 0.95 and 0.90; no TMDb record, so no path.
        assert_eq!(decision.confidences, Confidences { identity: 0.0, title: 0.85, track: 0.90 });
        assert_eq!(decision.reviews, [ReviewReason::IdentityUnknown]);
        assert_eq!(decision.path, None);
        assert_eq!(decision.disposition, Disposition::NeedsReview);
        assert_eq!(layout_streams(decision), [0, 1, 5]);
    }

    #[test]
    fn a_rip_with_the_eng_selection_is_decided_over_its_selected_streams_only() {
        let tmp = Scratch::new("decision-eng");
        let root = tmp.0.clone();
        disc_b(&root);
        let doc = doc_b(&root, "eng-only");
        let decision = &results_of(&doc.decision).decision;
        // The cores (2, 4) are not in this rip: they are neither kept nor dropped; the twin is dropped by its MD5.
        let kept: Vec<u32> = decision.audio.kept.iter().map(|track| track.stream).collect();
        assert_eq!(kept, [1]);
        assert_eq!(decision.audio.dropped, vec![(3, AudioDrop::Duplicate { of: 1 })]);
        assert_eq!(kept_subtitles(decision), [(5, SubtitleRole::Regular, Some(40))]);
        assert_eq!(decision.subtitles.dropped, vec![(6, SubtitleDrop::DerivedForcedOnlyEmpty)]);
        assert_eq!(layout_streams(decision), [0, 1, 5]);
        // The all-tracks rip of the same title makes the same final file.
        let doc = doc_b(&root, "all-tracks");
        assert_eq!(layout_streams(&results_of(&doc.decision).decision), [0, 1, 5]);
    }

    #[test]
    fn a_title_the_plan_does_not_include_is_skipped_with_the_planned_titles() {
        let tmp = Scratch::new("decision-unplanned");
        let root = tmp.0.clone();
        disc_b(&root);
        let doc = doc_b(&root, "extra");
        assert_eq!(doc.reconcile.title, 1);
        assert!(matches!(doc.plan, PlanEntry::Results(_)), "{:?}", doc.plan);
        assert_eq!(
            doc.decision,
            DecisionEntry::Skipped { skipped: "the plan does not include title 1 (planned: 0)".to_string() }
        );
        assert_eq!(
            serde_json::to_value(&doc.decision).unwrap(),
            json!({"skipped": "the plan does not include title 1 (planned: 0)"})
        );
    }

    #[test]
    fn a_refused_plan_skips_both_new_sections_and_leaves_decides_exit_code() {
        let tmp = Scratch::new("plan-refused");
        let root = tmp.0.clone();
        disc_b(&root);
        // A lookup file that exists and does not parse is the one input plan_doc refuses for a recording (Task 15).
        write(&root.join("recs/disc-b/thediscdb-hash.json"), "{");
        let ctx = Ctx::for_test(&root);
        let rec = resolve_recording(&ctx, None, "recs/disc-b").unwrap();
        let refused = plan_doc(&ctx, &PlanSource::Recording(&rec), &PlanOptions::default()).unwrap_err();
        assert_eq!(refused.code, 2);
        let doc = decide_doc(&ctx, &rec, "all-tracks", None, None, None).unwrap();
        assert_eq!(doc.plan, PlanEntry::Skipped { skipped: refused.line.clone() });
        assert_eq!(doc.decision, DecisionEntry::Skipped { skipped: format!("no plan: {}", refused.line) });
        // decide still prints its document and exits with the reconciler's code.
        let (code, out, err) = run_in(&root, &["recs/disc-b", "all-tracks"]);
        assert_eq!((code, err.as_str()), (0, ""));
        let value: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(value["plan"], json!({"skipped": refused.line}));
        assert_eq!(value["decision"], json!({"skipped": format!("no plan: {}", refused.line)}));
        assert_eq!(value["reviews"], json!([]));
    }

    #[test]
    fn an_unreadable_input_of_the_file_phase_skips_the_decision_with_its_error() {
        let tmp = Scratch::new("decision-unreadable");
        let root = tmp.0.clone();
        disc_b(&root);
        write(&root.join("recs/disc-b/all-tracks/ffprobe-streams.json"), "{");
        let doc = doc_b(&root, "all-tracks");
        assert!(matches!(doc.plan, PlanEntry::Results(_)), "{:?}", doc.plan);
        assert_eq!(
            doc.decision,
            DecisionEntry::Skipped {
                skipped: "recs/disc-b/all-tracks/ffprobe-streams.json: EOF while parsing an object at line 1 column 1"
                    .to_string(),
            }
        );
        assert_eq!(doc.reconcile.exit_code, 0);
    }

    #[test]
    fn an_answer_the_plan_leaves_unread_is_left_unread_by_the_decision() {
        let tmp = Scratch::new("decision-unread-answers");
        let root = tmp.0.clone();
        disc_b(&root);
        // The recorder keeps the body of every status (spec 8.1): TMDb's error body of a 404, and the server's page of a
        // failed TheDiscDB lookup. plan_doc parses an answer only when its status line is 200 or absent (Task 15), and
        // the decision takes the answers plan_doc parsed, so neither section parses these bodies.
        write(&root.join("recs/disc-b/lookup-status.txt"), "dischash 500\ntmdb movie/4242 404\n");
        write(&root.join("recs/disc-b/thediscdb-hash.json"), "<html>Internal Server Error</html>");
        write(
            &root.join("recs/disc-b/tmdb.json"),
            json!({"success": false, "status_code": 34,
                   "status_message": "The resource you requested could not be found."})
            .to_string(),
        );
        let doc = doc_b(&root, "all-tracks");
        let PlanEntry::Results(plan) = &doc.plan else {
            panic!("the plan was skipped: {:?}", doc.plan);
        };
        let answers = (plan.inputs.discdb_hash.answer.as_str(), plan.inputs.tmdb.answer.as_str());
        assert_eq!(answers, ("failed", "miss"));
        // No record names the file: the decision is made without a path, and held (8.8).
        let decided = results_of(&doc.decision);
        assert_eq!(decided.decision.path, None);
        assert_eq!(decided.decision.disposition, Disposition::NeedsReview);
    }

    /// A level folder's audio-proxy.json as the recorder writes it, reduced to what decide reads: per (ffprobe stream,
    /// packet MD5) one track whose `source_stream` and `source.md5` they are.
    fn proxy_b(tracks: &[(u32, &str)]) -> String {
        let tracks: Vec<Value> = tracks
            .iter()
            .enumerate()
            .map(|(k, (stream, md5))| {
                json!({"track": k, "source_stream": stream, "series": format!("track-{k}.txt"),
                       "source": {"codec": "dts", "channels": 6, "md5": md5}})
            })
            .collect();
        let count = tracks.len();
        json!({"format": "lazymkv audio proxy v1", "source": {"file": "title_t00.mkv", "audio_streams": count},
               "tracks": tracks})
        .to_string()
    }

    #[test]
    fn without_a_manifest_the_md5s_come_from_audio_proxy_json() {
        let tmp = Scratch::new("decision-proxy-md5");
        let root = tmp.0.clone();
        disc_b(&root);
        // A clone without the git-ignored level files still holds each level folder's audio-proxy.json, with the same
        // packet MD5s (6.4 step 10a, D13): the twin is still dropped as a duplicate.
        let folder = root.join("recs/disc-b/all-tracks/audio-levels-20ms-1-2-3-4");
        fs::remove_file(folder.join("manifest.txt")).unwrap();
        write(&folder.join("audio-proxy.json"), proxy_b(&[(1, TWIN_A), (2, TWIN_B), (3, TWIN_A), (4, TWIN_B)]));
        let doc = doc_b(&root, "all-tracks");
        let decided = results_of(&doc.decision);
        assert_eq!(decided.facts.md5_streams, vec![1, 2, 3, 4]);
        assert_eq!(
            decided.decision.audio.dropped,
            vec![(2, AudioDrop::Derived), (3, AudioDrop::Duplicate { of: 1 }), (4, AudioDrop::Derived)]
        );
        // With a manifest that names stream 1 only, the proxy still gives the other streams' MD5s.
        write(&folder.join("manifest.txt"), manifest_b(&[("main", 1, TWIN_A)]));
        let doc = doc_b(&root, "all-tracks");
        assert_eq!(results_of(&doc.decision).facts.md5_streams, vec![1, 2, 3, 4]);
    }

    /// decision_doc over recs/disc-b's `label` after `change` edited the plan decide made and decide's document (its
    /// audio and subtitles sections), as the engine decides with other settings or other evidence.
    fn decided_with(
        root: &Path,
        label: &str,
        change: impl FnOnce(&mut PlanDoc, &mut DecideDoc),
    ) -> Result<DecisionDoc, String> {
        let ctx = Ctx::for_test(root);
        let rec = resolve_recording(&ctx, None, "recs/disc-b").unwrap();
        let mut doc = decide_doc(&ctx, &rec, label, None, None, None).unwrap();
        let mut plan = match &doc.plan {
            PlanEntry::Results(found) => found.clone(),
            PlanEntry::Skipped { skipped } => panic!("the plan was skipped: {skipped}"),
        };
        change(&mut plan, &mut doc);
        decision_doc(&ctx, &rec, label, &plan, &doc)
    }

    /// A level folder's results as decide's audio section holds them: measured against file track `main`, with these
    /// (candidate track, verdict) pairs and D21's veto answer.
    fn measured(main: &str, verdicts: &[(&str, Verdict)], holds: Option<bool>) -> FolderJson {
        let mut fields = Ordered::new();
        fields.insert("main".to_string(), main.to_string());
        FolderJson {
            folder: format!("audio-levels-20ms-{main}"),
            main: fields,
            frames: 0,
            dialogue: Dialogue::Mix,
            veto: Veto { holds, why: "made-up".to_string() },
            veto_line: String::new(),
            candidates: verdicts.iter().map(|(stream, verdict)| candidate(stream, *verdict)).collect(),
        }
    }

    #[test]
    fn the_verdicts_come_from_the_audio_section_measured_against_its_main_stream() {
        let tmp = Scratch::new("decision-verdicts");
        let root = tmp.0.clone();
        disc_b(&root);
        // Without the MD5s nothing says stream 3 is a copy of stream 1.
        fs::remove_file(root.join("recs/disc-b/all-tracks/audio-levels-20ms-1-2-3-4/manifest.txt")).unwrap();
        let decided = decided_with(&root, "all-tracks", |_, doc| {
            let verdicts = [("2", Verdict::Keep), ("3", Verdict::Commentary), ("4", Verdict::Keep)];
            doc.audio = vec![AudioEntry::Results(measured("1", &verdicts, Some(false)))];
        })
        .unwrap();
        assert_eq!((decided.facts.verdict_streams.clone(), decided.facts.verdict_reference), (vec![2, 3, 4], Some(1)));
        assert_eq!(decided.facts.md5_streams, Vec::<u32>::new());
        let audio = &decided.decision.audio;
        let kept: Vec<u32> = audio.kept.iter().map(|track| track.stream).collect();
        // D15: the commentary verdict drops nothing; it holds the job.
        assert_eq!(kept, [1, 3]);
        assert_eq!(audio.dropped, vec![(2, AudioDrop::Derived), (4, AudioDrop::Derived)]);
        assert_eq!((audio.reference, audio.verdict_reference), (Some(1), Some(1)));
        assert_eq!(audio.reviews, [ReviewReason::CommentaryDetected]);
        assert_eq!(decided.decision.reviews, [ReviewReason::IdentityUnknown, ReviewReason::CommentaryDetected]);
        // A folder measured against a stream but holding no candidate gives no verdict, and so no reference and no veto
        // (AudioFacts.verdict_reference is None when there are no verdicts): measured against stream 3 while the
        // decision's reference is stream 1, it raises nothing.
        let empty = decided_with(&root, "all-tracks", |_, doc| {
            doc.audio = vec![AudioEntry::Results(measured("3", &[], Some(false)))];
        })
        .unwrap();
        assert_eq!((empty.facts.verdict_streams.clone(), empty.facts.verdict_reference), (Vec::new(), None));
        assert_eq!(empty.decision.audio.reviews, Vec::<ReviewReason>::new());
    }

    #[test]
    fn verdicts_measured_against_another_stream_or_under_the_veto_raise_audio_reference_unclear() {
        let tmp = Scratch::new("decision-reference");
        let root = tmp.0.clone();
        disc_b(&root);
        // Measured against stream 3, but the decision's reference is stream 1.
        let other = decided_with(&root, "all-tracks", |_, doc| {
            doc.audio = vec![AudioEntry::Results(measured("3", &[("1", Verdict::Keep)], Some(false)))];
        })
        .unwrap();
        assert_eq!((other.facts.verdict_streams.clone(), other.facts.verdict_reference), (vec![1], Some(3)));
        assert_eq!(other.decision.audio.reviews, [ReviewReason::AudioReferenceUnclear]);
        assert_eq!(other.decision.reviews, [ReviewReason::IdentityUnknown, ReviewReason::AudioReferenceUnclear]);
        // D21's veto held.
        let vetoed = decided_with(&root, "all-tracks", |_, doc| {
            doc.audio = vec![AudioEntry::Results(measured("1", &[("2", Verdict::Keep)], Some(true)))];
        })
        .unwrap();
        assert_eq!(vetoed.facts.verdict_reference, Some(1));
        assert_eq!(vetoed.decision.audio.reviews, [ReviewReason::AudioReferenceUnclear]);
        // A second folder measured against another stream is left out, with its veto and its verdicts.
        let two = decided_with(&root, "all-tracks", |_, doc| {
            doc.audio = vec![
                AudioEntry::Results(measured("1", &[("2", Verdict::Keep)], Some(false))),
                AudioEntry::Results(measured("3", &[("4", Verdict::Commentary)], Some(true))),
            ];
        })
        .unwrap();
        assert_eq!((two.facts.verdict_streams.clone(), two.facts.verdict_reference), (vec![2], Some(1)));
        assert_eq!(two.decision.audio.reviews, Vec::<ReviewReason>::new());
        // A main-only folder gives nothing: it does not fix the reference for the folders after it.
        let lead = decided_with(&root, "all-tracks", |_, doc| {
            doc.audio = vec![
                AudioEntry::Results(measured("3", &[], Some(false))),
                AudioEntry::Results(measured("1", &[("2", Verdict::Commentary)], Some(false))),
            ];
        })
        .unwrap();
        assert_eq!((lead.facts.verdict_streams.clone(), lead.facts.verdict_reference), (vec![2], Some(1)));
        // A skipped folder gives nothing.
        let skipped = decided_with(&root, "all-tracks", |_, doc| {
            doc.audio = vec![AudioEntry::Skipped { folder: "audio-levels-20ms-1-2".to_string(), skipped: "made-up".to_string() }];
        })
        .unwrap();
        assert_eq!((skipped.facts.verdict_streams.clone(), skipped.facts.verdict_reference), (Vec::new(), None));
        assert_eq!(skipped.decision.audio.reviews, Vec::<ReviewReason>::new());
    }

    /// Puts the subtitles section of a run with OCR text in `doc`: one stream per (subs/index.json `n`, the text rule's
    /// role, None for a stream the rule did not decide).
    fn with_text(doc: &mut DecideDoc, roles: &[(i64, Option<Role>)]) {
        let streams = roles.iter().map(|(n, role)| stream(*n, role.map(|role| decision(role, None)), None)).collect();
        doc.subtitles = SubtitleEntry::Results(subs(streams));
    }

    /// The (note, stream) pairs of the subtitle decision's notes.
    fn subtitle_notes(decided: &DecisionDoc) -> Vec<(Note, Option<u32>)> {
        decided.decision.subtitles.notes.iter().map(|noted| (noted.note, noted.stream)).collect()
    }

    /// The kept subtitle tracks of forced-kept: the derived forced-only stream 6 forced (its parent 5 is the regular
    /// track), then stream 5 as "English", each with `events` events.
    fn forced_kept_tracks(events: usize) -> [(u32, SubtitleRole, Option<usize>); 2] {
        [(6, SubtitleRole::Forced, Some(events)), (5, SubtitleRole::Regular, Some(events))]
    }

    #[test]
    fn the_text_decisions_come_from_the_subtitles_section_through_subs_index_json() {
        let tmp = Scratch::new("decision-text");
        let root = tmp.0.clone();
        disc_b(&root);
        forced_kept(&root);
        // n 0 is file track 5, scan stream 5; n 1 is file track 6, scan stream 6; n 2 is in no index entry and is left
        // out. The file holds two English subtitle streams, so 6.4 step 10 runs and the text is read.
        let disagree = decided_with(&root, "forced-kept", |_, doc| {
            with_text(doc, &[(0, Some(Role::Sdh)), (1, Some(Role::Forced)), (2, Some(Role::Regular))]);
        })
        .unwrap();
        assert_eq!(disagree.facts.text_streams, vec![5, 6]);
        // The text never changes a track: the counts keep stream 5 as "English".
        assert_eq!(kept_subtitles(&disagree.decision), forced_kept_tracks(40));
        assert_eq!(disagree.decision.subtitles.reviews, [ReviewReason::SubtitleAmbiguous]);
        assert_eq!(subtitle_notes(&disagree), [(Note::SubtitleTextDisagrees, Some(5))]);
        assert_eq!(disagree.decision.reviews, [ReviewReason::IdentityUnknown, ReviewReason::SubtitleAmbiguous]);
        assert_eq!(disagree.decision.confidences.track, 0.60);
        // A text verdict that agrees adds nothing.
        let agree = decided_with(&root, "forced-kept", |_, doc| with_text(doc, &[(0, Some(Role::Regular))])).unwrap();
        assert_eq!(agree.facts.text_streams, vec![5]);
        assert_eq!(agree.decision.subtitles.reviews, Vec::<ReviewReason>::new());
        assert_eq!(subtitle_notes(&agree), Vec::<(Note, Option<u32>)>::new());
        assert_eq!(agree.decision.confidences.track, 0.90);
        // A stream the text rule did not decide gives no text decision.
        let undecided = decided_with(&root, "forced-kept", |_, doc| with_text(doc, &[(0, None)])).unwrap();
        assert_eq!(undecided.facts.text_streams, Vec::<u32>::new());
    }

    #[test]
    fn a_title_with_one_english_stream_in_the_file_keeps_it_as_english_whatever_the_text_reads() {
        // all-tracks holds one English PGS stream: MakeMKV removed its derived forced-only stream 6 (MSG:4047). 6.4 step
        // 10 does not run (8.7 "Text": the text is read only with two or more English subtitle streams), so a text verdict
        // reading stream 5 as SDH adds no review and no note, and the decision is the one retune --subtitles' 8.7 column
        // gives (section 17 row 14 (b)'s fallback).
        let tmp = Scratch::new("decision-one-english");
        let root = tmp.0.clone();
        disc_b(&root);
        let decided = decided_with(&root, "all-tracks", |_, doc| with_text(doc, &[(0, Some(Role::Sdh))])).unwrap();
        assert_eq!(decided.facts.removed, vec![6]);
        assert_eq!(decided.facts.text_streams, Vec::<u32>::new());
        assert_eq!(kept_subtitles(&decided.decision), [(5, SubtitleRole::Regular, Some(40))]);
        assert_eq!(decided.decision.subtitles.reviews, Vec::<ReviewReason>::new());
        assert_eq!(subtitle_notes(&decided), Vec::<(Note, Option<u32>)>::new());
        assert_eq!(decided.decision.reviews, [ReviewReason::IdentityUnknown]);
        assert_eq!(decided.decision.confidences.track, 0.90);
    }

    #[test]
    fn sub_clear_max_bytes_counts_the_events_and_another_text_rule_leaves_the_text_out() {
        let tmp = Scratch::new("decision-subtitle-settings");
        let root = tmp.0.clone();
        disc_b(&root);
        forced_kept(&root);
        assert_eq!(Settings::default().text_rule(), TextRule::default());
        // Defaults: 64-byte clear packets are not events; the text rule is the section's.
        let default = decided_with(&root, "forced-kept", |_, doc| with_text(doc, &[(0, Some(Role::Sdh))])).unwrap();
        assert_eq!(kept_subtitles(&default.decision), forced_kept_tracks(40));
        assert_eq!(default.facts.text_streams, vec![5]);
        assert_eq!(default.decision.subtitles.reviews, [ReviewReason::SubtitleAmbiguous]);
        // sub_clear_max_bytes 0: every packet is an event, and the text rule differs from the section's.
        let zero = decided_with(&root, "forced-kept", |plan, doc| {
            plan.settings.sub_clear_max_bytes = 0;
            with_text(doc, &[(0, Some(Role::Sdh))]);
        })
        .unwrap();
        assert_eq!(kept_subtitles(&zero.decision), forced_kept_tracks(80));
        assert_eq!(zero.facts.text_streams, Vec::<u32>::new());
        assert_eq!(zero.decision.subtitles.reviews, Vec::<ReviewReason>::new());
        // subs_same_min 0.8: the events are the default's, the text is left out.
        let stricter = decided_with(&root, "forced-kept", |plan, doc| {
            plan.settings.subs_same_min = 0.8;
            with_text(doc, &[(0, Some(Role::Sdh))]);
        })
        .unwrap();
        assert_eq!(kept_subtitles(&stricter.decision), forced_kept_tracks(40));
        assert_eq!(stricter.facts.text_streams, Vec::<u32>::new());
        assert_eq!(stricter.decision.confidences.track, 0.90);
    }

    /// A made-up MSG:2003 read error (spec 9.4): the error text, the drive (made up) and the offset.
    const READ_ERROR_LINE: &str = "MSG:2003,0,3,\"Error 'Made-up read error' occurred while reading 'MADE-UP DRIVE' at \
                                   offset '4096'\",\"Error '%1' occurred while reading '%2' at offset '%3'\",\"Made-up \
                                   read error\",\"MADE-UP DRIVE\",\"4096\"\n";

    /// A TheDiscDB dischash answer of the API's shape for recs/disc-b: a movie whose TMDb ID is 4242, and one MainMovie
    /// title with title 0's source file, segment map, duration and size, and title 0's tracks in scan order with
    /// TheDiscDB's fields (`name` is attribute 7; an audio track's `audio_type` gives its channels), the second DTS-HD MA
    /// track described as commentary.
    fn discdb_answer() -> String {
        let audio = |index: u32, name: &str, description: Option<&str>| {
            json!({"index": index, "type": "Audio", "name": name, "audio_type": "Surround 5.1", "language_code": "eng",
                   "language": "English", "description": description})
        };
        let subtitles = |index: u32| {
            json!({"index": index, "type": "Subtitles", "name": "HDMV PGS Subtitles", "language_code": "eng",
                   "language": "English"})
        };
        let tracks = vec![
            json!({"index": 0, "type": "Video", "name": "Mpeg4 AVC High@L4.1", "resolution": "1920x1080"}),
            audio(1, "DTS-HD Master Audio", None),
            audio(2, "DTS", None),
            audio(3, "DTS-HD Master Audio", Some("Commentary")),
            audio(4, "DTS", None),
            subtitles(5),
            subtitles(6),
        ];
        json!({
            "content_hash": "00000000000000000000000000000000",
            "format": "Blu-ray",
            "media": {"title": "Made Up Film", "full_title": "Made Up Film (2001)", "year": 2001, "type": "Movie",
                      "external_ids": {"tmdb": "4242", "imdb": "tt0004242"}},
            "release": {"title": "Made Up Film", "year": 2001, "slug": "2001-blu-ray", "locale": "en-us"},
            "disc": {"index": 1, "name": "Disc 1", "slug": "disc-1"},
            "titles": [{"index": 0, "source_file": "00800.mpls", "segment_map": "1", "duration": "0:50:00",
                        "size": 6_000_000_000_u64, "tracks": tracks,
                        "item": {"title": "Made Up Film", "type": "MainMovie"}}]
        })
        .to_string()
    }

    /// The recording's slim tmdb.json (the recorder's shape) for the made-up film, with TMDb ID `id`.
    fn tmdb_answer(id: i64) -> String {
        json!({"id": id, "title": "Made Up Film", "original_title": "Made Up Film", "release_date": "2001-05-04",
               "original_language": "en", "external_ids": {"imdb_id": "tt0004242"}, "fetched_at": "2026-10-16"})
        .to_string()
    }

    #[test]
    fn read_errors_of_the_rip_reach_the_decision_but_not_decides_own_reviews() {
        let tmp = Scratch::new("decision-read-errors");
        let root = tmp.0.clone();
        disc_b(&root);
        write(&root.join("recs/disc-b/all-tracks/rip.txt"), format!("{}{}", removal(6), READ_ERROR_LINE));
        let doc = doc_b(&root, "all-tracks");
        let decided = results_of(&doc.decision);
        let error = ReadError { offset: "4096".to_string(), count: 1, message: "Made-up read error".to_string() };
        assert_eq!(decided.verify.read_errors, vec![error.clone()]);
        // The rip saved: it verifies, and is kept with read_errors (6.2 step 7); a rip that verifies raises no
        // verify_failed.
        assert!(decided.verify.ok, "{:?}", decided.verify.mismatches);
        assert_eq!(decided.decision.read_errors, vec![error]);
        assert_eq!(decided.decision.reviews, [ReviewReason::IdentityUnknown, ReviewReason::ReadErrors]);
        assert_eq!(decided.decision.disposition, Disposition::NeedsReview);
        // decide's own reviews are the audio and subtitle sections', unchanged.
        assert_eq!(doc.reviews, Vec::<String>::new());
        let printed = serde_json::to_string(&doc.decision).unwrap();
        assert!(!printed.contains("MADE-UP DRIVE"), "{}", printed);
    }

    #[test]
    fn the_recorded_tmdb_record_names_the_file_only_when_it_is_the_plans_identity() {
        let tmp = Scratch::new("decision-naming");
        let root = tmp.0.clone();
        disc_b(&root);
        write(&root.join("recs/disc-b/thediscdb-hash.json"), discdb_answer());
        write(&root.join("recs/disc-b/tmdb.json"), tmdb_answer(4242));
        let movie = |id: i64| TmdbIdentity { kind: TmdbKind::Movie, id };
        let decided = decided_with(&root, "all-tracks", |plan, _| {
            assert_eq!(plan.plan.identity.tmdb, Some(movie(4242)));
            plan.settings.movies_root = Some("/srv/media/Movies".to_string());
        })
        .unwrap();
        let decision = &decided.decision;
        // TheDiscDB's matched title reaches the facts: its track list aligns with every stream of the title (8.6 step 1).
        assert_eq!(decision.audio.description_alignment, Some(Alignment::Full));
        assert_eq!(decision.confidences, Confidences { identity: 0.95, title: 0.98, track: 0.90 });
        assert_eq!(
            decision.path,
            Some(NamedPath {
                root: "/srv/media/Movies".to_string(),
                components: vec![
                    "Made Up Film (2001) {tmdb-4242}".to_string(),
                    "Made Up Film (2001) {tmdb-4242} - 1080p.mkv".to_string(),
                ],
                path: "/srv/media/Movies/Made Up Film (2001) {tmdb-4242}/Made Up Film (2001) {tmdb-4242} - 1080p.mkv"
                    .to_string(),
            })
        );
        assert_eq!(decision.reviews, Vec::<ReviewReason>::new());
        assert_eq!(decision.disposition, Disposition::AutoFile);
        // decide's own document plans with spec 14's defaults, which set no library root: no path, held (Task 13).
        let doc = doc_b(&root, "all-tracks");
        let own = &results_of(&doc.decision).decision;
        assert_eq!((own.path.clone(), own.disposition), (None, Disposition::NeedsReview));
        // The record names the file only when it is the plan's identity's.
        let record = parse_recorded(&tmdb_answer(4242), "tmdb.json").unwrap();
        assert_eq!(record_for_naming(Some(movie(4242)), Some(record.clone())), Some(record.clone()));
        assert_eq!(record_for_naming(Some(movie(9999)), Some(record.clone())), None);
        assert_eq!(record_for_naming(Some(TmdbIdentity { kind: TmdbKind::Tv, id: 4242 }), Some(record.clone())), None);
        assert_eq!(record_for_naming(None, Some(record)), None);
        assert_eq!(record_for_naming(Some(movie(4242)), None), None);
        // The same record answered by disc ID (an array) gives the same title, the plan's own choice; without the
        // MD5s, the second DTS-HD MA track, described as commentary, is dropped as described (8.6 step 1, R6).
        fs::remove_file(root.join("recs/disc-b/thediscdb-hash.json")).unwrap();
        fs::remove_file(root.join("recs/disc-b/all-tracks/audio-levels-20ms-1-2-3-4/manifest.txt")).unwrap();
        let record: Value = serde_json::from_str(&discdb_answer()).unwrap();
        write(&root.join("recs/disc-b/thediscdb-discid.json"), json!([record]).to_string());
        let doc = doc_b(&root, "all-tracks");
        let audio = &results_of(&doc.decision).decision.audio;
        assert_eq!(audio.description_alignment, Some(Alignment::Full));
        assert_eq!(
            audio.dropped,
            vec![
                (2, AudioDrop::Derived),
                (3, AudioDrop::Described { kind: DescriptionKind::Commentary }),
                (4, AudioDrop::Derived),
            ]
        );
    }

    #[test]
    fn a_rip_that_fails_verification_raises_verify_failed_and_is_held() {
        let tmp = Scratch::new("decision-verify-failed");
        let root = tmp.0.clone();
        disc_b(&root);
        // The file lacks the PGS track the selection expects (6.2 step 7: the ordered match fails).
        let video = ("video", "AVC/H.264/MPEG-4p10", None);
        let hd_ma = ("audio", "DTS-HD Master Audio", Some(6));
        let core = ("audio", "DTS", Some(6));
        let ident = ident_b(&[video, hd_ma, core, hd_ma, core], 5, 3_000_500_000_000);
        write(&root.join("recs/disc-b/all-tracks/mkvmerge.json"), ident);
        let doc = doc_b(&root, "all-tracks");
        let decided = results_of(&doc.decision);
        assert!(!decided.verify.ok);
        // The recorded rip is the last attempt there will be: verify_failed holds the job, as a second failure does.
        let reviews = &decided.decision.reviews;
        assert!(reviews.contains(&ReviewReason::VerifyFailed), "{:?}", reviews);
        assert_eq!(decided.decision.disposition, Disposition::NeedsReview);
    }

    #[test]
    fn a_settings_file_gives_decides_own_decision_its_final_path() {
        let tmp = Scratch::new("decision-settings");
        let root = tmp.0.clone();
        disc_b(&root);
        write(&root.join("recs/disc-b/thediscdb-hash.json"), discdb_answer());
        write(&root.join("recs/disc-b/tmdb.json"), tmdb_answer(4242));
        write(&root.join("settings.json"), json!({"movies_root": "/srv/media/Movies"}).to_string());
        let path = "/srv/media/Movies/Made Up Film (2001) {tmdb-4242}/Made Up Film (2001) {tmdb-4242} - 1080p.mkv";
        // decide_doc with the settings file (spec 14): the plan carries the root, and decide's own decision files.
        let ctx = Ctx::for_test(&root);
        let rec = resolve_recording(&ctx, None, "recs/disc-b").unwrap();
        let doc = decide_doc(&ctx, &rec, "all-tracks", None, None, Some(Path::new("settings.json"))).unwrap();
        let PlanEntry::Results(plan) = &doc.plan else {
            panic!("the plan was skipped: {:?}", doc.plan);
        };
        assert_eq!(plan.settings.movies_root.as_deref(), Some("/srv/media/Movies"));
        let decision = &results_of(&doc.decision).decision;
        assert_eq!(decision.path.as_ref().map(|named| named.path.as_str()), Some(path));
        assert_eq!(decision.disposition, Disposition::AutoFile);
        // The command takes it as --settings FILE.
        let (code, out, err) = run_in(&root, &["recs/disc-b", "all-tracks", "--settings", "settings.json"]);
        assert_eq!((code, err.as_str()), (0, ""));
        let value: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(value["decision"]["decision"]["path"]["path"], json!(path));
        assert_eq!(value["decision"]["decision"]["disposition"], json!("auto_file"));
        // A settings file plan_doc refuses (an unknown key, Task 15) skips the plan with its line, and the decision.
        write(&root.join("bad-settings.json"), r#"{"bogus": 1}"#);
        let doc = decide_doc(&ctx, &rec, "all-tracks", None, None, Some(Path::new("bad-settings.json"))).unwrap();
        let PlanEntry::Skipped { skipped } = &doc.plan else {
            panic!("the plan was not skipped: {:?}", doc.plan);
        };
        assert!(skipped.starts_with("bad-settings.json: bogus: "), "{}", skipped);
        assert_eq!(doc.decision, DecisionEntry::Skipped { skipped: format!("no plan: {skipped}") });
        let (code, _, err) = run_in(&root, &["recs/disc-b", "all-tracks", "--settings", "bad-settings.json"]);
        assert_eq!((code, err.as_str()), (0, ""));
    }

    #[test]
    fn a_printed_document_reads_back_whole() {
        let tmp = Scratch::new("decision-read-back");
        let root = tmp.0.clone();
        disc_b(&root);
        write(&root.join("recs/disc-b/thediscdb-hash.json"), discdb_answer());
        write(&root.join("recs/disc-b/tmdb.json"), tmdb_answer(4242));
        let doc = doc_b(&root, "all-tracks");
        // TheDiscDB's matches are a list of entries sorted by database index (the interfaces' "Keyed collections").
        let PlanEntry::Results(plan) = &doc.plan else {
            panic!("the plan was skipped: {:?}", doc.plan);
        };
        assert_eq!(plan.plan.evidence.discdb_matches, vec![DiscDbMatch { discdb_index: 0, title: 0 }]);
        let (code, out, err) = run_in(&root, &["recs/disc-b", "all-tracks"]);
        assert_eq!((code, err.as_str()), (0, ""));
        let read: DecideDoc = serde_json::from_str(&out).unwrap();
        assert_eq!(read, doc);
        // A skipped section reads back as skipped.
        let skipped: DecisionEntry = serde_json::from_value(json!({"skipped": "made-up"})).unwrap();
        assert_eq!(skipped, DecisionEntry::Skipped { skipped: "made-up".to_string() });
    }

    #[test]
    fn the_command_is_registered_with_its_synopsis() {
        let row = crate::COMMANDS.iter().find(|(name, _, _)| *name == "decide");
        assert_eq!(
            row.map(|(_, _, synopsis)| *synopsis),
            Some("(<slug> <label> | --rec DIR) [--text DIR] [--selection all|eng] [--settings FILE]")
        );
    }
}
