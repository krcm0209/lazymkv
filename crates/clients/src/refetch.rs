//! The refetch of a stale TMDb copy (design part 8.4, R-B4; Ruling T4-R7): which of a folder's TMDb copies are past
//! the six-month limit or were deleted, the record its status file names, and the fetches that write each copy again
//! with today's date. The rule itself is core's `lookup::stale`; this module reads through a `read` closure and asks
//! through the transport, so the engine and dev share it. Used by Tasks 9, 10 and 11.

use lazymkv_core::lookup::stale::{fetched_at, is_stale, TMDB_COPIES};
use lazymkv_core::lookup::tmdb::{merge_parsed_season, TmdbIdentity, TmdbKind, TmdbRecord, TmdbSearch};
use serde_json::Value;

use crate::lookups::{tmdb_record, tmdb_search, tmdb_season, Fetched, Outcome, PendingReason};
use crate::merge::merge_pages;
use crate::saved::{json_file, slim_record, slim_search, slim_tmdb, splice_season, SavedFile};
use crate::transport::HttpTransport;

/// A TMDb copy to fetch again (design part 8.4, R-B4; Ruling T4-R7): one of `TMDB_COPIES`, with the `fetched_at` it
/// holds (None when it has none, cannot be read, or was deleted). Def: T4-8. Use: T4-9, T4-10, T4-11.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaleCopy {
    pub file: &'static str,
    pub fetched_at: Option<String>,
}

/// Whether `status_text` holds a line for `file` whose last word is `200`: the `tmdb <kind>/<id>` line for
/// `tmdb.json` and `tmdb-record.json`, any `search`, `search-tagged` or `search-entry` line for `tmdb-search.json`.
fn answered_200(status_text: &str, file: &str) -> bool {
    let keys: &[&str] =
        if file == "tmdb-search.json" { &["search", "search-tagged", "search-entry"] } else { &["tmdb"] };
    status_text.lines().any(|line| {
        let words: Vec<&str> = line.split_whitespace().collect();
        words.first().is_some_and(|key| keys.contains(key)) && words.last() == Some(&"200")
    })
}

/// The TMDb copies of a folder to fetch again, in `TMDB_COPIES` order (design part 8.4, R-B4; Ruling T4-R7). A copy
/// that `read` gives is listed when `is_stale` holds for its `fetched_at` (None when the bytes are not UTF-8) and,
/// outside a job folder, only when it has a date: a clone's copy without one is never refetched. A copy `read` does
/// not give is listed, with no date, only in a job folder and only when its status line in `status_text` says `200`
/// (it was answered and then deleted). `today` is the host's local date. Def: T4-8. Use: T4-9, T4-10, T4-11.
pub fn stale_copies(
    status_text: &str,
    read: &dyn Fn(&str) -> Option<Vec<u8>>,
    job_folder: bool,
    today: &str,
) -> Vec<StaleCopy> {
    let mut stale = Vec::new();
    for file in TMDB_COPIES {
        match read(file) {
            Some(bytes) => {
                let date = std::str::from_utf8(&bytes).ok().and_then(fetched_at);
                if is_stale(date.as_deref(), today) && (job_folder || date.is_some()) {
                    stale.push(StaleCopy { file, fetched_at: date });
                }
            }
            None => {
                if job_folder && answered_200(status_text, file) {
                    stale.push(StaleCopy { file, fetched_at: None });
                }
            }
        }
    }
    stale
}

/// The TMDb record `lookup-status.txt` names (design part 8.4): the first line whose first word is `tmdb`, its second
/// word read as `<kind>/<id>`; None when there is no such line or the word does not read. Def: T4-8. Use: T4-9, T4-10,
/// T4-11.
pub fn status_record(status_text: &str) -> Option<TmdbIdentity> {
    let line = status_text.lines().find(|line| line.split_whitespace().next() == Some("tmdb"))?;
    let (kind, id) = line.split_whitespace().nth(1)?.split_once('/')?;
    Some(TmdbIdentity { kind: kind.parse().ok()?, id: id.parse().ok()? })
}

/// An answered body as JSON, after any byte order mark. Core's parser already read it as JSON, so this gives Some.
fn body_json(fetched: &Fetched) -> Option<Value> {
    let text = std::str::from_utf8(&fetched.body).ok()?;
    serde_json::from_str(text.strip_prefix('\u{feff}').unwrap_or(text)).ok()
}

/// What a refetch did with a TV record's seasons (design part 8.4; Ruling T4-R4), each list in the order asked: the
/// seasons answered and merged, the seasons TMDb answered 404, the first season left pending with its reason (the
/// requests stop there), and the seasons after it, not asked. Def: T4-8 (PR #15 review round 1, R4). Use: T4-9,
/// T4-10.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SeasonsFetched {
    pub merged: Vec<u32>,
    pub missing: Vec<u32>,
    pub pending: Option<(u32, PendingReason)>,
    pub unasked: Vec<u32>,
}

/// A record fetched again (design part 8.4): the parsed record, the slim `tmdb.json` and `tmdb-record.json` in that
/// order, and what its seasons gave (`SeasonsFetched`, empty for a movie). Def: T4-8 (PR #15 review round 1, R4). Use:
/// T4-9, T4-10.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordRefetch {
    pub record: TmdbRecord,
    pub files: Vec<SavedFile>,
    pub seasons: SeasonsFetched,
}

/// The TV record `record`'s `seasons` fetched again in order (design part 8.4; Ruling T4-R4): each answered season
/// merged into `record` and spliced into its slim `file` (`splice_season`), a season answered 404 reported missing,
/// and the first pending season ending the requests, the seasons after it reported unasked. Def: T4-8 (PR #15 review
/// round 1, R4). Use: T4-8, T4-9.
pub fn refetch_seasons(
    t: &dyn HttpTransport,
    token: Option<&str>,
    record: &mut TmdbRecord,
    file: &mut Value,
    seasons: &[u32],
) -> SeasonsFetched {
    let mut fetched = SeasonsFetched::default();
    for (at, &n) in seasons.iter().enumerate() {
        match tmdb_season(t, token, record.identity.id, n) {
            Outcome::Answered { value, fetched: answer } => {
                // The record is a TV record, which takes every season, so the merge cannot refuse it.
                let merged = merge_parsed_season(record, value);
                debug_assert!(merged.is_ok(), "a TV record takes every season");
                if let Some(season_raw) = body_json(&answer) {
                    splice_season(file, &season_raw);
                }
                fetched.merged.push(n);
            }
            Outcome::Miss { .. } => fetched.missing.push(n),
            Outcome::Pending { reason, .. } => {
                fetched.pending = Some((n, reason));
                fetched.unasked = seasons[at + 1..].to_vec();
                break;
            }
        }
    }
    fetched
}

/// TMDb's record `id` fetched again (design part 8.4; Ruling T4-R4): a `RecordRefetch` with the parsed record and the
/// slim `tmdb.json` and `tmdb-record.json` dated `today`; for a TV record, the seasons of Ruling T4-R4 (`[n]` for a
/// parsed season `n`, else every season the record lists) fetched too with `refetch_seasons`, its `seasons` saying
/// which were merged, which TMDb answered 404, and the first pending one and those after it, not asked: the files
/// then hold the record and the seasons merged before it (PR #15 review round 1, R2). Err `Status(404)` when TMDb has
/// no such record, else the record's own pending reason (`NoToken` asks nothing). Def: T4-8. Use: T4-9, T4-10, T4-11.
pub fn refetch_record(
    t: &dyn HttpTransport,
    token: Option<&str>,
    id: TmdbIdentity,
    season: Option<u32>,
    today: &str,
) -> Result<RecordRefetch, PendingReason> {
    let (mut record, fetched) = match tmdb_record(t, token, id) {
        Outcome::Answered { value, fetched } => (value, fetched),
        Outcome::Miss { .. } => return Err(PendingReason::Status(404)),
        Outcome::Pending { reason, .. } => return Err(reason),
    };
    let raw = body_json(&fetched).unwrap_or_default();
    let mut file = slim_record(&raw, id.kind, today);
    let tmdb = json_file("tmdb.json", &slim_tmdb(&raw, today));
    let mut seasons = SeasonsFetched::default();
    if id.kind == TmdbKind::Tv {
        let asked: Vec<u32> = match season {
            Some(n) => vec![n],
            None => record.seasons.iter().map(|listed| listed.season_number).collect(),
        };
        seasons = refetch_seasons(t, token, &mut record, &mut file, &asked);
    }
    Ok(RecordRefetch { record, files: vec![tmdb, json_file("tmdb-record.json", &file)], seasons })
}

/// TMDb's search fetched again for each of `queries` in order (design part 8.4; J8, S47; `core::lookup::search_queries`
/// gives them): the pages folded through `merge_pages`, and the slim `tmdb-search.json` of every page with its query,
/// dated `today`. Err `Unparsed("no query to search")` for no query, `Status(404)` for a search TMDb answers 404, else
/// the first pending reason (`NoToken` asks nothing). Def: T4-8. Use: T4-9, T4-10, T4-11.
pub fn refetch_search(
    t: &dyn HttpTransport,
    token: Option<&str>,
    queries: &[String],
    today: &str,
) -> Result<(TmdbSearch, SavedFile), PendingReason> {
    if queries.is_empty() {
        return Err(PendingReason::Unparsed("no query to search".to_string()));
    }
    let mut pages = Vec::new();
    for query in queries {
        match tmdb_search(t, token, query) {
            Outcome::Answered { value, .. } => pages.push(value),
            Outcome::Miss { .. } => return Err(PendingReason::Status(404)),
            Outcome::Pending { reason, .. } => return Err(reason),
        }
    }
    // Serializing a parsed page cannot fail: its fields are strings, numbers and options.
    let values: Vec<Value> = pages.iter().map(|page| serde_json::to_value(page).unwrap_or_default()).collect();
    let slim: Vec<(&Value, &str)> = values.iter().zip(queries).map(|(page, query)| (page, query.as_str())).collect();
    let file = json_file("tmdb-search.json", &slim_search(&slim, today));
    let mut merged = pages.remove(0);
    for page in &pages {
        merged = merge_pages(&merged, page);
    }
    Ok((merged, file))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use lazymkv_core::lookup::tmdb::{TmdbIdentity, TmdbKind};
    use serde_json::{json, Value};

    use crate::lookups::{reason_text, record_url, search_url, season_url, PendingReason};
    use crate::recorded::RecordedTransport;
    use crate::saved::{json_file, slim_record, slim_tmdb};

    const TOKEN: &str = "token-sentinel-5d2e";
    const TODAY: &str = "2026-10-23";
    const MOVIE: TmdbIdentity = TmdbIdentity { kind: TmdbKind::Movie, id: 900001 };
    const MOVIE_BODY: &[u8] = br#"{"id":900001,"imdb_id":"tt0900001","title":"Synthetic Movie","original_title":"Synthetic Movie","release_date":"2001-02-03","original_language":"en","runtime":119,"external_ids":{"imdb_id":"tt0900001"}}"#;
    const SHOW: TmdbIdentity = TmdbIdentity { kind: TmdbKind::Tv, id: 1437 };
    /// A /tv/1437 answer listing seasons 0, 1 and 2, without their episodes.
    const SHOW_BODY: &[u8] = br#"{"id":1437,"name":"Firefly","original_name":"Firefly","first_air_date":"2002-09-20","original_language":"en","external_ids":{"imdb_id":"tt0303461","tvdb_id":78874},"seasons":[{"season_number":0,"episode_count":2},{"season_number":1,"episode_count":14},{"season_number":2,"episode_count":2}]}"#;

    /// A /tv/1437/season/<n> answer with episodes 1 (44 minutes) and 2 (43 minutes).
    fn season_body(n: u32) -> String {
        json!({"season_number": n, "episodes": [{"episode_number": 1, "name": "One", "runtime": 44},
                                                {"episode_number": 2, "name": "Two", "runtime": 43}]})
        .to_string()
    }

    /// A /search/multi first page of movies with these IDs (titles and dates made up).
    fn page(ids: &[i64]) -> String {
        let results: Vec<Value> = ids
            .iter()
            .map(|id| json!({"media_type": "movie", "id": id, "title": format!("T{id}"), "original_title": format!("T{id}"),
                             "release_date": "2010-08-20"}))
            .collect();
        json!({"page": 1, "results": results, "total_pages": 1, "total_results": ids.len()}).to_string()
    }

    /// A `read` closure over the files given as (name, text).
    fn files(entries: &[(&'static str, &'static str)]) -> impl Fn(&str) -> Option<Vec<u8>> {
        let map: BTreeMap<&str, &str> = entries.iter().copied().collect();
        move |name: &str| map.get(name).map(|text| text.as_bytes().to_vec())
    }

    #[test]
    fn a_copy_without_fetched_at_is_stale_only_in_a_job_folder() {
        let read = files(&[("tmdb.json", r#"{"original_language": "es"}"#)]);
        assert_eq!(stale_copies("", &read, false, TODAY), vec![]);
        assert_eq!(stale_copies("", &read, true, TODAY), vec![StaleCopy { file: "tmdb.json", fetched_at: None }]);
    }

    #[test]
    fn stale_copies_lists_old_copies_and_a_deleted_one_only_in_a_job_folder() {
        let status = "dischash 404\nsearch 200\ntmdb movie/1 200\n";
        let read = files(&[("tmdb.json", r#"{"fetched_at": "2026-01-01"}"#), ("tmdb-search.json", r#"{"fetched_at": "2026-10-01"}"#)]);
        let old = StaleCopy { file: "tmdb.json", fetched_at: Some("2026-01-01".to_string()) };
        assert_eq!(stale_copies(status, &read, false, TODAY), vec![old.clone()]);
        assert_eq!(stale_copies(status, &read, true, TODAY), vec![old.clone(), StaleCopy { file: "tmdb-record.json", fetched_at: None }]);
        assert_eq!(stale_copies("tmdb movie/1 404\nsearch 000\n", &files(&[]), true, TODAY), vec![], "never answered: not deleted");
    }

    #[test]
    fn status_record_reads_the_record_line() {
        assert_eq!(status_record("dischash 404\ntmdb tv/1437 200\n"), Some(SHOW));
        assert_eq!(status_record("tmdb movie/x 200\n"), None);
        assert_eq!(status_record("search 200\n"), None);
    }

    #[test]
    fn refetch_record_writes_both_slim_files_with_todays_date() {
        let t = RecordedTransport::new();
        t.answer(&record_url(MOVIE), 200, MOVIE_BODY);
        let RecordRefetch { record, files: saved, seasons } = refetch_record(&t, Some(TOKEN), MOVIE, None, TODAY).unwrap();
        assert_eq!(record.identity, MOVIE);
        assert_eq!(seasons, SeasonsFetched::default(), "a movie has no season");
        let raw: Value = serde_json::from_slice(MOVIE_BODY).unwrap();
        assert_eq!(saved, vec![json_file("tmdb.json", &slim_tmdb(&raw, TODAY)),
                               json_file("tmdb-record.json", &slim_record(&raw, TmdbKind::Movie, TODAY))]);
    }

    #[test]
    fn refetch_record_of_a_tv_show_fetches_its_seasons_again() {
        let t = RecordedTransport::new();
        t.answer(&record_url(SHOW), 200, SHOW_BODY);
        for n in [0, 1, 2] { t.answer(&season_url(1437, n), 200, season_body(n).as_bytes()); }
        let RecordRefetch { record, files: saved, seasons } = refetch_record(&t, Some(TOKEN), SHOW, None, TODAY).unwrap();
        let urls: Vec<String> = t.requests().into_iter().map(|(url, _)| url).collect();
        assert_eq!(urls, [record_url(SHOW), season_url(1437, 0), season_url(1437, 1), season_url(1437, 2)], "every listed season (T4-R4)");
        assert_eq!(seasons, SeasonsFetched { merged: vec![0, 1, 2], ..SeasonsFetched::default() });
        assert_eq!(record.season(2).unwrap().episodes.len(), 2);
        let file: Value = serde_json::from_slice(&saved[1].bytes).unwrap();
        assert_eq!(file["seasons"][2]["episodes"], json!([{"episode_number": 1, "runtime": 44}, {"episode_number": 2, "runtime": 43}]));
        let parsed = RecordedTransport::new();
        parsed.answer(&record_url(SHOW), 200, SHOW_BODY);
        parsed.answer(&season_url(1437, 2), 200, season_body(2).as_bytes());
        refetch_record(&parsed, Some(TOKEN), SHOW, Some(2), TODAY).unwrap();
        assert_eq!(parsed.requests().len(), 2, "a parsed season asks that season only");
    }

    /// PR #15 review round 1, R4: a season TMDb answers 404 is reported missing, not merged, and the seasons after it
    /// are still asked.
    #[test]
    fn a_season_answered_404_is_reported_missing() {
        let t = RecordedTransport::new();
        t.answer(&record_url(SHOW), 200, SHOW_BODY);
        t.answer(&season_url(1437, 0), 200, season_body(0).as_bytes());
        t.answer(&season_url(1437, 1), 404, b"{}");
        t.answer(&season_url(1437, 2), 200, season_body(2).as_bytes());
        let refetched = refetch_record(&t, Some(TOKEN), SHOW, None, TODAY).unwrap();
        assert_eq!(refetched.seasons, SeasonsFetched { merged: vec![0, 2], missing: vec![1], ..SeasonsFetched::default() });
        assert!(refetched.record.season(1).unwrap().episodes.is_empty());
        let file: Value = serde_json::from_slice(&refetched.files[1].bytes).unwrap();
        assert_eq!(file["seasons"][1].get("episodes"), None, "a missing season is not spliced");
    }

    /// SHOW with season 0 answered, season 1 answered 429 and season 2 recorded but never reached.
    fn season_1_pending() -> RecordedTransport {
        let t = RecordedTransport::new();
        t.answer(&record_url(SHOW), 200, SHOW_BODY);
        t.answer(&season_url(1437, 0), 200, season_body(0).as_bytes());
        t.answer(&season_url(1437, 1), 429, b"{}");
        t.answer(&season_url(1437, 2), 200, season_body(2).as_bytes());
        t
    }

    /// PR #15 review round 1, R2: a pending season keeps the record, its files and the season merged before it.
    #[test]
    fn a_pending_season_keeps_the_record_and_the_seasons_before_it() {
        let t = season_1_pending();
        let refetched = refetch_record(&t, Some(TOKEN), SHOW, None, TODAY).unwrap();
        assert_eq!(refetched.seasons, SeasonsFetched {
            merged: vec![0],
            missing: vec![],
            pending: Some((1, PendingReason::Status(429))),
            unasked: vec![2],
        });
        assert_eq!(refetched.record.season(0).unwrap().episodes.len(), 2);
        assert_eq!(refetched.files.iter().map(|file| file.name.as_str()).collect::<Vec<_>>(), ["tmdb.json", "tmdb-record.json"]);
        let file: Value = serde_json::from_slice(&refetched.files[1].bytes).unwrap();
        assert_eq!(file["seasons"][0]["episodes"].as_array().map(Vec::len), Some(2));
        assert_eq!(file["fetched_at"], json!(TODAY));
    }

    /// PR #15 review round 1, R2 (the owner's ruling): the requests stop at the first pending season.
    #[test]
    fn a_pending_season_stops_the_season_requests_and_leaves_the_rest_unasked() {
        let t = season_1_pending();
        refetch_record(&t, Some(TOKEN), SHOW, None, TODAY).unwrap();
        let urls: Vec<String> = t.requests().into_iter().map(|(url, _)| url).collect();
        assert_eq!(urls, [record_url(SHOW), season_url(1437, 0), season_url(1437, 1)]);
    }

    /// PR #15 review round 1, R2: `refetch_seasons` asks the seasons it is given and splices only those.
    #[test]
    fn refetch_seasons_splices_only_the_seasons_it_fetched() {
        let raw: Value = serde_json::from_slice(SHOW_BODY).unwrap();
        let mut record = lazymkv_core::lookup::tmdb::parse_tv(std::str::from_utf8(SHOW_BODY).unwrap(), "tv-1437").unwrap();
        let mut file = slim_record(&raw, TmdbKind::Tv, "2026-10-01");
        let before = file.clone();
        let t = RecordedTransport::new();
        t.answer(&season_url(1437, 2), 200, season_body(2).as_bytes());
        let fetched = refetch_seasons(&t, Some(TOKEN), &mut record, &mut file, &[2]);
        assert_eq!(fetched, SeasonsFetched { merged: vec![2], ..SeasonsFetched::default() });
        assert_eq!(t.requests().len(), 1);
        assert_eq!((&file["seasons"][0], &file["seasons"][1]), (&before["seasons"][0], &before["seasons"][1]));
        assert_eq!(file["seasons"][2]["episodes"].as_array().map(Vec::len), Some(2));
        assert_eq!(file["fetched_at"], json!("2026-10-01"), "the record's date stays");
        assert_eq!(record.season(2).unwrap().episodes.len(), 2);
        assert!(record.season(1).unwrap().episodes.is_empty());
    }

    #[test]
    fn refetch_search_runs_each_query_and_merges_them() {
        let t = RecordedTransport::new();
        t.answer(&search_url("PIRANHA"), 200, page(&[1, 2]).as_bytes());
        t.answer(&search_url("PIRANHA 3D"), 200, page(&[2, 3]).as_bytes());
        let queries = ["PIRANHA".to_string(), "PIRANHA 3D".to_string()];
        let (search, saved) = refetch_search(&t, Some(TOKEN), &queries, TODAY).unwrap();
        assert_eq!(search.results.iter().map(|r| r.id).collect::<Vec<_>>(), [1, 2, 3]);
        let file: Value = serde_json::from_slice(&saved.bytes).unwrap();
        assert_eq!((saved.name.as_str(), &file["queries"], &file["fetched_at"]), ("tmdb-search.json", &json!(queries), &json!(TODAY)));
        assert_eq!(refetch_search(&t, Some(TOKEN), &[], TODAY).unwrap_err(), PendingReason::Unparsed("no query to search".to_string()));
    }

    #[test]
    fn a_refetch_without_a_token_asks_nothing_and_says_so() {
        let t = RecordedTransport::new();
        assert_eq!(refetch_record(&t, None, MOVIE, None, TODAY).unwrap_err(), PendingReason::NoToken);
        assert_eq!(refetch_search(&t, None, &["X".to_string()], TODAY).unwrap_err(), PendingReason::NoToken);
        assert!(t.requests().is_empty());
    }

    #[test]
    fn a_refetch_answered_404_or_401_is_an_error_whose_text_holds_no_token() {
        let t = RecordedTransport::new();
        t.answer(&record_url(MOVIE), 404, b"");
        assert_eq!(refetch_record(&t, Some(TOKEN), MOVIE, None, TODAY).unwrap_err(), PendingReason::Status(404));
        t.answer(&search_url("X"), 401, b"{}");
        let refused = refetch_search(&t, Some(TOKEN), &["X".to_string()], TODAY).unwrap_err();
        assert_eq!(reason_text(&refused), "TMDb refused the token (HTTP 401)");
    }
}
