//! The review document (phase 3 design, part 6.2): one value that says what is open on a held or filed job, what
//! lazymkv knows about each of its tracks, and which answers each item allows; and the way back from the owner's answer
//! words to `remembered::Overrides` and the labels they carry (part 6.4, 6.6). Pure: the engine hands in the paths,
//! subtitle lines and colliding files it read. Pull request 2 answers tracks and collisions only (ruling T32-R1).

use std::collections::{BTreeMap, BTreeSet};

use crate::decision::FileDecision;
use crate::disc::{Disc, Stream, Title};
use crate::ipc::job_summary;
use crate::job::{Held, State};
use crate::lang::is_english;
use crate::lookup::DescriptionKind;
use crate::menu::AudioRole;
use crate::record::{JobRecord, TitleRecord};
use crate::remembered::{
    AudioOverride, AudioTrackOverride, Collision, CollisionAnswer, Overrides, SubtitleOverride, SubtitleTrackOverride,
};
use crate::review::{sorted_unique, Noted, ReviewReason};
use crate::settings::same_path;
use crate::tracks::audio::AudioDrop;
use crate::tracks::subtitle::{SubtitleDrop, SubtitleTrack};
use crate::tracks::SubtitleRole;

/// The review document's format (phase 3 design, part 6.2). Def: P3-T4. Use: P3-T5, P3-T8, P3-T9, P3-T14.
pub const DOC_FORMAT: &str = "lazymkv review document v1";

/// The job folder's file of the owner's answers, which `lazymkv-dev`'s `--overrides FILE` also reads (phase 3 design,
/// part 6.4; ruling T32-R9). Def: P3-T7. Use: P3-T8, P3-T12, P3-T16.
pub const REVIEW_FILE: &str = "review.json";

/// The format of `REVIEW_FILE`, compared whole (phase 3 design, part 6.4). Def: P3-T7. Use: P3-T8, P3-T12, P3-T16.
pub const REVIEW_FORMAT: &str = "lazymkv review v1";

/// How the job stands for review: held at the disposition gate, held after a disc copy, or filed (phase 3 design, part
/// 6.2). Pull request 2 builds documents for `Gate` and `Filed` only. Def: P3-T4. Use: P3-T5, P3-T8, P3-T9, P3-T14.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeldAs {
    /// Held at the disposition gate (6.4 step 12).
    Gate,
    /// Held after a disc copy, titles not yet extracted (6.3 step 7).
    AfterCopy,
    /// In the library; every item can still be changed.
    Filed,
}

/// The kind of a point, in spec 8.8's order of reasons (phase 3 design, part 6.2's table). Def: P3-T4. Use: P3-T5,
/// P3-T8, P3-T9, P3-T14.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PointKind {
    /// Which film this is: a point of the job, not answerable in pull request 2.
    Identity,
    /// Which titles are the film: a point of the job, not answerable in pull request 2.
    Titles,
    /// A title's audio tracks.
    Audio,
    /// A title's subtitle tracks.
    Subtitles,
    /// A title's final path taken by a file this job did not make.
    Collision,
    /// A title's rip: accepted as it is by filing, or discarded (ruling T32-R4); no item.
    Rip,
}

/// What an item is (phase 3 design, part 6.2). Def: P3-T4. Use: P3-T5, P3-T8, P3-T9, P3-T14.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Audio,
    Subtitle,
    Collision,
}

/// A job's review document (phase 3 design, part 6.2). Field order is the JSON's key order. Def: P3-T4. Use: P3-T5,
/// P3-T8, P3-T9, P3-T14.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReviewDoc {
    /// `DOC_FORMAT`.
    pub format: String,
    /// The job's ID.
    pub job: String,
    pub held: HeldAs,
    /// When the job was held (the record's `updated_unix`) or filed (`filed_unix`), in Unix seconds.
    pub since_unix: u64,
    pub identity: DocIdentity,
    /// The job's review reasons, sorted (`ipc::job_summary`'s `review_reasons`).
    pub reasons: Vec<ReviewReason>,
    /// The job's notes.
    pub notes: Vec<Noted>,
    /// The points, in spec 8.8's order of reasons, then by title key.
    pub points: Vec<Point>,
    /// Every ripped title, by key.
    pub titles: Vec<DocTitle>,
}

/// The plan's identity as the sheet shows it (phase 3 design, part 6.2). Def: P3-T4. Use: P3-T5, P3-T8, P3-T9, P3-T14.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DocIdentity {
    /// `lookup::IdentityStep`'s spelling.
    pub step: String,
    pub confidence: f64,
    pub title: Option<String>,
    pub year: Option<i32>,
}

/// One point of the review: what is open and why, and the items that answer it (phase 3 design, part 6.2). Def: P3-T4.
/// Use: P3-T5, P3-T8, P3-T9, P3-T14.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Point {
    pub kind: PointKind,
    /// The reasons the point answers, sorted; empty for a point shown with `all` or on a filed job that no reason raised.
    pub reasons: Vec<ReviewReason>,
    /// The title the point is about; None for an identity or titles point.
    pub title_key: Option<String>,
    /// One plain sentence: what is open, then why lazymkv cannot settle it.
    pub sentence: String,
    /// Whether this sheet takes answers for it (false for identity, titles and rip points, ruling T32-R1 and T32-R4).
    pub answerable: bool,
    /// The ids of the items that answer it, as `DocTitle.items` lists them.
    pub items: Vec<String>,
}

/// One ripped title (phase 3 design, part 6.2). Def: P3-T4. Use: P3-T5, P3-T8, P3-T9, P3-T14.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DocTitle {
    /// The plan's title key.
    pub key: String,
    /// The scan's title number.
    pub scan_title: u32,
    /// The scan's length, chapters (None when it lists none) and size.
    pub seconds: Option<u32>,
    pub chapters: Option<u32>,
    pub bytes: Option<u64>,
    /// The staged file's path to watch, as the engine shows it.
    pub staged_path: Option<String>,
    /// The decision's final path as it stands.
    pub final_path: Option<String>,
    /// The title's items: its audio items by stream, its subtitle items by stream, then its collision item.
    pub items: Vec<Item>,
}

/// One thing the owner can answer: a track or a collision (phase 3 design, part 6.2; rulings T32-R2, T32-R3). Def:
/// P3-T4. Use: P3-T5, P3-T8, P3-T9, P3-T14.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Item {
    /// `<title key> audio <stream>`, `<title key> sub <stream>` or `<title key> collision`.
    pub id: String,
    pub kind: ItemKind,
    /// The scan's title number.
    pub title: u32,
    /// The scan's stream number; None for a collision.
    pub stream: Option<u32>,
    /// A collision's final path; None for a track.
    pub path: Option<String>,
    /// The language word, two spaces, then what the track is; a collision's path.
    pub describe: String,
    /// The answer word of what the decision did; "" for a collision.
    pub stands: String,
    /// An audio track's place among the kept audio tracks (the count of kept tracks for a dropped one); None otherwise.
    pub order_if_kept: Option<u32>,
    /// The answer words the item offers.
    pub answers: Vec<String>,
    pub evidence: Vec<Evidence>,
}

/// One piece of evidence for an item: `lines`, `unread`, `events`, `note`, `verdict` or `file` (phase 3 design, part
/// 6.2: open-ended, so later evidence needs no format change). Def: P3-T4. Use: P3-T5, P3-T8, P3-T9, P3-T14.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Evidence {
    pub kind: String,
    pub text: String,
}

/// What `build` reads: the job's record and scan, and what the engine read for it (phase 3 design, part 6.2). Def:
/// P3-T4. Use: P3-T5, P3-T8, P3-T9, P3-T14.
pub struct DocInputs<'a> {
    pub record: &'a JobRecord,
    /// The scan the plan was made from.
    pub disc: &'a Disc,
    /// Title key to the staged file's path as shown.
    pub staged: &'a BTreeMap<String, String>,
    /// (title key, stream) to the subtitle lines shown for it.
    pub lines: &'a BTreeMap<(String, u32), Vec<String>>,
    /// (title key, stream) to why it has no lines.
    pub unread: &'a BTreeMap<(String, u32), String>,
    /// Colliding final path to the file there, "<bytes> bytes, <date>".
    pub collisions: &'a BTreeMap<String, String>,
    /// The collision paths the engine looked at and found nothing at (a folder the share could not open has since
    /// opened): such a path has no collision item.
    pub nothing_at: &'a BTreeSet<String>,
    /// Every title gets an audio and a subtitles point, not only those a reason raised.
    pub all: bool,
}

/// What a label says a track is (phase 3 design, part 6.6). Def: P3-T4. Use: P3-T5, P3-T8, P3-T9, P3-T14.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelKind {
    Audio,
    Subtitle,
}

/// One label an answer gives: the scan title and stream, and the answer word that says what the track is (phase 3
/// design, part 6.6). Def: P3-T4. Use: P3-T5, P3-T8, P3-T9, P3-T14.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReviewLabel {
    pub title: u32,
    pub stream: u32,
    pub kind: LabelKind,
    pub label: String,
}

/// The answers of one review: the overrides, and the labels of the answers that say what a track is (phase 3 design,
/// parts 6.4 and 6.6). Def: P3-T4. Use: P3-T5, P3-T8, P3-T9, P3-T14.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Answers {
    pub overrides: Overrides,
    pub labels: Vec<ReviewLabel>,
}

const AUDIO_ANSWERS: [&str; 4] = ["keep", "default", "commentary", "drop"];
const SUBTITLE_ANSWERS: [&str; 5] = ["regular", "sdh", "forced", "commentary", "drop"];
const COLLISION_ANSWERS: [&str; 2] = ["keep_both", "replace"];

// One sentence per review reason: what is open, then why lazymkv cannot settle it. `point_sentence` puts the point's
// label before them.
const IDENTITY_UNKNOWN: &str =
    "lazymkv could not tell which film this disc holds. Neither the disc's name nor the film databases named it.";
const IDENTITY_AMBIGUOUS: &str = "lazymkv is not sure which film this disc holds. More than one film fits what it read, \
     or the one it found was matched by its name alone.";
const TITLE_AMBIGUOUS: &str = "lazymkv cannot tell which title on the disc is the film. Several titles look alike, or \
     the likeliest one runs far from the film's length.";
const MULTIPLE_FEATURES: &str =
    "the disc holds more than one title that looks like the film, and their contents differ. They may be different cuts.";
const UNNAMED_CUT: &str = "one title looks like another cut of the film, but nothing on the disc names that cut.";
const TV_UNMAPPED: &str = "this looks like a TV disc, and no database says which episodes its titles are. lazymkv \
     cannot file episodes yet.";
const ORIGINAL_LANGUAGE_MISSING: &str =
    "the film's original language is wanted, but no audio track on the disc is in that language.";
const NO_WANTED_AUDIO: &str = "no audio track is in a language you want, and none is without a language.";
const AUDIO_REFERENCE_UNCLEAR: &str = "the track that sounds most like the film is not the first track of its language. \
     lazymkv cannot be sure which one is the film's own soundtrack.";
const COMMENTARY_DETECTED: &str = "one track sounds like people talking over the film, as a commentary does. lazymkv \
     does not drop a track on that alone.";
const SUBTITLE_AMBIGUOUS: &str =
    "two full English tracks that do not match each other. lazymkv cannot tell which one carries the film's dialogue.";
const PATH_COLLISION: &str = "a file this job did not make is already where this title would be filed. lazymkv never \
     replaces a file it did not make without asking.";
const COPY_HASH_ERRORS: &str =
    "the copy of the disc finished, but parts of it failed their checks, so some picture or sound may be damaged.";
const VERIFY_FAILED: &str = "the ripped file failed its checks twice, so it may not be complete.";
const READ_ERRORS: &str =
    "the drive could not read parts of the disc, so the ripped file may skip or stutter in places.";
/// After a rip point's reasons (ruling T32-R4).
const RIP_CHOICE: &str = "Filing it accepts the rip as it is; the other way is to discard the job and rip the disc again.";
/// After an identity or titles point's reasons (ruling T32-R1).
const NOT_ON_THIS_SHEET: &str = "This sheet cannot answer it yet.";
/// What a collision point says when nothing is at the path, after "Nothing is at <path> now." (phase 3 design, part
/// 6.7: the folder the share could not open clears by itself, and the only answer is to try again).
pub const NOTHING_AT_PATH: &str =
    "A folder the share could not open clears by itself within minutes, so approving tries to file again.";
/// A point that no reason raised (`all`, or a filed job).
const NOTHING_OPEN: &str = "nothing is open here, and each track can still be changed.";

/// The sentence of `reason`.
fn sentence(reason: ReviewReason) -> &'static str {
    match reason {
        ReviewReason::IdentityUnknown => IDENTITY_UNKNOWN,
        ReviewReason::IdentityAmbiguous => IDENTITY_AMBIGUOUS,
        ReviewReason::TitleAmbiguous => TITLE_AMBIGUOUS,
        ReviewReason::MultipleFeatures => MULTIPLE_FEATURES,
        ReviewReason::UnnamedCut => UNNAMED_CUT,
        ReviewReason::TvUnmapped => TV_UNMAPPED,
        ReviewReason::OriginalLanguageMissing => ORIGINAL_LANGUAGE_MISSING,
        ReviewReason::NoWantedAudio => NO_WANTED_AUDIO,
        ReviewReason::AudioReferenceUnclear => AUDIO_REFERENCE_UNCLEAR,
        ReviewReason::CommentaryDetected => COMMENTARY_DETECTED,
        ReviewReason::SubtitleAmbiguous => SUBTITLE_AMBIGUOUS,
        ReviewReason::PathCollision => PATH_COLLISION,
        ReviewReason::CopyHashErrors => COPY_HASH_ERRORS,
        ReviewReason::VerifyFailed => VERIFY_FAILED,
        ReviewReason::ReadErrors => READ_ERRORS,
    }
}

/// The point kind a reason raises (phase 3 design, part 6.2's table).
fn kind_of(reason: ReviewReason) -> PointKind {
    match reason {
        ReviewReason::IdentityUnknown | ReviewReason::IdentityAmbiguous => PointKind::Identity,
        ReviewReason::TitleAmbiguous
        | ReviewReason::MultipleFeatures
        | ReviewReason::UnnamedCut
        | ReviewReason::TvUnmapped => PointKind::Titles,
        ReviewReason::OriginalLanguageMissing
        | ReviewReason::NoWantedAudio
        | ReviewReason::AudioReferenceUnclear
        | ReviewReason::CommentaryDetected => PointKind::Audio,
        ReviewReason::SubtitleAmbiguous => PointKind::Subtitles,
        ReviewReason::PathCollision => PointKind::Collision,
        ReviewReason::CopyHashErrors | ReviewReason::VerifyFailed | ReviewReason::ReadErrors => PointKind::Rip,
    }
}

/// The point's label, its reasons' sentences (each after the first capitalised) and its closing sentence.
fn point_sentence(kind: PointKind, reasons: &[ReviewReason]) -> String {
    let label = match kind {
        PointKind::Identity => "Which film",
        PointKind::Titles => "Titles",
        PointKind::Audio => "Audio",
        PointKind::Subtitles => "Subtitles",
        PointKind::Collision => "Already in the library",
        PointKind::Rip => "The rip",
    };
    let mut parts: Vec<String> = Vec::new();
    for (at, reason) in reasons.iter().enumerate() {
        let text = sentence(*reason);
        parts.push(if at == 0 { text.to_string() } else { capitalised(text) });
    }
    if parts.is_empty() {
        parts.push(NOTHING_OPEN.to_string());
    }
    match kind {
        PointKind::Identity | PointKind::Titles => parts.push(NOT_ON_THIS_SHEET.to_string()),
        PointKind::Rip => parts.push(RIP_CHOICE.to_string()),
        _ => {}
    }
    format!("{label}: {}", parts.join(" "))
}

fn capitalised(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// `n` with a comma between each group of three digits ("1,224").
fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (at, digit) in digits.chars().enumerate() {
        if at > 0 && (digits.len() - at) % 3 == 0 {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// `English` for an English tag, `no language` for none, else the tag as the scan wrote it.
fn language_word(code: Option<&str>) -> String {
    match code {
        Some(code) if is_english(code) => "English".to_string(),
        Some(code) => code.to_string(),
        None => "no language".to_string(),
    }
}

/// The language word, then two spaces and `what` when there is something to say.
fn describe(language: Option<&str>, what: Option<String>) -> String {
    match what {
        Some(what) if !what.is_empty() => format!("{}  {what}", language_word(language)),
        _ => language_word(language),
    }
}

fn evidence(kind: &str, text: impl Into<String>) -> Evidence {
    Evidence { kind: kind.to_string(), text: text.into() }
}

/// What TheDiscDB's description kind means, in words.
fn described(kind: DescriptionKind) -> &'static str {
    match kind {
        DescriptionKind::Commentary => "a commentary",
        DescriptionKind::AudioDescription => "a description for people who cannot see the picture",
        DescriptionKind::Sdh => "subtitles for the deaf and hard of hearing",
        DescriptionKind::IsolatedScore => "the music alone",
        DescriptionKind::MusicAndEffects => "music and effects without dialogue",
        DescriptionKind::Dialogue => "dialogue",
    }
}

/// Why an audio track was dropped, in words; None for a drop the owner cannot overrule, which is never an item
/// (ruling T32-R2).
fn audio_drop(why: &AudioDrop) -> Option<String> {
    let text = match why {
        AudioDrop::Derived | AudioDrop::UnwantedLanguage => return None,
        AudioDrop::Flagged { .. } => {
            "dropped: the disc marks it as a director's commentary or a description for people who cannot see the picture"
                .to_string()
        }
        AudioDrop::Secondary => "dropped: the disc marks it as picture-in-picture sound".to_string(),
        AudioDrop::Duplicate { of } => format!("dropped: it is an exact copy of track {of}"),
        AudioDrop::Described { kind } => format!("dropped: TheDiscDB describes it as {}", described(*kind)),
        AudioDrop::Remembered => "dropped: an earlier answer for this disc said so".to_string(),
        AudioDrop::Reviewed => "dropped: an earlier answer to this review said so".to_string(),
        // Menu maps design 5.4a (MA-1): the menu's word alone dropped it, which the owner's answer outranks.
        AudioDrop::Menu { role } => match role {
            AudioRole::Commentary => "dropped: the disc's menu calls it a commentary".to_string(),
            AudioRole::AudioDescription => {
                "dropped: the disc's menu calls it a description for people who cannot see the picture".to_string()
            }
            AudioRole::Programme => "dropped: the disc's menu said so".to_string(),
        },
    };
    Some(text)
}

/// Why a subtitle track was dropped, in words; None for a drop the owner cannot overrule (ruling T32-R2).
fn subtitle_drop(why: &SubtitleDrop) -> Option<&'static str> {
    let text = match why {
        SubtitleDrop::OtherLanguage
        | SubtitleDrop::DerivedForcedOnlyEmpty
        | SubtitleDrop::DerivedForcedOnlyUnused
        | SubtitleDrop::NoEvents => return None,
        SubtitleDrop::DirectorsComments => "dropped: the disc marks it as a director's commentary",
        SubtitleDrop::OutsideDialogueGroup => {
            "dropped: its timing does not follow the film's dialogue, so it is likely a commentary or trivia track"
        }
        SubtitleDrop::Partial => "dropped: it covers only part of the film",
        SubtitleDrop::ExtraDialogueTrack => "dropped: another kept track carries the same dialogue",
        SubtitleDrop::ExtraForced => "dropped: another forced track is kept",
        SubtitleDrop::ClosedCaptionsUnused => "dropped: these are the disc's closed captions, and an SDH track is kept",
        SubtitleDrop::Remembered => "dropped: an earlier answer for this disc said so",
        SubtitleDrop::Reviewed => "dropped: an earlier answer to this review said so",
    };
    Some(text)
}

/// What the decision did with a kept subtitle track and how it decided, in words.
fn subtitle_kept(track: &SubtitleTrack) -> String {
    let role = match track.role {
        SubtitleRole::Forced => "forced English",
        SubtitleRole::Regular => "regular English",
        SubtitleRole::Sdh => "English SDH (for the deaf and hard of hearing)",
    };
    let how = match track.decided_by.as_str() {
        "flag" => ", from the disc's own marking",
        "description" => ", from TheDiscDB's description",
        "count" => ", from how many lines it has",
        "alignment" => ", from the timing of its lines",
        "closed_captions" => ", as the disc's closed captions",
        "single" => ", as the only full English track",
        "remembered" => ", from an earlier answer for this disc",
        "reviewed" => ", from an earlier answer to this review",
        _ => "",
    };
    format!("kept as {role}{how}")
}

/// The scan stream `number` of `title`.
fn scan_stream(title: &Title, number: u32) -> Result<&Stream, String> {
    title
        .streams
        .iter()
        .find(|stream| stream.number == number)
        .ok_or_else(|| format!("the scan lists no stream {number} in title {}", title.number))
}

/// The notes of `decision` that name `stream`, as evidence.
fn notes_on(decision: &FileDecision, stream: u32) -> impl Iterator<Item = Evidence> + '_ {
    decision.notes.iter().filter(move |noted| noted.stream == Some(stream)).map(|noted| evidence("note", noted.detail.clone()))
}

fn track_item(key: &str, kind: ItemKind, title: u32, stream: u32) -> Item {
    let (word, answers): (&str, &[&str]) = match kind {
        ItemKind::Audio => ("audio", &AUDIO_ANSWERS),
        _ => ("sub", &SUBTITLE_ANSWERS),
    };
    Item {
        id: format!("{key} {word} {stream}"),
        kind,
        title,
        stream: Some(stream),
        path: None,
        describe: String::new(),
        stands: String::new(),
        order_if_kept: None,
        answers: answers.iter().map(|answer| answer.to_string()).collect(),
        evidence: Vec::new(),
    }
}

/// The audio items of a decided title, by stream: every kept track and every dropped one the owner can overrule.
fn audio_items(key: &str, scanned: &Title, decision: &FileDecision) -> Result<Vec<Item>, String> {
    let kept = &decision.audio.kept;
    // 8.10 and `tracks::layout`: the kept track flagged default, else the first kept track.
    let default = kept.iter().find(|track| track.default).or(kept.first()).map(|track| track.stream);
    let mut items = Vec::new();
    for (place, track) in kept.iter().enumerate() {
        let mut item = track_item(key, ItemKind::Audio, scanned.number, track.stream);
        let is_default = default == Some(track.stream);
        item.describe = describe(track.language.as_deref(), Some(track.name.clone()));
        item.stands = if is_default { "default" } else { "keep" }.to_string();
        item.order_if_kept = Some(place as u32);
        item.evidence.push(evidence("verdict", if is_default { "kept, and played by default" } else { "kept" }));
        item.evidence.extend(notes_on(decision, track.stream));
        items.push(item);
    }
    for (stream, why) in &decision.audio.dropped {
        let Some(verdict) = audio_drop(why) else { continue };
        let scan = scan_stream(scanned, *stream)?;
        let what = match scan.channels {
            Some(channels) => format!("{}, {channels} channels", scan.codec.display),
            None => scan.codec.display.clone(),
        };
        let mut item = track_item(key, ItemKind::Audio, scanned.number, *stream);
        item.describe = describe(scan.language.as_deref(), Some(what));
        item.stands = "drop".to_string();
        item.order_if_kept = Some(kept.len() as u32);
        item.evidence.push(evidence("verdict", verdict));
        item.evidence.extend(notes_on(decision, *stream));
        items.push(item);
    }
    items.sort_by_key(|item| item.stream);
    Ok(items)
}

/// The subtitle items of a decided title, by stream: every kept track and every dropped one the owner can overrule,
/// each with its lines, or why it has none.
fn subtitle_items(inputs: &DocInputs<'_>, key: &str, scanned: &Title, decision: &FileDecision) -> Result<Vec<Item>, String> {
    let mut items = Vec::new();
    let tracks = decision.subtitles.kept.iter().map(|track| (track.stream, Some(track), None));
    let dropped = decision.subtitles.dropped.iter().filter_map(|(stream, why)| subtitle_drop(why).map(|text| (*stream, None, Some(text))));
    for (stream, kept, why) in tracks.chain(dropped) {
        let scan = scan_stream(scanned, stream)?;
        let mut item = track_item(key, ItemKind::Subtitle, scanned.number, stream);
        let events = kept.and_then(|track| track.events);
        item.describe = describe(scan.language.as_deref(), events.map(|count| format!("{} lines", thousands(count))));
        match kept {
            Some(track) => {
                item.stands = track.role.as_str().to_string();
                item.evidence.push(evidence("verdict", subtitle_kept(track)));
            }
            None => {
                item.stands = "drop".to_string();
                item.evidence.push(evidence("verdict", why.unwrap_or_default()));
            }
        }
        if let Some(count) = events {
            item.evidence.push(evidence("events", format!("{} lines in the title", thousands(count))));
        }
        let at = (key.to_string(), stream);
        for line in inputs.lines.get(&at).into_iter().flatten() {
            item.evidence.push(evidence("lines", line.clone()));
        }
        if let Some(why) = inputs.unread.get(&at) {
            item.evidence.push(evidence("unread", why.clone()));
        }
        item.evidence.extend(notes_on(decision, stream));
        items.push(item);
    }
    items.sort_by_key(|item| item.stream);
    Ok(items)
}

/// A title's collision item: its final path, with the file there when the engine found it.
fn collision_item(inputs: &DocInputs<'_>, key: &str, title: u32, path: &str) -> Item {
    let file = inputs.collisions.iter().find(|(colliding, _)| same_path(colliding, path)).map(|(_, file)| file);
    Item {
        id: format!("{key} collision"),
        kind: ItemKind::Collision,
        title,
        stream: None,
        path: Some(path.to_string()),
        describe: path.to_string(),
        stands: String::new(),
        order_if_kept: None,
        answers: COLLISION_ANSWERS.iter().map(|answer| answer.to_string()).collect(),
        evidence: file.map(|file| evidence("file", file.clone())).into_iter().collect(),
    }
}

/// Whether the engine looked at `path` and found nothing there: it is in `nothing_at`, and no file is found at it
/// (a path in both sets cannot happen; the file wins).
fn is_empty_spot(inputs: &DocInputs<'_>, path: &str) -> bool {
    inputs.nothing_at.iter().any(|other| same_path(other, path))
        && !inputs.collisions.keys().any(|other| same_path(other, path))
}

/// The item kind that answers a point kind; None for the points that have no item.
fn item_kind(kind: PointKind) -> Option<ItemKind> {
    match kind {
        PointKind::Audio => Some(ItemKind::Audio),
        PointKind::Subtitles => Some(ItemKind::Subtitle),
        PointKind::Collision => Some(ItemKind::Collision),
        PointKind::Identity | PointKind::Titles | PointKind::Rip => None,
    }
}

/// A title's sorted reasons: its rip's and its decision's.
fn title_reasons(title: &TitleRecord) -> Vec<ReviewReason> {
    let mut reasons = title.rip_reviews.clone();
    reasons.extend(title.decision.iter().flat_map(|decision| decision.reviews.iter().copied()));
    sorted_unique(reasons)
}

/// The review document of a job held at the disposition gate or filed (phase 3 design, part 6.2; rulings T32-R1 to
/// T32-R4): its points in spec 8.8's order of reasons (a held job's only for the reasons it carries; with `all`, or for
/// a filed job, every title also gets an audio and a subtitles point listing all its items), and every ripped title with
/// its items. Err `job <id> is <state>: there is nothing to review` for any other state, and Err for a job with no plan
/// or a title or stream the scan lacks. A title whose final path is in `nothing_at` (and holds no file) has its
/// collision point without an item and says nothing is there (phase 3 design, part 6.7). Def: P3-T4. Use: P3-T5, P3-T8,
/// P3-T9, P3-T14.
pub fn build(inputs: &DocInputs<'_>) -> Result<ReviewDoc, String> {
    let record = inputs.record;
    let (held, since_unix) = match &record.state {
        State::NeedsReview { held: Held::DispositionGate } => (HeldAs::Gate, record.updated_unix),
        State::Filed => (HeldAs::Filed, record.filed_unix.unwrap_or(record.updated_unix)),
        other => return Err(format!("job {} is {}: there is nothing to review", record.id, other.as_str())),
    };
    let plan = record.plan.as_ref().ok_or_else(|| format!("job {} has no plan: there is nothing to review", record.id))?;
    let every = inputs.all || held == HeldAs::Filed;

    let mut ordered: Vec<&TitleRecord> = record.titles.iter().collect();
    ordered.sort_by(|a, b| a.key.cmp(&b.key));
    let mut job_reasons = plan.reviews.clone();
    let mut points = Vec::new();
    let mut titles = Vec::new();
    for title in ordered {
        let reasons = title_reasons(title);
        job_reasons.extend(reasons.iter().copied());
        let scanned = inputs.disc.title(title.scan_title)?;
        let mut items = Vec::new();
        // The final path of a title whose collision point has nothing at it: no collision item, only the sentence.
        let mut nothing_at: Option<String> = None;
        if let Some(decision) = &title.decision {
            items.extend(audio_items(&title.key, scanned, decision)?);
            items.extend(subtitle_items(inputs, &title.key, scanned, decision)?);
            if let Some(path) = decision.path.as_ref().filter(|_| reasons.contains(&ReviewReason::PathCollision)) {
                if is_empty_spot(inputs, &path.path) {
                    nothing_at = Some(path.path.clone());
                } else {
                    items.push(collision_item(inputs, &title.key, scanned.number, &path.path));
                }
            }
        }
        for kind in [PointKind::Audio, PointKind::Subtitles, PointKind::Collision, PointKind::Rip] {
            let raised: Vec<ReviewReason> = reasons.iter().copied().filter(|reason| kind_of(*reason) == kind).collect();
            let ids: Vec<String> = items
                .iter()
                .filter(|item| Some(item.kind) == item_kind(kind))
                .map(|item| item.id.clone())
                .collect();
            let shown_anyway = every && matches!(kind, PointKind::Audio | PointKind::Subtitles) && !ids.is_empty();
            if raised.is_empty() && !shown_anyway {
                continue;
            }
            let sentence = match (&nothing_at, kind) {
                (Some(path), PointKind::Collision) => format!("Nothing is at {path} now. {NOTHING_AT_PATH}"),
                _ => point_sentence(kind, &raised),
            };
            points.push(Point {
                kind,
                sentence,
                reasons: raised,
                title_key: Some(title.key.clone()),
                answerable: !ids.is_empty(),
                items: ids,
            });
        }
        titles.push(DocTitle {
            key: title.key.clone(),
            scan_title: title.scan_title,
            seconds: scanned.seconds,
            chapters: (scanned.chapters > 0).then_some(scanned.chapters),
            bytes: scanned.size,
            staged_path: inputs.staged.get(&title.key).cloned(),
            final_path: title.decision.as_ref().and_then(|decision| decision.path.as_ref()).map(|path| path.path.clone()),
            items,
        });
    }
    let job_reasons = sorted_unique(job_reasons);
    for kind in [PointKind::Identity, PointKind::Titles] {
        let raised: Vec<ReviewReason> = job_reasons.iter().copied().filter(|reason| kind_of(*reason) == kind).collect();
        if !raised.is_empty() {
            points.push(Point {
                kind,
                sentence: point_sentence(kind, &raised),
                reasons: raised,
                title_key: None,
                answerable: false,
                items: Vec::new(),
            });
        }
    }
    // Spec 8.8's order: the kinds follow the table's order of reasons; within a kind, the job's point, then by title.
    points.sort_by(|a, b| (a.kind, &a.title_key).cmp(&(b.kind, &b.title_key)));

    Ok(ReviewDoc {
        format: DOC_FORMAT.to_string(),
        job: record.id.clone(),
        held,
        since_unix,
        identity: DocIdentity {
            step: plan.identity.step.as_str().to_string(),
            confidence: plan.identity.confidence,
            title: plan.identity.title.clone(),
            year: plan.identity.year,
        },
        reasons: job_summary(record).review_reasons,
        notes: record.notes.clone(),
        points,
        titles,
    })
}

fn items_of(doc: &ReviewDoc) -> impl Iterator<Item = &Item> {
    doc.titles.iter().flat_map(|title| &title.items)
}

/// The item `id` of `doc` offering `word`, or the refusal.
fn offered<'a>(doc: &'a ReviewDoc, id: &str, word: &str) -> Result<&'a Item, String> {
    let item = items_of(doc).find(|item| item.id == id).ok_or_else(|| format!("no item \"{id}\" on this sheet"))?;
    if item.answers.iter().any(|answer| answer == word) {
        Ok(item)
    } else {
        Err(format!("\"{word}\" is not an answer for {id} ({})", item.answers.join(", ")))
    }
}

/// A track item's stream, or why the item names none.
fn stream_of(item: &Item) -> Result<u32, String> {
    item.stream.ok_or_else(|| format!("item {} names no stream", item.id))
}

/// The answers that `words` give: one (item id, answer word) pair per item changed (phase 3 design, part 6.4; ruling
/// T32-R3). `default` and `keep` keep an audio track at its `order_if_kept`, `commentary` marks it as commentary with a
/// label; a subtitle role keeps the track in that role with a label, a subtitle `commentary` drops it with a label,
/// `drop` drops it with none; `keep_both` and `replace` answer the collision of the item's path. A word equal to the
/// item's `stands` gives nothing; a later pair for the same item replaces an earlier one. Err
/// `no item "<id>" on this sheet` or `"<word>" is not an answer for <id> (<the item's words>)`. Def: P3-T4. Use: P3-T5,
/// P3-T8, P3-T9, P3-T14.
pub fn answers_of(doc: &ReviewDoc, words: &[(String, String)]) -> Result<Answers, String> {
    let mut chosen: BTreeMap<&str, &str> = BTreeMap::new();
    for (id, word) in words {
        offered(doc, id, word)?;
        chosen.insert(id, word);
    }
    let mut overrides = Overrides::default();
    let mut labels = Vec::new();
    for item in items_of(doc) {
        let Some(&word) = chosen.get(item.id.as_str()) else { continue };
        if word == item.stands {
            continue;
        }
        let label = |kind| ReviewLabel { title: item.title, stream: item.stream.unwrap_or_default(), kind, label: word.to_string() };
        match item.kind {
            ItemKind::Audio => {
                let order = item.order_if_kept.ok_or_else(|| format!("item {} has no place among the kept tracks", item.id))?;
                let action = match word {
                    "default" => AudioOverride::Keep { order, default: true },
                    "keep" => AudioOverride::Keep { order, default: false },
                    "commentary" => {
                        labels.push(label(LabelKind::Audio));
                        AudioOverride::Commentary
                    }
                    _ => AudioOverride::Drop,
                };
                overrides.audio.push(AudioTrackOverride { title: item.title, stream: stream_of(item)?, action });
            }
            ItemKind::Subtitle => {
                let action = match SubtitleRole::ALL.into_iter().find(|role| role.as_str() == word) {
                    Some(role) => {
                        labels.push(label(LabelKind::Subtitle));
                        SubtitleOverride::Keep { role }
                    }
                    None if word == "commentary" => {
                        labels.push(label(LabelKind::Subtitle));
                        SubtitleOverride::Drop
                    }
                    None => SubtitleOverride::Drop,
                };
                overrides.subtitles.push(SubtitleTrackOverride { title: item.title, stream: stream_of(item)?, action });
            }
            ItemKind::Collision => {
                let action = word.parse::<Collision>()?;
                let path = item.path.clone().ok_or_else(|| format!("item {} names no path", item.id))?;
                overrides.collision.push(CollisionAnswer { path, action });
            }
        }
    }
    overrides.audio.sort_by_key(|entry| (entry.title, entry.stream));
    overrides.subtitles.sort_by_key(|entry| (entry.title, entry.stream));
    overrides.collision.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(Answers { overrides, labels })
}

/// The id a track override names: its title's key when the sheet shows the title, else the scan number.
fn track_id(doc: &ReviewDoc, title: u32, word: &str, stream: u32) -> String {
    match doc.titles.iter().find(|shown| shown.scan_title == title) {
        Some(shown) => format!("{} {word} {stream}", shown.key),
        None => format!("title {title} {word} {stream}"),
    }
}

/// Ok when every entry of `overrides` names an item of `doc` and an answer that item offers (phase 3 design, part 6.4):
/// each audio and subtitle entry a track item and its word (`keep` or `default` for a kept audio action, `commentary`
/// for an audio commentary, a role's word for a subtitle keep, `drop` for a drop), each collision a collision item whose
/// path is the entry's (`settings::same_path`) and `keep_both` or `replace`. An identity, season or titles answer is
/// refused: this sheet does not answer them yet (ruling T32-R1). Two entries for one item (the same track, or two paths
/// of the same collision) are refused with `the answers name <item id> twice`, since the first would otherwise win
/// silently (P3-T8). Def: P3-T4. Use: P3-T5, P3-T8, P3-T9, P3-T14.
pub fn check(doc: &ReviewDoc, overrides: &Overrides) -> Result<(), String> {
    if overrides.identity.is_some() || overrides.season.is_some() || !overrides.titles.is_empty() {
        return Err("which film this is and which titles to file cannot be answered on this sheet yet".to_string());
    }
    let mut named: BTreeSet<String> = BTreeSet::new();
    let mut once = |id: &str| if named.insert(id.to_string()) { Ok(()) } else { Err(format!("the answers name {id} twice")) };
    for entry in &overrides.audio {
        let word = match entry.action {
            AudioOverride::Keep { default: true, .. } => "default",
            AudioOverride::Keep { default: false, .. } => "keep",
            AudioOverride::Commentary => "commentary",
            AudioOverride::Drop => "drop",
        };
        let id = track_id(doc, entry.title, "audio", entry.stream);
        offered(doc, &id, word)?;
        once(&id)?;
    }
    for entry in &overrides.subtitles {
        let word = match entry.action {
            SubtitleOverride::Keep { role } => role.as_str(),
            SubtitleOverride::Drop => "drop",
        };
        let id = track_id(doc, entry.title, "sub", entry.stream);
        offered(doc, &id, word)?;
        once(&id)?;
    }
    for entry in &overrides.collision {
        let item = items_of(doc)
            .find(|item| item.kind == ItemKind::Collision && item.path.as_deref().is_some_and(|path| same_path(path, &entry.path)))
            .ok_or_else(|| format!("no collision with \"{}\" on this sheet", entry.path))?;
        offered(doc, &item.id, entry.action.as_str())?;
        once(&item.id)?;
    }
    Ok(())
}

/// The three moments of a title, as fractions of its length, at which a subtitle track's lines are taken (ruling
/// T32-R5; phase 3 design, part 6.3). Def: P3-T5. Use: P3-T9, P3-T13, P3-T14.
pub const LINE_MOMENTS: [f64; 3] = [0.25, 0.50, 0.75];

/// The most characters of a subtitle line the sheet shows (ruling T32-R5; phase 3 design, part 6.3). Def: P3-T5. Use:
/// P3-T9, P3-T13, P3-T14.
pub const LINE_WIDTH: usize = 60;

/// One event's text for the sheet: its lines joined by a space, control characters removed, cut to `LINE_WIDTH`
/// characters; None when nothing but blanks is left.
fn clean_line(parts: &[String]) -> Option<String> {
    let joined = parts.join(" ");
    let kept: String = joined.chars().filter(|c| !c.is_control()).collect();
    let cut: String = kept.trim().chars().take(LINE_WIDTH).collect();
    let cut = cut.trim_end();
    (!cut.is_empty()).then(|| cut.to_string())
}

/// The lines shown for a subtitle track (phase 3 design, part 6.3; rulings T32-R5, T32-E7): first, at each of
/// `LINE_MOMENTS` of `seconds` in order, the first event with text at or after that moment that an earlier moment has
/// not used; then, while fewer than three events are chosen and unused events with text remain, the earliest unused
/// event with text (T32-E7, so a track whose few events all fall early still shows them). Each is one line, cut to
/// `LINE_WIDTH` characters with control characters removed. `events` are (start in seconds, the event's text lines) in
/// time order. Zero to three lines, in the time order of their events. Def: P3-T5, P3-T9 (T32-E7). Use: P3-T9,
/// P3-T13, P3-T14.
pub fn sample_lines(events: &[(f64, Vec<String>)], seconds: f64) -> Vec<String> {
    let texts: Vec<Option<String>> = events.iter().map(|(_, text)| clean_line(text)).collect();
    let mut chosen: Vec<usize> = Vec::new();
    for moment in LINE_MOMENTS {
        let at = seconds * moment;
        let found = (0..events.len()).find(|&index| !chosen.contains(&index) && events[index].0 >= at && texts[index].is_some());
        if let Some(index) = found {
            chosen.push(index);
        }
    }
    // Ruling T32-E7: fill from the earliest unused events with text.
    for index in 0..events.len() {
        if chosen.len() >= LINE_MOMENTS.len() {
            break;
        }
        if !chosen.contains(&index) && texts[index].is_some() {
            chosen.push(index);
        }
    }
    // In the time order of their events; equal starts keep their order in `events`.
    chosen.sort_by(|&a, &b| events[a].0.total_cmp(&events[b].0).then(a.cmp(&b)));
    chosen.into_iter().filter_map(|index| texts[index].clone()).collect()
}

/// One answer an item offers: its letter, its word and how a question line names it.
type Choice = (&'static str, &'static str, &'static str);

const AUDIO_CHOICES: [Choice; 4] =
    [("k", "keep", "keep"), ("p", "default", "keep, play by default"), ("c", "commentary", "commentary (dropped)"), ("d", "drop", "drop")];
const SUBTITLE_CHOICES: [Choice; 5] = [
    ("r", "regular", "regular"),
    ("s", "sdh", "SDH"),
    ("f", "forced", "forced"),
    ("c", "commentary", "commentary (dropped)"),
    ("d", "drop", "drop"),
];
const COLLISION_CHOICES: [Choice; 2] = [("k", "keep_both", "keep both"), ("r", "replace", "replace the existing file")];

/// The choices an item of this kind can offer, in the order of ruling T32-R3.
fn choices(kind: ItemKind) -> &'static [Choice] {
    match kind {
        ItemKind::Audio => &AUDIO_CHOICES,
        ItemKind::Subtitle => &SUBTITLE_CHOICES,
        ItemKind::Collision => &COLLISION_CHOICES,
    }
}

/// The answer word that `letter` stands for on `item`'s question line, or None when the item does not offer it (phase
/// 3 design, part 6.3; ruling T32-R3). Def: P3-T5. Use: P3-T9, P3-T13, P3-T14.
pub fn letter_word(item: &Item, letter: &str) -> Option<&'static str> {
    choices(item.kind)
        .iter()
        .find(|(each, word, _)| *each == letter && item.answers.iter().any(|answer| answer == word))
        .map(|(_, word, _)| *word)
}

/// One item's question line (phase 3 design, part 6.3): what it is, the letters and words it offers, and the letter
/// that Enter stands for; `leave unanswered` for an item with no standing answer. Def: P3-T5. Use: P3-T9, P3-T13,
/// P3-T14.
pub fn question(item: &Item) -> String {
    let subject = match (item.kind, item.stream) {
        (ItemKind::Collision, _) | (_, None) => format!("the file at {}:", item.path.as_deref().unwrap_or(&item.describe)),
        (_, Some(stream)) => format!("track {stream} is:"),
    };
    let offered: Vec<&Choice> =
        choices(item.kind).iter().filter(|(_, word, _)| item.answers.iter().any(|answer| answer == word)).collect();
    let options: Vec<String> = offered.iter().map(|(letter, _, label)| format!("{letter} {label}")).collect();
    let standing = offered.iter().find(|(_, word, _)| *word == item.stands).map(|(letter, _, _)| *letter);
    let enter = standing.unwrap_or("leave unanswered");
    format!("{subject}  {}     [Enter = {enter}]", options.join("   "))
}

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// `23 Nov`, in UTC.
fn day_month(unix_secs: u64) -> String {
    let (_, month, day) = crate::lookup::stale::civil_date(unix_secs);
    format!("{day} {}", MONTHS[(month.clamp(1, 12) - 1) as usize])
}

/// The identity step in words, for "Identified by ...".
fn step_words(step: &str) -> String {
    match step {
        "reviewed" => "your own answer".to_string(),
        "remembered" => "an earlier answer for this disc".to_string(),
        "discdb_both" | "discdb_one" => "TheDiscDB".to_string(),
        "tmdb_one" | "tmdb_several" => "TMDb".to_string(),
        "nothing" => "no database".to_string(),
        other => other.replace('_', " "),
    }
}

/// The head line's last sentence: how many open points this sheet can answer and how many it cannot yet.
fn open_words(answerable: usize, unanswerable: usize) -> String {
    let count = |n: usize| if n == 1 { "One".to_string() } else { n.to_string() };
    match (answerable, unanswerable) {
        (0, 0) => "Nothing is open.".to_string(),
        (0, 1) => "One point is open and cannot be answered yet.".to_string(),
        (0, b) => format!("{b} points are open and cannot be answered yet."),
        (a, 0) if a == 1 => "One point needs your answer.".to_string(),
        (a, 0) => format!("{a} points need your answer."),
        (a, b) => format!("{} {} more cannot be answered yet.", open_words(a, 0), count(b)),
    }
}

/// The text shown on a track's row after its description: its lines in double quotes joined by ` / `, or why it has
/// none.
fn evidence_text(item: &Item) -> Option<String> {
    let lines: Vec<String> =
        item.evidence.iter().filter(|each| each.kind == "lines").map(|each| format!("\"{}\"", each.text)).collect();
    if !lines.is_empty() {
        return Some(lines.join(" / "));
    }
    if let Some(unread) = item.evidence.iter().find(|each| each.kind == "unread") {
        return Some(format!("(not read: {})", unread.text));
    }
    item.evidence.iter().find(|each| each.kind == "file").map(|each| each.text.clone())
}

/// What an item stands as, for the point's `As it stands:` line.
fn standing(item: &Item) -> String {
    match (item.kind, item.stream) {
        (ItemKind::Collision, _) | (_, None) => format!("{}: not answered", item.path.as_deref().unwrap_or(&item.describe)),
        (_, Some(stream)) => format!("track {stream} {}", item.stands),
    }
}

/// The sheet of a review document (phase 3 design, part 6.3), without the questions: the head line, each point with
/// its sentence, its tracks and what stands, and the path each title files as. Every point is shown in the document's
/// order, numbered from 1. Def: P3-T5. Use: P3-T9, P3-T13, P3-T14.
pub fn render(doc: &ReviewDoc) -> String {
    let when = day_month(doc.since_unix);
    let since = if doc.held == HeldAs::Filed { format!("Filed on {when}.") } else { format!("Held since {when}.") };
    // Identity and titles points wait for a later version; every other open point is settled on this sheet.
    let open = |later: bool| {
        let waits = |point: &Point| matches!(point.kind, PointKind::Identity | PointKind::Titles);
        doc.points.iter().filter(|point| !point.reasons.is_empty() && waits(point) == later).count()
    };
    let mut out = format!(
        "{since} Identified by {} ({:.2}). {}\n",
        step_words(&doc.identity.step),
        doc.identity.confidence,
        open_words(open(false), open(true))
    );
    for (number, point) in doc.points.iter().enumerate() {
        out.push('\n');
        let label = format!("{}. ", number + 1);
        let indent = " ".repeat(label.len());
        // The closing line below says what NOT_ON_THIS_SHEET says, so it is not shown twice.
        let body = point.sentence.strip_suffix(NOT_ON_THIS_SHEET).unwrap_or(&point.sentence);
        // One sentence per line; `split_inclusive` keeps each sentence's own full stop. The nothing-at-the-path sentence
        // holds a path, which may hold ". " ("Dr. X"), so its path part is kept whole and only its fixed part is split off.
        let (whole, rest) = match body.strip_suffix(NOTHING_AT_PATH) {
            Some(path_part) if path_part.starts_with("Nothing is at ") => (path_part.trim_end(), NOTHING_AT_PATH),
            _ => ("", body),
        };
        let sentences = Some(whole).filter(|text| !text.is_empty()).into_iter().chain(rest.split_inclusive(". "));
        for (at, text) in sentences.enumerate() {
            out.push_str(if at == 0 { &label } else { &indent });
            out.push_str(text.trim_end());
            out.push('\n');
        }
        let items: Vec<&Item> = point.items.iter().filter_map(|id| items_of(doc).find(|item| &item.id == id)).collect();
        if !items.is_empty() {
            out.push('\n');
            let names: Vec<String> = items
                .iter()
                .map(|item| match (item.kind, item.stream) {
                    (ItemKind::Collision, _) | (_, None) => format!("the file at {}", item.path.as_deref().unwrap_or(&item.describe)),
                    (_, Some(stream)) => format!("track {stream}"),
                })
                .collect();
            let width = names.iter().map(|name| name.chars().count()).max().unwrap_or(0);
            for (item, name) in items.iter().zip(&names) {
                let describe = if item.kind == ItemKind::Collision { String::new() } else { item.describe.clone() };
                let mut row = format!("{indent}{name:<width$}  {describe}", width = width);
                if let Some(text) = evidence_text(item) {
                    row.push_str(if describe.is_empty() { " " } else { "   " });
                    row.push_str(&text);
                }
                out.push_str(row.trim_end());
                out.push('\n');
            }
            let staged = point
                .title_key
                .as_ref()
                .and_then(|key| doc.titles.iter().find(|title| &title.key == key))
                .and_then(|title| title.staged_path.as_ref());
            if let (Some(path), PointKind::Audio | PointKind::Subtitles) = (staged, point.kind) {
                out.push_str(&format!("{indent}To watch first: {path}\n"));
            }
            out.push('\n');
            let stands: Vec<String> = items.iter().map(|item| standing(item)).collect();
            out.push_str(&format!("{indent}As it stands: {}.\n", stands.join(", ")));
        }
        if matches!(point.kind, PointKind::Identity | PointKind::Titles) {
            out.push('\n');
            out.push_str(&format!("{indent}This can be answered from a later version of lazymkv.\n"));
        }
    }
    let finals: Vec<&String> = doc.titles.iter().filter_map(|title| title.final_path.as_ref()).collect();
    if !finals.is_empty() {
        out.push('\n');
        for path in finals {
            out.push_str(&format!("Files as: {path}\n"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::BTreeMap;

    use crate::decision::{Confidences, Disposition, FileDecision};
    use crate::disc::{disc, Disc};
    use crate::job::{Held, State};
    use crate::lang::wanted_languages;
    use crate::lookup::tmdb::{TmdbIdentity, TmdbKind};
    use crate::lookup::{Identity, IdentityStep, Kind, RuntimeFacts};
    use crate::naming::NamedPath;
    use crate::plan::movie::TitleRule;
    use crate::plan::{KindSource, Mode, Plan, PlanEvidence, PlannedRole, PlannedTitle};
    use crate::record::{JobRecord, TitleRecord, JOB_FORMAT};
    use crate::remembered::{
        AudioOverride, AudioTrackOverride, Collision, CollisionAnswer, Overrides, SubtitleOverride, SubtitleTrackOverride,
    };
    use crate::review::{Note, Noted, ReviewReason};
    use crate::robot::parse;
    use crate::testsupport::scan_text;
    use crate::tracks::audio::{AudioDecision, AudioTrack, LangGroup};
    use crate::tracks::layout::layout;
    use crate::tracks::subtitle::{SubtitleCase, SubtitleDecision, SubtitleDrop, SubtitleTrack};
    use crate::tracks::SubtitleRole;

    const KEY: &str = "t00-00801";
    /// The scan title the fixture's planned title is (not 0, so a test cannot mistake the plan position for it).
    const SCAN_TITLE: u32 = 2;
    const FINAL: &str = "M:/Movies/F (2000)/F (2000) - 2160p.mkv";

    const UHD_VIDEO: &[(u32, &str)] =
        &[(1, "Video"), (5, "V_MPEGH/ISO/HEVC"), (6, "MpegH"), (7, "MpegH HEVC Main10@L5.1"), (19, "3840x2160")];
    const ENG_TRUEHD_ATMOS: &[(u32, &str)] =
        &[(1, "Audio"), (3, "eng"), (5, "A_TRUEHD"), (6, "TrueHD"), (7, "TrueHD Atmos"), (14, "8")];
    const ENG_AC3_2CH: &[(u32, &str)] =
        &[(1, "Audio"), (3, "eng"), (5, "A_AC3"), (6, "DD"), (7, "Dolby Digital"), (14, "2")];
    const ENG_PGS: &[(u32, &str)] = &[(1, "Subtitles"), (3, "eng"), (5, "S_HDMV/PGS"), (6, "PGS"), (7, "HDMV PGS Subtitles")];
    const FRA_PGS: &[(u32, &str)] = &[(1, "Subtitles"), (3, "fra"), (5, "S_HDMV/PGS"), (6, "PGS"), (7, "HDMV PGS Subtitles")];

    /// A synthetic UHD Blu-ray whose scan title 2 (playlist 00801) has video 0, English TrueHD Atmos 7.1 at 1, English
    /// stereo Dolby Digital at 8, English PGS at 9, 10 and 33, and French PGS at 11.
    fn film_disc() -> Disc {
        let streams: &[(u32, &[(u32, &str)])] = &[
            (0, UHD_VIDEO),
            (1, ENG_TRUEHD_ATMOS),
            (8, ENG_AC3_2CH),
            (9, ENG_PGS),
            (10, ENG_PGS),
            (11, FRA_PGS),
            (33, ENG_PGS),
        ];
        let attrs: &[(u32, &str)] = &[(8, "24"), (9, "1:58:42"), (11, "40000000000"), (16, "00801.mpls"), (26, "1,2,3")];
        disc(&parse(&scan_text(&[(1, "Blu-ray disc")], &[(SCAN_TITLE, attrs, streams)]))).unwrap()
    }

    /// The kept audio track of scan stream `stream` as 8.6 leaves it.
    fn kept_audio(disc: &Disc, stream: u32, name: &str, default: bool, first: bool) -> AudioTrack {
        let scanned = disc.title(SCAN_TITLE).unwrap().streams.iter().find(|s| s.number == stream).unwrap();
        AudioTrack {
            stream,
            language: scanned.language.clone(),
            group: LangGroup::English,
            codec: scanned.codec.clone(),
            file_codec: scanned.file_codec.clone(),
            channels: scanned.channels.unwrap_or(0),
            tier: None,
            object: scanned.codec.object,
            sample_size: None,
            sample_rate: None,
            bitrate_kbps: None,
            first_of_language: first,
            name: name.to_string(),
            default,
            original: true,
        }
    }

    fn kept_subtitle(stream: u32, events: usize) -> SubtitleTrack {
        SubtitleTrack {
            stream,
            role: SubtitleRole::Regular,
            name: "English".to_string(),
            forced: false,
            hearing_impaired: false,
            default: false,
            events: Some(events),
            closed_captions: false,
            parent: None,
            decided_by: "alignment".to_string(),
        }
    }

    /// The title's decision: audio 1 (default) and 8 kept, 8 noted possible_commentary; subtitles 9 and 33 kept as
    /// regular with subtitle_ambiguous; 10 dropped as an empty derived forced-only stream, 11 for its language.
    fn decision(disc: &Disc) -> FileDecision {
        let commentary =
            Noted { note: Note::PossibleCommentary, stream: Some(8), detail: "track 8 may be a commentary".to_string() };
        let audio = AudioDecision {
            kept: vec![
                kept_audio(disc, 1, "TrueHD Atmos 7.1", true, true),
                kept_audio(disc, 8, "Dolby Digital 2.0", false, false),
            ],
            dropped: vec![],
            reference: Some(1),
            verdict_reference: None,
            notes: vec![commentary.clone()],
            reviews: vec![],
            confidence: 0.95,
            description_alignment: None,
            menu: Vec::new(),
        };
        let subtitles = SubtitleDecision {
            kept: vec![kept_subtitle(9, 1224), kept_subtitle(33, 1410)],
            dropped: vec![(10, SubtitleDrop::DerivedForcedOnlyEmpty), (11, SubtitleDrop::OtherLanguage)],
            set_aside: vec![],
            full: vec![9, 33],
            groups: vec![vec![9], vec![33]],
            dialogue_group: Some(0),
            dialogue_pick: None,
            pairs: vec![],
            case: SubtitleCase::Ambiguous,
            confidence: 0.60,
            notes: vec![],
            reviews: vec![ReviewReason::SubtitleAmbiguous],
        };
        let layout = layout(disc.title(SCAN_TITLE).unwrap(), &audio, &subtitles);
        FileDecision {
            title_key: KEY.to_string(),
            audio,
            subtitles,
            layout,
            resolution: Some("2160p".to_string()),
            edition: None,
            path: Some(NamedPath {
                root: "M:/Movies".to_string(),
                components: vec!["F (2000)".to_string(), "F (2000) - 2160p.mkv".to_string()],
                path: FINAL.to_string(),
            }),
            confidences: Confidences { identity: 0.90, title: 0.98, track: 0.60 },
            reviews: vec![ReviewReason::SubtitleAmbiguous],
            notes: vec![commentary],
            read_errors: vec![],
            disposition: Disposition::NeedsReview,
        }
    }

    fn plan(disc: &Disc) -> Plan {
        let title = disc.title(SCAN_TITLE).unwrap();
        Plan {
            identity: Identity {
                step: IdentityStep::TmdbOne,
                confidence: 0.90,
                tmdb: Some(TmdbIdentity { kind: TmdbKind::Movie, id: 1 }),
                title: Some("F".to_string()),
                year: Some(2000),
                original_language: Some("en".to_string()),
                review: None,
                plausible: vec![],
                runtime_check: Some(true),
                lookup_pending: false,
                evidence: vec![],
            },
            kind: Kind::Movie,
            kind_source: KindSource::Discdb,
            titles: vec![PlannedTitle {
                key: KEY.to_string(),
                title: SCAN_TITLE,
                role: PlannedRole::Feature { edition: None, theatrical: true },
                seconds: title.seconds,
                size: title.size,
                source_file: title.source_file.clone(),
                suggested: false,
                remembered: false,
            }],
            mode: Mode::Direct,
            identity_confidence: 0.90,
            title_confidence: 0.98,
            reviews: vec![],
            notes: vec![],
            wanted: wanted_languages(Some("en")),
            session: None,
            evidence: PlanEvidence {
                name: None,
                candidates: vec![SCAN_TITLE],
                groups: vec![],
                rivals: vec![],
                title_rule: Some(TitleRule::Database),
                labels: vec![],
                discdb_matches: vec![],
                runtime: RuntimeFacts::default(),
                sentences: vec![],
            },
        }
    }

    /// A job held at the disposition gate with the fixture's one title decided.
    fn record(disc: &Disc) -> JobRecord {
        JobRecord {
            format: JOB_FORMAT.to_string(),
            id: "job-1".to_string(),
            fingerprint: "ABC".to_string(),
            disc_id: None,
            drive: "drive-0".to_string(),
            created_unix: 1_800_000_000,
            updated_unix: 1_800_000_900,
            state: State::NeedsReview { held: Held::DispositionGate },
            message: None,
            rip_again: false,
            profile: "job.mmcp.xml".to_string(),
            plan: Some(plan(disc)),
            titles: vec![TitleRecord {
                key: KEY.to_string(),
                scan_title: SCAN_TITLE,
                rip_index: Some(0),
                attempts: 1,
                verified: true,
                verify_failures: 0,
                read_errors: vec![],
                rip_reviews: vec![],
                decision: Some(decision(disc)),
                filed: None,
                rip_started_ms: None,
            }],
            filed_unix: None,
            collisions: vec![],
            notes: vec![],
            tuning_copy: None,
        }
    }

    /// The fixture's title decision, to change a test's reasons.
    fn decided(record: &mut JobRecord) -> &mut FileDecision {
        record.titles[0].decision.as_mut().unwrap()
    }

    /// The document of `record` with no staged file, no subtitle lines and the given collisions.
    fn doc_with(record: &JobRecord, disc: &Disc, all: bool, collisions: &BTreeMap<String, String>) -> Result<ReviewDoc, String> {
        let staged = BTreeMap::from([(KEY.to_string(), "S:/staging/job-1/title_t00.mkv".to_string())]);
        let lines = BTreeMap::from([((KEY.to_string(), 9), vec!["Where are you going?".to_string(), "Wait.".to_string()])]);
        let unread = BTreeMap::from([((KEY.to_string(), 33), "the time cap was reached".to_string())]);
        build(&DocInputs { record, disc, staged: &staged, lines: &lines, unread: &unread, collisions, nothing_at: &BTreeSet::new(), all })
    }

    fn sheet(record: &JobRecord, disc: &Disc, all: bool) -> ReviewDoc {
        doc_with(record, disc, all, &BTreeMap::new()).unwrap()
    }

    fn item<'a>(doc: &'a ReviewDoc, id: &str) -> &'a Item {
        doc.titles.iter().flat_map(|title| &title.items).find(|item| item.id == id).unwrap()
    }

    fn words(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(id, word)| (id.to_string(), word.to_string())).collect()
    }

    #[test]
    fn a_held_job_has_one_point_per_reason_in_the_table_order() {
        let disc = film_disc();
        let doc = sheet(&record(&disc), &disc, false);
        assert_eq!(doc.format, DOC_FORMAT);
        assert_eq!((doc.job.as_str(), doc.held, doc.since_unix), ("job-1", HeldAs::Gate, 1_800_000_900));
        assert_eq!(doc.reasons, vec![ReviewReason::SubtitleAmbiguous]);
        assert_eq!(
            doc.identity,
            DocIdentity { step: "tmdb_one".to_string(), confidence: 0.90, title: Some("F".to_string()), year: Some(2000) }
        );
        assert_eq!(doc.points.len(), 1, "{:?}", doc.points);
        let point = &doc.points[0];
        assert_eq!(point.kind, PointKind::Subtitles);
        assert_eq!(point.reasons, vec![ReviewReason::SubtitleAmbiguous]);
        assert_eq!(point.title_key.as_deref(), Some(KEY));
        assert_eq!(point.items, vec!["t00-00801 sub 9", "t00-00801 sub 33"]);
        assert!(point.answerable);
        assert_eq!(
            point.sentence,
            "Subtitles: two full English tracks that do not match each other. lazymkv cannot tell which one carries the \
             film's dialogue."
        );
        let title = &doc.titles[0];
        assert_eq!((title.key.as_str(), title.scan_title), (KEY, SCAN_TITLE));
        assert_eq!((title.seconds, title.chapters, title.bytes), (Some(7122), Some(24), Some(40_000_000_000)));
        assert_eq!(title.staged_path.as_deref(), Some("S:/staging/job-1/title_t00.mkv"));
        assert_eq!(title.final_path.as_deref(), Some(FINAL));
        let nine = item(&doc, "t00-00801 sub 9");
        assert_eq!((nine.kind, nine.title, nine.stream), (ItemKind::Subtitle, SCAN_TITLE, Some(9)));
        assert_eq!(nine.describe, "English  1,224 lines");
        assert_eq!(nine.stands, "regular");
        assert_eq!(nine.answers, vec!["regular", "sdh", "forced", "commentary", "drop"]);
        let kinds = |item: &Item| item.evidence.iter().map(|e| e.kind.clone()).collect::<Vec<String>>();
        assert_eq!(kinds(nine), vec!["verdict", "events", "lines", "lines"]);
        assert_eq!(nine.evidence[2].text, "Where are you going?");
        assert_eq!(kinds(item(&doc, "t00-00801 sub 33")), vec!["verdict", "events", "unread"]);
    }

    #[test]
    fn tracks_dropped_for_their_language_or_as_derived_are_never_items() {
        let disc = film_disc();
        let doc = sheet(&record(&disc), &disc, true);
        let ids: Vec<&str> = doc.titles.iter().flat_map(|title| &title.items).map(|item| item.id.as_str()).collect();
        assert!(!ids.iter().any(|id| id.ends_with("sub 10") || id.ends_with("sub 11")), "{ids:?}");
        assert!(doc.points.iter().flat_map(|point| &point.items).all(|id| !id.ends_with("sub 10") && !id.ends_with("sub 11")));
        // A track dropped for a reason the owner can overrule is an item, standing as dropped.
        let mut overrulable = record(&disc);
        decided(&mut overrulable).subtitles.dropped[0].1 = SubtitleDrop::OutsideDialogueGroup;
        let doc = sheet(&overrulable, &disc, false);
        let ten = item(&doc, "t00-00801 sub 10");
        assert_eq!((ten.stands.as_str(), ten.describe.as_str()), ("drop", "English"));
        assert_eq!(doc.points[0].items, vec!["t00-00801 sub 9", "t00-00801 sub 10", "t00-00801 sub 33"]);
    }

    #[test]
    fn all_adds_an_audio_point_with_every_item() {
        let disc = film_disc();
        let doc = sheet(&record(&disc), &disc, true);
        let kinds: Vec<PointKind> = doc.points.iter().map(|point| point.kind).collect();
        assert_eq!(kinds, vec![PointKind::Audio, PointKind::Subtitles]);
        let audio = &doc.points[0];
        assert_eq!(audio.items, vec!["t00-00801 audio 1", "t00-00801 audio 8"]);
        assert!(audio.reasons.is_empty() && audio.answerable);
        let one = item(&doc, "t00-00801 audio 1");
        assert_eq!(one.stands, "default");
        assert_eq!(one.describe, "English  TrueHD Atmos 7.1");
        assert_eq!(one.order_if_kept, Some(0));
        assert_eq!(one.answers, vec!["keep", "default", "commentary", "drop"]);
        let eight = item(&doc, "t00-00801 audio 8");
        assert_eq!(eight.stands, "keep");
        assert_eq!(eight.order_if_kept, Some(1));
        assert!(eight.evidence.iter().any(|e| e.kind == "note" && e.text == "track 8 may be a commentary"), "{:?}", eight.evidence);
        assert!(!one.evidence.iter().any(|e| e.kind == "note"));
    }

    #[test]
    fn a_filed_job_shows_every_title_with_both_points() {
        let disc = film_disc();
        let mut filed = record(&disc);
        filed.state = State::Filed;
        filed.filed_unix = Some(1_800_001_000);
        let doc = sheet(&filed, &disc, false);
        assert_eq!((doc.held, doc.since_unix), (HeldAs::Filed, 1_800_001_000));
        assert_eq!(doc.points.iter().map(|point| point.kind).collect::<Vec<_>>(), vec![PointKind::Audio, PointKind::Subtitles]);
    }

    #[test]
    fn a_dropped_audio_track_is_described_from_the_scan_and_ordered_after_the_kept_ones() {
        let disc = film_disc();
        let mut held = record(&disc);
        let audio = &mut decided(&mut held).audio;
        audio.kept.truncate(1);
        audio.dropped = vec![(8, crate::tracks::audio::AudioDrop::Secondary)];
        let doc = sheet(&held, &disc, true);
        let eight = item(&doc, "t00-00801 audio 8");
        assert_eq!((eight.stands.as_str(), eight.order_if_kept), ("drop", Some(1)));
        assert_eq!(eight.describe, "English  Dolby Digital, 2 channels");
        // Derived and unwanted-language audio is never an item.
        let mut derived = held.clone();
        decided(&mut derived).audio.dropped = vec![(8, crate::tracks::audio::AudioDrop::Derived)];
        assert!(doc_with(&derived, &disc, true, &BTreeMap::new()).unwrap().points[0].items == vec!["t00-00801 audio 1"]);
    }

    #[test]
    fn a_track_the_menu_dropped_is_an_item() {
        // Ruling T32-R2: a track dropped on the menu's word is one the owner can overrule, so it is an item.
        let disc = film_disc();
        let mut held = record(&disc);
        let audio = &mut decided(&mut held).audio;
        audio.kept.truncate(1);
        audio.dropped = vec![(8, crate::tracks::audio::AudioDrop::Menu { role: crate::menu::AudioRole::Commentary })];
        let doc = sheet(&held, &disc, true);
        assert_eq!(doc.points[0].items, vec!["t00-00801 audio 1", "t00-00801 audio 8"]);
        let eight = item(&doc, "t00-00801 audio 8");
        assert_eq!(eight.kind, ItemKind::Audio);
        assert_eq!(eight.answers, AUDIO_ANSWERS.iter().map(|answer| answer.to_string()).collect::<Vec<String>>());
        assert_eq!((eight.stands.as_str(), eight.order_if_kept), ("drop", Some(1)));
        assert_eq!(eight.describe, "English  Dolby Digital, 2 channels");
        assert!(
            eight.evidence.iter().any(|e| e.kind == "verdict" && e.text == "dropped: the disc's menu calls it a commentary"),
            "{:?}",
            eight.evidence
        );
    }

    #[test]
    fn a_collision_is_one_item_with_no_standing_answer() {
        let disc = film_disc();
        let mut held = record(&disc);
        decided(&mut held).reviews = vec![ReviewReason::PathCollision];
        let collisions = BTreeMap::from([(FINAL.to_string(), "40000000000 bytes, 2026-11-01".to_string())]);
        let doc = doc_with(&held, &disc, false, &collisions).unwrap();
        assert_eq!(doc.points.len(), 1, "{:?}", doc.points);
        let point = &doc.points[0];
        assert_eq!((point.kind, point.answerable), (PointKind::Collision, true));
        assert_eq!(point.items, vec!["t00-00801 collision"]);
        let collision = item(&doc, "t00-00801 collision");
        assert_eq!(collision.kind, ItemKind::Collision);
        assert_eq!(collision.answers, vec!["keep_both", "replace"]);
        assert_eq!(collision.stands, "");
        assert_eq!((collision.path.as_deref(), collision.describe.as_str(), collision.stream), (Some(FINAL), FINAL, None));
        assert_eq!(collision.evidence, vec![Evidence { kind: "file".to_string(), text: "40000000000 bytes, 2026-11-01".to_string() }]);
    }

    /// The fixture's held job with a path collision, and the document of it when the engine looked at its final path
    /// and found nothing there (or, with `collisions`, a file).
    fn nothing_at_the_path(disc: &Disc, collisions: &BTreeMap<String, String>, nothing_at: &BTreeSet<String>) -> ReviewDoc {
        let mut held = record(disc);
        decided(&mut held).reviews = vec![ReviewReason::PathCollision];
        let staged = BTreeMap::new();
        let (lines, unread) = (BTreeMap::new(), BTreeMap::new());
        build(&DocInputs { record: &held, disc, staged: &staged, lines: &lines, unread: &unread, collisions, nothing_at, all: false })
            .unwrap()
    }

    fn only_the_final_path() -> BTreeSet<String> {
        BTreeSet::from([FINAL.to_string()])
    }

    #[test]
    fn a_collision_with_nothing_at_the_path_is_a_point_without_an_item() {
        let disc = film_disc();
        let doc = nothing_at_the_path(&disc, &BTreeMap::new(), &only_the_final_path());
        assert_eq!(doc.points.len(), 1, "{:?}", doc.points);
        let point = &doc.points[0];
        assert_eq!((point.kind, point.title_key.as_deref()), (PointKind::Collision, Some(KEY)));
        assert_eq!(point.reasons, vec![ReviewReason::PathCollision]);
        assert!(!point.answerable && point.items.is_empty());
        assert_eq!(
            point.sentence,
            format!(
                "Nothing is at {FINAL} now. A folder the share could not open clears by itself within minutes, so \
                 approving tries to file again."
            )
        );
        assert_eq!(point.sentence, format!("Nothing is at {FINAL} now. {NOTHING_AT_PATH}"));
        assert!(doc.titles[0].items.iter().all(|item| item.kind != ItemKind::Collision), "{:?}", doc.titles[0].items);
        // The reason still stands on the job, and the path is still the one the title files as.
        assert_eq!(doc.reasons, vec![ReviewReason::PathCollision]);
        assert_eq!(doc.titles[0].final_path.as_deref(), Some(FINAL));
    }

    #[test]
    fn the_sheet_says_nothing_is_at_the_path_and_asks_nothing() {
        let disc = film_disc();
        let text = render(&nothing_at_the_path(&disc, &BTreeMap::new(), &only_the_final_path()));
        // One sentence per line, as every point's sentences are shown.
        let sentence = format!(
            "1. Nothing is at {FINAL} now.\n   A folder the share could not open clears by itself within minutes, so approving \
             tries to file again.\n"
        );
        assert_eq!(text.matches(&sentence).count(), 1, "{text}");
        assert_eq!(text.matches("Nothing is at").count(), 1, "{text}");
        assert!(text.contains("One point needs your answer.\n"), "{text}");
        assert!(!text.contains("not answered"), "{text}");
        assert!(!text.contains("later version of lazymkv"), "{text}");
        assert!(!text.contains("As it stands"), "{text}");
    }

    /// A path with a full stop and a space in it ("Dr. Nobody") is not a sentence end: the sheet prints it whole.
    #[test]
    fn the_sheet_keeps_a_path_with_a_full_stop_in_one_piece() {
        let disc = film_disc();
        let dotted = "M:/Movies/Dr. Nobody (1964)/Dr. Nobody (1964) - 2160p.mkv";
        let mut held = record(&disc);
        decided(&mut held).reviews = vec![ReviewReason::PathCollision];
        decided(&mut held).path.as_mut().unwrap().path = dotted.to_string();
        let (staged, lines, unread) = (BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
        let nothing_at = BTreeSet::from([dotted.to_string()]);
        let doc = build(&DocInputs {
            record: &held,
            disc: &disc,
            staged: &staged,
            lines: &lines,
            unread: &unread,
            collisions: &BTreeMap::new(),
            nothing_at: &nothing_at,
            all: false,
        })
        .unwrap();
        assert_eq!(doc.points[0].sentence, format!("Nothing is at {dotted} now. {NOTHING_AT_PATH}"));
        let text = render(&doc);
        assert!(text.contains(&format!("1. Nothing is at {dotted} now.\n")), "{text}");
        assert_eq!(text.matches(NOTHING_AT_PATH).count(), 1, "{text}");
        assert!(text.contains(&format!("\n   {NOTHING_AT_PATH}\n")), "{text}");
    }

    #[test]
    fn a_collision_answer_for_a_path_with_nothing_at_it_is_refused() {
        let disc = film_disc();
        let doc = nothing_at_the_path(&disc, &BTreeMap::new(), &only_the_final_path());
        let keep_both = Overrides {
            collision: vec![CollisionAnswer { path: FINAL.to_string(), action: Collision::KeepBoth }],
            ..Overrides::default()
        };
        assert_eq!(check(&doc, &keep_both), Err(format!("no collision with \"{FINAL}\" on this sheet")));
        assert_eq!(
            answers_of(&doc, &words(&[("t00-00801 collision", "keep_both")])),
            Err("no item \"t00-00801 collision\" on this sheet".to_string())
        );
        // No answers at all is fine: approving is the answer.
        assert_eq!(check(&doc, &Overrides::default()), Ok(()));
    }

    #[test]
    fn a_file_at_the_path_still_gives_the_ordinary_item() {
        let disc = film_disc();
        let collisions = BTreeMap::from([(FINAL.to_string(), "40000000000 bytes, 2026-11-01".to_string())]);
        let doc = nothing_at_the_path(&disc, &collisions, &BTreeSet::new());
        let point = &doc.points[0];
        assert_eq!((point.kind, point.answerable), (PointKind::Collision, true));
        assert_eq!(point.items, vec!["t00-00801 collision"]);
        assert!(!point.sentence.contains("Nothing is at"), "{}", point.sentence);
        assert_eq!(item(&doc, "t00-00801 collision").answers, vec!["keep_both", "replace"]);
        // A path in both sets cannot happen; the file wins.
        let both = nothing_at_the_path(&disc, &collisions, &only_the_final_path());
        assert_eq!(both, doc);
    }

    #[test]
    fn an_identity_reason_gives_a_point_that_cannot_be_answered_yet() {
        let disc = film_disc();
        let mut held = record(&disc);
        held.plan.as_mut().unwrap().reviews = vec![ReviewReason::IdentityUnknown];
        decided(&mut held).reviews = vec![ReviewReason::IdentityUnknown, ReviewReason::SubtitleAmbiguous];
        let doc = sheet(&held, &disc, false);
        assert_eq!(doc.points.iter().map(|point| point.kind).collect::<Vec<_>>(), vec![PointKind::Identity, PointKind::Subtitles]);
        let identity = &doc.points[0];
        assert_eq!(identity.reasons, vec![ReviewReason::IdentityUnknown]);
        assert_eq!(identity.title_key, None);
        assert!(!identity.answerable && identity.items.is_empty());
        assert!(identity.sentence.contains("which film"), "{}", identity.sentence);
        assert!(!identity.sentence.contains('_'), "{}", identity.sentence);
        // A titles reason is the same: a point of the job that cannot be answered here.
        held.plan.as_mut().unwrap().reviews = vec![ReviewReason::UnnamedCut];
        decided(&mut held).reviews = vec![ReviewReason::UnnamedCut];
        let titles = &doc_with(&held, &disc, false, &BTreeMap::new()).unwrap().points[0];
        assert_eq!((titles.kind, titles.title_key.as_deref(), titles.answerable), (PointKind::Titles, None, false));
    }

    #[test]
    fn a_rip_reason_gives_a_point_with_no_item() {
        let disc = film_disc();
        let mut held = record(&disc);
        held.titles[0].rip_reviews = vec![ReviewReason::ReadErrors];
        decided(&mut held).reviews = vec![ReviewReason::ReadErrors];
        let doc = sheet(&held, &disc, false);
        assert_eq!(doc.points.len(), 1, "{:?}", doc.points);
        let rip = &doc.points[0];
        assert_eq!((rip.kind, rip.title_key.as_deref()), (PointKind::Rip, Some(KEY)));
        assert!(!rip.answerable && rip.items.is_empty());
        assert!(rip.sentence.contains("accepts the rip as it is"), "{}", rip.sentence);
        assert!(rip.sentence.contains("discard the job and rip the disc again"), "{}", rip.sentence);
    }

    #[test]
    fn a_missing_original_language_heads_the_audio_point() {
        let disc = film_disc();
        let mut held = record(&disc);
        decided(&mut held).reviews = vec![ReviewReason::OriginalLanguageMissing, ReviewReason::CommentaryDetected];
        let doc = sheet(&held, &disc, false);
        assert_eq!(doc.points.len(), 1, "{:?}", doc.points);
        let audio = &doc.points[0];
        assert_eq!(audio.kind, PointKind::Audio);
        assert_eq!(audio.reasons, vec![ReviewReason::OriginalLanguageMissing, ReviewReason::CommentaryDetected]);
        assert_eq!(audio.items, vec!["t00-00801 audio 1", "t00-00801 audio 8"]);
        assert!(audio.sentence.starts_with("Audio: the film's original language"), "{}", audio.sentence);
        assert_eq!(audio.sentence.matches("Audio:").count(), 1, "{}", audio.sentence);
    }

    #[test]
    fn a_job_that_is_ripping_has_nothing_to_review() {
        let disc = film_disc();
        let mut ripping = record(&disc);
        ripping.state = State::Ripping;
        assert_eq!(doc_with(&ripping, &disc, false, &BTreeMap::new()), Err("job job-1 is Ripping: there is nothing to review".to_string()));
    }

    #[test]
    fn answers_become_overrides_and_labels() {
        let disc = film_disc();
        let doc = sheet(&record(&disc), &disc, false);
        let answers = answers_of(&doc, &words(&[("t00-00801 sub 9", "sdh"), ("t00-00801 sub 33", "commentary")])).unwrap();
        assert_eq!(
            answers.overrides.subtitles,
            vec![
                SubtitleTrackOverride { title: SCAN_TITLE, stream: 9, action: SubtitleOverride::Keep { role: SubtitleRole::Sdh } },
                SubtitleTrackOverride { title: SCAN_TITLE, stream: 33, action: SubtitleOverride::Drop },
            ]
        );
        assert_eq!(
            answers.labels,
            vec![
                ReviewLabel { title: SCAN_TITLE, stream: 9, kind: LabelKind::Subtitle, label: "sdh".to_string() },
                ReviewLabel { title: SCAN_TITLE, stream: 33, kind: LabelKind::Subtitle, label: "commentary".to_string() },
            ]
        );
        assert_eq!(answers_of(&doc, &words(&[("t00-00801 sub 9", "regular")])).unwrap(), Answers { overrides: Overrides::default(), labels: vec![] });
        // A drop says what the owner wants, not what the track is: no label.
        let dropped = answers_of(&doc, &words(&[("t00-00801 sub 9", "drop")])).unwrap();
        assert!(dropped.labels.is_empty());
        assert_eq!(dropped.overrides.subtitles[0].action, SubtitleOverride::Drop);
        // Audio, with every item answerable.
        let all = doc_of_all(&disc);
        let audio = answers_of(&all, &words(&[("t00-00801 audio 8", "default"), ("t00-00801 audio 1", "commentary")])).unwrap();
        assert_eq!(
            audio.overrides.audio,
            vec![
                AudioTrackOverride { title: SCAN_TITLE, stream: 1, action: AudioOverride::Commentary },
                AudioTrackOverride { title: SCAN_TITLE, stream: 8, action: AudioOverride::Keep { order: 1, default: true } },
            ]
        );
        assert_eq!(audio.labels, vec![ReviewLabel { title: SCAN_TITLE, stream: 1, kind: LabelKind::Audio, label: "commentary".to_string() }]);
        let keep = answers_of(&all, &words(&[("t00-00801 audio 1", "keep")])).unwrap();
        assert_eq!(keep.overrides.audio[0].action, AudioOverride::Keep { order: 0, default: false });
        assert!(keep.labels.is_empty());
        // A collision answer names the path the sheet showed.
        let mut colliding = record(&disc);
        decided(&mut colliding).reviews = vec![ReviewReason::PathCollision];
        let collided = sheet(&colliding, &disc, false);
        let both = answers_of(&collided, &words(&[("t00-00801 collision", "keep_both")])).unwrap();
        assert_eq!(both.overrides.collision, vec![CollisionAnswer { path: FINAL.to_string(), action: Collision::KeepBoth }]);
        assert!(both.labels.is_empty());
        assert_eq!(check(&collided, &both.overrides), Ok(()));
        assert_eq!(check(&doc, &answers.overrides), Ok(()));
        assert_eq!(check(&all, &audio.overrides), Ok(()));
    }

    fn doc_of_all(disc: &Disc) -> ReviewDoc {
        sheet(&record(disc), disc, true)
    }

    #[test]
    fn an_answer_for_a_track_the_sheet_lacks_is_refused() {
        let disc = film_disc();
        let doc = sheet(&record(&disc), &disc, false);
        assert_eq!(
            answers_of(&doc, &words(&[("t00-00801 sub 11", "drop")])),
            Err("no item \"t00-00801 sub 11\" on this sheet".to_string())
        );
        assert_eq!(
            answers_of(&doc, &words(&[("t00-00801 sub 9", "louder")])),
            Err("\"louder\" is not an answer for t00-00801 sub 9 (regular, sdh, forced, commentary, drop)".to_string())
        );
        // Every track item is on the sheet's titles, whether or not a point is open for it.
        assert!(answers_of(&doc, &words(&[("t00-00801 audio 1", "keep")])).is_ok());
        let eleven = Overrides {
            subtitles: vec![SubtitleTrackOverride { title: SCAN_TITLE, stream: 11, action: SubtitleOverride::Drop }],
            ..Overrides::default()
        };
        assert_eq!(check(&doc, &eleven), Err("no item \"t00-00801 sub 11\" on this sheet".to_string()));
        let elsewhere = Overrides {
            collision: vec![CollisionAnswer { path: "M:/Movies/G (2001)/G (2001).mkv".to_string(), action: Collision::Replace }],
            ..Overrides::default()
        };
        assert!(check(&doc, &elsewhere).is_err());
        let other_title =
            Overrides { audio: vec![AudioTrackOverride { title: 7, stream: 1, action: AudioOverride::Drop }], ..Overrides::default() };
        assert_eq!(check(&doc, &other_title), Err("no item \"title 7 audio 1\" on this sheet".to_string()));
        let discard = Overrides {
            collision: vec![CollisionAnswer { path: FINAL.to_string(), action: Collision::Discard }],
            ..Overrides::default()
        };
        let mut colliding = record(&disc);
        decided(&mut colliding).reviews = vec![ReviewReason::PathCollision];
        assert_eq!(
            check(&doc_with(&colliding, &disc, false, &BTreeMap::new()).unwrap(), &discard),
            Err("\"discard\" is not an answer for t00-00801 collision (keep_both, replace)".to_string())
        );
        let identity = Overrides { identity: Some(TmdbIdentity { kind: TmdbKind::Movie, id: 2 }), ..Overrides::default() };
        assert!(check(&doc, &identity).is_err());
    }

    /// An answers object naming one track or one path twice is refused: the first entry would otherwise win silently
    /// (phase 3 design, part 6.4; carried from P3-T7's review).
    #[test]
    fn answers_naming_an_item_twice_are_refused() {
        let disc = film_disc();
        let doc = sheet(&record(&disc), &disc, false);
        let sub = |action| SubtitleTrackOverride { title: SCAN_TITLE, stream: 9, action };
        let subtitles = Overrides {
            subtitles: vec![sub(SubtitleOverride::Drop), sub(SubtitleOverride::Keep { role: SubtitleRole::Sdh })],
            ..Overrides::default()
        };
        assert_eq!(check(&doc, &subtitles), Err("the answers name t00-00801 sub 9 twice".to_string()));
        let audio = Overrides {
            audio: vec![
                AudioTrackOverride { title: SCAN_TITLE, stream: 1, action: AudioOverride::Drop },
                AudioTrackOverride { title: SCAN_TITLE, stream: 1, action: AudioOverride::Drop },
            ],
            ..Overrides::default()
        };
        assert_eq!(check(&doc, &audio), Err("the answers name t00-00801 audio 1 twice".to_string()));
        // The same stream as an audio and as a subtitle answer is two items, not one named twice.
        let both = Overrides {
            audio: vec![AudioTrackOverride { title: SCAN_TITLE, stream: 1, action: AudioOverride::Drop }],
            subtitles: vec![sub(SubtitleOverride::Drop)],
            ..Overrides::default()
        };
        assert_eq!(check(&doc, &both), Ok(()));
        // A collision path named twice, spelt the same or not, names the collision item twice.
        let mut colliding = record(&disc);
        decided(&mut colliding).reviews = vec![ReviewReason::PathCollision];
        let collided = sheet(&colliding, &disc, false);
        let twice = Overrides {
            collision: vec![
                CollisionAnswer { path: FINAL.to_string(), action: Collision::KeepBoth },
                CollisionAnswer { path: FINAL.to_string(), action: Collision::Replace },
            ],
            ..Overrides::default()
        };
        assert_eq!(check(&collided, &twice), Err("the answers name t00-00801 collision twice".to_string()));
    }

    #[test]
    fn the_document_round_trips_through_json() {
        let disc = film_disc();
        let mut held = record(&disc);
        decided(&mut held).reviews = vec![ReviewReason::SubtitleAmbiguous, ReviewReason::PathCollision];
        let collisions = BTreeMap::from([(FINAL.to_string(), "1 bytes, 2026-11-01".to_string())]);
        let doc = doc_with(&held, &disc, true, &collisions).unwrap();
        let text = serde_json::to_string(&doc).unwrap();
        assert!(text.starts_with(r#"{"format":"lazymkv review document v1","job":"job-1","held":"gate","#), "{text}");
        assert!(text.contains(r#""kind":"subtitles""#) && text.contains(r#""kind":"subtitle""#), "{text}");
        assert_eq!(serde_json::from_str::<ReviewDoc>(&text).unwrap(), doc);
    }

    #[test]
    fn every_reason_has_a_plain_sentence() {
        for reason in ReviewReason::ALL {
            let text = sentence(reason);
            assert!(!text.is_empty() && !text.contains('_'), "{reason:?}: {text}");
        }
    }

    fn event(start: f64, text: &[&str]) -> (f64, Vec<String>) {
        (start, text.iter().map(|line| line.to_string()).collect())
    }

    #[test]
    fn lines_are_taken_at_the_three_moments() {
        let events = vec![event(100.0, &["one"]), event(1500.0, &["two"]), event(3100.0, &["three"]), event(4600.0, &["four"])];
        assert_eq!(sample_lines(&events, 6000.0), vec!["two", "three", "four"]);
    }

    /// Ruling T32-E7: a track whose events all come before the first moment still shows its lines, in time order.
    #[test]
    fn events_before_the_first_moment_still_give_lines() {
        let events = vec![event(1.0, &["one"]), event(3.0, &["two"]), event(6.0, &["three"])];
        assert_eq!(sample_lines(&events, 7150.0), vec!["one", "two", "three"]);
    }

    /// Ruling T32-E7: the moments' choices are filled from the earliest unused events, and shown in time order.
    #[test]
    fn a_sparse_track_fills_from_its_earliest_events() {
        let events = vec![event(10.0, &["early"]), event(5000.0, &["late"])];
        assert_eq!(sample_lines(&events, 6000.0), vec!["early", "late"]);
    }

    #[test]
    fn a_track_with_one_event_gives_one_line() {
        assert_eq!(sample_lines(&[event(5000.0, &["only"])], 6000.0), vec!["only"]);
    }

    #[test]
    fn no_event_with_text_gives_none() {
        assert!(sample_lines(&[], 6000.0).is_empty());
        let blank = vec![event(2000.0, &[]), event(3000.0, &["  ", "\t"]), event(4000.0, &[""])];
        assert!(sample_lines(&blank, 6000.0).is_empty());
        // An event before the first moment is found by the second pass (ruling T32-E7).
        assert_eq!(sample_lines(&[event(10.0, &["early"])], 6000.0), vec!["early"]);
    }

    #[test]
    fn a_line_is_cut_and_cleaned() {
        let long = format!("{}\t\u{1b}{}", "a".repeat(30), "b".repeat(470));
        let lines = sample_lines(&[event(2000.0, &[&long])], 6000.0);
        assert_eq!(lines, vec![format!("{}{}", "a".repeat(30), "b".repeat(30))]);
        assert_eq!(lines[0].chars().count(), LINE_WIDTH);
        let two = sample_lines(&[event(2000.0, &["First part", "second part"])], 6000.0);
        assert_eq!(two, vec!["First part second part"]);
        // The 60 counts characters, not bytes.
        let wide = sample_lines(&[event(2000.0, &[&"é".repeat(100)])], 6000.0);
        assert_eq!(wide[0].chars().count(), 60);
    }

    /// The fixture's held job with two lines on each subtitle track and a staged path.
    fn lined_doc(disc: &Disc, held: &JobRecord) -> ReviewDoc {
        let staged = BTreeMap::from([(KEY.to_string(), r"S:\staging\job\rip\t00-00801\title.mkv".to_string())]);
        let lines = BTreeMap::from([
            ((KEY.to_string(), 9), vec!["Where are you going?".to_string(), "[door slams]".to_string()]),
            ((KEY.to_string(), 33), vec!["Hi, I'm the director.".to_string(), "We shot this scene in March.".to_string()]),
        ]);
        let unread = BTreeMap::new();
        let none = BTreeMap::new();
        let nowhere = BTreeSet::new();
        build(&DocInputs {
            record: held,
            disc,
            staged: &staged,
            lines: &lines,
            unread: &unread,
            collisions: &none,
            nothing_at: &nowhere,
            all: false,
        })
        .unwrap()
    }

    #[test]
    fn the_sheet_for_two_subtitle_tracks_reads_as_the_design_shows_it() {
        let disc = film_disc();
        let text = render(&lined_doc(&disc, &record(&disc)));
        let expected = "\
Held since 15 Jan. Identified by TMDb (0.90). One point needs your answer.

1. Subtitles: two full English tracks that do not match each other.
   lazymkv cannot tell which one carries the film's dialogue.

   track 9   English  1,224 lines   \"Where are you going?\" / \"[door slams]\"
   track 33  English  1,410 lines   \"Hi, I'm the director.\" / \"We shot this scene in March.\"
   To watch first: S:\\staging\\job\\rip\\t00-00801\\title.mkv

   As it stands: track 9 regular, track 33 regular.

Files as: M:/Movies/F (2000)/F (2000) - 2160p.mkv
";
        assert_eq!(text, expected, "\n{text}");
    }

    #[test]
    fn an_unread_track_says_why_in_place_of_lines() {
        let disc = film_disc();
        let text = render(&sheet(&record(&disc), &disc, false));
        assert!(text.contains("track 33  English  1,410 lines   (not read: the time cap was reached)\n"), "{text}");
        assert!(text.contains("track 9   English  1,224 lines   \"Where are you going?\" / \"Wait.\"\n"), "{text}");
    }

    #[test]
    fn a_point_that_cannot_be_answered_says_when_it_can() {
        let disc = film_disc();
        let mut held = record(&disc);
        held.plan.as_mut().unwrap().reviews = vec![ReviewReason::IdentityUnknown];
        decided(&mut held).reviews = vec![ReviewReason::IdentityUnknown];
        let text = render(&sheet(&held, &disc, false));
        assert!(text.contains("1. Which film: lazymkv could not tell which film this disc holds."), "{text}");
        assert!(text.contains("\n   This can be answered from a later version of lazymkv.\n"), "{text}");
        assert!(!text.contains("cannot answer it yet"), "{text}");
        assert!(!text.contains("track "), "{text}");
        println!("{text}");
    }

    #[test]
    fn a_collision_point_and_a_filed_job_read_sensibly() {
        let disc = film_disc();
        let mut held = record(&disc);
        decided(&mut held).reviews = vec![ReviewReason::PathCollision];
        let collisions = BTreeMap::from([(FINAL.to_string(), "40000000000 bytes, 2026-11-01".to_string())]);
        let doc = doc_with(&held, &disc, false, &collisions).unwrap();
        let text = render(&doc);
        println!("{text}");
        assert!(text.contains(&format!("   the file at {FINAL}   40000000000 bytes, 2026-11-01\n")), "{text}");
        assert!(text.contains(&format!("   As it stands: {FINAL}: not answered.\n")), "{text}");
        assert!(!text.contains("To watch first"), "{text}");
        assert_eq!(
            question(item(&doc, "t00-00801 collision")),
            format!("the file at {FINAL}:  k keep both   r replace the existing file     [Enter = leave unanswered]")
        );
        let mut filed = record(&disc);
        filed.state = State::Filed;
        filed.filed_unix = Some(1_800_001_000);
        decided(&mut filed).reviews = vec![];
        let text = render(&sheet(&filed, &disc, false));
        println!("{text}");
        assert!(text.starts_with("Filed on 15 Jan. Identified by TMDb (0.90). Nothing is open.\n"), "{text}");
        assert!(text.contains("1. Audio: nothing is open here, and each track can still be changed.\n"), "{text}");
        assert!(text.contains("2. Subtitles: "), "{text}");
    }

    #[test]
    fn the_question_line_marks_what_enter_keeps() {
        let disc = film_disc();
        let doc = sheet(&record(&disc), &disc, true);
        assert_eq!(
            question(item(&doc, "t00-00801 sub 9")),
            "track 9 is:  r regular   s SDH   f forced   c commentary (dropped)   d drop     [Enter = r]"
        );
        assert_eq!(
            question(item(&doc, "t00-00801 audio 1")),
            "track 1 is:  k keep   p keep, play by default   c commentary (dropped)   d drop     [Enter = p]"
        );
        assert!(question(item(&doc, "t00-00801 audio 8")).ends_with("[Enter = k]"));
    }

    #[test]
    fn a_letter_the_item_does_not_offer_is_none() {
        let disc = film_disc();
        let doc = sheet(&record(&disc), &disc, true);
        let sub = item(&doc, "t00-00801 sub 9");
        assert_eq!(letter_word(sub, "s"), Some("sdh"));
        assert_eq!(letter_word(sub, "c"), Some("commentary"));
        assert_eq!(letter_word(sub, "k"), None);
        assert_eq!(letter_word(sub, "x"), None);
        assert_eq!(letter_word(item(&doc, "t00-00801 audio 1"), "p"), Some("default"));
        assert_eq!(letter_word(item(&doc, "t00-00801 audio 1"), "r"), None);
        let mut narrowed = sub.clone();
        narrowed.answers = vec!["regular".to_string()];
        assert_eq!(letter_word(&narrowed, "s"), None);
        let mut colliding = record(&disc);
        decided(&mut colliding).reviews = vec![ReviewReason::PathCollision];
        let doc = sheet(&colliding, &disc, false);
        let collision = item(&doc, "t00-00801 collision");
        assert_eq!((letter_word(collision, "k"), letter_word(collision, "r")), (Some("keep_both"), Some("replace")));
        assert_eq!(letter_word(collision, "d"), None);
    }

    #[test]
    fn the_head_line_says_which_points_cannot_be_answered_yet() {
        let disc = film_disc();
        let mut held = record(&disc);
        held.plan.as_mut().unwrap().reviews = vec![ReviewReason::IdentityUnknown];
        decided(&mut held).reviews = vec![ReviewReason::IdentityUnknown, ReviewReason::SubtitleAmbiguous];
        let text = render(&sheet(&held, &disc, false));
        let head = text.lines().next().unwrap();
        println!("{head}");
        assert_eq!(head, "Held since 15 Jan. Identified by TMDb (0.90). One point needs your answer. One more cannot be answered yet.");
        decided(&mut held).reviews = vec![ReviewReason::IdentityUnknown];
        let text = render(&sheet(&held, &disc, false));
        let head = text.lines().next().unwrap();
        println!("{head}");
        assert_eq!(head, "Held since 15 Jan. Identified by TMDb (0.90). One point is open and cannot be answered yet.");
    }

    #[test]
    fn a_rip_point_needs_an_answer_and_is_not_for_a_later_version() {
        let disc = film_disc();
        let mut held = record(&disc);
        held.titles[0].rip_reviews = vec![ReviewReason::ReadErrors];
        decided(&mut held).reviews = vec![ReviewReason::ReadErrors];
        let text = render(&sheet(&held, &disc, false));
        let head = text.lines().next().unwrap();
        println!("{head}");
        assert!(head.ends_with("One point needs your answer."), "{head}");
        assert!(!text.contains("later version"), "{text}");
    }

    #[test]
    fn every_printed_sentence_of_an_identity_or_titles_point_ends_with_a_full_stop() {
        let disc = film_disc();
        for reason in [ReviewReason::IdentityUnknown, ReviewReason::IdentityAmbiguous, ReviewReason::TitleAmbiguous, ReviewReason::UnnamedCut] {
            let mut held = record(&disc);
            held.plan.as_mut().unwrap().reviews = vec![reason];
            decided(&mut held).reviews = vec![reason];
            let text = render(&sheet(&held, &disc, false));
            let block: Vec<&str> = text.lines().skip(2).take_while(|line| !line.starts_with("Files as")).filter(|line| !line.is_empty()).collect();
            assert!(block.len() >= 2, "{text}");
            for line in block {
                assert!(line.ends_with('.'), "{reason:?}: {line:?}\n{text}");
            }
        }
    }
}
