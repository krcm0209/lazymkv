//! TheDiscDB's and TMDb's recorded answers in the evidence clone through `lazymkv_core::lookup` (spec 8.1, last
//! bullet: "the owner's 2 recorded answers"; spec 8.3, first paragraph): the two `thediscdb-hash.json` hits parse and
//! their titles match the recordings' scans, the empty answers read as misses, the slim `tmdb.json` files map to
//! records, and the 18 API answers with the 2 recorded ones hold the track counts spec 8.1 states. Every test skips with
//! its reason when the clone is absent (it is git-ignored and not on CI), never failing for the missing input.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use lazymkv_core::disc::{disc, StreamKind};
use lazymkv_core::lookup::discdb::{match_titles, parse_dischash, parse_discid, DiscDbRecord};
use lazymkv_core::lookup::tmdb::{parse_recorded, TmdbIdentity, TmdbKind, TmdbRecord};
use lazymkv_core::lookup::Kind;
use lazymkv_core::robot::parse;
use lazymkv_dev::evidence::{repo_root, resolve_evidence, skip, Ctx};

/// The evidence clone as every dev command finds it (`resolve_evidence`: LAZYMKV_EVIDENCE, else corpus/owner under the
/// repository root), or None after printing the skip line with the refusal's text.
fn evidence() -> Option<PathBuf> {
    match resolve_evidence(&Ctx::from_process(), None) {
        Ok(root) => Some(root),
        Err(refused) => {
            skip(&format!("no evidence clone: {}", refused.line));
            None
        }
    }
}

fn text(path: &Path) -> String {
    let bytes = std::fs::read(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    String::from_utf8_lossy(&bytes).into_owned()
}

/// A recording's thediscdb-hash.json, which must be a hit.
fn recorded_hit(root: &Path, slug: &str) -> DiscDbRecord {
    let path = root.join(slug).join("thediscdb-hash.json");
    parse_dischash(&text(&path), &path.display().to_string()).unwrap().unwrap()
}

fn indices(titles: Vec<&lazymkv_core::lookup::discdb::DiscDbTitle>) -> Vec<u32> {
    titles.iter().map(|title| title.index).collect()
}

#[test]
fn the_two_recorded_dischash_answers_parse_and_match_their_scans() {
    let Some(root) = evidence() else { return };

    let pans = recorded_hit(&root, "pans-labyrinth-2006-uhd");
    assert_eq!(pans.kind(), Some(Kind::Movie));
    assert_eq!(pans.tmdb_identity(), Some(TmdbIdentity { kind: TmdbKind::Movie, id: 1417 }));
    assert_eq!(pans.title_and_year(), Some(("Pan's Labyrinth".to_string(), Some(2006))));
    // The format, disc number, title count and title items are read from the recorded file at run time (RP21).
    let pans_raw = raw_json(&root.join("pans-labyrinth-2006-uhd/thediscdb-hash.json"));
    assert_eq!((pans.format.clone(), pans.disc_number(), pans.titles.len()), raw_head(&pans_raw));
    assert_eq!(indices(pans.main_movies()), vec![0]);
    let pans_scan = disc(&parse(&text(&root.join("pans-labyrinth-2006-uhd/scan.txt")))).unwrap();
    // The Expected plans table publishes database title 0 as scan title 0; every MainMovie or Episode title matches.
    let pans_matched = match_titles(&pans, &pans_scan);
    assert_eq!(pans_matched.get(&0), Some(&0), "{pans_matched:?}");
    for index in raw_indices_of(&pans_raw, "MainMovie").into_iter().chain(raw_indices_of(&pans_raw, "Episode")) {
        assert!(pans_matched.contains_key(&index), "database title {index} matches no scan title: {pans_matched:?}");
    }

    let office = recorded_hit(&root, "the-office-2005-s3d3-bluray");
    assert_eq!(office.kind(), Some(Kind::Tv));
    assert_eq!(office.tmdb_identity(), Some(TmdbIdentity { kind: TmdbKind::Tv, id: 2316 }));
    assert_eq!(office.title_and_year(), Some(("The Office".to_string(), Some(2005))));
    let office_raw = raw_json(&root.join("the-office-2005-s3d3-bluray/thediscdb-hash.json"));
    assert_eq!((office.format.clone(), office.disc_number(), office.titles.len()), raw_head(&office_raw));
    let episodes = raw_indices_of(&office_raw, "Episode");
    assert_eq!(indices(office.episodes()), episodes);
    // The Expected plans table maps six episode titles.
    assert_eq!(episodes.len(), 6);
    assert!(office.main_movies().is_empty());
    let double = office.titles.iter().find(|title| title.index == 3).and_then(|title| title.item.as_ref()).unwrap();
    // Season 3 and episodes 12-13 are published by the Expected plans table; the item's name is the file's.
    assert_eq!((double.season.as_deref(), double.episode.as_deref()), (Some("3"), Some("12-13")));
    let double_raw = raw_titles(&office_raw).iter().find(|title| raw_index(title) == Some(3)).unwrap();
    assert_eq!(Some(double.title.clone()), string_at(double_raw, "/item/title"));
    let office_scan = disc(&parse(&text(&root.join("the-office-2005-s3d3-bluray/scan.txt")))).unwrap();
    // Values are read from the recorded file at run time (RP21). MakeMKV title numbers are never used for matching
    // (spec 8.3); the Expected plans table publishes the scan titles of the six episodes.
    let matched = match_titles(&office, &office_scan);
    let in_file: BTreeSet<u32> = raw_titles(&office_raw).iter().filter_map(raw_index).collect();
    assert!(matched.keys().all(|index| in_file.contains(index)), "{matched:?}");
    assert!(episodes.iter().all(|index| matched.contains_key(index)), "{matched:?}");
    let episode_scans: BTreeSet<u32> = episodes.iter().map(|index| matched[index]).collect();
    assert_eq!(episode_scans, BTreeSet::from([0, 1, 2, 3, 5, 6]));
    let scans: BTreeSet<&u32> = matched.values().collect();
    assert_eq!(scans.len(), matched.len(), "a scan title is matched twice: {matched:?}");
}

#[test]
fn the_recorded_empty_answers_read_as_misses() {
    let Some(root) = evidence() else { return };
    for slug in ["short-circuit-1986-dvd", "short-circuit-2-1988-dvd"] {
        let path = root.join(slug).join("thediscdb-hash.json");
        assert_eq!(parse_dischash(&text(&path), &path.display().to_string()), Ok(None), "{}", path.display());
    }
    for slug in ["pans-labyrinth-2006-uhd", "the-office-2005-s3d3-bluray"] {
        let path = root.join(slug).join("thediscdb-discid.json");
        assert_eq!(parse_discid(&text(&path), &path.display().to_string()), Ok(Vec::new()), "{}", path.display());
    }
}

/// A recorded tmdb.json or thediscdb-hash.json as raw JSON (a leading byte-order mark dropped), so the fields no public
/// source gives (the original title, the IMDb and TVDB IDs, TheDiscDB's format, disc number, titles and item names) are
/// compared with the file itself when the test runs, never written here as values copied from the private clone
/// (ruling RP21).
fn raw_json(path: &Path) -> serde_json::Value {
    serde_json::from_str(text(path).trim_start_matches('\u{feff}'))
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The `titles` array of a recorded thediscdb-hash.json, read raw.
fn raw_titles(value: &serde_json::Value) -> &Vec<serde_json::Value> {
    value.pointer("/titles").and_then(serde_json::Value::as_array).expect("a dischash answer has a titles array")
}

/// A raw title's `index`.
fn raw_index(title: &serde_json::Value) -> Option<u32> {
    title.pointer("/index").and_then(serde_json::Value::as_u64).and_then(|index| u32::try_from(index).ok())
}

/// The indices of a recorded answer's titles whose item type is `item_type`, read raw, in index order.
fn raw_indices_of(value: &serde_json::Value, item_type: &str) -> Vec<u32> {
    let mut found: Vec<u32> = raw_titles(value)
        .iter()
        .filter(|title| string_at(title, "/item/type").as_deref() == Some(item_type))
        .filter_map(raw_index)
        .collect();
    found.sort_unstable();
    found
}

/// A recorded answer's format, disc number (`disc.index`) and title count, read raw.
fn raw_head(value: &serde_json::Value) -> (Option<String>, Option<i64>, usize) {
    let disc_number = value.pointer("/disc/index").and_then(serde_json::Value::as_i64);
    (string_at(value, "/format"), disc_number, raw_titles(value).len())
}

/// The string at `pointer` in `value`, or None.
fn string_at(value: &serde_json::Value, pointer: &str) -> Option<String> {
    value.pointer(pointer).and_then(serde_json::Value::as_str).map(str::to_string)
}

#[test]
fn the_recorded_tmdb_json_files_map_to_their_records() {
    let Some(root) = evidence() else { return };
    let path = root.join("pans-labyrinth-2006-uhd/tmdb.json");
    let raw = raw_json(&path);
    let pans = parse_recorded(&text(&path), &path.display().to_string()).unwrap();
    assert_eq!(
        pans,
        TmdbRecord {
            identity: TmdbIdentity { kind: TmdbKind::Movie, id: 1417 },
            title: "Pan's Labyrinth".to_string(),
            original_title: string_at(&raw, "/original_title").expect("the recorded movie has an original title"),
            year: Some(2006),
            original_language: Some("es".to_string()),
            runtime: None,
            imdb_id: string_at(&raw, "/external_ids/imdb_id"),
            tvdb_id: None,
            seasons: Vec::new(),
        }
    );
    // The embedded external ID is read, and the recorder kept one.
    assert!(pans.imdb_id.as_deref().is_some_and(|id| id.starts_with("tt")), "{:?}", pans.imdb_id);
    assert_ne!(pans.original_title, pans.title, "Pan's Labyrinth is a Spanish film with a Spanish original title");
    let path = root.join("the-office-2005-s3d3-bluray/tmdb.json");
    let raw = raw_json(&path);
    let office = parse_recorded(&text(&path), &path.display().to_string()).unwrap();
    assert_eq!(
        office,
        TmdbRecord {
            identity: TmdbIdentity { kind: TmdbKind::Tv, id: 2316 },
            title: "The Office".to_string(),
            original_title: "The Office".to_string(),
            year: Some(2005),
            original_language: Some("en".to_string()),
            runtime: None,
            imdb_id: string_at(&raw, "/external_ids/imdb_id"),
            tvdb_id: raw.pointer("/external_ids/tvdb_id").and_then(serde_json::Value::as_i64),
            seasons: Vec::new(),
        }
    );
    assert!(office.imdb_id.as_deref().is_some_and(|id| id.starts_with("tt")), "{:?}", office.imdb_id);
    assert!(office.tvdb_id.is_some_and(|id| id > 0), "{:?}", office.tvdb_id);
}

#[test]
fn the_api_answers_and_the_two_recorded_ones_hold_the_track_counts_spec_8_1_states() {
    let Some(root) = evidence() else { return };
    let api = repo_root(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap().join("corpus/thediscdb-api");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&api)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "json"))
        .collect();
    paths.sort();
    assert_eq!(paths.len(), 18);
    let mut records: Vec<DiscDbRecord> = Vec::new();
    for path in &paths {
        let shown = path.display().to_string();
        if path.file_name().is_some_and(|name| name.to_string_lossy().starts_with("dischash-")) {
            records.push(parse_dischash(&text(path), &shown).unwrap().unwrap());
        } else {
            records.extend(parse_discid(&text(path), &shown).unwrap());
        }
    }
    records.push(recorded_hit(&root, "pans-labyrinth-2006-uhd"));
    records.push(recorded_hit(&root, "the-office-2005-s3d3-bluray"));
    let (mut audio, mut audio_described, mut subtitles, mut subtitles_described) = (0, 0, 0, 0);
    for track in records.iter().flat_map(|record| &record.titles).flat_map(|title| &title.tracks) {
        match track.kind() {
            Some(StreamKind::Audio) => {
                audio += 1;
                audio_described += usize::from(track.description.is_some());
            }
            Some(StreamKind::Subtitles) => {
                subtitles += 1;
                subtitles_described += usize::from(track.description.is_some());
            }
            _ => {}
        }
    }
    // "An audio or subtitle track sometimes has description (58 of 877 audio and 30 of 2018 subtitle tracks)".
    assert_eq!((audio_described, audio, subtitles_described, subtitles), (58, 877, 30, 2018));
}
