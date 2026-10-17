//! TMDb's answers as spec 8.1 reads them, `core::lookup::tmdb`: the identity (TMDb numbers movies and TV shows
//! separately, so an identity is always the pair of kind and ID), the first page of `/search/multi` ("Only the first page
//! of results is considered"; "Movies and TV are searched together"), the record of `/movie/{id}` or `/tv/{id}` with its
//! external IDs ("title, year, original language, runtime, the IMDb and TVDB IDs and, for TV, seasons and episodes with
//! runtimes"), a season answer of `/tv/{id}/season/{n}`, the recording's slim `tmdb.json`, and `Work`, the title, year
//! and IDs spec 8.9 names a file with. No TMDb search answer and no full record is recorded anywhere, so the tests use
//! synthetic answers in TMDb's documented shapes; phase 2's client verifies the shapes live. A runtime of 0 minutes,
//! which TMDb writes for an unknown runtime as often as null, reads as None. Used by Tasks 4 to 7, 11, 13 and 15.

use std::str::FromStr;

use serde_json::Value;

use super::parse_json;
use crate::recording::files::Tmdb;

/// TMDb's two kinds; an identity is always the pair (8.1 "the two kinds number their records separately"). Spec 8.1.
/// Def: T3. Use: T4 to T7, T11, T15.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TmdbKind {
    Movie,
    Tv,
}

impl TmdbKind {
    /// Both kinds, in TMDb's own order (movie, then tv).
    pub const ALL: [TmdbKind; 2] = [TmdbKind::Movie, TmdbKind::Tv];

    /// "movie" or "tv": TMDb's `media_type` spelling and the serde one. Spec 8.1.
    pub fn as_str(self) -> &'static str {
        match self {
            TmdbKind::Movie => "movie",
            TmdbKind::Tv => "tv",
        }
    }
}

/// "movie" or "tv" exactly (a search result's `media_type`); "person" and every other text is refused. Spec 8.1.
impl FromStr for TmdbKind {
    type Err = String;

    fn from_str(text: &str) -> Result<TmdbKind, String> {
        TmdbKind::ALL
            .into_iter()
            .find(|kind| kind.as_str() == text)
            .ok_or_else(|| format!("{text:?} is not a TMDb kind (movie or tv)"))
    }
}

/// A TMDb identity: the kind and the ID within that kind's numbering. Spec 8.1. Def: T3. Use: T4, T6, T7, T14.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct TmdbIdentity {
    pub kind: TmdbKind,
    pub id: i64,
}

/// One `/search/multi` result: `media_type` "movie" or "tv" (a "person" result is dropped by the parser). Spec 8.1.
/// Def: T3. Use: T4.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TmdbSearchResult {
    pub media_type: String,
    pub id: i64,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub original_title: Option<String>,
    #[serde(default)]
    pub original_name: Option<String>,
    #[serde(default)]
    pub release_date: Option<String>,
    #[serde(default)]
    pub first_air_date: Option<String>,
}

impl TmdbSearchResult {
    /// The (kind, ID) pair of a movie or tv result; None for any other media type. Spec 8.1. Def: T3. Use: T4.
    pub fn identity(&self) -> Option<TmdbIdentity> {
        Some(TmdbIdentity { kind: self.kind()?, id: self.id })
    }

    /// `title` for a movie, `name` for tv; "" when absent (or for another media type). Spec 8.1. Def: T3.
    pub fn display_title(&self) -> &str {
        let text = match self.kind() {
            Some(TmdbKind::Movie) => &self.title,
            Some(TmdbKind::Tv) => &self.name,
            None => return "",
        };
        text.as_deref().unwrap_or("")
    }

    /// `original_title` for a movie, `original_name` for tv; "" when absent (or for another media type). Spec 8.1
    /// "its title or original title". Def: T3.
    pub fn original(&self) -> &str {
        let text = match self.kind() {
            Some(TmdbKind::Movie) => &self.original_title,
            Some(TmdbKind::Tv) => &self.original_name,
            None => return "",
        };
        text.as_deref().unwrap_or("")
    }

    /// The year of release_date (a movie) or first_air_date (tv): the four digits before the first '-'. Spec 8.1.
    /// Def: T3.
    pub fn year(&self) -> Option<i32> {
        match self.kind()? {
            TmdbKind::Movie => year_of(self.release_date.as_deref()),
            TmdbKind::Tv => year_of(self.first_air_date.as_deref()),
        }
    }

    /// The media type as a TMDb kind; None for "person" and any other.
    fn kind(&self) -> Option<TmdbKind> {
        self.media_type.parse().ok()
    }
}

/// The first page of `/search/multi` (8.1 "Only the first page of results is considered"). Spec 8.1. Def: T3. Use: T4,
/// T15.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TmdbSearch {
    #[serde(default)]
    pub results: Vec<TmdbSearchResult>,
}

/// Parses the answer, keeping movie and tv results only (a result of another media type is dropped before it is read);
/// the page number and totals are not read. Err is serde_json's message prefixed by `shown`. Spec 8.1. Def: T3. Use: T15.
pub fn parse_search(text: &str, shown: &str) -> Result<TmdbSearch, String> {
    #[derive(serde::Deserialize)]
    struct Page {
        #[serde(default)]
        results: Vec<Value>,
    }
    let page: Page = parse_json(text, shown)?;
    let mut results: Vec<TmdbSearchResult> = Vec::new();
    for result in page.results {
        if matches!(result.get("media_type").and_then(Value::as_str), Some("movie") | Some("tv")) {
            results.push(serde_json::from_value(result).map_err(|error| format!("{shown}: {error}"))?);
        }
    }
    Ok(TmdbSearch { results })
}

/// One episode of `/tv/{id}/season/{n}`: `runtime` minutes may be null; 0 reads as None (parse_tv, merge_season).
/// Spec 8.1. Def: T3. Use: T6.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TmdbEpisode {
    pub episode_number: u32,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub runtime: Option<u32>,
}

/// One season: `season_number` (0 is specials), `episode_count`, and the episodes once the season call is merged in
/// (empty until then). Spec 8.1. Def: T3. Use: T4, T6.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TmdbSeason {
    pub season_number: u32,
    #[serde(default)]
    pub episode_count: Option<u32>,
    #[serde(default)]
    pub episodes: Vec<TmdbEpisode>,
}

/// The TMDb record 8.1 fetches: title, year, original language, runtime, the IMDb and TVDB IDs and, for TV, seasons and
/// episodes with runtimes. Read from `/movie/{id}` (`title`, `release_date`, `runtime`), `/tv/{id}` (`name`,
/// `first_air_date`, `seasons`), their `external_ids`, and the recording's slim `tmdb.json` (which has no runtime and no
/// seasons). Spec 8.1. Def: T3. Use: T4 to T7, T11, T13, T15.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TmdbRecord {
    pub identity: TmdbIdentity,
    /// `title` or `name`.
    pub title: String,
    /// `original_title` or `original_name` (the title when TMDb gives none).
    pub original_title: String,
    /// The year of `release_date` or `first_air_date`; None when absent.
    pub year: Option<i32>,
    /// `original_language`, two letters ("es", "en", "xx").
    pub original_language: Option<String>,
    /// `runtime` minutes (movies); None when TMDb has none (null or 0).
    pub runtime: Option<u32>,
    /// `external_ids.imdb_id` ("tt0001417") and `external_ids.tvdb_id`.
    pub imdb_id: Option<String>,
    pub tvdb_id: Option<i64>,
    /// TV only; empty for a movie.
    pub seasons: Vec<TmdbSeason>,
}

impl TmdbRecord {
    /// The season with `season_number == n`. Spec 8.1. Def: T3. Use: T4, T6, T7.
    pub fn season(&self, n: u32) -> Option<&TmdbSeason> {
        self.seasons.iter().find(|season| season.season_number == n)
    }

    /// Every episode runtime of season `n`, or of every season when `n` is None (8.1 "of the whole series when no season
    /// was parsed"; the whole series is every season numbered 1 or more, without season 0's specials), in the record's
    /// order, leaving out episodes without a runtime. Spec 8.1. Def: T3. Use: T4.
    pub fn episode_runtimes(&self, season: Option<u32>) -> Vec<u32> {
        self.seasons
            .iter()
            .filter(|listed| match season {
                Some(n) => listed.season_number == n,
                None => listed.season_number != 0,
            })
            .flat_map(|listed| listed.episodes.iter().filter_map(|episode| episode.runtime))
            .collect()
    }

    /// Spec 8.9 "Title and year always come from the same TMDb record as the ID": the Work for naming. Def: T3. Use: T11,
    /// T13.
    pub fn work(&self) -> Work {
        Work {
            kind: self.identity.kind,
            tmdb_id: self.identity.id,
            title: self.title.clone(),
            year: self.year,
            imdb_id: self.imdb_id.clone(),
            tvdb_id: self.tvdb_id,
        }
    }
}

/// Title, year and IDs from one TMDb record, as naming reads them (8.9). Spec 8.9. Def: T3. Use: T11, T13.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Work {
    pub kind: TmdbKind,
    pub tmdb_id: i64,
    pub title: String,
    pub year: Option<i32>,
    pub imdb_id: Option<String>,
    pub tvdb_id: Option<i64>,
}

/// The external IDs `append_to_response=external_ids` embeds; their other keys (wikidata_id, facebook_id) are not read.
#[derive(Default, serde::Deserialize)]
struct RawExternalIds {
    #[serde(default)]
    imdb_id: Option<String>,
    #[serde(default)]
    tvdb_id: Option<i64>,
}

/// The keys of `/movie/{id}` and `/tv/{id}` a record reads; every other key is ignored.
#[derive(serde::Deserialize)]
struct RawRecord {
    id: i64,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    original_title: Option<String>,
    #[serde(default)]
    original_name: Option<String>,
    #[serde(default)]
    release_date: Option<String>,
    #[serde(default)]
    first_air_date: Option<String>,
    #[serde(default)]
    original_language: Option<String>,
    #[serde(default)]
    runtime: Option<u32>,
    #[serde(default)]
    imdb_id: Option<String>,
    #[serde(default)]
    external_ids: Option<RawExternalIds>,
    #[serde(default)]
    seasons: Vec<TmdbSeason>,
}

/// `/movie/{id}` text (with `external_ids` appended or embedded as `append_to_response` gives it): the IMDb ID from
/// `external_ids.imdb_id`, else the top-level `imdb_id`; a runtime of 0 is None. Err without a title. Spec 8.1. Def: T3.
/// Use: T15.
pub fn parse_movie(text: &str, shown: &str) -> Result<TmdbRecord, String> {
    let raw: RawRecord = parse_json(text, shown)?;
    let id = raw.id;
    let title = present(raw.title).ok_or_else(|| format!("{shown}: the movie record {id} has no title"))?;
    let original_title = present(raw.original_title).unwrap_or_else(|| title.clone());
    let (imdb_id, tvdb_id) = external_ids(raw.external_ids, raw.imdb_id);
    Ok(TmdbRecord {
        identity: TmdbIdentity { kind: TmdbKind::Movie, id },
        title,
        original_title,
        year: year_of(raw.release_date.as_deref()),
        original_language: present(raw.original_language),
        runtime: known_minutes(raw.runtime),
        imdb_id,
        tvdb_id,
        seasons: Vec::new(),
    })
}

/// `/tv/{id}` text; `seasons[].episodes` are read when present (a test fixture or a merged season call), an episode
/// runtime of 0 as None. Err without a name. Spec 8.1. Def: T3. Use: T15.
pub fn parse_tv(text: &str, shown: &str) -> Result<TmdbRecord, String> {
    let raw: RawRecord = parse_json(text, shown)?;
    let id = raw.id;
    let title = present(raw.name).ok_or_else(|| format!("{shown}: the TV record {id} has no name"))?;
    let original_title = present(raw.original_name).unwrap_or_else(|| title.clone());
    let (imdb_id, tvdb_id) = external_ids(raw.external_ids, raw.imdb_id);
    let seasons = raw
        .seasons
        .into_iter()
        .map(|season| TmdbSeason { episodes: season.episodes.into_iter().map(known_episode).collect(), ..season })
        .collect();
    Ok(TmdbRecord {
        identity: TmdbIdentity { kind: TmdbKind::Tv, id },
        title,
        original_title,
        year: year_of(raw.first_air_date.as_deref()),
        original_language: present(raw.original_language),
        runtime: None,
        imdb_id,
        tvdb_id,
        seasons,
    })
}

/// A season answer merged into `record.seasons` (replacing that season's episodes; a season the record lacks is added
/// and the seasons kept in number order; an absent episode_count takes the count merged). Err for a movie record, before
/// the text is read, and for a malformed answer. Spec 8.1. Def: T3. Use: none in phase 1 (phase 2's TMDb client merges
/// its season calls).
pub fn merge_season(record: &mut TmdbRecord, season_text: &str, shown: &str) -> Result<(), String> {
    #[derive(serde::Deserialize)]
    struct SeasonAnswer {
        season_number: u32,
        #[serde(default)]
        episodes: Vec<TmdbEpisode>,
    }
    if record.identity.kind != TmdbKind::Tv {
        return Err(format!("{shown}: a season answer belongs to a TV record, not to movie {}", record.identity.id));
    }
    let answer: SeasonAnswer = parse_json(season_text, shown)?;
    let episodes: Vec<TmdbEpisode> = answer.episodes.into_iter().map(known_episode).collect();
    let count = u32::try_from(episodes.len()).unwrap_or(u32::MAX);
    if let Some(season) = record.seasons.iter_mut().find(|season| season.season_number == answer.season_number) {
        season.episode_count = season.episode_count.or(Some(count));
        season.episodes = episodes;
        return Ok(());
    }
    record.seasons.push(TmdbSeason { season_number: answer.season_number, episode_count: Some(count), episodes });
    record.seasons.sort_by_key(|season| season.season_number);
    Ok(())
}

/// The IMDb ID (the embedded `external_ids.imdb_id`, else the top-level one) and the TVDB ID (positive only).
fn external_ids(ids: Option<RawExternalIds>, top_level_imdb: Option<String>) -> (Option<String>, Option<i64>) {
    let ids = ids.unwrap_or_default();
    (present(ids.imdb_id).or_else(|| present(top_level_imdb)), ids.tvdb_id.filter(|&tvdb| tvdb > 0))
}

/// Text that holds something other than white space, trimmed; None for absent, empty or blank text.
fn present(text: Option<String>) -> Option<String> {
    text.map(|text| text.trim().to_string()).filter(|text| !text.is_empty())
}

/// The year of a TMDb date ("2006-10-11"): the four ASCII digits before the first '-'; None otherwise.
fn year_of(date: Option<&str>) -> Option<i32> {
    let year = date?.split('-').next()?;
    if year.len() == 4 && year.bytes().all(|byte| byte.is_ascii_digit()) {
        year.parse().ok()
    } else {
        None
    }
}

/// A runtime in minutes, with TMDb's 0 for "unknown" read as None.
fn known_minutes(minutes: Option<u32>) -> Option<u32> {
    minutes.filter(|&minutes| minutes > 0)
}

/// An episode with `known_minutes` of its runtime.
fn known_episode(episode: TmdbEpisode) -> TmdbEpisode {
    TmdbEpisode { runtime: known_minutes(episode.runtime), ..episode }
}

/// The recording's slim `tmdb.json`, read as `recording::files::Tmdb` (extended by this task with the optional fields
/// `id`, `title`, `name`, `original_title`, `original_name`, `release_date`, `first_air_date: Option<String>` and
/// `external_ids: Option<serde_json::Map<String, serde_json::Value>>`, read leniently: a value of another type is None,
/// never an error; unknown keys stay ignored and its existing readers are untouched), then mapped: a movie when it has
/// `title`, tv when it has `name`; `fetched_at` is ignored. No runtime and no seasons (the slim file has neither); the IMDb
/// ID is `external_ids.imdb_id`, the TVDB ID `external_ids.tvdb_id`.
/// Err without an id, or with neither a title nor a name. Spec 8.1. Def: T3. Use: T15.
pub fn parse_recorded(text: &str, shown: &str) -> Result<TmdbRecord, String> {
    let slim: Tmdb = parse_json(text, shown)?;
    let id = slim.id.ok_or_else(|| format!("{shown}: the record has no id"))?;
    let ids = slim.external_ids.unwrap_or_default();
    let imdb_id = present(ids.get("imdb_id").and_then(Value::as_str).map(str::to_string));
    let tvdb_id = ids.get("tvdb_id").and_then(Value::as_i64).filter(|&tvdb| tvdb > 0);
    let (kind, title, original, date) = match (present(slim.title), present(slim.name)) {
        (Some(title), _) => (TmdbKind::Movie, title, slim.original_title, slim.release_date),
        (None, Some(name)) => (TmdbKind::Tv, name, slim.original_name, slim.first_air_date),
        (None, None) => {
            return Err(format!("{shown}: the record has neither a title (a movie) nor a name (a TV show)"));
        }
    };
    let original_title = present(original).unwrap_or_else(|| title.clone());
    Ok(TmdbRecord {
        identity: TmdbIdentity { kind, id },
        title,
        original_title,
        year: year_of(date.as_deref()),
        original_language: present(slim.original_language),
        runtime: None,
        imdb_id,
        tvdb_id,
        seasons: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testsupport::{search_json, tmdb_movie_json, tmdb_tv_json};

    #[test]
    fn tmdb_kinds_spell_themselves_as_tmdbs_media_types() {
        assert_eq!(TmdbKind::ALL, [TmdbKind::Movie, TmdbKind::Tv]);
        assert_eq!(TmdbKind::ALL.map(TmdbKind::as_str), ["movie", "tv"]);
        assert_eq!(serde_json::to_string(&TmdbKind::Movie).unwrap(), "\"movie\"");
        assert_eq!(serde_json::from_str::<TmdbKind>("\"tv\"").unwrap(), TmdbKind::Tv);
        assert!(serde_json::from_str::<TmdbKind>("\"person\"").is_err());
        assert_eq!("movie".parse::<TmdbKind>(), Ok(TmdbKind::Movie));
        assert_eq!("person".parse::<TmdbKind>(), Err("\"person\" is not a TMDb kind (movie or tv)".to_string()));
    }

    #[test]
    fn an_identity_is_the_pair_of_kind_and_id() {
        // TMDb numbers movies and TV shows separately (spec 8.1): the same number names two works.
        let movie = TmdbIdentity { kind: TmdbKind::Movie, id: 1437 };
        let show = TmdbIdentity { kind: TmdbKind::Tv, id: 1437 };
        assert_ne!(movie, show);
        assert!(movie < show);
        assert_eq!(serde_json::to_string(&show).unwrap(), r#"{"kind":"tv","id":1437}"#);
        assert_eq!(serde_json::from_str::<TmdbIdentity>(r#"{"kind":"movie","id":1437}"#).unwrap(), movie);
    }

    /// `/search/multi` in TMDb's documented shape: a movie, a person and a TV show on the first page (synthetic).
    const SEARCH: &str = r#"{
  "page": 1,
  "results": [
    {"adult": false, "id": 1417, "media_type": "movie", "title": "Pan's Labyrinth", "original_title": "El laberinto del fauno", "original_language": "es", "release_date": "2006-10-11", "popularity": 31.2},
    {"adult": false, "id": 52, "media_type": "person", "name": "A Person", "known_for_department": "Acting", "known_for": []},
    {"adult": false, "id": 2316, "media_type": "tv", "name": "The Office", "original_name": "The Office", "original_language": "en", "first_air_date": "2005-03-24", "origin_country": ["US"]}
  ],
  "total_pages": 1,
  "total_results": 3
}"#;

    /// `/movie/1417?append_to_response=external_ids` in TMDb's documented shape (synthetic).
    const MOVIE: &str = r#"{
  "adult": false,
  "genres": [{"id": 18, "name": "Drama"}, {"id": 14, "name": "Fantasy"}],
  "id": 1417,
  "imdb_id": "tt0001417",
  "original_language": "es",
  "original_title": "El laberinto del fauno",
  "release_date": "2006-10-11",
  "runtime": 119,
  "status": "Released",
  "title": "Pan's Labyrinth",
  "vote_average": 7.8,
  "external_ids": {"imdb_id": "tt0001417", "wikidata_id": null, "facebook_id": null, "instagram_id": null, "twitter_id": null}
}"#;

    /// `/tv/2316?append_to_response=external_ids` in TMDb's documented shape (synthetic; its seasons carry no episodes,
    /// as TMDb's do not).
    const TV: &str = r#"{
  "episode_run_time": [22],
  "first_air_date": "2005-03-24",
  "id": 2316,
  "name": "The Office",
  "number_of_seasons": 9,
  "original_language": "en",
  "original_name": "The Office",
  "seasons": [
    {"air_date": "2007-09-27", "episode_count": 26, "id": 101, "name": "Specials", "season_number": 0},
    {"air_date": "2005-03-24", "episode_count": 6, "id": 102, "name": "Season 1", "season_number": 1},
    {"air_date": "2005-09-20", "episode_count": 22, "id": 103, "name": "Season 2", "season_number": 2},
    {"air_date": "2006-09-21", "episode_count": 25, "id": 104, "name": "Season 3", "season_number": 3}
  ],
  "external_ids": {"imdb_id": "tt0002316", "tvdb_id": 102316, "wikidata_id": null}
}"#;

    /// `/tv/2316/season/3` in TMDb's documented shape (synthetic): two runtimes, a 0 and a null.
    const SEASON_3: &str = r#"{
  "air_date": "2006-09-21",
  "episodes": [
    {"air_date": "2007-01-04", "episode_number": 12, "id": 201, "name": "Traveling Salesmen", "runtime": 22, "season_number": 3},
    {"air_date": "2007-01-11", "episode_number": 13, "id": 202, "name": "The Return", "runtime": 22, "season_number": 3},
    {"air_date": "2007-01-18", "episode_number": 14, "id": 203, "name": "Ben Franklin", "runtime": 0, "season_number": 3},
    {"air_date": "2007-02-01", "episode_number": 15, "id": 204, "name": "Phyllis' Wedding", "runtime": null, "season_number": 3}
  ],
  "id": 104,
  "name": "Season 3",
  "season_number": 3
}"#;

    fn pans_labyrinth() -> TmdbRecord {
        TmdbRecord {
            identity: TmdbIdentity { kind: TmdbKind::Movie, id: 1417 },
            title: "Pan's Labyrinth".to_string(),
            original_title: "El laberinto del fauno".to_string(),
            year: Some(2006),
            original_language: Some("es".to_string()),
            runtime: Some(119),
            imdb_id: Some("tt0001417".to_string()),
            tvdb_id: None,
            seasons: Vec::new(),
        }
    }

    fn season(season_number: u32, episode_count: u32) -> TmdbSeason {
        TmdbSeason { season_number, episode_count: Some(episode_count), episodes: Vec::new() }
    }

    fn episode(episode_number: u32, name: &str, runtime: Option<u32>) -> TmdbEpisode {
        TmdbEpisode { episode_number, name: Some(name.to_string()), runtime }
    }

    fn the_office() -> TmdbRecord {
        TmdbRecord {
            identity: TmdbIdentity { kind: TmdbKind::Tv, id: 2316 },
            title: "The Office".to_string(),
            original_title: "The Office".to_string(),
            year: Some(2005),
            original_language: Some("en".to_string()),
            runtime: None,
            imdb_id: Some("tt0002316".to_string()),
            tvdb_id: Some(102316),
            seasons: vec![season(0, 26), season(1, 6), season(2, 22), season(3, 25)],
        }
    }

    #[test]
    fn a_search_answer_keeps_movie_and_tv_results_and_drops_other_media_types() {
        let search = parse_search(SEARCH, "search.json").unwrap();
        assert_eq!(
            search.results,
            vec![
                TmdbSearchResult {
                    media_type: "movie".to_string(),
                    id: 1417,
                    title: Some("Pan's Labyrinth".to_string()),
                    name: None,
                    original_title: Some("El laberinto del fauno".to_string()),
                    original_name: None,
                    release_date: Some("2006-10-11".to_string()),
                    first_air_date: None,
                },
                TmdbSearchResult {
                    media_type: "tv".to_string(),
                    id: 2316,
                    title: None,
                    name: Some("The Office".to_string()),
                    original_title: None,
                    original_name: Some("The Office".to_string()),
                    release_date: None,
                    first_air_date: Some("2005-03-24".to_string()),
                },
            ]
        );
        // Movies and TV are searched together, and an identity is the pair of kind and ID (spec 8.1).
        let identities: Vec<Option<TmdbIdentity>> = search.results.iter().map(TmdbSearchResult::identity).collect();
        assert_eq!(
            identities,
            vec![Some(TmdbIdentity { kind: TmdbKind::Movie, id: 1417 }), Some(TmdbIdentity { kind: TmdbKind::Tv, id: 2316 })]
        );
    }

    #[test]
    fn a_search_result_reads_its_title_original_and_year_by_its_media_type() {
        let search = parse_search(SEARCH, "search.json").unwrap();
        let (movie, show) = (&search.results[0], &search.results[1]);
        assert_eq!((movie.display_title(), movie.original(), movie.year()), ("Pan's Labyrinth", "El laberinto del fauno", Some(2006)));
        assert_eq!((show.display_title(), show.original(), show.year()), ("The Office", "The Office", Some(2005)));
        // A movie's name and first_air_date are not its title and date; a missing one reads as "" or None.
        let crossed = TmdbSearchResult {
            media_type: "movie".to_string(),
            id: 1,
            title: None,
            name: Some("Name".to_string()),
            original_title: None,
            original_name: Some("Name".to_string()),
            release_date: None,
            first_air_date: Some("2001-01-01".to_string()),
        };
        assert_eq!((crossed.display_title(), crossed.original(), crossed.year()), ("", "", None));
        let other = TmdbSearchResult { media_type: "person".to_string(), ..crossed.clone() };
        assert_eq!((other.identity(), other.display_title(), other.original(), other.year()), (None, "", "", None));
        // The year is the four digits before the date's first '-'.
        for (date, year) in
            [("2006-10-11", Some(2006)), ("2006", Some(2006)), ("", None), ("206-10-11", None), ("20o6-10-11", None), ("10/11/2006", None)]
        {
            let result = TmdbSearchResult { release_date: Some(date.to_string()), ..crossed.clone() };
            assert_eq!(result.year(), year, "{date:?}");
        }
    }

    #[test]
    fn a_search_answer_without_results_is_empty_and_a_malformed_one_is_refused() {
        let none = r#"{"page": 1, "results": [], "total_pages": 0, "total_results": 0}"#;
        assert_eq!(parse_search(none, "search.json"), Ok(TmdbSearch::default()));
        assert_eq!(parse_search("\u{feff}{}", "search.json"), Ok(TmdbSearch::default()));
        // A result whose media type is not movie or tv is dropped before it is read, whatever its shape.
        let dropped = r#"{"results": [{"media_type": "person"}, {"name": "no media type"}]}"#;
        assert_eq!(parse_search(dropped, "search.json"), Ok(TmdbSearch::default()));
        let empty = parse_search("", "search.json").unwrap_err();
        assert!(empty.starts_with("search.json: EOF while parsing"), "{empty}");
        let no_id = parse_search(r#"{"results": [{"media_type": "movie", "title": "Untitled"}]}"#, "search.json").unwrap_err();
        assert!(no_id.starts_with("search.json: missing field `id`"), "{no_id}");
    }

    #[test]
    fn a_movie_record_reads_title_year_language_runtime_and_external_ids() {
        assert_eq!(parse_movie(MOVIE, "movie.json"), Ok(pans_labyrinth()));
        assert_eq!(parse_movie(&format!("\u{feff}{MOVIE}"), "movie.json"), Ok(pans_labyrinth()));
    }

    #[test]
    fn a_movie_record_falls_back_to_its_top_level_imdb_id_and_reads_unknown_values_as_none() {
        let bare = r#"{"id": 1417, "title": "Pan's Labyrinth", "imdb_id": "tt0001417", "runtime": null, "release_date": "2006-10-11"}"#;
        assert_eq!(
            parse_movie(bare, "movie.json"),
            Ok(TmdbRecord {
                original_title: "Pan's Labyrinth".to_string(),
                original_language: None,
                runtime: None,
                ..pans_labyrinth()
            })
        );
        // TMDb writes 0 minutes and empty strings for what it does not know (Review Focus 2: no runtime, no pass).
        let unknown = r#"{"id": 1417, "title": "Pan's Labyrinth", "original_title": " ", "original_language": "", "release_date": "",
            "runtime": 0, "imdb_id": "", "external_ids": {"imdb_id": null, "tvdb_id": null}}"#;
        assert_eq!(
            parse_movie(unknown, "movie.json"),
            Ok(TmdbRecord {
                original_title: "Pan's Labyrinth".to_string(),
                year: None,
                original_language: None,
                runtime: None,
                imdb_id: None,
                ..pans_labyrinth()
            })
        );
        // The embedded external_ids win over the top-level imdb_id.
        let both = r#"{"id": 1417, "title": "Pan's Labyrinth", "imdb_id": "tt0000001", "external_ids": {"imdb_id": "tt0001417"}}"#;
        assert_eq!(parse_movie(both, "movie.json").unwrap().imdb_id.as_deref(), Some("tt0001417"));
    }

    #[test]
    fn a_record_without_a_title_or_a_name_or_an_id_is_refused() {
        assert_eq!(
            parse_movie(r#"{"id": 1417, "title": ""}"#, "movie.json"),
            Err("movie.json: the movie record 1417 has no title".to_string())
        );
        assert_eq!(
            parse_tv(r#"{"id": 2316, "title": "The Office"}"#, "tv.json"),
            Err("tv.json: the TV record 2316 has no name".to_string())
        );
        let no_id = parse_movie(r#"{"title": "Pan's Labyrinth"}"#, "movie.json").unwrap_err();
        assert!(no_id.starts_with("movie.json: missing field `id`"), "{no_id}");
        let broken = parse_tv("{", "tv.json").unwrap_err();
        assert!(broken.starts_with("tv.json: EOF while parsing"), "{broken}");
    }

    #[test]
    fn a_tv_record_reads_its_seasons_and_its_imdb_and_tvdb_ids() {
        let record = parse_tv(TV, "tv.json").unwrap();
        assert_eq!(record, the_office());
        assert_eq!(record.season(3), Some(&season(3, 25)));
        assert_eq!(record.season(4), None);
        // A /tv answer whose seasons already hold their episodes (a merged answer, a test fixture) reads them too.
        let expanded = r#"{"id": 2316, "name": "The Office", "seasons": [{"season_number": 3, "episode_count": 2, "episodes": [
            {"episode_number": 12, "name": "Traveling Salesmen", "runtime": 22}, {"episode_number": 13, "name": "The Return", "runtime": 0}]}]}"#;
        let record = parse_tv(expanded, "tv.json").unwrap();
        assert_eq!(
            record.seasons,
            vec![TmdbSeason {
                season_number: 3,
                episode_count: Some(2),
                episodes: vec![episode(12, "Traveling Salesmen", Some(22)), episode(13, "The Return", None)],
            }]
        );
        assert_eq!((record.original_title.as_str(), record.year, record.imdb_id, record.tvdb_id), ("The Office", None, None, None));
        // A TVDB ID of 0 or less is no ID.
        assert_eq!(parse_tv(r#"{"id": 2316, "name": "The Office", "external_ids": {"tvdb_id": 0}}"#, "tv.json").unwrap().tvdb_id, None);
    }

    #[test]
    fn a_season_answer_merges_its_episodes_into_the_tv_record() {
        let mut record = parse_tv(TV, "tv.json").unwrap();
        merge_season(&mut record, SEASON_3, "season-3.json").unwrap();
        let episodes = vec![
            episode(12, "Traveling Salesmen", Some(22)),
            episode(13, "The Return", Some(22)),
            episode(14, "Ben Franklin", None),
            episode(15, "Phyllis' Wedding", None),
        ];
        assert_eq!(record.season(3), Some(&TmdbSeason { season_number: 3, episode_count: Some(25), episodes }));
        // Merging again replaces that season's episodes; the other seasons are untouched.
        let again = r#"{"season_number": 3, "episodes": [{"episode_number": 12, "name": "Traveling Salesmen", "runtime": 23}]}"#;
        merge_season(&mut record, again, "season-3.json").unwrap();
        assert_eq!(record.season(3).map(|season| season.episodes.clone()), Some(vec![episode(12, "Traveling Salesmen", Some(23))]));
        assert_eq!(record.seasons.iter().map(|season| season.season_number).collect::<Vec<u32>>(), vec![0, 1, 2, 3]);
        assert!(record.seasons.iter().filter(|season| season.season_number != 3).all(|season| season.episodes.is_empty()));
    }

    #[test]
    fn a_season_answer_for_a_season_the_record_lacks_is_added_in_season_order() {
        let mut record = TmdbRecord { seasons: vec![season(1, 6), season(3, 25)], ..the_office() };
        let season_2 = r#"{"season_number": 2, "episodes": [{"episode_number": 1, "name": "The Dundies", "runtime": 22},
            {"episode_number": 2, "name": "Sexual Harassment"}]}"#;
        merge_season(&mut record, season_2, "season-2.json").unwrap();
        assert_eq!(record.seasons.iter().map(|season| season.season_number).collect::<Vec<u32>>(), vec![1, 2, 3]);
        assert_eq!(
            record.season(2),
            Some(&TmdbSeason {
                season_number: 2,
                episode_count: Some(2),
                episodes: vec![episode(1, "The Dundies", Some(22)), episode(2, "Sexual Harassment", None)],
            })
        );
        // A listed season without an episode count takes the count of the episodes merged.
        let mut record = TmdbRecord { seasons: vec![TmdbSeason { season_number: 3, episode_count: None, episodes: Vec::new() }], ..the_office() };
        merge_season(&mut record, SEASON_3, "season-3.json").unwrap();
        assert_eq!(record.season(3).and_then(|season| season.episode_count), Some(4));
    }

    #[test]
    fn a_season_answer_cannot_be_merged_into_a_movie_record_or_be_malformed() {
        let mut movie = pans_labyrinth();
        assert_eq!(
            merge_season(&mut movie, SEASON_3, "season-3.json"),
            Err("season-3.json: a season answer belongs to a TV record, not to movie 1417".to_string())
        );
        assert_eq!(movie, pans_labyrinth());
        let mut show = the_office();
        let no_number = merge_season(&mut show, r#"{"episodes": []}"#, "season.json").unwrap_err();
        assert!(no_number.starts_with("season.json: missing field `season_number`"), "{no_number}");
        assert_eq!(show, the_office());
    }

    #[test]
    fn episode_runtimes_are_one_seasons_or_every_regular_seasons() {
        let record = TmdbRecord {
            seasons: vec![
                TmdbSeason { season_number: 0, episode_count: Some(1), episodes: vec![episode(1, "The Accountants", Some(5))] },
                TmdbSeason {
                    season_number: 1,
                    episode_count: Some(2),
                    episodes: vec![episode(1, "Pilot", Some(23)), episode(2, "Diversity Day", Some(22))],
                },
                TmdbSeason {
                    season_number: 3,
                    episode_count: Some(3),
                    episodes: vec![
                        episode(12, "Traveling Salesmen", Some(22)),
                        episode(14, "Ben Franklin", None),
                        episode(13, "The Return", Some(21)),
                    ],
                },
            ],
            ..the_office()
        };
        // One season's runtimes in the answer's order, without the unknown ones.
        assert_eq!(record.episode_runtimes(Some(3)), vec![22, 21]);
        assert_eq!(record.episode_runtimes(Some(0)), vec![5]);
        assert_eq!(record.episode_runtimes(Some(2)), Vec::<u32>::new());
        // The whole series (no season parsed, spec 8.1): every regular season in order, without season 0's specials.
        assert_eq!(record.episode_runtimes(None), vec![23, 22, 22, 21]);
        assert_eq!(pans_labyrinth().episode_runtimes(None), Vec::<u32>::new());
    }

    #[test]
    fn work_takes_title_year_and_ids_from_the_same_record() {
        assert_eq!(
            pans_labyrinth().work(),
            Work {
                kind: TmdbKind::Movie,
                tmdb_id: 1417,
                title: "Pan's Labyrinth".to_string(),
                year: Some(2006),
                imdb_id: Some("tt0001417".to_string()),
                tvdb_id: None,
            }
        );
        assert_eq!(
            the_office().work(),
            Work {
                kind: TmdbKind::Tv,
                tmdb_id: 2316,
                title: "The Office".to_string(),
                year: Some(2005),
                imdb_id: Some("tt0002316".to_string()),
                tvdb_id: Some(102316),
            }
        );
        // Spec 8.9 names the file with the record's own title, never its original title.
        let undated = TmdbRecord { year: None, ..pans_labyrinth() };
        assert_eq!((undated.work().title.as_str(), undated.work().year), ("Pan's Labyrinth", None));
    }

    #[test]
    fn the_testsupport_builders_parse_through_the_same_parsers() {
        let text = tmdb_movie_json(1417, "Pan's Labyrinth", "El laberinto del fauno", "2006-10-11", "es", Some(119));
        let movie = parse_movie(&text, "movie.json").unwrap();
        assert_eq!(
            (movie.identity, movie.title.as_str(), movie.original_title.as_str(), movie.year),
            (TmdbIdentity { kind: TmdbKind::Movie, id: 1417 }, "Pan's Labyrinth", "El laberinto del fauno", Some(2006))
        );
        assert_eq!((movie.original_language.as_deref(), movie.runtime, movie.seasons.len()), (Some("es"), Some(119), 0));
        let text = tmdb_movie_json(1417, "Pan's Labyrinth", "El laberinto del fauno", "2006-10-11", "es", None);
        assert_eq!(parse_movie(&text, "movie.json").unwrap().runtime, None);

        let season_3: &[(u32, &str, Option<u32>)] = &[(12, "Traveling Salesmen", Some(22)), (13, "The Return", None)];
        let show = parse_tv(&tmdb_tv_json(2316, "The Office", "2005-03-24", "en", &[(3, season_3)]), "tv.json").unwrap();
        assert_eq!(
            (show.identity, show.title.as_str(), show.original_title.as_str(), show.year, show.original_language.as_deref()),
            (TmdbIdentity { kind: TmdbKind::Tv, id: 2316 }, "The Office", "The Office", Some(2005), Some("en"))
        );
        assert_eq!(
            show.season(3).map(|season| season.episodes.clone()),
            Some(vec![episode(12, "Traveling Salesmen", Some(22)), episode(13, "The Return", None)])
        );
        assert_eq!(show.episode_runtimes(Some(3)), vec![22]);

        let text = search_json(&[
            ("movie", 1417, "Pan's Labyrinth", "El laberinto del fauno", "2006-10-11"),
            ("tv", 2316, "The Office", "The Office", "2005-03-24"),
        ]);
        let search = parse_search(&text, "search.json").unwrap();
        let read: Vec<(Option<TmdbIdentity>, &str, &str, Option<i32>)> =
            search.results.iter().map(|result| (result.identity(), result.display_title(), result.original(), result.year())).collect();
        assert_eq!(
            read,
            vec![
                (Some(TmdbIdentity { kind: TmdbKind::Movie, id: 1417 }), "Pan's Labyrinth", "El laberinto del fauno", Some(2006)),
                (Some(TmdbIdentity { kind: TmdbKind::Tv, id: 2316 }), "The Office", "The Office", Some(2005)),
            ]
        );
    }

    /// A synthetic slim tmdb.json in the shape of `pans-labyrinth-2006-uhd/tmdb.json` (a top-level `imdb_id` too), with
    /// the synthetic IMDb ID of `pans_labyrinth()` and a made-up fetch date (ruling RP21: no recorded file is copied).
    const PANS_SLIM: &str = r#"{
  "id": 1417,
  "imdb_id": "tt0001417",
  "title": "Pan's Labyrinth",
  "original_title": "El laberinto del fauno",
  "release_date": "2006-10-11",
  "original_language": "es",
  "external_ids": {
    "imdb_id": "tt0001417",
    "wikidata_id": null
  },
  "fetched_at": "2026-01-02"
}"#;

    /// A synthetic slim tmdb.json in the shape of `the-office-2005-s3d3-bluray/tmdb.json`, with the synthetic IMDb and
    /// TVDB IDs of `the_office()` and a made-up fetch date (ruling RP21: no recorded file is copied).
    const OFFICE_SLIM: &str = r#"{
  "id": 2316,
  "name": "The Office",
  "original_name": "The Office",
  "first_air_date": "2005-03-24",
  "original_language": "en",
  "external_ids": {
    "imdb_id": "tt0002316",
    "tvdb_id": 102316,
    "wikidata_id": null
  },
  "fetched_at": "2026-01-02"
}"#;

    #[test]
    fn the_slim_tmdb_json_maps_to_a_movie_or_a_tv_record() {
        // A movie when it has a title, TV when it has a name; no runtime and no seasons; fetched_at is not read.
        assert_eq!(parse_recorded(PANS_SLIM, "tmdb.json"), Ok(TmdbRecord { runtime: None, ..pans_labyrinth() }));
        assert_eq!(
            parse_recorded(&format!("\u{feff}{OFFICE_SLIM}"), "tmdb.json"),
            Ok(TmdbRecord { seasons: Vec::new(), ..the_office() })
        );
    }

    #[test]
    fn a_slim_tmdb_json_without_an_id_or_without_title_and_name_is_refused() {
        assert_eq!(
            parse_recorded(r#"{"title": "Pan's Labyrinth", "original_language": "es"}"#, "tmdb.json"),
            Err("tmdb.json: the record has no id".to_string())
        );
        assert_eq!(
            parse_recorded(r#"{"id": 1417, "original_language": "es", "fetched_at": "2026-01-02"}"#, "tmdb.json"),
            Err("tmdb.json: the record has neither a title (a movie) nor a name (a TV show)".to_string())
        );
        // A value of another type reads as absent (recording::files::Tmdb is lenient): an id given as text is no id.
        assert_eq!(
            parse_recorded(r#"{"id": "1417", "title": "Pan's Labyrinth"}"#, "tmdb.json"),
            Err("tmdb.json: the record has no id".to_string())
        );
        let broken = parse_recorded("{", "tmdb.json").unwrap_err();
        assert!(broken.starts_with("tmdb.json: EOF while parsing"), "{broken}");
        // External IDs of another type give no IMDb or TVDB ID; a missing original name is the name.
        let record =
            parse_recorded(r#"{"id": 2316, "name": "The Office", "external_ids": {"imdb_id": 5, "tvdb_id": "102316"}}"#, "tmdb.json")
                .unwrap();
        assert_eq!(
            (record.identity, record.imdb_id, record.tvdb_id, record.original_title.as_str(), record.year),
            (TmdbIdentity { kind: TmdbKind::Tv, id: 2316 }, None, None, "The Office", None)
        );
        // A TVDB ID of 0 or less is no ID here too.
        assert_eq!(
            parse_recorded(r#"{"id": 2316, "name": "The Office", "external_ids": {"tvdb_id": -1}}"#, "tmdb.json").unwrap().tvdb_id,
            None
        );
    }
}
