//! What the engine saves from the lookups (design parts 7.1 and 8.4): the slim copies of TMDb's answers, in phase 0's
//! shape (P29: no full TMDb answer is ever kept), and the `lookup-status.txt` text. Dev reads that file's first word
//! of a line as the key and its last word as the status, and parses an answer file only when its line says `200` or
//! is absent. Any word other than `200` and `404` reads as failed, so `000` (no answer) leaves the lookup pending
//! (S66). Used by Tasks 16 and 24.

use std::collections::BTreeMap;

use lazymkv_core::lookup::tmdb::TmdbKind;
use serde_json::{Map, Value};

/// A file the lookups ask the engine to save in the job's folder: its name there and its bytes. Design part 8.4.
/// Def: T15. Use: T16, T24.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedFile {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// Phase 0's slim keys, in phase 0's order (scripts/phase0/record-disc.ps1).
const SLIM_FIELDS: [&str; 9] = [
    "id",
    "imdb_id",
    "title",
    "original_title",
    "release_date",
    "name",
    "original_name",
    "first_air_date",
    "original_language",
];

/// The external IDs phase 0 keeps, in its order.
const EXTERNAL_ID_FIELDS: [&str; 3] = ["imdb_id", "tvdb_id", "wikidata_id"];

/// Copies `field` of `raw` into `into` when it is present and not null.
fn keep(into: &mut Map<String, Value>, raw: &Value, field: &str) {
    if let Some(value) = raw.get(field).filter(|value| !value.is_null()) {
        into.insert(field.to_string(), value.clone());
    }
}

/// PowerShell's `if ($value)` for the values an ID can take: null, `""`, `0` and `false` are false.
fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_f64() != Some(0.0),
        Value::String(text) => !text.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

fn dated(mut slim: Map<String, Value>, today: &str) -> Value {
    slim.insert("fetched_at".to_string(), Value::String(today.to_string()));
    Value::Object(slim)
}

/// Phase 0's fields without `fetched_at`: the nine keys when present and not null, then `external_ids` when one of its
/// three IDs is set.
fn phase_0_fields(raw: &Value) -> Map<String, Value> {
    let mut slim = Map::new();
    for field in SLIM_FIELDS {
        keep(&mut slim, raw, field);
    }
    let mut ids = Map::new();
    for field in EXTERNAL_ID_FIELDS {
        if let Some(id) = raw.get("external_ids").and_then(|external| external.get(field)).filter(|id| truthy(id)) {
            ids.insert(field.to_string(), id.clone());
        }
    }
    if !ids.is_empty() {
        slim.insert("external_ids".to_string(), Value::Object(ids));
    }
    slim
}

/// The slim `tmdb.json`: phase 0's keys in phase 0's order, `external_ids` when one ID is set, and `fetched_at` (the
/// day of the fetch, which starts TMDb's six-month limit) last. Design parts 7.1 and 8.4 (S68, P29). Def: T15.
/// Use: T16.
pub fn slim_tmdb(raw: &Value, today: &str) -> Value {
    dated(phase_0_fields(raw), today)
}

fn slim_season(season: &Value) -> Value {
    let mut slim = Map::new();
    keep(&mut slim, season, "season_number");
    keep(&mut slim, season, "episode_count");
    if let Some(episodes) = season.get("episodes").and_then(Value::as_array) {
        let episodes = episodes
            .iter()
            .map(|episode| {
                let mut slim = Map::new();
                keep(&mut slim, episode, "episode_number");
                keep(&mut slim, episode, "runtime");
                Value::Object(slim)
            })
            .collect();
        slim.insert("episodes".to_string(), Value::Array(episodes));
    }
    Value::Object(slim)
}

/// The slim `tmdb-record.json`: `slim_tmdb`'s keys, then `kind`, then `runtime` when present and not null. A tv record
/// then gets `seasons`: each season's `season_number`, its `episode_count` when present and, when the raw season has
/// `episodes`, each episode's `episode_number` and `runtime`. `fetched_at` comes last. Design parts 7.1 and 8.4 (S68,
/// P29). Def: T15. Use: T16.
pub fn slim_record(raw: &Value, kind: TmdbKind, today: &str) -> Value {
    let mut slim = phase_0_fields(raw);
    slim.insert("kind".to_string(), Value::String(kind.as_str().to_string()));
    keep(&mut slim, raw, "runtime");
    if kind == TmdbKind::Tv {
        let seasons = raw.get("seasons").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
        slim.insert("seasons".to_string(), Value::Array(seasons.iter().map(slim_season).collect()));
    }
    dated(slim, today)
}

/// The slim `tmdb-search.json`: only the results whose `media_type` is `movie` or `tv`, each with `media_type`, `id`
/// and the kind's title, original title and date (`title`, `original_title`, `release_date` for a movie; `name`,
/// `original_name`, `first_air_date` for tv) when present and not null. The page then gets `queries` (the one query
/// searched) and `fetched_at`. Design parts 7.1 and 8.4 (S68, P29). Def: T15. Use: T16.
pub fn slim_search(raw: &Value, query: &str, today: &str) -> Value {
    let results = raw.get("results").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
    let results = results
        .iter()
        .filter_map(|result| {
            let kind: TmdbKind = result.get("media_type").and_then(Value::as_str)?.parse().ok()?;
            let fields = match kind {
                TmdbKind::Movie => ["title", "original_title", "release_date"],
                TmdbKind::Tv => ["name", "original_name", "first_air_date"],
            };
            let mut slim = Map::new();
            slim.insert("media_type".to_string(), Value::String(kind.as_str().to_string()));
            keep(&mut slim, result, "id");
            for field in fields {
                keep(&mut slim, result, field);
            }
            Some(Value::Object(slim))
        })
        .collect();
    let mut page = Map::new();
    page.insert("results".to_string(), Value::Array(results));
    page.insert("queries".to_string(), Value::Array(vec![Value::String(query.to_string())]));
    dated(page, today)
}

/// The text of `lookup-status.txt`: `<key> <status>` and a newline per entry, in key order (S21, S66). Design part 8.4.
/// Def: T15. Use: T16.
pub fn status_text(lines: &BTreeMap<String, String>) -> String {
    lines.iter().map(|(key, status)| format!("{key} {status}\n")).collect()
}

// PF-17: only the tests below call this. Task 16's chain writes each status word from its `LookupState`, so the
// non-test build has no caller and the function exists only in the test build.
/// The status word S66 gives an outcome: the HTTP status for `Answered`, `Miss` and a `Pending` that has a fetched
/// answer; `unparsed` for a `Pending` with `Unparsed`; `000` for a `Pending` with no fetch, `NoToken` included.
#[cfg(test)]
pub(crate) fn status_word<T>(outcome: &crate::lookups::Outcome<T>) -> String {
    use crate::lookups::{Outcome, PendingReason};
    match outcome {
        Outcome::Answered { fetched, .. } | Outcome::Miss { fetched } => fetched.status.to_string(),
        Outcome::Pending { reason: PendingReason::Unparsed(_), .. } => "unparsed".to_string(),
        Outcome::Pending { fetched: Some(fetched), .. } => fetched.status.to_string(),
        Outcome::Pending { fetched: None, .. } => "000".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lookups::{discdb_by_disc_id, discdb_by_fingerprint, discid_url, dischash_url, record_url, tmdb_record, tmdb_search};
    use crate::recorded::RecordedTransport;
    use crate::transport::TransportError;
    use lazymkv_core::lookup::tmdb::{parse_movie, parse_recorded, parse_search, parse_tv, TmdbIdentity, TmdbKind};
    use serde_json::json;

    fn movie_raw() -> serde_json::Value {
        json!({"adult": false, "id": 900001, "imdb_id": "tt0900001", "original_language": "en",
            "original_title": "Synthetic Movie", "overview": "synthetic", "release_date": "2001-02-03", "runtime": 119,
            "title": "Synthetic Movie",
            "external_ids": {"imdb_id": "tt0900001", "wikidata_id": "Q900001", "facebook_id": null, "tvdb_id": null}})
    }

    #[test]
    fn slim_tmdb_keeps_phase_0s_keys_in_order_with_fetched_at() {
        let slim = slim_tmdb(&movie_raw(), "2026-10-23");
        let text = serde_json::to_string(&slim).unwrap();
        assert_eq!(text, r#"{"id":900001,"imdb_id":"tt0900001","title":"Synthetic Movie","original_title":"Synthetic Movie","release_date":"2001-02-03","original_language":"en","external_ids":{"imdb_id":"tt0900001","wikidata_id":"Q900001"},"fetched_at":"2026-10-23"}"#);
        let record = parse_recorded(&text, "tmdb.json").unwrap();
        assert_eq!((record.title.as_str(), record.year), ("Synthetic Movie", Some(2001)));
    }

    #[test]
    fn slim_record_adds_kind_and_runtime_and_parses_back() {
        let text = serde_json::to_string(&slim_record(&movie_raw(), TmdbKind::Movie, "2026-10-23")).unwrap();
        assert_eq!(text, r#"{"id":900001,"imdb_id":"tt0900001","title":"Synthetic Movie","original_title":"Synthetic Movie","release_date":"2001-02-03","original_language":"en","external_ids":{"imdb_id":"tt0900001","wikidata_id":"Q900001"},"kind":"movie","runtime":119,"fetched_at":"2026-10-23"}"#);
        assert_eq!(parse_movie(&text, "tmdb-record.json").unwrap().runtime, Some(119));
        let tv = json!({"id": 5, "name": "Show", "original_name": "Show", "first_air_date": "2002-01-01",
            "original_language": "en", "seasons": [{"season_number": 1, "episode_count": 2, "name": "S1", "poster_path": "x"}]});
        let tv_text = serde_json::to_string(&slim_record(&tv, TmdbKind::Tv, "2026-10-23")).unwrap();
        assert_eq!(tv_text, r#"{"id":5,"name":"Show","original_name":"Show","first_air_date":"2002-01-01","original_language":"en","kind":"tv","seasons":[{"season_number":1,"episode_count":2}],"fetched_at":"2026-10-23"}"#);
        assert_eq!(parse_tv(&tv_text, "tmdb-record.json").unwrap().seasons.len(), 1);
    }

    #[test]
    fn slim_search_keeps_movie_and_tv_results_with_queries() {
        let raw = json!({"page": 1, "total_results": 3, "results": [
            {"media_type": "movie", "id": 900001, "title": "Synthetic Movie", "original_title": "Synthetic Movie",
             "release_date": "2001-02-03", "popularity": 1.5},
            {"media_type": "person", "id": 7, "name": "Someone"},
            {"media_type": "tv", "id": 5, "name": "Show", "original_name": "Show", "first_air_date": "2002-01-01"}]});
        let text = serde_json::to_string(&slim_search(&raw, "SYNTHETIC MOVIE", "2026-10-23")).unwrap();
        assert_eq!(text, r#"{"results":[{"media_type":"movie","id":900001,"title":"Synthetic Movie","original_title":"Synthetic Movie","release_date":"2001-02-03"},{"media_type":"tv","id":5,"name":"Show","original_name":"Show","first_air_date":"2002-01-01"}],"queries":["SYNTHETIC MOVIE"],"fetched_at":"2026-10-23"}"#);
        assert_eq!(parse_search(&text, "tmdb-search.json").unwrap().results.len(), 2);
    }

    #[test]
    fn the_status_file_writes_000_for_no_answer() {
        let movie = TmdbIdentity { kind: TmdbKind::Movie, id: 900001 };
        let t = RecordedTransport::new();
        t.fail(&dischash_url("ABC"), TransportError::Timeout);
        t.answer(&discid_url("D1"), 404, b"");
        t.answer(&record_url(movie), 200, b"{}");
        let mut lines = BTreeMap::new();
        lines.insert("dischash".to_string(), status_word(&discdb_by_fingerprint(&t, "ABC")));
        lines.insert("discid".to_string(), status_word(&discdb_by_disc_id(&t, "D1")));
        lines.insert("search".to_string(), status_word(&tmdb_search(&t, None, "SYNTHETIC MOVIE")));
        lines.insert("tmdb movie/900001".to_string(), status_word(&tmdb_record(&t, Some("token"), movie)));
        assert_eq!(status_text(&lines), "dischash 000\ndiscid 404\nsearch 000\ntmdb movie/900001 unparsed\n");
    }

}
