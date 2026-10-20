//! Tests of `lookup/mod.rs`: `Kind` (Task 3), and spec 8.1's identification (Task 4): the confidence table and its 0.84
//! cap, the runtime checks at their 3% and 15% edges, the description kinds over the recorded TheDiscDB answers, and
//! `identify` step by step. TheDiscDB answers are the recorded ones of `corpus/thediscdb-api` and the pinned
//! `corpus/thediscdb` files. TMDb answers are synthetic, built by `testsupport` in TMDb's documented shapes
//! (`/search/multi`, `/movie/{id}`, `/tv/{id}` with its seasons), since no search answer or full record is recorded
//! anywhere; phase 2's client checks the shapes live. Every TMDb ID other than TheDiscDB's (245891 John Wick, 13811
//! Knowing, 1437 Firefly) and the recordings' (1417, 2316) is made up for the test.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use super::discdb::{parse_dischash, parse_discid, DiscDbRecord};
use super::name::{disc_name, DiscName};
use super::tmdb::{
    parse_movie, parse_search, parse_tv, TmdbIdentity, TmdbKind, TmdbRecord, TmdbSearch, TmdbSearchResult,
};
use super::*;
use crate::review::ReviewReason;
use crate::settings::Settings;
use crate::testsupport::{search_json, tmdb_movie_json, tmdb_tv_json};

#[test]
fn kind_spells_itself_as_movie_and_tv() {
    assert_eq!(Kind::ALL, [Kind::Movie, Kind::Tv]);
    assert_eq!(Kind::ALL.map(Kind::as_str), ["movie", "tv"]);
    assert_eq!(serde_json::to_string(&Kind::Tv).unwrap(), "\"tv\"");
    assert_eq!(serde_json::from_str::<Kind>("\"movie\"").unwrap(), Kind::Movie);
    assert!(serde_json::from_str::<Kind>("\"Movie\"").is_err());
    assert_eq!("tv".parse::<Kind>(), Ok(Kind::Tv));
    assert_eq!("series".parse::<Kind>(), Err("\"series\" is not a disc kind (movie or tv)".to_string()));
}

#[test]
fn kind_and_tmdb_kind_convert_both_ways() {
    assert_eq!(Kind::from(TmdbKind::Movie), Kind::Movie);
    assert_eq!(Kind::from(TmdbKind::Tv), Kind::Tv);
    assert_eq!(TmdbKind::from(Kind::Movie), TmdbKind::Movie);
    assert_eq!(TmdbKind::from(Kind::Tv), TmdbKind::Tv);
    for kind in Kind::ALL {
        assert_eq!(Kind::from(TmdbKind::from(kind)), kind);
        assert_eq!(TmdbKind::from(kind).as_str(), kind.as_str());
    }
}

/// The settings with spec 14's tmdb_only_autofile turned off (its default is on).
fn tmdb_only_autofile_off() -> Settings {
    Settings { tmdb_only_autofile: false, ..Settings::default() }
}

#[test]
fn the_identity_confidence_table_at_the_default_settings() {
    let settings = Settings::default();
    assert!(settings.tmdb_only_autofile, "spec 14's default turns tmdb_only_autofile on");
    let table: Vec<(IdentityStep, (f64, Option<ReviewReason>))> =
        IdentityStep::ALL.iter().map(|&step| (step, identity_confidence(step, &settings))).collect();
    assert_eq!(
        table,
        vec![
            (IdentityStep::Reviewed, (1.00, None)),
            (IdentityStep::Remembered, (1.00, None)),
            (IdentityStep::DiscdbBoth, (0.99, None)),
            (IdentityStep::DiscdbOne, (0.95, None)),
            (IdentityStep::TmdbOne, (0.90, None)),
            (IdentityStep::TmdbSeveral, (0.50, Some(ReviewReason::IdentityAmbiguous))),
            (IdentityStep::Nothing, (0.0, Some(ReviewReason::IdentityUnknown))),
        ]
    );
}

#[test]
fn with_tmdb_only_autofile_off_step_4_is_capped_at_0_84_with_identity_ambiguous() {
    let settings = tmdb_only_autofile_off();
    let table: Vec<(IdentityStep, (f64, Option<ReviewReason>))> =
        IdentityStep::ALL.iter().map(|&step| (step, identity_confidence(step, &settings))).collect();
    assert_eq!(
        table,
        vec![
            (IdentityStep::Reviewed, (1.00, None)),
            (IdentityStep::Remembered, (1.00, None)),
            (IdentityStep::DiscdbBoth, (0.99, None)),
            (IdentityStep::DiscdbOne, (0.95, None)),
            (IdentityStep::TmdbOne, (0.84, Some(ReviewReason::IdentityAmbiguous))),
            (IdentityStep::TmdbSeveral, (0.50, Some(ReviewReason::IdentityAmbiguous))),
            (IdentityStep::Nothing, (0.0, Some(ReviewReason::IdentityUnknown))),
        ]
    );
}

#[test]
fn the_confidence_constants_are_spec_8_1s() {
    assert_eq!(REMEMBERED_CONFIDENCE, 1.00);
    assert_eq!(DISCDB_BOTH_CONFIDENCE, 0.99);
    assert_eq!(DISCDB_ONE_CONFIDENCE, 0.95);
    assert_eq!(TMDB_ONE_CONFIDENCE, 0.90);
    assert_eq!(TMDB_ONE_CAPPED_CONFIDENCE, 0.84);
    assert_eq!(TMDB_SEVERAL_CONFIDENCE, 0.50);
    assert_eq!(NOTHING_CONFIDENCE, 0.0);
}

#[test]
fn the_cap_sits_just_below_the_default_auto_threshold() {
    let threshold = Settings::default().auto_threshold;
    assert_eq!(threshold, 0.85);
    assert!(TMDB_ONE_CONFIDENCE >= threshold, "step 4 files automatically");
    assert!(TMDB_ONE_CAPPED_CONFIDENCE < threshold, "the cap holds the job for review");
    assert!(DISCDB_ONE_CONFIDENCE >= threshold);
    assert!(TMDB_SEVERAL_CONFIDENCE < threshold);
}

#[test]
fn identity_step_spellings_round_trip() {
    let spellings = ["reviewed", "remembered", "discdb_both", "discdb_one", "tmdb_one", "tmdb_several", "nothing"];
    assert_eq!(IdentityStep::ALL.map(IdentityStep::as_str), spellings);
    for step in IdentityStep::ALL {
        let text = step.as_str();
        assert_eq!(serde_json::to_string(&step).unwrap(), format!("\"{text}\""));
        assert_eq!(serde_json::from_str::<IdentityStep>(&format!("\"{text}\"")).unwrap(), step);
        assert_eq!(text.parse::<IdentityStep>(), Ok(step));
    }
    assert_eq!("DiscdbOne".parse::<IdentityStep>(), Err("'DiscdbOne' is not an identity step".to_string()));
}

/// RuntimeFacts with planned features or cuts only.
fn features(seconds: &[u32]) -> RuntimeFacts {
    RuntimeFacts { feature_seconds: seconds.to_vec(), longest_rival_seconds: None, episode_group_seconds: Vec::new() }
}

/// RuntimeFacts with episode-length groups only.
fn episode_groups(seconds: &[u32]) -> RuntimeFacts {
    RuntimeFacts { feature_seconds: Vec::new(), longest_rival_seconds: None, episode_group_seconds: seconds.to_vec() }
}

#[test]
fn within_is_inclusive_and_relative_to_the_second_value() {
    assert!(within(46.0, 40.0, 0.15));
    assert!(within(34.0, 40.0, 0.15));
    assert!(!within(46.5, 40.0, 0.15));
    assert!(!within(40.0, 34.0, 0.15));
    assert!(within(6180.0, 6000.0, 0.03));
    assert!(within(0.0, 0.0, 0.03));
}

#[test]
fn the_movie_runtime_check_passes_within_3_percent_and_fails_beyond() {
    assert!(movie_runtime_ok(&features(&[6180]), Some(100)));
    assert!(movie_runtime_ok(&features(&[5820]), Some(100)));
    assert!(!movie_runtime_ok(&features(&[6181]), Some(100)));
    assert!(!movie_runtime_ok(&features(&[5819]), Some(100)));
}

#[test]
fn any_planned_feature_or_cut_can_pass_the_movie_runtime_check() {
    // Two cuts (8.4): the theatrical one matches TMDb's runtime, the longer one does not.
    assert!(movie_runtime_ok(&features(&[6900, 6100]), Some(100)));
    assert!(!movie_runtime_ok(&features(&[6900, 5000]), Some(100)));
}

#[test]
fn the_longest_rival_counts_only_when_no_feature_is_planned() {
    let rival_only = RuntimeFacts { longest_rival_seconds: Some(6000), ..RuntimeFacts::default() };
    assert!(movie_runtime_ok(&rival_only, Some(100)));
    let both = RuntimeFacts { feature_seconds: vec![5000], longest_rival_seconds: Some(6000), ..RuntimeFacts::default() };
    assert!(!movie_runtime_ok(&both, Some(100)));
}

#[test]
fn the_movie_runtime_check_fails_without_a_runtime_or_a_duration() {
    assert!(!movie_runtime_ok(&features(&[6000]), None));
    assert!(!movie_runtime_ok(&RuntimeFacts::default(), Some(100)));
    assert!(!movie_runtime_ok(&episode_groups(&[6000]), Some(100)), "episode groups are not features");
}

#[test]
fn the_tv_runtime_check_passes_within_15_percent_and_fails_beyond() {
    let groups = episode_groups(&[2400, 2400, 2400]);
    assert!(tv_runtime_ok(&groups, &[46, 46, 46]));
    assert!(tv_runtime_ok(&groups, &[34]));
    assert!(!tv_runtime_ok(&groups, &[47]));
    assert!(!tv_runtime_ok(&groups, &[33]));
    // The median, not the mean: one long episode and one long group move nothing.
    assert!(tv_runtime_ok(&episode_groups(&[1200, 1320, 3000]), &[22, 22, 60]));
}

#[test]
fn the_median_of_an_even_count_is_the_mean_of_the_two_middle_values() {
    // Groups of 38 and 42 minutes: the disc median is 40.
    let groups = episode_groups(&[2520, 2280]);
    assert!(tv_runtime_ok(&groups, &[52, 40]), "(40 + 52) / 2 = 46");
    assert!(!tv_runtime_ok(&groups, &[40, 54]), "(40 + 54) / 2 = 47");
}

#[test]
fn the_tv_runtime_check_fails_when_either_list_is_empty() {
    assert!(!tv_runtime_ok(&episode_groups(&[]), &[22]));
    assert!(!tv_runtime_ok(&episode_groups(&[1320]), &[]));
    assert!(!tv_runtime_ok(&features(&[1320]), &[22]), "features are not episode groups");
}

#[test]
fn the_tolerances_are_spec_8_1s() {
    assert_eq!(MOVIE_RUNTIME_TOLERANCE, 0.03);
    assert_eq!(TV_RUNTIME_TOLERANCE, 0.15);
}

/// A file under `corpus/`, as text.
fn corpus(relative: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus").join(relative))
        .unwrap_or_else(|error| panic!("{relative}: {error}"))
}

/// Knowing's pinned TheDiscDB record (the data repository's form: no media, no TMDb ID).
const KNOWING_PINNED: &str = "thediscdb/movie/Knowing (2009)/2010-dvd/disc01.json";

#[test]
fn a_description_containing_comment_is_commentary() {
    for text in [
        "Director Commentary",
        "Commentary by Director Parker Finn",
        "Commentary - Whedon & Fillian",
        "Commentary - Espenson, Baccarin, and Trpcic",
        "director's comments",
        "COMMENTARY",
    ] {
        assert_eq!(description_kinds(text, Some("English")), vec![DescriptionKind::Commentary], "{text}");
    }
}

#[test]
fn each_audio_description_phrase_gives_audio_description() {
    for text in ["Audio Description", "English Descriptive Audio", "DVS", "Descriptive Video Service (DVS)", "For the visually impaired"] {
        assert_eq!(description_kinds(text, Some("English")), vec![DescriptionKind::AudioDescription], "{text}");
    }
}

#[test]
fn sdh_and_hearing_give_sdh() {
    for text in ["SDH", "English SDH", "For the deaf and hard of hearing"] {
        assert_eq!(description_kinds(text, Some("English")), vec![DescriptionKind::Sdh], "{text}");
    }
}

#[test]
fn isolated_score_phrases_give_isolated_score() {
    for text in ["Isolated Score", "Score only"] {
        assert_eq!(description_kinds(text, Some("English")), vec![DescriptionKind::IsolatedScore], "{text}");
    }
}

#[test]
fn music_and_effects_phrases_give_music_and_effects() {
    for text in ["Music and Effects", "Music & Effects", "M&E"] {
        assert_eq!(description_kinds(text, Some("English")), vec![DescriptionKind::MusicAndEffects], "{text}");
    }
}

#[test]
fn subtitle_or_the_tracks_own_language_gives_dialogue() {
    assert_eq!(description_kinds("English Subtitles", Some("English")), vec![DescriptionKind::Dialogue]);
    assert_eq!(description_kinds("ENglish", Some("English")), vec![DescriptionKind::Dialogue]);
    assert_eq!(description_kinds(" english ", Some("English")), vec![DescriptionKind::Dialogue]);
    assert_eq!(description_kinds("Spanish", Some("Spanish")), vec![DescriptionKind::Dialogue]);
    assert_eq!(description_kinds("subtitle", None), vec![DescriptionKind::Dialogue]);
}

#[test]
fn another_language_name_or_an_unknown_text_gives_nothing() {
    assert_eq!(description_kinds("Spanish", Some("English")), Vec::<DescriptionKind>::new());
    assert_eq!(description_kinds("English", None), Vec::<DescriptionKind>::new());
    assert_eq!(description_kinds("", Some("English")), Vec::<DescriptionKind>::new());
    assert_eq!(description_kinds("", Some("")), Vec::<DescriptionKind>::new());
    assert_eq!(description_kinds("Bonus", Some("English")), Vec::<DescriptionKind>::new());
}

#[test]
fn a_text_can_carry_several_kinds_in_order() {
    use DescriptionKind::{AudioDescription, Commentary, Dialogue, IsolatedScore, MusicAndEffects, Sdh};
    assert_eq!(description_kinds("Commentary with audio description", Some("English")), vec![Commentary, AudioDescription]);
    assert_eq!(description_kinds("SDH subtitles", Some("English")), vec![Sdh, Dialogue]);
    // A commentary description is not a plain subtitle description (Open point 29), so it is not Dialogue.
    assert_eq!(description_kinds("Commentary subtitles", Some("English")), vec![Commentary]);
    assert_eq!(description_kinds("Commentary Subtitles", Some("English")), vec![Commentary]);
    assert_eq!(description_kinds("Isolated score with music and effects", Some("English")), vec![IsolatedScore, MusicAndEffects]);
}

#[test]
fn knowings_described_tracks_read_as_dialogue_and_commentary() {
    let record: DiscDbRecord = serde_json::from_str(&corpus(KNOWING_PINNED)).unwrap();
    let title = record.titles.iter().find(|title| title.index == 0).expect("Knowing's title 0");
    let kinds: BTreeMap<u32, Vec<DescriptionKind>> = title
        .tracks
        .iter()
        .filter_map(|track| {
            let description = track.description.as_deref()?;
            Some((track.index, description_kinds(description, track.language.as_deref())))
        })
        .collect();
    assert_eq!(
        kinds,
        BTreeMap::from([
            (1, vec![DescriptionKind::Dialogue]),
            (2, vec![DescriptionKind::Dialogue]),
            (3, vec![DescriptionKind::Commentary]),
        ])
    );
}

#[test]
fn every_described_track_of_the_18_answers_is_commentary_or_its_own_language() {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/thediscdb-api");
    let mut files: Vec<String> = fs::read_dir(&folder)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|file| file.ends_with(".json"))
        .collect();
    files.sort();
    assert_eq!(files.len(), 18);
    let (mut commentary, mut dialogue) = (0, 0);
    for file in &files {
        let text = corpus(&format!("thediscdb-api/{file}"));
        let records: Vec<DiscDbRecord> = if file.starts_with("dischash-") {
            parse_dischash(&text, file).unwrap().into_iter().collect()
        } else {
            parse_discid(&text, file).unwrap()
        };
        for track in records.iter().flat_map(|record| &record.titles).flat_map(|title| &title.tracks) {
            let Some(description) = track.description.as_deref() else {
                continue;
            };
            let kinds = description_kinds(description, track.language.as_deref());
            if description.to_lowercase().contains("comment") {
                assert_eq!(kinds, vec![DescriptionKind::Commentary], "{file}: {description}");
                commentary += 1;
            } else {
                assert_eq!(kinds, vec![DescriptionKind::Dialogue], "{file}: {description} ({:?})", track.language);
                dialogue += 1;
            }
        }
    }
    assert_eq!((commentary, dialogue), (12, 76));
}

#[test]
fn description_kind_spellings_round_trip() {
    let spellings = ["commentary", "audio_description", "sdh", "isolated_score", "music_and_effects", "dialogue"];
    assert_eq!(DescriptionKind::ALL.map(DescriptionKind::as_str), spellings);
    for kind in DescriptionKind::ALL {
        let text = kind.as_str();
        assert_eq!(serde_json::to_string(&kind).unwrap(), format!("\"{text}\""));
        assert_eq!(serde_json::from_str::<DescriptionKind>(&format!("\"{text}\"")).unwrap(), kind);
        assert_eq!(text.parse::<DescriptionKind>(), Ok(kind));
    }
    assert_eq!("Sdh".parse::<DescriptionKind>(), Err("'Sdh' is not a description kind".to_string()));
}

const JOHN_WICK_UHD_BY_FINGERPRINT: &str = "thediscdb-api/dischash-76E63EBB5B66F73B0C37BFAB59B8ABA5.json";
const JOHN_WICK_UHD_BY_DISC_ID: &str = "thediscdb-api/discid-8D0355F530C31D44912831F8637FF65ACBCD6886.json";
const KNOWING_BY_DISC_ID: &str = "thediscdb-api/discid-7A3CF988A6F70F7F081EBD8A73ED3DB6.json";
const FIREFLY_D1_BY_FINGERPRINT: &str = "thediscdb-api/dischash-5A961BC67EB34171F24852D3A7688020.json";
/// John Wick's MainMovie (TheDiscDB title 0 of the UHD disc, 1:41:13).
const JOHN_WICK_MAIN_S: u32 = 6073;
/// Knowing's MainMovie (pinned title 0, 2:01:14).
const KNOWING_MAIN_S: u32 = 7274;
/// A first page of `/search/multi` for "JOHN WICK": one plausible result.
const JOHN_WICK_RESULTS: [(&str, i64, &str, &str, &str); 3] = [
    ("movie", 245891, "John Wick", "John Wick", "2014-10-22"),
    ("movie", 324552, "John Wick: Chapter 2", "John Wick: Chapter 2", "2017-02-08"),
    (
        "tv",
        94997,
        "The Continental: From the World of John Wick",
        "The Continental: From the World of John Wick",
        "2023-09-22",
    ),
];

fn by_fingerprint(relative: &str) -> DiscDbRecord {
    parse_dischash(&corpus(relative), relative).unwrap().expect("a recorded hit")
}

fn by_disc_id(relative: &str) -> DiscDbRecord {
    parse_discid(&corpus(relative), relative).unwrap().remove(0)
}

/// A TheDiscDB movie entry in the API's shape whose media has a title and a year but no TMDb ID.
fn without_tmdb_id(title: &str, year: i32) -> DiscDbRecord {
    serde_json::from_str(&format!(
        r#"{{"media": {{"title": "{title}", "full_title": "{title}", "year": {year}, "type": "Movie", "external_ids": {{}}}}, "titles": []}}"#
    ))
    .unwrap()
}

fn movie(id: i64) -> TmdbIdentity {
    TmdbIdentity { kind: TmdbKind::Movie, id }
}

fn tv(id: i64) -> TmdbIdentity {
    TmdbIdentity { kind: TmdbKind::Tv, id }
}

/// A `/search/multi` answer: (media_type, id, title or name, original, date).
fn search(results: &[(&str, i64, &str, &str, &str)]) -> TmdbSearch {
    parse_search(&search_json(results), "search.json").unwrap()
}

/// A `/movie/{id}` answer whose original language is English.
fn movie_record(id: i64, title: &str, release_date: &str, runtime: Option<u32>) -> TmdbRecord {
    parse_movie(&tmdb_movie_json(id, title, title, release_date, "en", runtime), "movie.json").unwrap()
}

/// A `/tv/{id}` answer with its seasons' episodes merged in.
fn tv_record(id: i64, name: &str, seasons: &[(u32, &[(u32, &str, Option<u32>)])]) -> TmdbRecord {
    parse_tv(&tmdb_tv_json(id, name, "2005-03-24", "en", seasons), "tv.json").unwrap()
}

/// The disc name of a volume label.
fn label(raw: &str) -> DiscName {
    disc_name(None, Some(raw)).unwrap()
}

fn sentences(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|line| line.to_string()).collect()
}

/// Example Show's season 3, as TMDb lists its first episodes (22-minute runtimes, made up).
const OFFICE_SEASON_3: [(u32, &str, Option<u32>); 3] =
    [(1, "Gay Witch Hunt", Some(22)), (2, "The Convention", Some(22)), (3, "The Coup", Some(22))];
/// A season 1 of five 30-minute episodes (made up), so the whole series' median differs from season 3's.
const OFFICE_SEASON_1: [(u32, &str, Option<u32>); 5] = [
    (1, "Pilot", Some(30)),
    (2, "Diversity Day", Some(30)),
    (3, "Health Care", Some(30)),
    (4, "The Alliance", Some(30)),
    (5, "Basketball", Some(30)),
];
/// Three 22-minute episode-length groups.
const OFFICE_GROUPS_S: [u32; 3] = [1320, 1320, 1320];

#[test]
fn the_owners_identity_comes_before_every_other_step_at_1_00() {
    let fingerprint = by_fingerprint(JOHN_WICK_UHD_BY_FINGERPRINT);
    let record = movie_record(13811, "Knowing", "2009-03-19", Some(121));
    let inputs = LookupInputs {
        discdb_by_fingerprint: Some(&fingerprint),
        record: Some(&record),
        remembered: Some(movie(245891)),
        reviewed: Some(movie(13811)),
        ..LookupInputs::default()
    };
    assert_eq!(
        identify(&inputs, Kind::Movie, &RuntimeFacts::default(), &Settings::default()),
        Identity {
            step: IdentityStep::Reviewed,
            confidence: 1.00,
            tmdb: Some(movie(13811)),
            title: Some("Knowing".to_string()),
            year: Some(2009),
            original_language: Some("en".to_string()),
            review: None,
            plausible: vec![],
            runtime_check: None,
            lookup_pending: false,
            evidence: sentences(&["the owner set the identity to movie 13811"]),
        }
    );
}

#[test]
fn a_remembered_identity_comes_before_thediscdb_at_1_00() {
    let fingerprint = by_fingerprint(FIREFLY_D1_BY_FINGERPRINT);
    let disc_id = by_disc_id(JOHN_WICK_UHD_BY_DISC_ID);
    let inputs = LookupInputs {
        discdb_by_fingerprint: Some(&fingerprint),
        discdb_by_disc_id: Some(&disc_id),
        remembered: Some(tv(1437)),
        ..LookupInputs::default()
    };
    assert_eq!(
        identify(&inputs, Kind::Tv, &RuntimeFacts::default(), &Settings::default()),
        Identity {
            step: IdentityStep::Remembered,
            confidence: 1.00,
            tmdb: Some(tv(1437)),
            title: Some("Firefly".to_string()),
            year: Some(2002),
            original_language: None,
            review: None,
            plausible: vec![],
            runtime_check: None,
            lookup_pending: false,
            evidence: sentences(&["a remembered decision names tv 1437"]),
        }
    );
}

#[test]
fn thediscdb_by_disc_id_and_fingerprint_gives_0_99() {
    let fingerprint = by_fingerprint(JOHN_WICK_UHD_BY_FINGERPRINT);
    let disc_id = by_disc_id(JOHN_WICK_UHD_BY_DISC_ID);
    let inputs = LookupInputs {
        discdb_by_fingerprint: Some(&fingerprint),
        discdb_by_disc_id: Some(&disc_id),
        ..LookupInputs::default()
    };
    assert_eq!(
        identify(&inputs, Kind::Movie, &RuntimeFacts::default(), &Settings::default()),
        Identity {
            step: IdentityStep::DiscdbBoth,
            confidence: 0.99,
            tmdb: Some(movie(245891)),
            title: Some("John Wick".to_string()),
            year: Some(2014),
            original_language: None,
            review: None,
            plausible: vec![],
            runtime_check: None,
            lookup_pending: false,
            evidence: sentences(&["TheDiscDB knows the disc ID and the fingerprint", "TheDiscDB names movie 245891"]),
        }
    );
}

#[test]
fn thediscdb_by_both_is_still_both_when_one_record_lacks_a_tmdb_id() {
    let fingerprint = by_fingerprint(JOHN_WICK_UHD_BY_FINGERPRINT);
    let disc_id = by_disc_id(JOHN_WICK_UHD_BY_DISC_ID);
    let bare = without_tmdb_id("John Wick", 2014);
    for (by_id, by_hash) in [(&bare, &fingerprint), (&disc_id, &bare)] {
        let inputs = LookupInputs {
            discdb_by_disc_id: Some(by_id),
            discdb_by_fingerprint: Some(by_hash),
            ..LookupInputs::default()
        };
        let identity = identify(&inputs, Kind::Movie, &RuntimeFacts::default(), &Settings::default());
        assert_eq!((identity.step, identity.confidence, identity.tmdb), (IdentityStep::DiscdbBoth, 0.99, Some(movie(245891))));
    }
}

#[test]
fn thediscdb_by_both_without_a_tmdb_id_searches_the_entry_title_at_0_99() {
    let bare = without_tmdb_id("Example Film", 1986);
    let results = search(&[("movie", 1001, "Example Film", "Example Film", "1986-05-09")]);
    let inputs = LookupInputs {
        discdb_by_disc_id: Some(&bare),
        discdb_by_fingerprint: Some(&bare),
        search: Some(&results),
        ..LookupInputs::default()
    };
    let identity = identify(&inputs, Kind::Movie, &RuntimeFacts::default(), &Settings::default());
    assert_eq!((identity.step, identity.confidence, identity.tmdb), (IdentityStep::DiscdbBoth, 0.99, Some(movie(1001))));
    assert_eq!(
        identity.evidence,
        sentences(&[
            "TheDiscDB knows the disc ID and the fingerprint",
            "TheDiscDB gives no TMDb ID: TMDb is searched with its title 'Example Film' and year 1986",
            "1 of 1 search results are plausible: movie 1001",
        ])
    );
}

#[test]
fn when_neither_thediscdb_record_has_a_tmdb_id_the_one_with_a_title_is_searched() {
    // The disc ID record has a title and the fingerprint's has none: the disc ID record's title is searched (8.1 "TMDb
    // is searched with the entry's title and year"), and the step stays 2.
    let by_id = without_tmdb_id("Example Film", 1986);
    let by_hash: DiscDbRecord = serde_json::from_str(r#"{"titles": []}"#).unwrap();
    let results = search(&[("movie", 1001, "Example Film", "Example Film", "1986-05-09")]);
    let inputs = LookupInputs {
        discdb_by_disc_id: Some(&by_id),
        discdb_by_fingerprint: Some(&by_hash),
        search: Some(&results),
        ..LookupInputs::default()
    };
    let identity = identify(&inputs, Kind::Movie, &RuntimeFacts::default(), &Settings::default());
    assert_eq!((identity.step, identity.confidence, identity.tmdb), (IdentityStep::DiscdbBoth, 0.99, Some(movie(1001))));
    assert_eq!(
        identity.evidence,
        sentences(&[
            "TheDiscDB knows the disc ID and the fingerprint",
            "TheDiscDB gives no TMDb ID: TMDb is searched with its title 'Example Film' and year 1986",
            "1 of 1 search results are plausible: movie 1001",
        ])
    );
}

#[test]
fn thediscdb_records_that_disagree_fall_to_the_fingerprints_at_0_95() {
    let fingerprint = by_fingerprint(JOHN_WICK_UHD_BY_FINGERPRINT);
    let disc_id = by_disc_id(KNOWING_BY_DISC_ID);
    let inputs = LookupInputs {
        discdb_by_fingerprint: Some(&fingerprint),
        discdb_by_disc_id: Some(&disc_id),
        ..LookupInputs::default()
    };
    let identity = identify(&inputs, Kind::Movie, &RuntimeFacts::default(), &Settings::default());
    assert_eq!((identity.step, identity.confidence, identity.tmdb), (IdentityStep::DiscdbOne, 0.95, Some(movie(245891))));
    assert_eq!(
        identity.evidence,
        sentences(&[
            "TheDiscDB knows the disc ID and the fingerprint",
            "TheDiscDB's disc ID record names movie 13811 and its fingerprint record movie 245891: the fingerprint's is used",
            "TheDiscDB names movie 245891",
        ])
    );
}

#[test]
fn thediscdb_by_fingerprint_alone_gives_0_95() {
    let fingerprint = by_fingerprint(FIREFLY_D1_BY_FINGERPRINT);
    let name = label("FIREFLY_D1");
    let inputs = LookupInputs { name: Some(&name), discdb_by_fingerprint: Some(&fingerprint), ..LookupInputs::default() };
    assert_eq!(
        identify(&inputs, Kind::Tv, &RuntimeFacts::default(), &Settings::default()),
        Identity {
            step: IdentityStep::DiscdbOne,
            confidence: 0.95,
            tmdb: Some(tv(1437)),
            title: Some("Firefly".to_string()),
            year: Some(2002),
            original_language: None,
            review: None,
            plausible: vec![],
            runtime_check: None,
            lookup_pending: false,
            evidence: sentences(&[
                "name 'FIREFLY_D1' cleaned to 'FIREFLY'",
                "disc 1 parsed from the name",
                "TheDiscDB knows the fingerprint",
                "TheDiscDB names tv 1437",
            ]),
        }
    );
}

#[test]
fn thediscdb_by_disc_id_alone_gives_0_95() {
    let disc_id = by_disc_id(JOHN_WICK_UHD_BY_DISC_ID);
    let inputs = LookupInputs { discdb_by_disc_id: Some(&disc_id), ..LookupInputs::default() };
    let identity = identify(&inputs, Kind::Movie, &RuntimeFacts::default(), &Settings::default());
    assert_eq!((identity.step, identity.confidence, identity.tmdb), (IdentityStep::DiscdbOne, 0.95, Some(movie(245891))));
    assert_eq!(identity.evidence, sentences(&["TheDiscDB knows the disc ID", "TheDiscDB names movie 245891"]));
}

#[test]
fn the_fetched_record_gives_the_title_year_and_original_language() {
    let fingerprint = by_fingerprint(JOHN_WICK_UHD_BY_FINGERPRINT);
    let record = movie_record(245891, "John Wick", "2014-10-22", Some(101));
    let inputs = LookupInputs { discdb_by_fingerprint: Some(&fingerprint), record: Some(&record), ..LookupInputs::default() };
    let identity = identify(&inputs, Kind::Movie, &RuntimeFacts::default(), &Settings::default());
    assert_eq!(
        (identity.title.as_deref(), identity.year, identity.original_language.as_deref()),
        (Some("John Wick"), Some(2014), Some("en"))
    );
    // A record of another identity is not this disc's: the title and year come from TheDiscDB's media, and there is
    // no original language.
    let other = movie_record(13811, "Knowing", "2009-03-19", Some(121));
    let inputs = LookupInputs { discdb_by_fingerprint: Some(&fingerprint), record: Some(&other), ..LookupInputs::default() };
    let identity = identify(&inputs, Kind::Movie, &RuntimeFacts::default(), &Settings::default());
    assert_eq!((identity.title.as_deref(), identity.year, identity.original_language), (Some("John Wick"), Some(2014), None));
}

#[test]
fn an_entry_without_a_tmdb_id_is_searched_by_its_title_kind_and_year() {
    let fingerprint = without_tmdb_id("Example Film", 1986);
    let results = search(&[
        ("movie", 1001, "Example Film", "Example Film", "1986-05-09"),
        ("movie", 1002, "Example Film", "Example Film", "2021-03-01"),
        ("tv", 1003, "Example Film", "Example Film", "2020-01-24"),
    ]);
    let inputs = LookupInputs { discdb_by_fingerprint: Some(&fingerprint), search: Some(&results), ..LookupInputs::default() };
    assert_eq!(
        identify(&inputs, Kind::Tv, &RuntimeFacts::default(), &Settings::default()),
        Identity {
            step: IdentityStep::DiscdbOne,
            confidence: 0.95,
            tmdb: Some(movie(1001)),
            title: Some("Example Film".to_string()),
            year: Some(1986),
            original_language: None,
            review: None,
            plausible: vec![movie(1001)],
            runtime_check: None,
            lookup_pending: false,
            evidence: sentences(&[
                "TheDiscDB knows the fingerprint",
                "TheDiscDB gives no TMDb ID: TMDb is searched with its title 'Example Film' and year 1986",
                "3 of 3 search results are plausible: movie 1001, movie 1002, tv 1003",
                "preferring movie results leaves movie 1001, movie 1002",
                "preferring results from 1986 leaves movie 1001",
            ]),
        }
    );
}

#[test]
fn an_entry_without_a_tmdb_id_and_several_results_is_ambiguous() {
    let fingerprint = without_tmdb_id("Example Film", 1986);
    let results = search(&[
        ("movie", 1001, "Example Film", "Example Film", "1986-05-09"),
        ("movie", 1004, "Example Film", "Example Film", "1986-11-01"),
    ]);
    let inputs = LookupInputs { discdb_by_fingerprint: Some(&fingerprint), search: Some(&results), ..LookupInputs::default() };
    assert_eq!(
        identify(&inputs, Kind::Movie, &RuntimeFacts::default(), &Settings::default()),
        Identity {
            step: IdentityStep::TmdbSeveral,
            confidence: 0.50,
            tmdb: None,
            title: None,
            year: None,
            original_language: None,
            review: Some(ReviewReason::IdentityAmbiguous),
            plausible: vec![movie(1001), movie(1004)],
            runtime_check: None,
            lookup_pending: false,
            evidence: sentences(&[
                "TheDiscDB knows the fingerprint",
                "TheDiscDB gives no TMDb ID: TMDb is searched with its title 'Example Film' and year 1986",
                "2 of 2 search results are plausible: movie 1001, movie 1004",
            ]),
        }
    );
}

#[test]
fn an_entry_that_states_no_kind_is_never_narrowed_by_the_structural_kind() {
    // TheDiscDB's media has a title and a year but no type: the structural kind may only pick the result shown
    // (settled items 5 and 6), so two same-year results of different kinds stay ambiguous.
    let fingerprint: DiscDbRecord = serde_json::from_str(
        r#"{"media": {"title": "Example Film", "full_title": "Example Film", "year": 1986, "external_ids": {}}, "titles": []}"#,
    )
    .unwrap();
    assert_eq!(fingerprint.kind(), None);
    let results = search(&[
        ("movie", 1001, "Example Film", "Example Film", "1986-05-09"),
        ("tv", 1003, "Example Film", "Example Film", "1986-01-24"),
    ]);
    let inputs = LookupInputs { discdb_by_fingerprint: Some(&fingerprint), search: Some(&results), ..LookupInputs::default() };
    let identity = identify(&inputs, Kind::Movie, &RuntimeFacts::default(), &Settings::default());
    assert_eq!(
        (identity.step, identity.confidence, identity.tmdb, identity.review),
        (IdentityStep::TmdbSeveral, 0.50, None, Some(ReviewReason::IdentityAmbiguous))
    );
    assert_eq!(identity.plausible, vec![movie(1001), tv(1003)]);
    assert_eq!(
        identity.evidence,
        sentences(&[
            "TheDiscDB knows the fingerprint",
            "TheDiscDB gives no TMDb ID: TMDb is searched with its title 'Example Film' and year 1986",
            "2 of 2 search results are plausible: movie 1001, tv 1003",
        ])
    );
}

#[test]
fn an_entry_without_a_tmdb_id_and_no_plausible_result_is_unknown() {
    let fingerprint = without_tmdb_id("Example Film", 1986);
    let results = search(&[("movie", 1002, "Example Film 2", "Example Film 2", "1988-07-06")]);
    let inputs = LookupInputs { discdb_by_fingerprint: Some(&fingerprint), search: Some(&results), ..LookupInputs::default() };
    let identity = identify(&inputs, Kind::Movie, &RuntimeFacts::default(), &Settings::default());
    assert_eq!(
        (identity.step, identity.confidence, identity.tmdb, identity.review),
        (IdentityStep::Nothing, 0.0, None, Some(ReviewReason::IdentityUnknown))
    );
    assert_eq!(identity.evidence.last().map(String::as_str), Some("0 of 1 search results are plausible"));
    let inputs = LookupInputs { discdb_by_fingerprint: Some(&fingerprint), ..LookupInputs::default() };
    let identity = identify(&inputs, Kind::Movie, &RuntimeFacts::default(), &Settings::default());
    assert_eq!(identity.step, IdentityStep::Nothing);
    assert_eq!(identity.evidence.last().map(String::as_str), Some("no TMDb search answer is in hand"));
}

#[test]
fn an_entry_without_a_tmdb_id_never_takes_a_result_of_another_year_or_kind() {
    let fingerprint = without_tmdb_id("Example Film", 1986);
    for (media_type, id, date) in [("movie", 1002, "2021-03-01"), ("tv", 1003, "1986-01-24")] {
        let results = search(&[(media_type, id, "Example Film", "Example Film", date)]);
        let inputs =
            LookupInputs { discdb_by_fingerprint: Some(&fingerprint), search: Some(&results), ..LookupInputs::default() };
        let identity = identify(&inputs, Kind::Movie, &RuntimeFacts::default(), &Settings::default());
        assert_eq!(
            (identity.step, identity.confidence, identity.review),
            (IdentityStep::TmdbSeveral, 0.50, Some(ReviewReason::IdentityAmbiguous))
        );
        let expected = format!("{media_type} {id} is not TheDiscDB's kind and year");
        assert_eq!(identity.evidence.last().map(String::as_str), Some(expected.as_str()));
    }
}

#[test]
fn an_entry_without_a_tmdb_id_or_a_title_falls_back_to_the_disc_name() {
    let pinned: DiscDbRecord = serde_json::from_str(&corpus(KNOWING_PINNED)).unwrap();
    let name = label("KNOWING");
    let inputs = LookupInputs { name: Some(&name), discdb_by_fingerprint: Some(&pinned), ..LookupInputs::default() };
    let identity = identify(&inputs, Kind::Movie, &features(&[KNOWING_MAIN_S]), &Settings::default());
    assert_eq!((identity.step, identity.confidence), (IdentityStep::Nothing, 0.0));
    assert_eq!(
        identity.evidence,
        sentences(&[
            "name 'KNOWING' cleaned to 'KNOWING'",
            "TheDiscDB knows the fingerprint",
            "TheDiscDB gives neither a TMDb ID nor a title",
            "no TMDb search answer is in hand",
        ])
    );
    let results = search(&[("movie", 13811, "Knowing", "Knowing", "2009-03-19")]);
    let record = movie_record(13811, "Knowing", "2009-03-19", Some(121));
    let inputs = LookupInputs {
        name: Some(&name),
        discdb_by_fingerprint: Some(&pinned),
        search: Some(&results),
        record: Some(&record),
        ..LookupInputs::default()
    };
    let identity = identify(&inputs, Kind::Movie, &features(&[KNOWING_MAIN_S]), &Settings::default());
    assert_eq!(
        (identity.step, identity.confidence, identity.tmdb, identity.runtime_check),
        (IdentityStep::TmdbOne, 0.90, Some(movie(13811)), Some(true))
    );
}

#[test]
fn one_plausible_result_with_a_passing_runtime_check_gives_0_90() {
    let name = label("JOHN_WICK_UHD");
    let results = search(&JOHN_WICK_RESULTS);
    let record = movie_record(245891, "John Wick", "2014-10-22", Some(101));
    let inputs = LookupInputs { name: Some(&name), search: Some(&results), record: Some(&record), ..LookupInputs::default() };
    assert_eq!(
        identify(&inputs, Kind::Movie, &features(&[JOHN_WICK_MAIN_S]), &Settings::default()),
        Identity {
            step: IdentityStep::TmdbOne,
            confidence: 0.90,
            tmdb: Some(movie(245891)),
            title: Some("John Wick".to_string()),
            year: Some(2014),
            original_language: Some("en".to_string()),
            review: None,
            plausible: vec![movie(245891)],
            runtime_check: Some(true),
            lookup_pending: false,
            evidence: sentences(&[
                "name 'JOHN_WICK_UHD' cleaned to 'JOHN WICK'",
                "1 of 3 search results are plausible: movie 245891",
                "the runtime check passes",
            ]),
        }
    );
}

#[test]
fn with_tmdb_only_autofile_off_identify_caps_step_4_at_0_84() {
    let name = label("JOHN_WICK_UHD");
    let results = search(&JOHN_WICK_RESULTS);
    let record = movie_record(245891, "John Wick", "2014-10-22", Some(101));
    let inputs = LookupInputs { name: Some(&name), search: Some(&results), record: Some(&record), ..LookupInputs::default() };
    let identity = identify(&inputs, Kind::Movie, &features(&[JOHN_WICK_MAIN_S]), &tmdb_only_autofile_off());
    assert_eq!(
        (identity.step, identity.confidence, identity.review, identity.tmdb),
        (IdentityStep::TmdbOne, 0.84, Some(ReviewReason::IdentityAmbiguous), Some(movie(245891)))
    );
}

#[test]
fn a_failed_runtime_check_gives_0_50_with_identity_ambiguous() {
    let name = label("JOHN_WICK_UHD");
    let results = search(&JOHN_WICK_RESULTS);
    let record = movie_record(245891, "John Wick", "2014-10-22", Some(101));
    let inputs = LookupInputs { name: Some(&name), search: Some(&results), record: Some(&record), ..LookupInputs::default() };
    for settings in [Settings::default(), tmdb_only_autofile_off()] {
        let identity = identify(&inputs, Kind::Movie, &features(&[5400]), &settings);
        assert_eq!(
            (identity.step, identity.confidence, identity.review, identity.tmdb, identity.runtime_check),
            (IdentityStep::TmdbSeveral, 0.50, Some(ReviewReason::IdentityAmbiguous), Some(movie(245891)), Some(false))
        );
        assert_eq!(identity.plausible, vec![movie(245891)]);
        assert_eq!(identity.evidence.last().map(String::as_str), Some("the runtime check fails"));
    }
}

#[test]
fn a_tmdb_record_without_a_runtime_cannot_pass_step_4() {
    let name = label("JOHN_WICK_UHD");
    let results = search(&JOHN_WICK_RESULTS);
    let record = movie_record(245891, "John Wick", "2014-10-22", None);
    let inputs = LookupInputs { name: Some(&name), search: Some(&results), record: Some(&record), ..LookupInputs::default() };
    let identity = identify(&inputs, Kind::Movie, &features(&[JOHN_WICK_MAIN_S]), &Settings::default());
    assert_eq!(
        (identity.step, identity.confidence, identity.review, identity.tmdb, identity.runtime_check),
        (IdentityStep::TmdbSeveral, 0.50, Some(ReviewReason::IdentityAmbiguous), Some(movie(245891)), None)
    );
    assert_eq!(
        identity.evidence.last().map(String::as_str),
        Some("no runtime check: the TMDb record for movie 245891 has no runtime")
    );
}

#[test]
fn without_the_record_of_the_plausible_result_the_runtime_check_cannot_run() {
    let name = label("JOHN_WICK_UHD");
    let results = search(&JOHN_WICK_RESULTS);
    let other = movie_record(13811, "Knowing", "2009-03-19", Some(101));
    for record in [None, Some(&other)] {
        let inputs = LookupInputs { name: Some(&name), search: Some(&results), record, ..LookupInputs::default() };
        let identity = identify(&inputs, Kind::Movie, &features(&[JOHN_WICK_MAIN_S]), &Settings::default());
        assert_eq!(
            (identity.step, identity.tmdb, identity.runtime_check),
            (IdentityStep::TmdbSeveral, Some(movie(245891)), None)
        );
        // The title and year then come from the one plausible result.
        assert_eq!((identity.title.as_deref(), identity.year, identity.original_language), (Some("John Wick"), Some(2014), None));
        assert_eq!(identity.evidence.last().map(String::as_str), Some("no runtime check: no TMDb record for movie 245891"));
    }
}

#[test]
fn several_plausible_results_give_0_50_with_identity_ambiguous() {
    let name = label("EXAMPLE_FILM");
    let results = search(&[
        ("movie", 1001, "Example Film", "Example Film", "1986-05-09"),
        ("movie", 1002, "Example Film", "Example Film", "2021-03-01"),
    ]);
    let inputs = LookupInputs { name: Some(&name), search: Some(&results), ..LookupInputs::default() };
    assert_eq!(
        identify(&inputs, Kind::Movie, &features(&[6000]), &Settings::default()),
        Identity {
            step: IdentityStep::TmdbSeveral,
            confidence: 0.50,
            tmdb: None,
            title: None,
            year: None,
            original_language: None,
            review: Some(ReviewReason::IdentityAmbiguous),
            plausible: vec![movie(1001), movie(1002)],
            runtime_check: None,
            lookup_pending: false,
            evidence: sentences(&[
                "name 'EXAMPLE_FILM' cleaned to 'EXAMPLE FILM'",
                "2 of 2 search results are plausible: movie 1001, movie 1002",
            ]),
        }
    );
}

#[test]
fn the_structural_kind_picks_the_result_shown_but_several_plausible_stay_step_5() {
    let name = label("EXAMPLE SHOW S3D3");
    let record = tv_record(2316, "Example Show", &[(3, &OFFICE_SEASON_3[..])]);
    let two = search(&[
        ("tv", 2316, "Example Show", "Example Show", "2005-03-24"),
        ("movie", 3003, "Example Show", "Example Show", "2010-01-01"),
    ]);
    let inputs = LookupInputs { name: Some(&name), search: Some(&two), record: Some(&record), ..LookupInputs::default() };
    let identity = identify(&inputs, Kind::Tv, &episode_groups(&OFFICE_GROUPS_S), &Settings::default());
    // Spec 8.1 row 5 "TMDb search with several plausible results": preferring the structural kind picks the show
    // shown for review, and its runtime check runs and passes, but two plausible results are never step 4.
    assert_eq!(
        (identity.step, identity.confidence, identity.tmdb, identity.runtime_check),
        (IdentityStep::TmdbSeveral, 0.50, Some(tv(2316)), Some(true))
    );
    assert_eq!(identity.review, Some(ReviewReason::IdentityAmbiguous));
    assert_eq!(identity.plausible, vec![tv(2316), movie(3003)]);
    assert_eq!(
        identity.evidence,
        sentences(&[
            "name 'EXAMPLE SHOW S3D3' cleaned to 'EXAMPLE SHOW'",
            "season 3 parsed from the name",
            "disc 3 parsed from the name",
            "2 of 2 search results are plausible: tv 2316, movie 3003",
            "preferring tv results leaves tv 2316",
            "the runtime check passes",
        ])
    );
    // Two shows of that name stay two: step 5 with no result picked, and every plausible result listed.
    let three = search(&[
        ("tv", 2316, "Example Show", "Example Show", "2005-03-24"),
        ("movie", 3003, "Example Show", "Example Show", "2010-01-01"),
        ("tv", 2996, "Example Show", "Example Show", "2001-07-09"),
    ]);
    let inputs = LookupInputs { name: Some(&name), search: Some(&three), record: Some(&record), ..LookupInputs::default() };
    let identity = identify(&inputs, Kind::Tv, &episode_groups(&OFFICE_GROUPS_S), &Settings::default());
    assert_eq!(
        (identity.step, identity.tmdb, identity.plausible),
        (IdentityStep::TmdbSeveral, None, vec![tv(2316), movie(3003), tv(2996)])
    );
}

#[test]
fn the_runtime_check_is_the_one_of_the_results_own_kind() {
    // The structure said movie, but the one plausible result is a show: its TV check runs over the episode groups.
    let name = label("EXAMPLE SHOW S3D3");
    let record = tv_record(2316, "Example Show", &[(3, &OFFICE_SEASON_3[..])]);
    let results = search(&[("tv", 2316, "Example Show", "Example Show", "2005-03-24")]);
    let facts = RuntimeFacts {
        feature_seconds: vec![5400],
        longest_rival_seconds: None,
        episode_group_seconds: OFFICE_GROUPS_S.to_vec(),
    };
    assert!(!movie_runtime_ok(&facts, Some(22)), "a movie check over the same facts would fail");
    let inputs = LookupInputs { name: Some(&name), search: Some(&results), record: Some(&record), ..LookupInputs::default() };
    let identity = identify(&inputs, Kind::Movie, &facts, &Settings::default());
    assert_eq!(
        (identity.step, identity.tmdb, identity.runtime_check),
        (IdentityStep::TmdbOne, Some(tv(2316)), Some(true))
    );
}

#[test]
fn the_runtime_check_of_a_movie_result_reads_the_features_even_when_the_structure_says_tv() {
    // The structure said TV, but the one plausible result is a film: its movie check runs over the planned features.
    let name = label("EXAMPLE_FILM");
    let results = search(&[("movie", 1001, "Example Film", "Example Film", "1986-05-09")]);
    let record = movie_record(1001, "Example Film", "1986-05-09", Some(100));
    let inputs = LookupInputs { name: Some(&name), search: Some(&results), record: Some(&record), ..LookupInputs::default() };
    let held = identify(&inputs, Kind::Tv, &episode_groups(&[6000]), &Settings::default());
    assert_eq!(
        (held.step, held.confidence, held.tmdb, held.runtime_check, held.review),
        (IdentityStep::TmdbSeveral, 0.50, Some(movie(1001)), Some(false), Some(ReviewReason::IdentityAmbiguous))
    );
    assert_eq!(held.evidence.last().map(String::as_str), Some("the runtime check fails"));
    let found = identify(&inputs, Kind::Tv, &features(&[6000]), &Settings::default());
    assert_eq!((found.step, found.confidence, found.runtime_check), (IdentityStep::TmdbOne, 0.90, Some(true)));
}

#[test]
fn the_tv_runtime_check_reads_the_parsed_season_else_the_whole_series() {
    let record = tv_record(2316, "Example Show", &[(1, &OFFICE_SEASON_1[..]), (3, &OFFICE_SEASON_3[..])]);
    let results = search(&[("tv", 2316, "Example Show", "Example Show", "2005-03-24")]);
    // Season 3 parsed: its 22-minute median matches the 22-minute groups.
    let season_three = label("EXAMPLE SHOW S3D3");
    let inputs = LookupInputs { name: Some(&season_three), search: Some(&results), record: Some(&record), ..LookupInputs::default() };
    let identity = identify(&inputs, Kind::Tv, &episode_groups(&OFFICE_GROUPS_S), &Settings::default());
    assert_eq!((identity.step, identity.runtime_check), (IdentityStep::TmdbOne, Some(true)));
    // No season parsed: the whole series' median is (30 + 30) / 2 = 30 minutes, more than 15% from 22.
    let no_season = label("EXAMPLE SHOW");
    let inputs = LookupInputs { name: Some(&no_season), search: Some(&results), record: Some(&record), ..LookupInputs::default() };
    let identity = identify(&inputs, Kind::Tv, &episode_groups(&OFFICE_GROUPS_S), &Settings::default());
    assert_eq!(
        (identity.step, identity.review, identity.runtime_check),
        (IdentityStep::TmdbSeveral, Some(ReviewReason::IdentityAmbiguous), Some(false))
    );
}

#[test]
fn a_parsed_season_the_record_lacks_cannot_pass_step_4() {
    let record = tv_record(2316, "Example Show", &[(1, &OFFICE_SEASON_1[..]), (3, &OFFICE_SEASON_3[..])]);
    let results = search(&[("tv", 2316, "Example Show", "Example Show", "2005-03-24")]);
    let name = label("EXAMPLE SHOW S4D1");
    let inputs = LookupInputs { name: Some(&name), search: Some(&results), record: Some(&record), ..LookupInputs::default() };
    let identity = identify(&inputs, Kind::Tv, &episode_groups(&OFFICE_GROUPS_S), &Settings::default());
    assert_eq!((identity.step, identity.tmdb, identity.runtime_check), (IdentityStep::TmdbSeveral, Some(tv(2316)), None));
    assert_eq!(
        identity.evidence.last().map(String::as_str),
        Some("no runtime check: the TMDb record for tv 2316 has no episode runtime for season 4")
    );
}

#[test]
fn a_show_without_episode_runtimes_and_no_parsed_season_cannot_pass_step_4() {
    let name = label("EXAMPLE SHOW");
    let record = tv_record(2316, "Example Show", &[]);
    let results = search(&[("tv", 2316, "Example Show", "Example Show", "2005-03-24")]);
    let inputs = LookupInputs { name: Some(&name), search: Some(&results), record: Some(&record), ..LookupInputs::default() };
    let identity = identify(&inputs, Kind::Tv, &episode_groups(&OFFICE_GROUPS_S), &Settings::default());
    assert_eq!(
        (identity.step, identity.tmdb, identity.runtime_check),
        (IdentityStep::TmdbSeveral, Some(tv(2316)), None)
    );
    assert_eq!(
        identity.evidence.last().map(String::as_str),
        Some("no runtime check: the TMDb record for tv 2316 has no episode runtime")
    );
}

#[test]
fn a_generic_name_is_never_searched() {
    let name = label("DVD_VIDEO");
    let results = search(&[("movie", 5000, "Video", "Video", "2000-01-01")]);
    let inputs = LookupInputs { name: Some(&name), search: Some(&results), ..LookupInputs::default() };
    assert_eq!(
        identify(&inputs, Kind::Movie, &features(&[6606]), &Settings::default()),
        Identity {
            step: IdentityStep::Nothing,
            confidence: 0.0,
            tmdb: None,
            title: None,
            year: None,
            original_language: None,
            review: Some(ReviewReason::IdentityUnknown),
            plausible: vec![],
            runtime_check: None,
            lookup_pending: false,
            evidence: sentences(&[
                "name 'DVD_VIDEO' cleaned to 'VIDEO'",
                "name 'DVD_VIDEO' is generic: TMDb is not searched",
            ]),
        }
    );
}

#[test]
fn without_a_name_or_a_search_answer_identity_is_unknown() {
    let results = search(&JOHN_WICK_RESULTS);
    let inputs = LookupInputs { search: Some(&results), ..LookupInputs::default() };
    let identity = identify(&inputs, Kind::Movie, &features(&[JOHN_WICK_MAIN_S]), &Settings::default());
    assert_eq!((identity.step, identity.review), (IdentityStep::Nothing, Some(ReviewReason::IdentityUnknown)));
    assert_eq!(identity.evidence, sentences(&["the disc has no name: TMDb is not searched"]));
    let name = label("JOHN_WICK_UHD");
    let inputs = LookupInputs { name: Some(&name), ..LookupInputs::default() };
    let identity = identify(&inputs, Kind::Movie, &features(&[JOHN_WICK_MAIN_S]), &Settings::default());
    assert_eq!((identity.step, identity.confidence), (IdentityStep::Nothing, 0.0));
    assert_eq!(
        identity.evidence,
        sentences(&["name 'JOHN_WICK_UHD' cleaned to 'JOHN WICK'", "no TMDb search answer is in hand"])
    );
}

#[test]
fn no_plausible_result_gives_nothing_at_0() {
    let name = label("SMILE");
    let results = search(&[("movie", 2002, "Smile 2", "Smile 2", "2024-10-16")]);
    let inputs = LookupInputs { name: Some(&name), search: Some(&results), ..LookupInputs::default() };
    let identity = identify(&inputs, Kind::Movie, &features(&[6900]), &Settings::default());
    assert_eq!(
        (identity.step, identity.confidence, identity.review, identity.plausible),
        (IdentityStep::Nothing, 0.0, Some(ReviewReason::IdentityUnknown), vec![])
    );
    assert_eq!(identity.evidence.last().map(String::as_str), Some("0 of 1 search results are plausible"));
}

#[test]
fn a_person_result_is_never_plausible() {
    let name = label("JOHN_WICK_UHD");
    let results = TmdbSearch {
        results: vec![TmdbSearchResult {
            media_type: "person".to_string(),
            id: 7,
            title: None,
            name: Some("John Wick".to_string()),
            original_title: None,
            original_name: Some("John Wick".to_string()),
            release_date: None,
            first_air_date: None,
        }],
    };
    let inputs = LookupInputs { name: Some(&name), search: Some(&results), ..LookupInputs::default() };
    let identity = identify(&inputs, Kind::Movie, &features(&[JOHN_WICK_MAIN_S]), &Settings::default());
    assert_eq!(identity.step, IdentityStep::Nothing);
    assert_eq!(identity.evidence.last().map(String::as_str), Some("0 of 1 search results are plausible"));
}

/// Two films of one name, the later one in 3D (H1): Piranha 3D (2010, 88 minutes) and Piranha (1978, 94 minutes). The
/// TMDb IDs are made up.
const PIRANHA_RESULTS: [(&str, i64, &str, &str, &str); 2] = [
    ("movie", 3301, "Piranha 3D", "Piranha 3D", "2010-08-20"),
    ("movie", 3302, "Piranha", "Piranha", "1978-08-03"),
];

#[test]
fn a_result_matching_the_name_with_its_tags_kept_is_the_only_plausible_one() {
    // H1: `PIRANHA_3D` cleans to `PIRANHA`, which only the 1978 film matches; the name with its tags kept, `PIRANHA 3D`,
    // matches the 2010 film, and the more specific name wins.
    let name = label("PIRANHA_3D");
    let results = search(&PIRANHA_RESULTS);
    let record = movie_record(3301, "Piranha 3D", "2010-08-20", Some(88));
    let inputs = LookupInputs { name: Some(&name), search: Some(&results), record: Some(&record), ..LookupInputs::default() };
    assert_eq!(
        identify(&inputs, Kind::Movie, &features(&[88 * 60]), &Settings::default()),
        Identity {
            step: IdentityStep::TmdbOne,
            confidence: 0.90,
            tmdb: Some(movie(3301)),
            title: Some("Piranha 3D".to_string()),
            year: Some(2010),
            original_language: Some("en".to_string()),
            review: None,
            plausible: vec![movie(3301)],
            runtime_check: Some(true),
            lookup_pending: false,
            evidence: sentences(&[
                "name 'PIRANHA_3D' cleaned to 'PIRANHA'",
                "1 of 2 search results are plausible by the name with its tags kept, 'PIRANHA 3D': movie 3301",
                "the runtime check passes",
            ]),
        }
    );
}

#[test]
fn a_bdmt_name_ending_in_a_region_tag_word_matches_with_its_tags_kept() {
    // H1: the word Us of `The Last of Us` is a region tag, so the name cleans to `The Last of`, which no result matches;
    // with its tags kept it is itself, and only the show of that title is plausible, not "The Last".
    let name = disc_name(Some("The Last of Us"), None).unwrap();
    let results = search(&[
        ("tv", 4401, "The Last of Us", "The Last of Us", "2023-01-15"),
        ("movie", 4402, "The Last", "The Last", "2019-05-10"),
    ]);
    let inputs = LookupInputs { name: Some(&name), search: Some(&results), ..LookupInputs::default() };
    let identity = identify(&inputs, Kind::Tv, &episode_groups(&OFFICE_GROUPS_S), &Settings::default());
    assert_eq!((identity.tmdb, identity.plausible), (Some(tv(4401)), vec![tv(4401)]));
    assert_eq!(
        identity.evidence[..2],
        sentences(&[
            "name 'The Last of Us' cleaned to 'The Last of'",
            "1 of 2 search results are plausible by the name with its tags kept, 'The Last of Us': tv 4401",
        ])
    );
}

#[test]
fn a_name_without_tags_is_matched_by_its_cleaned_name_as_before() {
    // H1 changes nothing for a name without a region or format tag: `SMILE` matches "Smile" only.
    let name = label("SMILE");
    let results = search(&[
        ("movie", 5501, "Smile", "Smile", "2022-09-23"),
        ("movie", 5502, "Smile 2", "Smile 2", "2024-10-16"),
    ]);
    let inputs = LookupInputs { name: Some(&name), search: Some(&results), ..LookupInputs::default() };
    let identity = identify(&inputs, Kind::Movie, &features(&[6900]), &Settings::default());
    assert_eq!((identity.tmdb, identity.plausible), (Some(movie(5501)), vec![movie(5501)]));
    assert_eq!(identity.evidence[1], "1 of 2 search results are plausible: movie 5501");
}

#[test]
fn the_tagged_match_wins_over_the_cleaned_one_and_s3d_is_kept_as_written() {
    // H1 with made-up titles: `EXAMPLE_3D` matches "Example 3D" with its tags kept, so "Example" is not plausible.
    let results = search(&[
        ("movie", 6601, "Example", "Example", "2001-01-01"),
        ("movie", 6602, "Example 3D", "Example 3D", "2011-01-01"),
    ]);
    let three_d = label("EXAMPLE_3D");
    let inputs = LookupInputs { name: Some(&three_d), search: Some(&results), ..LookupInputs::default() };
    let identity = identify(&inputs, Kind::Movie, &features(&[6000]), &Settings::default());
    assert_eq!((identity.tmdb, identity.plausible), (Some(movie(6602)), vec![movie(6602)]));
    // The tag is kept as written: `EXAMPLE_S3D` with its tags kept is `EXAMPLE S3D`, which no title matches, so the
    // cleaned name decides and "Example" is the plausible result.
    let s3d = label("EXAMPLE_S3D");
    let inputs = LookupInputs { name: Some(&s3d), search: Some(&results), ..LookupInputs::default() };
    let identity = identify(&inputs, Kind::Movie, &features(&[6000]), &Settings::default());
    assert_eq!((identity.tmdb, identity.plausible), (Some(movie(6601)), vec![movie(6601)]));
    assert_eq!(identity.evidence[1], "1 of 2 search results are plausible: movie 6601");
}

#[test]
fn a_pending_lookup_does_not_lower_the_step_reached() {
    // TheDiscDB answered by fingerprint; its other lookup is still pending.
    let fingerprint = by_fingerprint(JOHN_WICK_UHD_BY_FINGERPRINT);
    let inputs = LookupInputs { discdb_by_fingerprint: Some(&fingerprint), discdb_pending: true, ..LookupInputs::default() };
    let identity = identify(&inputs, Kind::Movie, &RuntimeFacts::default(), &Settings::default());
    assert_eq!((identity.step, identity.confidence, identity.lookup_pending), (IdentityStep::DiscdbOne, 0.95, true));
    assert_eq!(identity.evidence[0], "the TheDiscDB lookup is pending");
    // TMDb unreachable: planning proceeds on structure alone, the identity is unknown and pending.
    let name = label("JOHN_WICK_UHD");
    let inputs = LookupInputs { name: Some(&name), tmdb_pending: true, ..LookupInputs::default() };
    let identity = identify(&inputs, Kind::Movie, &features(&[JOHN_WICK_MAIN_S]), &Settings::default());
    assert_eq!((identity.step, identity.lookup_pending), (IdentityStep::Nothing, true));
    assert_eq!(
        identity.evidence,
        sentences(&[
            "the TMDb lookup is pending",
            "name 'JOHN_WICK_UHD' cleaned to 'JOHN WICK'",
            "no TMDb search answer is in hand",
        ])
    );
    // The search is in hand but the record is pending: step 5 on what is in hand.
    let results = search(&JOHN_WICK_RESULTS);
    let inputs = LookupInputs { name: Some(&name), search: Some(&results), tmdb_pending: true, ..LookupInputs::default() };
    let identity = identify(&inputs, Kind::Movie, &features(&[JOHN_WICK_MAIN_S]), &Settings::default());
    assert_eq!(
        (identity.step, identity.tmdb, identity.lookup_pending),
        (IdentityStep::TmdbSeveral, Some(movie(245891)), true)
    );
}

#[test]
fn the_identity_serialises_with_its_fields_in_order() {
    let fingerprint = by_fingerprint(JOHN_WICK_UHD_BY_FINGERPRINT);
    let inputs = LookupInputs { discdb_by_fingerprint: Some(&fingerprint), ..LookupInputs::default() };
    let identity = identify(&inputs, Kind::Movie, &RuntimeFacts::default(), &Settings::default());
    let text = serde_json::to_string(&identity).unwrap();
    assert_eq!(
        text,
        concat!(
            r#"{"step":"discdb_one","confidence":0.95,"tmdb":{"kind":"movie","id":245891},"title":"John Wick","#,
            r#""year":2014,"original_language":null,"review":null,"plausible":[],"runtime_check":null,"#,
            r#""lookup_pending":false,"evidence":["TheDiscDB knows the fingerprint","TheDiscDB names movie 245891"]}"#
        )
    );
    assert_eq!(serde_json::from_str::<Identity>(&text).unwrap(), identity);
}
