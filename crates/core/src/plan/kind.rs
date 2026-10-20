//! Spec 8.2, the disc kind: the candidate filter (menu loops and titles shorter than `minlength_s` out), the groups
//! of titles with identical segment maps (on a DVD keyed by the title number too, since its segments restart in
//! every title), the episode-length groups and the structural TV test, TheDiscDB's kind and the disc name's season
//! marker, and the "play all" title (a group whose segments are two or more other groups' in order) in its two readings:
//! 8.3 rule 3.1's (every part episode-length, ruling OR1) and 8.5's (any parts, ruling OR15).
//! `Kind` itself lives in `lookup` (identification is kind-aware) and is re-exported here. Used by `plan::movie`
//! (Task 5), `plan::tv` and `remembered` (Task 6) and the planner (Task 7).

use crate::disc::{Disc, StreamKind, Title};
use crate::lookup::discdb::DiscDbRecord;
use crate::lookup::name::DiscName;
use crate::settings::Settings;

pub use crate::lookup::Kind;

/// 8.2's candidate filter: menu loops out (`is_menu_loop`), titles shorter than settings.minlength_s out, titles without a
/// duration out; the rest in title order. Reads `settings.minlength_s` only. Def: T5. Use: T5, T7, phase 2 (6.3 step 6a's
/// fallback list, 11's Assign titles).
pub fn candidate_filter(disc: &Disc, settings: &Settings) -> Vec<u32> {
    disc.titles
        .values()
        .filter(|title| !is_menu_loop(title))
        .filter(|title| title.seconds.map_or(false, |seconds| seconds >= settings.minlength_s))
        .map(|title| title.number)
        .collect()
}

/// A title with no audio stream, with more than one chapter every two minutes (`chapters as f64 > seconds as f64 / 120.0`, 0 s
/// counts as a loop when it has chapters, and so does a title without a duration), or whose segment map repeats one clip
/// (every segment equal and two or more of them). Spec 8.2. Def: T5. Use: T5.
pub fn is_menu_loop(title: &Title) -> bool {
    if title.streams_of(StreamKind::Audio).is_empty() {
        return true;
    }
    let seconds = f64::from(title.seconds.unwrap_or(0));
    if f64::from(title.chapters) > seconds / 120.0 {
        return true;
    }
    title.segments.len() >= 2 && title.segments.iter().all(|&segment| segment == title.segments[0])
}

/// Titles with identical segment maps (8.2). On a DVD the key is (attribute 24, segment map) because its segments restart at
/// 1 in every title (Review Focus 1); attribute 24 is unique per DVD title, so on a DVD every title is its own group. Def: T5. Use: T5, T6, T7.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Group {
    /// The group's key as text ("280,284,299" or, on a DVD, "01:1-29"); "#<title number>" for a title with no segment map,
    /// and for a DVD title without attribute 24.
    pub key: String,
    /// The representative's attribute 26 as the scan wrote it.
    pub segment_map: String,
    /// `audio::titles::segments` of it.
    pub segments: Vec<i64>,
    /// The members in title order; the first is the representative.
    pub titles: Vec<u32>,
    /// "a group's duration is that of its titles": the longest member's seconds.
    pub seconds: u32,
    /// The segment map ends in "..." (MakeMKV truncated it).
    pub truncated: bool,
}

/// The groups of `candidates`, in order of first title. A candidate the disc lacks is skipped; a title without a segment
/// map is a group of its own, and so is a DVD title without attribute 24. Spec 8.2. Def: T5. Use: T5, T6, T7.
pub fn groups(disc: &Disc, candidates: &[u32]) -> Vec<Group> {
    let mut found: Vec<Group> = Vec::new();
    for &number in candidates {
        let Some(title) = disc.titles.get(&number) else {
            continue;
        };
        let key = group_key(title, disc.is_dvd);
        let seconds = title.seconds.unwrap_or(0);
        match found.iter_mut().find(|group| group.key == key) {
            Some(group) => {
                group.titles.push(number);
                group.seconds = group.seconds.max(seconds);
            }
            None => found.push(Group {
                key,
                segment_map: title.segment_map.clone(),
                segments: title.segments.clone(),
                titles: vec![number],
                seconds,
                truncated: title.truncated,
            }),
        }
    }
    found
}

/// A title's group key: its segment map; on a DVD prefixed by its DVD title number (attribute 24) and ':'; "#<number>"
/// when it has no segment map, or is a DVD title without attribute 24, so such titles never merge.
fn group_key(title: &Title, dvd: bool) -> String {
    if title.segment_map.is_empty() {
        return format!("#{}", title.number);
    }
    if dvd {
        match title.dvd_title.as_deref() {
            Some(number) => format!("{number}:{}", title.segment_map),
            None => format!("#{}", title.number),
        }
    } else {
        title.segment_map.clone()
    }
}

/// Spec 8.2: the shortest episode length, 15 minutes, included.
pub const EPISODE_MIN_S: u32 = 15 * 60;
/// Spec 8.2: the longest episode length, 120 minutes, included.
pub const EPISODE_MAX_S: u32 = 120 * 60;
/// Spec 8.2: the longest episode-length group is at most 15% longer than the shortest, as a whole percentage so the test
/// is exact in integers (ruling G2).
pub const TV_SPREAD_PERCENT: u64 = 15;

/// True when the group lasts between 15 and 120 minutes inclusive. Spec 8.2; the part test of 8.3 rule 3.1's play-all title
/// under ruling OR1. Def: this plan. Use: `episode_length_groups`, `play_all_of_episodes`.
pub fn is_episode_length(group: &Group) -> bool {
    (EPISODE_MIN_S..=EPISODE_MAX_S).contains(&group.seconds)
}

/// The groups lasting between 15 and 120 minutes inclusive, in group order. Spec 8.2. Def: T5. Use: T5, T6, T7 (fills
/// RuntimeFacts.episode_group_seconds).
pub fn episode_length_groups(groups: &[Group]) -> Vec<&Group> {
    groups.iter().filter(|group| is_episode_length(group)).collect()
}

/// Three or more episode-length groups and the longest at most 15% longer than the shortest, compared in integers
/// (`longest * 100 <= shortest * 115`, so a pair exactly 15% apart such as 1300 s and 1495 s counts; ruling G2). Spec
/// 8.2. Def: T5. Use: T5.
pub fn structure_says_tv(groups: &[Group]) -> bool {
    let episodes = episode_length_groups(groups);
    if episodes.len() < 3 {
        return false;
    }
    let shortest = episodes.iter().map(|group| group.seconds).min().unwrap_or(0);
    let longest = episodes.iter().map(|group| group.seconds).max().unwrap_or(0);
    u64::from(longest) * 100 <= u64::from(shortest) * (100 + TV_SPREAD_PERCENT)
}

/// Spec 8.5's play-all title, ruling OR15: the index of a group whose segment list is the concatenation of two or more other
/// groups' lists, whose segments follow one another in the whole list (the parts may be any other groups of any length in
/// any group order, each used once; truncated groups never count); None when none is. Not applied on a DVD (segments
/// restart per title). A truncated group is never the play-all title either. The first such group in group order is
/// returned. 8.3 rule 3.1 reads it more narrowly, see `play_all_of_episodes`. Def: T5. Use: T6 (`tv::without_play_all`).
pub fn play_all(groups: &[Group], dvd: bool) -> Option<usize> {
    play_all_where(groups, dvd, &|_| true)
}

/// Spec 8.3 rule 3.1 under ruling OR1: a play-all title whose every part is episode-length (15 to 120 minutes, 8.2); a
/// concatenation of shorter extras is not one. Same search as `play_all` with the parts held to `is_episode_length`.
/// Def: this plan. Use: `movie::by_structure`.
pub fn play_all_of_episodes(groups: &[Group], dvd: bool) -> Option<usize> {
    play_all_where(groups, dvd, &is_episode_length)
}

/// The search behind both readings: the first group that is the concatenation of two or more other groups each passing `part`.
fn play_all_where(groups: &[Group], dvd: bool, part: &dyn Fn(&Group) -> bool) -> Option<usize> {
    if dvd {
        return None;
    }
    (0..groups.len()).find(|&index| {
        let whole = &groups[index];
        if whole.truncated || whole.segments.is_empty() {
            return false;
        }
        let mut used = vec![false; groups.len()];
        used[index] = true;
        concatenates(&whole.segments, groups, part, &mut used, 0)
    })
}

/// True when `rest` is the segments of two or more (counting `parts` already taken) unused, untruncated, non-empty groups
/// passing `part`, one after another.
fn concatenates(rest: &[i64], groups: &[Group], part: &dyn Fn(&Group) -> bool, used: &mut [bool], parts: usize) -> bool {
    if rest.is_empty() {
        return parts >= 2;
    }
    for (index, group) in groups.iter().enumerate() {
        if used[index]
            || group.truncated
            || group.segments.is_empty()
            || !part(group)
            || !rest.starts_with(&group.segments)
        {
            continue;
        }
        used[index] = true;
        if concatenates(&rest[group.segments.len()..], groups, part, used, parts + 1) {
            return true;
        }
        used[index] = false;
    }
    false
}

/// How the kind was decided (8.2's three sources plus the movie default and 8.3 rule 3.1's play-all). Def: T5. Use: T7.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KindSource {
    Discdb,
    Structure,
    SeasonMarker,
    PlayAll,
    Default,
}

impl KindSource {
    /// Every source, in the order 8.2 and 8.3 rule 3.1 name them.
    pub const ALL: [KindSource; 5] =
        [KindSource::Discdb, KindSource::Structure, KindSource::SeasonMarker, KindSource::PlayAll, KindSource::Default];

    /// The serde spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            KindSource::Discdb => "discdb",
            KindSource::Structure => "structure",
            KindSource::SeasonMarker => "season_marker",
            KindSource::PlayAll => "play_all",
            KindSource::Default => "default",
        }
    }
}

impl std::str::FromStr for KindSource {
    type Err = String;

    /// Only the `as_str` spellings.
    fn from_str(text: &str) -> Result<KindSource, String> {
        KindSource::ALL
            .into_iter()
            .find(|source| source.as_str() == text)
            .ok_or_else(|| format!("'{text}' is not a kind source"))
    }
}

/// 8.2's decision: TheDiscDB's kind when it knows the disc; else Tv when structure_says_tv or the name has a season marker;
/// else Movie. (Rule 3.1's play-all turns a Movie into Tv later, in plan::plan.) Def: T5. Use: T7.
pub fn disc_kind(discdb: Option<&DiscDbRecord>, groups: &[Group], name: Option<&DiscName>) -> (Kind, KindSource) {
    if let Some(kind) = discdb.and_then(|record| record.kind()) {
        return (kind, KindSource::Discdb);
    }
    if structure_says_tv(groups) {
        return (Kind::Tv, KindSource::Structure);
    }
    if name.map_or(false, |name| name.season_marker) {
        return (Kind::Tv, KindSource::SeasonMarker);
    }
    (Kind::Movie, KindSource::Default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::titles::segments;
    use crate::lookup::name::disc_name;
    use crate::plan::fixtures::*;
    use crate::testsupport::scan_text;

    /// The candidates under the default settings (`minlength_s` 120).
    fn candidates(disc: &Disc) -> Vec<u32> {
        candidate_filter(disc, &Settings::default())
    }

    #[test]
    fn a_title_with_no_audio_stream_is_a_menu_loop() {
        let disc = disc_of(&scan_text(
            BLURAY,
            &[
                (0, &[(9, "1:40:00"), (8, "12"), (16, "00001.mpls"), (26, "1")], &[(0, VIDEO)]),
                (1, &[(9, "1:40:00"), (8, "12"), (16, "00002.mpls"), (26, "2")], &[(0, VIDEO), (1, ENG_AC3)]),
            ],
        ));
        assert!(is_menu_loop(&disc.titles[&0]));
        assert!(!is_menu_loop(&disc.titles[&1]));
    }

    #[test]
    fn more_than_one_chapter_every_two_minutes_is_a_menu_loop() {
        // Four minutes: two chapters are one every two minutes, not more; three are more; none is no loop.
        let disc = disc_of(&scan_text(
            BLURAY,
            &[
                (0, &[(9, "0:04:00"), (8, "2"), (16, "00001.mpls"), (26, "1")], &[(0, VIDEO), (1, ENG_AC3)]),
                (1, &[(9, "0:04:00"), (8, "3"), (16, "00002.mpls"), (26, "2")], &[(0, VIDEO), (1, ENG_AC3)]),
                (2, &[(9, "0:04:00"), (16, "00003.mpls"), (26, "3")], &[(0, VIDEO), (1, ENG_AC3)]),
            ],
        ));
        assert!(!is_menu_loop(&disc.titles[&0]));
        assert!(is_menu_loop(&disc.titles[&1]));
        assert!(!is_menu_loop(&disc.titles[&2]));
    }

    #[test]
    fn a_title_of_zero_seconds_or_without_a_duration_is_a_menu_loop_when_it_has_chapters() {
        let disc = disc_of(&scan_text(
            BLURAY,
            &[
                (0, &[(9, "0:00:00"), (8, "1"), (16, "00001.mpls"), (26, "1")], &[(0, VIDEO), (1, ENG_AC3)]),
                (1, &[(9, "0:00:00"), (16, "00002.mpls"), (26, "2")], &[(0, VIDEO), (1, ENG_AC3)]),
                (2, &[(8, "1"), (16, "00003.mpls"), (26, "3")], &[(0, VIDEO), (1, ENG_AC3)]),
            ],
        ));
        assert!(is_menu_loop(&disc.titles[&0]));
        assert!(!is_menu_loop(&disc.titles[&1]));
        assert!(is_menu_loop(&disc.titles[&2]));
    }

    #[test]
    fn a_segment_map_that_repeats_one_clip_is_a_menu_loop() {
        let disc = disc_of(&scan_text(
            BLURAY,
            &[
                (0, &[(9, "1:00:00"), (8, "10"), (16, "00001.mpls"), (26, "7,7")], &[(0, VIDEO), (1, ENG_AC3)]),
                (1, &[(9, "1:00:00"), (8, "10"), (16, "00002.mpls"), (26, "7,7,7")], &[(0, VIDEO), (1, ENG_AC3)]),
                (2, &[(9, "1:00:00"), (8, "10"), (16, "00003.mpls"), (26, "7")], &[(0, VIDEO), (1, ENG_AC3)]),
                (3, &[(9, "1:00:00"), (8, "10"), (16, "00004.mpls"), (26, "7,8")], &[(0, VIDEO), (1, ENG_AC3)]),
                (4, &[(9, "1:00:00"), (8, "10"), (16, "00005.mpls"), (26, "7-8")], &[(0, VIDEO), (1, ENG_AC3)]),
            ],
        ));
        assert!(is_menu_loop(&disc.titles[&0]));
        assert!(is_menu_loop(&disc.titles[&1]));
        assert!(!is_menu_loop(&disc.titles[&2]));
        assert!(!is_menu_loop(&disc.titles[&3]));
        assert!(!is_menu_loop(&disc.titles[&4]));
    }

    #[test]
    fn candidates_are_at_least_minlength_s_long_and_have_a_duration() {
        let disc = disc_of(&scan_text(
            BLURAY,
            &[
                (0, &[(9, "0:01:59"), (16, "00001.mpls"), (26, "1")], &[(0, VIDEO), (1, ENG_AC3)]),
                (1, &[(9, "0:02:00"), (16, "00002.mpls"), (26, "2")], &[(0, VIDEO), (1, ENG_AC3)]),
                (2, &[(16, "00003.mpls"), (26, "3")], &[(0, VIDEO), (1, ENG_AC3)]),
            ],
        ));
        assert_eq!(candidates(&disc), vec![1]);
        assert_eq!(candidate_filter(&disc, &Settings { minlength_s: 60, ..Settings::default() }), vec![0, 1]);
        assert_eq!(candidate_filter(&disc, &Settings { minlength_s: 121, ..Settings::default() }), Vec::<u32>::new());
    }

    #[test]
    fn john_wick_uhd_candidates_leave_out_the_menu_loop_and_the_short_trailer() {
        assert_eq!(candidates(&pinned(JOHN_WICK_UHD)), vec![0, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn a_longer_minlength_s_leaves_out_john_wick_uhds_shorter_extras() {
        // Titles 2 to 5 last 360, 386, 318 and 379 seconds; 6, 7 and 8 last 718, 917 and 6073.
        let settings = Settings { minlength_s: 400, ..Settings::default() };
        assert_eq!(candidate_filter(&pinned(JOHN_WICK_UHD), &settings), vec![0, 6, 7, 8]);
    }

    #[test]
    fn john_wick_blu_ray_candidates() {
        assert_eq!(candidates(&pinned(JOHN_WICK_BLU_RAY)), vec![0, 2, 3, 4, 5, 6, 7]);
    }

    #[test]
    fn knowing_candidates_leave_out_the_credits_the_previews_and_the_logos() {
        assert_eq!(candidates(&pinned(KNOWING)), vec![0, 3, 4]);
    }

    #[test]
    fn smile_candidates_are_its_feature_and_its_extras() {
        assert_eq!(candidates(&pinned(SMILE)), vec![10, 11, 12, 13, 14, 117, 119, 122]);
    }

    #[test]
    fn expendables_candidates() {
        assert_eq!(
            candidates(&pinned(EXPENDABLES)),
            vec![17, 18, 19, 20, 21, 23, 25, 26, 27, 29, 30, 32, 33, 36, 38, 41, 43, 44, 45, 46, 47, 48, 51, 54, 55, 56, 57, 58, 59]
        );
    }

    #[test]
    fn chernobyl_disc_1_candidates() {
        assert_eq!(candidates(&pinned(CHERNOBYL_1)), vec![1, 6, 7, 8, 11, 12, 13, 27, 28, 29]);
    }

    #[test]
    fn chernobyl_disc_2_candidates() {
        assert_eq!(candidates(&pinned(CHERNOBYL_2)), vec![6, 7, 8, 9, 12, 13, 14, 15]);
    }

    #[test]
    fn firefly_disc_1_candidates() {
        assert_eq!(candidates(&pinned(FIREFLY_1)), vec![0, 1, 2]);
    }

    #[test]
    fn firefly_disc_2_candidates() {
        assert_eq!(candidates(&pinned(FIREFLY_2)), vec![0, 1, 2, 5]);
    }

    /// A group as `groups` builds it: key `key`, the representative's segment map `map`, not truncated.
    fn group(key: &str, map: &str, titles: &[u32], seconds: u32) -> Group {
        Group {
            key: key.to_string(),
            segment_map: map.to_string(),
            segments: segments(map),
            titles: titles.to_vec(),
            seconds,
            truncated: false,
        }
    }

    /// A Blu-ray group, keyed by its own segment map.
    fn bd(map: &str, titles: &[u32], seconds: u32) -> Group {
        group(map, map, titles, seconds)
    }

    /// The groups of the default candidates.
    fn groups_of(disc: &Disc) -> Vec<Group> {
        groups(disc, &candidates(disc))
    }

    #[test]
    fn john_wick_uhd_groups_its_playlist_with_its_clip() {
        assert_eq!(
            groups_of(&pinned(JOHN_WICK_UHD)),
            vec![
                bd("1", &[0, 8], 6073),
                bd("7", &[2], 360),
                bd("6", &[3], 386),
                bd("5", &[4], 318),
                bd("4", &[5], 379),
                bd("3", &[6], 718),
                bd("2", &[7], 917),
            ]
        );
    }

    #[test]
    fn chernobyl_disc_1_groups_each_episode_playlist_with_its_clip() {
        assert_eq!(
            groups_of(&pinned(CHERNOBYL_1)),
            vec![
                bd("20", &[1, 13], 3534),
                bd("4,11,12", &[6], 440),
                bd("22", &[7, 11], 3707),
                bd("21", &[8, 12], 3896),
                bd("12", &[27], 121),
                bd("11", &[28], 167),
                bd("4", &[29], 151),
            ]
        );
    }

    #[test]
    fn on_a_dvd_titles_with_the_same_segment_map_are_separate_groups() {
        let knowing = pinned(KNOWING);
        assert!(knowing.is_dvd);
        assert_eq!(knowing.titles[&3].segment_map, knowing.titles[&4].segment_map);
        assert_eq!(
            groups_of(&knowing),
            vec![group("01:1-19", "1-19", &[0], 7274), group("04:1", "1", &[3], 754), group("05:1", "1", &[4], 1034)]
        );
    }

    #[test]
    fn firefly_disc_2_keeps_two_titles_of_one_segment_map_apart() {
        assert_eq!(
            groups_of(&pinned(FIREFLY_2)),
            vec![
                group("01:1-5,6-9,10-14,15", "1-5,6-9,10-14,15", &[0], 2631),
                group("02:1-5,6-10,11-14,15", "1-5,6-10,11-14,15", &[1], 2559),
                group("03:1-5,6-9,10-14,15", "1-5,6-9,10-14,15", &[2], 2629),
                group("11:1-4,5-9,10-14,15", "1-4,5-9,10-14,15", &[5], 2628),
            ]
        );
    }

    #[test]
    fn a_group_lasts_as_long_as_its_longest_title() {
        let disc = disc_of(&scan_text(
            BLURAY,
            &[
                (0, &[(9, "1:40:00"), (8, "12"), (16, "00001.mpls"), (26, "5")], &[(0, VIDEO), (1, ENG_AC3)]),
                (1, &[(9, "1:40:03"), (16, "00005.m2ts"), (26, "5")], &[(0, VIDEO), (1, ENG_AC3)]),
            ],
        ));
        assert_eq!(groups_of(&disc), vec![bd("5", &[0, 1], 6003)]);
    }

    #[test]
    fn a_truncated_segment_map_marks_its_group() {
        let disc = disc_of(&scan_text(
            BLURAY,
            &[(0, &[(9, "1:40:00"), (8, "12"), (16, "00001.mpls"), (26, "1,2,3,...")], &[(0, VIDEO), (1, ENG_AC3)])],
        ));
        assert_eq!(
            groups_of(&disc),
            vec![Group {
                key: "1,2,3,...".to_string(),
                segment_map: "1,2,3,...".to_string(),
                segments: vec![1, 2, 3],
                titles: vec![0],
                seconds: 6000,
                truncated: true,
            }]
        );
    }

    #[test]
    fn titles_without_a_segment_map_are_never_grouped_together() {
        let disc = disc_of(&scan_text(
            BLURAY,
            &[
                (0, &[(9, "1:40:00"), (8, "12"), (16, "00001.mpls")], &[(0, VIDEO), (1, ENG_AC3)]),
                (1, &[(9, "1:40:00"), (8, "12"), (16, "00002.mpls")], &[(0, VIDEO), (1, ENG_AC3)]),
            ],
        ));
        assert_eq!(groups_of(&disc), vec![group("#0", "", &[0], 6000), group("#1", "", &[1], 6000)]);
    }

    #[test]
    fn a_dvd_title_without_attribute_24_is_a_group_of_its_own() {
        // Both titles carry segment map "1" (a DVD's segments restart in every title) and neither has a DVD title number.
        let disc = disc_of(&scan_text(
            DVD,
            &[
                (0, &[(9, "0:10:00"), (26, "1")], &[(0, DVD_VIDEO), (1, ENG_AC3)]),
                (1, &[(9, "0:10:00"), (26, "1")], &[(0, DVD_VIDEO), (1, ENG_AC3)]),
            ],
        ));
        assert!(disc.is_dvd);
        assert_eq!(groups_of(&disc), vec![group("#0", "1", &[0], 600), group("#1", "1", &[1], 600)]);
    }

    #[test]
    fn expendables_has_one_group_per_candidate() {
        let disc = pinned(EXPENDABLES);
        let found = groups_of(&disc);
        assert_eq!(found.len(), 29);
        assert!(found.iter().all(|group| group.titles.len() == 1));
        assert_eq!(found.iter().map(|group| group.titles[0]).collect::<Vec<_>>(), candidates(&disc));
    }

    /// The keys of some groups, in order.
    fn keys(groups: &[&Group]) -> Vec<String> {
        groups.iter().map(|group| group.key.clone()).collect()
    }

    #[test]
    fn episode_length_is_15_to_120_minutes_inclusive() {
        let found = vec![bd("1", &[0], 899), bd("2", &[1], 900), bd("3", &[2], 7200), bd("4", &[3], 7201)];
        assert_eq!(keys(&episode_length_groups(&found)), vec!["2", "3"]);
    }

    #[test]
    fn chernobyl_disc_1_has_three_episode_length_groups_and_is_tv_by_structure() {
        let found = groups_of(&pinned(CHERNOBYL_1));
        assert_eq!(keys(&episode_length_groups(&found)), vec!["20", "22", "21"]);
        assert!(structure_says_tv(&found));
    }

    #[test]
    fn firefly_disc_2_is_tv_by_structure() {
        let found = groups_of(&pinned(FIREFLY_2));
        assert_eq!(episode_length_groups(&found).len(), 4);
        assert!(structure_says_tv(&found));
    }

    #[test]
    fn two_episode_length_groups_are_not_enough() {
        let found = groups_of(&pinned(CHERNOBYL_2));
        assert_eq!(keys(&episode_length_groups(&found)), vec!["2", "4"]);
        assert!(!structure_says_tv(&found));
    }

    #[test]
    fn the_longest_episode_group_may_be_at_most_15_percent_longer_than_the_shortest() {
        assert!(structure_says_tv(&[bd("1", &[0], 2000), bd("2", &[1], 2100), bd("3", &[2], 2300)]));
        assert!(!structure_says_tv(&[bd("1", &[0], 2000), bd("2", &[1], 2100), bd("3", &[2], 2301)]));
    }

    #[test]
    fn the_15_percent_spread_is_exact_at_its_boundary() {
        // G2: 1495 is exactly 115% of 1300, which `1300.0 * 1.15` (1494.9999999999998) missed in floating point.
        assert!(structure_says_tv(&[bd("1", &[0], 1300), bd("2", &[1], 1300), bd("3", &[2], 1495)]));
        assert!(!structure_says_tv(&[bd("1", &[0], 1300), bd("2", &[1], 1300), bd("3", &[2], 1496)]));
    }

    #[test]
    fn firefly_disc_1_is_not_tv_by_structure_since_its_pilot_is_double_length() {
        let found = groups_of(&pinned(FIREFLY_1));
        assert_eq!(episode_length_groups(&found).len(), 3);
        assert!(!structure_says_tv(&found));
    }

    #[test]
    fn the_movie_logs_are_not_tv_by_structure() {
        for log in [JOHN_WICK_UHD, JOHN_WICK_BLU_RAY, KNOWING, SMILE, EXPENDABLES] {
            assert!(!structure_says_tv(&groups_of(&pinned(log))), "{log}");
        }
    }

    #[test]
    fn a_group_made_of_other_groups_in_order_is_play_all() {
        let found = vec![bd("1", &[0], 600), bd("2", &[1], 600), bd("1,2", &[2], 1200)];
        assert_eq!(play_all(&found, false), Some(2));
        let reversed = vec![bd("1", &[0], 600), bd("2", &[1], 600), bd("2,1", &[2], 1200)];
        assert_eq!(play_all(&reversed, false), Some(2));
        let unknown_part = vec![bd("1", &[0], 600), bd("2", &[1], 600), bd("1,3", &[2], 1200)];
        assert_eq!(play_all(&unknown_part, false), None);
        let a_missing_part = vec![bd("1", &[0], 600), bd("2", &[1], 600), bd("2,3,1", &[2], 1200)];
        assert_eq!(play_all(&a_missing_part, false), None);
        // The parts' segments must follow one another: "1,3,2,4" holds the segments of "1,2" and "3,4", interleaved.
        let interleaved = vec![bd("1,2", &[0], 600), bd("3,4", &[1], 600), bd("1,3,2,4", &[2], 1200)];
        assert_eq!(play_all(&interleaved, false), None);
    }

    #[test]
    fn play_all_needs_two_or_more_other_groups() {
        // "1-2" and "1,2" are two keys with the same segments: one other group is not a concatenation.
        assert_eq!(play_all(&[bd("1-2", &[0], 600), bd("1,2", &[1], 600)], false), None);
        assert_eq!(play_all(&[bd("1,2", &[0], 600), bd("3", &[1], 300), bd("1,2,3", &[2], 900)], false), Some(2));
    }

    #[test]
    fn play_all_is_not_looked_for_on_a_dvd() {
        let disc = disc_of(&scan_text(
            DVD,
            &[
                (0, &[(9, "0:10:00"), (8, "2"), (24, "01"), (26, "1")], &[(0, DVD_VIDEO), (1, ENG_AC3)]),
                (1, &[(9, "0:10:00"), (8, "2"), (24, "02"), (26, "2")], &[(0, DVD_VIDEO), (1, ENG_AC3)]),
                (2, &[(9, "0:20:00"), (8, "4"), (24, "03"), (26, "1,2")], &[(0, DVD_VIDEO), (1, ENG_AC3)]),
            ],
        ));
        assert!(disc.is_dvd);
        let found = groups_of(&disc);
        assert_eq!(found.iter().map(|group| group.key.as_str()).collect::<Vec<_>>(), vec!["01:1", "02:2", "03:1,2"]);
        // The same segments on a Blu-ray would make title 2 a play-all title; a DVD's segments restart in every title.
        assert_eq!(play_all(&found, false), Some(2));
        assert_eq!(play_all(&found, disc.is_dvd), None);
        let knowing = pinned(KNOWING);
        assert_eq!(play_all(&groups_of(&knowing), knowing.is_dvd), None);
    }

    #[test]
    fn a_truncated_group_is_never_play_all_nor_part_of_one() {
        let truncated_whole = vec![
            bd("1", &[0], 600),
            bd("2", &[1], 600),
            Group { truncated: true, ..group("1,2,...", "1,2,...", &[2], 1200) },
        ];
        assert_eq!(play_all(&truncated_whole, false), None);
        let truncated_part =
            vec![Group { truncated: true, ..bd("1", &[0], 600) }, bd("2", &[1], 600), bd("1,2", &[2], 1200)];
        assert_eq!(play_all(&truncated_part, false), None);
    }

    #[test]
    fn a_play_all_of_episodes_counts_only_parts_of_episode_length() {
        let two_parts = |a: u32, b: u32| vec![bd("1", &[1], a), bd("2", &[2], b), bd("1,2", &[3], a + b)];
        assert_eq!(play_all_of_episodes(&two_parts(1320, 1320), false), Some(2));
        assert_eq!(play_all_of_episodes(&two_parts(900, 7200), false), Some(2), "the band is inclusive");
        assert_eq!(play_all_of_episodes(&two_parts(1320, 600), false), None, "a ten-minute part");
        assert_eq!(play_all_of_episodes(&two_parts(899, 1320), false), None);
        assert_eq!(play_all_of_episodes(&two_parts(7201, 1320), false), None);
        assert_eq!(play_all_of_episodes(&two_parts(1320, 1320), true), None, "never on a DVD");
        // 8.5's reading (OR15) is unchanged: any parts.
        assert_eq!(play_all(&two_parts(1320, 600), false), Some(2));
    }

    #[test]
    fn a_whole_with_only_an_extras_decomposition_is_a_play_all_for_8_5_but_not_for_rule_3_1() {
        let groups = vec![bd("1", &[1], 1320), bd("5", &[2], 300), bd("1,5", &[3], 1620)];
        assert_eq!(play_all(&groups, false), Some(2));
        assert_eq!(play_all_of_episodes(&groups, false), None);
    }

    #[test]
    fn is_episode_length_is_15_to_120_minutes_inclusive() {
        for (seconds, expected) in [(899, false), (900, true), (7200, true), (7201, false)] {
            assert_eq!(is_episode_length(&bd("1", &[1], seconds)), expected, "{seconds}");
        }
    }

    #[test]
    fn the_pinned_logs_play_all_groups() {
        for (log, key, title) in [(CHERNOBYL_1, "4,11,12", 6), (CHERNOBYL_2, "11,12", 7), (SMILE, "596,597", 117)] {
            let disc = pinned(log);
            let found = groups_of(&disc);
            let index = play_all(&found, disc.is_dvd).unwrap_or_else(|| panic!("{log}: no play-all group"));
            assert_eq!((found[index].key.as_str(), found[index].titles.clone()), (key, vec![title]), "{log}");
            assert_eq!(play_all_of_episodes(&found, false), None, "{log}: its parts are short extras");
        }
        assert_eq!(play_all(&groups_of(&pinned(CHERNOBYL_1)), false), Some(1));
        assert_eq!(play_all(&groups_of(&pinned(SMILE)), false), Some(5));
        for log in [JOHN_WICK_UHD, JOHN_WICK_BLU_RAY, EXPENDABLES, KNOWING, FIREFLY_1, FIREFLY_2] {
            let disc = pinned(log);
            assert_eq!(play_all(&groups_of(&disc), disc.is_dvd), None, "{log}");
        }
    }

    #[test]
    fn thediscdb_states_the_kind_when_it_knows_the_disc() {
        let chernobyl_2 = groups_of(&pinned(CHERNOBYL_2));
        let record = api_record(CHERNOBYL_2_API);
        assert_eq!(disc_kind(Some(&record), &chernobyl_2, None), (Kind::Tv, KindSource::Discdb));
        // TheDiscDB's kind wins over the structure: a movie record with a TV-shaped disc is a movie.
        let movie = api_record(JOHN_WICK_UHD_API);
        let chernobyl_1 = groups_of(&pinned(CHERNOBYL_1));
        assert_eq!(disc_kind(Some(&movie), &chernobyl_1, None), (Kind::Movie, KindSource::Discdb));
        let firefly = api_record(FIREFLY_1_API);
        assert_eq!(disc_kind(Some(&firefly), &groups_of(&pinned(FIREFLY_1)), None), (Kind::Tv, KindSource::Discdb));
    }

    #[test]
    fn without_thediscdb_three_episode_length_groups_make_a_tv_disc() {
        let chernobyl_1 = groups_of(&pinned(CHERNOBYL_1));
        assert_eq!(disc_kind(None, &chernobyl_1, None), (Kind::Tv, KindSource::Structure));
        // The pinned form has no media, so it states no kind and the structure decides.
        let pinned_form = pinned_record(CHERNOBYL_1_PINNED_JSON);
        assert_eq!(pinned_form.kind(), None);
        assert_eq!(disc_kind(Some(&pinned_form), &chernobyl_1, None), (Kind::Tv, KindSource::Structure));
        assert_eq!(disc_kind(None, &groups_of(&pinned(FIREFLY_2)), None), (Kind::Tv, KindSource::Structure));
    }

    #[test]
    fn a_season_marker_in_the_disc_name_makes_a_tv_disc() {
        let office = disc_name(None, Some("EXAMPLE SHOW S3D3")).expect("a name");
        assert!(office.season_marker);
        let john_wick = groups_of(&pinned(JOHN_WICK_UHD));
        assert_eq!(disc_kind(None, &john_wick, Some(&office)), (Kind::Tv, KindSource::SeasonMarker));
        // The structure is looked at first.
        let chernobyl_1 = groups_of(&pinned(CHERNOBYL_1));
        assert_eq!(disc_kind(None, &chernobyl_1, Some(&office)), (Kind::Tv, KindSource::Structure));
    }

    #[test]
    fn a_3d_label_is_not_a_season_marker_for_the_kind() {
        let hugo = disc_name(None, Some("HUGO_S3D")).expect("a name");
        assert!(!hugo.season_marker);
        let john_wick = groups_of(&pinned(JOHN_WICK_UHD));
        assert_eq!(disc_kind(None, &john_wick, Some(&hugo)), (Kind::Movie, KindSource::Default));
    }

    #[test]
    fn a_disc_number_alone_never_makes_a_tv_disc() {
        let name = disc_name(None, Some("CHERNOBYL_D2")).expect("a name");
        assert!(!name.season_marker);
        let chernobyl_2 = groups_of(&pinned(CHERNOBYL_2));
        assert_eq!(disc_kind(None, &chernobyl_2, Some(&name)), (Kind::Movie, KindSource::Default));
        assert_eq!(disc_kind(None, &chernobyl_2, None), (Kind::Movie, KindSource::Default));
        let firefly = disc_name(None, Some("FIREFLY_D1")).expect("a name");
        assert_eq!(disc_kind(None, &groups_of(&pinned(FIREFLY_1)), Some(&firefly)), (Kind::Movie, KindSource::Default));
    }

    #[test]
    fn kind_sources_spell_themselves_as_the_spec_does() {
        assert_eq!(KindSource::ALL.map(KindSource::as_str), ["discdb", "structure", "season_marker", "play_all", "default"]);
        for source in KindSource::ALL {
            assert_eq!(serde_json::to_string(&source).unwrap(), format!("\"{}\"", source.as_str()));
            assert_eq!(source.as_str().parse::<KindSource>(), Ok(source));
        }
        assert!("Discdb".parse::<KindSource>().is_err());
    }

    #[test]
    fn the_episode_length_constants_are_the_spec_values() {
        assert_eq!((EPISODE_MIN_S, EPISODE_MAX_S, TV_SPREAD_PERCENT), (900, 7200, 15));
    }
}
