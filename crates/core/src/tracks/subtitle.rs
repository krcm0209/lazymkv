//! English subtitle tracks, design spec 8.7, under the policy of D20. Task 9 builds the evidence the rule decides on:
//! "Set aside first" (with the section's first sentence: every stream in another language is dropped), "Events" (the
//! 10% full rule over the event counts that 6.4 step 10 measures), "Alignment" (the three timing bands and the groups
//! they make) and "Dialogue group" (its picks, in the spec's order). Task 10 adds "Regular and SDH", "Commentary or
//! trivia", "Forced", "Other tracks", the confidence table and "Text" to this file. The event start times arrive in
//! `SubtitleFacts`: Task 12's `verify::subtitle_events` counts them from a recording with `subtitle::align::events`,
//! phase 2 from the file being decided; nothing here reads a file (spec 4.2, RP19). Written from the spec (RP20); used
//! by Tasks 10, 13, 16 and 18.

use std::collections::{BTreeMap, BTreeSet};

use crate::codec::Family;
use crate::disc::{Flags, Stream, StreamKind, Title};
use crate::lang::{is_english, is_no_language, norm_lang};
use crate::lookup::discdb::DiscDbTitle;
use crate::lookup::DescriptionKind;
use crate::remembered::{SubtitleOverride, SubtitleOverrideEntry};
use crate::settings::Settings;
use crate::subtitle::align::aligned;
use crate::subtitle::text::Decision;

/// The roles of a kept English subtitle track (spec 8.7 "Regular and SDH" and "Forced", 8.10's order), defined in
/// `tracks` by Task 1 for the review overrides and re-exported here beside the rule that assigns them.
pub use crate::tracks::SubtitleRole;

/// Facts for one title's subtitle streams, keyed by SCAN stream number: what 6.4 steps 7 and 10 measured, what
/// TheDiscDB and the text rule said, and the remembered and review overrides (spec 5, 11). Task 10's `decide_subtitles`
/// reads every field; at planning time every field is empty. Spec 8.7.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SubtitleFacts<'a> {
    /// 6.4 step 10: event start times per stream (align::events with settings.sub_clear_max_bytes for image streams, 0
    /// for text streams). A stream absent here has no evidence (step 10 skipped: one English stream).
    pub events: BTreeMap<u32, Vec<f64>>,
    /// Streams MakeMKV removed as empty (MSG:4047 mapped to scan streams).
    pub removed: BTreeSet<u32>,
    /// TheDiscDB's title when it reconciles (8.7 "the one TheDiscDB describes as subtitles or SDH").
    pub discdb: Option<&'a DiscDbTitle>,
    /// The text rule's decisions (`crate::subtitle::text::Decision`, from `text::decide`) keyed by scan stream, when OCR
    /// text was read.
    pub text: Option<&'a BTreeMap<u32, Decision>>,
    /// The remembered overrides for this fingerprint (spec 5).
    pub remembered: Option<&'a [SubtitleOverrideEntry]>,
    /// The owner's Override tracks for this title (spec 11), by scan stream; they win over remembered ones. Task 13 passes
    /// `overrides.subtitles_of(title.number)` (that title's entries of `Overrides.subtitles`, by stream).
    pub reviewed: Option<&'a BTreeMap<u32, SubtitleOverride>>,
}

/// Why a subtitle stream was dropped (spec 8.7, and the overrides of spec 5 and 11). Serialised as `{"why": "<snake
/// case>"}`. Spec 8.7.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "why")]
pub enum SubtitleDrop {
    /// A stream in another language (8.7's first sentence), forced-only streams of that language included.
    OtherLanguage,
    /// A derived forced-only stream that turned out empty: MakeMKV removed it (`MSG:4047`) or it has no events.
    DerivedForcedOnlyEmpty,
    /// A derived forced-only stream with events that 8.7 "Forced" did not keep.
    DerivedForcedOnlyUnused,
    /// A DVD stream flagged director's comments (flags 1 or 2), 8.7 "Set aside first".
    DirectorsComments,
    /// A full track outside the dialogue group (8.7 "Commentary or trivia", R6).
    OutsideDialogueGroup,
    /// A track that is not full and has FORCED_MAX_EVENTS events or more (8.7 "Other tracks", with the note
    /// partial_subtitle_dropped).
    Partial,
    /// A track of the dialogue group beyond the two 8.7 "Regular and SDH" keeps (D20: two dialogue tracks with the same
    /// dialogue and neither SDH, keep one).
    ExtraDialogueTrack,
    /// A forced candidate after the one 8.7 "Forced" keeps ("Keep at most one").
    ExtraForced,
    /// The closed-caption stream when the dialogue group already has an SDH track (8.7 "Regular and SDH").
    ClosedCaptionsUnused,
    /// A counted stream with no events that 8.7 keeps in no role: MakeMKV removed it as empty (`MSG:4047`), or 6.4 step 10
    /// measured nothing for it (two or more English streams and no evidence). Task 10.
    NoEvents,
    /// A remembered decision dropped it (spec 5).
    Remembered,
    /// The owner's review dropped it (spec 11 "Override tracks").
    Reviewed,
}

/// Spec 8.7's three alignment bands: Same when the larger aligned share is at least `subs_same_min`, Different when it is
/// at most `subs_diff_max`, Between otherwise (which raises `subtitle_ambiguous`). Spec 8.7 "Alignment".
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Band {
    Same,
    Different,
    Between,
}

/// Spec 14's fixed share of 8.7 "Events": a track is full with at least 10% as many events as the largest English
/// track. Equal to `text::Rule::default().FULL_SHARE` (pinned by a test; Global Constraints' one-copy rule).
pub const FULL_SHARE: f64 = 0.10;

/// Spec 14's fixed limit of 8.7 "Forced" and "Other tracks": a separately authored forced track has fewer events; a
/// track that is not full with this many or more is dropped. Equal to `text::Rule::default().FORCED_MAX_EVENTS` (pinned
/// by a test).
pub const FORCED_MAX_EVENTS: usize = 150;

/// Which of spec 8.7's picks chose the dialogue group, in the spec's order. Spec 8.7 "Dialogue group".
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DialoguePick {
    /// "With one full track, it is the dialogue track."
    OneFull,
    /// "the one containing the parent of a forced-only stream that has events".
    ForcedParent,
    /// "else the one TheDiscDB describes as subtitles or SDH".
    Described,
    /// "else the largest group if it has two or more tracks".
    Largest,
}

/// What spec 8.7's "Set aside first" takes out of counting, alignment and grouping, and the streams left to count. Every
/// subtitle stream of the title is in exactly one list. Spec 8.7.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SetAside {
    /// English or no-language derived forced-only streams (flags 4096 and 2048) with their parent (`parent_of`), in scan
    /// order.
    pub forced_only: Vec<(u32, Option<u32>)>,
    /// DVD streams flagged 1 or 2 (director's comments): dropped (DirectorsComments). A DVD title is one with
    /// attribute 24 (`Title.dvd_title` is Some); a Blu-ray stream flagged 1 or 2 is counted.
    pub directors_comments: Vec<u32>,
    /// The closed-caption stream (codec family ClosedCaptions), when the title has one.
    pub closed_captions: Option<u32>,
    /// Subtitle streams in another language, forced-only ones included (dropped, OtherLanguage), in scan order.
    pub other_language: Vec<u32>,
    /// The English and no-language streams left to count, in scan order (a DVD stream flagged forced, 4096 without
    /// 2048, included).
    pub counted: Vec<u32>,
}

/// Spec 8.7 "Set aside first" and its first sentence (other languages dropped), over the title's subtitle streams in
/// scan order. Each stream is tested in this order and lands in the first list that takes it: a language that is neither
/// English nor no-language goes to `other_language`; a derived forced-only stream (flags 4096 and 2048) to
/// `forced_only`, with its parent; on a DVD title (one with attribute 24, the DVD title number) a stream flagged 1 or 2
/// to `directors_comments`; the first closed-caption stream to `closed_captions`; every other stream to `counted`. A
/// Blu-ray stream flagged 1 or 2 is counted (the spec sets aside DVD streams only), and so is a second closed-caption
/// stream. Spec 8.7.
pub fn set_aside(title: &Title) -> SetAside {
    let subtitles = title.streams_of(StreamKind::Subtitles);
    let dvd = title.dvd_title.is_some();
    let mut out = SetAside {
        forced_only: Vec::new(),
        directors_comments: Vec::new(),
        closed_captions: None,
        other_language: Vec::new(),
        counted: Vec::new(),
    };
    for stream in &subtitles {
        if !english_or_none(stream.language.as_deref()) {
            out.other_language.push(stream.number);
        } else if stream.flags.derived_forced_only() {
            out.forced_only.push((stream.number, parent_of(&subtitles, stream.number)));
        } else if dvd && directors_comments(stream.flags) {
            out.directors_comments.push(stream.number);
        } else if stream.codec.family == Family::ClosedCaptions && out.closed_captions.is_none() {
            out.closed_captions = Some(stream.number);
        } else {
            out.counted.push(stream.number);
        }
    }
    out
}

/// The nearest preceding non-derived subtitle stream of the same language as stream `forced_only`, searching `streams`
/// backwards from it (streams of other kinds and streams flagged 2048 are skipped); None when there is none, when no
/// stream of `streams` is numbered `forced_only`, or when only other languages precede it: a forced-only stream is never
/// adopted by another language (Review Focus 4). Spec 8.7 "Set aside first".
pub fn parent_of(streams: &[&Stream], forced_only: u32) -> Option<u32> {
    let at = streams.iter().position(|stream| stream.number == forced_only)?;
    let language = streams[at].language.as_deref();
    streams[..at]
        .iter()
        .rev()
        .find(|stream| {
            stream.kind == StreamKind::Subtitles
                && !stream.flags.has(Flags::DERIVED)
                && same_language(stream.language.as_deref(), language)
        })
        .map(|stream| stream.number)
}

/// English ("eng" or "en") or no language (lang::is_no_language): the streams 8.7 counts. Spec 8.7, 8.6 "Languages".
pub(crate) fn english_or_none(language: Option<&str>) -> bool {
    is_no_language(language) || language.is_some_and(is_english)
}

/// Two attribute 3 codes name the same language: both no language; both English ("eng" or "en"); or equal through
/// lang::norm_lang (which joins the bibliographic and terminology codes of one language). A no-language code and a
/// language code are never the same language. Spec 8.7 "of the same language".
fn same_language(a: Option<&str>, b: Option<&str>) -> bool {
    if is_no_language(a) || is_no_language(b) {
        return is_no_language(a) && is_no_language(b);
    }
    if a.is_some_and(is_english) && b.is_some_and(is_english) {
        return true;
    }
    norm_lang(a) == norm_lang(b)
}

/// Flags 1 or 2: director's comments or alternate director's comments (spec 9.5; 8.7 "DVD streams flagged director's
/// comments").
fn directors_comments(flags: Flags) -> bool {
    flags.has(Flags::DIRECTORS_COMMENTS) || flags.has(Flags::ALT_DIRECTORS_COMMENTS)
}

/// Spec 8.7 "Events": a track is full when it has at least 10% (FULL_SHARE) as many events as the largest English track,
/// `largest` being that track's count, compared as the text rule compares it (`events as f64 >= FULL_SHARE * largest as
/// f64`, `subtitle::text`), which for a whole number of events is `events >= ceil(FULL_SHARE * largest)`. A track with 0
/// events is never full. Spec 8.7, 14 (the fixed 10%).
pub fn is_full(events: usize, largest: usize) -> bool {
    events > 0 && events as f64 >= FULL_SHARE * largest as f64
}

/// Spec 8.7 "Alignment": the larger of `aligned(a, b)` and `aligned(b, a)` (subtitle::align::aligned, the share of the
/// first list's starts within settings.subs_tolerance_s() of some start of the second), and its band: Same when it is at
/// least settings.subs_same_min, else Different when it is at most settings.subs_diff_max, else Between. Both lists are
/// sorted first (aligned needs its second list sorted; the caller's order never changes the answer). Two lists without
/// events share 0.0, Different. Reads settings.subs_tolerance_ms, subs_same_min and subs_diff_max. Spec 8.7, 14.
pub fn band(a: &[f64], b: &[f64], settings: &Settings) -> (f64, Band) {
    let tolerance = settings.subs_tolerance_s();
    let (a, b) = (sorted(a), sorted(b));
    let larger = aligned(&a, &b, tolerance).max(aligned(&b, &a, tolerance));
    let which = if larger >= settings.subs_same_min {
        Band::Same
    } else if larger <= settings.subs_diff_max {
        Band::Different
    } else {
        Band::Between
    };
    (larger, which)
}

/// A sorted copy of event start times (ascending, `f64::total_cmp`).
fn sorted(starts: &[f64]) -> Vec<f64> {
    let mut out = starts.to_vec();
    out.sort_by(f64::total_cmp);
    out
}

/// Spec 8.7 "Alignment" over the full tracks (stream number and event starts, in scan order): every pair is banded with
/// `band`; two tracks share a group when their band is Same, transitively (A with B and B with C puts A, B and C in one
/// group even when A and C are Different); `ambiguous` is true when any pair is Between (8.7: "In between: review
/// reason subtitle_ambiguous", which Task 10 raises). Groups come in order of their first stream, each listing its
/// streams in `full`'s order. Reads settings.subs_tolerance_ms, subs_same_min and subs_diff_max through `band`. Spec 8.7.
pub fn alignment_groups(full: &[(u32, &[f64])], settings: &Settings) -> (Vec<Vec<u32>>, bool) {
    // parent[i] is a track of i's group listed no later than i; the root of a group is its first track.
    let mut parent: Vec<usize> = (0..full.len()).collect();
    let mut ambiguous = false;
    for i in 0..full.len() {
        for j in (i + 1)..full.len() {
            match band(full[i].1, full[j].1, settings).1 {
                Band::Same => {
                    let (a, b) = (root(&parent, i), root(&parent, j));
                    if a != b {
                        parent[a.max(b)] = a.min(b);
                    }
                }
                Band::Between => ambiguous = true,
                Band::Different => {}
            }
        }
    }
    let mut groups: Vec<Vec<u32>> = Vec::new();
    let mut group_of_root: BTreeMap<usize, usize> = BTreeMap::new();
    for (at, (number, _)) in full.iter().enumerate() {
        let first = root(&parent, at);
        let index = match group_of_root.get(&first) {
            Some(&index) => index,
            None => {
                groups.push(Vec::new());
                group_of_root.insert(first, groups.len() - 1);
                groups.len() - 1
            }
        };
        groups[index].push(*number);
    }
    (groups, ambiguous)
}

/// The first track of `at`'s group: follows `parent` until a track is its own parent.
fn root(parent: &[usize], mut at: usize) -> usize {
    while parent[at] != at {
        at = parent[at];
    }
    at
}

/// Spec 8.7 "Dialogue group", over `groups` (alignment_groups' result), with the spec's picks in order: exactly one full
/// track overall (one group of one track) is OneFull; else the one group holding a stream of `forced_parents` (the
/// parents of forced-only streams that have events) is ForcedParent; else the one group holding a stream whose
/// `described` kinds (`tracks::audio::descriptions`) include Dialogue or Sdh is Described; else the largest group when
/// it has two or more tracks (two or more groups of that size tie and pick nothing: None, so the caller raises
/// subtitle_ambiguous; position never chooses the dialogue group, D15) is Largest; else None (the caller raises
/// subtitle_ambiguous). Forced parents in two or more groups contradict each other: None (subtitle_ambiguous);
/// descriptions in two or more groups name no group, and the largest group is tried. Returns the group's index and the
/// pick. Spec 8.7, D15.
pub fn dialogue_group(
    groups: &[Vec<u32>],
    forced_parents: &[u32],
    described: &BTreeMap<u32, Vec<DescriptionKind>>,
) -> Option<(usize, DialoguePick)> {
    if groups.len() == 1 && groups[0].len() == 1 {
        return Some((0, DialoguePick::OneFull));
    }
    let holding: Vec<usize> = groups
        .iter()
        .enumerate()
        .filter(|(_, group)| group.iter().any(|stream| forced_parents.contains(stream)))
        .map(|(index, _)| index)
        .collect();
    match holding.as_slice() {
        [index] => return Some((*index, DialoguePick::ForcedParent)),
        [] => {}
        _ => return None,
    }
    let says_dialogue = |stream: u32| {
        described.get(&stream).is_some_and(|kinds| {
            kinds.iter().any(|kind| matches!(kind, DescriptionKind::Dialogue | DescriptionKind::Sdh))
        })
    };
    if let Some(index) = only_group(groups, says_dialogue) {
        return Some((index, DialoguePick::Described));
    }
    let largest = groups.iter().map(Vec::len).max().unwrap_or(0);
    if largest >= 2 && groups.iter().filter(|group| group.len() == largest).count() == 1 {
        let index = groups.iter().position(|group| group.len() == largest)?;
        return Some((index, DialoguePick::Largest));
    }
    None
}

/// The index of the one group holding a stream for which `holds` is true; None when no group or two or more groups do.
/// Used by the Described pick only (split descriptions fall through to the largest group; split forced parents do not).
fn only_group(groups: &[Vec<u32>], holds: impl Fn(u32) -> bool) -> Option<usize> {
    let mut found = groups.iter().enumerate().filter(|(_, group)| group.iter().any(|&stream| holds(stream)));
    let (index, _) = found.next()?;
    if found.next().is_some() {
        None
    } else {
        Some(index)
    }
}

#[cfg(test)]
mod tests;
