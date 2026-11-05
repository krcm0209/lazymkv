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

/// A slim copy as saved: pretty JSON and a final newline (T16 rulings). Moved here from the chain so the refetch
/// writes the same bytes (design part 8.4).
pub(crate) fn json_file(name: &str, value: &Value) -> SavedFile {
    // Writing a `Value` into memory cannot fail: its map keys are strings and a Vec takes every byte.
    let mut bytes = serde_json::to_vec_pretty(value).unwrap_or_default();
    bytes.push(b'\n');
    SavedFile { name: name.to_string(), bytes }
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

/// A raw season answer spliced into a slim `tmdb-record.json` (design part 7.1: "for a TV record, its seasons and
/// episodes"; Ruling T4-R4; no season file is saved). The season whose `season_number` matches is rebuilt in place as
/// `season_number`, `episode_count` (kept when present, else the number of episodes), `episodes` (each
/// `episode_number` and `runtime` when present and not null, as `slim_record` keeps them); a season the file lacks is
/// inserted before the first season with a higher number. A file without a `seasons` array, or an answer without a
/// number, is left as it is. `fetched_at` stays last. Def: T4-4. Use: T4-4, T4-8.
pub fn splice_season(record_file: &mut Value, season_raw: &Value) {
    let Some(number) = season_raw.get("season_number").and_then(Value::as_u64) else { return };
    let Some(seasons) = record_file.get_mut("seasons").and_then(Value::as_array_mut) else { return };
    let number_of = |season: &Value| season.get("season_number").and_then(Value::as_u64);
    let episodes = season_raw.get("episodes").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
    let listed = seasons.iter().position(|season| number_of(season) == Some(number));
    let mut slim = Map::new();
    slim.insert("season_number".to_string(), Value::from(number));
    let count = listed
        .and_then(|index| seasons[index].get("episode_count").filter(|count| !count.is_null()).cloned())
        .unwrap_or_else(|| Value::from(episodes.len()));
    slim.insert("episode_count".to_string(), count);
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
    match listed {
        Some(index) => seasons[index] = Value::Object(slim),
        None => {
            let at = seasons
                .iter()
                .position(|season| number_of(season).is_some_and(|other| other > number))
                .unwrap_or(seasons.len());
            seasons.insert(at, Value::Object(slim));
        }
    }
}

/// The slim `tmdb-search.json` of one or more pages (design parts 7.1 and 8.4; S68, P29, J8): each page's movie and tv
/// results slimmed as before, in page order, where a result of a later page is left out when a kept result has its
/// `media_type` and `id` (so the results are `merge_pages`'s); then `queries`, each page's query in order; then
/// `fetched_at`. One page gives pull request 2's file byte for byte. Def: T4-2. Use: T4-2, T4-3, T4-8.
pub fn slim_search(pages: &[(&Value, &str)], today: &str) -> Value {
    let mut kept: Vec<(TmdbKind, Option<Value>)> = Vec::new();
    let mut results = Vec::new();
    for (index, (raw, _)) in pages.iter().enumerate() {
        let page = raw.get("results").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
        for result in page {
            let Some(kind) = result.get("media_type").and_then(Value::as_str).and_then(|kind| kind.parse().ok()) else {
                continue;
            };
            let key = (kind, result.get("id").cloned());
            if index > 0 && kept.contains(&key) {
                continue;
            }
            kept.push(key);
            // A slim result: `media_type`, `id`, and the kind's title, original title and date when present and not
            // null. A result of another media type was dropped above.
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
            results.push(Value::Object(slim));
        }
    }
    let queries = pages.iter().map(|(_, query)| Value::String(query.to_string())).collect();
    let mut page = Map::new();
    page.insert("results".to_string(), Value::Array(results));
    page.insert("queries".to_string(), Value::Array(queries));
    dated(page, today)
}

/// The text of `lookup-status.txt`: `<key> <status>` and a newline per entry, in key order (S21, S66). Design part 8.4.
/// Def: T15. Use: T16.
pub fn status_text(lines: &BTreeMap<String, String>) -> String {
    lines.iter().map(|(key, status)| format!("{key} {status}\n")).collect()
}

/// The shape record of design part 8.5 (P29, Ruling T4-R19): every scalar becomes its JSON type name ("string",
/// "number", "boolean", "null"); an object keeps its keys, in order, with their values' shapes; an array becomes a
/// one-element array of its elements' merged shape (`[]` when empty). Merging unites two objects' keys (first seen
/// first) key by key, merges two arrays' element shapes, and otherwise joins the type names with `|` in the order
/// object, array, string, number, boolean, null (`"number|null"`); an object or array meeting a scalar keeps only its
/// type name (`"object|null"`). No value of the answer survives. Def: T4-19. Use: T4-19.
pub fn shape_of(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(map.iter().map(|(key, value)| (key.clone(), shape_of(value))).collect()),
        Value::Array(items) => Value::Array(items.iter().map(shape_of).reduce(merge_shapes).into_iter().collect()),
        Value::String(_) => Value::from("string"),
        Value::Number(_) => Value::from("number"),
        Value::Bool(_) => Value::from("boolean"),
        Value::Null => Value::from("null"),
    }
}

/// The order a shape joins type names in (Ruling T4-R19).
const SHAPE_TYPES: [&str; 6] = ["object", "array", "string", "number", "boolean", "null"];

/// Two shapes as one, by `shape_of`'s merge rules (Ruling T4-R19).
fn merge_shapes(a: Value, b: Value) -> Value {
    match (a, b) {
        (Value::Object(mut a), Value::Object(b)) => {
            for (key, shape) in b {
                match a.get_mut(&key) {
                    // get_mut and take, never remove and insert: serde_json's preserve_order map would reorder keys.
                    Some(slot) => *slot = merge_shapes(std::mem::take(slot), shape),
                    None => drop(a.insert(key, shape)),
                }
            }
            Value::Object(a)
        }
        (Value::Array(a), Value::Array(b)) => Value::Array(a.into_iter().chain(b).reduce(merge_shapes).into_iter().collect()),
        (a, b) => {
            let names = |shape: &Value| -> Vec<String> {
                match shape {
                    Value::Object(_) => vec!["object".to_string()],
                    Value::Array(_) => vec!["array".to_string()],
                    other => other.as_str().unwrap_or_default().split('|').map(str::to_string).collect(),
                }
            };
            let (a, b) = (names(&a), names(&b));
            let joined: Vec<&str> = SHAPE_TYPES.into_iter().filter(|name| a.iter().chain(&b).any(|kept| kept == name)).collect();
            Value::from(joined.join("|"))
        }
    }
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
        let text = serde_json::to_string(&slim_search(&[(&raw, "SYNTHETIC MOVIE")], "2026-10-23")).unwrap();
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

    #[test]
    fn splice_season_adds_episodes_and_keeps_the_slim_order() {
        let tv = json!({"id": 5, "name": "Show", "original_name": "Show", "first_air_date": "2002-01-01",
            "original_language": "en", "seasons": [{"season_number": 0, "episode_count": 1}, {"season_number": 1},
            {"season_number": 3, "episode_count": 2}]});
        let mut file = slim_record(&tv, TmdbKind::Tv, "2026-10-23");
        splice_season(&mut file, &json!({"season_number": 1, "name": "S1", "episodes": [
            {"episode_number": 1, "name": "Pilot", "runtime": 44, "still_path": "/x.jpg"}, {"episode_number": 2, "runtime": null}]}));
        splice_season(&mut file, &json!({"season_number": 2, "episodes": [{"episode_number": 1, "runtime": 0}]}));
        let text = serde_json::to_string(&file).unwrap();
        assert_eq!(text, r#"{"id":5,"name":"Show","original_name":"Show","first_air_date":"2002-01-01","original_language":"en","kind":"tv","seasons":[{"season_number":0,"episode_count":1},{"season_number":1,"episode_count":2,"episodes":[{"episode_number":1,"runtime":44},{"episode_number":2}]},{"season_number":2,"episode_count":1,"episodes":[{"episode_number":1,"runtime":0}]},{"season_number":3,"episode_count":2}],"fetched_at":"2026-10-23"}"#);
        assert_eq!(parse_tv(&text, "tmdb-record.json").unwrap().episode_runtimes(None), vec![44]);
    }

    #[test]
    fn shape_records_hold_names_and_types_only() {
        let raw = json!({"id": 900001, "title": "Synthetic Movie", "adult": false, "imdb_id": null,
            "genres": [{"id": 28, "name": "Action"}, {"id": 53, "name": null, "extra": true}], "videos": [],
            "external_ids": {"imdb_id": "tt0900001", "tvdb_id": 7}, "mixed": [1, null, "x"],
            "maybe": [{"a": 1}, null], "nested": [[1], [2.5, "y"]]});
        let shape = shape_of(&raw);
        assert_eq!(shape, json!({"id": "number", "title": "string", "adult": "boolean", "imdb_id": "null",
            "genres": [{"id": "number", "name": "string|null", "extra": "boolean"}], "videos": [],
            "external_ids": {"imdb_id": "string", "tvdb_id": "number"}, "mixed": ["string|number|null"],
            "maybe": ["object|null"], "nested": [["string|number"]]}));
        let keys: Vec<&str> = shape.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys, ["id", "title", "adult", "imdb_id", "genres", "videos", "external_ids", "mixed", "maybe", "nested"]);
        let genre: Vec<&str> = shape["genres"][0].as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(genre, ["id", "name", "extra"]);
        let text = serde_json::to_string(&shape).unwrap();
        assert_eq!(text.find(|c: char| c.is_ascii_digit()), None, "{text}");
        for value in ["Synthetic Movie", "Action", "tt0900001", "\"x\"", "\"y\""] {
            assert!(!text.contains(value), "{value} in {text}");
        }
        assert_eq!(shape_of(&json!("x")), json!("string"));
    }
}
