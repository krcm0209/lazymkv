//! The review document (phase 3 design, part 6.2): one value that says what is open on a held or filed job, what
//! lazymkv knows about each of its tracks, and which answers each item allows; and the way back from the owner's answer
//! words to `remembered::Overrides` and the labels they carry (part 6.4, 6.6). Pure: the engine hands in the paths,
//! subtitle lines and colliding files it read. Pull request 2 answers tracks and collisions only (ruling T32-R1).

use std::collections::BTreeMap;

use crate::decision::FileDecision;
use crate::disc::{Disc, Stream, Title};
use crate::ipc::job_summary;
use crate::job::{Held, State};
use crate::lang::is_english;
use crate::lookup::DescriptionKind;
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
/// or a title or stream the scan lacks. Def: P3-T4. Use: P3-T5, P3-T8, P3-T9, P3-T14.
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
        if let Some(decision) = &title.decision {
            items.extend(audio_items(&title.key, scanned, decision)?);
            items.extend(subtitle_items(inputs, &title.key, scanned, decision)?);
            if let Some(path) = decision.path.as_ref().filter(|_| reasons.contains(&ReviewReason::PathCollision)) {
                items.push(collision_item(inputs, &title.key, scanned.number, &path.path));
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
            points.push(Point {
                kind,
                sentence: point_sentence(kind, &raised),
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
/// refused: this sheet does not answer them yet (ruling T32-R1). Def: P3-T4. Use: P3-T5, P3-T8, P3-T9, P3-T14.
pub fn check(doc: &ReviewDoc, overrides: &Overrides) -> Result<(), String> {
    if overrides.identity.is_some() || overrides.season.is_some() || !overrides.titles.is_empty() {
        return Err("which film this is and which titles to file cannot be answered on this sheet yet".to_string());
    }
    for entry in &overrides.audio {
        let word = match entry.action {
            AudioOverride::Keep { default: true, .. } => "default",
            AudioOverride::Keep { default: false, .. } => "keep",
            AudioOverride::Commentary => "commentary",
            AudioOverride::Drop => "drop",
        };
        offered(doc, &track_id(doc, entry.title, "audio", entry.stream), word)?;
    }
    for entry in &overrides.subtitles {
        let word = match entry.action {
            SubtitleOverride::Keep { role } => role.as_str(),
            SubtitleOverride::Drop => "drop",
        };
        offered(doc, &track_id(doc, entry.title, "sub", entry.stream), word)?;
    }
    for entry in &overrides.collision {
        let item = items_of(doc)
            .find(|item| item.kind == ItemKind::Collision && item.path.as_deref().is_some_and(|path| same_path(path, &entry.path)))
            .ok_or_else(|| format!("no collision with \"{}\" on this sheet", entry.path))?;
        offered(doc, &item.id, entry.action.as_str())?;
    }
    Ok(())
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
        build(&DocInputs { record, disc, staged: &staged, lines: &lines, unread: &unread, collisions, all })
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
}
