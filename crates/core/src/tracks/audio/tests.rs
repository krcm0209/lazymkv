//! Tests of core::tracks::audio (spec 8.6 steps 1 to 6 and "Content", D9, D13, D15, D21, RP7). The pinned logs and
//! TheDiscDB records of Knowing (DVD) and Smile (UHD) are read from corpus/thediscdb; every other title is built
//! with testsupport::scan_text. The Office's twin tracks are a synthetic copy of the layout of its title 1 (S03E15)
//! with made-up packet MD5s.

use std::fs;
use std::path::Path;

use serde_json::json;

use super::*;
use crate::codec::{codec, tier};
use crate::disc::{disc, Disc};
use crate::lang::wanted_languages;
use crate::lookup::discdb::{parse_dischash, DiscDbRecord};
use crate::remembered::signature;
use crate::robot::parse;
use crate::testsupport::scan_text;

/// The text of a file under the repository's `corpus` folder.
fn corpus(path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus").join(path);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// Knowing's pinned DVD: `disc01.txt` (the scan) and `disc01.json` (TheDiscDB's record).
const KNOWING: &str = "thediscdb/movie/Knowing (2009)/2010-dvd/disc01";
/// Smile's pinned UHD.
const SMILE: &str = "thediscdb/movie/Smile (2022)/2022-4k/disc01";

/// The typed disc of a scan's text.
fn pinned_disc(text: &str) -> Disc {
    disc(&parse(text)).unwrap()
}

/// A pinned TheDiscDB record (an object with a non-empty `Titles` is a hit).
fn pinned_record(stem: &str) -> DiscDbRecord {
    let shown = format!("{stem}.json");
    parse_dischash(&corpus(&shown), &shown).unwrap().unwrap()
}

/// TheDiscDB's title `index` of a record.
fn db_title(record: &DiscDbRecord, index: u32) -> DiscDbTitle {
    record.titles.iter().find(|title| title.index == index).unwrap().clone()
}

/// One stream's attributes as (attribute id, value) pairs; a later pair overrides an earlier one with the same id,
/// as the scan's last line wins.
type Attrs = Vec<(u32, &'static str)>;

/// A Blu-ray video stream: AVC 1080p.
fn video() -> Attrs {
    vec![(1, "Video"), (5, "V_MPEG4/ISO/AVC"), (6, "Mpeg4"), (7, "Mpeg4 AVC High@L4.1"), (19, "1920x1080")]
}

/// An audio stream: language (attribute 3, left out when ""), codec attributes 5, 6 and 7, channels (14), bit rate
/// text (13, left out when "") and a sample rate of 48000 (17).
fn audio(
    language: &'static str,
    id: &'static str,
    short: &'static str,
    long: &'static str,
    channels: &'static str,
    bitrate: &'static str,
) -> Attrs {
    let mut attrs = vec![(1, "Audio")];
    if !language.is_empty() {
        attrs.push((3, language));
    }
    attrs.extend([(5, id), (6, short), (7, long), (14, channels), (17, "48000")]);
    if !bitrate.is_empty() {
        attrs.push((13, bitrate));
    }
    attrs
}

/// A Dolby Digital (AC-3) stream.
fn dd(language: &'static str, channels: &'static str, bitrate: &'static str) -> Attrs {
    audio(language, "A_AC3", "DD", "Dolby Digital", channels, bitrate)
}

/// Title 0 of a one-title Blu-ray scan (1:40:00, `00800.mpls`, segment map `1`) whose streams are numbered from 0 in
/// the order given.
fn title_of(streams: &[Attrs]) -> Title {
    let numbered: Vec<(u32, &[(u32, &str)])> =
        streams.iter().enumerate().map(|(number, attrs)| (number as u32, attrs.as_slice())).collect();
    let attrs: &[(u32, &str)] = &[(9, "1:40:00"), (16, "00800.mpls"), (26, "1")];
    let text = scan_text(&[(1, "Blu-ray disc")], &[(0, attrs, numbered.as_slice())]);
    disc(&parse(&text)).unwrap().titles[&0].clone()
}

#[test]
fn the_audio_confidence_is_0_95_and_g1s_line_is_256_kbps() {
    assert_eq!(AUDIO_CONFIDENCE, 0.95);
    assert_eq!(G1_MAX_KBPS, 256);
}

#[test]
fn the_language_groups_and_alignments_spell_as_serde_writes_them() {
    for (group, text) in
        [(LangGroup::Original, "original"), (LangGroup::English, "english"), (LangGroup::NoLanguage, "no_language")]
    {
        assert_eq!(group.as_str(), text);
        assert_eq!(serde_json::to_value(group).unwrap(), json!(text));
        assert_eq!(serde_json::from_value::<LangGroup>(json!(text)).unwrap(), group);
        assert_eq!(text.parse::<LangGroup>(), Ok(group));
    }
    assert_eq!(LangGroup::ALL, [LangGroup::Original, LangGroup::English, LangGroup::NoLanguage]);
    assert_eq!(
        "English".parse::<LangGroup>(),
        Err("\"English\" is not a language group (spec 8.6 step 5)".to_string())
    );
    for (alignment, text) in [(Alignment::Full, "full"), (Alignment::WithoutDerived, "without_derived")] {
        assert_eq!(alignment.as_str(), text);
        assert_eq!(serde_json::to_value(alignment).unwrap(), json!(text));
        assert_eq!(text.parse::<Alignment>(), Ok(alignment));
    }
    assert_eq!(Alignment::ALL, [Alignment::Full, Alignment::WithoutDerived]);
    assert_eq!("partial".parse::<Alignment>(), Err("\"partial\" is not an alignment (spec 8.6 step 1)".to_string()));
}

#[test]
fn a_drop_reason_is_written_with_its_why_tag() {
    let cases = [
        (AudioDrop::Derived, json!({"why": "derived"})),
        (AudioDrop::Flagged { flags: 1 }, json!({"why": "flagged", "flags": 1})),
        (AudioDrop::Secondary, json!({"why": "secondary"})),
        (AudioDrop::Duplicate { of: 1 }, json!({"why": "duplicate", "of": 1})),
        (AudioDrop::Described { kind: DescriptionKind::Commentary }, json!({"why": "described", "kind": "commentary"})),
        (AudioDrop::UnwantedLanguage, json!({"why": "unwanted_language"})),
        (AudioDrop::Remembered, json!({"why": "remembered"})),
        (AudioDrop::Reviewed, json!({"why": "reviewed"})),
    ];
    for (reason, value) in cases {
        assert_eq!(serde_json::to_value(&reason).unwrap(), value);
        assert_eq!(serde_json::from_value::<AudioDrop>(value).unwrap(), reason);
    }
}

#[test]
fn knowings_feature_aligns_with_its_record_as_full_since_it_has_no_derived_stream() {
    let disc = pinned_disc(&corpus(&format!("{KNOWING}.txt")));
    let title = &disc.titles[&0];
    let record = db_title(&pinned_record(KNOWING), 0);
    assert_eq!(title.streams.len(), 8);
    assert!(title.streams.iter().all(|stream| !stream.flags.has(Flags::DERIVED)));
    let identity: BTreeMap<u32, u32> = (0..8).map(|number| (number, number)).collect();
    assert_eq!(align_descriptions(title, &record), Some((Alignment::Full, identity)));
}

#[test]
fn knowings_descriptions_name_its_commentary_and_two_bare_language_dialogue_tracks() {
    let disc = pinned_disc(&corpus(&format!("{KNOWING}.txt")));
    let record = db_title(&pinned_record(KNOWING), 0);
    assert_eq!(
        descriptions(&disc.titles[&0], Some(&record)),
        (
            Some(Alignment::Full),
            BTreeMap::from([
                (1, vec![DescriptionKind::Dialogue]),
                (2, vec![DescriptionKind::Dialogue]),
                (3, vec![DescriptionKind::Commentary]),
            ])
        )
    );
}

#[test]
fn smiles_feature_aligns_on_the_full_list_with_its_derived_core_and_forced_only_streams() {
    let disc = pinned_disc(&corpus(&format!("{SMILE}.txt")));
    let title = &disc.titles[&14];
    let record = db_title(&pinned_record(SMILE), 14);
    assert_eq!(title.source_file.as_deref(), Some("00040.mpls"));
    assert_eq!(title.streams.len(), 47);
    assert_eq!(title.streams.iter().filter(|stream| stream.flags.has(Flags::DERIVED)).count(), 19);
    let identity: BTreeMap<u32, u32> = (0..47).map(|number| (number, number)).collect();
    assert_eq!(align_descriptions(title, &record), Some((Alignment::Full, identity)));
    assert_eq!(
        descriptions(title, Some(&record)),
        (Some(Alignment::Full), BTreeMap::from([(10, vec![DescriptionKind::Commentary])]))
    );
}

#[test]
fn a_record_without_the_derived_streams_aligns_without_derived() {
    let disc = pinned_disc(&corpus(&format!("{SMILE}.txt")));
    let title = &disc.titles[&14];
    let mut record = db_title(&pinned_record(SMILE), 14);
    let derived: Vec<u32> =
        title.streams.iter().filter(|stream| stream.flags.has(Flags::DERIVED)).map(|stream| stream.number).collect();
    record.tracks.retain(|track| !derived.contains(&track.index));
    assert_eq!(record.tracks.len(), 28);
    let (alignment, map) = align_descriptions(title, &record).unwrap();
    assert_eq!(alignment, Alignment::WithoutDerived);
    assert_eq!(map.len(), 28);
    assert_eq!(map.get(&1), Some(&1));
    assert_eq!(map.get(&2), None);
    assert_eq!(map.get(&3), Some(&3));
    assert_eq!(map.get(&10), Some(&10));
    assert_eq!(map.get(&12), None);
    assert_eq!(map.get(&45), Some(&45));
    let expected: BTreeMap<u32, u32> = title
        .streams
        .iter()
        .filter(|stream| !stream.flags.has(Flags::DERIVED))
        .map(|stream| (stream.number, stream.number))
        .collect();
    assert_eq!(map, expected);
    assert_eq!(
        descriptions(title, Some(&record)),
        (Some(Alignment::WithoutDerived), BTreeMap::from([(10, vec![DescriptionKind::Commentary])]))
    );
}

#[test]
fn a_record_of_another_length_or_with_one_track_different_aligns_neither_way() {
    let disc = pinned_disc(&corpus(&format!("{KNOWING}.txt")));
    let feature = &disc.titles[&0];
    let record = db_title(&pinned_record(KNOWING), 0);
    // Title 3 (a featurette) has 4 streams against the record's 8 tracks.
    assert_eq!(disc.titles[&3].streams.len(), 4);
    assert_eq!(align_descriptions(&disc.titles[&3], &record), None);
    assert_eq!(descriptions(&disc.titles[&3], Some(&record)), (None, BTreeMap::new()));
    // The Spanish 5.1 track (index 2) named stereo: the channels differ.
    let mut stereo = record.clone();
    stereo.tracks.iter_mut().find(|track| track.index == 2).unwrap().audio_type = Some("Stereo".to_string());
    assert_eq!(align_descriptions(feature, &stereo), None);
    // The English 5.1 track (index 1) tagged French: the languages differ.
    let mut french = record.clone();
    french.tracks.iter_mut().find(|track| track.index == 1).unwrap().language_code = Some("fra".to_string());
    assert_eq!(align_descriptions(feature, &french), None);
    // The video track (index 0) named another codec: attribute 7 differs.
    let mut codec_name = record.clone();
    codec_name.tracks.iter_mut().find(|track| track.index == 0).unwrap().name = Some("Mpeg4".to_string());
    assert_eq!(align_descriptions(feature, &codec_name), None);
    assert_eq!(descriptions(feature, Some(&codec_name)), (None, BTreeMap::new()));
}

#[test]
fn a_bibliographic_language_code_aligns_with_its_terminology_code_and_video_compares_no_language() {
    let title = title_of(&[video(), dd("deu", "6", "640 Kb/s")]);
    let record: DiscDbTitle = serde_json::from_value(json!({
        "index": 0,
        "tracks": [
            {"index": 0, "type": "Video", "name": "Mpeg4 AVC High@L4.1", "language_code": "eng"},
            {"index": 1, "type": "Audio", "name": "Dolby Digital", "audio_type": "Surround 5.1",
             "language_code": "ger", "language": "German", "description": "German"}
        ]
    }))
    .unwrap();
    assert_eq!(align_descriptions(&title, &record), Some((Alignment::Full, BTreeMap::from([(0, 0), (1, 1)]))));
    assert_eq!(
        descriptions(&title, Some(&record)),
        (Some(Alignment::Full), BTreeMap::from([(1, vec![DescriptionKind::Dialogue])]))
    );
}

#[test]
fn without_a_record_or_with_an_empty_track_list_nothing_is_described() {
    let disc = pinned_disc(&corpus(&format!("{KNOWING}.txt")));
    let feature = &disc.titles[&0];
    assert_eq!(descriptions(feature, None), (None, BTreeMap::new()));
    let mut empty = db_title(&pinned_record(KNOWING), 0);
    empty.tracks.clear();
    assert_eq!(align_descriptions(feature, &empty), None);
    assert_eq!(descriptions(feature, Some(&empty)), (None, BTreeMap::new()));
}

/// MakeMKV's attributes 5, 6 and 7 of the codecs the rank and guard tests use.
const AC3: (&str, &str, &str) = ("A_AC3", "DD", "Dolby Digital");
const TRUEHD: (&str, &str, &str) = ("A_TRUEHD", "TrueHD", "TrueHD");
const LPCM: (&str, &str, &str) = ("A_LPCM", "LPCM", "LPCM");
const AAC: (&str, &str, &str) = ("A_AAC", "AAC", "AAC");
/// A codec the table does not know (Family::Unknown, no tier).
const OPUS: (&str, &str, &str) = ("A_OPUS", "Opus", "Opus");

/// A kept English track at 48000 Hz for the rank and guard tests, its tier from the codec and the bit rate.
fn ranked(
    stream: u32,
    attrs: (&str, &str, &str),
    channels: u32,
    bitrate_kbps: Option<u32>,
    sample_size: Option<u32>,
    first_of_language: bool,
) -> AudioTrack {
    let codec = codec(attrs.0, attrs.1, attrs.2);
    let tier_of = tier(codec.family, bitrate_kbps);
    AudioTrack {
        stream,
        language: Some("eng".to_string()),
        group: LangGroup::English,
        codec,
        channels,
        tier: tier_of,
        object: None,
        sample_size,
        sample_rate: Some(48000),
        bitrate_kbps,
        first_of_language,
        name: String::new(),
        default: false,
        original: true,
    }
}

/// The stream numbers of tracks, in order.
fn streams_of(tracks: &[AudioTrack]) -> Vec<u32> {
    tracks.iter().map(|track| track.stream).collect()
}

#[test]
fn the_rank_key_orders_channels_tier_object_depth_rate_lossy_bitrate_then_position() {
    let settings = Settings::default();
    let ac3 = ranked(3, AC3, 6, Some(640), None, false);
    assert_eq!(rank_key(&ac3, &settings), (-6, 2, 1, 0, -48000, -640, 3));
    let mut atmos = ranked(1, TRUEHD, 8, Some(128), Some(24), true);
    atmos.object = Some(ObjectAudio::Atmos);
    assert_eq!(rank_key(&atmos, &settings), (-8, 0, 0, -24, -48000, 0, 1));
    assert!(rank_key(&atmos, &settings) < rank_key(&ac3, &settings));
}

#[test]
fn with_prefer_most_channels_off_tier_comes_before_channels() {
    let settings = Settings { prefer_most_channels: false, ..Settings::default() };
    let ac3 = ranked(3, AC3, 6, Some(640), None, false);
    assert_eq!(rank_key(&ac3, &settings), (2, -6, 1, 0, -48000, -640, 3));
    let mut atmos = ranked(1, TRUEHD, 8, Some(128), Some(24), true);
    atmos.object = Some(ObjectAudio::Atmos);
    assert_eq!(rank_key(&atmos, &settings), (0, -8, 0, -24, -48000, 0, 1));
    // A lossless stereo track beats a lossy 5.1 only with the setting off.
    let lpcm = ranked(1, LPCM, 2, None, Some(24), true);
    assert!(rank_key(&lpcm, &settings) < rank_key(&ac3, &settings));
    assert!(rank_key(&ac3, &Settings::default()) < rank_key(&lpcm, &Settings::default()));
}

#[test]
fn an_unknown_tier_sorts_after_low_and_a_lossy_track_without_bitrate_after_every_lossy_track_with_one() {
    let settings = Settings::default();
    let aac = ranked(1, AAC, 2, Some(128), None, true);
    let opus = ranked(2, OPUS, 2, Some(128), None, false);
    let no_rate = ranked(3, AC3, 2, None, None, false);
    let low_rate = ranked(4, AC3, 2, Some(32), None, false);
    assert_eq!(aac.tier, Some(Tier::Low));
    assert_eq!(opus.tier, None);
    assert_eq!(rank_key(&aac, &settings), (-2, 3, 1, 0, -48000, -128, 1));
    assert_eq!(rank_key(&opus, &settings), (-2, 4, 1, 0, -48000, -128, 2));
    assert_eq!(rank_key(&no_rate, &settings), (-2, 2, 1, 0, -48000, 1, 3));
    assert!(rank_key(&aac, &settings) < rank_key(&opus, &settings));
    assert!(rank_key(&low_rate, &settings) < rank_key(&no_rate, &settings));
}

#[test]
fn g1_puts_a_lossless_track_before_a_lossy_one_at_256_kbps_or_less() {
    let lossy = ranked(2, AC3, 6, Some(256), None, false);
    let lossless = ranked(1, LPCM, 2, None, Some(24), true);
    assert_eq!(streams_of(&guards(vec![lossy, lossless], &Settings::default())), vec![1, 2]);
}

#[test]
fn g1_puts_a_lossless_track_before_a_lossy_one_without_bitrate() {
    let lossy = ranked(2, AC3, 6, None, None, false);
    let lossless = ranked(1, LPCM, 2, None, Some(24), true);
    assert_eq!(streams_of(&guards(vec![lossy, lossless], &Settings::default())), vec![1, 2]);
}

#[test]
fn g1_leaves_a_lossy_track_above_256_kbps_before_a_lossless_one() {
    let lossy = ranked(2, AC3, 6, Some(257), None, false);
    let lossless = ranked(1, LPCM, 2, None, Some(24), true);
    assert_eq!(streams_of(&guards(vec![lossy, lossless], &Settings::default())), vec![2, 1]);
}

#[test]
fn g1_moves_every_weak_track_after_the_last_lossless_one_and_keeps_the_rest_in_order() {
    let ranked_tracks = vec![
        ranked(2, AC3, 6, Some(192), None, false),
        ranked(3, AC3, 6, Some(640), None, false),
        ranked(1, LPCM, 2, None, Some(24), true),
        ranked(4, LPCM, 2, None, Some(16), false),
        ranked(5, AC3, 2, Some(192), None, false),
    ];
    assert_eq!(streams_of(&guards(ranked_tracks, &Settings::default())), vec![3, 1, 4, 2, 5]);
}

#[test]
fn g2_places_the_first_track_first_when_the_top_lossy_stereo_track_beats_it_on_channels_only() {
    let stereo = ranked(2, AC3, 2, Some(192), None, false);
    let mono = ranked(1, AC3, 1, Some(192), None, true);
    assert_eq!(streams_of(&guards(vec![stereo, mono], &Settings::default())), vec![1, 2]);
}

#[test]
fn g2_does_not_apply_when_the_top_track_wins_a_later_criterion_is_lossless_has_more_channels_or_is_first() {
    let settings = Settings::default();
    let mono = || ranked(1, AC3, 1, Some(192), None, true);
    let higher_rate = ranked(2, AC3, 2, Some(224), None, false);
    assert_eq!(streams_of(&guards(vec![higher_rate, mono()], &settings)), vec![2, 1]);
    let lossless = ranked(2, LPCM, 2, None, Some(16), false);
    assert_eq!(streams_of(&guards(vec![lossless, mono()], &settings)), vec![2, 1]);
    let surround = ranked(2, AC3, 6, Some(192), None, false);
    assert_eq!(streams_of(&guards(vec![surround, mono()], &settings)), vec![2, 1]);
    let first = ranked(1, AC3, 2, Some(192), None, true);
    let later_mono = ranked(2, AC3, 1, Some(192), None, false);
    assert_eq!(streams_of(&guards(vec![first, later_mono], &settings)), vec![1, 2]);
}

#[test]
fn g2_compares_bit_rate_only_when_the_first_track_is_lossy_too() {
    // A lossy stereo track above G1's line beats a lossless mono first track on channels and on no later criterion
    // (its tier and depth are worse); bit rate ranks lossy tracks only (8.6 step 3.5), so it does not count.
    let stereo = ranked(2, AC3, 2, Some(320), None, false);
    let mono = ranked(1, LPCM, 1, None, Some(16), true);
    assert_eq!(streams_of(&guards(vec![stereo, mono], &Settings::default())), vec![1, 2]);
}

#[test]
fn the_guards_apply_whatever_prefer_most_channels_says() {
    let off = Settings { prefer_most_channels: false, ..Settings::default() };
    let lossy = ranked(2, AC3, 6, Some(256), None, false);
    let lossless = ranked(1, LPCM, 2, None, Some(24), true);
    assert_eq!(streams_of(&guards(vec![lossy, lossless], &off)), vec![1, 2]);
    let stereo = ranked(2, AC3, 2, Some(192), None, false);
    let mono = ranked(1, AC3, 1, Some(192), None, true);
    assert_eq!(streams_of(&guards(vec![stereo, mono], &off)), vec![1, 2]);
}

/// `attrs` with one more attribute, which overrides an earlier one with the same id.
fn with(mut attrs: Attrs, id: u32, value: &'static str) -> Attrs {
    attrs.push((id, value));
    attrs
}

/// A TrueHD stream, 24-bit.
fn truehd(language: &'static str, channels: &'static str) -> Attrs {
    with(audio(language, "A_TRUEHD", "TrueHD", "TrueHD", channels, ""), 18, "24")
}

/// A DTS-HD Master Audio stream, 24-bit.
fn dts_hd_ma(language: &'static str, channels: &'static str) -> Attrs {
    with(audio(language, "A_DTS", "DTS-HD MA", "DTS-HD Master Audio", channels, ""), 18, "24")
}

/// A DTS stream.
fn dts(language: &'static str, channels: &'static str, bitrate: &'static str) -> Attrs {
    audio(language, "A_DTS", "DTS", "DTS", channels, bitrate)
}

/// An E-AC-3 stream, with MakeMKV's documented short name (not yet seen in a recording).
fn eac3(language: &'static str, channels: &'static str, bitrate: &'static str) -> Attrs {
    audio(language, "A_EAC3", "E-AC3", "Dolby Digital Plus", channels, bitrate)
}

/// An LPCM stream of this bit depth and sample rate.
fn lpcm(language: &'static str, channels: &'static str, depth: &'static str, rate: &'static str) -> Attrs {
    with(with(audio(language, "A_LPCM", "LPCM", "LPCM", channels, ""), 17, rate), 18, depth)
}

/// The decision with no facts (planning time) and the default settings, for a work whose TMDb original language is
/// `original`.
fn decide(title: &Title, original: Option<&str>) -> AudioDecision {
    decide_audio(title, &wanted_languages(original), &AudioFacts::default(), &Settings::default())
}

/// The kept streams, in final order.
fn order_of(decision: &AudioDecision) -> Vec<u32> {
    decision.kept.iter().map(|track| track.stream).collect()
}

/// The kept tracks' names, in final order.
fn names(decision: &AudioDecision) -> Vec<&str> {
    decision.kept.iter().map(|track| track.name.as_str()).collect()
}

/// The notes as (note, stream) pairs, in the order raised.
fn noted(decision: &AudioDecision) -> Vec<(Note, Option<u32>)> {
    decision.notes.iter().map(|noted| (noted.note, noted.stream)).collect()
}

/// The kept tracks' default flags, in final order.
fn defaults(decision: &AudioDecision) -> Vec<bool> {
    decision.kept.iter().map(|track| track.default).collect()
}

/// The kept tracks' original flags, in final order.
fn originals(decision: &AudioDecision) -> Vec<bool> {
    decision.kept.iter().map(|track| track.original).collect()
}

/// Knowing's feature (title 0) of a scan text.
fn knowing_feature(text: &str) -> Title {
    pinned_disc(text).titles[&0].clone()
}

/// The Office's title 1 (S03E15) as its scan lays it out: video, DTS-HD MA 5.1 (stream 1), its DTS core (2), a second
/// DTS-HD MA 5.1 (3), its core (4), PGS English (5) and the derived forced-only PGS (6).
fn office_title_1() -> Title {
    let hd_ma = with(with(dts_hd_ma("eng", "6"), 2, "Surround 5.1"), 22, "1024");
    let core = with(with(dts("eng", "6", "1.5 Mb/s"), 2, "Surround 5.1"), 22, "2304");
    let pgs: Attrs = vec![(1, "Subtitles"), (3, "eng"), (5, "S_HDMV/PGS"), (6, "PGS"), (7, "HDMV PGS Subtitles")];
    title_of(&[video(), hd_ma.clone(), core.clone(), hd_ma, core, pgs.clone(), with(pgs, 22, "6144")])
}

/// Made-up packet MD5s of `office_title_1`'s twins (D13 needs only that identical streams carry equal strings): streams 1
/// and 3 share one, their cores 2 and 4 another.
fn twin_md5() -> BTreeMap<u32, String> {
    BTreeMap::from([
        (1, "aaaa0000000000000000000000000001".to_string()),
        (2, "bbbb0000000000000000000000000002".to_string()),
        (3, "aaaa0000000000000000000000000001".to_string()),
        (4, "bbbb0000000000000000000000000002".to_string()),
    ])
}

#[test]
fn knowings_feature_keeps_its_english_5_1_and_drops_the_flagged_commentary_and_the_spanish_dub() {
    let disc = pinned_disc(&corpus(&format!("{KNOWING}.txt")));
    let record = db_title(&pinned_record(KNOWING), 0);
    let facts = AudioFacts { discdb: Some(&record), ..AudioFacts::default() };
    let decision = decide_audio(&disc.titles[&0], &wanted_languages(Some("en")), &facts, &Settings::default());
    assert_eq!(
        decision,
        AudioDecision {
            kept: vec![AudioTrack {
                stream: 1,
                language: Some("eng".to_string()),
                group: LangGroup::English,
                codec: codec("A_AC3", "DD", "Dolby Digital"),
                channels: 6,
                tier: Some(Tier::OtherLossy),
                object: None,
                sample_size: None,
                sample_rate: Some(48000),
                bitrate_kbps: Some(448),
                first_of_language: true,
                name: "Dolby Digital 5.1".to_string(),
                default: true,
                original: true,
            }],
            dropped: vec![(2, AudioDrop::UnwantedLanguage), (3, AudioDrop::Flagged { flags: 1 })],
            reference: Some(1),
            verdict_reference: None,
            notes: Vec::new(),
            reviews: Vec::new(),
            confidence: 0.95,
            description_alignment: Some(Alignment::Full),
        }
    );
}

#[test]
fn a_commentary_thediscdb_describes_is_dropped_when_the_disc_does_not_flag_it() {
    let text = corpus(&format!("{KNOWING}.txt"));
    assert_eq!(text.matches("SINFO:0,3,22,0,\"1\"").count(), 1);
    let unflagged = text.replace("SINFO:0,3,22,0,\"1\"", "SINFO:0,3,22,0,\"0\"");
    let title = knowing_feature(&unflagged);
    let record = db_title(&pinned_record(KNOWING), 0);
    let facts = AudioFacts { discdb: Some(&record), ..AudioFacts::default() };
    let decision = decide_audio(&title, &wanted_languages(Some("en")), &facts, &Settings::default());
    assert_eq!(
        decision.dropped,
        vec![(2, AudioDrop::UnwantedLanguage), (3, AudioDrop::Described { kind: DescriptionKind::Commentary })]
    );
    assert_eq!(order_of(&decision), vec![1]);
    // Without TheDiscDB nothing confirms it: it is kept and noted (D12).
    let unconfirmed = decide(&title, Some("en"));
    assert_eq!(order_of(&unconfirmed), vec![1, 3]);
    assert_eq!(noted(&unconfirmed), vec![(Note::PossibleCommentary, Some(3))]);
}

#[test]
fn smiles_feature_keeps_truehd_atmos_7_1_then_dolby_digital_5_1() {
    let disc = pinned_disc(&corpus(&format!("{SMILE}.txt")));
    let record = db_title(&pinned_record(SMILE), 14);
    let facts = AudioFacts { discdb: Some(&record), ..AudioFacts::default() };
    let decision = decide_audio(&disc.titles[&14], &wanted_languages(Some("en")), &facts, &Settings::default());
    let mut dropped = vec![(2, AudioDrop::Derived)];
    dropped.extend((4..=9).map(|stream| (stream, AudioDrop::UnwantedLanguage)));
    dropped.push((10, AudioDrop::Described { kind: DescriptionKind::Commentary }));
    assert_eq!(
        decision,
        AudioDecision {
            kept: vec![
                AudioTrack {
                    stream: 1,
                    language: Some("eng".to_string()),
                    group: LangGroup::English,
                    codec: codec("A_TRUEHD", "TrueHD", "TrueHD Atmos"),
                    channels: 8,
                    tier: Some(Tier::Lossless),
                    object: Some(ObjectAudio::Atmos),
                    sample_size: Some(24),
                    sample_rate: Some(48000),
                    bitrate_kbps: Some(128),
                    first_of_language: true,
                    name: "TrueHD Atmos 7.1".to_string(),
                    default: true,
                    original: true,
                },
                AudioTrack {
                    stream: 3,
                    language: Some("eng".to_string()),
                    group: LangGroup::English,
                    codec: codec("A_AC3", "DD", "Dolby Digital"),
                    channels: 6,
                    tier: Some(Tier::OtherLossy),
                    object: None,
                    sample_size: None,
                    sample_rate: Some(48000),
                    bitrate_kbps: Some(640),
                    first_of_language: false,
                    name: "Dolby Digital 5.1".to_string(),
                    default: false,
                    original: true,
                },
            ],
            dropped,
            reference: Some(1),
            verdict_reference: None,
            notes: Vec::new(),
            reviews: Vec::new(),
            confidence: 0.95,
            description_alignment: Some(Alignment::Full),
        }
    );
}

#[test]
fn without_thediscdb_smiles_commentary_is_kept_and_noted_possible_commentary() {
    let disc = pinned_disc(&corpus(&format!("{SMILE}.txt")));
    let decision = decide(&disc.titles[&14], Some("en"));
    assert_eq!(order_of(&decision), vec![1, 3, 10]);
    assert_eq!(
        decision.notes,
        vec![Noted {
            note: Note::PossibleCommentary,
            stream: Some(10),
            detail: "stream 10 (Dolby Digital 2.0) is lossy with two channels or fewer and is not the first eng stream on the disc"
                .to_string(),
        }]
    );
    let mut dropped = vec![(2, AudioDrop::Derived)];
    dropped.extend((4..=9).map(|stream| (stream, AudioDrop::UnwantedLanguage)));
    assert_eq!(decision.dropped, dropped);
    assert_eq!(decision.description_alignment, None);
    assert_eq!(decision.reviews, Vec::new());
}

#[test]
fn every_flag_of_step_1_drops_its_stream() {
    let stereo = || dd("eng", "2", "192 Kb/s");
    let title = title_of(&[
        video(),
        dd("eng", "6", "640 Kb/s"),
        with(stereo(), 22, "2"),
        with(stereo(), 22, "4"),
        with(stereo(), 22, "5"),
        with(stereo(), 22, "512"),
        with(dts("eng", "6", "1.5 Mb/s"), 22, "2304"),
        with(stereo(), 22, "2049"),
        with(stereo(), 22, "65536"),
    ]);
    let decision = decide(&title, Some("en"));
    assert_eq!(
        decision.dropped,
        vec![
            (2, AudioDrop::Flagged { flags: 2 }),
            (3, AudioDrop::Flagged { flags: 4 }),
            (4, AudioDrop::Flagged { flags: 5 }),
            (5, AudioDrop::Secondary),
            (6, AudioDrop::Derived),
            (7, AudioDrop::Derived),
        ]
    );
    // 65536 is a bit spec 9.5 does not name: ignored, so stream 8 is kept.
    assert_eq!(order_of(&decision), vec![1, 8]);
    assert_eq!(noted(&decision), vec![(Note::PossibleCommentary, Some(8))]);
}

#[test]
fn an_exact_duplicate_by_md5_is_dropped_and_the_first_copy_kept() {
    let title = office_title_1();
    let facts = AudioFacts { md5: twin_md5(), ..AudioFacts::default() };
    let decision = decide_audio(&title, &wanted_languages(Some("en")), &facts, &Settings::default());
    assert_eq!(
        decision,
        AudioDecision {
            kept: vec![AudioTrack {
                stream: 1,
                language: Some("eng".to_string()),
                group: LangGroup::English,
                codec: codec("A_DTS", "DTS-HD MA", "DTS-HD Master Audio"),
                channels: 6,
                tier: Some(Tier::Lossless),
                object: None,
                sample_size: Some(24),
                sample_rate: Some(48000),
                bitrate_kbps: None,
                first_of_language: true,
                name: "DTS-HD MA 5.1".to_string(),
                default: true,
                original: true,
            }],
            dropped: vec![(2, AudioDrop::Derived), (3, AudioDrop::Duplicate { of: 1 }), (4, AudioDrop::Derived)],
            reference: Some(1),
            verdict_reference: None,
            notes: Vec::new(),
            reviews: Vec::new(),
            confidence: 0.95,
            description_alignment: None,
        }
    );
    // At planning time no MD5 is known: both lossless copies are kept.
    let planned = decide(&title, Some("en"));
    assert_eq!(order_of(&planned), vec![1, 3]);
    assert_eq!(planned.dropped, vec![(2, AudioDrop::Derived), (4, AudioDrop::Derived)]);
    assert_eq!(planned.notes, Vec::new());
}

#[test]
fn the_copy_of_a_stream_dropped_for_its_language_is_kept_so_one_copy_survives() {
    let title = title_of(&[video(), dd("fra", "6", "640 Kb/s"), dd("eng", "6", "640 Kb/s")]);
    let facts = AudioFacts {
        md5: BTreeMap::from([(1, "x".to_string()), (2, "x".to_string())]),
        ..AudioFacts::default()
    };
    let decision = decide_audio(&title, &wanted_languages(Some("en")), &facts, &Settings::default());
    // Stream 1 is dropped for its language, so stream 2 duplicates no kept copy (D13: one copy is kept).
    assert_eq!(decision.dropped, vec![(1, AudioDrop::UnwantedLanguage)]);
    assert_eq!(order_of(&decision), vec![2]);
    assert_eq!(decision.reviews, Vec::new());
}

#[test]
fn thediscdb_descriptions_drop_audio_description_isolated_score_and_music_and_effects_and_keep_the_rest() {
    let stereo = || dd("eng", "2", "192 Kb/s");
    let title = title_of(&[video(), dd("eng", "6", "640 Kb/s"), stereo(), stereo(), stereo(), stereo()]);
    let track = |index: u32, audio_type: &str, description: &str| {
        json!({"index": index, "type": "Audio", "name": "Dolby Digital", "audio_type": audio_type,
               "language_code": "eng", "language": "English", "description": description})
    };
    let record: DiscDbTitle = serde_json::from_value(json!({
        "index": 0,
        "tracks": [
            {"index": 0, "type": "Video", "name": "Mpeg4 AVC High@L4.1"},
            track(1, "Surround 5.1", "English"),
            track(2, "Stereo", "Descriptive Audio"),
            track(3, "Stereo", "Isolated Score"),
            track(4, "Stereo", "Music and Effects"),
            track(5, "Stereo", "Stereo Mix")
        ]
    }))
    .unwrap();
    let facts = AudioFacts { discdb: Some(&record), ..AudioFacts::default() };
    let decision = decide_audio(&title, &wanted_languages(Some("en")), &facts, &Settings::default());
    assert_eq!(
        decision.dropped,
        vec![
            (2, AudioDrop::Described { kind: DescriptionKind::AudioDescription }),
            (3, AudioDrop::Described { kind: DescriptionKind::IsolatedScore }),
            (4, AudioDrop::Described { kind: DescriptionKind::MusicAndEffects }),
        ]
    );
    // "English" is a dialogue description and "Stereo Mix" names no kind: both kept.
    assert_eq!(order_of(&decision), vec![1, 5]);
    assert_eq!(noted(&decision), vec![(Note::PossibleCommentary, Some(5))]);
    assert_eq!(decision.description_alignment, Some(Alignment::Full));
}

#[test]
fn a_stream_in_an_unwanted_language_is_dropped_and_streams_without_one_are_kept_last() {
    let title = title_of(&[
        video(),
        dd("fra", "6", "640 Kb/s"),
        dd("und", "2", "192 Kb/s"),
        dd("eng", "2", "192 Kb/s"),
        dd("", "6", "640 Kb/s"),
    ]);
    let decision = decide(&title, Some("en"));
    assert_eq!(decision.dropped, vec![(1, AudioDrop::UnwantedLanguage)]);
    assert_eq!(order_of(&decision), vec![3, 4, 2]);
    let groups: Vec<LangGroup> = decision.kept.iter().map(|track| track.group).collect();
    assert_eq!(groups, vec![LangGroup::English, LangGroup::NoLanguage, LangGroup::NoLanguage]);
    let languages: Vec<Option<&str>> = decision.kept.iter().map(|track| track.language.as_deref()).collect();
    assert_eq!(languages, vec![Some("eng"), None, Some("und")]);
    let firsts: Vec<bool> = decision.kept.iter().map(|track| track.first_of_language).collect();
    assert_eq!(firsts, vec![true, false, true]);
    assert_eq!(defaults(&decision), vec![true, false, false]);
    assert_eq!(originals(&decision), vec![true, false, false]);
    assert_eq!(decision.reference, Some(3));
    assert_eq!(decision.notes, Vec::new());
    assert_eq!(decision.reviews, Vec::new());
}

#[test]
fn original_language_first_puts_the_spanish_track_first_and_makes_it_the_default() {
    let title = knowing_feature(&corpus(&format!("{KNOWING}.txt")));
    let decision = decide(&title, Some("es"));
    assert_eq!(order_of(&decision), vec![2, 1]);
    let groups: Vec<LangGroup> = decision.kept.iter().map(|track| track.group).collect();
    assert_eq!(groups, vec![LangGroup::Original, LangGroup::English]);
    assert_eq!(defaults(&decision), vec![true, false]);
    assert_eq!(originals(&decision), vec![true, false]);
    assert_eq!(decision.reference, Some(2));
    assert_eq!(decision.dropped, vec![(3, AudioDrop::Flagged { flags: 1 })]);
    assert_eq!(decision.notes, Vec::new());
    assert_eq!(decision.reviews, Vec::new());
}

#[test]
fn with_original_language_first_off_english_comes_first_and_the_spanish_track_keeps_its_original_flag() {
    let title = knowing_feature(&corpus(&format!("{KNOWING}.txt")));
    let settings = Settings { original_language_first: false, ..Settings::default() };
    let decision = decide_audio(&title, &wanted_languages(Some("es")), &AudioFacts::default(), &settings);
    assert_eq!(order_of(&decision), vec![1, 2]);
    assert_eq!(defaults(&decision), vec![true, false]);
    assert_eq!(originals(&decision), vec![false, true]);
    // The programme reference is the original-language track whatever the order.
    assert_eq!(decision.reference, Some(2));
}

#[test]
fn with_prefer_most_channels_on_a_lossy_7_1_leads_a_lossless_5_1_with_both_notes() {
    let title = title_of(&[video(), truehd("eng", "6"), eac3("eng", "8", "1024 Kb/s")]);
    let decision = decide(&title, Some("en"));
    assert_eq!(order_of(&decision), vec![2, 1]);
    assert_eq!(names(&decision), vec!["Dolby Digital Plus 7.1", "TrueHD 5.1"]);
    assert_eq!(decision.kept[0].tier, Some(Tier::HighRateLossy));
    assert_eq!(
        decision.notes,
        vec![
            Noted {
                note: Note::LosslessOriginalAvailable,
                stream: None,
                detail: "the first eng track, stream 2 (Dolby Digital Plus 7.1), is lossy while stream 1 (TrueHD 5.1) is lossless with fewer channels"
                    .to_string(),
            },
            Noted {
                note: Note::LossyBeforeLossless,
                stream: Some(2),
                detail: "stream 2 (Dolby Digital Plus 7.1) is lossy and placed before the lossless stream 1 (TrueHD 5.1), which has fewer channels (D9)"
                    .to_string(),
            },
        ]
    );
}

#[test]
fn with_prefer_most_channels_off_the_lossless_5_1_leads_and_nothing_is_noted() {
    let title = title_of(&[video(), truehd("eng", "6"), eac3("eng", "8", "1024 Kb/s")]);
    let settings = Settings { prefer_most_channels: false, ..Settings::default() };
    let decision = decide_audio(&title, &wanted_languages(Some("en")), &AudioFacts::default(), &settings);
    assert_eq!(order_of(&decision), vec![1, 2]);
    assert_eq!(decision.notes, Vec::new());
}

#[test]
fn the_track_with_more_channels_ranks_first() {
    let title = title_of(&[video(), dd("eng", "2", "192 Kb/s"), dd("eng", "6", "448 Kb/s")]);
    let decision = decide(&title, Some("en"));
    assert_eq!(order_of(&decision), vec![2, 1]);
    // Stream 1 is the first English stream on the disc, so it is not a possible commentary.
    assert_eq!(decision.notes, Vec::new());
}

#[test]
fn among_equal_channels_the_better_fidelity_tier_ranks_first() {
    let title = title_of(&[
        video(),
        audio("eng", "A_AAC", "AAC", "AAC", "6", "256 Kb/s"),
        dd("eng", "6", "448 Kb/s"),
        dts("eng", "6", "1.5 Mb/s"),
        dts_hd_ma("eng", "6"),
        audio("eng", "A_OPUS", "Opus", "Opus", "6", "256 Kb/s"),
    ]);
    let decision = decide(&title, Some("en"));
    assert_eq!(order_of(&decision), vec![4, 3, 2, 1, 5]);
    let tiers: Vec<Option<Tier>> = decision.kept.iter().map(|track| track.tier).collect();
    assert_eq!(
        tiers,
        vec![Some(Tier::Lossless), Some(Tier::HighRateLossy), Some(Tier::OtherLossy), Some(Tier::Low), None]
    );
    assert_eq!(names(&decision), vec!["DTS-HD MA 5.1", "DTS 5.1", "Dolby Digital 5.1", "AAC 5.1", "Opus 5.1"]);
    assert_eq!(decision.notes, Vec::new());
}

#[test]
fn an_object_based_track_ranks_first_when_ffprobe_reports_it_and_not_at_planning_time() {
    let title = title_of(&[
        video(),
        truehd("eng", "8"),
        with(audio("eng", "A_TRUEHD", "TrueHD", "TrueHD Atmos", "8", ""), 18, "24"),
    ]);
    let planned = decide(&title, Some("en"));
    assert_eq!(order_of(&planned), vec![1, 2]);
    assert_eq!(names(&planned), vec!["TrueHD 7.1", "TrueHD Atmos 7.1"]);
    assert_eq!(planned.kept[1].object, Some(ObjectAudio::Atmos));
    let probed = AudioFacts {
        ffprobe_profile: BTreeMap::from([(1, "TrueHD".to_string()), (2, "Dolby TrueHD + Dolby Atmos".to_string())]),
        ..AudioFacts::default()
    };
    let decision = decide_audio(&title, &wanted_languages(Some("en")), &probed, &Settings::default());
    assert_eq!(order_of(&decision), vec![2, 1]);
    assert_eq!(names(&decision), vec!["TrueHD Atmos 7.1", "TrueHD 7.1"]);
    // A profile that names no extension does not rank the track first (3.3 reads ffprobe only), but its name and
    // object still take attribute 7's Atmos: the profile spellings are unverified (EXP-06).
    let plain = AudioFacts {
        ffprobe_profile: BTreeMap::from([(1, "TrueHD".to_string()), (2, "TrueHD".to_string())]),
        ..AudioFacts::default()
    };
    let decision = decide_audio(&title, &wanted_languages(Some("en")), &plain, &Settings::default());
    assert_eq!(order_of(&decision), vec![1, 2]);
    assert_eq!(names(&decision), vec!["TrueHD 7.1", "TrueHD Atmos 7.1"]);
    assert_eq!(decision.kept[1].object, Some(ObjectAudio::Atmos));
}

#[test]
fn among_equal_channels_tier_and_object_the_deeper_then_the_faster_sampled_track_ranks_first() {
    let title = title_of(&[
        video(),
        lpcm("eng", "2", "16", "48000"),
        lpcm("eng", "2", "24", "48000"),
        lpcm("eng", "2", "24", "96000"),
    ]);
    let decision = decide(&title, Some("en"));
    assert_eq!(order_of(&decision), vec![3, 2, 1]);
    assert_eq!(names(&decision), vec!["LPCM 2.0", "LPCM 2.0", "LPCM 2.0"]);
    assert_eq!(decision.notes, Vec::new());
}

#[test]
fn bit_rate_decides_between_lossy_tracks_only() {
    let lossy = title_of(&[video(), dd("eng", "6", "448 Kb/s"), dd("eng", "6", "640 Kb/s"), dd("eng", "6", "")]);
    assert_eq!(order_of(&decide(&lossy, Some("en"))), vec![2, 1, 3]);
    let lossless =
        title_of(&[video(), with(truehd("eng", "6"), 13, "4.5 Mb/s"), with(truehd("eng", "6"), 13, "9.8 Mb/s")]);
    let decision = decide(&lossless, Some("en"));
    assert_eq!(order_of(&decision), vec![1, 2]);
    let rates: Vec<Option<u32>> = decision.kept.iter().map(|track| track.bitrate_kbps).collect();
    assert_eq!(rates, vec![Some(4500), Some(9800)]);
}

#[test]
fn identical_tracks_keep_their_disc_order() {
    let title = title_of(&[video(), dd("eng", "6", "448 Kb/s"), dd("eng", "6", "448 Kb/s")]);
    let decision = decide(&title, Some("en"));
    assert_eq!(order_of(&decision), vec![1, 2]);
    assert_eq!(decision.dropped, Vec::new());
}

#[test]
fn g1_keeps_a_256_kbps_lossy_5_1_behind_a_lossless_stereo_track() {
    let title = title_of(&[video(), lpcm("eng", "2", "24", "48000"), dd("eng", "6", "256 Kb/s")]);
    let decision = decide(&title, Some("en"));
    assert_eq!(order_of(&decision), vec![1, 2]);
    assert_eq!(decision.notes, Vec::new());
    let above = title_of(&[video(), lpcm("eng", "2", "24", "48000"), dd("eng", "6", "257 Kb/s")]);
    let decision = decide(&above, Some("en"));
    assert_eq!(order_of(&decision), vec![2, 1]);
    assert_eq!(
        noted(&decision),
        vec![(Note::LosslessOriginalAvailable, None), (Note::LossyBeforeLossless, Some(2))]
    );
}

#[test]
fn g2_keeps_the_first_mono_track_first_when_a_stereo_track_beats_it_on_channels_alone() {
    let title = title_of(&[video(), dd("eng", "1", "192 Kb/s"), dd("eng", "2", "192 Kb/s")]);
    let decision = decide(&title, Some("en"));
    assert_eq!(order_of(&decision), vec![1, 2]);
    assert_eq!(names(&decision), vec!["Dolby Digital 1.0", "Dolby Digital 2.0"]);
    assert_eq!(noted(&decision), vec![(Note::PossibleCommentary, Some(2))]);
}

#[test]
fn a_lossy_stereo_track_that_is_not_first_of_its_language_is_noted_possible_commentary() {
    let title = title_of(&[
        video(),
        with(dts_hd_ma("eng", "6"), 22, "1024"),
        with(dts("eng", "6", "1.5 Mb/s"), 22, "2304"),
        dd("eng", "2", "192 Kb/s"),
    ]);
    let decision = decide(&title, Some("en"));
    assert_eq!(order_of(&decision), vec![1, 3]);
    assert_eq!(decision.dropped, vec![(2, AudioDrop::Derived)]);
    assert_eq!(
        decision.notes,
        vec![Noted {
            note: Note::PossibleCommentary,
            stream: Some(3),
            detail: "stream 3 (Dolby Digital 2.0) is lossy with two channels or fewer and is not the first eng stream on the disc"
                .to_string(),
        }]
    );
}

#[test]
fn a_spanish_work_with_only_spanish_audio_notes_no_english_audio() {
    let title = title_of(&[video(), dd("spa", "6", "448 Kb/s")]);
    let decision = decide(&title, Some("es"));
    assert_eq!(order_of(&decision), vec![1]);
    assert_eq!(decision.kept[0].group, LangGroup::Original);
    assert_eq!(defaults(&decision), vec![true]);
    assert_eq!(originals(&decision), vec![true]);
    assert_eq!(
        decision.notes,
        vec![Noted {
            note: Note::NoEnglishAudio,
            stream: None,
            detail: "no English audio stream is kept; the original-language audio is".to_string(),
        }]
    );
    assert_eq!(decision.reviews, Vec::new());
    assert_eq!(decision.reference, Some(1));
}

#[test]
fn a_spanish_work_without_spanish_audio_needs_review_for_the_missing_original_language() {
    let title = title_of(&[video(), dd("eng", "6", "448 Kb/s")]);
    let decision = decide(&title, Some("es"));
    assert_eq!(decision.reviews, vec![ReviewReason::OriginalLanguageMissing]);
    assert_eq!(order_of(&decision), vec![1]);
    assert_eq!(defaults(&decision), vec![true]);
    assert_eq!(originals(&decision), vec![false]);
    assert_eq!(decision.reference, Some(1));
    assert_eq!(decision.notes, Vec::new());
}

#[test]
fn only_an_english_work_marks_its_english_tracks_original() {
    // TMDb's original language unknown (None) or "xx" (no language): English is the reference language and comes
    // first, but the work is not an English one, so no track gets the original flag (8.6 step 5).
    let title = title_of(&[video(), dd("eng", "6", "448 Kb/s")]);
    for original in [None, Some("xx")] {
        let decision = decide(&title, original);
        assert_eq!(order_of(&decision), vec![1]);
        assert_eq!(defaults(&decision), vec![true]);
        assert_eq!(originals(&decision), vec![false]);
        assert_eq!(decision.reference, Some(1));
        assert_eq!(decision.reviews, Vec::new());
    }
    assert_eq!(originals(&decide(&title, Some("en"))), vec![true]);
}

#[test]
fn a_title_without_wanted_or_language_free_audio_needs_review_and_has_no_reference() {
    let french = title_of(&[video(), dd("fra", "6", "448 Kb/s")]);
    let decision = decide(&french, Some("en"));
    assert_eq!(decision.kept, Vec::new());
    assert_eq!(decision.dropped, vec![(1, AudioDrop::UnwantedLanguage)]);
    assert_eq!(decision.reviews, vec![ReviewReason::NoWantedAudio]);
    assert_eq!(decision.reference, None);
    assert_eq!(decision.confidence, 0.95);
    assert_eq!(
        decide(&french, Some("es")).reviews,
        vec![ReviewReason::OriginalLanguageMissing, ReviewReason::NoWantedAudio]
    );
    let silent = decide(&title_of(&[video()]), Some("en"));
    assert_eq!(silent.reviews, vec![ReviewReason::NoWantedAudio]);
    assert_eq!(silent.dropped, Vec::new());
    assert_eq!(silent.confidence, 0.95);
}

#[test]
fn a_no_language_stream_alone_is_kept_without_review_and_without_reference() {
    let title = title_of(&[video(), dd("und", "6", "448 Kb/s")]);
    let decision = decide(&title, Some("en"));
    assert_eq!(order_of(&decision), vec![1]);
    assert_eq!(decision.kept[0].group, LangGroup::NoLanguage);
    assert_eq!(defaults(&decision), vec![true]);
    assert_eq!(originals(&decision), vec![false]);
    assert_eq!(decision.reviews, Vec::new());
    assert_eq!(decision.reference, None);
}

#[test]
fn a_track_without_a_channel_count_is_named_without_a_layout() {
    let no_channels: Attrs =
        vec![(1, "Audio"), (3, "eng"), (5, "A_AC3"), (6, "DD"), (7, "Dolby Digital"), (13, "448 Kb/s")];
    let decision = decide(&title_of(&[video(), no_channels]), Some("en"));
    assert_eq!(decision.kept[0].channels, 0);
    assert_eq!(names(&decision), vec!["Dolby Digital"]);
    assert_eq!(decision.notes, Vec::new());
}

/// TrueHD 7.1 (stream 1), Dolby Digital 5.1 at 640 kb/s (stream 2) and Dolby Digital stereo at 224 kb/s (stream 3), all
/// English: the programme reference is stream 1, and step 6 notes stream 3 as a possible commentary.
fn verdict_title() -> Title {
    title_of(&[video(), truehd("eng", "8"), dd("eng", "6", "640 Kb/s"), dd("eng", "2", "224 Kb/s")])
}

/// The decision of `verdict_title` for an English work with these verdicts, measured against `reference`, and the veto.
fn with_verdicts(verdicts: &[(u32, Verdict)], reference: Option<u32>, veto_holds: bool) -> AudioDecision {
    let facts = AudioFacts {
        verdicts: verdicts.iter().copied().collect(),
        verdict_reference: reference,
        veto_holds,
        ..AudioFacts::default()
    };
    decide_audio(&verdict_title(), &wanted_languages(Some("en")), &facts, &Settings::default())
}

#[test]
fn reference_unclear_raises_audio_reference_unclear_and_drops_nothing() {
    let decision = with_verdicts(&[(2, Verdict::ReferenceUnclear)], Some(1), false);
    assert_eq!(decision.reviews, vec![ReviewReason::AudioReferenceUnclear]);
    assert_eq!(order_of(&decision), vec![1, 2, 3]);
    assert_eq!(decision.dropped, Vec::new());
    assert_eq!(decision.reference, Some(1));
    assert_eq!(decision.verdict_reference, Some(1));
    assert_eq!(noted(&decision), vec![(Note::PossibleCommentary, Some(3))]);
}

#[test]
fn commentary_and_cross_language_commentary_raise_commentary_detected_and_drop_nothing() {
    let commentary = with_verdicts(&[(3, Verdict::Commentary)], Some(1), false);
    assert_eq!(commentary.reviews, vec![ReviewReason::CommentaryDetected]);
    assert_eq!(order_of(&commentary), vec![1, 2, 3]);
    assert_eq!(commentary.dropped, Vec::new());
    let cross = with_verdicts(&[(3, Verdict::CommentaryCrossLanguage)], Some(1), false);
    assert_eq!(cross.reviews, vec![ReviewReason::CommentaryDetected]);
    assert_eq!(order_of(&cross), vec![1, 2, 3]);
    let both = with_verdicts(&[(2, Verdict::Commentary), (3, Verdict::ReferenceUnclear)], Some(1), false);
    assert_eq!(both.reviews, vec![ReviewReason::AudioReferenceUnclear, ReviewReason::CommentaryDetected]);
}

#[test]
fn ad_pattern_and_note_ad_are_notes_of_possible_audio_description() {
    let step_6 = Noted {
        note: Note::PossibleCommentary,
        stream: Some(3),
        detail: "stream 3 (Dolby Digital 2.0) is lossy with two channels or fewer and is not the first eng stream on the disc"
            .to_string(),
    };
    let pattern = with_verdicts(&[(2, Verdict::AdPattern)], Some(1), false);
    assert_eq!(
        pattern.notes,
        vec![
            step_6.clone(),
            Noted {
                note: Note::PossibleAudioDescription,
                stream: Some(2),
                detail: "the content rule found the audio-description pattern on stream 2 (ad-pattern)".to_string(),
            },
        ]
    );
    assert_eq!(pattern.reviews, Vec::new());
    let note = with_verdicts(&[(2, Verdict::NoteAd)], Some(1), false);
    assert_eq!(
        note.notes,
        vec![
            step_6,
            Noted {
                note: Note::PossibleAudioDescription,
                stream: Some(2),
                detail: "the content rule notes a possible audio description on stream 2 (note-ad)".to_string(),
            },
        ]
    );
    assert_eq!(note.reviews, Vec::new());
}

#[test]
fn note_commentary_is_a_possible_commentary_note_raised_once_per_stream() {
    // Stream 3 is already noted by step 6: the verdict adds no second note.
    let again = with_verdicts(&[(3, Verdict::NoteCommentary)], Some(1), false);
    assert_eq!(noted(&again), vec![(Note::PossibleCommentary, Some(3))]);
    let other = with_verdicts(&[(2, Verdict::NoteCommentary)], Some(1), false);
    assert_eq!(noted(&other), vec![(Note::PossibleCommentary, Some(3)), (Note::PossibleCommentary, Some(2))]);
    assert_eq!(other.notes[1].detail, "the content rule notes a possible commentary on stream 2 (note-commentary)");
    assert_eq!(other.reviews, Vec::new());
}

#[test]
fn keep_not_judged_and_duplicate_verdicts_add_nothing() {
    let quiet = with_verdicts(&[(2, Verdict::Keep), (3, Verdict::NotJudged)], Some(1), false);
    assert_eq!(quiet.reviews, Vec::new());
    assert_eq!(noted(&quiet), vec![(Note::PossibleCommentary, Some(3))]);
    let duplicate = with_verdicts(&[(2, Verdict::Duplicate)], Some(1), false);
    assert_eq!(duplicate.reviews, Vec::new());
    assert_eq!(order_of(&duplicate), vec![1, 2, 3]);
    assert_eq!(duplicate.dropped, Vec::new());
}

#[test]
fn the_veto_raises_audio_reference_unclear_and_confirms_no_commentary() {
    let decision = with_verdicts(&[(2, Verdict::AdPattern), (3, Verdict::Commentary)], Some(1), true);
    assert_eq!(decision.reviews, vec![ReviewReason::AudioReferenceUnclear]);
    assert_eq!(noted(&decision), vec![(Note::PossibleCommentary, Some(3))]);
    assert_eq!(order_of(&decision), vec![1, 2, 3]);
}

#[test]
fn verdicts_measured_against_another_reference_are_not_applied_and_raise_audio_reference_unclear() {
    let decision = with_verdicts(&[(2, Verdict::AdPattern), (3, Verdict::Commentary)], Some(2), false);
    assert_eq!(decision.reference, Some(1));
    assert_eq!(decision.verdict_reference, Some(2));
    assert_eq!(decision.reviews, vec![ReviewReason::AudioReferenceUnclear]);
    assert_eq!(noted(&decision), vec![(Note::PossibleCommentary, Some(3))]);
}

#[test]
fn verdicts_without_a_measured_reference_are_ignored() {
    let decision = with_verdicts(&[(3, Verdict::Commentary)], None, false);
    assert_eq!(decision.reviews, Vec::new());
    assert_eq!(decision.verdict_reference, None);
}

#[test]
fn a_verdict_on_a_no_language_stream_is_not_judged() {
    // 8.6 "Content" measures the streams in a wanted language only; phase 0 measures the no-language ones for the
    // dataset (audio::reference::wanted), and the app does not judge them.
    let title = title_of(&[video(), truehd("eng", "8"), dd("und", "2", "224 Kb/s")]);
    let facts = AudioFacts {
        verdicts: BTreeMap::from([(2, Verdict::Commentary)]),
        verdict_reference: Some(1),
        ..AudioFacts::default()
    };
    let decision = decide_audio(&title, &wanted_languages(Some("en")), &facts, &Settings::default());
    assert_eq!(decision.reference, Some(1));
    assert_eq!(decision.reviews, Vec::new());
    assert_eq!(order_of(&decision), vec![1, 2]);
}

#[test]
fn verdicts_on_a_title_without_a_reference_are_not_judged() {
    // No kept original-language or English track: no reference and nothing to judge (8.6 "Content"), although phase
    // 0's programme_reference measures the title against a stream of any language.
    let unlabelled = title_of(&[video(), dd("und", "6", "448 Kb/s"), dd("und", "2", "224 Kb/s")]);
    let facts = AudioFacts {
        verdicts: BTreeMap::from([(2, Verdict::Commentary)]),
        verdict_reference: Some(1),
        ..AudioFacts::default()
    };
    let decision = decide_audio(&unlabelled, &wanted_languages(Some("en")), &facts, &Settings::default());
    assert_eq!(decision.reference, None);
    assert_eq!(decision.verdict_reference, Some(1));
    assert_eq!(decision.reviews, Vec::new());
    assert_eq!(order_of(&decision), vec![1, 2]);
    // Nothing kept: only step 6's gap is raised.
    let french = title_of(&[video(), dd("fra", "6", "448 Kb/s"), dd("fra", "2", "224 Kb/s")]);
    let decision = decide_audio(&french, &wanted_languages(Some("en")), &facts, &Settings::default());
    assert_eq!(decision.reference, None);
    assert_eq!(decision.reviews, vec![ReviewReason::NoWantedAudio]);
}

#[test]
fn a_verdict_on_a_dropped_stream_is_ignored() {
    let title = knowing_feature(&corpus(&format!("{KNOWING}.txt")));
    let facts = AudioFacts {
        verdicts: BTreeMap::from([(2, Verdict::AdPattern), (3, Verdict::Commentary)]),
        verdict_reference: Some(1),
        ..AudioFacts::default()
    };
    let decision = decide_audio(&title, &wanted_languages(Some("en")), &facts, &Settings::default());
    assert_eq!(decision.dropped, vec![(2, AudioDrop::UnwantedLanguage), (3, AudioDrop::Flagged { flags: 1 })]);
    assert_eq!(decision.reviews, Vec::new());
    assert_eq!(decision.notes, Vec::new());
}

/// The decision of `title` for an English work with these remembered and review overrides and no other fact.
fn with_overrides(
    title: &Title,
    remembered: Option<&[AudioOverrideEntry]>,
    reviewed: Option<&BTreeMap<u32, AudioOverride>>,
) -> AudioDecision {
    let facts = AudioFacts { remembered, reviewed, ..AudioFacts::default() };
    decide_audio(title, &wanted_languages(Some("en")), &facts, &Settings::default())
}

#[test]
fn a_remembered_drop_or_commentary_drops_the_stream_it_names() {
    let title = verdict_title();
    let entries = vec![
        AudioOverrideEntry { signature: signature(&title.streams, 2), action: AudioOverride::Drop },
        AudioOverrideEntry { signature: signature(&title.streams, 3), action: AudioOverride::Commentary },
    ];
    let decision = with_overrides(&title, Some(entries.as_slice()), None);
    assert_eq!(decision.dropped, vec![(2, AudioDrop::Remembered), (3, AudioDrop::Remembered)]);
    assert_eq!(order_of(&decision), vec![1]);
    assert_eq!(defaults(&decision), vec![true]);
    // Stream 1 has no override, so the confidence stays the rule's.
    assert_eq!(decision.confidence, AUDIO_CONFIDENCE);
    // A remembered drop of the first of The Office's twins keeps the second: a dropped stream is no copy to keep (D13).
    let office = office_title_1();
    let office_entries = vec![AudioOverrideEntry { signature: signature(&office.streams, 1), action: AudioOverride::Drop }];
    let facts = AudioFacts { md5: twin_md5(), remembered: Some(office_entries.as_slice()), ..AudioFacts::default() };
    let twins = decide_audio(&office, &wanted_languages(Some("en")), &facts, &Settings::default());
    assert_eq!(twins.dropped, vec![(1, AudioDrop::Remembered), (2, AudioDrop::Derived), (4, AudioDrop::Derived)]);
    assert_eq!(order_of(&twins), vec![3]);
}

#[test]
fn a_review_override_wins_over_a_remembered_one() {
    let title = verdict_title();
    let remembered_drop = vec![AudioOverrideEntry { signature: signature(&title.streams, 3), action: AudioOverride::Drop }];
    let reviewed_keep = BTreeMap::from([(3, AudioOverride::Keep { order: 2, default: false })]);
    let kept = with_overrides(&title, Some(remembered_drop.as_slice()), Some(&reviewed_keep));
    assert_eq!(order_of(&kept), vec![1, 2, 3]);
    assert_eq!(kept.dropped, Vec::new());
    assert_eq!(defaults(&kept), vec![true, false, false]);
    let remembered_keep = vec![AudioOverrideEntry {
        signature: signature(&title.streams, 3),
        action: AudioOverride::Keep { order: 2, default: false },
    }];
    let reviewed_drop = BTreeMap::from([(3, AudioOverride::Drop)]);
    let dropped = with_overrides(&title, Some(remembered_keep.as_slice()), Some(&reviewed_drop));
    assert_eq!(dropped.dropped, vec![(3, AudioDrop::Reviewed)]);
    assert_eq!(order_of(&dropped), vec![1, 2]);
}

#[test]
fn a_keep_override_keeps_a_stream_the_rule_would_drop() {
    let title = knowing_feature(&corpus(&format!("{KNOWING}.txt")));
    let reviewed = BTreeMap::from([(3, AudioOverride::Keep { order: 1, default: false })]);
    let decision = with_overrides(&title, None, Some(&reviewed));
    assert_eq!(order_of(&decision), vec![1, 3]);
    assert_eq!(decision.dropped, vec![(2, AudioDrop::UnwantedLanguage)]);
    assert_eq!(defaults(&decision), vec![true, false]);
    assert_eq!(names(&decision), vec!["Dolby Digital 5.1", "Dolby Digital 2.0"]);
    assert_eq!(noted(&decision), vec![(Note::PossibleCommentary, Some(3))]);
}

#[test]
fn keep_orders_and_defaults_fix_the_order_and_the_default_track() {
    let title = verdict_title();
    let reviewed = BTreeMap::from([
        (3, AudioOverride::Keep { order: 0, default: true }),
        (1, AudioOverride::Keep { order: 1, default: false }),
    ]);
    let decision = with_overrides(&title, None, Some(&reviewed));
    assert_eq!(order_of(&decision), vec![3, 1, 2]);
    assert_eq!(defaults(&decision), vec![true, false, false]);
    // The programme reference comes from the rule's ranking, not from the owner's order.
    assert_eq!(decision.reference, Some(1));
    assert_eq!(decision.confidence, AUDIO_CONFIDENCE);
    assert_eq!(noted(&decision), vec![(Note::PossibleCommentary, Some(3))]);
}

#[test]
fn when_every_audio_stream_has_an_override_the_confidence_is_1_00() {
    let title = verdict_title();
    let reviewed = BTreeMap::from([
        (1, AudioOverride::Keep { order: 0, default: true }),
        (2, AudioOverride::Keep { order: 1, default: false }),
        (3, AudioOverride::Drop),
    ]);
    let decision = with_overrides(&title, None, Some(&reviewed));
    assert_eq!(decision.confidence, 1.0);
    assert_eq!(order_of(&decision), vec![1, 2]);
    assert_eq!(decision.dropped, vec![(3, AudioDrop::Reviewed)]);
    let remembered = vec![AudioOverrideEntry { signature: signature(&title.streams, 3), action: AudioOverride::Commentary }];
    let partly = BTreeMap::from([
        (1, AudioOverride::Keep { order: 0, default: true }),
        (2, AudioOverride::Keep { order: 1, default: false }),
    ]);
    let decision = with_overrides(&title, Some(remembered.as_slice()), Some(&partly));
    assert_eq!(decision.confidence, 1.0);
    assert_eq!(decision.dropped, vec![(3, AudioDrop::Remembered)]);
    // A title without audio has nothing an override covers.
    let silent = with_overrides(&title_of(&[video()]), None, Some(&reviewed));
    assert_eq!(silent.confidence, AUDIO_CONFIDENCE);
}

#[test]
fn an_order_out_of_range_or_taken_goes_to_the_next_free_position() {
    let title = verdict_title();
    let far = BTreeMap::from([(2, AudioOverride::Keep { order: 7, default: false })]);
    let decision = with_overrides(&title, None, Some(&far));
    assert_eq!(order_of(&decision), vec![1, 3, 2]);
    assert_eq!(defaults(&decision), vec![true, false, false]);
    let shared = BTreeMap::from([
        (3, AudioOverride::Keep { order: 0, default: false }),
        (2, AudioOverride::Keep { order: 0, default: false }),
    ]);
    let decision = with_overrides(&title, None, Some(&shared));
    assert_eq!(order_of(&decision), vec![2, 3, 1]);
    // No override names a default: the very first track is the default (8.6 step 5).
    assert_eq!(defaults(&decision), vec![true, false, false]);
}

#[test]
fn an_override_of_a_stream_that_is_not_audio_is_ignored() {
    let title = verdict_title();
    let reviewed = BTreeMap::from([(0, AudioOverride::Drop)]);
    let remembered = vec![AudioOverrideEntry { signature: signature(&title.streams, 0), action: AudioOverride::Drop }];
    let decision = with_overrides(&title, Some(remembered.as_slice()), Some(&reviewed));
    assert_eq!(order_of(&decision), vec![1, 2, 3]);
    assert_eq!(decision.dropped, Vec::new());
    assert_eq!(decision.confidence, AUDIO_CONFIDENCE);
}
