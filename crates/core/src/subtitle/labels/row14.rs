//! Spec section 17 row 14 (a) read from the spec text, its costly verdicts and the subtitle harness behind
//! `lazymkv-dev retune --subtitles` (RP15, spec 15 "Rules against the stored evidence", Task 18). The module is private:
//! `labels.rs` re-exports what other code uses.

use std::collections::{BTreeMap, BTreeSet};

use regex::Regex;

use crate::audio::labels::is_slug;
use crate::codec::Family;
use crate::disc::{disc, Disc, StreamKind, Title};
use crate::recording::files::{rip_label, MkvmergeIdent, Tmdb};
use crate::recording::{read_json, read_text, Encoding, Errors, PathKey, Tree};
use crate::reconcile::RIP_LOG_NAMES;
use crate::review::ReviewReason;
use crate::robot::{parse, Scan};
use crate::settings::Settings;
use crate::subtitle::text::{subs_run, Decision, Role};
use crate::tracks::subtitle::{decide_subtitles, english_or_none, SubtitleDecision, SubtitleFacts};
use crate::tracks::SubtitleRole;
use crate::verify::{
    by_scan_stream, removed_scan_streams, selected_streams, stream_of_track, subs_index, subs_streams,
    subtitle_events, verify_rip, VerifyReport,
};

use super::{load, SubtitleKind, SubtitleLabel, COUNTING_HOW, FILE_NAME};

// ------------------------------------------------------------------ row 14 (a)'s list

/// A number in row 14: ASCII digits, or a word from one to ten (the spec spells small numbers either way).
const NUM: &str = "([0-9]+|one|two|three|four|five|six|seven|eight|nine|ten)";
/// The number words NUM allows. Not audio::labels' private WORDS and number (row 13's reader): that pair is the
/// Python's port (`py_int`, i64, Python's ValueError text), while row 14 is read from the spec alone (RP20) into u32
/// counts with this reader's own sentence.
const WORDS: [(&str, u32); 10] = [
    ("one", 1),
    ("two", 2),
    ("three", 3),
    ("four", 4),
    ("five", 5),
    ("six", 6),
    ("seven", 7),
    ("eight", 8),
    ("nine", 9),
    ("ten", 10),
];
/// Row 14 (a)'s phrases, in the row's order, `{num}` standing for NUM; a plural `s` is optional wherever the row may
/// grow past 1.
const PAIR_TITLES: &str = r"at least {num} titles? from at least {num} discs? \(at least {num} feature films? and {num} TV episodes?\) that each have both a regular and an SDH English track";
const SDH_REGULAR: &str = "at least {num} SDH and {num} regular English tracks in all";
const FORCED_WITH_EVENTS: &str = "at least {num} real forced English tracks? with events";
const COMMENTARY_TRACK: &str = "at least {num} commentary subtitle tracks?";
const DVD_VOBSUB: &str = "at least {num} DVDs? with English VobSub subtitles";
const UK_SDH: &str = "at least {num} UK-style SDH tracks?";
/// The sentence naming the discs every threshold was fitted on, and a slug in backticks inside it.
const FITTED: &str = "Every threshold was fitted on (.{0,200}?), so none of their tracks counts";
const QUOTED_SLUG: &str = "`([a-z0-9][a-z0-9-]*)`";

/// Row 14 (a)'s required counts read from the spec text (the first line starting "| 14 |"): titles with both regular
/// and SDH (3, from 2 discs, 1 feature, 1 TV), SDH (6), regular (6), forced with events (1), commentary (1), DVD VobSub
/// (1), UK SDH (1, only if the owner has one), the two fitting discs. Spec 17 row 14 (a), RP15. Def: T18. Use: T18.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row14 {
    pub pair_titles: u32,
    pub pair_discs: u32,
    pub pair_features: u32,
    pub pair_tv: u32,
    pub sdh: u32,
    pub regular: u32,
    pub forced: u32,
    pub commentary: u32,
    pub dvd_vobsub: u32,
    pub uk_sdh: u32,
    pub fitting_discs: Vec<String>,
}

/// Reads row 14 (the first line of `spec_text` starting "| 14 |") with the phrases above. Err names the phrase it no
/// longer finds, so a reworded row is noticed instead of being counted against old numbers, as row 13's reader does.
/// Spec 17 row 14 (a), RP15. Def: T18. Use: T18.
pub fn exp14_requirements(spec_text: &str) -> Result<Row14, String> {
    let row = spec_text
        .lines()
        .find(|line| line.starts_with("| 14 |"))
        .ok_or_else(|| "spec section 17 has no row starting '| 14 |'".to_string())?;
    let pairs = numbers(row, PAIR_TITLES)?;
    let totals = numbers(row, SDH_REGULAR)?;
    let forced = numbers(row, FORCED_WITH_EVENTS)?;
    let commentary = numbers(row, COMMENTARY_TRACK)?;
    let dvd_vobsub = numbers(row, DVD_VOBSUB)?;
    let uk_sdh = numbers(row, UK_SDH)?;
    let unnamed = || "spec section 17 row 14 no longer names the discs the thresholds were fitted on".to_string();
    let sentence = Regex::new(FITTED)
        .expect("the fitted pattern is valid")
        .captures(row)
        .map(|caps| caps[1].to_string())
        .ok_or_else(unnamed)?;
    let fitting_discs: Vec<String> = Regex::new(QUOTED_SLUG)
        .expect("the slug pattern is valid")
        .captures_iter(&sentence)
        .map(|caps| caps[1].to_string())
        .collect();
    if fitting_discs.is_empty() {
        return Err(unnamed());
    }
    Ok(Row14 {
        pair_titles: pairs[0],
        pair_discs: pairs[1],
        pair_features: pairs[2],
        pair_tv: pairs[3],
        sdh: totals[0],
        regular: totals[1],
        forced: forced[0],
        commentary: commentary[0],
        dvd_vobsub: dvd_vobsub[0],
        uk_sdh: uk_sdh[0],
        fitting_discs,
    })
}

/// The numbers `template` (with `{num}` for NUM) finds in `row`, one per group, in order; Err names the phrase.
fn numbers(row: &str, template: &str) -> Result<Vec<u32>, String> {
    let pattern = template.replace("{num}", NUM);
    let caps = Regex::new(&pattern).expect("row 14's patterns are valid").captures(row).ok_or_else(|| {
        format!("spec section 17 row 14 no longer contains the phrase /{pattern}/; update core::subtitle::labels")
    })?;
    caps.iter().skip(1).map(|group| number(group.map_or("", |found| found.as_str()))).collect()
}

/// A count written in digits or as a word from one to ten. Kept apart from audio::labels' private `number`, which
/// gives an i64 through `py_int` with Python's ValueError text; this one gives a u32 with row 14's own sentence.
fn number(text: &str) -> Result<u32, String> {
    if let Ok(count) = text.parse::<u32>() {
        return Ok(count);
    }
    WORDS
        .iter()
        .find(|(word, _)| *word == text)
        .map(|&(_, count)| count)
        .ok_or_else(|| format!("spec section 17 row 14: {text} is not a count"))
}

// ------------------------------------------------------------------ the costly verdicts

/// Row 14's costly verdicts, in the row's order. Spec 17 row 14 (a). Def: T18. Use: T18.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Costly {
    DialogueDropped,
    SdhAsRegular,
    RegularAsSdh,
    ForcedWrongly,
    ForcedAsDialogue,
    UnwantedKept,
}

impl Costly {
    /// Every costly verdict, in row 14's order. Spec 17 row 14 (a).
    pub const ALL: [Costly; 6] = [
        Costly::DialogueDropped,
        Costly::SdhAsRegular,
        Costly::RegularAsSdh,
        Costly::ForcedWrongly,
        Costly::ForcedAsDialogue,
        Costly::UnwantedKept,
    ];

    /// The verdict's snake_case spelling, its serde name. Spec 17 row 14 (a).
    pub fn as_str(self) -> &'static str {
        match self {
            Costly::DialogueDropped => "dialogue_dropped",
            Costly::SdhAsRegular => "sdh_as_regular",
            Costly::RegularAsSdh => "regular_as_sdh",
            Costly::ForcedWrongly => "forced_wrongly",
            Costly::ForcedAsDialogue => "forced_as_dialogue",
            Costly::UnwantedKept => "unwanted_kept",
        }
    }

    /// What the verdict is, in row 14's words, for the report's COSTLY lines. Spec 17 row 14 (a).
    pub fn meaning(self) -> &'static str {
        match self {
            Costly::DialogueDropped => "a dialogue track dropped, with or without a review",
            Costly::SdhAsRegular => "an SDH track placed as regular without review",
            Costly::RegularAsSdh => "a regular track placed as SDH without review",
            Costly::ForcedWrongly => "a track that is not forced flagged forced without review",
            Costly::ForcedAsDialogue => "a forced track placed as regular or SDH without review",
            Costly::UnwantedKept => "a commentary, trivia or lyrics track kept without review",
        }
    }
}

impl std::str::FromStr for Costly {
    type Err = String;

    /// Exactly the `as_str` spellings. Spec 17 row 14 (a).
    fn from_str(text: &str) -> Result<Costly, String> {
        Costly::ALL
            .into_iter()
            .find(|verdict| verdict.as_str() == text)
            .ok_or_else(|| format!("not a costly verdict: {text}"))
    }
}

/// What the rules gave a labelled track: the role the judged rule places it in (None = dropped) and whether a review
/// was raised. The harness builds two per track: the text rule's (what row 14 (a) judges; for a
/// stream the text rule sets aside, 8.7's) and 8.7's as the app decides before experiment 14 answers (shown beside it).
/// Spec 17 row 14 (a), 8.7 "Text". Def: T18. Use: T18.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub role: Option<SubtitleRole>,
    pub review: bool,
}

/// Row 14's dialogue kinds: dropping one is costly, with or without a review.
const DIALOGUE_KINDS: [SubtitleKind; 5] = [
    SubtitleKind::Regular,
    SubtitleKind::Sdh,
    SubtitleKind::Translation,
    SubtitleKind::DubTranscript,
    SubtitleKind::ClosedCaptions,
];
/// Row 14's unwanted kinds: keeping one without review is costly.
const UNWANTED_KINDS: [SubtitleKind; 3] = [SubtitleKind::Commentary, SubtitleKind::Trivia, SubtitleKind::Lyrics];

/// The costly verdict, if any, of one labelled track's outcome, checked in row 14's order (the first that applies): a
/// dialogue kind (regular, sdh, translation, dub-transcript, closed-captions) dropped; sdh as Regular or regular as Sdh
/// without review; Forced without review on a non-forced kind, or a forced kind as Regular or Sdh without review;
/// commentary, trivia or lyrics kept without review. Reads `role` and `review` only. Spec 17 row 14 (a). Def: T18.
/// Use: T18.
pub fn costly(kind: SubtitleKind, outcome: &Outcome) -> Option<Costly> {
    let quiet = !outcome.review;
    match outcome.role {
        None if DIALOGUE_KINDS.contains(&kind) => Some(Costly::DialogueDropped),
        Some(SubtitleRole::Regular) if quiet && kind == SubtitleKind::Sdh => Some(Costly::SdhAsRegular),
        Some(SubtitleRole::Sdh) if quiet && kind == SubtitleKind::Regular => Some(Costly::RegularAsSdh),
        Some(SubtitleRole::Forced) if quiet && kind != SubtitleKind::Forced => Some(Costly::ForcedWrongly),
        Some(SubtitleRole::Regular | SubtitleRole::Sdh) if quiet && kind == SubtitleKind::Forced => {
            Some(Costly::ForcedAsDialogue)
        }
        Some(_) if quiet && UNWANTED_KINDS.contains(&kind) => Some(Costly::UnwantedKept),
        _ => None,
    }
}

// ------------------------------------------------------------------ the harness

/// The reading folder of a rip label under the OCR text, `<text>/<slug>/<label>/subs-ocr/`: the goldens' layout, the
/// folder ocr-subs.ps1 writes and `lazymkv-dev decide` reads first.
const READING: &str = "subs-ocr";
/// How to give the harness the OCR text, at the end of every reason that names none.
const TEXT_HINT: &str = "(give --text DIR or LAZYMKV_OCR_TEXT)";
/// The width the count block pads its names to.
const COLUMN: usize = 22;

/// What a recording's tmdb.json says it is, for row 14's "1 feature film and 1 TV episode".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Media {
    Film,
    Tv,
    Unknown,
}

/// What one rip label gives every labelled track of it.
struct Rip {
    title: Title,
    report: VerifyReport,
    events: BTreeMap<u32, Vec<f64>>,
    decision: SubtitleDecision,
    /// The text rule's decisions by scan stream, or why the title's text was not read.
    text: Result<BTreeMap<u32, Decision>, String>,
}

/// A labelled track mapped onto an English or no-language subtitle stream of its rip.
struct Mapped {
    /// The rip's title and the scan stream.
    title: u32,
    stream: u32,
    events: usize,
    family: Family,
    dvd: bool,
    /// 8.7 as the app decides before experiment 14 answers (the text adding reviews where the app reads it, 6.4 step
    /// 10 and 8.7 "Text").
    rule: Outcome,
    /// The text rule's decision for the stream (None when it gave none), or why the text was not read.
    text: Result<Option<Decision>, String>,
    /// What row 14 (a) judges: the text rule's outcome, or 8.7's for a stream the text rule sets aside; None when the
    /// text was not read.
    judged: Option<Outcome>,
}

/// Where a labelled track ended up.
enum Place {
    /// Not judged and not among the tracks: its scan or rip could not be read, or its file track is no subtitle
    /// stream of the rip.
    Unreadable(String),
    /// A subtitle stream in another language than English and none: listed, never judged (row 14 covers English and
    /// no-language tracks).
    OtherLanguage { stream: u32, language: String },
    Track(Mapped),
}

/// One labelled track and what the harness found for it.
struct Judged {
    slug: String,
    media: Media,
    entry: SubtitleLabel,
    place: Place,
}

impl Judged {
    fn mapped(&self) -> Option<&Mapped> {
        match &self.place {
            Place::Track(mapped) => Some(mapped),
            _ => None,
        }
    }

    fn outcome(&self) -> Option<&Outcome> {
        self.mapped().and_then(|mapped| mapped.judged.as_ref())
    }

    /// "<slug>/<rip label> stream <file track>".
    fn key(&self) -> String {
        format!("{}/{} stream {}", self.slug, self.entry.label, self.entry.stream)
    }

    /// Why the track was not judged; None when it was, and for a track in another language.
    fn why_not(&self) -> Option<String> {
        match &self.place {
            Place::Unreadable(why) => Some(why.clone()),
            Place::OtherLanguage { .. } => None,
            Place::Track(mapped) if mapped.judged.is_some() => None,
            Place::Track(mapped) => Some(match &mapped.text {
                Err(why) => why.clone(),
                Ok(_) => format!("the text rule gave scan stream {} no verdict", mapped.stream),
            }),
        }
    }
}

/// The harness's result. Spec 15, 17 row 14 (a), RP15. Def: T18. Use: T18.
#[derive(Clone, Debug, PartialEq)]
pub struct SubtitleHarness {
    /// Counted labels per kind, every kind of `SubtitleKind::ALL` present (0 when none counts).
    pub counts: BTreeMap<String, u32>,
    /// ("<slug>/<rip label> stream <file track>", verdict) of every costly verdict, in the report's track order.
    pub costly: Vec<(String, Costly)>,
    /// Judged tracks placed without review (row 14 (a)'s "at least half").
    pub placed_without_review: u32,
    /// Every label of every valid labels file.
    pub labelled: u32,
    /// Row 14 (a)'s totals not reached, by name, in the row's order.
    pub missing: Vec<String>,
    /// The report.
    pub lines: Vec<String>,
    /// 0; 1 a costly verdict; 2 a labels file that does not validate; 3 no label, or a label that could not be judged.
    pub exit_code: i32,
}

/// Every labelled English or no-language track of every rip label of the evidence: decide_subtitles over the
/// recording's scan and subs/ packets (the rip verified with `verify_rip` over `selected_streams`, events through
/// `verify::subtitle_events` with settings.sub_clear_max_bytes), text::decide with `settings.text_rule()` when OCR text
/// is found under `text` (re-keyed through `verify::by_scan_stream`), the costly verdicts, the counts against `row14`
/// (counting only `read` and `watched` labels the owner is sure of, given before the verdict, never a fitting disc's;
/// row 14's UK-style SDH track from `style` "uk" on a counted `sdh` label, RP25), the report lines, exit 1 on
/// any costly verdict, 2 when a labels file does not validate (the run stops), 3 when there is no label or a labelled
/// track could not be judged (spec 15: "separately, when some evidence could not be read"). The recordings are the
/// slug folders of `evidence` holding subtitle-labels.json. Spec 15, 17 row 14 (a), 8.7 "Text", RP15. Def: T18.
/// Use: T18 (retune --subtitles).
pub fn harness(evidence: &dyn Tree, text: Option<&dyn Tree>, row14: &Row14, settings: &Settings) -> SubtitleHarness {
    let root = PathKey::root();
    let shown = evidence.display(&root);
    let recordings: Vec<PathKey> = evidence
        .list(&root)
        .into_iter()
        .filter(|dir| evidence.is_dir(dir) && is_slug(dir.name()) && evidence.is_file(&dir.join(FILE_NAME)))
        .collect();
    let mut files = Vec::new();
    let mut bad = Vec::new();
    for dir in &recordings {
        // load's Err already starts with the file as shown ("<shown>: <problems>", Task 17).
        match load(evidence, &dir.join(FILE_NAME)) {
            Ok(entries) => files.push((dir.clone(), entries)),
            Err(problems) => bad.push(problems),
        }
    }
    if !bad.is_empty() {
        let mut lines = vec![format!("subtitle labels: {shown}: {} labels file(s) do not validate", bad.len())];
        lines.extend(bad.iter().map(|problem| format!("  {problem}")));
        lines.push(String::new());
        lines.push("RESULT FAILED: a subtitle labels file does not validate (lazymkv-dev labels --subtitles check)".to_string());
        return SubtitleHarness {
            counts: BTreeMap::new(),
            costly: Vec::new(),
            placed_without_review: 0,
            labelled: 0,
            missing: Vec::new(),
            lines,
            exit_code: 2,
        };
    }
    let mut judged: Vec<Judged> = files
        .iter()
        .flat_map(|(dir, entries)| judge_recording(evidence, text, dir, entries, settings))
        .collect();
    judged.sort_by(|a, b| {
        (a.slug.as_str(), a.entry.label.as_str(), a.entry.stream).cmp(&(b.slug.as_str(), b.entry.label.as_str(), b.entry.stream))
    });
    report(&shown, files.len(), &judged, row14, settings)
}

/// TMDb's kind of the recording from its tmdb.json: `name` a show, `title` a film, else (or unreadable) unknown.
fn media(evidence: &dyn Tree, dir: &PathKey) -> Media {
    let path = dir.join("tmdb.json");
    if !evidence.is_file(&path) {
        return Media::Unknown;
    }
    match read_json::<Tmdb>(evidence, &path) {
        Ok(tmdb) if tmdb.name.is_some() => Media::Tv,
        Ok(tmdb) if tmdb.title.is_some() => Media::Film,
        _ => Media::Unknown,
    }
}

/// Every label of one recording: its scan once, then each rip label once for the labels on it.
fn judge_recording(
    evidence: &dyn Tree,
    text: Option<&dyn Tree>,
    dir: &PathKey,
    entries: &[SubtitleLabel],
    settings: &Settings,
) -> Vec<Judged> {
    let slug = dir.name().to_string();
    let media = media(evidence, dir);
    let judged = |entry: &SubtitleLabel, place: Place| Judged { slug: slug.clone(), media, entry: entry.clone(), place };
    let scanned = read_text(evidence, &dir.join("scan.txt"), Encoding::Utf8Sig, Errors::Replace)
        .map(|log| parse(&log))
        .and_then(|scan| disc(&scan).map(|typed| (scan, typed)));
    let (scan, typed) = match scanned {
        Ok(found) => found,
        Err(why) => return entries.iter().map(|entry| judged(entry, Place::Unreadable(why.clone()))).collect(),
    };
    let mut by_label: BTreeMap<&str, Vec<&SubtitleLabel>> = BTreeMap::new();
    for entry in entries {
        by_label.entry(entry.label.as_str()).or_default().push(entry);
    }
    let mut out = Vec::new();
    for (label, labelled) in by_label {
        match read_rip(evidence, text, dir, label, &scan, &typed, settings) {
            Ok(rip) => out.extend(labelled.into_iter().map(|entry| judged(entry, map_track(entry, &rip, typed.is_dvd)))),
            Err(why) => out.extend(labelled.into_iter().map(|entry| judged(entry, Place::Unreadable(why.clone())))),
        }
    }
    out
}

/// One rip label as 6.4 step 10 and 8.7 read it: rip_label's title and selection, the rip verified against the scan
/// (`verify_rip` over `selected_streams`, with its rip log), the events of subs/ keyed by scan stream, MSG:4047's
/// removals mapped to scan streams (`verify::removed_scan_streams`), the text rule's decisions when the OCR text is
/// there, and `decide_subtitles` with them. `decide_subtitles` gets the text only where the app reads it (6.4 step 10,
/// 8.7 "Text"): a file holding two or more English or no-language subtitle streams; a title with one keeps it as
/// "English" without the text (row 14 (b)'s fallback), so the 8.7 column shows what the app decides. The text rule's
/// decisions are still judged for every title (row 14 (a) judges the text rule). Err when a file cannot be read or the
/// rip does not verify (its file tracks could not be mapped).
fn read_rip(
    evidence: &dyn Tree,
    text: Option<&dyn Tree>,
    dir: &PathKey,
    label: &str,
    scan: &Scan,
    typed: &Disc,
    settings: &Settings,
) -> Result<Rip, String> {
    let rip = rip_label(evidence, dir, label)?;
    let number = u32::try_from(rip.title).map_err(|_| format!("{label}: the summary names title {}", rip.title))?;
    let title = typed.title(number)?.clone();
    let selection = selected_streams(scan, number, rip.selection)?;
    let folder = dir.join(label);
    let ident: MkvmergeIdent = read_json(evidence, &folder.join("mkvmerge.json"))?;
    let log_path = RIP_LOG_NAMES.iter().map(|name| folder.join(name)).find(|path| evidence.is_file(path));
    let rip_log = match &log_path {
        Some(path) => Some(read_text(evidence, path, Encoding::Utf8Sig, Errors::Replace)?),
        None => None,
    };
    let report = verify_rip(&title, &ident, &selection, rip_log.as_deref(), typed.is_dvd);
    if !report.ok {
        return Err(format!("{label}: the rip does not verify against the scan: {}", report.mismatches.join("; ")));
    }
    let events = subtitle_events(evidence, &folder.join("subs"), &report, settings.sub_clear_max_bytes)?;
    let removed = removed_scan_streams(rip_log.as_deref(), &selection);
    let english_in_file = title
        .streams
        .iter()
        .filter(|s| {
            s.kind == StreamKind::Subtitles
                && selection.contains(&s.number)
                && !removed.contains(&s.number)
                && english_or_none(s.language.as_deref())
        })
        .count();
    let text = text_decisions(evidence, text, dir, label, &report, settings);
    let decision = {
        let facts = SubtitleFacts {
            events: events.clone(),
            removed,
            discdb: None,
            text: if english_in_file >= 2 { text.as_ref().ok() } else { None },
            remembered: None,
            reviewed: None,
        };
        decide_subtitles(&title, &facts, settings)
    };
    Ok(Rip { title, report, events, decision, text })
}

/// The text rule's decisions for one rip label, keyed by scan stream: `subs_run` over the label folder with the OCR
/// text of `<text>/<slug>/<label>/subs-ocr/`, `settings.sub_clear_max_bytes` and `settings.text_rule()`, the original
/// language from the recording's tmdb.json; Err with why the text was not read (no text given, no reading folder,
/// or subs_run's own refusal).
fn text_decisions(
    evidence: &dyn Tree,
    text: Option<&dyn Tree>,
    dir: &PathKey,
    label: &str,
    report: &VerifyReport,
    settings: &Settings,
) -> Result<BTreeMap<u32, Decision>, String> {
    let text_tree = text.ok_or_else(|| format!("no OCR text {TEXT_HINT}"))?;
    let folder = dir.join(label);
    let reading = folder.join(READING);
    if !text_tree.is_dir(&reading) {
        return Err(format!("no OCR text: {} {TEXT_HINT}", text_tree.display(&reading)));
    }
    let run = subs_run(evidence, &folder, text_tree, &reading, None, settings.sub_clear_max_bytes, &settings.text_rule());
    if run.exit_code != 0 {
        return Err(run.stderr.join("; "));
    }
    let analysis = run.analysis.ok_or_else(|| "the text rule gave no analysis".to_string())?;
    let index = subs_index(evidence, &folder.join("subs"))?;
    Ok(by_scan_stream(&analysis.decide, &subs_streams(&index, report)))
}

/// Maps one label onto its rip: the file track through the verified pairs, which must be a subtitle stream; a stream
/// in another language is listed apart; an English or no-language one gets 8.7's outcome and the judged one.
fn map_track(entry: &SubtitleLabel, rip: &Rip, dvd: bool) -> Place {
    let Some(stream) = stream_of_track(&rip.report, entry.stream) else {
        return Place::Unreadable(format!("file track {} is not a track of the rip", entry.stream));
    };
    let scan_stream = match rip.title.streams.iter().find(|candidate| candidate.number == stream) {
        Some(found) if found.kind == StreamKind::Subtitles => found,
        Some(found) => {
            return Place::Unreadable(format!(
                "file track {} is scan stream {stream}, a {} stream, not a subtitle stream",
                entry.stream,
                found.kind.as_str()
            ))
        }
        None => return Place::Unreadable(format!("file track {} is not a track of the rip", entry.stream)),
    };
    let language = scan_stream.language.as_deref();
    if !english_or_none(language) {
        return Place::OtherLanguage { stream, language: language.unwrap_or_default().to_string() };
    }
    let text = match &rip.text {
        Ok(decisions) => Ok(decisions.get(&stream).cloned()),
        Err(why) => Err(why.clone()),
    };
    let rule = rule_outcome(&rip.decision, stream);
    let judged = match &text {
        Ok(Some(decision)) if decision.role == Role::SetAside => Some(rule.clone()),
        Ok(Some(decision)) => Some(text_outcome(decision)),
        _ => None,
    };
    Place::Track(Mapped {
        title: rip.title.number,
        stream,
        events: rip.events.get(&stream).map_or(0, Vec::len),
        family: scan_stream.codec.family,
        dvd,
        rule,
        text,
        judged,
    })
}

/// 8.7's outcome for a stream, as the app decides before experiment 14 answers: its kept role (None when dropped) and
/// whether the title raised subtitle_ambiguous.
fn rule_outcome(decision: &SubtitleDecision, stream: u32) -> Outcome {
    Outcome {
        role: decision.kept.iter().find(|kept| kept.stream == stream).map(|kept| kept.role),
        review: decision.reviews.contains(&ReviewReason::SubtitleAmbiguous),
    }
}

/// The text rule's outcome: regular, sdh and forced place the track, drop drops it, keep keeps it unplaced (always
/// with a review: counted as kept as regular), with the decision's own review.
fn text_outcome(decision: &Decision) -> Outcome {
    let role = match decision.role {
        Role::Regular | Role::Keep => Some(SubtitleRole::Regular),
        Role::Sdh => Some(SubtitleRole::Sdh),
        Role::Forced => Some(SubtitleRole::Forced),
        Role::Drop | Role::SetAside => None,
    };
    Outcome { role, review: decision.review.is_some() }
}

/// "regular", "sdh", "forced" or "dropped", with " (review)" when a review was raised.
fn outcome_word(outcome: &Outcome) -> String {
    let role = outcome.role.map_or("dropped", SubtitleRole::as_str);
    if outcome.review {
        format!("{role} (review)")
    } else {
        role.to_string()
    }
}

/// The text rule's role for the report, with " (review)"; "no verdict" or "not read" without one.
fn text_word(text: &Result<Option<Decision>, String>) -> String {
    match text {
        Ok(Some(decision)) if decision.review.is_some() => format!("{} (review)", decision.role.as_str()),
        Ok(Some(decision)) => decision.role.as_str().to_string(),
        Ok(None) => "no verdict".to_string(),
        Err(_) => "not read".to_string(),
    }
}

/// One line of the Tracks section.
fn track_line(judged: &Judged, mapped: &Mapped) -> String {
    let entry = &judged.entry;
    let sure = if entry.sure { "" } else { ", not sure" };
    let seen = if entry.saw_verdict { ", saw the verdict" } else { "" };
    let style = entry.style.as_deref().map_or(String::new(), |style| format!(", style {style}"));
    let verdict = match &mapped.judged {
        Some(outcome) => match costly(entry.kind, outcome) {
            Some(found) => format!("COSTLY {}", found.as_str()),
            None if outcome.review => "reviewed".to_string(),
            None => "placed".to_string(),
        },
        None => "not judged".to_string(),
    };
    format!(
        "  {} (scan stream {}, {} events): labelled {} ({}{sure}{seen}{style}); 8.7 {}; text {}; {verdict}",
        judged.key(),
        mapped.stream,
        mapped.events,
        entry.kind.as_str(),
        entry.how,
        outcome_word(&mapped.rule),
        text_word(&mapped.text)
    )
}

/// A count-block line: two spaces, the name padded to COLUMN, the tail.
fn column(name: &str, tail: &str) -> String {
    format!("  {name:<width$}{tail}", width = COLUMN)
}

/// What the count block says after a kind's count.
fn kind_tail(kind: SubtitleKind, count: u32, forced_with_events: u32, row14: &Row14) -> String {
    match kind {
        SubtitleKind::Regular => format!("{count} of {}", row14.regular),
        SubtitleKind::Sdh => format!("{count} of {}", row14.sdh),
        SubtitleKind::Forced => format!("{count}, with events {forced_with_events} of {}", row14.forced),
        SubtitleKind::Commentary => format!("{count} of {}", row14.commentary),
        SubtitleKind::Trivia | SubtitleKind::Lyrics | SubtitleKind::DubTranscript => {
            format!("{count} (every one the owner has; checked by hand)")
        }
        SubtitleKind::Translation => format!("{count} (every second translation the owner has; checked by hand)"),
        SubtitleKind::ClosedCaptions => format!("{count} (with its DVD's VobSub tracks)"),
        SubtitleKind::Other => count.to_string(),
    }
}

/// The counts, the answer, the report lines and the exit code over the judged labels.
fn report(shown: &str, recordings: usize, judged: &[Judged], row14: &Row14, settings: &Settings) -> SubtitleHarness {
    let fitting = |slug: &str| row14.fitting_discs.iter().any(|fitted| fitted == slug);
    // D23 and RP25: judged here, read or watched, the owner sure, given before the verdict was seen, off
    // the fitting discs.
    let counts_here = |j: &Judged| {
        j.outcome().is_some()
            && COUNTING_HOW.contains(&j.entry.how.as_str())
            && j.entry.sure
            && !j.entry.saw_verdict
            && !fitting(&j.slug)
    };
    let counted: Vec<&Judged> = judged.iter().filter(|&j| counts_here(j)).collect();
    let mut counts: BTreeMap<String, u32> = BTreeMap::new();
    for kind in SubtitleKind::ALL {
        counts.insert(kind.as_str().to_string(), counted.iter().filter(|j| j.entry.kind == kind).count() as u32);
    }
    let forced_with_events = counted
        .iter()
        .filter(|j| j.entry.kind == SubtitleKind::Forced && j.mapped().is_some_and(|mapped| mapped.events > 0))
        .count() as u32;
    let titles_of = |kind: SubtitleKind| -> BTreeSet<(String, u32)> {
        counted
            .iter()
            .filter(|j| j.entry.kind == kind)
            .filter_map(|j| j.mapped().map(|mapped| (j.slug.clone(), mapped.title)))
            .collect()
    };
    let regular_titles = titles_of(SubtitleKind::Regular);
    let sdh_titles = titles_of(SubtitleKind::Sdh);
    let pairs: BTreeSet<(String, u32)> = regular_titles.intersection(&sdh_titles).cloned().collect();
    let pair_titles = pairs.len() as u32;
    let pair_discs = pairs.iter().map(|(slug, _)| slug.as_str()).collect::<BTreeSet<&str>>().len() as u32;
    let media_of = |slug: &str| judged.iter().find(|j| j.slug == slug).map_or(Media::Unknown, |j| j.media);
    let pair_features = pairs.iter().filter(|(slug, _)| media_of(slug) == Media::Film).count() as u32;
    let pair_tv = pairs.iter().filter(|(slug, _)| media_of(slug) == Media::Tv).count() as u32;
    let dvd_vobsub = counted
        .iter()
        .filter(|j| j.mapped().is_some_and(|mapped| mapped.dvd && mapped.family == Family::VobSub))
        .map(|j| j.slug.as_str())
        .collect::<BTreeSet<&str>>()
        .len() as u32;
    // Row 14's UK-style SDH track (RP25): a counted sdh label whose style is "uk".
    let uk_sdh = counted
        .iter()
        .filter(|j| j.entry.kind == SubtitleKind::Sdh && j.entry.style.as_deref() == Some("uk"))
        .count() as u32;
    let judged_count = judged.iter().filter(|j| j.outcome().is_some()).count() as u32;
    let placed = judged.iter().filter(|j| j.outcome().is_some_and(|outcome| !outcome.review)).count() as u32;
    let costly_found: Vec<(String, Costly)> = judged
        .iter()
        .filter_map(|j| {
            let outcome = j.outcome()?;
            costly(j.entry.kind, outcome).map(|found| (j.key(), found))
        })
        .collect();
    let not_judged: Vec<(String, String)> = judged.iter().filter_map(|j| j.why_not().map(|why| (j.key(), why))).collect();
    let others: Vec<&Judged> = judged.iter().filter(|j| matches!(j.place, Place::OtherLanguage { .. })).collect();
    let checks = [
        (pair_titles, row14.pair_titles, "titles with regular and SDH"),
        (pair_discs, row14.pair_discs, "discs with such titles"),
        (pair_features, row14.pair_features, "feature films with such a title"),
        (pair_tv, row14.pair_tv, "TV episodes with such a title"),
        (counts["sdh"], row14.sdh, "SDH tracks"),
        (counts["regular"], row14.regular, "regular tracks"),
        (forced_with_events, row14.forced, "forced tracks with events"),
        (counts["commentary"], row14.commentary, "commentary tracks"),
        (dvd_vobsub, row14.dvd_vobsub, "DVDs with English VobSub"),
    ];
    let missing: Vec<String> =
        checks.iter().filter(|(have, want, _)| have < want).map(|(_, _, name)| name.to_string()).collect();
    let answer = if !costly_found.is_empty() {
        format!("no: {} costly verdict(s)", costly_found.len())
    } else if !not_judged.is_empty() {
        format!("not answered: {} labelled track(s) not judged", not_judged.len())
    } else if !missing.is_empty() {
        format!("not answered: too few {}", missing.join(", "))
    } else if placed * 2 < judged_count {
        "no: fewer than half of the judged tracks are placed without review".to_string()
    } else {
        "yes for (a) on the tracks judged (row 14 also asks (b))".to_string()
    };
    let mut not_counted: BTreeMap<&str, usize> = BTreeMap::new();
    for j in judged.iter().filter(|&j| !counts_here(j)) {
        *not_counted.entry(j.slug.as_str()).or_default() += 1;
    }
    let not_counted_text = if not_counted.is_empty() {
        "none".to_string()
    } else {
        not_counted.iter().map(|(slug, count)| format!("{slug} {count}")).collect::<Vec<String>>().join(", ")
    };

    let mut lines = Vec::new();
    lines.push(format!(
        "subtitle labels: {shown}: {recordings} recordings with {FILE_NAME}, {} labelled tracks: {judged_count} judged, {} not judged, {} in another language",
        judged.len(),
        not_judged.len(),
        others.len()
    ));
    lines.push(format!(
        "rule: 8.7 with sub_clear_max_bytes {}, subs_tolerance_ms {}, subs_same_min {}, subs_diff_max {}; the text rule (EXP-14-design.md section 3) with the same four and its other thresholds as in the code, all provisional",
        settings.sub_clear_max_bytes, settings.subs_tolerance_ms, settings.subs_same_min, settings.subs_diff_max
    ));
    lines.push(String::new());
    lines.push(
        "Tracks (the owner's label; 8.7 as the app decides before experiment 14 answers; the text rule as row 14 (a) judges it):"
            .to_string(),
    );
    let start = lines.len();
    for j in judged {
        if let Some(mapped) = j.mapped() {
            lines.push(track_line(j, mapped));
        }
    }
    if lines.len() == start {
        lines.push("  none".to_string());
    }
    lines.push(String::new());
    let never = if row14.fitting_discs.is_empty() {
        String::new()
    } else {
        format!(", never on {}", row14.fitting_discs.join(" or "))
    };
    lines.push(format!(
        "Experiment 14 (spec section 17 row 14 (a); tracks that count: judged here, labelled from text read or watched before the verdict, the owner sure{never}; the packaging, the menu, a flag, the player and TheDiscDB are observations and do not count on their own, D23):"
    ));
    for kind in SubtitleKind::ALL {
        lines.push(column(kind.as_str(), &kind_tail(kind, counts[kind.as_str()], forced_with_events, row14)));
    }
    lines.push(column(
        "regular and SDH",
        &format!(
            "{pair_titles} of {} titles, from {pair_discs} of {} discs; feature films {pair_features} of {}, TV episodes {pair_tv} of {}",
            row14.pair_titles, row14.pair_discs, row14.pair_features, row14.pair_tv
        ),
    ));
    lines.push(column("DVD VobSub", &format!("{dvd_vobsub} of {} DVDs with a counted English VobSub track", row14.dvd_vobsub)));
    // Row 14's UK-style SDH track, counted from the labels' style (RP25); never among the missing kinds, since row 14
    // asks for it only if the owner has a UK disc, which the harness cannot know.
    lines.push(column(
        "UK-style SDH",
        &format!("{uk_sdh} of {} counted SDH tracks labelled style uk (only if the owner has a UK disc)", row14.uk_sdh),
    ));
    lines.push(column(
        "placed, no review",
        &format!("{placed} of {judged_count} judged tracks (row 14 (a) asks for at least half)"),
    ));
    lines.push(column("costly verdicts", &costly_found.len().to_string()));
    lines.push(format!("  as it stands: {answer}"));
    lines.push(format!("  not counted: {not_counted_text}"));
    lines.push(String::new());
    lines.push("Costly verdicts (row 14 (a): the text rule's verdict, or 8.7's for a stream the text rule sets aside):".to_string());
    for j in judged {
        if let Some(found) = j.outcome().and_then(|outcome| costly(j.entry.kind, outcome)) {
            lines.push(format!("  COSTLY {}: labelled {}, {}", j.key(), j.entry.kind.as_str(), found.meaning()));
        }
    }
    if costly_found.is_empty() {
        lines.push("  none".to_string());
    }
    lines.push(String::new());
    lines.push("Labels that cannot be judged:".to_string());
    for (key, why) in &not_judged {
        lines.push(format!("  {key}: {why}"));
    }
    if not_judged.is_empty() {
        lines.push("  none".to_string());
    }
    lines.push(String::new());
    lines.push("Labels in another language (row 14 judges English and no-language tracks only):".to_string());
    for j in &others {
        if let Place::OtherLanguage { stream, language } = &j.place {
            lines.push(format!("  {}: labelled {}, scan stream {stream} is in {language}", j.key(), j.entry.kind.as_str()));
        }
    }
    if others.is_empty() {
        lines.push("  none".to_string());
    }
    lines.push(String::new());
    let (result, exit_code) = if !costly_found.is_empty() {
        let named: Vec<String> = costly_found.iter().map(|(key, found)| format!("{key} ({})", found.as_str())).collect();
        let mut result = format!("RESULT FAILED: {} costly verdict(s): {}", costly_found.len(), named.join("; "));
        if !not_judged.is_empty() {
            result.push_str(&format!("; not judged: {} labelled track(s)", not_judged.len()));
        }
        (result, 1)
    } else if judged.is_empty() {
        ("RESULT FAILED: no subtitle label to judge (lazymkv-dev labels --subtitles set)".to_string(), 3)
    } else if !not_judged.is_empty() {
        (
            format!(
                "RESULT FAILED: not everything was judged: {} labelled track(s) (see Labels that cannot be judged)",
                not_judged.len()
            ),
            3,
        )
    } else {
        ("RESULT ok".to_string(), 0)
    };
    lines.push(result);
    SubtitleHarness {
        counts,
        costly: costly_found,
        placed_without_review: placed,
        labelled: judged.len() as u32,
        missing,
        lines,
        exit_code,
    }
}

#[cfg(test)]
mod tests;
