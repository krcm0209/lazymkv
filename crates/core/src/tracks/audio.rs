//! The audio track rule of design spec 8.6: which audio streams of a title are kept (steps 1 and 2), how each
//! language's tracks are ranked and guarded (steps 3 and 4), their order and flags (step 5), the notes and review
//! reasons of step 6, and how the content rule's verdicts reach the decision ("Content", D15, D21, RP7). Step 1's
//! TheDiscDB descriptions come through `align_descriptions` and `descriptions`, which Tasks 9 and 10 reuse for
//! subtitles. D13's exact duplicates are found by packet MD5 (6.4 step 10a); D9's defaults are the settings
//! `prefer_most_channels` and `original_language_first`; spec 5's remembered decisions and spec 11's review
//! overrides fix what they cover. Written from the spec (RP20): `rank_key` and `guards` are the product's copy of
//! steps 3 and 4 over scan facts, beside the Python-exact `audio::reference` that the content rule's goldens use.
//! Used by Tasks 9, 11, 13 and 16.

use std::collections::BTreeMap;
use std::str::FromStr;

use crate::audio::rule::Verdict;
use crate::codec::{channel_layout, object_audio, tier, Codec, ObjectAudio, Tier};
use crate::disc::{Flags, Stream, StreamKind, Title};
use crate::lang::{is_english, is_no_language, norm_lang, Wanted};
use crate::lookup::discdb::{DiscDbTitle, DiscDbTrack};
use crate::lookup::{description_kinds, DescriptionKind, REMEMBERED_CONFIDENCE};
use crate::remembered::{apply_audio, AudioOverride, AudioOverrideEntry};
use crate::review::{sorted_unique, Note, Noted, ReviewReason};
use crate::settings::Settings;

/// Facts the file phase measured for one title, keyed by SCAN stream number (the dev crate and phase 2 map file tracks
/// back through verify::stream_of_track). At planning time it is `AudioFacts::default()`: no MD5, no profile, no
/// verdict. Spec 8.6, 6.4 steps 9 and 10a. Def: T8. Use: T13, T16.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AudioFacts<'a> {
    /// 6.4 step 10a: the packet MD5 of every audio stream (D13). Empty at planning time.
    pub md5: BTreeMap<u32, String>,
    /// 6.4 step 9: ffprobe's profile per audio stream ("DTS-HD MA", "DTS-HD MA + DTS:X"; the Atmos spelling is
    /// unverified). Empty at planning time (8.3: the ffprobe criterion is skipped).
    pub ffprobe_profile: BTreeMap<u32, String>,
    /// TheDiscDB's title when it reconciles (8.6 step 1's descriptions).
    pub discdb: Option<&'a DiscDbTitle>,
    /// The content rule's verdict (`audio::rule::Verdict`) per candidate stream (from audio::rule::results, mapped to
    /// scan streams).
    pub verdicts: BTreeMap<u32, Verdict>,
    /// Whether D21's veto held for the title.
    pub veto_holds: bool,
    /// The scan stream the verdicts were measured against: the level folder's `main=` stream
    /// (audio::reference::programme_reference in phase 0's harness) mapped to a scan stream; None when there are no
    /// verdicts.
    pub verdict_reference: Option<u32>,
    /// The remembered overrides for this fingerprint (spec 5).
    pub remembered: Option<&'a [AudioOverrideEntry]>,
    /// The owner's Override tracks for this title (spec 11), by scan stream; they win over remembered ones.
    pub reviewed: Option<&'a BTreeMap<u32, AudioOverride>>,
}

/// Why an audio stream was dropped, in spec 8.6 step 1's bullet order (D13, RP7, spec 5 and 11 overrides); written
/// with its reason under "why" (`{"why": "flagged", "flags": 1}`). Def: T8. Use: T13, T16.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "why")]
pub enum AudioDrop {
    /// Flag 2048: a stream MakeMKV derived from another (the AC-3 or DTS core of a lossless track).
    Derived,
    /// Flags 1, 2 or 4 (director's comments, alternate director's comments, visually impaired); `flags` holds the
    /// ones of the three that are set.
    Flagged {
        /// The set bits among 1, 2 and 4.
        flags: u32,
    },
    /// Flag 512: secondary (picture-in-picture) audio.
    Secondary,
    /// D13: the same packet MD5 as the earlier stream `of`, the copy that is kept.
    Duplicate {
        /// The scan stream kept.
        of: u32,
    },
    /// TheDiscDB describes it as a commentary, an audio description, an isolated score or a music-and-effects track
    /// (8.1, D14), its track list aligning uniquely.
    Described {
        /// The first of those kinds the description names.
        kind: DescriptionKind,
    },
    /// A language that is not wanted (R4).
    UnwantedLanguage,
    /// A remembered decision drops it or marks it as commentary (spec 5).
    Remembered,
    /// The owner's Override tracks drops it or marks it as commentary (spec 11).
    Reviewed,
}

/// Which language group a kept track belongs to (8.6 step 5's order); the derived order is Original, English,
/// NoLanguage. Def: T8. Use: T11, T13.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LangGroup {
    /// The work's original language when it is not English (R4).
    Original,
    /// English (attribute 3 "eng" or "en").
    English,
    /// No language code, or a code that names none ("und"): kept and placed last (8.6 "Languages", step 5).
    NoLanguage,
}

impl LangGroup {
    /// Every group, in the derived order. Spec 8.6 step 5.
    pub const ALL: [LangGroup; 3] = [LangGroup::Original, LangGroup::English, LangGroup::NoLanguage];

    /// "original", "english" or "no_language", the string serde writes. Spec 8.6 step 5.
    pub fn as_str(self) -> &'static str {
        match self {
            LangGroup::Original => "original",
            LangGroup::English => "english",
            LangGroup::NoLanguage => "no_language",
        }
    }
}

impl FromStr for LangGroup {
    type Err = String;

    /// Only the `as_str` spellings; otherwise Err `"<text>" is not a language group (spec 8.6 step 5)`.
    fn from_str(text: &str) -> Result<LangGroup, String> {
        LangGroup::ALL
            .into_iter()
            .find(|group| group.as_str() == text)
            .ok_or_else(|| format!("\"{text}\" is not a language group (spec 8.6 step 5)"))
    }
}

/// A kept audio track with everything 8.6 and 8.10 need. Def: T8. Use: T11, T13, T16.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AudioTrack {
    /// The scan stream number.
    pub stream: u32,
    /// Attribute 3 lower-cased (None when absent).
    pub language: Option<String>,
    /// The language group (8.6 step 5).
    pub group: LangGroup,
    /// `codec::codec` of attributes 5, 6 and 7.
    pub codec: Codec,
    /// Attribute 14 (0 when absent).
    pub channels: u32,
    /// `codec::tier` of the family and the bit rate (8.6 step 3.2); None for an unknown codec.
    pub tier: Option<Tier>,
    /// From ffprobe's profile, else the scan's attribute 7.
    pub object: Option<ObjectAudio>,
    /// Attribute 18, the bit depth.
    pub sample_size: Option<u32>,
    /// Attribute 17, the sample rate.
    pub sample_rate: Option<u32>,
    /// `codec::bitrate_kbps` of attribute 13.
    pub bitrate_kbps: Option<u32>,
    /// Lowest disc position among the language's surviving streams (8.6 "Position": computed after step 1's drops).
    pub first_of_language: bool,
    /// 8.10: "<display> [Atmos|DTS:X] <layout>", for example "TrueHD Atmos 7.1".
    pub name: String,
    /// The default flag (8.6 step 5: the very first track).
    pub default: bool,
    /// The original flag (8.6 step 5: the original-language tracks; for an English work the English ones).
    pub original: bool,
}

/// Track confidence for audio (8.6: "0.95, because nothing is dropped on a guess"). Def: T8. Use: T13.
pub const AUDIO_CONFIDENCE: f64 = 0.95;
/// Guard G1's line (8.6 step 4): a lossy track at this rate or less never comes before a lossless one (a fixed value of
/// spec 14). Def: T8. Use: T8.
pub const G1_MAX_KBPS: u32 = 256;

/// How TheDiscDB's track list aligned with the scan's streams (8.6 step 1). Def: T8. Use: T8, T10, T16.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Alignment {
    /// Against the title's full stream list.
    Full,
    /// Against the list without derived streams (flag 2048).
    WithoutDerived,
}

impl Alignment {
    /// Both alignments, in the order step 1 tries them. Spec 8.6 step 1.
    pub const ALL: [Alignment; 2] = [Alignment::Full, Alignment::WithoutDerived];

    /// "full" or "without_derived", the string serde writes. Spec 8.6 step 1.
    pub fn as_str(self) -> &'static str {
        match self {
            Alignment::Full => "full",
            Alignment::WithoutDerived => "without_derived",
        }
    }
}

impl FromStr for Alignment {
    type Err = String;

    /// Only the `as_str` spellings; otherwise Err `"<text>" is not an alignment (spec 8.6 step 1)`.
    fn from_str(text: &str) -> Result<Alignment, String> {
        Alignment::ALL
            .into_iter()
            .find(|alignment| alignment.as_str() == text)
            .ok_or_else(|| format!("\"{text}\" is not an alignment (spec 8.6 step 1)"))
    }
}

/// The audio decision of one title (spec 8.6, 6.4 step 11). Def: T8. Use: T11, T13, T16.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AudioDecision {
    /// In final order (8.6 step 5), flags set.
    pub kept: Vec<AudioTrack>,
    /// Every dropped audio stream with its reason, in scan order.
    pub dropped: Vec<(u32, AudioDrop)>,
    /// The programme reference (8.6 "Content"): the first original-language track of the rule's order, else the first
    /// English one; None when no original-language or English track is kept (no-language tracks are never the
    /// reference).
    pub reference: Option<u32>,
    /// `facts.verdict_reference`, copied so the document shows what the verdicts were measured against.
    pub verdict_reference: Option<u32>,
    /// The notes of 8.6 step 6, D9 and RP7, in the order raised.
    pub notes: Vec<Noted>,
    /// Sorted and unique (`review::sorted_unique`).
    pub reviews: Vec<ReviewReason>,
    /// AUDIO_CONFIDENCE, or 1.00 when every audio stream has a remembered or review override.
    pub confidence: f64,
    /// How TheDiscDB's track list aligned; None when it did not align uniquely (descriptions unused) or there is no
    /// record.
    pub description_alignment: Option<Alignment>,
}

/// A TheDiscDB track and a scan stream agree on kind, language (`norm_lang` of both; video compares none), codec
/// (attribute 7 against the track's `name`) and, for audio, channels.
fn same_track(track: &DiscDbTrack, stream: &Stream) -> bool {
    if track.kind() != Some(stream.kind) {
        return false;
    }
    if stream.kind != StreamKind::Video
        && norm_lang(track.language_code.as_deref()) != norm_lang(stream.language.as_deref())
    {
        return false;
    }
    if track.name.as_deref().unwrap_or("") != stream.codec_long {
        return false;
    }
    stream.kind != StreamKind::Audio || track.channels() == stream.channels
}

/// The track list aligns with the stream list completely: equal lengths and every pair the same track.
fn aligns(tracks: &[&DiscDbTrack], streams: &[&Stream]) -> bool {
    tracks.len() == streams.len() && tracks.iter().zip(streams).all(|(track, stream)| same_track(track, stream))
}

/// Scan stream number to TheDiscDB track index, pair by pair.
fn pairs(tracks: &[&DiscDbTrack], streams: &[&Stream]) -> BTreeMap<u32, u32> {
    streams.iter().zip(tracks).map(|(stream, track)| (stream.number, track.index)).collect()
}

/// Spec 8.6 step 1: TheDiscDB's whole track list (every kind, index order) against the title's whole stream list (scan
/// order), pairwise on kind (`DiscDbTrack::kind`), language (`norm_lang` of `language_code` against `norm_lang` of
/// attribute 3; video compares no language), codec as attribute 7 (`Stream.codec_long == DiscDbTrack.name`) and
/// channels (`DiscDbTrack::channels` against attribute 14, audio only); tried on all streams and on the streams without
/// flag 2048; Some only when exactly one of the two aligns completely. A title without derived streams has one list,
/// tried once (`Full` when it aligns); an empty track list never aligns. The map is scan stream number to TheDiscDB
/// track index. Def: T8. Use: T8.
pub fn align_descriptions(title: &Title, discdb: &DiscDbTitle) -> Option<(Alignment, BTreeMap<u32, u32>)> {
    let mut tracks: Vec<&DiscDbTrack> = discdb.tracks.iter().collect();
    tracks.sort_by_key(|track| track.index);
    if tracks.is_empty() {
        return None;
    }
    let full: Vec<&Stream> = title.streams.iter().collect();
    let without: Vec<&Stream> = title.streams.iter().filter(|stream| !stream.flags.has(Flags::DERIVED)).collect();
    let full_aligns = aligns(&tracks, &full);
    if without.len() == full.len() {
        return full_aligns.then(|| (Alignment::Full, pairs(&tracks, &full)));
    }
    match (full_aligns, aligns(&tracks, &without)) {
        (true, false) => Some((Alignment::Full, pairs(&tracks, &full))),
        (false, true) => Some((Alignment::WithoutDerived, pairs(&tracks, &without))),
        _ => None,
    }
}

/// Every scan stream (any kind) whose aligned TheDiscDB track has a description that is not blank, to
/// `lookup::description_kinds(description, track.language)` (possibly empty); with the alignment used. Empty and None
/// without a record or without a unique alignment. Spec 8.6 step 1, 8.1 last bullet, D14. Def: T8. Use: T8, T9, T10.
pub fn descriptions(title: &Title, discdb: Option<&DiscDbTitle>) -> (Option<Alignment>, BTreeMap<u32, Vec<DescriptionKind>>) {
    let Some(record) = discdb else {
        return (None, BTreeMap::new());
    };
    let Some((alignment, paired)) = align_descriptions(title, record) else {
        return (None, BTreeMap::new());
    };
    let mut described = BTreeMap::new();
    for (stream, index) in paired {
        let Some(track) = record.tracks.iter().find(|track| track.index == index) else {
            continue;
        };
        if let Some(text) = track.description.as_deref().filter(|text| !text.trim().is_empty()) {
            described.insert(stream, description_kinds(text, track.language.as_deref()));
        }
    }
    (Some(alignment), described)
}

/// Spec 8.6 step 3's criteria of one track as sort components; a smaller value sorts first.
struct Criteria {
    /// 3.1: more channels first.
    channels: i64,
    /// 3.2: the tier's rank (`tier_rank`).
    tier: i64,
    /// 3.3: an object-based extension first.
    object: i64,
    /// 3.4: the deeper, then the faster sampled track first (unknown sorts last).
    sample_size: i64,
    sample_rate: i64,
    /// 3.5: the higher bit rate first, lossy tracks only (0 for every lossless track; 1, after every rate, for a
    /// lossy track without bit rate text).
    bitrate: i64,
    /// 3.6: the earlier disc position first.
    position: i64,
}

/// The criteria of `track`.
fn criteria(track: &AudioTrack) -> Criteria {
    Criteria {
        channels: -i64::from(track.channels),
        tier: tier_rank(track.tier),
        object: if track.object.is_some() { 0 } else { 1 },
        sample_size: -i64::from(track.sample_size.unwrap_or(0)),
        sample_rate: -i64::from(track.sample_rate.unwrap_or(0)),
        bitrate: if track.codec.lossless {
            0
        } else {
            match track.bitrate_kbps {
                Some(kbps) => -i64::from(kbps),
                None => 1,
            }
        },
        position: i64::from(track.stream),
    }
}

/// Lossless 0, high-rate lossy 1, other lossy 2, low 3; an unknown tier 4, after low (spec 8.6 step 3.2).
fn tier_rank(tier: Option<Tier>) -> i64 {
    match tier {
        Some(Tier::Lossless) => 0,
        Some(Tier::HighRateLossy) => 1,
        Some(Tier::OtherLossy) => 2,
        Some(Tier::Low) => 3,
        None => 4,
    }
}

/// 8.6 step 3's sort key (ascending): (channels descending, tier, object descending, sample size descending, sample rate
/// descending, bitrate descending for lossy only, disc position); with settings.prefer_most_channels off, tier comes before
/// channels. An unknown tier sorts after Low; a lossy track without bitrate text sorts after every lossy track with one.
/// Reads `prefer_most_channels` (D9). The product's copy beside audio::reference::rank (Global Constraints). Def: T8.
/// Use: T8.
pub fn rank_key(track: &AudioTrack, settings: &Settings) -> (i64, i64, i64, i64, i64, i64, i64) {
    let c = criteria(track);
    if settings.prefer_most_channels {
        (c.channels, c.tier, c.object, c.sample_size, c.sample_rate, c.bitrate, c.position)
    } else {
        (c.tier, c.channels, c.object, c.sample_size, c.sample_rate, c.bitrate, c.position)
    }
}

/// G1's weak track: lossy at G1_MAX_KBPS or less, or without a bit rate.
fn g1_weak(track: &AudioTrack) -> bool {
    !track.codec.lossless
        && match track.bitrate_kbps {
            Some(kbps) => kbps <= G1_MAX_KBPS,
            None => true,
        }
}

/// G1: every weak track ranked before the last lossless track moves to just after it; the order within the moved
/// tracks and within the others is kept.
fn g1(ranked: Vec<AudioTrack>) -> Vec<AudioTrack> {
    let Some(last_lossless) = ranked.iter().rposition(|track| track.codec.lossless) else {
        return ranked;
    };
    let mut front = Vec::new();
    let mut moved = Vec::new();
    let mut back = Vec::new();
    for (index, track) in ranked.into_iter().enumerate() {
        if index < last_lossless && g1_weak(&track) {
            moved.push(track);
        } else if index <= last_lossless {
            front.push(track);
        } else {
            back.push(track);
        }
    }
    front.extend(moved);
    front.extend(back);
    front
}

/// G2: a lossy top track with two channels or fewer that is not the first of its language and beats the first track
/// on channel count but on none of criteria 2 to 5 (tier, object, depth and rate, bit rate) gives way to it. Bit rate
/// counts only when the first track is lossy too: 3.5 ranks lossy tracks only, and `criteria` gives every lossless
/// track 0, which any lossy rate would beat. The top track is already known to be lossy.
fn g2(mut tracks: Vec<AudioTrack>) -> Vec<AudioTrack> {
    let applies = match tracks.first() {
        Some(top) => !top.codec.lossless && top.channels <= 2 && !top.first_of_language,
        None => false,
    };
    if !applies {
        return tracks;
    }
    let Some(first) = tracks.iter().position(|track| track.first_of_language) else {
        return tracks;
    };
    let (a, b) = (criteria(&tracks[0]), criteria(&tracks[first]));
    let later = a.tier < b.tier
        || a.object < b.object
        || a.sample_size < b.sample_size
        || a.sample_rate < b.sample_rate
        || (!tracks[first].codec.lossless && a.bitrate < b.bitrate);
    if a.channels < b.channels && !later {
        let track = tracks.remove(first);
        tracks.insert(0, track);
    }
    tracks
}

/// G1: a lossy track at G1_MAX_KBPS or less (or with no bitrate) never before a lossless track of the same language. G2: a
/// lossy top track with <= 2 channels that is not first of its language and beats the first on channel count but on no later
/// criterion gives way to the first. Returns the language's tracks reordered. `ranked` is one language's tracks in rank
/// order with `first_of_language` set; G1 applies before G2. The guards apply whatever the D9 setting says (spec 8.6 step
/// 4), so no setting is read. Def: T8. Use: T8.
pub fn guards(ranked: Vec<AudioTrack>, _settings: &Settings) -> Vec<AudioTrack> {
    g2(g1(ranked))
}

/// The flags of 8.6 step 1's second bullet: director's comments, alternate director's comments, visually impaired.
const COMMENTARY_FLAGS: u32 = Flags::DIRECTORS_COMMENTS | Flags::ALT_DIRECTORS_COMMENTS | Flags::VISUALLY_IMPAIRED;

/// The described kinds 8.6 step 1 drops (R6, D14); every other described track is kept.
const DROPPED_DESCRIPTIONS: [DescriptionKind; 4] = [
    DescriptionKind::Commentary,
    DescriptionKind::AudioDescription,
    DescriptionKind::IsolatedScore,
    DescriptionKind::MusicAndEffects,
];

/// Spec 8.6 steps 1 to 6 and "Content" (RP7), in order: drop (derived 2048; flags 1, 2, 4; secondary 512; duplicates by
/// equal MD5, one copy kept: `Duplicate { of }` names the first earlier stream with that MD5 that was kept; described
/// Commentary, AudioDescription, IsolatedScore or MusicAndEffects through `descriptions` when the alignment is unique;
/// unwanted language; a remembered or review Drop or Commentary); keep the rest and any no-language stream; rank with
/// `rank_key`; guards G1 and G2; order by language (original first when settings.original_language_first and the
/// original is not English, else English first; no language last), default on the very first track, original flag on
/// the original-language tracks (the English ones for an English work); notes possible_commentary,
/// lossless_original_available, no_english_audio, lossy_before_lossless (D9); reviews original_language_missing,
/// no_wanted_audio; verdicts apply only when `facts.verdict_reference` and the decision's `reference` are both Some (a
/// title without a kept original-language or English track has nothing to judge, spec 8.6 "Content"): when they differ
/// (spec 15: evidence measured against another reference than the rule's own choice), or `facts.veto_holds` is true
/// (D21: the rule confirms no commentary on the title), only audio_reference_unclear is raised and no verdict is mapped;
/// otherwise, for the kept streams in a wanted language (Original or English group; no-language streams are not judged,
/// spec 8.6 "Content"), ReferenceUnclear adds audio_reference_unclear, Commentary and CommentaryCrossLanguage add
/// commentary_detected (nothing dropped, D15), AdPattern, NoteCommentary and NoteAd add notes; a remembered or review
/// Keep { order, default } fixes the order and the default. Confidence 0.95, or
/// lookup::REMEMBERED_CONFIDENCE (1.00) when every audio stream of the title has a remembered or review override. At
/// planning time `facts` is empty and the ffprobe criterion is skipped (8.3). Reads the settings `prefer_most_channels`
/// and `original_language_first` (D9). Def: T8. Use: T13, T16.
pub fn decide_audio(title: &Title, wanted: &Wanted, facts: &AudioFacts<'_>, settings: &Settings) -> AudioDecision {
    let audio = title.streams_of(StreamKind::Audio);
    let (description_alignment, described) = descriptions(title, facts.discdb);
    let mut dropped = Vec::new();
    let mut kept_streams: Vec<&Stream> = Vec::new();
    let mut packets: Vec<(u32, String)> = Vec::new();
    let overrides = overrides_of(title, facts);
    for stream in &audio {
        let rule = rule_drop(stream, wanted, facts, &described, &packets);
        match settle(rule, overrides.get(&stream.number)) {
            Some(why) => dropped.push((stream.number, why)),
            None => {
                if let Some(md5) = facts.md5.get(&stream.number) {
                    packets.push((stream.number, md5.clone()));
                }
                kept_streams.push(*stream);
            }
        }
    }
    let tracks: Vec<AudioTrack> = kept_streams.iter().map(|stream| track_of(stream, wanted, facts)).collect();
    let ordered = rank_and_order(tracks, wanted, settings);
    let reference = reference_of(&ordered);
    let mut kept = finish(keep_orders(ordered, &overrides), wanted);
    override_defaults(&mut kept, &overrides);
    let mut notes = notes_of(&kept);
    let mut reviews = gaps(&kept, wanted);
    apply_verdicts(facts, reference, &kept, &mut notes, &mut reviews);
    AudioDecision {
        kept,
        dropped,
        reference,
        verdict_reference: facts.verdict_reference,
        notes,
        reviews: sorted_unique(reviews),
        confidence: confidence_of(&audio, &overrides),
        description_alignment,
    }
}

/// Step 1's reason to drop `stream`, or None: the bullets in the spec's order, the first that holds naming the reason.
/// `packets` holds the MD5 of every earlier stream that was kept (`decide_audio` adds a stream's MD5 only once the stream
/// is kept), so a duplicate names a kept copy and one copy always survives (D13).
fn rule_drop(
    stream: &Stream,
    wanted: &Wanted,
    facts: &AudioFacts<'_>,
    described: &BTreeMap<u32, Vec<DescriptionKind>>,
    packets: &[(u32, String)],
) -> Option<AudioDrop> {
    if stream.flags.has(Flags::DERIVED) {
        return Some(AudioDrop::Derived);
    }
    if stream.flags.commentary() {
        return Some(AudioDrop::Flagged { flags: stream.flags.0 & COMMENTARY_FLAGS });
    }
    if stream.flags.has(Flags::SECONDARY_AUDIO) {
        return Some(AudioDrop::Secondary);
    }
    if let Some(md5) = facts.md5.get(&stream.number) {
        if let Some((of, _)) = packets.iter().find(|(_, seen)| seen == md5) {
            return Some(AudioDrop::Duplicate { of: *of });
        }
    }
    let kind = described
        .get(&stream.number)
        .and_then(|kinds| kinds.iter().copied().find(|kind| DROPPED_DESCRIPTIONS.contains(kind)));
    if let Some(kind) = kind {
        return Some(AudioDrop::Described { kind });
    }
    match stream.language.as_deref() {
        Some(code) if !is_no_language(Some(code)) && !wanted.contains(code) => Some(AudioDrop::UnwantedLanguage),
        _ => None,
    }
}

/// The language group of a kept stream's attribute 3 (8.6 "Languages"): the original language's codes, English, or no
/// language (no code, a code naming none, or an unwanted code kept by an override).
fn group_of(language: Option<&str>, wanted: &Wanted) -> LangGroup {
    match language {
        Some(code) if !is_no_language(Some(code)) && wanted.is_original(code) => LangGroup::Original,
        Some(code) if !is_no_language(Some(code)) && is_english(code) => LangGroup::English,
        _ => LangGroup::NoLanguage,
    }
}

/// A kept stream as a track: its facts, its object from ffprobe's profile only (the ranking's 3.3), the flags and the
/// name filled by `finish`.
fn track_of(stream: &Stream, wanted: &Wanted, facts: &AudioFacts<'_>) -> AudioTrack {
    AudioTrack {
        stream: stream.number,
        language: stream.language.clone(),
        group: group_of(stream.language.as_deref(), wanted),
        codec: stream.codec.clone(),
        channels: stream.channels.unwrap_or(0),
        tier: tier(stream.codec.family, stream.bitrate_kbps),
        object: object_audio(facts.ffprobe_profile.get(&stream.number).map(String::as_str)),
        sample_size: stream.sample_size,
        sample_rate: stream.sample_rate,
        bitrate_kbps: stream.bitrate_kbps,
        first_of_language: false,
        name: String::new(),
        default: false,
        original: false,
    }
}

/// Spec 8.6 step 5's language order: the original language first when `original_language_first` is on and the work is
/// not English, else English first; no language last.
fn group_order(wanted: &Wanted, settings: &Settings) -> [LangGroup; 3] {
    if settings.original_language_first && !wanted.original_is_english {
        [LangGroup::Original, LangGroup::English, LangGroup::NoLanguage]
    } else {
        [LangGroup::English, LangGroup::Original, LangGroup::NoLanguage]
    }
}

/// Steps 3 to 5: per language in step 5's order, "first of its language" set (8.6 "Position"), ranked with `rank_key`,
/// then guarded.
fn rank_and_order(tracks: Vec<AudioTrack>, wanted: &Wanted, settings: &Settings) -> Vec<AudioTrack> {
    let mut ordered = Vec::with_capacity(tracks.len());
    for group in group_order(wanted, settings) {
        let mut members: Vec<AudioTrack> = tracks.iter().filter(|track| track.group == group).cloned().collect();
        let first = members.iter().map(|track| track.stream).min();
        for track in &mut members {
            track.first_of_language = Some(track.stream) == first;
        }
        members.sort_by_key(|track| rank_key(track, settings));
        ordered.extend(guards(members, settings));
    }
    ordered
}

/// The programme reference (8.6 "Content"): the first original-language track of the rule's order, else the first
/// English one; None when neither is kept.
fn reference_of(ordered: &[AudioTrack]) -> Option<u32> {
    ordered
        .iter()
        .find(|track| track.group == LangGroup::Original)
        .or_else(|| ordered.iter().find(|track| track.group == LangGroup::English))
        .map(|track| track.stream)
}

/// 8.10's audio name: the display name, the object-based extension, the channel layout (left out when the count is
/// unknown).
fn track_name(codec: &Codec, object: Option<ObjectAudio>, channels: u32) -> String {
    let mut name = codec.display.clone();
    if let Some(object) = object {
        name.push(' ');
        name.push_str(object.as_str());
    }
    if channels > 0 {
        name.push(' ');
        name.push_str(&channel_layout(channels));
    }
    name
}

/// Step 5's flags and 8.10's names on the final order: attribute 7's object for a track whose ffprobe profile names
/// none (no profile, or one without a known extension: the spellings are unverified, EXP-06), the name, the default flag
/// on the very first track, and the original flag on the original-language tracks, or on the English ones for an
/// English work (TMDb's code "en"). A work whose original language is unknown, "xx" or missing from the ISO 639 table
/// is not an English work and has no Original group, so none of its tracks gets the original flag (8.6 step 5).
fn finish(mut ordered: Vec<AudioTrack>, wanted: &Wanted) -> Vec<AudioTrack> {
    let english_work = wanted.original_two_letter.as_deref().is_some_and(is_english);
    for (position, track) in ordered.iter_mut().enumerate() {
        track.object = track.object.or(track.codec.object);
        track.name = track_name(&track.codec, track.object, track.channels);
        track.default = position == 0;
        track.original = if english_work {
            track.group == LangGroup::English
        } else {
            track.group == LangGroup::Original
        };
    }
    ordered
}

/// The language a note names: attribute 3, or "no-language".
fn language_word(track: &AudioTrack) -> &str {
    track.language.as_deref().filter(|code| !is_no_language(Some(*code))).unwrap_or("no-language")
}

/// The kept tracks of one group, in final order.
fn group_members(kept: &[AudioTrack], group: LangGroup) -> Vec<&AudioTrack> {
    kept.iter().filter(|track| track.group == group).collect()
}

/// Step 6's notes and D9's note on the final order, in the spec's bullet order.
fn notes_of(kept: &[AudioTrack]) -> Vec<Noted> {
    let mut notes = Vec::new();
    for track in kept {
        if !track.codec.lossless && track.channels <= 2 && !track.first_of_language {
            notes.push(Noted {
                note: Note::PossibleCommentary,
                stream: Some(track.stream),
                detail: format!(
                    "stream {} ({}) is lossy with two channels or fewer and is not the first {} stream on the disc",
                    track.stream,
                    track.name,
                    language_word(track)
                ),
            });
        }
    }
    let mut groups: Vec<LangGroup> = Vec::new();
    for track in kept {
        if !groups.contains(&track.group) {
            groups.push(track.group);
        }
    }
    for &group in &groups {
        let members = group_members(kept, group);
        let top = members[0];
        if top.codec.lossless {
            continue;
        }
        if let Some(lossless) = members.iter().find(|track| track.codec.lossless && track.channels < top.channels) {
            notes.push(Noted {
                note: Note::LosslessOriginalAvailable,
                stream: None,
                detail: format!(
                    "the first {} track, stream {} ({}), is lossy while stream {} ({}) is lossless with fewer channels",
                    language_word(top),
                    top.stream,
                    top.name,
                    lossless.stream,
                    lossless.name
                ),
            });
        }
    }
    let english = kept.iter().any(|track| track.group == LangGroup::English);
    let original = kept.iter().any(|track| track.group == LangGroup::Original);
    if !english && original {
        notes.push(Noted {
            note: Note::NoEnglishAudio,
            stream: None,
            detail: "no English audio stream is kept; the original-language audio is".to_string(),
        });
    }
    for &group in &groups {
        let members = group_members(kept, group);
        for (index, track) in members.iter().enumerate() {
            if track.codec.lossless {
                continue;
            }
            let later = members[index + 1..].iter().find(|other| other.codec.lossless && other.channels < track.channels);
            if let Some(lossless) = later {
                notes.push(Noted {
                    note: Note::LossyBeforeLossless,
                    stream: Some(track.stream),
                    detail: format!(
                        "stream {} ({}) is lossy and placed before the lossless stream {} ({}), which has fewer channels (D9)",
                        track.stream, track.name, lossless.stream, lossless.name
                    ),
                });
                break;
            }
        }
    }
    notes
}

/// Step 6's gaps, judged on the kept tracks: original_language_missing when the original language is wanted and no
/// kept track is in it; no_wanted_audio when nothing is kept.
fn gaps(kept: &[AudioTrack], wanted: &Wanted) -> Vec<ReviewReason> {
    let mut reviews = Vec::new();
    if !wanted.original_is_english && !kept.iter().any(|track| track.group == LangGroup::Original) {
        reviews.push(ReviewReason::OriginalLanguageMissing);
    }
    if kept.is_empty() {
        reviews.push(ReviewReason::NoWantedAudio);
    }
    reviews
}

/// Adds `note` for `stream` unless a note of the same kind already names that stream.
fn push_note(notes: &mut Vec<Noted>, note: Note, stream: u32, detail: String) {
    if !notes.iter().any(|noted| noted.note == note && noted.stream == Some(stream)) {
        notes.push(Noted { note, stream: Some(stream), detail });
    }
}

/// Spec 8.6 "Content" with RP7's mapping. Nothing without a measured reference. Nothing when the decision has no
/// reference (no wanted-language track: nothing to judge, spec 8.6 Content). Verdicts measured against another stream
/// than the decision's `reference` are not applied and raise audio_reference_unclear (spec 15). When D21's veto holds the
/// rule confirms no commentary on the title: audio_reference_unclear and nothing else. Otherwise, for the kept streams
/// in a wanted language (Original or English group; no-language streams are not judged, spec 8.6 Content):
/// reference-unclear raises audio_reference_unclear; commentary and commentary-cross-language raise
/// commentary_detected and drop nothing (D15); ad-pattern and note-ad note possible_audio_description; note-commentary
/// notes possible_commentary; duplicate, keep and not-judged add nothing (D13's drop is by MD5).
fn apply_verdicts(
    facts: &AudioFacts<'_>,
    reference: Option<u32>,
    kept: &[AudioTrack],
    notes: &mut Vec<Noted>,
    reviews: &mut Vec<ReviewReason>,
) {
    let Some(measured) = facts.verdict_reference else {
        return;
    };
    let Some(reference) = reference else {
        return;
    };
    if reference != measured || facts.veto_holds {
        reviews.push(ReviewReason::AudioReferenceUnclear);
        return;
    }
    for (&stream, &verdict) in &facts.verdicts {
        if !kept.iter().any(|track| track.stream == stream && track.group != LangGroup::NoLanguage) {
            continue;
        }
        match verdict {
            Verdict::ReferenceUnclear => reviews.push(ReviewReason::AudioReferenceUnclear),
            Verdict::Commentary | Verdict::CommentaryCrossLanguage => reviews.push(ReviewReason::CommentaryDetected),
            Verdict::AdPattern => push_note(
                notes,
                Note::PossibleAudioDescription,
                stream,
                format!("the content rule found the audio-description pattern on stream {stream} (ad-pattern)"),
            ),
            Verdict::NoteAd => push_note(
                notes,
                Note::PossibleAudioDescription,
                stream,
                format!("the content rule notes a possible audio description on stream {stream} (note-ad)"),
            ),
            Verdict::NoteCommentary => push_note(
                notes,
                Note::PossibleCommentary,
                stream,
                format!("the content rule notes a possible commentary on stream {stream} (note-commentary)"),
            ),
            Verdict::Duplicate | Verdict::Keep | Verdict::NotJudged => {}
        }
    }
}

/// Where an audio override came from: a remembered decision (spec 5) or the owner's Override tracks (spec 11).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Remembered,
    Reviewed,
}

/// Scan stream number to override for the title's audio streams: the remembered ones through
/// `remembered::apply_audio`, then the owner's, which win; an override naming a stream that is not audio is ignored.
fn overrides_of(title: &Title, facts: &AudioFacts<'_>) -> BTreeMap<u32, (AudioOverride, Source)> {
    let mut overrides = BTreeMap::new();
    if let Some(entries) = facts.remembered {
        for (stream, action) in apply_audio(entries, &title.streams) {
            overrides.insert(stream, (action.clone(), Source::Remembered));
        }
    }
    if let Some(reviewed) = facts.reviewed {
        let audio = title.streams_of(StreamKind::Audio);
        for (stream, action) in reviewed {
            if audio.iter().any(|candidate| candidate.number == *stream) {
                overrides.insert(*stream, (action.clone(), Source::Reviewed));
            }
        }
    }
    overrides
}

/// Step 1's outcome with the override: a Keep keeps the stream whatever step 1 says; otherwise step 1's reason stands;
/// a Drop or Commentary drops a stream step 1 keeps, labelled by where the override came from.
fn settle(rule: Option<AudioDrop>, action: Option<&(AudioOverride, Source)>) -> Option<AudioDrop> {
    match (rule, action) {
        (_, Some((AudioOverride::Keep { .. }, _))) => None,
        (Some(why), _) => Some(why),
        (None, Some((AudioOverride::Drop | AudioOverride::Commentary, Source::Remembered))) => Some(AudioDrop::Remembered),
        (None, Some((AudioOverride::Drop | AudioOverride::Commentary, Source::Reviewed))) => Some(AudioDrop::Reviewed),
        (None, None) => None,
    }
}

/// Spec 11's Keep orders (0 first): the overridden tracks in (order, rule position) order each take the first free
/// position at or after their order; the other tracks fill the remaining positions in the rule's order.
fn keep_orders(ordered: Vec<AudioTrack>, overrides: &BTreeMap<u32, (AudioOverride, Source)>) -> Vec<AudioTrack> {
    let total = ordered.len();
    let mut pinned: Vec<(u32, usize, AudioTrack)> = Vec::new();
    let mut free: Vec<AudioTrack> = Vec::new();
    for (index, track) in ordered.into_iter().enumerate() {
        match overrides.get(&track.stream) {
            Some((AudioOverride::Keep { order, .. }, _)) => pinned.push((*order, index, track)),
            _ => free.push(track),
        }
    }
    pinned.sort_by_key(|(order, index, _)| (*order, *index));
    let mut pinned = pinned.into_iter().peekable();
    let mut free = free.into_iter();
    let mut out = Vec::with_capacity(total);
    while out.len() < total {
        let position = u32::try_from(out.len()).unwrap_or(u32::MAX);
        let due = pinned.peek().is_some_and(|(order, _, _)| *order <= position);
        let next = if due {
            pinned.next().map(|(_, _, track)| track)
        } else {
            free.next().or_else(|| pinned.next().map(|(_, _, track)| track))
        };
        match next {
            Some(track) => out.push(track),
            None => break,
        }
    }
    out
}

/// Spec 11's Keep default: when an override says `default: true`, the first such track in final order is the default
/// and no other; otherwise step 5's default (the very first track) stands.
fn override_defaults(kept: &mut [AudioTrack], overrides: &BTreeMap<u32, (AudioOverride, Source)>) {
    let chosen = kept
        .iter()
        .map(|track| track.stream)
        .find(|stream| matches!(overrides.get(stream), Some((AudioOverride::Keep { default: true, .. }, _))));
    if let Some(chosen) = chosen {
        for track in kept.iter_mut() {
            track.default = track.stream == chosen;
        }
    }
}

/// Spec 8.6's 0.95, or 1.00 (8.8: a review decision sets the confidence it covers) when the title has audio and every
/// audio stream has a remembered or review override.
fn confidence_of(audio: &[&Stream], overrides: &BTreeMap<u32, (AudioOverride, Source)>) -> f64 {
    if !audio.is_empty() && audio.iter().all(|stream| overrides.contains_key(&stream.number)) {
        REMEMBERED_CONFIDENCE
    } else {
        AUDIO_CONFIDENCE
    }
}

#[cfg(test)]
mod tests;
