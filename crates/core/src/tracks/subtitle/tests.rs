//! Tests of core::tracks::subtitle, Task 9: design spec 8.7 "Set aside first" (with the section's first sentence),
//! "Events" and the 10% full rule, "Alignment" with its three bands and the groups they make, and "Dialogue group" with
//! each pick in order (Review Focus 4 among them). Scans are built with testsupport::scan_text or read from three of the
//! pinned logs of corpus/thediscdb (Knowing's DVD, Smile's UHD, Firefly's first DVD). Event start times are synthetic:
//! no title recorded in phase 0 has two full English tracks (spec 8.7 "Not yet verified").

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde_json::{json, Value};

use super::*;
use crate::disc::{disc, Stream, StreamKind, Title};
use crate::lookup::DescriptionKind;
use crate::recording::files::Packet;
use crate::robot::parse;
use crate::settings::Settings;
use crate::subtitle::align::events;
use crate::subtitle::text::Rule as TextRule;
use crate::testsupport::scan_text;
use crate::tracks::SubtitleRole as TracksRole;

// ------------------------------------------------------------------ fixtures

/// Knowing (2009), DVD: title 0 (2:01:14, DVD title "01") has subtitle streams 4 eng, 5 spa, 6 eng flagged 1, 7 spa
/// flagged 1 (director's comments).
const KNOWING: &str = "movie/Knowing (2009)/2010-dvd/disc01.txt";
/// Smile (2022), UHD: title 12 (0:09:03, 01188.mpls) has two English pairs, 3 and its derived forced-only 4, 21 and its
/// derived forced-only 22 (flags 6144); each English pair is followed by eight other-language pairs (5 to 20, 23 to 38:
/// German, Spanish, two French, Italian, Japanese, Dutch, Turkish).
const SMILE: &str = "movie/Smile (2022)/2022-4k/disc01.txt";
/// Firefly (2002), DVD 1: title 0 (1:26:26, DVD title "01") has subtitle streams 5 eng, 6 spa, 7 fre flagged 4096,
/// 8 spa flagged 4096, 9 eng closed captions (S_CC608/DVD).
const FIREFLY_1: &str = "series/Firefly (2002)/2003-dvd-the-complete-series/disc01.txt";

/// One title of a pinned TheDiscDB scan log under corpus/thediscdb, typed; the log is read as the corpus test reads it
/// (lossy UTF-8).
fn pinned_title(log: &str, number: u32) -> Title {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/thediscdb").join(log);
    let bytes = fs::read(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()));
    disc(&parse(&String::from_utf8_lossy(&bytes))).unwrap().title(number).unwrap().clone()
}

/// The SINFO attributes of one made-up stream. `kind` is "video", "audio", "pgs", "vobsub" or "cc", with the codec rows
/// of the recordings (AVC video, Dolby Digital stereo audio, HDMV PGS, DVD VobSub, and the DVD closed captions MakeMKV
/// converts to text, attributes 41 and 42); `language` "" writes no attribute 3; `flags` is attribute 22.
fn stream_attrs(kind: &str, language: &str, flags: u32) -> Vec<(u32, String)> {
    let rows: Vec<(u32, &str)> = match kind {
        "video" => vec![(1, "Video"), (5, "V_MPEG4/ISO/AVC"), (6, "Mpeg4"), (7, "Mpeg4 AVC High@L4.1")],
        "audio" => vec![(1, "Audio"), (5, "A_AC3"), (6, "DD"), (7, "Dolby Digital"), (14, "2")],
        "pgs" => vec![(1, "Subtitles"), (5, "S_HDMV/PGS"), (6, "PGS"), (7, "HDMV PGS Subtitles")],
        "vobsub" => vec![(1, "Subtitles"), (5, "S_VOBSUB"), (6, ""), (7, "Dvd Subtitles")],
        "cc" => vec![
            (1, "Subtitles"),
            (5, "S_CC608/DVD"),
            (6, "CC"),
            (7, "Closed Captions"),
            (41, "Text"),
            (42, "( Lossy conversion )"),
        ],
        other => panic!("no made-up stream kind {other}"),
    };
    let mut attrs: Vec<(u32, String)> = rows.into_iter().map(|(id, value)| (id, value.to_string())).collect();
    if !language.is_empty() {
        attrs.push((3, language.to_string()));
    }
    attrs.push((22, flags.to_string()));
    attrs
}

/// Title 0 of a made-up scan, one hour long: a Blu-ray title (attribute 16 "00800.mpls") or, with `dvd`, a DVD title
/// (attribute 24 "01", no attribute 16), holding `streams` as (kind, language, flags), numbered from 0 in that order.
fn title_of(dvd: bool, streams: &[(&str, &str, u32)]) -> Title {
    let owned: Vec<Vec<(u32, String)>> =
        streams.iter().map(|&(kind, language, flags)| stream_attrs(kind, language, flags)).collect();
    let borrowed: Vec<Vec<(u32, &str)>> =
        owned.iter().map(|attrs| attrs.iter().map(|(id, value)| (*id, value.as_str())).collect()).collect();
    let numbered: Vec<(u32, &[(u32, &str)])> =
        borrowed.iter().enumerate().map(|(at, attrs)| (at as u32, attrs.as_slice())).collect();
    let title_attrs: [(u32, &str); 2] =
        if dvd { [(9, "1:00:00"), (24, "01")] } else { [(9, "1:00:00"), (16, "00800.mpls")] };
    let kind = if dvd { "DVD disc" } else { "Blu-ray disc" };
    let text = scan_text(&[(1, kind)], &[(0, &title_attrs[..], &numbered[..])]);
    disc(&parse(&text)).unwrap().title(0).unwrap().clone()
}

/// Ten event starts 10 s apart: 0, 10, 20 and so on up to 90.
fn ten() -> Vec<f64> {
    (0..10u32).map(|k| f64::from(k) * 10.0).collect()
}

/// Ten starts 10 s apart whose first `matching` lie 0.1 s after `ten()`'s and the rest 5 s after: within the default
/// 300 ms of `ten()` exactly `matching` times, so aligned is `matching / 10` in both directions.
fn sharing(matching: u32) -> Vec<f64> {
    (0..10u32).map(|k| f64::from(k) * 10.0 + if k < matching { 0.1 } else { 5.0 }).collect()
}

/// `starts`, each `by` seconds later.
fn shifted(starts: &[f64], by: f64) -> Vec<f64> {
    starts.iter().map(|start| start + by).collect()
}

/// ffprobe's packets of a subs folder (`sub-<n>.packets.json`'s list): start time and size as ffprobe writes them.
fn packets(json: &str) -> Vec<Packet> {
    serde_json::from_str(json).unwrap()
}

// ------------------------------------------------------------------ types and fixed values

#[test]
fn the_fixed_values_equal_the_text_rules_full_share_and_forced_limit() {
    let rule = TextRule::default();
    assert_eq!(FULL_SHARE, 0.10);
    assert_eq!(FULL_SHARE, rule.FULL_SHARE);
    assert_eq!(FORCED_MAX_EVENTS, 150);
    assert_eq!(FORCED_MAX_EVENTS as i64, rule.FORCED_MAX_EVENTS);
}

#[test]
fn every_drop_reason_is_spelled_in_snake_case_under_why() {
    let spelled = [
        (SubtitleDrop::OtherLanguage, "other_language"),
        (SubtitleDrop::DerivedForcedOnlyEmpty, "derived_forced_only_empty"),
        (SubtitleDrop::DerivedForcedOnlyUnused, "derived_forced_only_unused"),
        (SubtitleDrop::DirectorsComments, "directors_comments"),
        (SubtitleDrop::OutsideDialogueGroup, "outside_dialogue_group"),
        (SubtitleDrop::Partial, "partial"),
        (SubtitleDrop::ExtraDialogueTrack, "extra_dialogue_track"),
        (SubtitleDrop::ExtraForced, "extra_forced"),
        (SubtitleDrop::ClosedCaptionsUnused, "closed_captions_unused"),
        (SubtitleDrop::NoEvents, "no_events"),
        (SubtitleDrop::Remembered, "remembered"),
        (SubtitleDrop::Reviewed, "reviewed"),
    ];
    for (reason, text) in spelled {
        assert_eq!(serde_json::to_value(&reason).unwrap(), json!({ "why": text }), "{reason:?}");
        assert_eq!(serde_json::from_value::<SubtitleDrop>(json!({ "why": text })).unwrap(), reason);
    }
    assert!(serde_json::from_value::<SubtitleDrop>(json!({ "why": "OtherLanguage" })).is_err());
}

#[test]
fn the_bands_are_spelled_same_different_and_between() {
    for (which, text) in [(Band::Same, "same"), (Band::Different, "different"), (Band::Between, "between")] {
        assert_eq!(serde_json::to_value(which).unwrap(), Value::String(text.to_string()));
        assert_eq!(serde_json::from_value::<Band>(Value::String(text.to_string())).unwrap(), which);
    }
    assert!(serde_json::from_value::<Band>(Value::String("Same".to_string())).is_err());
}

#[test]
fn the_dialogue_picks_are_spelled_in_snake_case() {
    let spelled = [
        (DialoguePick::OneFull, "one_full"),
        (DialoguePick::ForcedParent, "forced_parent"),
        (DialoguePick::Described, "described"),
        (DialoguePick::Largest, "largest"),
    ];
    for (pick, text) in spelled {
        assert_eq!(serde_json::to_value(pick).unwrap(), Value::String(text.to_string()));
        assert_eq!(serde_json::from_value::<DialoguePick>(Value::String(text.to_string())).unwrap(), pick);
    }
}

#[test]
fn default_facts_hold_no_evidence() {
    let facts = SubtitleFacts::default();
    assert!(facts.events.is_empty());
    assert_eq!(facts.removed, BTreeSet::new());
    assert!(facts.discdb.is_none());
    assert!(facts.text.is_none());
    assert!(facts.remembered.is_none());
    assert!(facts.reviewed.is_none());
    let measured = SubtitleFacts { events: BTreeMap::from([(2, vec![1.0, 4.5])]), ..SubtitleFacts::default() };
    assert_ne!(measured, facts);
    assert_eq!(measured.events[&2], vec![1.0, 4.5]);
}

#[test]
fn the_subtitle_role_is_the_tracks_role_re_exported() {
    let role: TracksRole = SubtitleRole::Sdh;
    assert_eq!(role.name(), "English (SDH)");
    assert_eq!(SubtitleRole::ALL, TracksRole::ALL);
}

// ------------------------------------------------------------------ set aside first (spec 8.7)

#[test]
fn knowings_dvd_title_sets_aside_its_directors_comments_stream_and_drops_spanish() {
    let title = pinned_title(KNOWING, 0);
    assert_eq!(title.dvd_title.as_deref(), Some("01"));
    assert_eq!(
        set_aside(&title),
        SetAside {
            forced_only: vec![],
            directors_comments: vec![6],
            closed_captions: None,
            other_language: vec![5, 7],
            counted: vec![4],
        }
    );
}

#[test]
fn smiles_title_12_sets_aside_two_forced_only_streams_each_with_its_own_parent() {
    let title = pinned_title(SMILE, 12);
    assert_eq!(title.dvd_title, None);
    let other_language: Vec<u32> = (5..=20).chain(23..=38).collect();
    assert_eq!(
        set_aside(&title),
        SetAside {
            forced_only: vec![(4, Some(3)), (22, Some(21))],
            directors_comments: vec![],
            closed_captions: None,
            other_language,
            counted: vec![3, 21],
        }
    );
}

#[test]
fn fireflys_dvd_title_sets_aside_its_closed_caption_stream_and_counts_its_english_vobsub_track() {
    let title = pinned_title(FIREFLY_1, 0);
    assert_eq!(title.dvd_title.as_deref(), Some("01"));
    assert_eq!(
        set_aside(&title),
        SetAside {
            forced_only: vec![],
            directors_comments: vec![],
            closed_captions: Some(9),
            other_language: vec![6, 7, 8],
            counted: vec![5],
        }
    );
}

#[test]
fn a_dvd_stream_flagged_alternate_directors_comments_is_set_aside() {
    let title = title_of(true, &[("video", "", 0), ("vobsub", "eng", 0), ("vobsub", "eng", 2)]);
    let aside = set_aside(&title);
    assert_eq!(aside.directors_comments, vec![2]);
    assert_eq!(aside.counted, vec![1]);
}

#[test]
fn a_blu_ray_stream_flagged_directors_comments_is_counted() {
    let title = title_of(false, &[("video", "", 0), ("pgs", "eng", 0), ("pgs", "eng", 1), ("pgs", "eng", 2)]);
    let aside = set_aside(&title);
    assert_eq!(aside.directors_comments, Vec::<u32>::new());
    assert_eq!(aside.counted, vec![1, 2, 3]);
}

#[test]
fn a_dvd_stream_flagged_forced_without_derived_is_counted() {
    let title = title_of(true, &[("video", "", 0), ("vobsub", "eng", 0), ("vobsub", "eng", 4096)]);
    let aside = set_aside(&title);
    assert!(aside.forced_only.is_empty());
    assert_eq!(aside.counted, vec![1, 2]);
}

#[test]
fn no_language_streams_are_counted_and_their_forced_only_streams_take_a_no_language_parent() {
    // Stream 1 has no attribute 3, stream 3 is "und": both name no language, so 2's parent is 1 and 4's is 3 (the
    // nearest preceding non-derived one; 2 is derived).
    let title = title_of(
        false,
        &[("video", "", 0), ("pgs", "", 0), ("pgs", "", 6144), ("pgs", "und", 0), ("pgs", "und", 6144)],
    );
    let aside = set_aside(&title);
    assert_eq!(aside.forced_only, vec![(2, Some(1)), (4, Some(3))]);
    assert_eq!(aside.counted, vec![1, 3]);
    assert_eq!(aside.other_language, Vec::<u32>::new());
}

#[test]
fn another_languages_forced_only_stream_is_dropped_with_its_language() {
    let title = title_of(false, &[("video", "", 0), ("pgs", "spa", 0), ("pgs", "spa", 6144), ("pgs", "eng", 0)]);
    let aside = set_aside(&title);
    assert_eq!(aside.other_language, vec![1, 2]);
    assert!(aside.forced_only.is_empty());
    assert_eq!(aside.counted, vec![3]);
}

#[test]
fn a_forced_only_stream_without_a_same_language_parent_is_set_aside_alone() {
    // Review Focus 4. Listed first: no stream precedes it.
    let first = title_of(false, &[("video", "", 0), ("pgs", "eng", 6144), ("pgs", "eng", 0)]);
    let subtitles = first.streams_of(StreamKind::Subtitles);
    assert_eq!(parent_of(&subtitles, 1), None);
    let aside = set_aside(&first);
    assert_eq!(aside.forced_only, vec![(1, None)]);
    assert_eq!(aside.counted, vec![2]);

    // After streams of other languages only: never adopted by Spanish or French.
    let after_others = title_of(false, &[("video", "", 0), ("pgs", "spa", 0), ("pgs", "eng", 6144), ("pgs", "fra", 0)]);
    let subtitles = after_others.streams_of(StreamKind::Subtitles);
    assert_eq!(parent_of(&subtitles, 2), None);
    let aside = set_aside(&after_others);
    assert_eq!(aside.forced_only, vec![(2, None)]);
    assert_eq!(aside.other_language, vec![1, 3]);
    assert_eq!(aside.counted, Vec::<u32>::new());

    // A no-language stream is not English: it parents no English forced-only stream.
    let after_untagged = title_of(false, &[("video", "", 0), ("pgs", "und", 0), ("pgs", "eng", 6144)]);
    let subtitles = after_untagged.streams_of(StreamKind::Subtitles);
    assert_eq!(parent_of(&subtitles, 2), None);
    assert_eq!(set_aside(&after_untagged).forced_only, vec![(2, None)]);
}

#[test]
fn parent_of_skips_derived_streams_and_streams_of_other_kinds() {
    // 1 English PGS, 2 English audio, 3 derived (2048) English PGS, 4 derived forced-only English PGS.
    let title = title_of(
        false,
        &[("video", "", 0), ("pgs", "eng", 0), ("audio", "eng", 0), ("pgs", "eng", 2048), ("pgs", "eng", 6144)],
    );
    let all: Vec<&Stream> = title.streams.iter().collect();
    assert_eq!(parent_of(&all, 4), Some(1));
    assert_eq!(parent_of(&title.streams_of(StreamKind::Subtitles), 4), Some(1));
    // A forced-only stream after another forced-only stream: the parent is still the non-derived one before both.
    let two = title_of(false, &[("video", "", 0), ("pgs", "eng", 0), ("pgs", "eng", 6144), ("pgs", "eng", 6144)]);
    assert_eq!(parent_of(&two.streams_of(StreamKind::Subtitles), 3), Some(1));
}

#[test]
fn parent_of_a_stream_not_in_the_list_is_none() {
    let title = pinned_title(SMILE, 12);
    let subtitles = title.streams_of(StreamKind::Subtitles);
    assert_eq!(parent_of(&subtitles, 99), None);
    assert_eq!(parent_of(&subtitles, 0), None);
    assert_eq!(parent_of(&Vec::<&Stream>::new(), 4), None);
    // The pinned pairs: each forced-only stream directly follows its parent, German's too.
    assert_eq!(parent_of(&subtitles, 4), Some(3));
    assert_eq!(parent_of(&subtitles, 22), Some(21));
    assert_eq!(parent_of(&subtitles, 24), Some(23));
}

#[test]
fn parent_of_treats_the_two_codes_of_one_language_as_one_language() {
    // "fra" (terminology) and "fre" (bibliographic) are both French (lang::norm_lang); "en" and "eng" both English.
    let french = title_of(false, &[("video", "", 0), ("pgs", "fra", 0), ("pgs", "fre", 6144)]);
    assert_eq!(parent_of(&french.streams_of(StreamKind::Subtitles), 2), Some(1));
    assert_eq!(set_aside(&french).other_language, vec![1, 2]);
    let english = title_of(false, &[("video", "", 0), ("pgs", "en", 0), ("pgs", "eng", 6144)]);
    assert_eq!(set_aside(&english).forced_only, vec![(2, Some(1))]);
}

#[test]
fn a_title_without_subtitle_streams_sets_nothing_aside() {
    let title = title_of(false, &[("video", "", 0), ("audio", "eng", 0), ("audio", "spa", 0)]);
    assert_eq!(
        set_aside(&title),
        SetAside {
            forced_only: vec![],
            directors_comments: vec![],
            closed_captions: None,
            other_language: vec![],
            counted: vec![],
        }
    );
}

#[test]
fn a_derived_forced_only_stream_is_set_aside_as_forced_only_even_when_flagged_directors_comments() {
    // 6145 = 4096 + 2048 + 1 on a DVD title: the forced-only test comes first.
    let title = title_of(true, &[("video", "", 0), ("vobsub", "eng", 0), ("vobsub", "eng", 6145)]);
    let aside = set_aside(&title);
    assert_eq!(aside.forced_only, vec![(2, Some(1))]);
    assert_eq!(aside.directors_comments, Vec::<u32>::new());
}

#[test]
fn only_the_first_closed_caption_stream_is_set_aside() {
    let title = title_of(true, &[("video", "", 0), ("vobsub", "eng", 0), ("cc", "eng", 0), ("cc", "eng", 0)]);
    let aside = set_aside(&title);
    assert_eq!(aside.closed_captions, Some(2));
    assert_eq!(aside.counted, vec![1, 3]);
    // A closed-caption stream in another language is that language's (8.7's first sentence).
    let french = title_of(true, &[("video", "", 0), ("vobsub", "eng", 0), ("cc", "fre", 0)]);
    assert_eq!(set_aside(&french).closed_captions, None);
    assert_eq!(set_aside(&french).other_language, vec![2]);
}

#[test]
fn the_set_aside_document_writes_its_lists_in_field_order() {
    let aside = SetAside {
        forced_only: vec![(4, Some(3)), (9, None)],
        directors_comments: vec![6],
        closed_captions: Some(10),
        other_language: vec![5],
        counted: vec![3],
    };
    let text = serde_json::to_string(&aside).unwrap();
    assert_eq!(
        text,
        r#"{"forced_only":[[4,3],[9,null]],"directors_comments":[6],"closed_captions":10,"other_language":[5],"counted":[3]}"#
    );
    assert_eq!(serde_json::from_str::<SetAside>(&text).unwrap(), aside);
}

// ------------------------------------------------------------------ events and the full rule (spec 8.7 "Events")

#[test]
fn image_packets_at_or_below_sub_clear_max_bytes_are_not_events() {
    // Two PGS events of the sizes EXP-14 measured (the smallest event 2,029 bytes), each cleared by a 30-byte packet.
    let pgs = packets(
        r#"[{"pts_time": "1.000000", "size": "2029"}, {"pts_time": "3.500000", "size": "30"},
            {"pts_time": "6.000000", "size": "4100"}, {"pts_time": "8.000000", "size": "30"}]"#,
    );
    let defaults = Settings::default();
    assert_eq!(defaults.sub_clear_max_bytes, 64);
    assert_eq!(events(&pgs, defaults.sub_clear_max_bytes), Ok(vec![1.0, 6.0]));
    let lower = Settings { sub_clear_max_bytes: 16, ..Settings::default() };
    assert_eq!(events(&pgs, lower.sub_clear_max_bytes), Ok(vec![1.0, 3.5, 6.0, 8.0]));
    // A packet of exactly the limit clears the screen: "larger than sub_clear_max_bytes" is an event.
    let at_the_event = Settings { sub_clear_max_bytes: 2029, ..Settings::default() };
    assert_eq!(events(&pgs, at_the_event.sub_clear_max_bytes), Ok(vec![6.0]));
}

#[test]
fn every_vobsub_packet_is_an_event_at_the_default_and_a_higher_limit_drops_the_small_ones() {
    // The DVD's VobSub tracks have no clear packets: their smallest packet is 148 bytes (spec 8.7 "Events").
    let vobsub = packets(
        r#"[{"pts_time": "0.500000", "size": "148"}, {"pts_time": "2.000000", "size": "1210"},
            {"pts_time": "4.000000", "size": "148"}]"#,
    );
    assert_eq!(events(&vobsub, Settings::default().sub_clear_max_bytes), Ok(vec![0.5, 2.0, 4.0]));
    let higher = Settings { sub_clear_max_bytes: 148, ..Settings::default() };
    assert_eq!(events(&vobsub, higher.sub_clear_max_bytes), Ok(vec![2.0]));
}

#[test]
fn every_text_packet_is_an_event_with_a_clear_limit_of_0() {
    // The closed captions MakeMKV converts to SubRip carry a few bytes per packet: text streams are counted with 0
    // (SubtitleFacts.events), image streams with sub_clear_max_bytes, which would drop them all.
    let text = packets(
        r#"[{"pts_time": "12.012000", "size": "12"}, {"pts_time": "15.181000", "size": "40"},
            {"pts_time": "19.019000", "size": "9"}]"#,
    );
    assert_eq!(events(&text, 0), Ok(vec![12.012, 15.181, 19.019]));
    assert_eq!(events(&text, Settings::default().sub_clear_max_bytes), Ok(vec![]));
}

#[test]
fn a_track_with_a_tenth_of_the_largest_tracks_events_is_full() {
    assert!(is_full(3, 30));
    assert!(!is_full(2, 30));
    assert!(is_full(100, 1000));
    assert!(!is_full(99, 1000));
}

#[test]
fn the_full_rule_counts_whole_events_where_a_tenth_is_not_a_whole_number() {
    // 10% of 25 is 2.5: 3 events are at least that, 2 are not.
    assert!(is_full(3, 25));
    assert!(!is_full(2, 25));
    // 10% of 1 and of 5 is less than one event: any event at all is full.
    assert!(is_full(1, 1));
    assert!(is_full(1, 5));
    // 10% of 11 is 1.1.
    assert!(is_full(2, 11));
    assert!(!is_full(1, 11));
}

#[test]
fn the_full_rule_agrees_with_the_text_rules_share_test_on_every_count_up_to_1500() {
    // subtitle::text compares `events as f64 >= rule.FULL_SHARE * largest as f64` with its own Rule's share.
    let share = TextRule::default().FULL_SHARE;
    for largest in 0..=1500usize {
        for count in 1..=largest {
            assert_eq!(is_full(count, largest), count as f64 >= share * largest as f64, "{count} of {largest}");
        }
    }
}

#[test]
fn a_track_with_no_events_is_never_full() {
    assert!(!is_full(0, 0));
    assert!(!is_full(0, 10));
    assert!(!is_full(0, 1));
}

#[test]
fn the_largest_track_is_full_and_so_is_any_track_with_events_beside_an_empty_largest() {
    assert!(is_full(30, 30));
    assert!(is_full(1520, 1520));
    // `largest` is the caller's count of the largest English track; a count above it is full too.
    assert!(is_full(5, 0));
    assert!(is_full(usize::MAX, usize::MAX));
}

// ------------------------------------------------------------------ alignment bands (spec 8.7 "Alignment")

#[test]
fn a_share_of_at_least_subs_same_min_is_the_same_group() {
    let defaults = Settings::default();
    assert_eq!(defaults.subs_same_min, 0.70);
    assert_eq!(band(&ten(), &sharing(8), &defaults), (0.8, Band::Same));
    // At the edge: 7 of 10 is 0.70, "at least subs_same_min".
    assert_eq!(band(&ten(), &sharing(7), &defaults), (0.7, Band::Same));
    assert_eq!(band(&ten(), &shifted(&ten(), 0.1), &defaults), (1.0, Band::Same));
}

#[test]
fn a_share_of_at_most_subs_diff_max_is_a_different_group() {
    let defaults = Settings::default();
    assert_eq!(defaults.subs_diff_max, 0.30);
    assert_eq!(band(&ten(), &sharing(2), &defaults), (0.2, Band::Different));
    // At the edge: 3 of 10 is 0.30, "at most subs_diff_max".
    assert_eq!(band(&ten(), &sharing(3), &defaults), (0.3, Band::Different));
    assert_eq!(band(&ten(), &sharing(0), &defaults), (0.0, Band::Different));
}

#[test]
fn a_share_between_the_two_bands_is_between() {
    let defaults = Settings::default();
    assert_eq!(band(&ten(), &sharing(5), &defaults), (0.5, Band::Between));
    assert_eq!(band(&ten(), &sharing(4), &defaults), (0.4, Band::Between));
    assert_eq!(band(&ten(), &sharing(6), &defaults), (0.6, Band::Between));
}

#[test]
fn the_larger_of_the_two_directions_decides() {
    // A forced-length track whose 3 events all start with one of the dialogue track's 10: aligned(short, long) is 1.0,
    // aligned(long, short) 0.3; the larger, 1.0, is the band's share whichever list comes first.
    let short = shifted(&[0.0, 10.0, 20.0], 0.1);
    let defaults = Settings::default();
    assert_eq!(band(&ten(), &short, &defaults), (1.0, Band::Same));
    assert_eq!(band(&short, &ten(), &defaults), (1.0, Band::Same));
}

#[test]
fn subs_same_min_moves_the_same_band() {
    let stricter = Settings { subs_same_min: 0.80, ..Settings::default() };
    assert_eq!(band(&ten(), &sharing(7), &stricter), (0.7, Band::Between));
    assert_eq!(band(&ten(), &sharing(8), &stricter), (0.8, Band::Same));
}

#[test]
fn subs_diff_max_moves_the_different_band() {
    assert_eq!(band(&ten(), &sharing(4), &Settings::default()), (0.4, Band::Between));
    let wider = Settings { subs_diff_max: 0.40, ..Settings::default() };
    assert_eq!(band(&ten(), &sharing(4), &wider), (0.4, Band::Different));
    assert_eq!(band(&ten(), &sharing(5), &wider), (0.5, Band::Between));
}

#[test]
fn subs_tolerance_ms_sets_how_far_apart_two_starts_may_be() {
    let defaults = Settings::default();
    assert_eq!(defaults.subs_tolerance_ms, 300);
    assert_eq!(band(&ten(), &shifted(&ten(), 0.2), &defaults), (1.0, Band::Same));
    assert_eq!(band(&ten(), &shifted(&ten(), 0.4), &defaults), (0.0, Band::Different));
    let looser = Settings { subs_tolerance_ms: 500, ..Settings::default() };
    assert_eq!(band(&ten(), &shifted(&ten(), 0.4), &looser), (1.0, Band::Same));
    let tighter = Settings { subs_tolerance_ms: 100, ..Settings::default() };
    assert_eq!(band(&ten(), &shifted(&ten(), 0.2), &tighter), (0.0, Band::Different));
}

#[test]
fn tracks_without_events_are_in_different_groups() {
    let defaults = Settings::default();
    assert_eq!(band(&[], &[], &defaults), (0.0, Band::Different));
    assert_eq!(band(&ten(), &[], &defaults), (0.0, Band::Different));
    assert_eq!(band(&[], &ten(), &defaults), (0.0, Band::Different));
}

#[test]
fn starts_given_out_of_order_give_the_same_band() {
    let defaults = Settings::default();
    let mut backwards = ten();
    backwards.reverse();
    let mut half = sharing(5);
    half.reverse();
    assert_eq!(band(&backwards, &half, &defaults), (0.5, Band::Between));
    assert_eq!(band(&half, &backwards, &defaults), band(&ten(), &sharing(5), &defaults));
}

// ------------------------------------------------------------------ alignment groups (spec 8.7 "Alignment")

#[test]
fn two_full_tracks_with_the_same_timing_form_one_group() {
    let regular = ten();
    let sdh = shifted(&ten(), 0.1);
    let full: Vec<(u32, &[f64])> = vec![(2, regular.as_slice()), (3, sdh.as_slice())];
    assert_eq!(alignment_groups(&full, &Settings::default()), (vec![vec![2, 3]], false));
}

#[test]
fn tracks_with_different_timing_form_groups_in_order_of_their_first_stream() {
    // 2 and 7 share their timing (1.0); 5 shares 0.2 with each: Different.
    let (two, five, seven) = (ten(), sharing(2), shifted(&ten(), 0.2));
    let full: Vec<(u32, &[f64])> = vec![(2, two.as_slice()), (5, five.as_slice()), (7, seven.as_slice())];
    assert_eq!(alignment_groups(&full, &Settings::default()), (vec![vec![2, 7], vec![5]], false));
}

#[test]
fn an_in_between_pair_keeps_its_tracks_apart_and_is_ambiguous() {
    let (two, five) = (ten(), sharing(5));
    let full: Vec<(u32, &[f64])> = vec![(2, two.as_slice()), (5, five.as_slice())];
    assert_eq!(alignment_groups(&full, &Settings::default()), (vec![vec![2], vec![5]], true));
}

#[test]
fn the_same_band_groups_tracks_transitively() {
    // 2 starts on the tens, 3 five seconds later; 4 holds both sets of starts. 2 and 3 share nothing (Different), but
    // each is wholly inside 4 (1.0, Same), so the three are one group.
    let (two, three) = (ten(), shifted(&ten(), 5.0));
    let mut four = ten();
    four.extend(shifted(&ten(), 5.0));
    let full: Vec<(u32, &[f64])> = vec![(2, two.as_slice()), (3, three.as_slice()), (4, four.as_slice())];
    assert_eq!(alignment_groups(&full, &Settings::default()), (vec![vec![2, 3, 4]], false));
}

#[test]
fn no_full_track_gives_no_group_and_one_gives_one() {
    assert_eq!(alignment_groups(&Vec::<(u32, &[f64])>::new(), &Settings::default()), (Vec::<Vec<u32>>::new(), false));
    let only = ten();
    let full: Vec<(u32, &[f64])> = vec![(4, only.as_slice())];
    assert_eq!(alignment_groups(&full, &Settings::default()), (vec![vec![4]], false));
}

#[test]
fn the_groups_follow_the_settings_bands() {
    let (two, three) = (ten(), sharing(7));
    let full: Vec<(u32, &[f64])> = vec![(2, two.as_slice()), (3, three.as_slice())];
    assert_eq!(alignment_groups(&full, &Settings::default()), (vec![vec![2, 3]], false));
    let stricter = Settings { subs_same_min: 0.80, ..Settings::default() };
    assert_eq!(alignment_groups(&full, &stricter), (vec![vec![2], vec![3]], true));
    let wider = Settings { subs_same_min: 0.80, subs_diff_max: 0.70, ..Settings::default() };
    assert_eq!(alignment_groups(&full, &wider), (vec![vec![2], vec![3]], false));
}

// ------------------------------------------------------------------ the dialogue group (spec 8.7 "Dialogue group")

/// No description for any stream.
fn undescribed() -> BTreeMap<u32, Vec<DescriptionKind>> {
    BTreeMap::new()
}

#[test]
fn one_full_track_is_the_dialogue_track() {
    let groups = vec![vec![4]];
    assert_eq!(dialogue_group(&groups, &[], &undescribed()), Some((0, DialoguePick::OneFull)));
    // The first pick: neither a forced parent elsewhere nor a description changes it.
    let described = BTreeMap::from([(4, vec![DescriptionKind::Commentary])]);
    assert_eq!(dialogue_group(&groups, &[9], &described), Some((0, DialoguePick::OneFull)));
}

#[test]
fn the_group_holding_a_forced_only_streams_parent_is_the_dialogue_group() {
    // The parent's group wins over a larger group.
    let groups = vec![vec![2, 3], vec![5]];
    assert_eq!(dialogue_group(&groups, &[5], &undescribed()), Some((1, DialoguePick::ForcedParent)));
}

#[test]
fn the_forced_parent_pick_comes_before_the_description_pick() {
    let groups = vec![vec![2], vec![5]];
    let described = BTreeMap::from([(5, vec![DescriptionKind::Sdh])]);
    assert_eq!(dialogue_group(&groups, &[2], &described), Some((0, DialoguePick::ForcedParent)));
}

#[test]
fn a_group_thediscdb_describes_as_subtitles_is_the_dialogue_group() {
    let groups = vec![vec![2], vec![5]];
    let described = BTreeMap::from([(5, vec![DescriptionKind::Dialogue])]);
    assert_eq!(dialogue_group(&groups, &[], &described), Some((1, DialoguePick::Described)));
}

#[test]
fn a_group_thediscdb_describes_as_sdh_is_the_dialogue_group() {
    let groups = vec![vec![2], vec![5]];
    let described = BTreeMap::from([(2, vec![DescriptionKind::Sdh])]);
    assert_eq!(dialogue_group(&groups, &[], &described), Some((0, DialoguePick::Described)));
}

#[test]
fn the_description_pick_comes_before_the_largest_group() {
    let groups = vec![vec![2, 3], vec![5]];
    let described = BTreeMap::from([(5, vec![DescriptionKind::Dialogue])]);
    assert_eq!(dialogue_group(&groups, &[], &described), Some((1, DialoguePick::Described)));
}

#[test]
fn other_descriptions_pick_no_group() {
    let groups = vec![vec![2], vec![5]];
    let described = BTreeMap::from([
        (2, vec![DescriptionKind::Commentary]),
        (5, vec![DescriptionKind::AudioDescription, DescriptionKind::IsolatedScore, DescriptionKind::MusicAndEffects]),
    ]);
    assert_eq!(dialogue_group(&groups, &[], &described), None);
    // A stream described as commentary and SDH still says SDH.
    let both = BTreeMap::from([(5, vec![DescriptionKind::Commentary, DescriptionKind::Sdh])]);
    assert_eq!(dialogue_group(&groups, &[], &both), Some((1, DialoguePick::Described)));
}

#[test]
fn the_largest_group_of_two_or_more_tracks_is_the_dialogue_group() {
    let groups = vec![vec![2], vec![3, 5], vec![7]];
    assert_eq!(dialogue_group(&groups, &[], &undescribed()), Some((1, DialoguePick::Largest)));
    let three = vec![vec![2, 3], vec![5, 6, 7]];
    assert_eq!(dialogue_group(&three, &[], &undescribed()), Some((1, DialoguePick::Largest)));
}

#[test]
fn equal_largest_groups_pick_nothing() {
    // Two groups of two: there is no single largest group, and disc position never chooses one (D15), so Task 10 raises
    // subtitle_ambiguous.
    let tied = vec![vec![2, 3], vec![5, 6]];
    assert_eq!(dialogue_group(&tied, &[], &undescribed()), None);
    // A tie below the largest size does not matter: the one group of three is the largest.
    let one_largest = vec![vec![2, 3], vec![5, 6], vec![7, 8, 9]];
    assert_eq!(dialogue_group(&one_largest, &[], &undescribed()), Some((2, DialoguePick::Largest)));
}

#[test]
fn one_group_of_two_tracks_is_the_largest() {
    let groups = vec![vec![2, 3]];
    assert_eq!(dialogue_group(&groups, &[], &undescribed()), Some((0, DialoguePick::Largest)));
}

#[test]
fn single_track_groups_without_other_evidence_pick_nothing() {
    let groups = vec![vec![2], vec![5]];
    assert_eq!(dialogue_group(&groups, &[], &undescribed()), None);
}

#[test]
fn no_group_picks_nothing() {
    assert_eq!(dialogue_group(&[], &[], &undescribed()), None);
    assert_eq!(dialogue_group(&[], &[2], &BTreeMap::from([(2, vec![DescriptionKind::Sdh])])), None);
}

#[test]
fn forced_parents_in_two_groups_are_ambiguous_but_split_descriptions_fall_through() {
    // Forced parents in groups 0 and 1 contradict each other: no group is picked (subtitle_ambiguous), even though
    // group 2 is larger.
    let groups = vec![vec![2], vec![5], vec![6, 7]];
    assert_eq!(dialogue_group(&groups, &[2, 5], &undescribed()), None);
    // Two parents in one group still pick it.
    let together = vec![vec![2, 3], vec![5]];
    assert_eq!(dialogue_group(&together, &[2, 3], &undescribed()), Some((0, DialoguePick::ForcedParent)));
    // Descriptions in both single-track groups: nothing is picked (no group has two tracks).
    let pair = vec![vec![2], vec![5]];
    let described =
        BTreeMap::from([(2, vec![DescriptionKind::Sdh]), (5, vec![DescriptionKind::Dialogue])]);
    assert_eq!(dialogue_group(&pair, &[], &described), None);
    // Descriptions in two groups name no group, so the largest group decides (`groups` is [[2], [5], [6, 7]] above).
    assert_eq!(dialogue_group(&groups, &[], &described), Some((2, DialoguePick::Largest)));
}

#[test]
fn a_forced_parent_or_a_description_outside_every_group_picks_nothing() {
    // Stream 9 is not full, so it is in no group.
    let groups = vec![vec![2], vec![5]];
    assert_eq!(dialogue_group(&groups, &[9], &undescribed()), None);
    let described = BTreeMap::from([(9, vec![DescriptionKind::Sdh])]);
    assert_eq!(dialogue_group(&groups, &[], &described), None);
    let uneven = vec![vec![2], vec![5, 6]];
    assert_eq!(dialogue_group(&uneven, &[9], &described), Some((1, DialoguePick::Largest)));
}

// ------------------------------------------------------------------ the evidence end to end

/// The parents of the forced-only streams that have events (8.7 "Dialogue group": "the parent of a forced-only stream
/// that has events"), as Task 10's caller builds `forced_parents`.
fn forced_parents(aside: &SetAside, events: &BTreeMap<u32, Vec<f64>>) -> Vec<u32> {
    aside
        .forced_only
        .iter()
        .filter(|(stream, _)| events.get(stream).is_some_and(|starts| !starts.is_empty()))
        .filter_map(|&(_, parent)| parent)
        .collect()
}

/// The counted streams that are full against the largest counted stream (8.7 "Events"), with their starts, in scan
/// order; a counted stream absent from `events` has no events.
fn full_tracks<'e>(aside: &SetAside, events: &'e BTreeMap<u32, Vec<f64>>) -> Vec<(u32, &'e [f64])> {
    let count = |stream: &u32| events.get(stream).map_or(0, Vec::len);
    let largest = aside.counted.iter().map(count).max().unwrap_or(0);
    aside
        .counted
        .iter()
        .filter(|&stream| is_full(count(stream), largest))
        .map(|stream| (*stream, events.get(stream).map(Vec::as_slice).unwrap_or(&[])))
        .collect()
}

#[test]
fn d20s_regular_and_sdh_tracks_share_the_dialogue_group_and_a_commentary_track_stands_apart() {
    // A Blu-ray title: 2 a regular track, 3 its SDH track (the same starts plus two sound cues), 4 a commentary track,
    // 5 a derived forced-only stream (parent 4) that has no events, 6 Spanish.
    let title = title_of(
        false,
        &[
            ("video", "", 0),
            ("audio", "eng", 0),
            ("pgs", "eng", 0),
            ("pgs", "eng", 0),
            ("pgs", "eng", 0),
            ("pgs", "eng", 6144),
            ("pgs", "spa", 0),
        ],
    );
    let aside = set_aside(&title);
    assert_eq!(aside.forced_only, vec![(5, Some(4))]);
    assert_eq!(aside.other_language, vec![6]);
    assert_eq!(aside.counted, vec![2, 3, 4]);

    let mut sdh = ten();
    sdh.extend([5.0, 15.0]);
    let events: BTreeMap<u32, Vec<f64>> = BTreeMap::from([(2, ten()), (3, sdh), (4, sharing(2)), (5, vec![])]);
    let full = full_tracks(&aside, &events);
    assert_eq!(full.iter().map(|(stream, _)| *stream).collect::<Vec<u32>>(), vec![2, 3, 4]);
    let (groups, ambiguous) = alignment_groups(&full, &Settings::default());
    assert_eq!(groups, vec![vec![2, 3], vec![4]]);
    assert!(!ambiguous);
    let parents = forced_parents(&aside, &events);
    assert_eq!(parents, Vec::<u32>::new());
    assert_eq!(dialogue_group(&groups, &parents, &BTreeMap::new()), Some((0, DialoguePick::Largest)));
}

#[test]
fn smiles_forced_only_stream_with_events_picks_its_parents_group() {
    // Smile's title 12: English 3 and 21 are counted, each with a forced-only stream (4 and 22). With made-up starts,
    // 3 and 21 share 0.1 of their timing (Different groups); 4 has an event, so its parent 3 picks the group.
    let title = pinned_title(SMILE, 12);
    let aside = set_aside(&title);
    assert_eq!(aside.counted, vec![3, 21]);
    let events: BTreeMap<u32, Vec<f64>> =
        BTreeMap::from([(3, ten()), (4, vec![12.0]), (21, sharing(1)), (22, vec![])]);
    let full = full_tracks(&aside, &events);
    let (groups, ambiguous) = alignment_groups(&full, &Settings::default());
    assert_eq!(groups, vec![vec![3], vec![21]]);
    assert!(!ambiguous);
    assert_eq!(forced_parents(&aside, &events), vec![3]);
    assert_eq!(
        dialogue_group(&groups, &forced_parents(&aside, &events), &BTreeMap::new()),
        Some((0, DialoguePick::ForcedParent))
    );
    // Every forced-only stream empty (as on every disc recorded so far): no pick decides, and Task 10 raises
    // subtitle_ambiguous.
    let quiet: BTreeMap<u32, Vec<f64>> = BTreeMap::from([(3, ten()), (4, vec![]), (21, sharing(1)), (22, vec![])]);
    assert_eq!(forced_parents(&aside, &quiet), Vec::<u32>::new());
    assert_eq!(dialogue_group(&groups, &forced_parents(&aside, &quiet), &BTreeMap::new()), None);
}

/// Task 10: spec 8.7 from "Regular and SDH" to the confidence table, and 8.7 "Text". Every title is made up (Task 9's
/// `title_of`) or a pinned TheDiscDB log (Task 9's `pinned_title`); every event series is made up, with the counts and
/// the aligned shares its doc comment names.
mod roles {
    use std::collections::{BTreeMap, BTreeSet};

    use serde_json::json;

    use super::{pinned_title, title_of, FIREFLY_1, KNOWING, SMILE};
    use crate::disc::Title;
    use crate::lookup::discdb::DiscDbTitle;
    use crate::remembered::{signature, SubtitleOverride, SubtitleOverrideEntry};
    use crate::review::{Note, Noted, ReviewReason};
    use crate::settings::Settings;
    use crate::subtitle::text::{Decision, Role};
    use crate::tracks::subtitle::{
        confidence, decide_subtitles, text_disagreements, Band, DialoguePick, SubtitleCase, SubtitleDecision, SubtitleDrop,
        SubtitleFacts, SubtitleTrack, SUBTITLE_ALIGNMENT_CONFIDENCE, SUBTITLE_AMBIGUOUS_CONFIDENCE, SUBTITLE_FLAGS_CONFIDENCE,
        SUBTITLE_ONE_FULL_CONFIDENCE,
    };
    use crate::tracks::SubtitleRole;

    #[test]
    fn the_confidence_table_gives_each_case_its_value() {
        assert_eq!(SUBTITLE_FLAGS_CONFIDENCE, 0.95);
        assert_eq!(SUBTITLE_ONE_FULL_CONFIDENCE, 0.90);
        assert_eq!(SUBTITLE_ALIGNMENT_CONFIDENCE, 0.85);
        assert_eq!(SUBTITLE_AMBIGUOUS_CONFIDENCE, 0.60);
        assert_eq!(confidence(SubtitleCase::FlagsOrDescriptions), 0.95);
        assert_eq!(confidence(SubtitleCase::NoEnglish), 0.95);
        assert_eq!(confidence(SubtitleCase::OneFull), 0.90);
        assert_eq!(confidence(SubtitleCase::Alignment), 0.85);
        assert_eq!(confidence(SubtitleCase::Ambiguous), 0.60);
    }

    #[test]
    fn the_confidence_cases_are_spelt_in_snake_case_and_read_back_only_so() {
        let spellings = ["flags_or_descriptions", "no_english", "one_full", "alignment", "ambiguous"];
        for (case, spelling) in SubtitleCase::ALL.into_iter().zip(spellings) {
            assert_eq!(case.as_str(), spelling);
            assert_eq!(serde_json::to_value(case).unwrap(), json!(spelling));
            assert_eq!(serde_json::from_value::<SubtitleCase>(json!(spelling)).unwrap(), case);
            assert_eq!(spelling.parse::<SubtitleCase>(), Ok(case));
        }
        assert_eq!("one-full".parse::<SubtitleCase>(), Err("not a subtitle confidence case: one-full".to_string()));
        assert!(serde_json::from_value::<SubtitleCase>(json!("OneFull")).is_err());
    }

    /// A kept track as `decide_subtitles` builds it: the role's name and flags, not default, not closed captions, no parent.
    fn track(stream: u32, role: SubtitleRole, events: Option<usize>, decided_by: &str) -> SubtitleTrack {
        SubtitleTrack {
            stream,
            role,
            name: role.name().to_string(),
            forced: role == SubtitleRole::Forced,
            hearing_impaired: role == SubtitleRole::Sdh,
            default: false,
            events,
            closed_captions: false,
            parent: None,
            decided_by: decided_by.to_string(),
        }
    }

    /// The text rule's decision for one stream, with no review and no note.
    fn verdict(role: Role) -> Decision {
        Decision { role, review: None, note: None }
    }

    /// A decision keeping stream 1 as regular and stream 2 as SDH, dropping stream 3 (a full track outside the dialogue
    /// group) and stream 4 (Spanish).
    fn two_kept() -> SubtitleDecision {
        SubtitleDecision {
            kept: vec![
                track(1, SubtitleRole::Regular, Some(1000), "alignment"),
                track(2, SubtitleRole::Sdh, Some(1200), "alignment"),
            ],
            dropped: vec![(3, SubtitleDrop::OutsideDialogueGroup), (4, SubtitleDrop::OtherLanguage)],
            set_aside: vec![],
            full: vec![1, 2, 3],
            groups: vec![vec![1, 2], vec![3]],
            dialogue_group: Some(0),
            dialogue_pick: Some(DialoguePick::Largest),
            pairs: vec![(1, 2, 1.0, Band::Same), (1, 3, 0.0, Band::Different), (2, 3, 0.0, Band::Different)],
            case: SubtitleCase::Alignment,
            confidence: SUBTITLE_ALIGNMENT_CONFIDENCE,
            notes: vec![],
            reviews: vec![],
        }
    }

    #[test]
    fn set_aside_verdicts_never_disagree_and_keep_always_does() {
        // text::decide writes Keep only with its own review: the text asks for a review that the counts did not raise,
        // on a kept track (1) and on a dropped one (3) alike. SetAside (2) never disagrees.
        let decision = two_kept();
        let text = BTreeMap::from([(1, verdict(Role::Keep)), (2, verdict(Role::SetAside)), (3, verdict(Role::Keep))]);
        assert_eq!(text_disagreements(&decision, &text), vec![1, 3]);
        // A verdict carrying the text rule's review disagrees even when its role is the kept one (text::before_yes).
        let reviewed = BTreeMap::from([(
            1,
            Decision { role: Role::Regular, review: Some("subtitle_ambiguous".to_string()), note: None },
        )]);
        assert_eq!(text_disagreements(&decision, &reviewed), vec![1]);
        let set_aside = BTreeMap::from([(
            2,
            Decision { role: Role::SetAside, review: Some("subtitle_ambiguous".to_string()), note: None },
        )]);
        assert_eq!(text_disagreements(&decision, &set_aside), Vec::<u32>::new());
    }

    #[test]
    fn a_text_role_other_than_the_kept_role_disagrees() {
        let decision = two_kept();
        let same = BTreeMap::from([(1, verdict(Role::Regular)), (2, verdict(Role::Sdh))]);
        assert_eq!(text_disagreements(&decision, &same), Vec::<u32>::new());
        let swapped = BTreeMap::from([(1, verdict(Role::Sdh)), (2, verdict(Role::Regular))]);
        assert_eq!(text_disagreements(&decision, &swapped), vec![1, 2]);
        let forced = BTreeMap::from([(1, verdict(Role::Forced)), (2, verdict(Role::Sdh))]);
        assert_eq!(text_disagreements(&decision, &forced), vec![1]);
    }

    #[test]
    fn a_text_drop_disagrees_with_a_kept_track_and_a_text_role_with_a_dropped_one() {
        let decision = two_kept();
        let drop_kept = BTreeMap::from([(1, verdict(Role::Drop))]);
        assert_eq!(text_disagreements(&decision, &drop_kept), vec![1]);
        let drop_dropped = BTreeMap::from([(3, verdict(Role::Drop))]);
        assert_eq!(text_disagreements(&decision, &drop_dropped), Vec::<u32>::new());
        let keep_dropped = BTreeMap::from([(3, verdict(Role::Regular))]);
        assert_eq!(text_disagreements(&decision, &keep_dropped), vec![3]);
    }

    #[test]
    fn streams_in_other_languages_or_outside_the_decision_are_never_compared() {
        let decision = two_kept();
        let text = BTreeMap::from([(4, verdict(Role::Regular)), (9, verdict(Role::Sdh))]);
        assert_eq!(text_disagreements(&decision, &text), Vec::<u32>::new());
    }

    // Streams as Task 9's `title_of` takes them: (kind, language, flags attribute 22).
    const VIDEO: (&str, &str, u32) = ("video", "", 0);
    const ENG_PGS: (&str, &str, u32) = ("pgs", "eng", 0);
    const ENG_PGS_FORCED_ONLY: (&str, &str, u32) = ("pgs", "eng", 6144);
    const SPA_PGS: (&str, &str, u32) = ("pgs", "spa", 0);
    const ENG_VOBSUB: (&str, &str, u32) = ("vobsub", "eng", 0);
    const ENG_VOBSUB_FORCED: (&str, &str, u32) = ("vobsub", "eng", 4096);
    const ENG_VOBSUB_FORCED_ONLY: (&str, &str, u32) = ("vobsub", "eng", 6144);
    const SPA_VOBSUB: (&str, &str, u32) = ("vobsub", "spa", 0);
    const ENG_CC: (&str, &str, u32) = ("cc", "eng", 0);

    /// Title 0 of a made-up Blu-ray scan holding `streams`, numbered from 0 (Task 9's `title_of`).
    fn bluray(streams: &[(&str, &str, u32)]) -> Title {
        title_of(false, streams)
    }

    /// Title 0 of a made-up DVD scan (attribute 24 "01", no attribute 16) holding `streams`.
    fn dvd(streams: &[(&str, &str, u32)]) -> Title {
        title_of(true, streams)
    }

    /// `count` start times, the first at `first` seconds, one every `step` seconds.
    fn times(count: usize, first: f64, step: f64) -> Vec<f64> {
        (0..count).map(|k| first + step * k as f64).collect()
    }

    /// Two series as one, sorted.
    fn merged(mut a: Vec<f64>, b: Vec<f64>) -> Vec<f64> {
        a.extend(b);
        a.sort_by(|x, y| x.partial_cmp(y).unwrap());
        a
    }

    /// 1,000 dialogue events, one every 6 s from 10 s.
    fn regular_events() -> Vec<f64> {
        times(1000, 10.0, 6.0)
    }

    /// The dialogue's 1,000 events and 200 sound cues 3 s after the first 200 of them: 1,200 events. Against
    /// `regular_events` the larger aligned share is 1.0 (every dialogue event starts at an SDH event).
    fn sdh_events() -> Vec<f64> {
        merged(regular_events(), times(200, 13.0, 6.0))
    }

    /// 800 events, each 2.5 s after a dialogue event: no start within 300 ms of `regular_events`' or `sdh_events`'
    /// (aligned share 0.0), every start within 3 s of one (aligned share 1.0 under a 3,000 ms tolerance).
    fn commentary_events() -> Vec<f64> {
        times(800, 12.5, 6.0)
    }

    /// 20 events of a forced track, one a minute from 100 s.
    fn forced_events() -> Vec<f64> {
        times(20, 100.0, 60.0)
    }

    /// Two full tracks whose larger aligned share is exactly 0.5: the dialogue's 1,000 events, and 1,100 events whose
    /// first 500 are the dialogue's first 500 and whose other 600 start 2.5 s off it. aligned(A, B) = 500/1000 = 0.5,
    /// aligned(B, A) = 500/1100; 0.5 lies between subs_diff_max 0.30 and subs_same_min 0.70.
    fn between_events() -> (Vec<f64>, Vec<f64>) {
        (regular_events(), merged(times(500, 10.0, 6.0), times(600, 3012.5, 6.0)))
    }

    /// Facts with these event series and nothing else.
    fn facts<'a>(events: Vec<(u32, Vec<f64>)>) -> SubtitleFacts<'a> {
        SubtitleFacts { events: events.into_iter().collect(), ..SubtitleFacts::default() }
    }

    /// The set-aside sentence of a derived forced-only stream whose parent is `parent`.
    fn forced_only_of(parent: u32) -> String {
        format!("derived forced-only stream of stream {parent} (flags 4096 and 2048; 8.7 \"Set aside first\")")
    }

    /// (stream, role, decided_by) of every kept track, in the decision's order.
    fn kept_roles(decision: &SubtitleDecision) -> Vec<(u32, SubtitleRole, String)> {
        decision.kept.iter().map(|kept| (kept.stream, kept.role, kept.decided_by.clone())).collect()
    }

    #[test]
    fn smile_title_14_keeps_the_forced_only_stream_of_the_regular_track_then_regular_and_sdh() {
        // Smile's main feature (title 14, 00040.mpls) lists English PGS streams 11, 13 and 31, each followed by its
        // derived forced-only stream (12, 14, 32: flags 6144), and 30 streams in seven other languages (15 to 30 and 33
        // to 46: German, Spanish, French, Italian, Japanese, Dutch, Turkish). The events are made up: 11 is the
        // dialogue, 13 the same dialogue with sound cues, 31 a commentary off the dialogue's timing; 12 has 20 events,
        // MakeMKV removed 14, and 32 has none.
        let title = pinned_title(SMILE, 14);
        let evidence = SubtitleFacts {
            removed: BTreeSet::from([14]),
            ..facts(vec![(11, regular_events()), (12, forced_events()), (13, sdh_events()), (31, commentary_events())])
        };
        let decision = decide_subtitles(&title, &evidence, &Settings::default());
        let dropped: Vec<(u32, SubtitleDrop)> = (14..=46)
            .map(|stream| {
                let why = match stream {
                    14 | 32 => SubtitleDrop::DerivedForcedOnlyEmpty,
                    31 => SubtitleDrop::OutsideDialogueGroup,
                    _ => SubtitleDrop::OtherLanguage,
                };
                (stream, why)
            })
            .collect();
        assert_eq!(
            decision,
            SubtitleDecision {
                kept: vec![
                    SubtitleTrack { parent: Some(11), ..track(12, SubtitleRole::Forced, Some(20), "flag") },
                    track(11, SubtitleRole::Regular, Some(1000), "alignment"),
                    track(13, SubtitleRole::Sdh, Some(1200), "alignment"),
                ],
                dropped,
                set_aside: vec![(12, forced_only_of(11)), (14, forced_only_of(13)), (32, forced_only_of(31))],
                full: vec![11, 13, 31],
                groups: vec![vec![11, 13], vec![31]],
                dialogue_group: Some(0),
                dialogue_pick: Some(DialoguePick::ForcedParent),
                pairs: vec![(11, 13, 1.0, Band::Same), (11, 31, 0.0, Band::Different), (13, 31, 0.0, Band::Different)],
                case: SubtitleCase::Alignment,
                confidence: 0.85,
                notes: vec![],
                reviews: vec![],
            }
        );
    }

    #[test]
    fn two_aligned_full_tracks_name_the_one_with_more_events_sdh() {
        let title = bluray(&[VIDEO, ENG_PGS, ENG_PGS, SPA_PGS]);
        let decision =
            decide_subtitles(&title, &facts(vec![(1, regular_events()), (2, sdh_events())]), &Settings::default());
        assert_eq!(
            decision,
            SubtitleDecision {
                kept: vec![
                    track(1, SubtitleRole::Regular, Some(1000), "alignment"),
                    track(2, SubtitleRole::Sdh, Some(1200), "alignment"),
                ],
                dropped: vec![(3, SubtitleDrop::OtherLanguage)],
                set_aside: vec![],
                full: vec![1, 2],
                groups: vec![vec![1, 2]],
                dialogue_group: Some(0),
                dialogue_pick: Some(DialoguePick::Largest),
                pairs: vec![(1, 2, 1.0, Band::Same)],
                case: SubtitleCase::Alignment,
                confidence: 0.85,
                notes: vec![],
                reviews: vec![],
            }
        );
        let regular = &decision.kept[0];
        assert_eq!((regular.name.as_str(), regular.forced, regular.hearing_impaired), ("English", false, false));
        let sdh = &decision.kept[1];
        assert_eq!((sdh.name.as_str(), sdh.forced, sdh.hearing_impaired), ("English (SDH)", false, true));
        assert!(decision.kept.iter().all(|kept| !kept.default && !kept.closed_captions && kept.parent.is_none()));
    }

    #[test]
    fn a_thediscdb_description_saying_sdh_overrides_the_count() {
        // TheDiscDB lists video and the two non-derived PGS streams (it aligns without the derived stream 2) and
        // describes stream 1 as "English SDH" (Sdh) and stream 3 as "English" (Dialogue). Stream 1 has fewer events.
        let title = bluray(&[VIDEO, ENG_PGS, ENG_PGS_FORCED_ONLY, ENG_PGS]);
        let record: DiscDbTitle = serde_json::from_value(json!({
            "index": 0,
            "source_file": "00800.mpls",
            "segment_map": "1",
            "duration": "1:40:00",
            "tracks": [
                {"index": 0, "type": "Video", "name": "Mpeg4 AVC High@L4.1"},
                {"index": 1, "type": "Subtitles", "name": "HDMV PGS Subtitles", "language_code": "eng",
                 "language": "English", "description": "English SDH"},
                {"index": 2, "type": "Subtitles", "name": "HDMV PGS Subtitles", "language_code": "eng",
                 "language": "English", "description": "English"}
            ]
        }))
        .unwrap();
        let evidence = SubtitleFacts { discdb: Some(&record), ..facts(vec![(1, regular_events()), (3, sdh_events())]) };
        let decision = decide_subtitles(&title, &evidence, &Settings::default());
        assert_eq!(
            decision,
            SubtitleDecision {
                kept: vec![
                    track(3, SubtitleRole::Regular, Some(1200), "description"),
                    track(1, SubtitleRole::Sdh, Some(1000), "description"),
                ],
                dropped: vec![(2, SubtitleDrop::DerivedForcedOnlyEmpty)],
                set_aside: vec![(2, forced_only_of(1))],
                full: vec![1, 3],
                groups: vec![vec![1, 3]],
                dialogue_group: Some(0),
                dialogue_pick: Some(DialoguePick::Described),
                pairs: vec![(1, 3, 1.0, Band::Same)],
                case: SubtitleCase::FlagsOrDescriptions,
                confidence: 0.95,
                notes: vec![],
                reviews: vec![],
            }
        );
        // Without the record the count decides: stream 3 has more events.
        let counted = decide_subtitles(&title, &facts(vec![(1, regular_events()), (3, sdh_events())]), &Settings::default());
        assert_eq!(
            kept_roles(&counted),
            vec![(1, SubtitleRole::Regular, "alignment".to_string()), (3, SubtitleRole::Sdh, "alignment".to_string())]
        );
        assert_eq!(counted.dialogue_pick, Some(DialoguePick::Largest));
        assert_eq!((counted.case, counted.confidence), (SubtitleCase::Alignment, 0.85));
    }

    #[test]
    fn a_one_track_group_described_sdh_is_sdh() {
        // TheDiscDB lists video, the English PGS stream described "English SDH" and the Spanish one described "Spanish".
        // Stream 1 is the one full track: its description makes it SDH, not the count.
        let title = bluray(&[VIDEO, ENG_PGS, SPA_PGS]);
        let record: DiscDbTitle = serde_json::from_value(json!({
            "index": 0,
            "source_file": "00800.mpls",
            "segment_map": "1",
            "duration": "1:00:00",
            "tracks": [
                {"index": 0, "type": "Video", "name": "Mpeg4 AVC High@L4.1"},
                {"index": 1, "type": "Subtitles", "name": "HDMV PGS Subtitles", "language_code": "eng",
                 "language": "English", "description": "English SDH"},
                {"index": 2, "type": "Subtitles", "name": "HDMV PGS Subtitles", "language_code": "spa",
                 "language": "Spanish", "description": "Spanish"}
            ]
        }))
        .unwrap();
        let evidence = SubtitleFacts { discdb: Some(&record), ..facts(vec![(1, regular_events())]) };
        let decision = decide_subtitles(&title, &evidence, &Settings::default());
        assert_eq!(
            decision,
            SubtitleDecision {
                kept: vec![track(1, SubtitleRole::Sdh, Some(1000), "description")],
                dropped: vec![(2, SubtitleDrop::OtherLanguage)],
                set_aside: vec![],
                full: vec![1],
                groups: vec![vec![1]],
                dialogue_group: Some(0),
                dialogue_pick: Some(DialoguePick::OneFull),
                pairs: vec![],
                case: SubtitleCase::FlagsOrDescriptions,
                confidence: 0.95,
                notes: vec![],
                reviews: vec![],
            }
        );
        let sdh = &decision.kept[0];
        assert_eq!((sdh.name.as_str(), sdh.forced, sdh.hearing_impaired), ("English (SDH)", false, true));
    }

    #[test]
    fn a_description_saying_sdh_on_both_tracks_leaves_the_count_to_decide() {
        // TheDiscDB describes both English PGS streams as "English SDH": the descriptions pick the dialogue group but
        // cannot tell the two apart, so the one with more events (stream 2) is SDH.
        let title = bluray(&[VIDEO, ENG_PGS, ENG_PGS]);
        let record: DiscDbTitle = serde_json::from_value(json!({
            "index": 0,
            "source_file": "00800.mpls",
            "segment_map": "1",
            "duration": "1:00:00",
            "tracks": [
                {"index": 0, "type": "Video", "name": "Mpeg4 AVC High@L4.1"},
                {"index": 1, "type": "Subtitles", "name": "HDMV PGS Subtitles", "language_code": "eng",
                 "language": "English", "description": "English SDH"},
                {"index": 2, "type": "Subtitles", "name": "HDMV PGS Subtitles", "language_code": "eng",
                 "language": "English", "description": "English SDH"}
            ]
        }))
        .unwrap();
        let evidence = SubtitleFacts { discdb: Some(&record), ..facts(vec![(1, regular_events()), (2, sdh_events())]) };
        let decision = decide_subtitles(&title, &evidence, &Settings::default());
        assert_eq!(
            kept_roles(&decision),
            vec![(1, SubtitleRole::Regular, "alignment".to_string()), (2, SubtitleRole::Sdh, "alignment".to_string())]
        );
        assert_eq!(decision.dialogue_pick, Some(DialoguePick::Described));
        assert_eq!((decision.case, decision.confidence, decision.reviews.clone()), (SubtitleCase::Alignment, 0.85, vec![]));
    }

    #[test]
    fn exactly_one_full_track_is_the_regular_dialogue_track_at_090() {
        let title = bluray(&[VIDEO, ENG_PGS, SPA_PGS]);
        let decision = decide_subtitles(&title, &facts(vec![(1, regular_events())]), &Settings::default());
        assert_eq!(
            decision,
            SubtitleDecision {
                kept: vec![track(1, SubtitleRole::Regular, Some(1000), "count")],
                dropped: vec![(2, SubtitleDrop::OtherLanguage)],
                set_aside: vec![],
                full: vec![1],
                groups: vec![vec![1]],
                dialogue_group: Some(0),
                dialogue_pick: Some(DialoguePick::OneFull),
                pairs: vec![],
                case: SubtitleCase::OneFull,
                confidence: 0.90,
                notes: vec![],
                reviews: vec![],
            }
        );
    }

    #[test]
    fn knowing_keeps_its_one_english_stream_as_regular_english_without_events() {
        // Knowing's main title (title 0, DVD title 01) has English VobSub 4, Spanish 5, and the director's comments in
        // English (6) and Spanish (7), both flagged 1. 6.4 step 10 does not run for one English stream: no events.
        let title = pinned_title(KNOWING, 0);
        let decision = decide_subtitles(&title, &SubtitleFacts::default(), &Settings::default());
        assert_eq!(
            decision,
            SubtitleDecision {
                kept: vec![track(4, SubtitleRole::Regular, None, "single")],
                dropped: vec![
                    (5, SubtitleDrop::OtherLanguage),
                    (6, SubtitleDrop::DirectorsComments),
                    (7, SubtitleDrop::OtherLanguage),
                ],
                set_aside: vec![(
                    6,
                    "DVD stream flagged director's comments (flags 1 or 2; 8.7 \"Set aside first\")".to_string()
                )],
                full: vec![],
                groups: vec![],
                dialogue_group: None,
                dialogue_pick: None,
                pairs: vec![],
                case: SubtitleCase::OneFull,
                confidence: 0.90,
                notes: vec![],
                reviews: vec![],
            }
        );
        assert_eq!(decision.kept[0].name, "English");
    }

    #[test]
    fn firefly_keeps_its_one_english_track_and_its_closed_captions_as_sdh() {
        // Firefly's first DVD, title 0 (DVD title "01"): English VobSub 5, Spanish 6, French 7 and Spanish 8 (both flagged
        // 4096) and the English closed captions 9 (S_CC608/DVD). One English stream: 6.4 step 10 does not run, no events.
        let title = pinned_title(FIREFLY_1, 0);
        let decision = decide_subtitles(&title, &SubtitleFacts::default(), &Settings::default());
        assert_eq!(
            decision,
            SubtitleDecision {
                kept: vec![
                    track(5, SubtitleRole::Regular, None, "single"),
                    SubtitleTrack { closed_captions: true, ..track(9, SubtitleRole::Sdh, None, "closed_captions") },
                ],
                dropped: vec![
                    (6, SubtitleDrop::OtherLanguage),
                    (7, SubtitleDrop::OtherLanguage),
                    (8, SubtitleDrop::OtherLanguage),
                ],
                set_aside: vec![(9, "DVD closed-caption stream (codec CC; 8.7 \"Set aside first\")".to_string())],
                full: vec![],
                groups: vec![],
                dialogue_group: None,
                dialogue_pick: None,
                pairs: vec![],
                case: SubtitleCase::OneFull,
                confidence: 0.90,
                notes: vec![],
                reviews: vec![],
            }
        );
    }

    #[test]
    fn a_single_english_stream_that_makemkv_removed_as_empty_is_not_kept() {
        let title = bluray(&[VIDEO, ENG_PGS]);
        let evidence = SubtitleFacts { removed: BTreeSet::from([1]), ..SubtitleFacts::default() };
        let decision = decide_subtitles(&title, &evidence, &Settings::default());
        assert_eq!(decision.kept, vec![]);
        assert_eq!(decision.dropped, vec![(1, SubtitleDrop::NoEvents)]);
        assert_eq!(decision.reviews, vec![]);
        assert_eq!((decision.case, decision.confidence), (SubtitleCase::FlagsOrDescriptions, 0.95));
    }

    #[test]
    fn a_lone_stream_flagged_forced_without_events_is_kept_forced_by_its_flag() {
        // A DVD title whose one English subtitle stream carries 4096 without 2048. 6.4 step 10 does not run for one
        // English stream, so it has no events; its flag makes it a forced source (8.7 "Forced"), not the regular track
        // of row 14 (b)'s fallback (row 14 (a) counts a forced track labelled regular without review as costly).
        let title = dvd(&[VIDEO, ENG_VOBSUB_FORCED]);
        let decision = decide_subtitles(&title, &SubtitleFacts::default(), &Settings::default());
        assert_eq!(decision.kept, vec![track(1, SubtitleRole::Forced, None, "flag")]);
        assert_eq!(decision.dropped, vec![]);
        assert_eq!(decision.reviews, vec![]);
        assert_eq!((decision.case, decision.confidence), (SubtitleCase::FlagsOrDescriptions, 0.95));
        let forced = &decision.kept[0];
        assert_eq!((forced.name.as_str(), forced.forced, forced.parent), ("English (Forced)", true, None));
    }

    #[test]
    fn the_closed_caption_stream_is_kept_as_sdh_when_the_dialogue_group_has_none() {
        let title = dvd(&[VIDEO, ENG_VOBSUB, SPA_VOBSUB, ENG_CC]);
        let evidence = facts(vec![(1, regular_events()), (3, times(900, 10.1, 6.0))]);
        let decision = decide_subtitles(&title, &evidence, &Settings::default());
        assert_eq!(
            decision,
            SubtitleDecision {
                kept: vec![
                    track(1, SubtitleRole::Regular, Some(1000), "count"),
                    SubtitleTrack { closed_captions: true, ..track(3, SubtitleRole::Sdh, Some(900), "closed_captions") },
                ],
                dropped: vec![(2, SubtitleDrop::OtherLanguage)],
                set_aside: vec![(3, "DVD closed-caption stream (codec CC; 8.7 \"Set aside first\")".to_string())],
                full: vec![1],
                groups: vec![vec![1]],
                dialogue_group: Some(0),
                dialogue_pick: Some(DialoguePick::OneFull),
                pairs: vec![],
                case: SubtitleCase::OneFull,
                confidence: 0.90,
                notes: vec![],
                reviews: vec![],
            }
        );
        let captions = &decision.kept[1];
        assert_eq!((captions.name.as_str(), captions.hearing_impaired), ("English (SDH)", true));
    }

    #[test]
    fn the_closed_caption_stream_is_dropped_when_the_dialogue_group_has_an_sdh_track() {
        let title = dvd(&[VIDEO, ENG_VOBSUB, ENG_VOBSUB, ENG_CC]);
        let decision =
            decide_subtitles(&title, &facts(vec![(1, regular_events()), (2, sdh_events())]), &Settings::default());
        assert_eq!(
            kept_roles(&decision),
            vec![(1, SubtitleRole::Regular, "alignment".to_string()), (2, SubtitleRole::Sdh, "alignment".to_string())]
        );
        assert_eq!(decision.dropped, vec![(3, SubtitleDrop::ClosedCaptionsUnused)]);
        assert_eq!((decision.case, decision.confidence), (SubtitleCase::Alignment, 0.85));
    }

    #[test]
    fn a_closed_caption_stream_without_english_dialogue_tracks_is_kept_by_its_type_at_095() {
        let title = dvd(&[VIDEO, SPA_VOBSUB, ENG_CC]);
        let decision = decide_subtitles(&title, &SubtitleFacts::default(), &Settings::default());
        assert_eq!(
            decision.kept,
            vec![SubtitleTrack { closed_captions: true, ..track(2, SubtitleRole::Sdh, None, "closed_captions") }]
        );
        assert_eq!(decision.dropped, vec![(1, SubtitleDrop::OtherLanguage)]);
        assert_eq!((decision.case, decision.confidence), (SubtitleCase::FlagsOrDescriptions, 0.95));
        assert_eq!(decision.reviews, vec![]);
    }

    #[test]
    fn a_full_track_outside_the_dialogue_group_is_dropped_as_commentary() {
        let title = bluray(&[VIDEO, ENG_PGS, ENG_PGS, ENG_PGS]);
        let evidence = facts(vec![(1, regular_events()), (2, sdh_events()), (3, commentary_events())]);
        let decision = decide_subtitles(&title, &evidence, &Settings::default());
        assert_eq!(
            kept_roles(&decision),
            vec![(1, SubtitleRole::Regular, "alignment".to_string()), (2, SubtitleRole::Sdh, "alignment".to_string())]
        );
        assert_eq!(decision.dropped, vec![(3, SubtitleDrop::OutsideDialogueGroup)]);
        assert_eq!(decision.groups, vec![vec![1, 2], vec![3]]);
        assert_eq!(decision.dialogue_pick, Some(DialoguePick::Largest));
        assert_eq!(
            decision.pairs,
            vec![(1, 2, 1.0, Band::Same), (1, 3, 0.0, Band::Different), (2, 3, 0.0, Band::Different)]
        );
        assert_eq!((decision.case, decision.confidence, decision.reviews.clone()), (SubtitleCase::Alignment, 0.85, vec![]));
    }

    #[test]
    fn a_title_without_english_subtitles_is_the_no_english_case_at_095() {
        let spanish = decide_subtitles(&bluray(&[VIDEO, SPA_PGS]), &SubtitleFacts::default(), &Settings::default());
        assert_eq!(spanish.kept, vec![]);
        assert_eq!(spanish.dropped, vec![(1, SubtitleDrop::OtherLanguage)]);
        assert_eq!((spanish.case, spanish.confidence), (SubtitleCase::NoEnglish, 0.95));
        let none = decide_subtitles(&bluray(&[VIDEO]), &SubtitleFacts::default(), &Settings::default());
        assert_eq!((none.kept.len(), none.dropped.len()), (0, 0));
        assert_eq!((none.case, none.confidence), (SubtitleCase::NoEnglish, 0.95));
    }

    #[test]
    fn the_forced_only_stream_of_the_regular_track_comes_before_that_of_the_sdh_track() {
        let title = bluray(&[VIDEO, ENG_PGS, ENG_PGS_FORCED_ONLY, ENG_PGS, ENG_PGS_FORCED_ONLY]);
        let evidence = facts(vec![
            (1, regular_events()),
            (2, forced_events()),
            (3, sdh_events()),
            (4, times(15, 130.0, 60.0)),
        ]);
        let decision = decide_subtitles(&title, &evidence, &Settings::default());
        assert_eq!(
            decision.kept,
            vec![
                SubtitleTrack { parent: Some(1), ..track(2, SubtitleRole::Forced, Some(20), "flag") },
                track(1, SubtitleRole::Regular, Some(1000), "alignment"),
                track(3, SubtitleRole::Sdh, Some(1200), "alignment"),
            ]
        );
        assert_eq!(decision.dropped, vec![(4, SubtitleDrop::DerivedForcedOnlyUnused)]);
        assert_eq!(decision.set_aside, vec![(2, forced_only_of(1)), (4, forced_only_of(3))]);
        assert_eq!(decision.dialogue_pick, Some(DialoguePick::ForcedParent));
        let forced = &decision.kept[0];
        assert_eq!((forced.name.as_str(), forced.forced, forced.hearing_impaired), ("English (Forced)", true, false));
        assert_eq!((decision.case, decision.confidence), (SubtitleCase::Alignment, 0.85));
    }

    #[test]
    fn the_forced_only_stream_of_the_sdh_track_is_taken_when_the_regular_tracks_one_is_empty() {
        let title = bluray(&[VIDEO, ENG_PGS, ENG_PGS_FORCED_ONLY, ENG_PGS, ENG_PGS_FORCED_ONLY]);
        let evidence = SubtitleFacts {
            removed: BTreeSet::from([2]),
            ..facts(vec![(1, regular_events()), (3, sdh_events()), (4, times(15, 130.0, 60.0))])
        };
        let decision = decide_subtitles(&title, &evidence, &Settings::default());
        assert_eq!(
            decision.kept,
            vec![
                SubtitleTrack { parent: Some(3), ..track(4, SubtitleRole::Forced, Some(15), "flag") },
                track(1, SubtitleRole::Regular, Some(1000), "alignment"),
                track(3, SubtitleRole::Sdh, Some(1200), "alignment"),
            ]
        );
        assert_eq!(decision.dropped, vec![(2, SubtitleDrop::DerivedForcedOnlyEmpty)]);
        assert_eq!(decision.dialogue_pick, Some(DialoguePick::ForcedParent));
    }

    #[test]
    fn a_stream_flagged_forced_comes_after_the_forced_only_streams() {
        let title = dvd(&[VIDEO, ENG_VOBSUB, ENG_VOBSUB_FORCED_ONLY, ENG_VOBSUB_FORCED]);
        let forty = times(40, 500.0, 30.0);
        let with_forced_only = facts(vec![(1, regular_events()), (2, forced_events()), (3, forty.clone())]);
        let decision = decide_subtitles(&title, &with_forced_only, &Settings::default());
        assert_eq!(
            decision.kept,
            vec![
                SubtitleTrack { parent: Some(1), ..track(2, SubtitleRole::Forced, Some(20), "flag") },
                track(1, SubtitleRole::Regular, Some(1000), "count"),
            ]
        );
        assert_eq!(decision.dropped, vec![(3, SubtitleDrop::ExtraForced)]);
        assert_eq!((decision.case, decision.confidence), (SubtitleCase::OneFull, 0.90));

        let flagged_only =
            SubtitleFacts { removed: BTreeSet::from([2]), ..facts(vec![(1, regular_events()), (3, forty)]) };
        let decision = decide_subtitles(&title, &flagged_only, &Settings::default());
        assert_eq!(
            decision.kept,
            vec![track(3, SubtitleRole::Forced, Some(40), "flag"), track(1, SubtitleRole::Regular, Some(1000), "count")]
        );
        assert_eq!(decision.dropped, vec![(2, SubtitleDrop::DerivedForcedOnlyEmpty)]);
    }

    #[test]
    fn a_separately_authored_short_track_is_the_last_forced_source() {
        let title = dvd(&[VIDEO, ENG_VOBSUB, ENG_VOBSUB_FORCED, ENG_VOBSUB]);
        let thirty = times(30, 600.0, 30.0);
        let flagged = facts(vec![(1, regular_events()), (2, times(40, 500.0, 30.0)), (3, thirty.clone())]);
        let decision = decide_subtitles(&title, &flagged, &Settings::default());
        assert_eq!(
            decision.kept,
            vec![track(2, SubtitleRole::Forced, Some(40), "flag"), track(1, SubtitleRole::Regular, Some(1000), "count")]
        );
        assert_eq!(decision.dropped, vec![(3, SubtitleDrop::ExtraForced)]);

        // The flagged stream has no events: the separately authored one is forced, by its count.
        let empty_flag = facts(vec![(1, regular_events()), (2, vec![]), (3, thirty)]);
        let decision = decide_subtitles(&title, &empty_flag, &Settings::default());
        assert_eq!(
            decision.kept,
            vec![track(3, SubtitleRole::Forced, Some(30), "count"), track(1, SubtitleRole::Regular, Some(1000), "count")]
        );
        assert_eq!(decision.dropped, vec![(2, SubtitleDrop::NoEvents)]);
        assert_eq!((decision.case, decision.confidence), (SubtitleCase::OneFull, 0.90));
    }

    #[test]
    fn a_stream_flagged_forced_is_never_a_dialogue_track_even_when_full() {
        // A DVD episode: 300 dialogue events, and a stream flagged forced (4096 without 2048) whose 40 events start with
        // the dialogue's first 40. 40 is at least 10% of 300, but the flag makes it a forced source (8.7 "Forced"), never
        // a full track: it neither joins the dialogue group nor turns the dialogue track into SDH.
        let title = dvd(&[VIDEO, ENG_VOBSUB, ENG_VOBSUB_FORCED]);
        let evidence = facts(vec![(1, times(300, 10.0, 6.0)), (2, times(40, 10.0, 6.0))]);
        let decision = decide_subtitles(&title, &evidence, &Settings::default());
        assert_eq!(
            decision.kept,
            vec![track(2, SubtitleRole::Forced, Some(40), "flag"), track(1, SubtitleRole::Regular, Some(300), "count")]
        );
        assert_eq!(decision.full, vec![1]);
        assert_eq!(decision.pairs, vec![]);
        assert_eq!(decision.dropped, vec![]);
        assert_eq!(decision.dialogue_pick, Some(DialoguePick::OneFull));
        assert_eq!((decision.case, decision.confidence), (SubtitleCase::OneFull, 0.90));
    }

    #[test]
    fn a_track_that_is_not_full_with_150_events_or_more_is_dropped_with_a_note() {
        // 2,000 dialogue events make a track full from 200 events: 149 and 150 are both not full.
        let title = bluray(&[VIDEO, ENG_PGS, ENG_PGS, ENG_PGS]);
        let evidence = facts(vec![
            (1, times(2000, 10.0, 3.0)),
            (2, times(149, 20.0, 30.0)),
            (3, times(150, 21.0, 30.0)),
        ]);
        let decision = decide_subtitles(&title, &evidence, &Settings::default());
        assert_eq!(
            decision.kept,
            vec![track(2, SubtitleRole::Forced, Some(149), "count"), track(1, SubtitleRole::Regular, Some(2000), "count")]
        );
        assert_eq!(decision.dropped, vec![(3, SubtitleDrop::Partial)]);
        assert_eq!(
            decision.notes,
            vec![Noted {
                note: Note::PartialSubtitleDropped,
                stream: Some(3),
                detail: "stream 3: 150 events, not full (the largest English track has 2000) and 150 or more (8.7 \
                         \"Other tracks\")"
                    .to_string(),
            }]
        );
        assert_eq!(decision.reviews, vec![]);
        assert_eq!((decision.case, decision.confidence), (SubtitleCase::OneFull, 0.90));
    }

    #[test]
    fn a_forced_only_stream_without_a_parent_never_becomes_the_forced_track() {
        let title = bluray(&[VIDEO, ENG_PGS_FORCED_ONLY, ENG_PGS]);
        let decision =
            decide_subtitles(&title, &facts(vec![(1, forced_events()), (2, regular_events())]), &Settings::default());
        assert_eq!(decision.kept, vec![track(2, SubtitleRole::Regular, Some(1000), "count")]);
        assert_eq!(decision.dropped, vec![(1, SubtitleDrop::DerivedForcedOnlyUnused)]);
        assert_eq!(
            decision.set_aside,
            vec![(
                1,
                "derived forced-only stream with no preceding stream of its language (flags 4096 and 2048; 8.7 \
                 \"Set aside first\")"
                    .to_string()
            )]
        );
        assert_eq!((decision.case, decision.confidence), (SubtitleCase::OneFull, 0.90));
    }

    #[test]
    fn a_pair_in_the_between_band_raises_subtitle_ambiguous() {
        let title = bluray(&[VIDEO, ENG_PGS, ENG_PGS_FORCED_ONLY, ENG_PGS]);
        let (a, b) = between_events();
        let decision = decide_subtitles(&title, &facts(vec![(1, a), (2, forced_events()), (3, b)]), &Settings::default());
        assert_eq!(decision.pairs, vec![(1, 3, 0.5, Band::Between)]);
        assert_eq!(decision.groups, vec![vec![1], vec![3]]);
        assert_eq!(decision.dialogue_pick, Some(DialoguePick::ForcedParent));
        // Stream 3 is outside the picked group, but its pair with the group's track 1 is undecided (8.7 "In between"):
        // nothing confirms it as commentary, so it is kept as regular under the review, not dropped (D20).
        assert_eq!(
            decision.kept,
            vec![
                SubtitleTrack { parent: Some(1), ..track(2, SubtitleRole::Forced, Some(20), "flag") },
                track(1, SubtitleRole::Regular, Some(1000), "alignment"),
                track(3, SubtitleRole::Regular, Some(1100), "alignment"),
            ]
        );
        assert_eq!(decision.dropped, vec![]);
        assert_eq!(decision.reviews, vec![ReviewReason::SubtitleAmbiguous]);
        assert_eq!((decision.case, decision.confidence), (SubtitleCase::Ambiguous, 0.60));
    }

    #[test]
    fn two_full_tracks_in_different_groups_with_nothing_to_pick_raise_subtitle_ambiguous() {
        let title = bluray(&[VIDEO, ENG_PGS, ENG_PGS]);
        let decision =
            decide_subtitles(&title, &facts(vec![(1, regular_events()), (2, commentary_events())]), &Settings::default());
        assert_eq!(decision.pairs, vec![(1, 2, 0.0, Band::Different)]);
        assert_eq!(decision.groups, vec![vec![1], vec![2]]);
        // No rule picks a group: the first stands in for the regular and SDH roles, under review. Nothing confirms the
        // other group's track as commentary, so it is kept as regular too, not dropped by its position (D15, D20).
        assert_eq!((decision.dialogue_group, decision.dialogue_pick), (Some(0), None));
        assert_eq!(
            decision.kept,
            vec![
                track(1, SubtitleRole::Regular, Some(1000), "alignment"),
                track(2, SubtitleRole::Regular, Some(800), "alignment"),
            ]
        );
        assert_eq!(decision.dropped, vec![]);
        assert_eq!(decision.reviews, vec![ReviewReason::SubtitleAmbiguous]);
        assert_eq!((decision.case, decision.confidence), (SubtitleCase::Ambiguous, 0.60));
    }

    #[test]
    fn two_largest_groups_of_equal_size_are_no_pick() {
        // Streams 1 and 2 are the dialogue and its SDH copy; 3 is a commentary (2.5 s off the dialogue) and 4 the same
        // commentary with 100 more events 2 s after its first 100 (900 events). Two groups of two: neither is the
        // largest, Task 9's `dialogue_group` picks nothing, and position never decides (D15): subtitle_ambiguous, and
        // no full track is dropped (D20). The first group stands in for the regular and SDH roles.
        let title = bluray(&[VIDEO, ENG_PGS, ENG_PGS, ENG_PGS, ENG_PGS]);
        let evidence = facts(vec![
            (1, regular_events()),
            (2, sdh_events()),
            (3, commentary_events()),
            (4, merged(commentary_events(), times(100, 14.5, 6.0))),
        ]);
        let decision = decide_subtitles(&title, &evidence, &Settings::default());
        assert_eq!(decision.groups, vec![vec![1, 2], vec![3, 4]]);
        assert_eq!(
            decision.pairs,
            vec![
                (1, 2, 1.0, Band::Same),
                (1, 3, 0.0, Band::Different),
                (1, 4, 0.0, Band::Different),
                (2, 3, 0.0, Band::Different),
                (2, 4, 0.0, Band::Different),
                (3, 4, 1.0, Band::Same),
            ]
        );
        assert_eq!((decision.dialogue_group, decision.dialogue_pick), (Some(0), None));
        assert_eq!(
            decision.kept,
            vec![
                track(1, SubtitleRole::Regular, Some(1000), "alignment"),
                track(3, SubtitleRole::Regular, Some(800), "alignment"),
                track(4, SubtitleRole::Regular, Some(900), "alignment"),
                track(2, SubtitleRole::Sdh, Some(1200), "alignment"),
            ]
        );
        assert_eq!(decision.dropped, vec![]);
        assert_eq!(decision.reviews, vec![ReviewReason::SubtitleAmbiguous]);
        assert_eq!((decision.case, decision.confidence), (SubtitleCase::Ambiguous, 0.60));
    }

    #[test]
    fn more_than_two_tracks_in_the_dialogue_group_raise_subtitle_ambiguous() {
        // Stream 3 is the dialogue plus 100 cues 4 s after the first 100 dialogue events: 1,100 events, the same group.
        let title = bluray(&[VIDEO, ENG_PGS, ENG_PGS, ENG_PGS]);
        let evidence = facts(vec![
            (1, regular_events()),
            (2, sdh_events()),
            (3, merged(regular_events(), times(100, 14.0, 6.0))),
        ]);
        let decision = decide_subtitles(&title, &evidence, &Settings::default());
        assert_eq!(decision.groups, vec![vec![1, 2, 3]]);
        assert_eq!(
            decision.pairs,
            vec![(1, 2, 1.0, Band::Same), (1, 3, 1.0, Band::Same), (2, 3, 1000.0 / 1100.0, Band::Same)]
        );
        // The two with the most events are kept: 2 (1,200) is SDH, 3 (1,100) regular; 1 (1,000) is dropped.
        assert_eq!(
            decision.kept,
            vec![
                track(3, SubtitleRole::Regular, Some(1100), "alignment"),
                track(2, SubtitleRole::Sdh, Some(1200), "alignment"),
            ]
        );
        assert_eq!(decision.dropped, vec![(1, SubtitleDrop::ExtraDialogueTrack)]);
        assert_eq!(decision.reviews, vec![ReviewReason::SubtitleAmbiguous]);
        assert_eq!((decision.case, decision.confidence), (SubtitleCase::Ambiguous, 0.60));
    }

    #[test]
    fn a_track_described_sdh_is_one_of_the_two_kept_from_a_group_of_three() {
        // The three aligned tracks of the test above (1,000, 1,200 and 1,100 events). TheDiscDB describes stream 1, the
        // one with the fewest events, as "English SDH" and the others as "English": a description saying SDH overrides
        // the count (8.7 "Regular and SDH"), so stream 1 is kept as SDH and the extra track is stream 3.
        let title = bluray(&[VIDEO, ENG_PGS, ENG_PGS, ENG_PGS]);
        let record: DiscDbTitle = serde_json::from_value(json!({
            "index": 0,
            "source_file": "00800.mpls",
            "segment_map": "1",
            "duration": "1:00:00",
            "tracks": [
                {"index": 0, "type": "Video", "name": "Mpeg4 AVC High@L4.1"},
                {"index": 1, "type": "Subtitles", "name": "HDMV PGS Subtitles", "language_code": "eng",
                 "language": "English", "description": "English SDH"},
                {"index": 2, "type": "Subtitles", "name": "HDMV PGS Subtitles", "language_code": "eng",
                 "language": "English", "description": "English"},
                {"index": 3, "type": "Subtitles", "name": "HDMV PGS Subtitles", "language_code": "eng",
                 "language": "English", "description": "English"}
            ]
        }))
        .unwrap();
        let evidence = SubtitleFacts {
            discdb: Some(&record),
            ..facts(vec![
                (1, regular_events()),
                (2, sdh_events()),
                (3, merged(regular_events(), times(100, 14.0, 6.0))),
            ])
        };
        let decision = decide_subtitles(&title, &evidence, &Settings::default());
        assert_eq!(decision.groups, vec![vec![1, 2, 3]]);
        assert_eq!(decision.dialogue_pick, Some(DialoguePick::Described));
        assert_eq!(
            decision.kept,
            vec![
                track(2, SubtitleRole::Regular, Some(1200), "description"),
                track(1, SubtitleRole::Sdh, Some(1000), "description"),
            ]
        );
        assert_eq!(decision.dropped, vec![(3, SubtitleDrop::ExtraDialogueTrack)]);
        assert_eq!(decision.reviews, vec![ReviewReason::SubtitleAmbiguous]);
        assert_eq!((decision.case, decision.confidence), (SubtitleCase::Ambiguous, 0.60));
    }

    #[test]
    fn a_track_thediscdb_describes_as_commentary_is_dropped_and_never_grouped() {
        // Stream 3 is timed on the dialogue (the 1,100 events of the tests above), so by timing alone it would be the
        // third track of the dialogue group and kept as regular. TheDiscDB describes it as "Commentary" (and streams 1
        // and 2 not at all): a database entry confirms commentary (R6, D15), so it is set aside and dropped, and the
        // dialogue group is the largest group of the other two.
        let title = bluray(&[VIDEO, ENG_PGS, ENG_PGS, ENG_PGS]);
        let record: DiscDbTitle = serde_json::from_value(json!({
            "index": 0,
            "source_file": "00800.mpls",
            "segment_map": "1",
            "duration": "1:00:00",
            "tracks": [
                {"index": 0, "type": "Video", "name": "Mpeg4 AVC High@L4.1"},
                {"index": 1, "type": "Subtitles", "name": "HDMV PGS Subtitles", "language_code": "eng",
                 "language": "English"},
                {"index": 2, "type": "Subtitles", "name": "HDMV PGS Subtitles", "language_code": "eng",
                 "language": "English"},
                {"index": 3, "type": "Subtitles", "name": "HDMV PGS Subtitles", "language_code": "eng",
                 "language": "English", "description": "Commentary"}
            ]
        }))
        .unwrap();
        let evidence = SubtitleFacts {
            discdb: Some(&record),
            ..facts(vec![
                (1, regular_events()),
                (2, sdh_events()),
                (3, merged(regular_events(), times(100, 14.0, 6.0))),
            ])
        };
        let decision = decide_subtitles(&title, &evidence, &Settings::default());
        assert_eq!(
            decision,
            SubtitleDecision {
                kept: vec![
                    track(1, SubtitleRole::Regular, Some(1000), "alignment"),
                    track(2, SubtitleRole::Sdh, Some(1200), "alignment"),
                ],
                dropped: vec![(3, SubtitleDrop::OutsideDialogueGroup)],
                set_aside: vec![(3, "described as commentary by TheDiscDB (R6)".to_string())],
                full: vec![1, 2],
                groups: vec![vec![1, 2]],
                dialogue_group: Some(0),
                dialogue_pick: Some(DialoguePick::Largest),
                pairs: vec![(1, 2, 1.0, Band::Same)],
                case: SubtitleCase::Alignment,
                confidence: 0.85,
                notes: vec![],
                reviews: vec![],
            }
        );
    }

    #[test]
    fn two_dialogue_tracks_with_equal_event_counts_raise_subtitle_ambiguous() {
        let title = bluray(&[VIDEO, ENG_PGS, ENG_PGS]);
        let decision =
            decide_subtitles(&title, &facts(vec![(1, regular_events()), (2, regular_events())]), &Settings::default());
        assert_eq!(
            decision.kept,
            vec![
                track(1, SubtitleRole::Regular, Some(1000), "alignment"),
                track(2, SubtitleRole::Sdh, Some(1000), "alignment"),
            ]
        );
        assert_eq!(decision.reviews, vec![ReviewReason::SubtitleAmbiguous]);
        assert_eq!((decision.case, decision.confidence), (SubtitleCase::Ambiguous, 0.60));
    }

    #[test]
    fn two_english_streams_without_events_raise_subtitle_ambiguous() {
        let title = bluray(&[VIDEO, ENG_PGS, ENG_PGS]);
        let decision = decide_subtitles(&title, &SubtitleFacts::default(), &Settings::default());
        assert_eq!(decision.kept, vec![]);
        assert_eq!(decision.dropped, vec![(1, SubtitleDrop::NoEvents), (2, SubtitleDrop::NoEvents)]);
        assert_eq!((decision.full.len(), decision.groups.len(), decision.dialogue_group), (0, 0, None));
        assert_eq!(decision.reviews, vec![ReviewReason::SubtitleAmbiguous]);
        assert_eq!((decision.case, decision.confidence), (SubtitleCase::Ambiguous, 0.60));
    }

    #[test]
    fn subs_tolerance_ms_decides_whether_two_tracks_align() {
        let title = bluray(&[VIDEO, ENG_PGS, ENG_PGS]);
        let evidence = facts(vec![(1, regular_events()), (2, commentary_events())]);
        // 300 ms (the default): 2.5 s apart, different groups, nothing picks one.
        let strict = decide_subtitles(&title, &evidence, &Settings::default());
        assert_eq!(strict.pairs, vec![(1, 2, 0.0, Band::Different)]);
        assert_eq!(strict.reviews, vec![ReviewReason::SubtitleAmbiguous]);
        // 3,000 ms: every commentary event starts within 3 s of a dialogue event, one group, the larger track SDH.
        let loose = decide_subtitles(&title, &evidence, &Settings { subs_tolerance_ms: 3000, ..Settings::default() });
        assert_eq!(loose.pairs, vec![(1, 2, 1.0, Band::Same)]);
        assert_eq!(loose.groups, vec![vec![1, 2]]);
        assert_eq!(
            kept_roles(&loose),
            vec![(2, SubtitleRole::Regular, "alignment".to_string()), (1, SubtitleRole::Sdh, "alignment".to_string())]
        );
        assert_eq!(loose.reviews, vec![]);
        assert_eq!((loose.case, loose.confidence), (SubtitleCase::Alignment, 0.85));
    }

    #[test]
    fn subs_same_min_is_the_lowest_share_of_the_same_band() {
        let title = bluray(&[VIDEO, ENG_PGS, ENG_PGS_FORCED_ONLY, ENG_PGS]);
        let (a, b) = between_events();
        let evidence = facts(vec![(1, a), (2, forced_events()), (3, b)]);
        // 0.70 (the default): 0.5 is between.
        let default = decide_subtitles(&title, &evidence, &Settings::default());
        assert_eq!(default.pairs, vec![(1, 3, 0.5, Band::Between)]);
        assert_eq!(default.reviews, vec![ReviewReason::SubtitleAmbiguous]);
        // 0.50: 0.5 is at least subs_same_min, so the two tracks share a group and the larger is SDH.
        let lowered = decide_subtitles(&title, &evidence, &Settings { subs_same_min: 0.5, ..Settings::default() });
        assert_eq!(lowered.pairs, vec![(1, 3, 0.5, Band::Same)]);
        assert_eq!(lowered.groups, vec![vec![1, 3]]);
        assert_eq!(
            lowered.kept,
            vec![
                SubtitleTrack { parent: Some(1), ..track(2, SubtitleRole::Forced, Some(20), "flag") },
                track(1, SubtitleRole::Regular, Some(1000), "alignment"),
                track(3, SubtitleRole::Sdh, Some(1100), "alignment"),
            ]
        );
        assert_eq!(lowered.reviews, vec![]);
        assert_eq!((lowered.case, lowered.confidence), (SubtitleCase::Alignment, 0.85));
    }

    #[test]
    fn subs_diff_max_is_the_highest_share_of_the_different_band() {
        let title = bluray(&[VIDEO, ENG_PGS, ENG_PGS_FORCED_ONLY, ENG_PGS]);
        let (a, b) = between_events();
        let evidence = facts(vec![(1, a), (2, forced_events()), (3, b)]);
        // 0.30 (the default): 0.5 is between.
        let default = decide_subtitles(&title, &evidence, &Settings::default());
        assert_eq!(default.reviews, vec![ReviewReason::SubtitleAmbiguous]);
        // 0.50: 0.5 is at most subs_diff_max, so the tracks are in different groups and the forced parent picks.
        let raised = decide_subtitles(&title, &evidence, &Settings { subs_diff_max: 0.5, ..Settings::default() });
        assert_eq!(raised.pairs, vec![(1, 3, 0.5, Band::Different)]);
        assert_eq!(raised.groups, vec![vec![1], vec![3]]);
        assert_eq!(raised.dialogue_pick, Some(DialoguePick::ForcedParent));
        assert_eq!(raised.dropped, vec![(3, SubtitleDrop::OutsideDialogueGroup)]);
        assert_eq!(raised.reviews, vec![]);
        assert_eq!((raised.case, raised.confidence), (SubtitleCase::Alignment, 0.85));
    }

    #[test]
    fn the_decision_serialises_with_the_spec_spellings_and_reads_back() {
        let title = bluray(&[VIDEO, ENG_PGS, SPA_PGS]);
        let decision = decide_subtitles(&title, &facts(vec![(1, regular_events())]), &Settings::default());
        let value = serde_json::to_value(&decision).unwrap();
        assert_eq!(
            value,
            json!({
                "kept": [{
                    "stream": 1, "role": "regular", "name": "English", "forced": false, "hearing_impaired": false,
                    "default": false, "events": 1000, "closed_captions": false, "parent": null, "decided_by": "count"
                }],
                "dropped": [[2, {"why": "other_language"}]],
                "set_aside": [],
                "full": [1],
                "groups": [[1]],
                "dialogue_group": 0,
                "dialogue_pick": "one_full",
                "pairs": [],
                "case": "one_full",
                "confidence": 0.9,
                "notes": [],
                "reviews": []
            })
        );
        assert_eq!(serde_json::from_value::<SubtitleDecision>(value).unwrap(), decision);
    }

    #[test]
    fn a_text_verdict_that_differs_raises_subtitle_ambiguous_with_a_note_per_stream() {
        let title = bluray(&[VIDEO, ENG_PGS, ENG_PGS, SPA_PGS]);
        // The text reads stream 2, which the counts make SDH, as a regular track; the Spanish drop is not compared.
        let text = BTreeMap::from([(1, verdict(Role::Regular)), (2, verdict(Role::Regular)), (3, verdict(Role::Drop))]);
        let evidence = SubtitleFacts { text: Some(&text), ..facts(vec![(1, regular_events()), (2, sdh_events())]) };
        let decision = decide_subtitles(&title, &evidence, &Settings::default());
        // The text never changes a role, a flag or a drop.
        assert_eq!(
            decision.kept,
            vec![
                track(1, SubtitleRole::Regular, Some(1000), "alignment"),
                track(2, SubtitleRole::Sdh, Some(1200), "alignment"),
            ]
        );
        assert_eq!(decision.dropped, vec![(3, SubtitleDrop::OtherLanguage)]);
        assert_eq!(
            decision.notes,
            vec![Noted {
                note: Note::SubtitleTextDisagrees,
                stream: Some(2),
                detail: "stream 2: the text reads regular, the counts keep it as sdh (8.7 \"Text\")".to_string(),
            }]
        );
        assert_eq!(decision.reviews, vec![ReviewReason::SubtitleAmbiguous]);
        assert_eq!((decision.case, decision.confidence), (SubtitleCase::Ambiguous, 0.60));
    }

    #[test]
    fn a_text_verdict_that_agrees_adds_nothing() {
        let title = bluray(&[VIDEO, ENG_PGS, ENG_PGS, SPA_PGS]);
        let text = BTreeMap::from([(1, verdict(Role::Regular)), (2, verdict(Role::Sdh)), (3, verdict(Role::Drop))]);
        let evidence = SubtitleFacts { text: Some(&text), ..facts(vec![(1, regular_events()), (2, sdh_events())]) };
        let decision = decide_subtitles(&title, &evidence, &Settings::default());
        assert_eq!((decision.notes.len(), decision.reviews.len()), (0, 0));
        assert_eq!((decision.case, decision.confidence), (SubtitleCase::Alignment, 0.85));
    }

    #[test]
    fn a_text_drop_of_a_kept_track_and_a_text_role_for_a_dropped_one_are_both_reviewed() {
        let title = bluray(&[VIDEO, ENG_PGS, ENG_PGS, ENG_PGS]);
        let text = BTreeMap::from([(1, verdict(Role::Drop)), (2, verdict(Role::Sdh)), (3, verdict(Role::Regular))]);
        let evidence = SubtitleFacts {
            text: Some(&text),
            ..facts(vec![(1, regular_events()), (2, sdh_events()), (3, commentary_events())])
        };
        let decision = decide_subtitles(&title, &evidence, &Settings::default());
        assert_eq!(decision.dropped, vec![(3, SubtitleDrop::OutsideDialogueGroup)]);
        assert_eq!(
            decision.notes,
            vec![
                Noted {
                    note: Note::SubtitleTextDisagrees,
                    stream: Some(1),
                    detail: "stream 1: the text reads drop, the counts keep it as regular (8.7 \"Text\")".to_string(),
                },
                Noted {
                    note: Note::SubtitleTextDisagrees,
                    stream: Some(3),
                    detail: "stream 3: the text reads regular, the counts drop it (8.7 \"Text\")".to_string(),
                },
            ]
        );
        assert_eq!(decision.reviews, vec![ReviewReason::SubtitleAmbiguous]);
        assert_eq!((decision.case, decision.confidence), (SubtitleCase::Ambiguous, 0.60));
    }

    #[test]
    fn a_text_keep_with_its_review_raises_subtitle_ambiguous_for_a_dropped_track() {
        // Stream 3 is timed as a commentary, outside the dialogue group of 1 and 2, and the counts drop it. The text
        // rule keeps it and asks for a review (text::decide writes keep only with subtitle_ambiguous). The text never
        // changes a drop, so stream 3 stays dropped, but the review and the note are raised (8.7 "Text", D20).
        let title = bluray(&[VIDEO, ENG_PGS, ENG_PGS, ENG_PGS]);
        let text = BTreeMap::from([(
            3,
            Decision { role: Role::Keep, review: Some("subtitle_ambiguous".into()), note: None },
        )]);
        let evidence = SubtitleFacts {
            text: Some(&text),
            ..facts(vec![(1, regular_events()), (2, sdh_events()), (3, commentary_events())])
        };
        let decision = decide_subtitles(&title, &evidence, &Settings::default());
        assert_eq!(decision.dropped, vec![(3, SubtitleDrop::OutsideDialogueGroup)]);
        assert_eq!(
            decision.notes,
            vec![Noted {
                note: Note::SubtitleTextDisagrees,
                stream: Some(3),
                detail: "stream 3: the text reads keep, the counts drop it (8.7 \"Text\")".to_string(),
            }]
        );
        assert_eq!(decision.reviews, vec![ReviewReason::SubtitleAmbiguous]);
        assert_eq!((decision.case, decision.confidence), (SubtitleCase::Ambiguous, 0.60));
    }

    #[test]
    fn remembered_overrides_follow_the_stream_signature_and_cover_the_streams_they_name() {
        let title = bluray(&[VIDEO, ENG_PGS, ENG_PGS, SPA_PGS]);
        let events = vec![(1, regular_events()), (2, sdh_events())];
        // One entry: the second English PGS stream (stream 2) is dropped. Stream 1 has no entry.
        let drop_second = vec![SubtitleOverrideEntry { signature: signature(&title.streams, 2), action: SubtitleOverride::Drop }];
        let evidence = SubtitleFacts { remembered: Some(drop_second.as_slice()), ..facts(events.clone()) };
        let decision = decide_subtitles(&title, &evidence, &Settings::default());
        assert_eq!(decision.kept, vec![track(1, SubtitleRole::Regular, Some(1000), "alignment")]);
        assert_eq!(decision.dropped, vec![(2, SubtitleDrop::Remembered), (3, SubtitleDrop::OtherLanguage)]);
        assert_eq!((decision.case, decision.confidence), (SubtitleCase::Alignment, 0.85));

        // Both English streams have an entry: the confidence is 1.00, the case is the rule's.
        let both = vec![
            SubtitleOverrideEntry {
                signature: signature(&title.streams, 1),
                action: SubtitleOverride::Keep { role: SubtitleRole::Sdh },
            },
            SubtitleOverrideEntry { signature: signature(&title.streams, 2), action: SubtitleOverride::Drop },
        ];
        let evidence = SubtitleFacts { remembered: Some(both.as_slice()), ..facts(events) };
        let decision = decide_subtitles(&title, &evidence, &Settings::default());
        assert_eq!(decision.kept, vec![track(1, SubtitleRole::Sdh, Some(1000), "remembered")]);
        assert_eq!(decision.dropped, vec![(2, SubtitleDrop::Remembered), (3, SubtitleDrop::OtherLanguage)]);
        assert_eq!((decision.case, decision.confidence), (SubtitleCase::Alignment, 1.00));
    }

    #[test]
    fn review_overrides_win_over_remembered_ones_and_ignore_other_streams() {
        let title = bluray(&[VIDEO, ENG_PGS, ENG_PGS_FORCED_ONLY, ENG_PGS, SPA_PGS]);
        let remembered =
            vec![SubtitleOverrideEntry { signature: signature(&title.streams, 1), action: SubtitleOverride::Drop }];
        let reviewed = BTreeMap::from([
            // Stream 0 is the video stream: not a subtitle stream, so ignored.
            (0, SubtitleOverride::Drop),
            (1, SubtitleOverride::Keep { role: SubtitleRole::Regular }),
            (2, SubtitleOverride::Keep { role: SubtitleRole::Forced }),
            // Stream 4 is Spanish: 8.7's first sentence drops it and `SubtitleRole` names only English roles: ignored.
            (4, SubtitleOverride::Keep { role: SubtitleRole::Regular }),
        ]);
        let evidence = SubtitleFacts {
            remembered: Some(remembered.as_slice()),
            reviewed: Some(&reviewed),
            ..facts(vec![(1, regular_events()), (3, sdh_events())])
        };
        let decision = decide_subtitles(&title, &evidence, &Settings::default());
        assert_eq!(
            decision.kept,
            vec![
                SubtitleTrack { parent: Some(1), ..track(2, SubtitleRole::Forced, None, "reviewed") },
                track(1, SubtitleRole::Regular, Some(1000), "reviewed"),
                track(3, SubtitleRole::Sdh, Some(1200), "alignment"),
            ]
        );
        assert_eq!(decision.dropped, vec![(4, SubtitleDrop::OtherLanguage)]);
        assert!(decision.kept.iter().all(|kept| kept.stream != 4));
        // Stream 3 has no override: the confidence stays the rule's.
        assert_eq!((decision.case, decision.confidence), (SubtitleCase::Alignment, 0.85));
        assert_eq!(decision.reviews, vec![]);
    }

    #[test]
    fn overrides_on_every_english_stream_set_100_and_settle_the_review() {
        // Equal event counts raise subtitle_ambiguous; the owner then sets both roles. The text disagrees on stream 2,
        // which the owner decided, so it is not compared.
        let title = bluray(&[VIDEO, ENG_PGS, ENG_PGS]);
        let reviewed = BTreeMap::from([
            (1, SubtitleOverride::Keep { role: SubtitleRole::Regular }),
            (2, SubtitleOverride::Keep { role: SubtitleRole::Sdh }),
        ]);
        let text = BTreeMap::from([(2, verdict(Role::Regular))]);
        let evidence = SubtitleFacts {
            reviewed: Some(&reviewed),
            text: Some(&text),
            ..facts(vec![(1, regular_events()), (2, regular_events())])
        };
        let decision = decide_subtitles(&title, &evidence, &Settings::default());
        assert_eq!(
            decision.kept,
            vec![
                track(1, SubtitleRole::Regular, Some(1000), "reviewed"),
                track(2, SubtitleRole::Sdh, Some(1000), "reviewed"),
            ]
        );
        assert_eq!(decision.notes, vec![]);
        assert_eq!(decision.reviews, vec![]);
        assert_eq!((decision.case, decision.confidence), (SubtitleCase::Ambiguous, 1.00));
    }

    #[test]
    fn an_override_replaces_the_drop_and_the_note_of_its_stream() {
        let title = bluray(&[VIDEO, ENG_PGS, ENG_PGS, ENG_PGS]);
        let reviewed = BTreeMap::from([(3, SubtitleOverride::Drop)]);
        let evidence = SubtitleFacts {
            reviewed: Some(&reviewed),
            ..facts(vec![(1, times(2000, 10.0, 3.0)), (2, times(149, 20.0, 30.0)), (3, times(150, 21.0, 30.0))])
        };
        let decision = decide_subtitles(&title, &evidence, &Settings::default());
        assert_eq!(decision.dropped, vec![(3, SubtitleDrop::Reviewed)]);
        assert_eq!(decision.notes, vec![]);
        assert_eq!(
            decision.kept,
            vec![track(2, SubtitleRole::Forced, Some(149), "count"), track(1, SubtitleRole::Regular, Some(2000), "count")]
        );
        assert_eq!((decision.case, decision.confidence), (SubtitleCase::OneFull, 0.90));
    }

    #[test]
    fn overrides_need_not_name_a_stream_makemkv_removed() {
        // A pair in the between band raises subtitle_ambiguous. MakeMKV removed the forced-only stream 2 as empty, so the
        // owner's Override tracks, which choose among the file's tracks, name only streams 1 and 3: they cover every
        // English stream in the file, so the confidence is 1.00 and the review they answer is settled.
        let title = bluray(&[VIDEO, ENG_PGS, ENG_PGS_FORCED_ONLY, ENG_PGS]);
        let (a, b) = between_events();
        let reviewed = BTreeMap::from([
            (1, SubtitleOverride::Keep { role: SubtitleRole::Regular }),
            (3, SubtitleOverride::Keep { role: SubtitleRole::Sdh }),
        ]);
        let evidence = SubtitleFacts {
            removed: BTreeSet::from([2]),
            reviewed: Some(&reviewed),
            ..facts(vec![(1, a), (3, b)])
        };
        let decision = decide_subtitles(&title, &evidence, &Settings::default());
        assert_eq!(
            decision.kept,
            vec![
                track(1, SubtitleRole::Regular, Some(1000), "reviewed"),
                track(3, SubtitleRole::Sdh, Some(1100), "reviewed"),
            ]
        );
        assert_eq!(decision.dropped, vec![(2, SubtitleDrop::DerivedForcedOnlyEmpty)]);
        assert_eq!(decision.reviews, vec![]);
        assert_eq!((decision.case, decision.confidence), (SubtitleCase::Ambiguous, 1.00));
    }

    #[test]
    fn a_keep_override_on_the_closed_captions_keeps_its_type() {
        // A DVD with one English VobSub stream and the closed captions; the owner keeps the captions as SDH. They stay
        // closed captions, decided by the review; stream 1 has no override, so the confidence stays the rule's.
        let title = dvd(&[VIDEO, ENG_VOBSUB, ENG_CC]);
        let reviewed = BTreeMap::from([(2, SubtitleOverride::Keep { role: SubtitleRole::Sdh })]);
        let evidence = SubtitleFacts { reviewed: Some(&reviewed), ..SubtitleFacts::default() };
        let decision = decide_subtitles(&title, &evidence, &Settings::default());
        assert_eq!(
            decision.kept,
            vec![
                track(1, SubtitleRole::Regular, None, "single"),
                SubtitleTrack { closed_captions: true, ..track(2, SubtitleRole::Sdh, None, "reviewed") },
            ]
        );
        assert_eq!(decision.dropped, vec![]);
        assert_eq!((decision.case, decision.confidence), (SubtitleCase::OneFull, 0.90));
    }

    // ------------------------------------------------------------------ the final review's fixes

    #[test]
    fn step10_runs_with_two_or_more_english_or_no_language_subtitle_streams_in_the_file() {
        use crate::tracks::subtitle::step10_runs;
        // One English stream: step 10 is skipped; a forced-only stream and the closed captions count as row 14 counts.
        assert!(!step10_runs(&bluray(&[VIDEO, ENG_PGS, SPA_PGS]), &BTreeSet::new()));
        assert!(step10_runs(&bluray(&[VIDEO, ENG_PGS, ENG_PGS_FORCED_ONLY]), &BTreeSet::new()));
        assert!(step10_runs(&dvd(&[VIDEO, ENG_VOBSUB, ENG_CC]), &BTreeSet::new()));
        assert!(step10_runs(&bluray(&[VIDEO, ENG_PGS, ("pgs", "", 0)]), &BTreeSet::new()));
        // A stream MakeMKV removed is not in the file.
        assert!(!step10_runs(&bluray(&[VIDEO, ENG_PGS, ENG_PGS_FORCED_ONLY]), &BTreeSet::from([2])));
        // Other languages never count.
        assert!(!step10_runs(&dvd(&[VIDEO, ENG_VOBSUB, SPA_VOBSUB, SPA_PGS]), &BTreeSet::new()));
    }

    #[test]
    fn one_english_stream_with_ocr_text_raises_no_text_review() {
        // The Office's layout: one English PGS stream (1) with events and its derived forced-only stream (2), which
        // MakeMKV removed as empty. The file holds one English stream, so 6.4 step 10 does not run and the text is not
        // read (8.7 "Text"): a disagreeing verdict adds no review and no note.
        let title = bluray(&[VIDEO, ENG_PGS, ENG_PGS_FORCED_ONLY]);
        let text = BTreeMap::from([(1, verdict(Role::Sdh))]);
        let evidence = SubtitleFacts {
            removed: BTreeSet::from([2]),
            text: Some(&text),
            ..facts(vec![(1, regular_events())])
        };
        let decision = decide_subtitles(&title, &evidence, &Settings::default());
        assert_eq!(decision.kept, vec![track(1, SubtitleRole::Regular, Some(1000), "count")]);
        assert_eq!(decision.dropped, vec![(2, SubtitleDrop::DerivedForcedOnlyEmpty)]);
        assert_eq!(decision.notes, vec![]);
        assert_eq!(decision.reviews, vec![]);
        assert_eq!((decision.case, decision.confidence), (SubtitleCase::OneFull, 0.90));
        // Two plain English streams, one of them removed: still one in the file, still no text review.
        let two = bluray(&[VIDEO, ENG_PGS, ENG_PGS]);
        let evidence = SubtitleFacts {
            removed: BTreeSet::from([2]),
            text: Some(&text),
            ..facts(vec![(1, regular_events())])
        };
        let decision = decide_subtitles(&two, &evidence, &Settings::default());
        assert_eq!(decision.kept, vec![track(1, SubtitleRole::Regular, Some(1000), "count")]);
        assert_eq!((decision.notes.len(), decision.reviews.len()), (0, 0));
        assert_eq!((decision.case, decision.confidence), (SubtitleCase::OneFull, 0.90));
    }

    #[test]
    fn two_english_streams_in_the_file_still_read_the_text_and_raise_its_review() {
        // The mirror case: the forced-only stream 2 is in the file with its events, so the file holds two English
        // streams, 6.4 step 10 runs and the disagreeing verdict on stream 1 is reviewed.
        let title = bluray(&[VIDEO, ENG_PGS, ENG_PGS_FORCED_ONLY]);
        let text = BTreeMap::from([(1, verdict(Role::Sdh))]);
        let evidence = SubtitleFacts { text: Some(&text), ..facts(vec![(1, regular_events()), (2, forced_events())]) };
        let decision = decide_subtitles(&title, &evidence, &Settings::default());
        assert_eq!(
            decision.kept,
            vec![
                SubtitleTrack { parent: Some(1), ..track(2, SubtitleRole::Forced, Some(20), "flag") },
                track(1, SubtitleRole::Regular, Some(1000), "count"),
            ]
        );
        assert_eq!(
            decision.notes,
            vec![Noted {
                note: Note::SubtitleTextDisagrees,
                stream: Some(1),
                detail: "stream 1: the text reads sdh, the counts keep it as regular (8.7 \"Text\")".to_string(),
            }]
        );
        assert_eq!(decision.reviews, vec![ReviewReason::SubtitleAmbiguous]);
        assert_eq!((decision.case, decision.confidence), (SubtitleCase::Ambiguous, 0.60));
    }

    /// TheDiscDB's title 0 for a made-up Blu-ray of a video stream and two English PGS streams, stream 2 described as
    /// `description`.
    fn described_second(description: &str) -> DiscDbTitle {
        serde_json::from_value(json!({
            "index": 0,
            "source_file": "00800.mpls",
            "segment_map": "1",
            "duration": "1:00:00",
            "tracks": [
                {"index": 0, "type": "Video", "name": "Mpeg4 AVC High@L4.1"},
                {"index": 1, "type": "Subtitles", "name": "HDMV PGS Subtitles", "language_code": "eng",
                 "language": "English"},
                {"index": 2, "type": "Subtitles", "name": "HDMV PGS Subtitles", "language_code": "eng",
                 "language": "English", "description": description}
            ]
        }))
        .unwrap()
    }

    #[test]
    fn a_stream_described_as_commentary_subtitles_is_dropped_and_never_the_dialogue_group() {
        // Stream 2 carries the commentary text and TheDiscDB describes it "Commentary Subtitles"; stream 1, the dialogue,
        // has no description, and the two fall in different alignment groups. A commentary description is not a plain
        // subtitle description (Open point 29), so stream 2 is set aside as described commentary and dropped (R6, D15),
        // and the dialogue is the Regular English track.
        let title = bluray(&[VIDEO, ENG_PGS, ENG_PGS]);
        let record = described_second("Commentary Subtitles");
        let evidence =
            SubtitleFacts { discdb: Some(&record), ..facts(vec![(1, regular_events()), (2, commentary_events())]) };
        let decision = decide_subtitles(&title, &evidence, &Settings::default());
        assert_eq!(decision.dropped, vec![(2, SubtitleDrop::OutsideDialogueGroup)]);
        assert_eq!(decision.set_aside, vec![(2, "described as commentary by TheDiscDB (R6)".to_string())]);
        assert_eq!(decision.kept, vec![track(1, SubtitleRole::Regular, Some(1000), "count")]);
        assert!(decision.kept.iter().all(|kept| kept.stream != 2));
        assert_eq!(decision.reviews, vec![]);
    }

    #[test]
    fn a_described_commentary_stream_never_sets_the_largest_count() {
        // Dialogue stream 1 has 1,400 events (a 10% bar of 140); the commentary stream 2, described "Commentary", has
        // 2,000 (a bar of 200); stream 3 has 160 events, each at a dialogue event. Set-aside streams take no part in
        // counting (8.7 "Set aside first"), so the largest is the dialogue's and stream 3 is full: it joins the dialogue
        // group and is never dropped as a partial track.
        let title = bluray(&[VIDEO, ENG_PGS, ENG_PGS, ENG_PGS]);
        let record: DiscDbTitle = serde_json::from_value(json!({
            "index": 0,
            "source_file": "00800.mpls",
            "segment_map": "1",
            "duration": "1:00:00",
            "tracks": [
                {"index": 0, "type": "Video", "name": "Mpeg4 AVC High@L4.1"},
                {"index": 1, "type": "Subtitles", "name": "HDMV PGS Subtitles", "language_code": "eng",
                 "language": "English"},
                {"index": 2, "type": "Subtitles", "name": "HDMV PGS Subtitles", "language_code": "eng",
                 "language": "English", "description": "Commentary"},
                {"index": 3, "type": "Subtitles", "name": "HDMV PGS Subtitles", "language_code": "eng",
                 "language": "English"}
            ]
        }))
        .unwrap();
        let dialogue = times(1400, 10.0, 2.0);
        let evidence = SubtitleFacts {
            discdb: Some(&record),
            ..facts(vec![(1, dialogue.clone()), (2, times(2000, 11.0, 1.4)), (3, dialogue[..160].to_vec())])
        };
        let decision = decide_subtitles(&title, &evidence, &Settings::default());
        assert_eq!(decision.full, vec![1, 3]);
        assert!(decision.dropped.iter().all(|(stream, _)| *stream != 3), "{:?}", decision.dropped);
        assert!(decision.notes.iter().all(|noted| noted.note != Note::PartialSubtitleDropped), "{:?}", decision.notes);
        assert!(decision.kept.iter().any(|kept| kept.stream == 3), "{:?}", decision.kept);
        assert_eq!(decision.dropped, vec![(2, SubtitleDrop::OutsideDialogueGroup)]);
    }
}
