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
    /// text was read. A caller passes the text it has; `decide_subtitles` uses it only where 6.4 step 10 runs
    /// (`step10_runs`: two or more English or no-language subtitle streams in the file), so a title with one English
    /// track keeps it as "English" without the text (8.7 "Text").
    pub text: Option<&'a BTreeMap<u32, Decision>>,
    /// The remembered overrides of this title (spec 5): the `RememberedTitle.subtitles` of the remembered title
    /// `remembered::matched_titles` matched to this scan title.
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

/// Whether 6.4 step 10 runs for `title` as its file holds it: true when the title has two or more English or
/// no-language subtitle streams (`english_or_none`) that are not in `removed` (MakeMKV's `MSG:4047` removals mapped to
/// scan streams). Every such stream counts, the derived forced-only streams and the closed captions included; streams
/// in another language never do. `title` is the title as the rip's file holds it (narrowed to the rip's selection,
/// `verify::rip_title`), so a stream the rip left out is not counted. Step 10 measures the subtitle evidence and reads
/// the text only where this holds; a title with exactly one English subtitle stream skips it and keeps that track as
/// "English" without the text (8.7 "Text"). The one copy of the rule: `decide_subtitles` uses the text rule's decisions
/// only where it holds, and phase 2's engine asks it whether to run step 10. Spec 6.4 step 10, 8.7 "Text". Def: final
/// review (E2, I1, X1). Use: `decide_subtitles`, `lazymkv-dev decide`, phase 2.
pub fn step10_runs(title: &Title, removed: &BTreeSet<u32>) -> bool {
    title
        .streams_of(StreamKind::Subtitles)
        .iter()
        .filter(|stream| english_or_none(stream.language.as_deref()) && !removed.contains(&stream.number))
        .count()
        >= 2
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

pub use roles::{
    confidence, decide_subtitles, text_disagreements, SubtitleCase, SubtitleDecision, SubtitleTrack,
    SUBTITLE_ALIGNMENT_CONFIDENCE, SUBTITLE_AMBIGUOUS_CONFIDENCE, SUBTITLE_FLAGS_CONFIDENCE, SUBTITLE_ONE_FULL_CONFIDENCE,
};

mod roles {
    //! Spec 8.7 from "Regular and SDH" to the confidence table, and 8.7 "Text" (D20, D23, section 17 row 14's
    //! fallback): the roles of the kept English subtitle tracks, the forced track, the drops of every other subtitle
    //! stream, the confidence of the subtitle decision and the reviews the text rule adds; then the remembered and
    //! review overrides of spec 5 and 11. Built on the evidence half of this file (8.7 "Set aside first" to "Dialogue
    //! group": `set_aside`, `is_full`, `band`, `alignment_groups`, `dialogue_group`). A private module with its own
    //! `use` lines, so that its names never meet those of the evidence half; the `pub use` line above it re-exports its
    //! public items as `tracks::subtitle::*`. Written from the spec (RP20). Used by Tasks 11, 13, 16 and 18.

    use std::collections::{BTreeMap, BTreeSet};

    use crate::disc::{Flags, StreamKind, Title};
    use crate::lookup::{DescriptionKind, REMEMBERED_CONFIDENCE};
    use crate::remembered::{apply_subtitles, SubtitleOverride};
    use crate::review::{sorted_unique, Note, Noted, ReviewReason};
    use crate::settings::Settings;
    use crate::subtitle::text::{Decision, Role};
    use crate::tracks::audio::descriptions;
    use crate::tracks::SubtitleRole;

    use super::{
        alignment_groups, band, dialogue_group, english_or_none, is_full, set_aside, step10_runs, Band, DialoguePick,
        SubtitleDrop, SubtitleFacts, FORCED_MAX_EVENTS,
    };

    // ------------------------------------------------------------------ the confidence table

    /// Spec 8.7's confidence table, first row: flags or database descriptions decide every track, or the title has no
    /// English subtitle stream. Def: T10. Use: T10, T13.
    pub const SUBTITLE_FLAGS_CONFIDENCE: f64 = 0.95;
    /// Second row: exactly one full English track. Def: T10. Use: T10, T13.
    pub const SUBTITLE_ONE_FULL_CONFIDENCE: f64 = 0.90;
    /// Third row: alignment decides between two or more full tracks, with every pair outside the in-between band.
    /// Def: T10. Use: T10, T13.
    pub const SUBTITLE_ALIGNMENT_CONFIDENCE: f64 = 0.85;
    /// Fourth row: subtitle_ambiguous was raised. Def: T10. Use: T10, T13.
    pub const SUBTITLE_AMBIGUOUS_CONFIDENCE: f64 = 0.60;

    /// The confidence rows of 8.7's table. The first row has two cases: flags or descriptions decided (no English or
    /// no-language stream was left to counting and timing, or every counted one was kept by its flag or by its
    /// TheDiscDB description), and no English subtitle stream at all. Spec 8.7. Def: T10. Use: T10, T13.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum SubtitleCase {
        /// Row 1: flags or database descriptions decide every track.
        FlagsOrDescriptions,
        /// Row 1: the title has no English or no-language subtitle stream.
        NoEnglish,
        /// Row 2: exactly one full English track, and the one-stream fallback of section 17 row 14 (b).
        OneFull,
        /// Row 3: alignment decided between two or more full tracks.
        Alignment,
        /// Row 4: subtitle_ambiguous was raised.
        Ambiguous,
    }

    impl SubtitleCase {
        /// Every case, in the table's order. Spec 8.7.
        pub const ALL: [SubtitleCase; 5] = [
            SubtitleCase::FlagsOrDescriptions,
            SubtitleCase::NoEnglish,
            SubtitleCase::OneFull,
            SubtitleCase::Alignment,
            SubtitleCase::Ambiguous,
        ];

        /// The snake_case spelling, as serde writes it. Spec 8.7.
        pub fn as_str(self) -> &'static str {
            match self {
                SubtitleCase::FlagsOrDescriptions => "flags_or_descriptions",
                SubtitleCase::NoEnglish => "no_english",
                SubtitleCase::OneFull => "one_full",
                SubtitleCase::Alignment => "alignment",
                SubtitleCase::Ambiguous => "ambiguous",
            }
        }
    }

    impl std::str::FromStr for SubtitleCase {
        type Err = String;

        /// Only the `as_str` spellings.
        fn from_str(text: &str) -> Result<SubtitleCase, String> {
            SubtitleCase::ALL
                .into_iter()
                .find(|case| case.as_str() == text)
                .ok_or_else(|| format!("not a subtitle confidence case: {text}"))
        }
    }

    /// The table's value for a case (FlagsOrDescriptions and NoEnglish 0.95, OneFull 0.90, Alignment 0.85, Ambiguous
    /// 0.60). Spec 8.7. Def: T10. Use: T10.
    pub fn confidence(case: SubtitleCase) -> f64 {
        match case {
            SubtitleCase::FlagsOrDescriptions | SubtitleCase::NoEnglish => SUBTITLE_FLAGS_CONFIDENCE,
            SubtitleCase::OneFull => SUBTITLE_ONE_FULL_CONFIDENCE,
            SubtitleCase::Alignment => SUBTITLE_ALIGNMENT_CONFIDENCE,
            SubtitleCase::Ambiguous => SUBTITLE_AMBIGUOUS_CONFIDENCE,
        }
    }

    // ------------------------------------------------------------------ the decision and the text's disagreements

    /// A kept subtitle track. Spec 8.7 ("Regular and SDH", "Forced") and 8.10 (its name and flags). Def: T10. Use: T11,
    /// T13, T16.
    #[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    pub struct SubtitleTrack {
        /// The scan's stream number.
        pub stream: u32,
        /// Forced, regular or SDH (8.10's order).
        pub role: SubtitleRole,
        /// `role.name()`: "English (Forced)", "English" or "English (SDH)" (8.7).
        pub name: String,
        /// True for the forced role (8.7 "Flag it forced").
        pub forced: bool,
        /// True for the SDH role (8.7 "hearing-impaired flag"; the closed-caption stream is SDH by type).
        pub hearing_impaired: bool,
        /// Always false in the SubtitleDecision; the file's default subtitle is the Layout's OutputTrack.default (8.10,
        /// `tracks::layout::default_subtitle`).
        pub default: bool,
        /// The stream's event count from 6.4 step 10; None when the step measured nothing for it.
        pub events: Option<usize>,
        /// True for the DVD closed-caption stream.
        pub closed_captions: bool,
        /// For a forced-only stream, its parent.
        pub parent: Option<u32>,
        /// How this track was decided: "flag", "description", "count", "alignment", "closed_captions", "single",
        /// "remembered", "reviewed".
        pub decided_by: String,
    }

    /// One title's subtitle decision. Spec 8.7. Def: T10. Use: T11, T13, T16, T18.
    #[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    pub struct SubtitleDecision {
        /// In order forced, regular, SDH (8.10); within one role by stream number.
        pub kept: Vec<SubtitleTrack>,
        /// Every subtitle stream that is not kept, with why, by stream number (8.7's first sentence).
        pub dropped: Vec<(u32, SubtitleDrop)>,
        /// Set aside (8.7): derived forced-only streams with their parents, flagged DVD streams, the closed-caption
        /// stream, the streams TheDiscDB describes as commentary (R6); each with a sentence; by stream number.
        pub set_aside: Vec<(u32, String)>,
        /// The full English and no-language tracks (8.7 "Events"), in scan order.
        pub full: Vec<u32>,
        /// `alignment_groups` over the full tracks.
        pub groups: Vec<Vec<u32>>,
        /// The index in `groups` of the dialogue group. When groups exist and no rule of 8.7 picks one (two largest
        /// groups of equal size included), the first group stands in for the regular and SDH roles, `dialogue_pick` is
        /// None and subtitle_ambiguous is raised; the full tracks of the other groups are then kept as regular, not
        /// dropped (D15, D20).
        pub dialogue_group: Option<usize>,
        /// Which rule picked the dialogue group.
        pub dialogue_pick: Option<DialoguePick>,
        /// Every pair of full tracks in scan order, with the larger aligned share and its band (8.7 "Alignment").
        pub pairs: Vec<(u32, u32, f64, Band)>,
        /// The row of 8.7's confidence table.
        pub case: SubtitleCase,
        /// `confidence(case)`, or 1.00 when remembered or review overrides cover every English and no-language
        /// subtitle stream of the title still in the file (spec 5, 8.8).
        pub confidence: f64,
        /// partial_subtitle_dropped and subtitle_text_disagrees, each naming its stream, in the order raised.
        pub notes: Vec<Noted>,
        /// subtitle_ambiguous, or nothing (sorted and unique).
        pub reviews: Vec<ReviewReason>,
    }

    /// The streams where the text rule's verdict differs from the decision's, with the text's role and the kept role
    /// (None when the decision drops the stream), in stream order. A stream of `text` is compared when the decision
    /// keeps it or drops it for any reason but another language (an English or no-language stream). SetAside never
    /// differs; Keep always differs (`text::decide` writes Keep only with its own review: the text asks for a review
    /// 8.7 did not raise); Drop differs from any kept role; Regular, Sdh and Forced differ from a drop and from another
    /// kept role; and any verdict but SetAside that carries the text rule's review differs (`text::before_yes` reads a
    /// review that 8.7 does not raise as a disagreement). Spec 8.7 "Text", Open point 13, D20.
    fn disagreements(
        kept: &[SubtitleTrack],
        dropped: &BTreeMap<u32, SubtitleDrop>,
        text: &BTreeMap<u32, Decision>,
    ) -> Vec<(u32, Role, Option<SubtitleRole>)> {
        let mut out = Vec::new();
        for (&stream, verdict) in text {
            let role = kept.iter().find(|track| track.stream == stream).map(|track| track.role);
            let why = dropped.get(&stream);
            if role.is_none() && why.is_none() {
                continue;
            }
            if matches!(why, Some(SubtitleDrop::OtherLanguage)) {
                continue;
            }
            // A review as text::before_yes reads one: present and not empty.
            let reviewed = verdict.review.as_deref().is_some_and(|review| !review.is_empty());
            let differs = match verdict.role {
                Role::SetAside => false,
                // text::decide writes Keep only with its own review: the text asks for a review 8.7 did not raise.
                Role::Keep => true,
                Role::Drop => role.is_some(),
                Role::Regular => role != Some(SubtitleRole::Regular),
                Role::Sdh => role != Some(SubtitleRole::Sdh),
                Role::Forced => role != Some(SubtitleRole::Forced),
            } || (verdict.role != Role::SetAside && reviewed);
            if differs {
                out.push((stream, verdict.role, role));
            }
        }
        out
    }

    /// 8.7 "Text": the streams where a text decision differs from the counts, for any English or no-language stream:
    /// its role (Regular, Sdh, Forced, Drop) differs from the kept role (or kept against a Drop, or dropped against a
    /// kept role). SetAside never disagrees; Keep, and any verdict carrying the text rule's review, always disagree
    /// (text::before_yes's reading: text::decide writes Keep only with its own review). Streams dropped as another
    /// language's, and streams the decision does not list, are not compared. In stream order. The public form of the
    /// rule's text check, kept for phase 2's review view. Def: T10. Use: T10.
    pub fn text_disagreements(decision: &SubtitleDecision, text: &BTreeMap<u32, Decision>) -> Vec<u32> {
        let dropped: BTreeMap<u32, SubtitleDrop> = decision.dropped.iter().cloned().collect();
        disagreements(&decision.kept, &dropped, text).into_iter().map(|(stream, _, _)| stream).collect()
    }

    // ------------------------------------------------------------------ the rule

    /// How a kept track was decided (`SubtitleTrack.decided_by`): by its flags (a derived forced-only stream, a DVD stream
    /// flagged forced), by TheDiscDB's description, by event counts (the one full track, a separately authored forced
    /// track), by alignment (two or more full tracks), by the closed-caption type, or as the one English stream.
    const BY_FLAG: &str = "flag";
    const BY_DESCRIPTION: &str = "description";
    const BY_COUNT: &str = "count";
    const BY_ALIGNMENT: &str = "alignment";
    const BY_CLOSED_CAPTIONS: &str = "closed_captions";
    const BY_SINGLE: &str = "single";

    /// The set-aside sentence of a DVD stream flagged director's comments.
    const DIRECTORS_COMMENTS_LINE: &str = "DVD stream flagged director's comments (flags 1 or 2; 8.7 \"Set aside first\")";
    /// The set-aside sentence of the DVD closed-caption stream.
    const CLOSED_CAPTIONS_LINE: &str = "DVD closed-caption stream (codec CC; 8.7 \"Set aside first\")";
    /// The set-aside sentence of a counted stream TheDiscDB describes as commentary (R6, D15). It is dropped as
    /// OutsideDialogueGroup: Task 9's `SubtitleDrop` has no variant for a described drop.
    const DESCRIBED_COMMENTARY_LINE: &str = "described as commentary by TheDiscDB (R6)";

    /// The set-aside sentence of a derived forced-only stream.
    fn forced_only_line(parent: Option<u32>) -> String {
        match parent {
            Some(parent) => {
                format!("derived forced-only stream of stream {parent} (flags 4096 and 2048; 8.7 \"Set aside first\")")
            }
            None => "derived forced-only stream with no preceding stream of its language (flags 4096 and 2048; 8.7 \
                     \"Set aside first\")"
                .to_string(),
        }
    }

    /// The detail of a partial_subtitle_dropped note.
    fn partial_line(stream: u32, events: usize, largest: usize) -> String {
        format!(
            "stream {stream}: {events} events, not full (the largest English track has {largest}) and \
             {FORCED_MAX_EVENTS} or more (8.7 \"Other tracks\")"
        )
    }

    /// The event start times 6.4 step 10 gave for `stream`; none for a stream MakeMKV removed as empty (MSG:4047) or one
    /// the step did not measure.
    fn events_of<'f>(facts: &'f SubtitleFacts<'_>, stream: u32) -> &'f [f64] {
        if facts.removed.contains(&stream) {
            return &[];
        }
        facts.events.get(&stream).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Attribute 22 of `stream` (no flags when the title has no such stream).
    fn flags_of(title: &Title, stream: u32) -> Flags {
        title
            .streams
            .iter()
            .find(|candidate| candidate.number == stream)
            .map(|candidate| candidate.flags)
            .unwrap_or_default()
    }

    /// A kept track with the name and flags of its role (8.7, 8.10).
    fn track(
        facts: &SubtitleFacts<'_>,
        stream: u32,
        role: SubtitleRole,
        decided_by: &str,
        parent: Option<u32>,
        closed_captions: bool,
    ) -> SubtitleTrack {
        SubtitleTrack {
            stream,
            role,
            name: role.name().to_string(),
            forced: role == SubtitleRole::Forced,
            hearing_impaired: role == SubtitleRole::Sdh,
            default: false,
            events: facts.events.get(&stream).map(Vec::len),
            closed_captions,
            parent,
            decided_by: decided_by.to_string(),
        }
    }

    /// Spec 8.7 in full, in order: `set_aside`; a counted stream in the file that TheDiscDB describes as commentary
    /// (not as dialogue or SDH) is set aside and dropped (OutsideDialogueGroup; R6, D15); events from `facts.events`;
    /// full = `is_full` against the largest English track's events (a stream flagged forced, 4096 without 2048, is
    /// never full: its flag makes it a forced source); `alignment_groups` over the full tracks with the three bands (a
    /// Between pair raises subtitle_ambiguous); `dialogue_group` with the parents of forced-only streams that have
    /// events and `tracks::audio::descriptions` (None, two largest groups of equal size included, raises
    /// subtitle_ambiguous, and the first group stands in for the regular and SDH roles); regular and SDH (up to two:
    /// more events is SDH, a description saying SDH overrides; the closed-caption stream is SDH when the group has
    /// none; more than two gives subtitle_ambiguous, and a track described SDH is one of the two kept); full tracks
    /// outside a picked group dropped (OutsideDialogueGroup; with no pick, or with a Between band against a track of
    /// the picked group, they are kept as regular under the review, never dropped unconfirmed or by position); forced
    /// (first: a forced-only stream with events whose parent is the regular track, then the SDH track's; then a DVD
    /// stream flagged forced; then a separately authored track that is not full with fewer than FORCED_MAX_EVENTS
    /// events); other not-full tracks with FORCED_MAX_EVENTS or more events dropped with partial_subtitle_dropped,
    /// those with no events dropped NoEvents; `text_disagreements` (subtitle_ambiguous and one subtitle_text_disagrees
    /// note per stream), only where 6.4 step 10 runs (`step10_runs`: the text rule's decisions are used only when the
    /// file holds two or more English or no-language subtitle streams, 8.7 "Text"; otherwise `facts.text` is ignored);
    /// the confidence case; remembered then review overrides applied last (decided_by "remembered" or
    /// "reviewed"; an override naming a stream that is not an English or no-language subtitle stream is ignored), with
    /// confidence lookup::REMEMBERED_CONFIDENCE (1.00) and subtitle_ambiguous cleared when every English and
    /// no-language subtitle stream of the title still in the file (not in facts.removed) has an override
    /// (other-language streams are dropped by 8.7's first sentence; the case unchanged). A title with one English
    /// stream and no events keeps it as Regular "English" (row 14 (b) fallback), case OneFull; when that stream is
    /// flagged forced (4096 without 2048) it is kept as Forced by its flag, case FlagsOrDescriptions.
    ///
    /// Where the spec leaves a choice (Task 10's list): `set_aside`'s five lists say which stream is which; a counted
    /// stream TheDiscDB describes as commentary is set aside and dropped; a stream MakeMKV removed (`facts.removed`)
    /// has no events and is not kept; with groups but no pick (two equal largest groups included) the first group
    /// stands in for the regular and SDH roles under subtitle_ambiguous and the other groups' full tracks are kept as
    /// regular, and so is a full track in the in-between band with a track of a picked group; two dialogue tracks with
    /// equal event counts raise subtitle_ambiguous (the first in scan order is regular); a description saying SDH makes
    /// a one-track group SDH and keeps its track among the two of a larger group; every forced source needs at least
    /// one event but the one English stream in the file flagged forced; a not-full track that is not the forced track
    /// is dropped as NoEvents with no events and as ExtraForced with fewer than FORCED_MAX_EVENTS. Spec 8.7, D15, D20,
    /// R5, R6. Def: T10. Use: T13, T16, T18.
    pub fn decide_subtitles(title: &Title, facts: &SubtitleFacts<'_>, settings: &Settings) -> SubtitleDecision {
        let aside = set_aside(title);
        // The English and no-language subtitle streams (Task 9's `english_or_none`, the one copy): the streams 8.7 judges.
        let english: BTreeSet<u32> = title
            .streams_of(StreamKind::Subtitles)
            .iter()
            .filter(|stream| english_or_none(stream.language.as_deref()))
            .map(|stream| stream.number)
            .collect();
        let mut dropped: BTreeMap<u32, SubtitleDrop> = BTreeMap::new();
        let mut aside_lines: Vec<(u32, String)> = Vec::new();
        let mut reviews: Vec<ReviewReason> = Vec::new();
        let mut notes: Vec<Noted> = Vec::new();

        // 8.7's first sentence: every stream in another language is dropped, forced-only ones included.
        for &stream in &aside.other_language {
            dropped.insert(stream, SubtitleDrop::OtherLanguage);
        }
        // "Set aside first": a DVD stream flagged director's comments (flags 1 or 2) is dropped.
        for &stream in &aside.directors_comments {
            dropped.insert(stream, SubtitleDrop::DirectorsComments);
            aside_lines.push((stream, DIRECTORS_COMMENTS_LINE.to_string()));
        }
        // "Set aside first": the derived forced-only streams with their parents, and the closed-caption stream.
        let forced_only: &[(u32, Option<u32>)] = &aside.forced_only;
        for &(stream, parent) in forced_only {
            aside_lines.push((stream, forced_only_line(parent)));
        }
        let closed_captions = aside.closed_captions;
        if let Some(stream) = closed_captions {
            aside_lines.push((stream, CLOSED_CAPTIONS_LINE.to_string()));
        }
        let counted: &[u32] = &aside.counted;
        // The counted streams still in the file: MakeMKV removes an empty stream and says so (MSG:4047, 6.2 step 7).
        let present: Vec<u32> = counted.iter().copied().filter(|stream| !facts.removed.contains(stream)).collect();
        // R6 and D15: a counted stream in the file that TheDiscDB describes as commentary (and not as dialogue or SDH)
        // is confirmed commentary. It takes no part in alignment or grouping and is dropped, whatever its count.
        let (_, described) = descriptions(title, facts.discdb);
        let commentary: Vec<u32> = present
            .iter()
            .copied()
            .filter(|stream| {
                described.get(stream).is_some_and(|kinds| {
                    kinds.contains(&DescriptionKind::Commentary)
                        && !kinds.iter().any(|kind| matches!(kind, DescriptionKind::Dialogue | DescriptionKind::Sdh))
                })
            })
            .collect();
        for &stream in &commentary {
            dropped.insert(stream, SubtitleDrop::OutsideDialogueGroup);
            aside_lines.push((stream, DESCRIBED_COMMENTARY_LINE.to_string()));
        }
        // The streams in the file left to judge by counts, flags and timing.
        let left_to_judge: Vec<u32> = present.iter().copied().filter(|stream| !commentary.contains(stream)).collect();

        // "Events": a track is full with at least 10% as many events as the largest English track. A stream described as
        // commentary is set aside above, and set-aside streams take no part in counting (8.7 "Set aside first"), so it
        // never sets the largest.
        let count = |stream: u32| events_of(facts, stream).len();
        let largest = counted
            .iter()
            .filter(|stream| !commentary.contains(stream))
            .map(|&stream| count(stream))
            .max()
            .unwrap_or(0);
        // A stream flagged forced (4096 without 2048) is never full: its flag makes it a forced source ("Forced"; the
        // confidence table's first row lets flags decide), so it never joins a dialogue group. Nor is a stream
        // described as commentary.
        let full: Vec<u32> = counted
            .iter()
            .copied()
            .filter(|&stream| {
                !commentary.contains(&stream)
                    && !flags_of(title, stream).forced_authored()
                    && is_full(count(stream), largest)
            })
            .collect();

        // "Alignment": every pair of full tracks with its band; a pair in between raises subtitle_ambiguous.
        let mut pairs: Vec<(u32, u32, f64, Band)> = Vec::new();
        for (at, &a) in full.iter().enumerate() {
            for &b in &full[at + 1..] {
                let (share, which) = band(events_of(facts, a), events_of(facts, b), settings);
                pairs.push((a, b, share, which));
            }
        }
        let full_events: Vec<(u32, &[f64])> = full.iter().map(|&stream| (stream, events_of(facts, stream))).collect();
        let (groups, between) = alignment_groups(&full_events, settings);
        if between {
            reviews.push(ReviewReason::SubtitleAmbiguous);
        }

        // "Dialogue group": with groups and no pick, subtitle_ambiguous, and the first group stands in for the regular
        // and SDH roles.
        let forced_parents: Vec<u32> = forced_only
            .iter()
            .filter(|(stream, _)| count(*stream) > 0)
            .filter_map(|(_, parent)| *parent)
            .collect();
        // Task 9's dialogue_group picks nothing when two or more groups tie for the largest size (D15): no pick.
        let (dialogue_index, dialogue_pick) = match dialogue_group(&groups, &forced_parents, &described) {
            Some((index, pick)) => (Some(index), Some(pick)),
            None if !groups.is_empty() => {
                reviews.push(ReviewReason::SubtitleAmbiguous);
                (Some(0), None)
            }
            None => (None, None),
        };
        let mut group: Vec<u32> = dialogue_index.map(|index| groups[index].clone()).unwrap_or_default();
        group.sort_unstable();

        // "Regular and SDH": up to two tracks of the group, the regular and the SDH track (R5, D20). More than two raise
        // subtitle_ambiguous and keep one of each: as SDH a track a TheDiscDB description calls SDH ("A TheDiscDB
        // description saying SDH overrides the count"), the one with the most events among them, else the track with the
        // most events; as regular, of the rest, the track with the fewest events ("fewer events means regular"), an
        // undescribed track before one described as SDH, so two described-SDH tracks are never kept while an undescribed
        // one exists (ties by scan order). The others are dropped.
        let says_sdh =
            |stream: u32| described.get(&stream).is_some_and(|kinds| kinds.contains(&DescriptionKind::Sdh));
        let mut chosen = group.clone();
        if chosen.len() > 2 {
            reviews.push(ReviewReason::SubtitleAmbiguous);
            let described_sdh: Vec<u32> = chosen.iter().copied().filter(|&stream| says_sdh(stream)).collect();
            let candidates = if described_sdh.is_empty() { &chosen } else { &described_sdh };
            let most = |stream: &u32| (count(*stream), std::cmp::Reverse(*stream));
            let sdh_pick = candidates.iter().copied().max_by_key(most);
            let regular_pick = chosen
                .iter()
                .copied()
                .filter(|&stream| Some(stream) != sdh_pick)
                .min_by_key(|&stream| (says_sdh(stream), count(stream), stream));
            let keep: BTreeSet<u32> = sdh_pick.into_iter().chain(regular_pick).collect();
            for &stream in &chosen {
                if !keep.contains(&stream) {
                    dropped.insert(stream, SubtitleDrop::ExtraDialogueTrack);
                }
            }
            chosen.retain(|stream| keep.contains(stream));
        }
        let by_events_rule = if full.len() >= 2 { BY_ALIGNMENT } else { BY_COUNT };
        let mut regular: Option<(u32, &'static str)> = None;
        let mut sdh: Option<(u32, &'static str)> = None;
        match chosen.as_slice() {
            [one] => {
                if says_sdh(*one) {
                    sdh = Some((*one, BY_DESCRIPTION));
                } else {
                    regular = Some((*one, by_events_rule));
                }
            }
            [a, b] => match (says_sdh(*a), says_sdh(*b)) {
                (true, false) => {
                    sdh = Some((*a, BY_DESCRIPTION));
                    regular = Some((*b, BY_DESCRIPTION));
                }
                (false, true) => {
                    sdh = Some((*b, BY_DESCRIPTION));
                    regular = Some((*a, BY_DESCRIPTION));
                }
                _ => {
                    if count(*a) == count(*b) {
                        reviews.push(ReviewReason::SubtitleAmbiguous);
                    }
                    let (fewer, more) = if count(*b) >= count(*a) { (*a, *b) } else { (*b, *a) };
                    regular = Some((fewer, by_events_rule));
                    sdh = Some((more, by_events_rule));
                }
            },
            _ => {}
        }
        // Section 17 row 14 (b)'s fallback: one English stream and no events (6.4 step 10 is skipped) is kept as
        // "English", unless it is flagged forced (4096 without 2048): its flag makes it the forced track, below. Two or
        // more streams in the file and no full track leave no group: subtitle_ambiguous.
        if groups.is_empty() {
            match left_to_judge.as_slice() {
                [only] if count(*only) == 0 && !flags_of(title, *only).forced_authored() => {
                    regular = Some((*only, BY_SINGLE))
                }
                [] | [_] => {}
                _ => reviews.push(ReviewReason::SubtitleAmbiguous),
            }
        }

        // "Commentary or trivia": a full track outside the dialogue group is dropped (R6). With no pick there is no
        // dialogue group to be outside of and nothing confirms the track as commentary: it is kept as regular under the
        // review already raised (D20: an unconfirmed one is kept and reviewed; D15: never decided by position). The
        // same holds with a pick for a track whose band with a track of the group is Between: 8.7 calls that pair
        // undecided and has raised subtitle_ambiguous for it. Only a track Different from every track of a picked group
        // is dropped.
        let mut unconfirmed: Vec<u32> = Vec::new();
        for &stream in &full {
            if group.contains(&stream) {
                continue;
            }
            let between_group = pairs.iter().any(|&(a, b, _, which)| {
                which == Band::Between && ((a == stream && group.contains(&b)) || (b == stream && group.contains(&a)))
            });
            if dialogue_pick.is_none() || between_group {
                unconfirmed.push(stream);
            } else {
                dropped.insert(stream, SubtitleDrop::OutsideDialogueGroup);
            }
        }
        // The closed-caption stream is SDH by type, kept when the dialogue group has no SDH track.
        let mut kept_closed_captions = None;
        if let Some(stream) = closed_captions {
            if sdh.is_none() {
                kept_closed_captions = Some(stream);
            } else {
                dropped.insert(stream, SubtitleDrop::ClosedCaptionsUnused);
            }
        }

        // "Forced": at most one, in the spec's order; every source needs at least one event but the lone stream
        // flagged forced below.
        let regular_stream = regular.map(|(stream, _)| stream);
        let sdh_stream = sdh.map(|(stream, _)| stream);
        let mut forced: Option<(u32, Option<u32>, &'static str)> = None;
        for dialogue in [regular_stream, sdh_stream].into_iter().flatten() {
            if forced.is_none() {
                forced = forced_only
                    .iter()
                    .find(|(stream, parent)| *parent == Some(dialogue) && count(*stream) > 0)
                    .map(|&(stream, parent)| (stream, parent, BY_FLAG));
            }
        }
        let rest: Vec<u32> = counted
            .iter()
            .copied()
            .filter(|stream| {
                !full.contains(stream)
                    && Some(*stream) != regular_stream
                    && Some(*stream) != sdh_stream
                    && !dropped.contains_key(stream)
            })
            .collect();
        // A stream flagged forced needs events, unless it is the only English stream in the file left to judge (6.4
        // step 10 does not run for one English stream, and a stream flagged forced is never full, so there is no
        // group): its flag alone makes it forced.
        if forced.is_none() {
            forced = rest
                .iter()
                .copied()
                .find(|&stream| {
                    flags_of(title, stream).forced_authored()
                        && (count(stream) > 0 || (groups.is_empty() && left_to_judge == [stream]))
                })
                .map(|stream| (stream, None, BY_FLAG));
        }
        if forced.is_none() {
            forced = rest
                .iter()
                .copied()
                .find(|&stream| count(stream) > 0 && count(stream) < FORCED_MAX_EVENTS)
                .map(|stream| (stream, None, BY_COUNT));
        }
        let forced_stream = forced.map(|(stream, _, _)| stream);
        // "Other tracks": not full with FORCED_MAX_EVENTS events or more, dropped with partial_subtitle_dropped; a stream
        // with no events (MakeMKV removed it, or 6.4 step 10 measured nothing) is dropped NoEvents; the other shorter
        // ones that are not the forced track are dropped as extra forced candidates.
        for &stream in &rest {
            if Some(stream) == forced_stream {
                continue;
            }
            if count(stream) >= FORCED_MAX_EVENTS {
                dropped.insert(stream, SubtitleDrop::Partial);
                notes.push(Noted {
                    note: Note::PartialSubtitleDropped,
                    stream: Some(stream),
                    detail: partial_line(stream, count(stream), largest),
                });
            } else if count(stream) == 0 {
                dropped.insert(stream, SubtitleDrop::NoEvents);
            } else {
                dropped.insert(stream, SubtitleDrop::ExtraForced);
            }
        }
        for &(stream, _) in forced_only {
            if Some(stream) != forced_stream {
                let why = if count(stream) > 0 {
                    SubtitleDrop::DerivedForcedOnlyUnused
                } else {
                    SubtitleDrop::DerivedForcedOnlyEmpty
                };
                dropped.insert(stream, why);
            }
        }

        let mut kept: Vec<SubtitleTrack> = Vec::new();
        if let Some((stream, parent, by)) = forced {
            kept.push(track(facts, stream, SubtitleRole::Forced, by, parent, false));
        }
        if let Some((stream, by)) = regular {
            kept.push(track(facts, stream, SubtitleRole::Regular, by, None, false));
        }
        if let Some((stream, by)) = sdh {
            kept.push(track(facts, stream, SubtitleRole::Sdh, by, None, false));
        }
        for &stream in &unconfirmed {
            kept.push(track(facts, stream, SubtitleRole::Regular, BY_ALIGNMENT, None, false));
        }
        if let Some(stream) = kept_closed_captions {
            kept.push(track(facts, stream, SubtitleRole::Sdh, BY_CLOSED_CAPTIONS, None, true));
        }

        // Spec 5 and 11: the remembered overrides, then the owner's, which win; they are applied last, below. A stream
        // with an override is the owner's decision, so the text is not compared on it.
        let overrides = overrides_of(title, facts, &english);

        // 8.7 "Text": where the text's verdict differs from the counts and timing, subtitle_ambiguous and one
        // subtitle_text_disagrees note per stream; the text never changes a role, a drop or a flag (D20). The text is
        // read only where 6.4 step 10 runs (two or more English or no-language subtitle streams in the file): a title
        // with one English track keeps it as "English" without the flag.
        if let Some(text) = facts.text.filter(|_| step10_runs(title, &facts.removed)) {
            for (stream, read, role) in disagreements(&kept, &dropped, text) {
                if overrides.contains_key(&stream) {
                    continue;
                }
                reviews.push(ReviewReason::SubtitleAmbiguous);
                notes.push(Noted {
                    note: Note::SubtitleTextDisagrees,
                    stream: Some(stream),
                    detail: text_line(stream, read, role),
                });
            }
        }

        // The confidence table: a review first; then nothing left to count (no English stream at all, or flags
        // decided); then every counted stream kept by its flag or its description, or dropped by its description; then
        // the number of full tracks.
        let case = if reviews.contains(&ReviewReason::SubtitleAmbiguous) {
            SubtitleCase::Ambiguous
        } else if present.is_empty() {
            if english.is_empty() {
                SubtitleCase::NoEnglish
            } else {
                SubtitleCase::FlagsOrDescriptions
            }
        } else if present.iter().all(|stream| {
            commentary.contains(stream)
                || kept.iter().any(|kept_track| {
                    kept_track.stream == *stream && matches!(kept_track.decided_by.as_str(), BY_FLAG | BY_DESCRIPTION)
                })
        }) {
            SubtitleCase::FlagsOrDescriptions
        } else if full.len() >= 2 {
            SubtitleCase::Alignment
        } else {
            SubtitleCase::OneFull
        };

        // The overrides, last (decided_by "remembered" or "reviewed"): a drop replaces the rule's outcome, a keep sets
        // the role (a forced-only stream keeps its parent). When they cover every English and no-language subtitle
        // stream still in the file the confidence is 1.00 and the review they answer is settled (8.8); the case stays
        // the rule's. A stream MakeMKV removed (facts.removed) is not in the file, and the owner's Override tracks
        // choose among the file's tracks, so coverage never waits for it.
        let judged: Vec<u32> = english.iter().copied().filter(|stream| !facts.removed.contains(stream)).collect();
        let covered = !judged.is_empty() && judged.iter().all(|stream| overrides.contains_key(stream));
        for (&stream, (action, by)) in &overrides {
            kept.retain(|kept_track| kept_track.stream != stream);
            notes.retain(|noted| noted.stream != Some(stream));
            match action {
                SubtitleOverride::Drop => {
                    let why = if *by == BY_REMEMBERED { SubtitleDrop::Remembered } else { SubtitleDrop::Reviewed };
                    dropped.insert(stream, why);
                }
                SubtitleOverride::Keep { role } => {
                    dropped.remove(&stream);
                    let parent =
                        forced_only.iter().find(|(forced, _)| *forced == stream).and_then(|(_, parent)| *parent);
                    kept.push(track(facts, stream, *role, by, parent, closed_captions == Some(stream)));
                }
            }
        }
        if covered {
            reviews.clear();
        }
        let value = if covered { REMEMBERED_CONFIDENCE } else { confidence(case) };

        kept.sort_by_key(|kept_track| (kept_track.role, kept_track.stream));
        aside_lines.sort_by_key(|(stream, _)| *stream);
        SubtitleDecision {
            kept,
            dropped: dropped.into_iter().collect(),
            set_aside: aside_lines,
            full,
            groups,
            dialogue_group: dialogue_index,
            dialogue_pick,
            pairs,
            case,
            confidence: value,
            notes,
            reviews: sorted_unique(reviews),
        }
    }

    /// The detail of a subtitle_text_disagrees note: what the text reads and what the counts and timing decided.
    fn text_line(stream: u32, read: Role, kept: Option<SubtitleRole>) -> String {
        let counts = match kept {
            Some(role) => format!("keep it as {}", role.as_str()),
            None => "drop it".to_string(),
        };
        format!("stream {stream}: the text reads {}, the counts {counts} (8.7 \"Text\")", read.as_str())
    }

    /// `decided_by` of a track a remembered decision keeps (spec 5), and of one the owner's Override tracks keeps (spec 11).
    const BY_REMEMBERED: &str = "remembered";
    const BY_REVIEWED: &str = "reviewed";

    /// The overrides of the title's English and no-language subtitle streams (`english`, the set `decide_subtitles`
    /// builds with `english_or_none`), by scan stream: the remembered ones (spec 5, matched by stream signature through
    /// `remembered::apply_subtitles`), then the owner's (spec 11), which replace a remembered one on the same stream;
    /// each with the `decided_by` it gives. An override naming a stream that is not one of the title's subtitle streams
    /// is ignored, and so is one naming another language's stream: 8.7's first sentence drops it, and `SubtitleRole`
    /// names only English roles (8.7, 8.10).
    fn overrides_of(
        title: &Title,
        facts: &SubtitleFacts<'_>,
        english: &BTreeSet<u32>,
    ) -> BTreeMap<u32, (SubtitleOverride, &'static str)> {
        let mut out = BTreeMap::new();
        if let Some(entries) = facts.remembered {
            for (stream, action) in apply_subtitles(entries, &title.streams) {
                if english.contains(&stream) {
                    out.insert(stream, (action.clone(), BY_REMEMBERED));
                }
            }
        }
        if let Some(reviewed) = facts.reviewed {
            for (stream, action) in reviewed {
                if english.contains(stream) {
                    out.insert(*stream, (action.clone(), BY_REVIEWED));
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests;
