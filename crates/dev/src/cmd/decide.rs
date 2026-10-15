//! `lazymkv-dev decide (<slug> <label> | --rec DIR) [--text DIR] [--selection all|eng] [--evidence DIR]` (port
//! spec 3.4): the first form of the design spec's phase 1 command that reads a recording and prints its decisions.
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

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};

use lazymkv_core::audio::measures::Constants;
use lazymkv_core::audio::report::{folder_json, FolderJson};
use lazymkv_core::audio::rule::{results, Plain, Rule as AudioRule, Verdict};
use lazymkv_core::recording::files::{rip_label, MkvmergeIdent, Selection};
use lazymkv_core::recording::{py_repr_str, read_text, text_mode, Encoding, Errors, PathKey, Tree};
use lazymkv_core::reconcile::{reconcile, Report, RipLog, RIP_LOG_NAMES};
use lazymkv_core::subtitle::text::{subs_json, subs_run, Rule as TextRule, SubsJson};

use crate::evidence::{ocr_text_dir, pathlib_split, read_scan_in, resolve_recording, Ctx, DiskTree, Recording, Refused};
use crate::pyjson::{dumps, Style};
use crate::{usage_error, Args};

/// The command's name as typed.
const NAME: &str = "decide";
/// The usage line, under the command's name: spec 3.4's synopsis with the --evidence flag every slug command takes.
const USAGE: &str = "decide (<slug> <label> | --rec DIR) [--text DIR] [--selection all|eng] [--evidence DIR]";
/// What -h prints after the usage line.
const HELP: &str = "
One JSON document for one rip label (port spec 3.4): reconcile (the scan's streams against mkvmerge.json, with
the title and the selection from the rip's summary), audio (the content rule over every audio-levels-20ms-*
folder of the label, or why a folder was skipped), subtitles (the text rule over the title's OCR text, or why it
was skipped) and reviews (the review reasons they raise).

arguments:
  slug                  a recording: a folder under the evidence clone, or a recording folder's path
  label                 a rip label of the recording: a folder with an ok rip-summary.json or
                        extract-summary.json and an mkvmerge.json
  --rec DIR             the rip label folder itself, instead of slug and label (no evidence check)
  --text DIR            the OCR text: DIR/<slug>/<label>/subs-ocr/sub-<n>.ocr.jsonl, else DIR/sub-<n>.ocr.jsonl
                        (default: LAZYMKV_OCR_TEXT)
  --selection {all,eng} the rip's track selection (default: eng for the profile eng-only.mmcp.xml, else all)
  --evidence DIR        the evidence clone (default: LAZYMKV_EVIDENCE, else corpus/owner)

exit status: 0; 1 when a reconcile verdict is no; 2 for an argument error, an unreadable scan.txt or
mkvmerge.json, or a label that is not a rip label; 3 when the evidence clone is not there.
";
/// The level folders of a label: the folders the tuning harness measures. Py: audio_dataset.find_folders' glob.
const LEVEL_FOLDERS: &str = "audio-levels-20ms-*";
/// The reading folder decide looks for first under --text DIR, in the goldens' layout DIR/<slug>/<label>/<reading>/
/// (port spec 5): the one ocr-subs.ps1 writes.
const READING: &str = "subs-ocr";
/// After "<slug>/<label>/<folder>: " when git kept only a level folder's streams.json, verbatim.
/// Py: audio_dataset.measure's unreadable line.
const LEVEL_FILES_ABSENT: &str =
    "its level files are not here (git keeps only streams.json): pull the recording or copy the folder from the dataset share";
/// How to give decide the OCR text, at the end of every no-text reason.
const TEXT_HINT: &str = "(give --text DIR or LAZYMKV_OCR_TEXT)";
/// The review reasons decide raises (design spec 8.6, 8.7 and 8.8).
const AUDIO_REFERENCE_UNCLEAR: &str = "audio_reference_unclear";
const COMMENTARY_DETECTED: &str = "commentary_detected";
const SUBTITLE_AMBIGUOUS: &str = "subtitle_ambiguous";
/// What a subtitle outcome that carries a review holds: spec87's "(review)", before_yes's "(review: ...)".
const REVIEW_MARK: &str = "(review";
/// The scan's sentence for a title it does not list (core::reconcile's), also used for a negative summary title.
const NO_SUCH_TITLE: &str = "the scan lists no title ";

/// decide's JSON document (spec 3.4). Field order is the output's key order.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DecideDoc {
    pub slug: String,
    pub label: String,
    pub reconcile: Report,
    /// One entry per level folder of the label, in PathKey order.
    pub audio: Vec<AudioEntry>,
    pub subtitles: SubtitleEntry,
    /// Sorted and unique: see audio_reviews and subtitle_reviews.
    pub reviews: Vec<String>,
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
/// the audio and subtitle sections, which never fail. `text` is taken against ctx.cwd when relative.
pub fn decide_doc(
    ctx: &Ctx,
    rec: &Recording,
    label: &str,
    text: Option<&Path>,
    selection: Option<Selection>,
) -> Result<DecideDoc, Refused> {
    let refused = |line: String| Refused { code: 2, line };
    let tree: &dyn Tree = &rec.tree;
    let rip = rip_label(tree, &rec.dir, label).map_err(refused)?;
    let label_dir = rec.dir.join(label);
    let selection = selection.unwrap_or(rip.selection);
    let title = u32::try_from(rip.title).map_err(|_| refused(format!("{}{}", NO_SUCH_TITLE, rip.title)))?;
    let scan = read_scan_in(tree, &rec.dir.join("scan.txt"), Encoding::Utf8, Errors::Replace).map_err(refused)?;
    let ident = read_ident(tree, &label_dir.join("mkvmerge.json")).map_err(refused)?;
    let rip_path = RIP_LOG_NAMES.iter().map(|name| label_dir.join(name)).find(|path| tree.exists(path));
    let rip_text = match &rip_path {
        Some(path) => Some(text_mode(&read_text(tree, path, Encoding::Utf8, Errors::Replace).map_err(refused)?)),
        None => None,
    };
    let rip_log = match (&rip_path, &rip_text) {
        (Some(path), Some(text)) => Some(RipLog { shown: tree.display(path), text: text.as_str() }),
        _ => None,
    };
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
    Ok(DecideDoc {
        slug: rec.slug.clone(),
        label: label.to_string(),
        reconcile: report,
        audio,
        subtitles,
        reviews: reviews.into_iter().map(str::to_string).collect(),
    })
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
            let skipped = format!("{}/{}/{}: {}", rec.slug, label, name, LEVEL_FILES_ABSENT);
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
    let mut parsed = Args::new(args, &["--rec", "--text", "--selection", "--evidence"]);
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
    let doc = match decide_doc(ctx, &rec, &label, text.as_deref(), selection) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    use lazymkv_core::audio::report::CandidateJson;
    use lazymkv_core::audio::rule::{Dialogue, Veto};
    use lazymkv_core::recording::Ordered;
    use lazymkv_core::reconcile::Verdicts;
    use lazymkv_core::robot::parse;
    use lazymkv_core::subtitle::text::{Class, Decision, Role, SubsStreamJson};
    use lazymkv_core::testsupport::{commentary, level_file_text, programme, raw_file_text};
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

    /// decide_doc over recs/disc-a (named by its path, so printed as recs/disc-a/...) with cwd `root`.
    fn doc_of(root: &Path, label: &str, text: Option<&str>, selection: Option<Selection>) -> Result<DecideDoc, Refused> {
        let ctx = Ctx::for_test(root);
        let rec = resolve_recording(&ctx, None, "recs/disc-a").unwrap();
        decide_doc(&ctx, &rec, label, text.map(Path::new), selection)
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
        assert!(
            out.ends_with(concat!(
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
                " \"reviews\": []\n",
                "}\n"
            )),
            "{}",
            out
        );
        let value: Value = serde_json::from_str(&out).unwrap();
        let keys: Vec<&str> = value.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys, ["slug", "label", "reconcile", "audio", "subtitles", "reviews"]);
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
                     [--evidence DIR]\n";
        let cases: [(&[&str], &str); 7] = [
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

    #[test]
    fn the_command_is_registered_with_its_synopsis() {
        let row = crate::COMMANDS.iter().find(|(name, _, _)| *name == "decide");
        assert_eq!(
            row.map(|(_, _, synopsis)| *synopsis),
            Some("(<slug> <label> | --rec DIR) [--text DIR] [--selection all|eng]")
        );
    }
}
