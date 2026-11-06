//! The live shape checks of design part 8.5 (ENG-20, Ruling T4-R19). The `#[ignore]` tests run only on the mini PC:
//! `cargo test -p lazymkv-clients --locked --test live -- --ignored --test-threads=1`, with `TMDB_TOKEN` and
//! `LAZYMKV_EVIDENCE` naming the evidence mirror. The others run everywhere; the replay skips without the clone.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use lazymkv_clients::{discdb_by_disc_id, discdb_by_fingerprint, merge_pages, reason_text, shape_of, slim_record,
    slim_search, splice_season, tmdb_record, tmdb_search, tmdb_season, Outcome, UreqTransport};
use lazymkv_core::lookup::discdb::{parse_discid, parse_dischash};
use lazymkv_core::lookup::name::{clean, fold_name, plausible, tagged};
use lazymkv_core::lookup::stale::{civil_date, fetched_at, is_stale};
use lazymkv_core::lookup::tmdb::{merge_parsed_season, parse_movie, parse_search, parse_tv, TmdbIdentity, TmdbKind};
use serde_json::Value;

/// John Wick's 4K disc, from `corpus/thediscdb-api` (public, RP21).
const FINGERPRINT: &str = "76E63EBB5B66F73B0C37BFAB59B8ABA5";
const DISC_ID: &str = "8D0355F530C31D44912831F8637FF65ACBCD6886";
/// A Firefly disc ID of `corpus/thediscdb-api`, whose answer names the show.
const FIREFLY_DISC_ID: &str = "66C36172D7981A2845308045DFBD9DE1";
const MOVIE: TmdbIdentity = TmdbIdentity { kind: TmdbKind::Movie, id: 245891 };
const SHOW: TmdbIdentity = TmdbIdentity { kind: TmdbKind::Tv, id: 1437 };
const SEASON: u32 = 1;
/// Decision-core H1's public disc name, whose tagged and cleaned forms differ (J8).
const NAME: &str = "PIRANHA_3D";

/// Writes `skipped: <reason>` and a newline straight to std::io::stderr() (past the harness's capture), as
/// `lazymkv_dev::evidence::skip` does; `clients` has no `dev` edge.
fn skip(reason: &str) {
    let _ = std::io::stderr().write_all(format!("skipped: {reason}\n").as_bytes());
}

/// `yyyy-MM-dd` (UTC) of a count of seconds since 1970-01-01: `civil_date(seconds)` formatted `{:04}-{:02}-{:02}`.
fn utc_date(seconds: u64) -> String {
    let (year, month, day) = civil_date(seconds);
    format!("{year:04}-{month:02}-{day:02}")
}

/// `utc_date` of `SystemTime::now()` (Ruling T4-R19: the records and their `fetched_at` carry the UTC date).
fn today() -> String {
    utc_date(SystemTime::now().duration_since(UNIX_EPOCH).map(|elapsed| elapsed.as_secs()).unwrap_or(0))
}

/// `<LAZYMKV_EVIDENCE>/live-lookups/<today>/`, created with `create_dir_all`. None after
/// `skip("LAZYMKV_EVIDENCE is not set")` when the variable is unset or empty, and after
/// `skip("<root> is not the evidence mirror (no folder, or it holds .git; design part 8.5)")` when it names no folder
/// or a folder holding `.git`: the records go to the mirror only, never to a clone or this repository.
fn records_dir(today: &str) -> Option<PathBuf> {
    let Some(root) = std::env::var_os("LAZYMKV_EVIDENCE").filter(|value| !value.is_empty()).map(PathBuf::from) else {
        skip("LAZYMKV_EVIDENCE is not set");
        return None;
    };
    if !root.is_dir() || root.ancestors().any(|folder| folder.join(".git").exists()) {
        skip(&format!("{} is not the evidence mirror (no folder, or it holds .git; design part 8.5)", root.display()));
        return None;
    }
    let dir = root.join("live-lookups").join(today);
    std::fs::create_dir_all(&dir).unwrap_or_else(|error| panic!("{}: {error}", dir.display()));
    Some(dir)
}

/// `TMDB_TOKEN` when set and not empty; else None after `skip("TMDB_TOKEN is not set")`.
fn token() -> Option<String> {
    let token = std::env::var("TMDB_TOKEN").ok().filter(|token| !token.is_empty());
    if token.is_none() {
        skip("TMDB_TOKEN is not set");
    }
    token
}

/// The value and the body of `Outcome::Answered`; a `Miss` panics `<what>: no such record (HTTP <status>)`, a
/// `Pending` panics `<what>: <reason_text>`, words that never hold the token (Task 7).
fn answered<T>(outcome: Outcome<T>, what: &str) -> (T, Vec<u8>) {
    match outcome {
        Outcome::Answered { value, fetched } => (value, fetched.body),
        Outcome::Miss { fetched } => panic!("{what}: no such record (HTTP {})", fetched.status),
        Outcome::Pending { reason, .. } => panic!("{what}: {}", reason_text(&reason)),
    }
}

/// `serde_json::from_slice(body)`, panicking `a parsed answer is JSON`.
fn json_of(body: &[u8]) -> Value {
    serde_json::from_slice(body).expect("a parsed answer is JSON")
}

/// Panics `<what>: no <pointer>` when `value.pointer(pointer)` is None, and `<what>: <pointer> is <kind>, not
/// <types>` when its kind ("object", "array", or `shape_of`'s scalar name) is not in `types`.
fn field(value: &Value, pointer: &str, types: &[&str], what: &str) {
    let Some(found) = value.pointer(pointer) else { panic!("{what}: no {pointer}") };
    let kind = match found {
        Value::Object(_) => "object".to_string(),
        Value::Array(_) => "array".to_string(),
        scalar => shape_of(scalar).as_str().unwrap_or_default().to_string(),
    };
    assert!(types.contains(&kind.as_str()), "{what}: {pointer} is {kind}, not {}", types.join(" or "));
}

/// Asserts the bytes hold no window equal to the token (`<name> would hold the token`, the token itself never
/// printed), then `std::fs::write(dir.join(name), bytes)`.
fn save(dir: &Path, name: &str, bytes: &[u8], token: Option<&str>) {
    if let Some(token) = token.filter(|token| !token.is_empty()) {
        assert!(!bytes.windows(token.len()).any(|window| window == token.as_bytes()), "{name} would hold the token");
    }
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
}

/// `serde_json::to_vec_pretty(value)` and a final `\n`.
fn pretty(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(value).expect("a Value is written as JSON");
    bytes.push(b'\n');
    bytes
}

#[test]
#[ignore = "live: mini PC only, TMDB_TOKEN and LAZYMKV_EVIDENCE (design part 8.5)"]
fn live_dischash_and_discid_parse_and_are_saved_as_received() {
    let Some(dir) = records_dir(&today()) else { return };
    let t = UreqTransport::new();
    let (record, body) = answered(discdb_by_fingerprint(&t, FINGERPRINT), "dischash");
    assert_eq!(record.tmdb_identity(), Some(MOVIE));
    save(&dir, &format!("thediscdb-dischash-{FINGERPRINT}.json"), &body, None);
    let (records, body) = answered(discdb_by_disc_id(&t, DISC_ID), "discid");
    assert!(records.iter().any(|record| record.tmdb_identity() == Some(MOVIE)), "discid names John Wick");
    save(&dir, &format!("thediscdb-discid-{DISC_ID}.json"), &body, None);
}

#[test]
#[ignore = "live: mini PC only, TMDB_TOKEN and LAZYMKV_EVIDENCE (design part 8.5)"]
fn live_search_runs_both_j8_queries_and_merges_them() {
    let today = today();
    let Some(dir) = records_dir(&today) else { return };
    let Some(token) = token() else { return };
    let (cleaned, tagged) = (clean(NAME), tagged(NAME));
    assert_eq!((cleaned.as_str(), tagged.as_str()), ("PIRANHA", "PIRANHA 3D"));
    assert_ne!(fold_name(&cleaned), fold_name(&tagged), "J8 asks twice only when the forms differ");
    let t = UreqTransport::new();
    let (cleaned_page, cleaned_body) = answered(tmdb_search(&t, Some(&token), &cleaned), "the cleaned search");
    let (tagged_page, tagged_body) = answered(tmdb_search(&t, Some(&token), &tagged), "the tagged search");
    let raws = [json_of(&cleaned_body), json_of(&tagged_body)];
    for (page, what) in raws.iter().zip(["the cleaned search", "the tagged search"]) {
        for (at, result) in page["results"].as_array().expect("results").iter().enumerate() {
            let fields = match result["media_type"].as_str() {
                Some("movie") => ["title", "original_title", "release_date"],
                Some("tv") => ["name", "original_name", "first_air_date"],
                _ => continue,
            };
            field(result, "/id", &["number"], what);
            for name in fields {
                field(result, &format!("/{name}"), &["string"], &format!("{what}, result {at}"));
            }
        }
    }
    let merged = merge_pages(&cleaned_page, &tagged_page);
    assert!(merged.results.iter().any(|result| plausible(result, &tagged)), "H1: the tagged search finds Piranha 3D");
    let slim = slim_search(&[(&raws[0], cleaned.as_str()), (&raws[1], tagged.as_str())], &today);
    assert_eq!(slim["queries"], serde_json::json!([cleaned, tagged]));
    assert_eq!(parse_search(&slim.to_string(), "the slim page").unwrap(), merged);
    save(&dir, "tmdb-search-PIRANHA_3D.slim.json", &pretty(&slim), Some(&token));
    let shape = shape_of(&Value::Array(raws.to_vec()))[0].clone(); // both pages' merged shape
    save(&dir, "tmdb-search-PIRANHA_3D.shape.json", &pretty(&shape), Some(&token));
}

#[test]
#[ignore = "live: mini PC only, TMDB_TOKEN and LAZYMKV_EVIDENCE (design part 8.5)"]
fn live_movie_record_has_every_field_core_reads() {
    let today = today();
    let Some(dir) = records_dir(&today) else { return };
    let Some(token) = token() else { return };
    let (record, body) = answered(tmdb_record(&UreqTransport::new(), Some(&token), MOVIE), "the movie record");
    let raw = json_of(&body);
    for pointer in ["/title", "/original_title", "/release_date", "/original_language", "/external_ids/imdb_id"] {
        field(&raw, pointer, &["string"], "the movie record");
    }
    field(&raw, "/id", &["number"], "the movie record");
    field(&raw, "/runtime", &["number"], "the movie record");
    assert_eq!((record.identity, record.year), (MOVIE, Some(2014)));
    assert!(record.runtime.is_some() && record.imdb_id.is_some() && record.original_language.is_some());
    let slim = slim_record(&raw, TmdbKind::Movie, &today);
    assert_eq!(parse_movie(&slim.to_string(), "the slim record").unwrap(), record, "the slim record keeps what core reads");
    save(&dir, "tmdb-movie-245891.slim.json", &pretty(&slim), Some(&token));
    save(&dir, "tmdb-movie-245891.shape.json", &pretty(&shape_of(&raw)), Some(&token));
}

#[test]
#[ignore = "live: mini PC only, TMDB_TOKEN and LAZYMKV_EVIDENCE (design part 8.5)"]
fn live_tv_record_and_season_have_every_field_core_reads() {
    let today = today();
    let Some(dir) = records_dir(&today) else { return };
    let Some(token) = token() else { return };
    let t = UreqTransport::new();
    let (mut record, body) = answered(tmdb_record(&t, Some(&token), SHOW), "the tv record");
    let raw = json_of(&body);
    for pointer in ["/name", "/original_name", "/first_air_date", "/original_language"] {
        field(&raw, pointer, &["string"], "the tv record");
    }
    field(&raw, "/id", &["number"], "the tv record");
    field(&raw, "/external_ids/tvdb_id", &["number", "null"], "the tv record");
    for (at, season) in raw["seasons"].as_array().expect("seasons").iter().enumerate() {
        field(season, "/season_number", &["number"], &format!("the tv record, season {at}"));
        field(season, "/episode_count", &["number"], &format!("the tv record, season {at}"));
    }
    assert_eq!((record.identity, record.year), (SHOW, Some(2002)));
    let slim = slim_record(&raw, TmdbKind::Tv, &today);
    assert_eq!(parse_tv(&slim.to_string(), "the slim record").unwrap(), record);
    let (season, season_body) = answered(tmdb_season(&t, Some(&token), SHOW.id, SEASON), "season 1");
    let season_raw = json_of(&season_body);
    field(&season_raw, "/season_number", &["number"], "season 1");
    for (at, episode) in season_raw["episodes"].as_array().expect("episodes").iter().enumerate() {
        field(episode, "/episode_number", &["number"], &format!("season 1, episode {at}"));
        field(episode, "/runtime", &["number", "null"], &format!("season 1, episode {at}"));
    }
    merge_parsed_season(&mut record, season).unwrap();
    let runtimes = record.episode_runtimes(Some(SEASON));
    assert!(!runtimes.is_empty(), "season 1 has episode runtimes");
    let mut spliced = slim.clone();
    splice_season(&mut spliced, &season_raw);
    assert_eq!(parse_tv(&spliced.to_string(), "the spliced record").unwrap().episode_runtimes(Some(SEASON)), runtimes);
    save(&dir, "tmdb-tv-1437.slim.json", &pretty(&slim), Some(&token));
    save(&dir, "tmdb-tv-1437.shape.json", &pretty(&shape_of(&raw)), Some(&token));
    save(&dir, "tmdb-tv-1437-season-1.slim.json", &pretty(&spliced), Some(&token));
    save(&dir, "tmdb-tv-1437-season-1.shape.json", &pretty(&shape_of(&season_raw)), Some(&token));
}

/// Reads the files to check the IDs are listed there; no body is replayed, S69 untouched.
#[test]
fn the_live_requests_use_only_public_ids() {
    let public = |name: String| {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/thediscdb-api").join(&name);
        std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{name}: {error}"))
    };
    let hash = parse_dischash(&public(format!("dischash-{FINGERPRINT}.json")), "dischash").unwrap();
    assert_eq!(hash.and_then(|record| record.tmdb_identity()), Some(MOVIE));
    for (disc_id, identity) in [(DISC_ID, MOVIE), (FIREFLY_DISC_ID, SHOW)] {
        let records = parse_discid(&public(format!("discid-{disc_id}.json")), "discid").unwrap();
        assert!(records.iter().any(|record| record.tmdb_identity() == Some(identity)), "{disc_id}");
    }
}

#[test]
fn the_utc_date_counts_days_as_the_gregorian_calendar() {
    for (seconds, date) in [(0, "1970-01-01"), (86_399, "1970-01-01"), (86_400, "1970-01-02"),
                            (951_782_400, "2000-02-29"), (1_793_750_400, "2026-11-04"), (1_793_836_799, "2026-11-04")] {
        assert_eq!(utc_date(seconds), date, "{seconds}");
    }
}

#[test]
fn saved_live_records_still_parse() {
    let root = match std::env::var_os("LAZYMKV_EVIDENCE").filter(|value| !value.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/owner"),
    };
    if !root.is_dir() {
        return skip(&format!("no evidence clone at {}", root.display()));
    }
    let live = root.join("live-lookups");
    if !live.is_dir() {
        return skip("no live-lookups folder in the evidence clone; run the live tests on the mini PC (design part 8.5)");
    }
    let (today, mut read) = (today(), 0);
    for date in sorted(&live) {
        for file in sorted(&date) {
            let name = file.file_name().unwrap().to_string_lossy().into_owned();
            let shown = format!("{}/{name}", date.file_name().unwrap().to_string_lossy());
            let text = std::fs::read_to_string(&file).unwrap_or_else(|error| panic!("{shown}: {error}"));
            if name.starts_with("tmdb-") && name.ends_with(".slim.json") && is_stale(fetched_at(&text).as_deref(), &today) {
                skip(&format!("{shown} is more than six months old; run the live tests again"));
                continue;
            }
            replay(&name, &text, &shown);
            read += 1;
        }
    }
    if read == 0 {
        skip("the live-lookups folder holds no record younger than six months");
    }
}

/// The folder's entries, sorted by path.
fn sorted(dir: &Path) -> Vec<PathBuf> {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|error| panic!("{}: {error}", dir.display()));
    let mut paths: Vec<PathBuf> =
        entries.map(|entry| entry.unwrap_or_else(|error| panic!("{}: {error}", dir.display())).path()).collect();
    paths.sort();
    paths
}

/// Each record through core's parser by its name (Task 19's ten names): a `.shape.json` passes `shape_only`;
/// `thediscdb-dischash-*` gives Some by `parse_dischash`, `thediscdb-discid-*` a non-empty list by `parse_discid`;
/// `tmdb-search-*.slim.json` a non-empty page by `parse_search`; `tmdb-movie-*.slim.json` `parse_movie` Ok;
/// `tmdb-tv-<id>-season-<n>.slim.json` `parse_tv` with `episode_runtimes(Some(n))` non-empty; `tmdb-tv-<id>.slim.json`
/// `parse_tv` Ok; any other name fails with `<shown>: not a live record of design part 8.5`.
fn replay(name: &str, text: &str, shown: &str) {
    let slim = name.strip_suffix(".slim.json");
    let season = slim
        .and_then(|stem| stem.strip_prefix("tmdb-tv-"))
        .and_then(|rest| rest.split_once("-season-"))
        .and_then(|(_, number)| number.parse::<u32>().ok());
    if name.ends_with(".shape.json") {
        let shape: Value = serde_json::from_str(text).unwrap_or_else(|error| panic!("{shown}: {error}"));
        shape_only(&shape, shown);
    } else if name.starts_with("thediscdb-dischash-") {
        let record = parse_dischash(text, shown).unwrap_or_else(|error| panic!("{shown}: {error}"));
        assert!(record.is_some(), "{shown}: no record");
    } else if name.starts_with("thediscdb-discid-") {
        let records = parse_discid(text, shown).unwrap_or_else(|error| panic!("{shown}: {error}"));
        assert!(!records.is_empty(), "{shown}: no record");
    } else if slim.is_some_and(|stem| stem.starts_with("tmdb-search-")) {
        let page = parse_search(text, shown).unwrap_or_else(|error| panic!("{shown}: {error}"));
        assert!(!page.results.is_empty(), "{shown}: no result");
    } else if slim.is_some_and(|stem| stem.starts_with("tmdb-movie-")) {
        parse_movie(text, shown).unwrap_or_else(|error| panic!("{shown}: {error}"));
    } else if let Some(season) = season {
        let record = parse_tv(text, shown).unwrap_or_else(|error| panic!("{shown}: {error}"));
        assert!(!record.episode_runtimes(Some(season)).is_empty(), "{shown}: season {season} has no episode runtime");
    } else if slim.is_some_and(|stem| stem.starts_with("tmdb-tv-")) {
        parse_tv(text, shown).unwrap_or_else(|error| panic!("{shown}: {error}"));
    } else {
        panic!("{shown}: not a live record of design part 8.5");
    }
}

/// Fails unless every leaf of the shape is a string of shape_of's type names joined by `|` and every array holds at
/// most one element: no value of an answer reached the record.
fn shape_only(shape: &Value, shown: &str) {
    match shape {
        Value::Object(map) => map.values().for_each(|value| shape_only(value, shown)),
        Value::Array(items) => {
            assert!(items.len() <= 1, "{shown}: an array of the shape holds {} elements", items.len());
            items.iter().for_each(|item| shape_only(item, shown));
        }
        Value::String(names) => {
            let known = ["object", "array", "string", "number", "boolean", "null"];
            assert!(names.split('|').all(|name| known.contains(&name)), "{shown}: a leaf is not a list of type names");
        }
        _ => panic!("{shown}: a leaf of the shape is not a string of type names"),
    }
}
