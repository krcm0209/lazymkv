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
