//! Spec 8.5 (TV) and spec 5's season sessions (`<tmdb show id>-s<NN>.json`), RP17: the episode titles TheDiscDB maps
//! (`by_discdb`: title confidence 0.98, play-all titles dropped, multi-episode titles kept as TheDiscDB describes them),
//! the suggestions by runtime when it does not (`unmapped`: 0.60 with `tv_unmapped`, so whole-disc copy mode is used),
//! and the season session record with its pure functions `find`, `suggest`, `assign` and `close` (persistence is
//! phase 2). Task 7's planner calls `by_discdb`, `unmapped`, `find`, `SeasonSession::next_unassigned` and `assign`;
//! `suggest` and `close` serve phase 2's review (Assign titles, the owner's close); Tasks 11 and 13 read `Episode`
//! through the plan. No setting is read here.

use std::collections::{BTreeMap, BTreeSet};

use crate::lookup::discdb::{DiscDbItem, DiscDbRecord, ItemType};
use crate::lookup::tmdb::{TmdbEpisode, TmdbSeason};
use crate::lookup::{within, TV_RUNTIME_TOLERANCE};
use crate::plan::kind::{episode_length_groups, play_all, Group};
use crate::review::{sorted_unique, ReviewReason};

/// An episode assignment: season and one or more episode numbers (multi-episode titles, 8.5). `season` None means not
/// known (no season parsed and no session found): review asks for it (8.5); never 0, which is the specials season (8.9
/// `Season 00`). Def: T6. Use: T7, T11, T13.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Episode {
    /// The season number (0 is the specials season); None when not known.
    pub season: Option<u32>,
    /// The episode numbers in TMDb's aired order (8.5): one, or two or more for a multi-episode title.
    pub episodes: Vec<u32>,
    /// TheDiscDB's item title (trimmed), or TMDb's episode names for a suggestion; None when neither gives one.
    pub name: Option<String>,
}

/// The widest range `parse_episode_numbers` reads as one title's episodes. TheDiscDB's answers are network data: a
/// wider range is a malformed answer, and reading it would allocate without bound.
const MAX_EPISODE_SPAN: u32 = 100;

/// A whole number of ASCII digits, surrounding whitespace ignored ("01" is 1); None for anything else or a number
/// above u32::MAX.
fn whole_number(text: &str) -> Option<u32> {
    let text = text.trim();
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// "12-13" gives [12, 13], "4" gives [4], "01" gives [1]; empty for anything else. A range `a-b` with `a <= b` gives
/// every number from a to b (TheDiscDB's multi-episode form), at most MAX_EPISODE_SPAN of them. Spec 8.5. Def: T6. Use: T6.
pub fn parse_episode_numbers(text: &str) -> Vec<u32> {
    match text.split_once('-') {
        None => whole_number(text).map(|number| vec![number]).unwrap_or_default(),
        Some((first, last)) => match (whole_number(first), whole_number(last)) {
            (Some(first), Some(last)) if first <= last && last - first < MAX_EPISODE_SPAN => (first..=last).collect(),
            _ => Vec::new(),
        },
    }
}

/// The episode as 8.9 codes it ("S03E12-E13"; "E05 (season unknown)" without a season), with its name in double
/// quotes when it has one: the form of every evidence sentence here.
fn describe(episode: &Episode) -> String {
    let numbers: Vec<String> = episode.episodes.iter().map(|number| format!("E{number:02}")).collect();
    let code = match episode.season {
        Some(season) => format!("S{season:02}{}", numbers.join("-")),
        None => format!("{} (season unknown)", numbers.join("-")),
    };
    match &episode.name {
        Some(name) => format!("{code} \"{name}\""),
        None => code,
    }
}

/// One title assigned or suggested. Spec 8.5. Def: T6. Use: T7.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EpisodeTitle {
    /// The scan's title number.
    pub title: u32,
    pub episode: Episode,
    /// True for 8.5's runtime suggestions (review confirms them); false for TheDiscDB's map.
    pub suggested: bool,
}

/// Spec 8.5: the title confidence when TheDiscDB maps the disc. Def: T6. Use: T7.
pub const MAPPED_TV_CONFIDENCE: f64 = 0.98;
/// Spec 8.5: the title confidence otherwise, with tv_unmapped. Def: T6. Use: T7.
pub const UNMAPPED_TV_CONFIDENCE: f64 = 0.60;

/// The TV titles of 8.5 with their confidence and review reasons. Def: T6. Use: T7.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TvPlan {
    /// True when TheDiscDB's Episode items gave the titles (`by_discdb`); false for suggestions (`unmapped`).
    pub mapped: bool,
    /// `by_discdb`: in database index order; `unmapped`: in title order. plan::plan orders the planned titles.
    pub episodes: Vec<EpisodeTitle>,
    /// MAPPED_TV_CONFIDENCE or UNMAPPED_TV_CONFIDENCE.
    pub confidence: f64,
    /// Empty, or [TvUnmapped] (8.5, 8.8).
    pub reviews: Vec<ReviewReason>,
    /// Play-all titles dropped (8.5 "play all titles are dropped").
    pub dropped_play_all: Vec<u32>,
    /// The plan's sentences: by_discdb one per mapped title in database index order, then one per dropped play-all title;
    /// unmapped the confidence and suggestion-method sentences, "season unknown" when it is, then one per suggested title
    /// in title order.
    pub evidence: Vec<String>,
}

/// 8.5 first bullet: every Episode item that `matches` reconciles becomes an EpisodeTitle (season from `item.season` parsed as
/// a number, None when absent, which plan::plan fills from the disc name's season; episodes from
/// `parse_episode_numbers(item.episode)`; name from `item.title`); every play-all group's titles are dropped (`kind::play_all`
/// applied again until it finds none); 0.98. None when the record has no reconciled Episode item, when any Episode item
/// matches no scan title (as Open point 4 for MainMovies), or when a reconciled Episode item on a title that is not
/// play-all names no episode number (D1); plan::plan then plans the disc as unmapped. A Direct job holds only its planned
/// titles (8.8 "Limits of D1"), so mapping the rest would file the disc without that episode. An Episode item whose
/// episode text gives no number makes `by_discdb` return None (D1), as an unmatched one does. A title of a group
/// `kind::play_all` finds is dropped even when TheDiscDB labels it an Episode item, unless that item names an episode
/// no other kept Episode item maps: then the title is the episode (a multi-clip episode playlist whose clips MakeMKV also
/// lists, and TheDiscDB describes with no item, is play-all shaped), it is kept and left out of `dropped_play_all`. The
/// play-all titles' items are taken fewest episodes first, each kept one's episodes then counting as mapped; one that
/// names no number is never kept. `play_all` is never applied on a DVD. Def: T6. Use: T7.
pub fn by_discdb(record: &DiscDbRecord, matches: &BTreeMap<u32, u32>, groups: &[Group], dvd: bool) -> Option<TvPlan> {
    // D1 ("When unsure, ... queue the disc for review"): an episode TheDiscDB describes but the scan does not show leaves
    // the disc unmapped, as 8.3 rule 1 needs every MainMovie to reconcile (Open point 4).
    if record.episodes().iter().any(|db| !matches.contains_key(&db.index)) {
        return None;
    }
    // 8.5 "play all titles are dropped": every play-all group (8.3 rule 3.1), found again until none is, as plan::plan does.
    let (_, mut dropped_play_all) = without_play_all(groups, dvd);
    let item_of = |index: u32| {
        record
            .titles
            .iter()
            .find(|db| db.index == index)
            .and_then(|db| db.item.as_ref())
            .filter(|item| item.item_type == ItemType::Episode)
    };
    let kept_play_all = kept_play_all_episodes(matches, &dropped_play_all, item_of);
    dropped_play_all.retain(|title| !kept_play_all.contains(title));
    let mut episodes = Vec::new();
    let mut evidence = Vec::new();
    for (&index, &title) in matches {
        let Some(item) = item_of(index) else {
            continue;
        };
        if dropped_play_all.contains(&title) {
            continue;
        }
        let numbers = parse_episode_numbers(item.episode.as_deref().unwrap_or(""));
        if numbers.is_empty() {
            // D1: an episode title TheDiscDB cannot number would be left off a Direct job; plan the disc as unmapped.
            return None;
        }
        let name = item.title.trim();
        let episode = Episode {
            season: item.season.as_deref().and_then(whole_number),
            episodes: numbers,
            name: if name.is_empty() { None } else { Some(name.to_string()) },
        };
        evidence.push(format!("TheDiscDB maps title {title} to {}", describe(&episode)));
        episodes.push(EpisodeTitle { title, episode, suggested: false });
    }
    if episodes.is_empty() {
        return None;
    }
    for title in &dropped_play_all {
        evidence.push(format!(
            "play-all title {title} dropped (8.5): its segment map is the concatenation of other groups' maps"
        ));
    }
    for title in &kept_play_all {
        evidence.push(format!(
            "title {title} is play-all shaped but kept: its Episode item names an episode no other title maps"
        ));
    }
    Some(TvPlan { mapped: true, episodes, confidence: MAPPED_TV_CONFIDENCE, reviews: Vec::new(), dropped_play_all, evidence })
}

/// The (season, episode) pairs an Episode item names: its season parsed as a number (None when absent) with each of
/// `parse_episode_numbers(item.episode)`.
fn named_episodes(item: &DiscDbItem) -> Vec<(Option<u32>, u32)> {
    let season = item.season.as_deref().and_then(whole_number);
    parse_episode_numbers(item.episode.as_deref().unwrap_or("")).into_iter().map(|number| (season, number)).collect()
}

/// The play-all titles (`dropped_play_all`) whose Episode item `by_discdb` keeps, sorted: the items of the other matched
/// titles map their episodes first; then the play-all titles' items, fewest episodes first (then by database index),
/// each kept when it names an episode not yet mapped, its episodes then counting as mapped. Spec 8.5, D1.
fn kept_play_all_episodes<'a>(
    matches: &BTreeMap<u32, u32>,
    dropped_play_all: &[u32],
    item_of: impl Fn(u32) -> Option<&'a DiscDbItem>,
) -> Vec<u32> {
    let mut mapped: BTreeSet<(Option<u32>, u32)> = matches
        .iter()
        .filter(|(_, title)| !dropped_play_all.contains(title))
        .filter_map(|(&index, _)| item_of(index))
        .flat_map(named_episodes)
        .collect();
    let mut candidates: Vec<(usize, u32, u32, Vec<(Option<u32>, u32)>)> = matches
        .iter()
        .filter(|(_, title)| dropped_play_all.contains(title))
        .filter_map(|(&index, &title)| item_of(index).map(|item| (index, title, named_episodes(item))))
        .map(|(index, title, named)| (named.len(), index, title, named))
        .collect();
    candidates.sort_by_key(|(count, index, _, _)| (*count, *index));
    let mut kept = Vec::new();
    for (_, _, title, named) in candidates {
        if named.iter().any(|pair| !mapped.contains(pair)) {
            mapped.extend(named);
            kept.push(title);
        }
    }
    kept.sort_unstable();
    kept.dedup();
    kept
}

/// 8.5 "play all titles are dropped": every play-all group (`kind::play_all`, parts of any length under OR15, found again
/// until none is; never on a DVD) is removed. Returns the remaining groups and the dropped titles, sorted. Def: T6. Use: T6, T7.
pub fn without_play_all(groups: &[Group], dvd: bool) -> (Vec<Group>, Vec<u32>) {
    let mut rest: Vec<Group> = groups.to_vec();
    let mut dropped: Vec<u32> = Vec::new();
    while let Some(index) = play_all(&rest, dvd) {
        dropped.extend(rest.remove(index).titles);
    }
    dropped.sort_unstable();
    (rest, dropped)
}

/// Spec 8.5 "pre-assigned to episodes by runtime as a suggestion": the groups' representatives in title order, assigned
/// from episode `start`: each takes the next episode, or the next two when its duration is within TV_RUNTIME_TOLERANCE of
/// the sum of the next two episodes' runtimes and not within it of the next one's; an episode without a runtime takes one
/// title. Returns (title, episode numbers) pairs. An episode is looked up in `episodes` by its `episode_number`; one TMDb
/// does not list counts as an episode without a runtime. The episodes `session` already assigned are skipped, as
/// `suggest` skips them (8.5 "Suggestions start at the session's next unassigned episode"): a title takes the next
/// free episode, and two only when the episode after it is free too (a two-episode title holds two episodes in a
/// row). Def: T6. Use: T6.
pub fn suggest_by_runtime(
    groups: &[&Group],
    episodes: &[TmdbEpisode],
    start: u32,
    session: Option<&SeasonSession>,
) -> Vec<(u32, Vec<u32>)> {
    let runtime_s = |number: u32| {
        episodes
            .iter()
            .find(|episode| episode.episode_number == number)
            .and_then(|episode| episode.runtime)
            .map(|minutes| f64::from(minutes) * 60.0)
    };
    let mut ordered: Vec<&Group> = groups.iter().copied().filter(|group| !group.titles.is_empty()).collect();
    ordered.sort_by_key(|group| group.titles[0]);
    // The first free episode at or after a number (u32::MAX when none is: the numbers saturate).
    let free = |episode: u32| free_from(session, episode).unwrap_or(u32::MAX);
    let mut next = free(start);
    let mut out = Vec::with_capacity(ordered.len());
    for group in ordered {
        let seconds = f64::from(group.seconds);
        let after = next.saturating_add(1);
        let two = free(after) == after
            && match (runtime_s(next), runtime_s(after)) {
                (Some(first), Some(second)) => {
                    within(seconds, first + second, TV_RUNTIME_TOLERANCE) && !within(seconds, first, TV_RUNTIME_TOLERANCE)
                }
                _ => false,
            };
        if two {
            out.push((group.titles[0], vec![next, after]));
            next = free(after.saturating_add(1));
        } else {
            out.push((group.titles[0], vec![next]));
            next = free(after);
        }
    }
    out
}

/// The first episode at or after `episode` that `session` has not assigned (`episode` itself without a session); None
/// when every one up to u32::MAX is assigned. The one copy of 8.5's skip, which `suggest` and `suggest_by_runtime` share.
fn free_from(session: Option<&SeasonSession>, episode: u32) -> Option<u32> {
    (episode..=u32::MAX).find(|&candidate| !session.is_some_and(|session| session.get(candidate).is_some()))
}

/// 8.5 second bullet: 0.60 with tv_unmapped; the episode-length groups' representatives are suggested from `start` (the
/// session's next unassigned episode, or 1), through `suggest_by_runtime` when `episodes` (the season's TMDb episodes) are
/// given, else one episode each in title order; `suggested: true`; in `season` (None when no season was parsed and no
/// session was found). Episode numbers run on from `start`, skipping the episodes `session` (the season session found
/// for the disc, or None) already assigned, as `suggest` does (8.5 "Suggestions start at the session's next unassigned
/// episode"), so a suggestion never names an episode another disc holds; review confirms every suggestion (0.60,
/// tv_unmapped). A suggestion's name is TMDb's episode name, a
/// two-episode suggestion's names joined with " / ".
/// The caller passes the groups to suggest: a play-all group (8.3 rule 3.1) is left out by plan::plan, since this
/// function has no DVD flag to find one. Def: T6. Use: T7.
pub fn unmapped(
    groups: &[Group],
    season: Option<u32>,
    start: u32,
    episodes: Option<&TmdbSeason>,
    session: Option<&SeasonSession>,
) -> TvPlan {
    let tmdb: &[TmdbEpisode] = match episodes {
        Some(listed) => &listed.episodes,
        None => &[],
    };
    let pairs = suggest_by_runtime(&episode_length_groups(groups), tmdb, start, session);
    let how = if tmdb.is_empty() {
        "one episode each in title order (no TMDb episodes for the season)"
    } else {
        "by TMDb's episode runtimes"
    };
    let mut evidence = vec![
        "no TheDiscDB episode map: title confidence 0.60 with tv_unmapped (8.5)".to_string(),
        format!("episode-length titles suggested from episode {start}: {}, {how}", pairs.len()),
    ];
    if season.is_none() {
        evidence.push("season unknown: review asks for it (8.5)".to_string());
    }
    let mut titles = Vec::with_capacity(pairs.len());
    for (title, numbers) in pairs {
        let names: Vec<&str> = numbers
            .iter()
            .filter_map(|number| tmdb.iter().find(|episode| episode.episode_number == *number))
            .filter_map(|episode| episode.name.as_deref())
            .filter(|name| !name.trim().is_empty())
            .collect();
        let name = if names.is_empty() { None } else { Some(names.join(" / ")) };
        let episode = Episode { season, episodes: numbers, name };
        evidence.push(format!("title {title} suggested as {}", describe(&episode)));
        titles.push(EpisodeTitle { title, episode, suggested: true });
    }
    TvPlan {
        mapped: false,
        episodes: titles,
        confidence: UNMAPPED_TV_CONFIDENCE,
        reviews: sorted_unique(vec![ReviewReason::TvUnmapped]),
        dropped_play_all: Vec::new(),
        evidence,
    }
}

/// One assigned episode of a season session: the episode number and the disc fingerprint and scan title it was assigned
/// from. Spec 8.5. Def: T6. Use: T6, T7.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AssignedEpisode {
    /// The episode number (the key: one entry per episode).
    pub episode: u32,
    /// The fingerprint of the disc it was assigned from.
    pub fingerprint: String,
    /// The scan title number on that disc.
    pub title: u32,
}

/// A season session (8.5, spec 5's `<tmdb show id>-s<NN>.json`); persistence is phase 2 (RP17). Def: T6. Use: T7, T15.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SeasonSession {
    /// TMDb's show ID (the TV half of a TmdbIdentity).
    pub show_id: i64,
    /// The season number (0 is specials).
    pub season: u32,
    /// Fingerprints of the discs seen, in order.
    pub discs: Vec<String>,
    /// The assigned episodes, one entry per episode, sorted by episode number (`assign` keeps the order). A list of
    /// entries, not a map keyed by the episode number: the session is part of the plan document, which serde reads back
    /// inside buffered content that cannot give an integer map key (the interfaces' "Keyed collections").
    pub assigned: Vec<AssignedEpisode>,
    /// TMDb's episode count for the season; None when TMDb gave none (the session then closes only by `close`).
    pub episode_count: Option<u32>,
    /// Closed when every episode is assigned or the owner closed it (8.5).
    pub closed: bool,
}

impl SeasonSession {
    /// An open session with no disc and no episode assigned. Spec 8.5. Def: T6. Use: T6, T7.
    pub fn new(show_id: i64, season: u32, episode_count: Option<u32>) -> SeasonSession {
        SeasonSession { show_id, season, discs: Vec::new(), assigned: Vec::new(), episode_count, closed: false }
    }

    /// The assignment of `episode`, if any. Spec 8.5. Def: T6. Use: T6.
    pub fn get(&self, episode: u32) -> Option<&AssignedEpisode> {
        self.assigned.iter().find(|entry| entry.episode == episode)
    }

    /// The lowest episode number from 1 that is not assigned. Spec 8.5. Def: T6. Use: T6, T7.
    pub fn next_unassigned(&self) -> u32 {
        (1..=u32::MAX).find(|&episode| self.get(episode).is_none()).unwrap_or(u32::MAX)
    }

    /// `<show_id>-s<NN>.json` (spec 5). Def: T6. Use: phase 2.
    pub fn file_name(&self) -> String {
        format!("{}-s{:02}.json", self.show_id, self.season)
    }
}

/// What `find` found. Spec 8.5. Def: T6. Use: T7.
#[derive(Debug, PartialEq)]
pub enum Found<'a> {
    /// The show's session for the parsed season (open or closed).
    Session(&'a SeasonSession),
    /// Without a parsed season: the show's only open session.
    SingleOpen(&'a SeasonSession),
    /// No session for the show (and parsed season): suggestions start at the first episode; without a parsed season
    /// review asks for it (8.5).
    None,
    /// Without a parsed season, the show has several open sessions: review asks for the season.
    Ambiguous,
}

/// By show ID and the parsed season; without a parsed season the show's single open session; Ambiguous when several are
/// open. With a parsed season the first session of that show and season is returned whether open or closed. Spec 8.5.
/// Def: T6. Use: T7.
pub fn find<'a>(sessions: &'a [SeasonSession], show_id: i64, season: Option<u32>) -> Found<'a> {
    match season {
        Some(season) => sessions
            .iter()
            .find(|session| session.show_id == show_id && session.season == season)
            .map_or(Found::None, Found::Session),
        None => {
            let open: Vec<&SeasonSession> =
                sessions.iter().filter(|session| session.show_id == show_id && !session.closed).collect();
            match open.as_slice() {
                [] => Found::None,
                [only] => Found::SingleOpen(*only),
                _ => Found::Ambiguous,
            }
        }
    }
}

/// `n` episode numbers from the session's next unassigned episode (from 1 without a session), skipping assigned ones.
/// Spec 8.5 "Suggestions start at the session's next unassigned episode". Def: T6. Use: phase 2 (review's Assign titles).
pub fn suggest(session: Option<&SeasonSession>, n: usize) -> Vec<u32> {
    let mut out = Vec::with_capacity(n);
    let mut from = Some(session.map_or(1, SeasonSession::next_unassigned));
    while out.len() < n {
        let Some(episode) = from.and_then(|from| free_from(session, from)) else {
            break;
        };
        out.push(episode);
        from = episode.checked_add(1);
    }
    out
}

/// Records the disc and its episodes; `episodes` are (episode number, scan title number) pairs; closes the session when
/// every episode of `episode_count` is assigned. The fingerprint is recorded once; an episode assigned again takes the
/// new disc and title (the owner's latest assignment); a closed session stays closed. Spec 8.5. Def: T6. Use: T7, phase 2.
pub fn assign(session: &mut SeasonSession, fingerprint: &str, episodes: &[(u32, u32)]) {
    if !session.discs.iter().any(|seen| seen == fingerprint) {
        session.discs.push(fingerprint.to_string());
    }
    for &(episode, title) in episodes {
        let entry = AssignedEpisode { episode, fingerprint: fingerprint.to_string(), title };
        // Kept sorted by episode number; an episode assigned again is replaced in place.
        match session.assigned.iter().position(|seen| seen.episode >= episode) {
            Some(index) if session.assigned[index].episode == episode => session.assigned[index] = entry,
            Some(index) => session.assigned.insert(index, entry),
            None => session.assigned.push(entry),
        }
    }
    if let Some(count) = session.episode_count {
        if (1..=count).all(|episode| session.get(episode).is_some()) {
            session.closed = true;
        }
    }
}

/// The owner's close (8.5 "or when the owner closes it"). Def: T6. Use: phase 2.
pub fn close(session: &mut SeasonSession) {
    session.closed = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::{json, Value};

    use crate::disc::{disc, Disc};
    use crate::lookup::discdb::{match_titles, parse_dischash};
    use crate::lookup::tmdb::parse_tv;
    use crate::plan::fixtures::{
        api_record, pinned, CHERNOBYL_1, CHERNOBYL_1_API, CHERNOBYL_2, CHERNOBYL_2_API, FIREFLY_1, FIREFLY_1_API, FIREFLY_2,
        FIREFLY_2_API,
    };
    use crate::plan::kind::{candidate_filter, groups};
    use crate::robot::parse;
    use crate::settings::Settings;
    use crate::testsupport::{scan_text, tmdb_tv_json};

    /// The groups the planner builds: 8.2's candidate filter at the default settings, then grouping.
    fn planner_groups(scanned: &Disc) -> Vec<Group> {
        groups(scanned, &candidate_filter(scanned, &Settings::default()))
    }

    /// `by_discdb` over a pinned log and its API answer, as the planner calls it.
    fn mapped(log: &str, hash: &str) -> TvPlan {
        let scanned = pinned(log);
        let record = api_record(hash);
        by_discdb(&record, &match_titles(&record, &scanned), &planner_groups(&scanned), scanned.is_dvd).unwrap()
    }

    fn episode_title(title: u32, season: Option<u32>, episodes: &[u32], name: Option<&str>, suggested: bool) -> EpisodeTitle {
        EpisodeTitle {
            title,
            episode: Episode { season, episodes: episodes.to_vec(), name: name.map(str::to_string) },
            suggested,
        }
    }

    /// A one-title group with this segment list and duration.
    fn group(title: u32, segments: &[i64], seconds: u32) -> Group {
        let map = segments.iter().map(|segment| segment.to_string()).collect::<Vec<_>>().join(",");
        Group { key: map.clone(), segment_map: map, segments: segments.to_vec(), titles: vec![title], seconds, truncated: false }
    }

    /// A TheDiscDB record holding only these titles (the API's snake_case keys).
    fn record(titles: Value) -> DiscDbRecord {
        serde_json::from_value(json!({ "titles": titles })).unwrap()
    }

    fn play_all_sentence(title: u32) -> String {
        format!("play-all title {title} dropped (8.5): its segment map is the concatenation of other groups' maps")
    }

    /// English stereo Dolby Digital: one audio stream, so the candidate filter keeps the title.
    const ENGLISH_STEREO: &[(u32, &str)] = &[(1, "Audio"), (3, "eng"), (5, "A_AC3"), (6, "DD"), (7, "Dolby Digital"), (14, "2")];

    /// A made-up season 2 disc 3 shaped like the-office-2005-s3d3-bluray (RP21: no value is copied from the clone):
    /// number, playlist (attribute 16), segment map (26), duration (9), size (11) and chapters (8).
    const SHOW_TITLES: [(u32, &str, &str, &str, u64, &str); 11] = [
        (0, "00021.mpls", "31", "0:22:10", 4_100_000_000, "5"),
        (1, "00022.mpls", "32", "0:22:40", 4_200_000_000, "5"),
        (2, "00023.mpls", "33", "0:21:55", 4_300_000_000, "5"),
        (3, "00024.mpls", "34", "0:43:20", 8_400_000_000, "5"),
        (4, "00027.mpls", "37,38", "0:25:10", 4_500_000_000, "2"),
        (5, "00026.mpls", "36", "0:28:05", 6_600_000_000, "5"),
        (6, "00025.mpls", "35", "0:22:25", 4_700_000_000, "5"),
        (7, "00031.m2ts", "41", "0:04:10", 800_000_000, "0"),
        (8, "00032.m2ts", "42", "0:05:40", 1_000_000_000, "0"),
        (9, "00033.m2ts", "43", "0:10:15", 1_900_000_000, "0"),
        (10, "00034.m2ts", "44", "0:07:20", 1_400_000_000, "0"),
    ];

    /// TheDiscDB's answer for that made-up disc in the API's shape: fourteen titles in database order (index 4 and the
    /// last two are titles the scan does not list), with the sizes of SHOW_TITLES.
    const SHOW_RECORD: &str = r#"{
  "format": "Blu-Ray",
  "media": {"title": "Example Show", "year": 2001, "type": "Series", "external_ids": {"tmdb": "9001", "imdb": "tt0009001"}},
  "disc": {"index": 8, "name": "Season Two Disc Three", "slug": "season-2-disc-3"},
  "titles": [
    {"index": 0, "source_file": "00021.mpls", "segment_map": "31", "duration": "0:22:10", "size": 4100000000,
     "item": {"title": "Pilot Light", "type": "Episode", "season": "2", "episode": "6"}},
    {"index": 1, "source_file": "00022.mpls", "segment_map": "32", "duration": "0:22:40", "size": 4200000000,
     "item": {"title": "Second Wind", "type": "Episode", "season": "2", "episode": "5"}},
    {"index": 2, "source_file": "00023.mpls", "segment_map": "33", "duration": "0:21:55", "size": 4300000000,
     "item": {"title": "Third Rail", "type": "Episode", "season": "2", "episode": "4"}},
    {"index": 3, "source_file": "00024.mpls", "segment_map": "34", "duration": "0:43:20", "size": 8400000000,
     "item": {"title": "Double Shift", "type": "Episode", "season": "2", "episode": "2-3"}},
    {"index": 4, "source_file": "00020.mpls(1)", "segment_map": "30,39", "duration": "0:00:20", "size": 40000000, "item": null},
    {"index": 5, "source_file": "00027.mpls", "segment_map": "37,38", "duration": "0:25:10", "size": 4500000000,
     "item": {"title": "Double Shift Deleted Scenes", "type": "DeletedScene", "season": "2", "episode": "2-3"}},
    {"index": 6, "source_file": "00026.mpls", "segment_map": "36", "duration": "0:28:05", "size": 6600000000,
     "item": {"title": "Night Watch", "type": "Episode", "season": "2", "episode": "8"}},
    {"index": 7, "source_file": "00025.mpls", "segment_map": "35", "duration": "0:22:25", "size": 4700000000,
     "item": {"title": "Open House", "type": "Episode", "season": "2", "episode": "7"}},
    {"index": 8, "source_file": "00031.m2ts", "segment_map": "41", "duration": "0:04:10", "size": 800000000,
     "item": {"title": "Open House Deleted Scenes", "type": "DeletedScene", "season": "2", "episode": "7"}},
    {"index": 9, "source_file": "00032.m2ts", "segment_map": "42", "duration": "0:05:40", "size": 1000000000,
     "item": {"title": "Pilot Light Deleted Scenes", "type": "DeletedScene", "season": "2", "episode": "6"}},
    {"index": 10, "source_file": "00033.m2ts", "segment_map": "43", "duration": "0:10:15", "size": 1900000000,
     "item": {"title": "Second Wind Deleted Scenes", "type": "DeletedScene", "season": "2", "episode": "5"}},
    {"index": 11, "source_file": "00034.m2ts", "segment_map": "44", "duration": "0:07:20", "size": 1400000000,
     "item": {"title": "Third Rail Deleted Scenes", "type": "DeletedScene", "season": "2", "episode": "4"}},
    {"index": 12, "source_file": "00040.m2ts", "segment_map": "40", "duration": "0:00:50", "size": 250000000, "item": null},
    {"index": 13, "source_file": "00030.m2ts", "segment_map": "29", "duration": "0:00:25", "size": 110000000, "item": null}
  ]
}"#;

    /// The made-up disc's scan: SHOW_TITLES, each with one English stereo audio stream.
    fn show_disc() -> Disc {
        let sizes: Vec<String> = SHOW_TITLES.iter().map(|row| row.4.to_string()).collect();
        let attrs: Vec<Vec<(u32, &str)>> = SHOW_TITLES
            .iter()
            .zip(&sizes)
            .map(|(row, size)| vec![(8, row.5), (9, row.3), (11, size.as_str()), (16, row.1), (26, row.2)])
            .collect();
        let streams: &[(u32, &[(u32, &str)])] = &[(0, ENGLISH_STEREO)];
        let titles: Vec<(u32, &[(u32, &str)], &[(u32, &[(u32, &str)])])> =
            SHOW_TITLES.iter().zip(&attrs).map(|(row, attrs)| (row.0, attrs.as_slice(), streams)).collect();
        let text = scan_text(&[(1, "Blu-ray disc"), (2, "EXAMPLE SHOW S2D3"), (32, "EXAMPLE SHOW S2D3")], &titles);
        disc(&parse(&text)).unwrap()
    }

    #[test]
    fn episode_numbers_read_one_number_with_or_without_leading_zeros() {
        assert_eq!(parse_episode_numbers("4"), vec![4]);
        assert_eq!(parse_episode_numbers("01"), vec![1]);
        assert_eq!(parse_episode_numbers(" 16 "), vec![16]);
        assert_eq!(parse_episode_numbers("0"), vec![0]);
    }

    #[test]
    fn episode_numbers_read_a_range_as_every_episode_in_it() {
        assert_eq!(parse_episode_numbers("12-13"), vec![12, 13]);
        assert_eq!(parse_episode_numbers("1-3"), vec![1, 2, 3]);
        assert_eq!(parse_episode_numbers("07 - 08"), vec![7, 8]);
        assert_eq!(parse_episode_numbers("5-5"), vec![5]);
        assert_eq!(parse_episode_numbers("1-100").len(), 100);
    }

    #[test]
    fn episode_numbers_are_empty_for_anything_else() {
        for text in ["", " ", "E4", "4a", "13-12", "12-", "-12", "12-13-14", "1-101", "4294967296", "1.5"] {
            assert_eq!(parse_episode_numbers(text), Vec::<u32>::new(), "{text:?}");
        }
    }

    #[test]
    fn chernobyl_disc_1_maps_three_episodes_and_drops_its_play_all_title() {
        assert_eq!(
            mapped(CHERNOBYL_1, CHERNOBYL_1_API),
            TvPlan {
                mapped: true,
                episodes: vec![
                    episode_title(1, Some(1), &[1], Some("1.23.45"), false),
                    episode_title(7, Some(1), &[3], Some("Open Wide O Earth"), false),
                    episode_title(8, Some(1), &[2], Some("Please Remain Calm"), false),
                ],
                confidence: 0.98,
                reviews: vec![],
                dropped_play_all: vec![6],
                evidence: vec![
                    "TheDiscDB maps title 1 to S01E01 \"1.23.45\"".to_string(),
                    "TheDiscDB maps title 7 to S01E03 \"Open Wide O Earth\"".to_string(),
                    "TheDiscDB maps title 8 to S01E02 \"Please Remain Calm\"".to_string(),
                    play_all_sentence(6),
                ],
            }
        );
        assert_eq!(MAPPED_TV_CONFIDENCE, 0.98);
    }

    #[test]
    fn chernobyl_disc_2_maps_episodes_4_and_5_and_drops_its_play_all_title() {
        assert_eq!(
            mapped(CHERNOBYL_2, CHERNOBYL_2_API),
            TvPlan {
                mapped: true,
                episodes: vec![
                    episode_title(6, Some(1), &[4], Some("The Happiness of All Mankind"), false),
                    episode_title(8, Some(1), &[5], Some("Vichnaya Pamyat"), false),
                ],
                confidence: MAPPED_TV_CONFIDENCE,
                reviews: vec![],
                dropped_play_all: vec![7],
                evidence: vec![
                    "TheDiscDB maps title 6 to S01E04 \"The Happiness of All Mankind\"".to_string(),
                    "TheDiscDB maps title 8 to S01E05 \"Vichnaya Pamyat\"".to_string(),
                    play_all_sentence(7),
                ],
            }
        );
    }

    #[test]
    fn firefly_disc_1_maps_its_episodes_by_dvd_title_number_and_the_long_pilot_stays_one_episode() {
        let scanned = pinned(FIREFLY_1);
        assert!(scanned.is_dvd);
        assert_eq!(scanned.title(0).unwrap().seconds, Some(5186), "Serenity runs 1:26:26, twice an episode");
        assert_eq!(
            mapped(FIREFLY_1, FIREFLY_1_API),
            TvPlan {
                mapped: true,
                episodes: vec![
                    episode_title(0, Some(1), &[1], Some("Serenity"), false),
                    episode_title(1, Some(1), &[2], Some("The Train Job"), false),
                    episode_title(2, Some(1), &[3], Some("Bushwhacked"), false),
                ],
                confidence: MAPPED_TV_CONFIDENCE,
                reviews: vec![],
                dropped_play_all: vec![],
                evidence: vec![
                    "TheDiscDB maps title 0 to S01E01 \"Serenity\"".to_string(),
                    "TheDiscDB maps title 1 to S01E02 \"The Train Job\"".to_string(),
                    "TheDiscDB maps title 2 to S01E03 \"Bushwhacked\"".to_string(),
                ],
            }
        );
    }

    #[test]
    fn firefly_disc_2_maps_title_5_to_episode_7() {
        // TheDiscDB writes attribute 24 in source_file on a DVD: "11" is scan title 5. Titles 0 and 2 share the segment
        // map 1-5,6-9,10-14,15 and last 0:43:51 and 0:43:49, so only attribute 24 tells them apart (Review Focus 1).
        let plan = mapped(FIREFLY_2, FIREFLY_2_API);
        assert_eq!(
            plan.episodes,
            vec![
                episode_title(0, Some(1), &[4], Some("Shindig"), false),
                episode_title(1, Some(1), &[5], Some("Safe"), false),
                episode_title(2, Some(1), &[6], Some("Our Mrs. Reynolds"), false),
                episode_title(5, Some(1), &[7], Some("Jaynestown"), false),
            ]
        );
        assert!(plan.mapped);
        assert_eq!(plan.confidence, MAPPED_TV_CONFIDENCE);
        assert!(plan.reviews.is_empty());
        assert_eq!(plan.dropped_play_all, Vec::<u32>::new());
        assert_eq!(plan.evidence.last().unwrap(), "TheDiscDB maps title 5 to S01E07 \"Jaynestown\"");
    }

    #[test]
    fn a_multi_episode_title_and_deleted_scenes_map_as_thediscdb_describes_them() {
        let scanned = show_disc();
        let record = parse_dischash(SHOW_RECORD, "show.json").unwrap().unwrap();
        let matches = match_titles(&record, &scanned);
        assert_eq!(
            matches,
            BTreeMap::from([(0, 0), (1, 1), (2, 2), (3, 3), (5, 4), (6, 5), (7, 6), (8, 7), (9, 8), (10, 9), (11, 10)])
        );
        let plan = by_discdb(&record, &matches, &planner_groups(&scanned), scanned.is_dvd).unwrap();
        assert_eq!(
            plan.episodes,
            vec![
                episode_title(0, Some(2), &[6], Some("Pilot Light"), false),
                episode_title(1, Some(2), &[5], Some("Second Wind"), false),
                episode_title(2, Some(2), &[4], Some("Third Rail"), false),
                episode_title(3, Some(2), &[2, 3], Some("Double Shift"), false),
                episode_title(5, Some(2), &[8], Some("Night Watch"), false),
                episode_title(6, Some(2), &[7], Some("Open House"), false),
            ]
        );
        assert_eq!(plan.confidence, MAPPED_TV_CONFIDENCE);
        assert!(plan.reviews.is_empty());
        assert_eq!(plan.dropped_play_all, Vec::<u32>::new());
        assert_eq!(plan.evidence[3], "TheDiscDB maps title 3 to S02E02-E03 \"Double Shift\"");
    }

    #[test]
    fn a_play_all_title_is_dropped_even_when_thediscdb_labels_it_an_episode() {
        let record = record(json!([
            {"index": 0, "item": {"title": "First", "type": "Episode", "season": "1", "episode": "1"}},
            {"index": 1, "item": {"title": "Second", "type": "Episode", "season": "1", "episode": "2"}},
            {"index": 2, "item": {"title": "Third", "type": "Episode", "season": "1", "episode": "3"}},
            {"index": 3, "item": {"title": "Play All", "type": "Episode", "season": "1", "episode": "1-3"}}
        ]));
        let matches = BTreeMap::from([(0, 0), (1, 1), (2, 2), (3, 3)]);
        let groups = vec![group(0, &[10], 1320), group(1, &[11], 1320), group(2, &[12], 1320), group(3, &[10, 11, 12], 3960)];
        let plan = by_discdb(&record, &matches, &groups, false).unwrap();
        assert_eq!(
            plan.episodes,
            vec![
                episode_title(0, Some(1), &[1], Some("First"), false),
                episode_title(1, Some(1), &[2], Some("Second"), false),
                episode_title(2, Some(1), &[3], Some("Third"), false),
            ]
        );
        assert_eq!(plan.dropped_play_all, vec![3]);
        assert_eq!(plan.evidence.last().unwrap(), &play_all_sentence(3));
    }

    #[test]
    fn every_play_all_title_is_dropped() {
        // An extras play-all (title 4, segments 10 and 11) comes first in group order; the episodes play-all (title 5,
        // segments 12 and 13), which TheDiscDB labels E03-E04, is found once the first is left out.
        let record = record(json!([
            {"index": 0, "item": {"title": "Third", "type": "Episode", "season": "1", "episode": "3"}},
            {"index": 1, "item": {"title": "Fourth", "type": "Episode", "season": "1", "episode": "4"}},
            {"index": 2, "item": {"title": "Play All", "type": "Episode", "season": "1", "episode": "3-4"}}
        ]));
        let groups = vec![
            group(0, &[12], 1320),
            group(1, &[13], 1320),
            group(4, &[10, 11], 600),
            group(5, &[12, 13], 2640),
            group(6, &[10], 300),
            group(7, &[11], 300),
        ];
        let plan = by_discdb(&record, &BTreeMap::from([(0, 0), (1, 1), (2, 5)]), &groups, false).unwrap();
        assert_eq!(plan.episodes.iter().map(|e| e.title).collect::<Vec<_>>(), vec![0, 1]);
        assert_eq!(plan.dropped_play_all, vec![4, 5]);
    }

    #[test]
    fn on_a_dvd_no_title_is_dropped_as_play_all() {
        let record = record(json!([
            {"index": 0, "source_file": "01", "item": {"title": "First", "type": "Episode", "season": "1", "episode": "1"}},
            {"index": 1, "source_file": "02", "item": {"title": "Second", "type": "Episode", "season": "1", "episode": "2"}},
            {"index": 2, "source_file": "03", "item": {"title": "Third", "type": "Episode", "season": "1", "episode": "3"}}
        ]));
        let matches = BTreeMap::from([(0, 0), (1, 1), (2, 2)]);
        // A DVD's segments restart at 1 in every title, so title 2's "1,2" only looks like titles 0 and 1 played in turn.
        let groups = vec![group(0, &[1], 1320), group(1, &[2], 1320), group(2, &[1, 2], 2640)];
        let dvd = by_discdb(&record, &matches, &groups, true).unwrap();
        assert_eq!(dvd.episodes.len(), 3);
        assert_eq!(dvd.dropped_play_all, Vec::<u32>::new());
        // On a Blu-ray the same groups make title 2 play-all shaped; it is dropped when its Episode item names only
        // episodes the other titles map (here E01-E02), and kept when it names one they do not (E03 above).
        let play_all = record_with_play_all("1-2");
        let blu_ray = by_discdb(&play_all, &matches, &groups, false).unwrap();
        assert_eq!(blu_ray.episodes.iter().map(|e| e.title).collect::<Vec<_>>(), vec![0, 1]);
        assert_eq!(blu_ray.dropped_play_all, vec![2]);
        assert_eq!(by_discdb(&play_all, &matches, &groups, true).unwrap().episodes.len(), 3);
        let kept = by_discdb(&record, &matches, &groups, false).unwrap();
        assert_eq!(kept.episodes.iter().map(|e| e.title).collect::<Vec<_>>(), vec![0, 1, 2]);
        assert_eq!(kept.dropped_play_all, Vec::<u32>::new());
    }

    /// The record of `on_a_dvd_no_title_is_dropped_as_play_all` with title 2 labelled as these episodes of season 1.
    fn record_with_play_all(episodes: &str) -> DiscDbRecord {
        record(json!([
            {"index": 0, "source_file": "01", "item": {"title": "First", "type": "Episode", "season": "1", "episode": "1"}},
            {"index": 1, "source_file": "02", "item": {"title": "Second", "type": "Episode", "season": "1", "episode": "2"}},
            {"index": 2, "source_file": "03", "item": {"title": "Both", "type": "Episode", "season": "1", "episode": episodes}}
        ]))
    }

    #[test]
    fn a_multi_clip_episode_playlist_whose_clips_carry_no_item_is_kept() {
        // Title 0 (00800.mpls, clips 20 and 21) is TheDiscDB's S01E01-E02 and title 1 (00801.mpls, clip 22) its S01E03;
        // titles 2 and 3 are the clips 00020.m2ts and 00021.m2ts, which MakeMKV lists too and TheDiscDB describes with no
        // item. Title 0 is play-all shaped (its segments are groups 2 and 3 in turn), but no other Episode item maps E01
        // or E02, so it is the episode and is kept; the clips carry no item and are not episodes.
        let opening = record(json!([
            {"index": 0, "item": {"title": "Opening", "type": "Episode", "season": "1", "episode": "1-2"}},
            {"index": 1, "item": {"title": "Third", "type": "Episode", "season": "1", "episode": "3"}},
            {"index": 2, "item": null},
            {"index": 3, "item": null}
        ]));
        let matches = BTreeMap::from([(0, 0), (1, 1), (2, 2), (3, 3)]);
        let groups = vec![group(0, &[20, 21], 2640), group(1, &[22], 1320), group(2, &[20], 1300), group(3, &[21], 1340)];
        let plan = by_discdb(&opening, &matches, &groups, false).unwrap();
        assert_eq!(
            plan.episodes,
            vec![
                episode_title(0, Some(1), &[1, 2], Some("Opening"), false),
                episode_title(1, Some(1), &[3], Some("Third"), false),
            ]
        );
        assert_eq!(plan.dropped_play_all, Vec::<u32>::new());
        assert_eq!((plan.confidence, plan.reviews.clone()), (MAPPED_TV_CONFIDENCE, vec![]));
        assert_eq!(
            plan.evidence.last().unwrap(),
            "title 0 is play-all shaped but kept: its Episode item names an episode no other title maps"
        );
        // One episode split across two clips is kept the same way.
        let split = record(json!([
            {"index": 0, "item": {"title": "Long One", "type": "Episode", "season": "1", "episode": "1"}},
            {"index": 1, "item": {"title": "Third", "type": "Episode", "season": "1", "episode": "3"}},
            {"index": 2, "item": null},
            {"index": 3, "item": null}
        ]));
        let plan = by_discdb(&split, &matches, &groups, false).unwrap();
        assert_eq!(plan.episodes.iter().map(|e| e.title).collect::<Vec<_>>(), vec![0, 1]);
        assert_eq!(plan.dropped_play_all, Vec::<u32>::new());
    }

    #[test]
    fn of_two_play_all_titles_the_one_whose_episodes_another_kept_title_maps_is_dropped() {
        // Title 0 (clips 20, 21) is E01-E02 and title 4 (clips 20, 21, 22) E01-E03 beside title 1 (clip 22, E03): title 0
        // is kept (no other title maps E01 or E02), and then title 4's episodes are all mapped, so it is dropped.
        let record = record(json!([
            {"index": 0, "item": {"title": "Opening", "type": "Episode", "season": "1", "episode": "1-2"}},
            {"index": 1, "item": {"title": "Third", "type": "Episode", "season": "1", "episode": "3"}},
            {"index": 2, "item": null},
            {"index": 3, "item": null},
            {"index": 4, "item": {"title": "Play All", "type": "Episode", "season": "1", "episode": "1-3"}}
        ]));
        let matches = BTreeMap::from([(0, 0), (1, 1), (2, 2), (3, 3), (4, 4)]);
        let groups = vec![
            group(0, &[20, 21], 2640),
            group(1, &[22], 1320),
            group(2, &[20], 1300),
            group(3, &[21], 1340),
            group(4, &[20, 21, 22], 3960),
        ];
        let plan = by_discdb(&record, &matches, &groups, false).unwrap();
        assert_eq!(plan.episodes.iter().map(|e| e.title).collect::<Vec<_>>(), vec![0, 1]);
        assert_eq!(plan.dropped_play_all, vec![4]);
    }

    #[test]
    fn an_episode_item_without_a_season_leaves_the_season_unknown() {
        let record = record(json!([{"index": 0, "item": {"title": " Fifth ", "type": "Episode", "episode": "5"}}]));
        let plan = by_discdb(&record, &BTreeMap::from([(0, 4)]), &[], false).unwrap();
        assert_eq!(plan.episodes, vec![episode_title(4, None, &[5], Some("Fifth"), false)]);
        assert_eq!(plan.evidence, vec!["TheDiscDB maps title 4 to E05 (season unknown) \"Fifth\"".to_string()]);
        let not_a_number = record_with_season("one");
        let plan = by_discdb(&not_a_number, &BTreeMap::from([(0, 4)]), &[], false).unwrap();
        assert_eq!(plan.episodes[0].episode.season, None);
        let specials = record_with_season("00");
        let plan = by_discdb(&specials, &BTreeMap::from([(0, 4)]), &[], false).unwrap();
        assert_eq!(plan.episodes[0].episode.season, Some(0));
    }

    fn record_with_season(season: &str) -> DiscDbRecord {
        record(json!([{"index": 0, "item": {"title": "Fifth", "type": "Episode", "season": season, "episode": "5"}}]))
    }

    #[test]
    fn an_episode_item_without_an_episode_number_leaves_the_disc_unmapped() {
        // Title 0 is an episode TheDiscDB cannot number: filing Pilot at 0.98 in Direct mode would leave it off the job
        // (8.8 "Limits of D1"), so the disc is left to the unmapped plan (D1), as with an unmatched Episode item.
        let record = record(json!([
            {"index": 0, "item": {"title": "Making Of", "type": "Episode", "season": "1"}},
            {"index": 1, "item": {"title": "Pilot", "type": "Episode", "season": "1", "episode": "1"}}
        ]));
        assert_eq!(by_discdb(&record, &BTreeMap::from([(0, 0), (1, 1)]), &[], false), None);
        let numbered = record_with_season("1");
        assert!(by_discdb(&numbered, &BTreeMap::from([(0, 0)]), &[], false).is_some());
    }

    #[test]
    fn a_record_without_a_reconciled_episode_item_maps_nothing() {
        let unmatched = record(json!([
            {"index": 0, "item": {"title": "Pilot", "type": "Episode", "season": "1", "episode": "1"}},
            {"index": 1, "item": {"title": "The Film", "type": "MainMovie"}},
            {"index": 2, "item": {"title": "Gag Reel", "type": "Extra"}},
            {"index": 3}
        ]));
        // The episode item matches no scan title.
        assert_eq!(by_discdb(&unmatched, &BTreeMap::new(), &[], false), None);
        // No Episode item at all: a main movie, an extra and an undescribed title, all reconciled.
        let no_episode = record(json!([
            {"index": 1, "item": {"title": "The Film", "type": "MainMovie"}},
            {"index": 2, "item": {"title": "Gag Reel", "type": "Extra"}},
            {"index": 3}
        ]));
        assert_eq!(by_discdb(&no_episode, &BTreeMap::from([(1, 0), (2, 1), (3, 2)]), &[], false), None);
        // Only a play-all title is labelled an episode, and its item names no episode number: it is dropped, and nothing
        // is mapped.
        let only_play_all =
            record(json!([{"index": 0, "item": {"title": "Play All", "type": "Episode", "season": "1", "episode": ""}}]));
        let groups = vec![group(0, &[10, 11], 2640), group(1, &[10], 1320), group(2, &[11], 1320)];
        assert_eq!(by_discdb(&only_play_all, &BTreeMap::from([(0, 0)]), &groups, false), None);
        // Naming E01-E02, which no other title maps, the same play-all-shaped title is the episode and is kept.
        let numbered =
            record(json!([{"index": 0, "item": {"title": "Play All", "type": "Episode", "season": "1", "episode": "1-2"}}]));
        let kept = by_discdb(&numbered, &BTreeMap::from([(0, 0)]), &groups, false).unwrap();
        assert_eq!(kept.episodes, vec![episode_title(0, Some(1), &[1, 2], Some("Play All"), false)]);
        assert_eq!(kept.dropped_play_all, Vec::<u32>::new());
    }

    #[test]
    fn an_episode_item_that_matches_no_scan_title_leaves_the_disc_unmapped() {
        // TheDiscDB describes three episodes and the scan shows two of them. Mapping the two at 0.98 would file the disc in
        // Direct mode without the third (8.8 "Limits of D1"), so the disc is left to the unmapped plan (D1), as 8.3 rule 1
        // needs every MainMovie to reconcile (Open point 4).
        let record = record(json!([
            {"index": 0, "item": {"title": "First", "type": "Episode", "season": "1", "episode": "1"}},
            {"index": 1, "item": {"title": "Second", "type": "Episode", "season": "1", "episode": "2"}},
            {"index": 2, "item": {"title": "Third", "type": "Episode", "season": "1", "episode": "3"}}
        ]));
        assert_eq!(by_discdb(&record, &BTreeMap::from([(0, 0), (1, 1)]), &[], false), None);
        let every = by_discdb(&record, &BTreeMap::from([(0, 0), (1, 1), (2, 2)]), &[], false).unwrap();
        assert_eq!(every.episodes.iter().map(|e| e.title).collect::<Vec<_>>(), vec![0, 1, 2]);
    }

    fn tmdb_episode(episode_number: u32, runtime: Option<u32>) -> TmdbEpisode {
        TmdbEpisode { episode_number, name: None, runtime }
    }

    #[test]
    fn a_double_length_title_takes_two_episodes() {
        let groups = [group(1, &[1], 1320), group(2, &[2], 2640), group(3, &[3], 1300)];
        let refs: Vec<&Group> = groups.iter().collect();
        let episodes = [1, 2, 3, 4].map(|number| tmdb_episode(number, Some(22)));
        assert_eq!(suggest_by_runtime(&refs, &episodes, 1, None), vec![(1, vec![1]), (2, vec![2, 3]), (3, vec![4])]);
    }

    #[test]
    fn a_long_title_whose_own_episode_runtime_matches_takes_one_episode() {
        // A pilot running 1:26:26 (Firefly's Serenity) against an episode runtime of 86 minutes, then two 44-minute
        // episodes: the pilot is one episode, not the 130 minutes of episodes 1 and 2.
        let groups = [group(0, &[1], 5186), group(1, &[2], 2554), group(2, &[3], 2627)];
        let refs: Vec<&Group> = groups.iter().collect();
        let episodes = [tmdb_episode(1, Some(86)), tmdb_episode(2, Some(44)), tmdb_episode(3, Some(44))];
        assert_eq!(suggest_by_runtime(&refs, &episodes, 1, None), vec![(0, vec![1]), (1, vec![2]), (2, vec![3])]);
    }

    #[test]
    fn two_episodes_are_taken_only_within_15_percent_of_their_sum() {
        // Twenty-minute episodes: two together last 2400 s, and 15% of that is 360 s.
        let groups = [group(0, &[1], 2700), group(1, &[2], 2800)];
        let refs: Vec<&Group> = groups.iter().collect();
        let episodes = [1, 2, 3, 4].map(|number| tmdb_episode(number, Some(20)));
        assert_eq!(suggest_by_runtime(&refs, &episodes, 1, None), vec![(0, vec![1, 2]), (1, vec![3])]);
    }

    #[test]
    fn a_title_within_15_percent_of_one_episode_takes_one_even_when_two_also_fit() {
        // A 40-minute episode followed by a 2-minute one: 2450 s is within 15% of both 2400 s and 2520 s.
        let groups = [group(0, &[1], 2450)];
        let refs: Vec<&Group> = groups.iter().collect();
        let episodes = [tmdb_episode(1, Some(40)), tmdb_episode(2, Some(2))];
        assert_eq!(suggest_by_runtime(&refs, &episodes, 1, None), vec![(0, vec![1])]);
    }

    #[test]
    fn an_episode_without_a_runtime_takes_one_title() {
        let groups = [group(1, &[1], 2640), group(2, &[2], 1320), group(3, &[3], 2640), group(4, &[4], 2640)];
        let refs: Vec<&Group> = groups.iter().collect();
        // Episode 2 has no runtime, and TMDb lists no episode 6.
        let episodes = [
            tmdb_episode(1, Some(22)),
            tmdb_episode(2, None),
            tmdb_episode(3, Some(22)),
            tmdb_episode(4, Some(22)),
            tmdb_episode(5, Some(22)),
        ];
        assert_eq!(
            suggest_by_runtime(&refs, &episodes, 1, None),
            vec![(1, vec![1]), (2, vec![2]), (3, vec![3, 4]), (4, vec![5])]
        );
    }

    #[test]
    fn suggestions_follow_title_order_from_the_start_episode() {
        let groups = [group(5, &[5], 1320), group(2, &[2], 1320)];
        let refs: Vec<&Group> = groups.iter().collect();
        assert_eq!(suggest_by_runtime(&refs, &[], 5, None), vec![(2, vec![5]), (5, vec![6])]);
    }

    #[test]
    fn an_unmapped_disc_is_0_60_with_tv_unmapped_and_suggestions_by_runtime() {
        // A /tv/{id} answer with its season expanded, in TMDb's documented shape (no full TMDb record is recorded; phase
        // 2's client checks the shape live).
        let season_1: &[(u32, &str, Option<u32>)] =
            &[(1, "First", Some(22)), (2, "Second", Some(22)), (3, "Third", Some(22)), (4, "Fourth", Some(22))];
        let record = parse_tv(&tmdb_tv_json(1437, "Show", "2002-09-20", "en", &[(1, season_1)]), "tv.json").unwrap();
        // Title 3 lasts 5 minutes: not episode length.
        let groups = vec![group(0, &[1], 1320), group(1, &[2], 2640), group(2, &[3], 1300), group(3, &[4], 300)];
        assert_eq!(
            unmapped(&groups, Some(1), 1, record.season(1), None),
            TvPlan {
                mapped: false,
                episodes: vec![
                    episode_title(0, Some(1), &[1], Some("First"), true),
                    episode_title(1, Some(1), &[2, 3], Some("Second / Third"), true),
                    episode_title(2, Some(1), &[4], Some("Fourth"), true),
                ],
                confidence: 0.60,
                reviews: vec![ReviewReason::TvUnmapped],
                dropped_play_all: vec![],
                evidence: vec![
                    "no TheDiscDB episode map: title confidence 0.60 with tv_unmapped (8.5)".to_string(),
                    "episode-length titles suggested from episode 1: 3, by TMDb's episode runtimes".to_string(),
                    "title 0 suggested as S01E01 \"First\"".to_string(),
                    "title 1 suggested as S01E02-E03 \"Second / Third\"".to_string(),
                    "title 2 suggested as S01E04 \"Fourth\"".to_string(),
                ],
            }
        );
        assert_eq!(UNMAPPED_TV_CONFIDENCE, 0.60);
        assert_eq!(ReviewReason::TvUnmapped.as_str(), "tv_unmapped");
    }

    #[test]
    fn without_tmdb_episodes_each_episode_length_title_takes_one_episode_in_title_order() {
        let groups = vec![group(2, &[3], 1300), group(0, &[1], 1320), group(1, &[2], 2640)];
        let plan = unmapped(&groups, Some(2), 1, None, None);
        assert_eq!(
            plan.episodes,
            vec![
                episode_title(0, Some(2), &[1], None, true),
                episode_title(1, Some(2), &[2], None, true),
                episode_title(2, Some(2), &[3], None, true),
            ]
        );
        assert_eq!(
            plan.evidence[1],
            "episode-length titles suggested from episode 1: 3, one episode each in title order (no TMDb episodes for the season)"
        );
        assert_eq!(plan.evidence[2], "title 0 suggested as S02E01");
        // A season TMDb lists without its episodes gives the same plan.
        let listed = TmdbSeason { season_number: 2, episode_count: Some(10), episodes: vec![] };
        assert_eq!(unmapped(&groups, Some(2), 1, Some(&listed), None), plan);
    }

    #[test]
    fn an_unknown_season_stays_none_and_review_asks_for_it() {
        let groups = vec![group(0, &[1], 1320), group(1, &[2], 1320)];
        let plan = unmapped(&groups, None, 1, None, None);
        assert_eq!(plan.episodes, vec![episode_title(0, None, &[1], None, true), episode_title(1, None, &[2], None, true)]);
        assert_eq!(plan.reviews, vec![ReviewReason::TvUnmapped]);
        assert_eq!(
            plan.evidence,
            vec![
                "no TheDiscDB episode map: title confidence 0.60 with tv_unmapped (8.5)".to_string(),
                "episode-length titles suggested from episode 1: 2, one episode each in title order (no TMDb episodes for the season)"
                    .to_string(),
                "season unknown: review asks for it (8.5)".to_string(),
                "title 0 suggested as E01 (season unknown)".to_string(),
                "title 1 suggested as E02 (season unknown)".to_string(),
            ]
        );
    }

    #[test]
    fn only_episode_length_groups_are_suggested() {
        // 10 and 130 minutes are outside 8.2's band of 15 to 120 minutes; 15 and 120 minutes are inside it.
        let groups =
            vec![group(0, &[1], 600), group(1, &[2], 1320), group(2, &[3], 7800), group(3, &[4], 900), group(4, &[5], 7200)];
        let plan = unmapped(&groups, Some(1), 1, None, None);
        assert_eq!(
            plan.episodes.iter().map(|e| (e.title, e.episode.episodes.clone())).collect::<Vec<_>>(),
            vec![(1, vec![1]), (3, vec![2]), (4, vec![3])]
        );
    }

    #[test]
    fn a_tv_plan_round_trips_through_json_with_a_null_season() {
        let plan = unmapped(&[group(0, &[1], 1320)], None, 1, None, None);
        let text = serde_json::to_string(&plan).unwrap();
        assert!(text.contains(r#""episode":{"season":null,"episodes":[1],"name":null},"suggested":true"#), "{text}");
        assert_eq!(serde_json::from_str::<TvPlan>(&text).unwrap(), plan);
    }

    #[test]
    fn a_new_session_is_open_with_nothing_assigned_and_starts_at_episode_1() {
        let session = SeasonSession::new(1437, 1, Some(14));
        assert_eq!(
            session,
            SeasonSession { show_id: 1437, season: 1, discs: vec![], assigned: vec![], episode_count: Some(14), closed: false }
        );
        assert_eq!(session.next_unassigned(), 1);
    }

    #[test]
    fn next_unassigned_is_the_lowest_episode_not_assigned() {
        let mut session = SeasonSession::new(1437, 1, Some(14));
        assign(&mut session, FIREFLY_1_API, &[(1, 0), (2, 1), (4, 2)]);
        assert_eq!(session.next_unassigned(), 3);
        assign(&mut session, FIREFLY_2_API, &[(3, 0)]);
        assert_eq!(session.next_unassigned(), 5);
    }

    #[test]
    fn a_session_file_is_named_by_show_id_and_two_digit_season() {
        assert_eq!(SeasonSession::new(1437, 1, None).file_name(), "1437-s01.json");
        assert_eq!(SeasonSession::new(2316, 12, None).file_name(), "2316-s12.json");
        assert_eq!(SeasonSession::new(87108, 0, None).file_name(), "87108-s00.json");
    }

    #[test]
    fn find_uses_the_show_id_and_the_parsed_season() {
        let mut sessions =
            vec![SeasonSession::new(2316, 2, Some(22)), SeasonSession::new(2316, 3, Some(23)), SeasonSession::new(1437, 1, Some(14))];
        assert_eq!(find(&sessions, 2316, Some(3)), Found::Session(&sessions[1]));
        assert_eq!(find(&sessions, 1437, Some(1)), Found::Session(&sessions[2]));
        // A closed session is still the session of a disc of that season.
        close(&mut sessions[1]);
        assert_eq!(find(&sessions, 2316, Some(3)), Found::Session(&sessions[1]));
    }

    #[test]
    fn find_with_a_parsed_season_and_no_session_for_it_finds_none() {
        let sessions = vec![SeasonSession::new(2316, 2, Some(22)), SeasonSession::new(1437, 1, Some(14))];
        assert_eq!(find(&sessions, 2316, Some(4)), Found::None);
        assert_eq!(find(&sessions, 87108, Some(1)), Found::None);
    }

    #[test]
    fn without_a_parsed_season_find_uses_the_shows_single_open_session() {
        let mut sessions =
            vec![SeasonSession::new(2316, 2, Some(22)), SeasonSession::new(2316, 3, Some(23)), SeasonSession::new(1437, 1, Some(14))];
        close(&mut sessions[0]);
        assert_eq!(find(&sessions, 2316, None), Found::SingleOpen(&sessions[1]));
        assert_eq!(find(&sessions, 1437, None), Found::SingleOpen(&sessions[2]));
    }

    #[test]
    fn without_a_parsed_season_several_open_sessions_are_ambiguous() {
        let sessions = vec![SeasonSession::new(2316, 2, Some(22)), SeasonSession::new(2316, 3, Some(23))];
        assert_eq!(find(&sessions, 2316, None), Found::Ambiguous);
    }

    #[test]
    fn without_a_parsed_season_and_no_open_session_find_finds_none() {
        let mut sessions = vec![SeasonSession::new(2316, 2, Some(22)), SeasonSession::new(1437, 1, Some(14))];
        close(&mut sessions[0]);
        assert_eq!(find(&sessions, 2316, None), Found::None);
        assert_eq!(find(&[], 1437, None), Found::None);
    }

    #[test]
    fn suggest_starts_at_the_next_unassigned_episode_and_skips_assigned_ones() {
        let mut session = SeasonSession::new(1437, 1, Some(14));
        assign(&mut session, FIREFLY_1_API, &[(1, 0), (2, 1), (3, 2), (5, 4)]);
        assert_eq!(suggest(Some(&session), 3), vec![4, 6, 7]);
        assert_eq!(suggest(Some(&session), 0), Vec::<u32>::new());
    }

    #[test]
    fn without_a_session_suggestions_start_at_episode_1() {
        assert_eq!(suggest(None, 3), vec![1, 2, 3]);
    }

    #[test]
    fn assign_records_the_disc_once_and_each_episode_with_its_title() {
        let mut session = SeasonSession::new(1437, 1, Some(14));
        assign(&mut session, FIREFLY_1_API, &[(1, 0), (2, 1)]);
        assign(&mut session, FIREFLY_1_API, &[(3, 2)]);
        // The owner assigns episode 2 again, to the second disc's title 5: the latest assignment wins.
        assign(&mut session, FIREFLY_2_API, &[(2, 5)]);
        assert_eq!(session.discs, vec![FIREFLY_1_API, FIREFLY_2_API]);
        let assigned = |episode: u32, fingerprint: &str, title: u32| AssignedEpisode {
            episode,
            fingerprint: fingerprint.to_string(),
            title,
        };
        assert_eq!(
            session.assigned,
            vec![assigned(1, FIREFLY_1_API, 0), assigned(2, FIREFLY_2_API, 5), assigned(3, FIREFLY_1_API, 2)]
        );
        assert_eq!(session.get(2), Some(&assigned(2, FIREFLY_2_API, 5)));
        assert_eq!(session.get(4), None);
        assert!(!session.closed);
    }

    #[test]
    fn assign_closes_the_session_when_every_episode_is_assigned() {
        let mut session = SeasonSession::new(1437, 1, Some(3));
        assign(&mut session, FIREFLY_1_API, &[(1, 0), (2, 1)]);
        assert!(!session.closed);
        assign(&mut session, FIREFLY_2_API, &[(3, 0)]);
        assert!(session.closed);
    }

    #[test]
    fn a_session_without_an_episode_count_never_closes_by_assignment() {
        let mut session = SeasonSession::new(1437, 1, None);
        assign(&mut session, FIREFLY_1_API, &[(1, 0), (2, 1), (3, 2)]);
        assert!(!session.closed);
    }

    #[test]
    fn close_closes_the_session_and_it_stays_closed() {
        let mut session = SeasonSession::new(1437, 1, Some(14));
        close(&mut session);
        assert!(session.closed);
        assign(&mut session, FIREFLY_1_API, &[(1, 0)]);
        assert!(session.closed);
        assert_eq!(session.assigned.len(), 1);
    }

    #[test]
    fn a_season_session_round_trips_through_json() {
        let mut session = SeasonSession::new(1437, 1, Some(14));
        assign(&mut session, FIREFLY_1_API, &[(1, 0), (2, 1), (3, 2)]);
        let text = serde_json::to_string(&session).unwrap();
        assert_eq!(
            text,
            concat!(
                r#"{"show_id":1437,"season":1,"discs":["5A961BC67EB34171F24852D3A7688020"],"assigned":["#,
                r#"{"episode":1,"fingerprint":"5A961BC67EB34171F24852D3A7688020","title":0},"#,
                r#"{"episode":2,"fingerprint":"5A961BC67EB34171F24852D3A7688020","title":1},"#,
                r#"{"episode":3,"fingerprint":"5A961BC67EB34171F24852D3A7688020","title":2}],"episode_count":14,"#,
                r#""closed":false}"#
            )
        );
        assert_eq!(serde_json::from_str::<SeasonSession>(&text).unwrap(), session);
    }

    #[test]
    fn unmapped_suggestions_start_at_the_sessions_next_unassigned_episode() {
        let mut session = SeasonSession::new(1437, 1, Some(14));
        assign(&mut session, FIREFLY_1_API, &[(1, 0), (2, 1), (3, 2)]);
        let groups = vec![group(0, &[1], 2631), group(1, &[2], 2559)];
        let plan = unmapped(&groups, Some(1), session.next_unassigned(), None, Some(&session));
        assert_eq!(plan.episodes, vec![episode_title(0, Some(1), &[4], None, true), episode_title(1, Some(1), &[5], None, true)]);
        assert_eq!(
            plan.evidence[1],
            "episode-length titles suggested from episode 4: 2, one episode each in title order (no TMDb episodes for the season)"
        );
    }

    #[test]
    fn unmapped_suggestions_skip_the_episodes_the_session_already_assigned() {
        // Episodes 1 to 4 and 7 and 8 belong to other discs; four episode-length titles take 5, 6, 9 and 10, never 7 and 8
        // (8.5: the same rule as `suggest`).
        let mut session = SeasonSession::new(1437, 1, Some(14));
        assign(&mut session, FIREFLY_1_API, &[(1, 0), (2, 1), (3, 2), (4, 3), (7, 4), (8, 5)]);
        assert_eq!(suggest(Some(&session), 4), vec![5, 6, 9, 10]);
        let groups = vec![group(0, &[1], 1320), group(1, &[2], 1320), group(2, &[3], 1320), group(3, &[4], 1320)];
        let plan = unmapped(&groups, Some(1), session.next_unassigned(), None, Some(&session));
        assert_eq!(
            plan.episodes.iter().map(|e| (e.title, e.episode.episodes.clone())).collect::<Vec<_>>(),
            vec![(0, vec![5]), (1, vec![6]), (2, vec![9]), (3, vec![10])]
        );
        // A double-length title takes two episodes only when both are free: episode 6 is free and 7 is not, so the
        // second title (two episodes long) takes 6 alone.
        let episodes = (1..=12).map(|number| tmdb_episode(number, Some(22))).collect::<Vec<TmdbEpisode>>();
        let doubled = [group(0, &[1], 1320), group(1, &[2], 2640), group(2, &[3], 2640)];
        let refs: Vec<&Group> = doubled.iter().collect();
        assert_eq!(
            suggest_by_runtime(&refs, &episodes, session.next_unassigned(), Some(&session)),
            vec![(0, vec![5]), (1, vec![6]), (2, vec![9, 10])]
        );
    }
}
