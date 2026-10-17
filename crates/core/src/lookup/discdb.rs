//! TheDiscDB's answers (spec 8.1, last bullet) and the match of database titles to scan titles (spec 8.3, first
//! paragraph), `core::lookup::discdb`. One set of types reads both forms of a record: the API's answers
//! (`https://thediscdb.com/api/dischash/{hash}`, one object, and `https://thediscdb.com/api/discid/{id}`, an array; the 18
//! saved in `corpus/thediscdb-api` and the owner's two recorded ones), with snake_case keys and the `media`, `release`
//! and `disc` objects, and the data repository's pinned `discNN.json` files (`corpus/thediscdb`), with PascalCase keys
//! read through `#[serde(alias)]` and the disc's own facts (`Index`, `Name`) at the top level. A miss is an empty body.
//! On a DVD, TheDiscDB writes the DVD title number (MakeMKV's attribute 24) in `source_file`, so `same_title` compares
//! it with the scan's attribute 24 as a number. Used by Tasks 4 to 11, 15 and 16.

use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use serde_json::Value;

use super::tmdb::{TmdbIdentity, TmdbKind};
use super::{parse_json, without_bom, Kind};
use crate::disc::{same_source, segment_maps_agree, title_source, Disc, StreamKind, Title, DURATION_MATCH_S};
use crate::recording::files::{thediscdb_answer, DiscDbAnswer};
use crate::robot::duration_seconds;

/// `item.type` of a described title (spec 8.1): the seven values of the fixtures, and Unknown (`#[serde(other)]`) for any
/// other TheDiscDB type, so one unknown item never fails a whole record. Spec 8.1. Def: T3. Use: T5, T6.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ItemType {
    MainMovie,
    Episode,
    Extra,
    Featurette,
    DeletedScene,
    Trailer,
    Other,
    #[serde(other)]
    Unknown,
}

impl ItemType {
    /// The seven types spec 8.1 lists, in its order; Unknown is not one of them.
    pub const ALL: [ItemType; 7] = [
        ItemType::MainMovie,
        ItemType::Episode,
        ItemType::Extra,
        ItemType::Featurette,
        ItemType::DeletedScene,
        ItemType::Trailer,
        ItemType::Other,
    ];

    /// TheDiscDB's spelling ("MainMovie", "DeletedScene"), which is also the serde one; "Unknown" for a type outside
    /// the seven. Spec 8.1.
    pub fn as_str(self) -> &'static str {
        match self {
            ItemType::MainMovie => "MainMovie",
            ItemType::Episode => "Episode",
            ItemType::Extra => "Extra",
            ItemType::Featurette => "Featurette",
            ItemType::DeletedScene => "DeletedScene",
            ItemType::Trailer => "Trailer",
            ItemType::Other => "Other",
            ItemType::Unknown => "Unknown",
        }
    }
}

/// One of the seven spellings exactly; "Unknown" and every other text is refused. Spec 8.1.
impl FromStr for ItemType {
    type Err = String;

    fn from_str(text: &str) -> Result<ItemType, String> {
        ItemType::ALL.into_iter().find(|item_type| item_type.as_str() == text).ok_or_else(|| {
            format!(
                "{text:?} is not a TheDiscDB item type (MainMovie, Episode, Extra, Featurette, DeletedScene, Trailer or Other)"
            )
        })
    }
}

/// A described title's item. `season` and `episode` are strings as the API gives them ("01", "1", "12-13"). Spec 8.1.
/// Def: T3. Use: T5, T6, T11.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DiscDbItem {
    #[serde(alias = "Title")]
    pub title: String,
    #[serde(rename = "type", alias = "Type")]
    pub item_type: ItemType,
    /// Present where the item belongs to an episode (an Episode, or the Office's DeletedScene items).
    #[serde(default, alias = "Season")]
    pub season: Option<String>,
    #[serde(default, alias = "Episode")]
    pub episode: Option<String>,
}

/// One track of a TheDiscDB title: `type` is Video, Audio or Subtitles; `name` is MakeMKV's attribute 7 (the long codec
/// name: "Dolby Digital", "DTS-HD Master Audio", "TrueHD Atmos", "HDMV PGS Subtitles", "Dvd Subtitles", "Mpeg2",
/// "MpegH HEVC Main10@L5.1 (dvhe.07.06 BL+FEL+RPU)"); an audio track has `audio_type` ("Stereo", "Surround 5.1",
/// "Surround 7.1"; no channel count), `language_code` (three letters) and `language` ("English"); a subtitle track
/// `language_code` and `language`; `description` sometimes ("ENglish", "Spanish", "Director Commentary"). Spec 8.1.
/// Def: T3. Use: T8, T9.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DiscDbTrack {
    #[serde(alias = "Index")]
    pub index: u32,
    #[serde(rename = "type", alias = "Type")]
    pub track_type: String,
    #[serde(default, alias = "Name")]
    pub name: Option<String>,
    /// Video only ("720x480", "3840x2160").
    #[serde(default, alias = "Resolution")]
    pub resolution: Option<String>,
    /// Video only ("16:9", "4:3").
    #[serde(default, alias = "AspectRatio")]
    pub aspect_ratio: Option<String>,
    #[serde(default, alias = "AudioType")]
    pub audio_type: Option<String>,
    #[serde(default, alias = "LanguageCode")]
    pub language_code: Option<String>,
    #[serde(default, alias = "Language")]
    pub language: Option<String>,
    #[serde(default, alias = "Description")]
    pub description: Option<String>,
}

impl DiscDbTrack {
    /// TheDiscDB's `type` is spelt as MakeMKV's attribute 1 ("Video", "Audio", "Subtitles"), so
    /// `disc::StreamKind::from_attr` reads it (one copy); None for anything else. Spec 8.1. Def: T3. Use: T8.
    pub fn kind(&self) -> Option<StreamKind> {
        StreamKind::from_attr(&self.track_type)
    }

    /// "Stereo" 2, "Surround 5.1" 6, "Surround 7.1" 8, "Mono" 1, "Surround 5.0" 5, "Surround 6.1" 7; None otherwise
    /// (spec 8.1: `audio_type` is the layout, there is no channel count). Def: T3. Use: T8.
    pub fn channels(&self) -> Option<u32> {
        match self.audio_type.as_deref()? {
            "Mono" => Some(1),
            "Stereo" => Some(2),
            "Surround 5.0" => Some(5),
            "Surround 5.1" => Some(6),
            "Surround 6.1" => Some(7),
            "Surround 7.1" => Some(8),
            _ => None,
        }
    }
}

/// One chapter of a TheDiscDB title. Spec 8.1. Def: T3. Use: none beyond parsing (kept for the full shape).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DiscDbChapter {
    #[serde(alias = "Index")]
    pub index: u32,
    #[serde(default, alias = "Title")]
    pub title: Option<String>,
}

/// One title of the answer. `duration` is "H:MM:SS", `size` bytes, `segment_map` MakeMKV's text (may end in "...").
/// Spec 8.1. Def: T3. Use: T5, T6, T8, T9, T10.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DiscDbTitle {
    #[serde(alias = "Index")]
    pub index: u32,
    /// Attribute 16 on a Blu-ray; the DVD title number (attribute 24's text) on a DVD.
    #[serde(default, alias = "SourceFile")]
    pub source_file: Option<String>,
    #[serde(default, alias = "SegmentMap")]
    pub segment_map: Option<String>,
    #[serde(default, alias = "Duration")]
    pub duration: Option<String>,
    #[serde(default, alias = "Size")]
    pub size: Option<u64>,
    #[serde(default, alias = "DisplaySize")]
    pub display_size: Option<String>,
    #[serde(default, alias = "Resolution")]
    pub resolution: Option<String>,
    /// MakeMKV's attribute 27, the output file name ("title.mkv", "Smile.mkv"); never compared with Title.comment
    /// (attribute 49). Only the pinned form has it.
    #[serde(default, alias = "Comment")]
    pub comment: Option<String>,
    #[serde(default, alias = "Chapters")]
    pub chapters: Vec<DiscDbChapter>,
    #[serde(default, alias = "Tracks")]
    pub tracks: Vec<DiscDbTrack>,
    /// Only on a title TheDiscDB has described (with `item`), and only in the API's form.
    #[serde(default, alias = "FileName")]
    pub file_name: Option<String>,
    #[serde(default, alias = "Item")]
    pub item: Option<DiscDbItem>,
}

impl DiscDbTitle {
    /// `robot::duration_seconds(duration)`; None without a duration or for one that is not "H:MM:SS". Spec 8.1.
    /// Def: T3. Use: T3 (`same_title`).
    pub fn seconds(&self) -> Option<u32> {
        self.duration.as_deref().and_then(duration_seconds)
    }

    /// The tracks of one kind, in index order (whatever order the answer lists them in). Spec 8.1. Def: T3. Use: T8.
    pub fn tracks_of(&self, kind: StreamKind) -> Vec<&DiscDbTrack> {
        let mut tracks: Vec<&DiscDbTrack> = self.tracks.iter().filter(|track| track.kind() == Some(kind)).collect();
        tracks.sort_by_key(|track| track.index);
        tracks
    }
}

/// `media`: `type` is "Movie" or "Series"; `external_ids.tmdb` and `.imdb` are strings ("1437", "tt0303461"). Spec 8.1.
/// Def: T3. Use: T3, T4, T5, T11.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DiscDbMedia {
    pub title: Option<String>,
    pub full_title: Option<String>,
    pub year: Option<i32>,
    #[serde(rename = "type")]
    pub media_type: Option<String>,
    #[serde(default)]
    pub external_ids: DiscDbExternalIds,
}

/// `media.external_ids`: TMDb's and IMDb's IDs as strings. Spec 8.1. Def: T3.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DiscDbExternalIds {
    pub tmdb: Option<String>,
    pub imdb: Option<String>,
}

/// `release`: title, year, slug, locale ("en-us"), region_code, upc. Spec 8.1. Def: T3.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DiscDbRelease {
    pub title: Option<String>,
    pub year: Option<i32>,
    pub slug: Option<String>,
    pub locale: Option<String>,
    pub region_code: Option<String>,
    pub upc: Option<String>,
}

/// The API's `disc` object: index, name ("Season Three Disc Three"), slug (the pinned form has none: its disc facts are
/// DiscDbRecord's `pinned_index` and `pinned_name`). Spec 8.1. Def: T3. Use: T3 (`DiscDbRecord::disc_number`).
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DiscDbDisc {
    pub index: Option<i64>,
    pub name: Option<String>,
    pub slug: Option<String>,
}

/// One TheDiscDB object (a `dischash` answer, or one element of a `discid` answer, or a pinned `discNN.json`). Spec 8.1.
/// Def: T3. Use: T4 to T11, T15.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DiscDbRecord {
    #[serde(default, alias = "ContentHash")]
    pub content_hash: Option<String>,
    #[serde(default, alias = "GlobalDiscId")]
    pub global_disc_id: Option<String>,
    /// "UHD", "Blu-ray", "Blu-Ray", "DVD".
    #[serde(default, alias = "Format")]
    pub format: Option<String>,
    #[serde(default)]
    pub media: Option<DiscDbMedia>,
    #[serde(default)]
    pub release: Option<DiscDbRelease>,
    #[serde(default)]
    pub disc: Option<DiscDbDisc>,
    /// The pinned form's top-level disc facts (`Index` 1 or 2, `Name`); None in an API answer.
    #[serde(default, rename = "Index")]
    pub pinned_index: Option<i64>,
    #[serde(default, rename = "Name")]
    pub pinned_name: Option<String>,
    #[serde(default, alias = "Titles")]
    pub titles: Vec<DiscDbTitle>,
}

impl DiscDbRecord {
    /// `media.type` "Movie" gives Kind::Movie, "Series" Kind::Tv; None otherwise (8.2 "TheDiscDB states the kind"; a
    /// pinned record has no media, so None). Spec 8.1, 8.2. Def: T3. Use: T5.
    pub fn kind(&self) -> Option<Kind> {
        match self.media.as_ref()?.media_type.as_deref()? {
            "Movie" => Some(Kind::Movie),
            "Series" => Some(Kind::Tv),
            _ => None,
        }
    }

    /// `media.external_ids.tmdb` parsed (a positive whole number) and paired with the kind (8.1 "A match supplies a TMDb
    /// ID"); None without a kind or without such an ID. Spec 8.1. Def: T3. Use: T4, T7.
    pub fn tmdb_identity(&self) -> Option<TmdbIdentity> {
        let kind = self.kind()?;
        let id = self.media.as_ref()?.external_ids.tmdb.as_deref()?.trim().parse::<i64>().ok().filter(|&id| id > 0)?;
        Some(TmdbIdentity { kind: TmdbKind::from(kind), id })
    }

    /// The titles whose item type is MainMovie, in index order. Spec 8.1 "The main movie is the title whose item.type is
    /// MainMovie". Def: T3. Use: T5.
    pub fn main_movies(&self) -> Vec<&DiscDbTitle> {
        self.titles_of(ItemType::MainMovie)
    }

    /// The titles whose item type is Episode, in index order. Spec 8.1, 8.5. Def: T3. Use: T6.
    pub fn episodes(&self) -> Vec<&DiscDbTitle> {
        self.titles_of(ItemType::Episode)
    }

    /// `media.title` (trimmed, not empty) and `media.year` for the TMDb search of 8.1 ("TMDb is searched with the entry's
    /// title and year"). Spec 8.1. Def: T3. Use: T4.
    pub fn title_and_year(&self) -> Option<(String, Option<i32>)> {
        let media = self.media.as_ref()?;
        let title = media.title.as_deref().map(str::trim).filter(|title| !title.is_empty())?;
        Some((title.to_string(), media.year))
    }

    /// The disc's number on its release: `disc.index`, else `pinned_index`. Spec 8.1. Def: T3. Use: phase 2.
    pub fn disc_number(&self) -> Option<i64> {
        self.disc.as_ref().and_then(|disc| disc.index).or(self.pinned_index)
    }

    /// The titles whose item has `item_type`, in index order.
    fn titles_of(&self, item_type: ItemType) -> Vec<&DiscDbTitle> {
        let mut titles: Vec<&DiscDbTitle> = self
            .titles
            .iter()
            .filter(|title| title.item.as_ref().is_some_and(|item| item.item_type == item_type))
            .collect();
        titles.sort_by_key(|title| title.index);
        titles
    }
}

/// A `dischash` answer: None for an empty body (the recorder writes an empty file on 404) or when
/// `recording::files::thediscdb_answer` of the parsed value is not Hit; else the object. A pinned `discNN.json` (an object
/// whose capitalised `Titles` is a non-empty array) is a hit too, so the pinned files read through this function; a
/// non-empty array is refused (a dischash answer is one object). Err is serde_json's message prefixed by `shown`.
/// Spec 8.1. Def: T3. Use: T15.
pub fn parse_dischash(text: &str, shown: &str) -> Result<Option<DiscDbRecord>, String> {
    if without_bom(text).trim().is_empty() {
        return Ok(None);
    }
    let value: Value = parse_json(text, shown)?;
    let pinned = value.get("Titles").and_then(Value::as_array).is_some_and(|titles| !titles.is_empty());
    if thediscdb_answer(&value) != DiscDbAnswer::Hit && !pinned {
        return Ok(None);
    }
    if !value.is_object() {
        return Err(format!("{shown}: a dischash answer is one object, not an array"));
    }
    serde_json::from_value(value).map(Some).map_err(|error| format!("{shown}: {error}"))
}

/// A `discid` answer: an array (empty on a miss), or an empty body as an empty array. Err is serde_json's message
/// prefixed by `shown` (an object is not an array). Spec 8.1. Def: T3. Use: T15.
pub fn parse_discid(text: &str, shown: &str) -> Result<Vec<DiscDbRecord>, String> {
    if without_bom(text).trim().is_empty() {
        return Ok(Vec::new());
    }
    parse_json(text, shown)
}

/// A database title matches a scan title (spec 8.3 "on source file name and segment map, then exact size and duration
/// within 2 s"): source first: when the scan title has attribute 16, `db.source_file == title.source_file`; otherwise
/// `disc::same_source(&title_source(None, db.source_file), &title.source())` (on a DVD TheDiscDB writes attribute 24 in
/// source_file, compared as numbers); a title with neither never matches. Then `disc::segment_maps_agree` (a database
/// title without a segment map compares as ""), exact size when both carry one, and duration within
/// `disc::DURATION_MATCH_S` (a side without a duration never matches). MakeMKV title numbers are never used. Spec 8.3.
/// Def: T3. Use: T3.
pub fn same_title(db: &DiscDbTitle, title: &Title) -> bool {
    let source_matches = match title.source_file.as_deref().filter(|name| !name.is_empty()) {
        Some(name) => db.source_file.as_deref() == Some(name),
        None => same_source(&title_source(None, db.source_file.as_deref()), &title.source()),
    };
    if !source_matches || !segment_maps_agree(db.segment_map.as_deref().unwrap_or(""), &title.segment_map) {
        return false;
    }
    if let (Some(db_size), Some(scan_size)) = (db.size, title.size) {
        if db_size != scan_size {
            return false;
        }
    }
    match (db.seconds(), title.seconds) {
        (Some(db_seconds), Some(scan_seconds)) => db_seconds.abs_diff(scan_seconds) <= DURATION_MATCH_S,
        _ => false,
    }
}

/// Every database title matched to the first scan title it matches, in database index order; a scan title is taken once.
/// Key: database index; value: scan title number. A database index the record repeats keeps its first match. Spec 8.3
/// "Database titles are matched to scan titles". Def: T3. Use: T5, T6, T7, T8, T9, T16.
pub fn match_titles(record: &DiscDbRecord, disc: &Disc) -> BTreeMap<u32, u32> {
    let mut by_index: Vec<&DiscDbTitle> = record.titles.iter().collect();
    by_index.sort_by_key(|db| db.index);
    let mut taken: BTreeSet<u32> = BTreeSet::new();
    let mut matches = BTreeMap::new();
    for db in by_index {
        if matches.contains_key(&db.index) {
            continue;
        }
        let found = disc.titles.values().find(|title| !taken.contains(&title.number) && same_title(db, title));
        if let Some(title) = found {
            taken.insert(title.number);
            matches.insert(db.index, title.number);
        }
    }
    matches
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::{Path, PathBuf};

    use crate::disc::disc;
    use crate::robot::parse;
    use crate::testsupport::scan_text;

    /// TheDiscDB's answer for Firefly disc 2 (`corpus/thediscdb-api/dischash-294A3E64C457A1BC9A0833335C090568.json`),
    /// shortened: the record's own fields unchanged, title 3 whole, and title 5 with three of its twelve chapters and
    /// four of its nine tracks.
    const FIREFLY_API_EXCERPT: &str = r#"{
  "global_disc_id": "C5B150FCB154727FAF60C1BEFA1DCB88",
  "format": "DVD",
  "content_hash": "294A3E64C457A1BC9A0833335C090568",
  "media": {"title": "Firefly", "full_title": "Firefly", "year": 2002, "type": "Series", "external_ids": {"tmdb": "1437", "imdb": "tt0303461"}},
  "release": {"slug": "2003-dvd-the-complete-series", "title": "The Complete Series DVD", "year": 2003, "region_code": "1", "locale": "en-us", "upc": "024543089292"},
  "disc": {"slug": "disc-2", "name": "Disc 2", "index": 2},
  "titles": [
    {"index": 3, "source_file": "05", "segment_map": "1,2", "duration": "0:00:07", "display_size": "7.5 MB", "size": 7931904, "resolution": "480p",
     "chapters": [],
     "tracks": [{"index": 0, "name": "Mpeg2", "type": "Video", "resolution": "720x480", "aspect_ratio": "4:3"}]},
    {"index": 5, "source_file": "11", "segment_map": "1-4,5-9,10-14,15", "duration": "0:43:48", "display_size": "1.8 GB", "size": 1966837760, "file_name": "Firefly.S01.E07.Jaynestown.mkv", "resolution": "480p",
     "chapters": [{"index": 1, "title": "Mud Buyers"}, {"index": 2, "title": "Main Titles"}, {"index": 3, "title": "The Statue"}],
     "tracks": [
      {"index": 0, "name": "Mpeg2", "type": "Video", "resolution": "720x480", "aspect_ratio": "16:9"},
      {"index": 1, "name": "Dolby Digital", "type": "Audio", "audio_type": "Stereo", "language_code": "eng", "language": "English", "description": "English"},
      {"index": 4, "name": "Dvd Subtitles", "type": "Subtitles", "language_code": "eng", "language": "English", "description": "English"},
      {"index": 8, "name": "Closed Captions", "type": "Subtitles", "language_code": "eng", "language": "English"}
     ],
     "item": {"title": "Jaynestown", "type": "Episode", "season": "1", "episode": "7"}}
  ]
}"#;

    /// The same disc in the data repository's pinned form (`corpus/thediscdb/series/Firefly (2002)/
    /// 2003-dvd-the-complete-series/disc02.json`), shortened the same way; the item's chapter list (which no type reads)
    /// is cut to one.
    const FIREFLY_PINNED_EXCERPT: &str = r#"{
  "Index": 2,
  "Slug": "disc-2",
  "Name": "Disc 2",
  "Format": "DVD",
  "ContentHash": "294A3E64C457A1BC9A0833335C090568",
  "GlobalDiscId": "C5B150FCB154727FAF60C1BEFA1DCB88",
  "Titles": [
    {"Index": 3, "Comment": "A1_t03_t05.mkv", "SourceFile": "05", "SegmentMap": "1,2", "Duration": "0:00:07", "Size": 7931904, "DisplaySize": "7.5 MB",
     "Tracks": [{"Index": 0, "Name": "Mpeg2", "Type": "Video", "Resolution": "720x480", "AspectRatio": "4:3"}]},
    {"Index": 5, "Comment": "F1_t05_t11.mkv", "SourceFile": "11", "SegmentMap": "1-4,5-9,10-14,15", "Duration": "0:43:48", "Size": 1966837760, "DisplaySize": "1.8 GB",
     "Item": {"Title": "Jaynestown", "Type": "Episode", "Chapters": [{"Index": 1, "Title": "Mud Buyers"}], "Season": "1", "Episode": "7"},
     "Tracks": [
      {"Index": 0, "Name": "Mpeg2", "Type": "Video", "Resolution": "720x480", "AspectRatio": "16:9"},
      {"Index": 1, "Name": "Dolby Digital", "Type": "Audio", "AudioType": "Stereo", "LanguageCode": "eng", "Language": "English", "Description": "English"},
      {"Index": 4, "Name": "Dvd Subtitles", "Type": "Subtitles", "LanguageCode": "eng", "Language": "English", "Description": "English"},
      {"Index": 8, "Name": "Closed Captions", "Type": "Subtitles", "LanguageCode": "eng", "Language": "English"}
     ]}
  ]
}"#;

    /// A track with its index, type and name, and nothing else.
    fn track(index: u32, track_type: &str, name: &str) -> DiscDbTrack {
        DiscDbTrack {
            index,
            track_type: track_type.to_string(),
            name: Some(name.to_string()),
            resolution: None,
            aspect_ratio: None,
            audio_type: None,
            language_code: None,
            language: None,
            description: None,
        }
    }

    fn chapter(index: u32, title: &str) -> DiscDbChapter {
        DiscDbChapter { index, title: Some(title.to_string()) }
    }

    /// A title with its index and nothing else.
    fn bare_title(index: u32) -> DiscDbTitle {
        DiscDbTitle {
            index,
            source_file: None,
            segment_map: None,
            duration: None,
            size: None,
            display_size: None,
            resolution: None,
            comment: None,
            chapters: Vec::new(),
            tracks: Vec::new(),
            file_name: None,
            item: None,
        }
    }

    /// A record with no field and no title.
    fn bare_record() -> DiscDbRecord {
        DiscDbRecord {
            content_hash: None,
            global_disc_id: None,
            format: None,
            media: None,
            release: None,
            disc: None,
            pinned_index: None,
            pinned_name: None,
            titles: Vec::new(),
        }
    }

    fn indices(titles: Vec<&DiscDbTitle>) -> Vec<u32> {
        titles.iter().map(|title| title.index).collect()
    }

    /// Firefly disc 2's titles 3 and 5 as both excerpts give them, without what only one form carries (the API's
    /// file_name, title resolution and chapters; the pinned form's comment).
    fn firefly_titles() -> Vec<DiscDbTitle> {
        let english = |track: DiscDbTrack| DiscDbTrack {
            language_code: Some("eng".to_string()),
            language: Some("English".to_string()),
            ..track
        };
        vec![
            DiscDbTitle {
                source_file: Some("05".to_string()),
                segment_map: Some("1,2".to_string()),
                duration: Some("0:00:07".to_string()),
                size: Some(7931904),
                display_size: Some("7.5 MB".to_string()),
                tracks: vec![DiscDbTrack {
                    resolution: Some("720x480".to_string()),
                    aspect_ratio: Some("4:3".to_string()),
                    ..track(0, "Video", "Mpeg2")
                }],
                ..bare_title(3)
            },
            DiscDbTitle {
                source_file: Some("11".to_string()),
                segment_map: Some("1-4,5-9,10-14,15".to_string()),
                duration: Some("0:43:48".to_string()),
                size: Some(1966837760),
                display_size: Some("1.8 GB".to_string()),
                tracks: vec![
                    DiscDbTrack {
                        resolution: Some("720x480".to_string()),
                        aspect_ratio: Some("16:9".to_string()),
                        ..track(0, "Video", "Mpeg2")
                    },
                    english(DiscDbTrack {
                        audio_type: Some("Stereo".to_string()),
                        description: Some("English".to_string()),
                        ..track(1, "Audio", "Dolby Digital")
                    }),
                    english(DiscDbTrack { description: Some("English".to_string()), ..track(4, "Subtitles", "Dvd Subtitles") }),
                    english(track(8, "Subtitles", "Closed Captions")),
                ],
                item: Some(DiscDbItem {
                    title: "Jaynestown".to_string(),
                    item_type: ItemType::Episode,
                    season: Some("1".to_string()),
                    episode: Some("7".to_string()),
                }),
                ..bare_title(5)
            },
        ]
    }

    #[test]
    fn item_types_spell_themselves_and_an_unknown_type_reads_as_unknown() {
        let spellings = ["MainMovie", "Episode", "Extra", "Featurette", "DeletedScene", "Trailer", "Other"];
        assert_eq!(ItemType::ALL.map(ItemType::as_str), spellings);
        for item_type in ItemType::ALL {
            let json = serde_json::to_string(&item_type).unwrap();
            assert_eq!(json, format!("\"{}\"", item_type.as_str()));
            assert_eq!(serde_json::from_str::<ItemType>(&json).unwrap(), item_type);
            assert_eq!(item_type.as_str().parse::<ItemType>(), Ok(item_type));
        }
        // A type outside the seven (spec 8.1 lists them) reads as Unknown instead of failing the record.
        assert_eq!(serde_json::from_str::<ItemType>("\"BonusDisc\"").unwrap(), ItemType::Unknown);
        assert_eq!(serde_json::from_str::<ItemType>("\"mainmovie\"").unwrap(), ItemType::Unknown);
        assert_eq!(ItemType::Unknown.as_str(), "Unknown");
        assert!(!ItemType::ALL.contains(&ItemType::Unknown));
        assert_eq!(
            "Unknown".parse::<ItemType>(),
            Err("\"Unknown\" is not a TheDiscDB item type (MainMovie, Episode, Extra, Featurette, DeletedScene, Trailer or Other)"
                .to_string())
        );
    }

    #[test]
    fn a_record_with_an_unknown_item_type_still_parses() {
        let record: DiscDbRecord = serde_json::from_str(
            r#"{"titles": [
                {"index": 0, "item": {"title": "Smile", "type": "MainMovie"}},
                {"index": 1, "item": {"title": "Smile (Commercial)", "type": "Commercial"}}
            ]}"#,
        )
        .unwrap();
        let unknown = record.titles[1].item.as_ref().unwrap();
        assert_eq!((unknown.title.as_str(), unknown.item_type), ("Smile (Commercial)", ItemType::Unknown));
        assert_eq!(indices(record.main_movies()), vec![0]);
        assert!(record.episodes().is_empty());
    }

    #[test]
    fn an_api_answer_reads_into_the_typed_record_field_by_field() {
        let record: DiscDbRecord = serde_json::from_str(FIREFLY_API_EXCERPT).unwrap();
        let mut titles = firefly_titles();
        titles[0].resolution = Some("480p".to_string());
        titles[1].resolution = Some("480p".to_string());
        titles[1].file_name = Some("Firefly.S01.E07.Jaynestown.mkv".to_string());
        titles[1].chapters = vec![chapter(1, "Mud Buyers"), chapter(2, "Main Titles"), chapter(3, "The Statue")];
        let expected = DiscDbRecord {
            content_hash: Some("294A3E64C457A1BC9A0833335C090568".to_string()),
            global_disc_id: Some("C5B150FCB154727FAF60C1BEFA1DCB88".to_string()),
            format: Some("DVD".to_string()),
            media: Some(DiscDbMedia {
                title: Some("Firefly".to_string()),
                full_title: Some("Firefly".to_string()),
                year: Some(2002),
                media_type: Some("Series".to_string()),
                external_ids: DiscDbExternalIds { tmdb: Some("1437".to_string()), imdb: Some("tt0303461".to_string()) },
            }),
            release: Some(DiscDbRelease {
                title: Some("The Complete Series DVD".to_string()),
                year: Some(2003),
                slug: Some("2003-dvd-the-complete-series".to_string()),
                locale: Some("en-us".to_string()),
                region_code: Some("1".to_string()),
                upc: Some("024543089292".to_string()),
            }),
            disc: Some(DiscDbDisc { index: Some(2), name: Some("Disc 2".to_string()), slug: Some("disc-2".to_string()) }),
            pinned_index: None,
            pinned_name: None,
            titles,
        };
        assert_eq!(record, expected);
    }

    #[test]
    fn a_pinned_file_reads_through_the_pascal_case_aliases() {
        let record: DiscDbRecord = serde_json::from_str(FIREFLY_PINNED_EXCERPT).unwrap();
        let mut titles = firefly_titles();
        titles[0].comment = Some("A1_t03_t05.mkv".to_string());
        titles[1].comment = Some("F1_t05_t11.mkv".to_string());
        let expected = DiscDbRecord {
            content_hash: Some("294A3E64C457A1BC9A0833335C090568".to_string()),
            global_disc_id: Some("C5B150FCB154727FAF60C1BEFA1DCB88".to_string()),
            format: Some("DVD".to_string()),
            pinned_index: Some(2),
            pinned_name: Some("Disc 2".to_string()),
            titles,
            ..bare_record()
        };
        assert_eq!(record, expected);
        assert_eq!(record.disc_number(), Some(2));
    }

    #[test]
    fn track_kinds_and_channel_counts_come_from_type_and_audio_type() {
        assert_eq!(track(0, "Video", "Mpeg2").kind(), Some(StreamKind::Video));
        assert_eq!(track(1, "Audio", "DTS").kind(), Some(StreamKind::Audio));
        assert_eq!(track(2, "Subtitles", "HDMV PGS Subtitles").kind(), Some(StreamKind::Subtitles));
        assert_eq!(track(3, "Data", "Text").kind(), None);
        assert_eq!(track(4, "audio", "DTS").kind(), None);
        // audio_type is a layout name with no channel count (spec 8.1); each name the fixtures or MakeMKV use gives one.
        let layouts = [
            ("Mono", Some(1)),
            ("Stereo", Some(2)),
            ("Surround 5.0", Some(5)),
            ("Surround 5.1", Some(6)),
            ("Surround 6.1", Some(7)),
            ("Surround 7.1", Some(8)),
            ("Surround 9.1", None),
            ("stereo", None),
            ("", None),
        ];
        for (layout, channels) in layouts {
            let audio = DiscDbTrack { audio_type: Some(layout.to_string()), ..track(1, "Audio", "Dolby Digital") };
            assert_eq!(audio.channels(), channels, "{layout:?}");
        }
        assert_eq!(track(1, "Audio", "Dolby Digital").channels(), None);
    }

    #[test]
    fn a_title_gives_its_seconds_and_its_tracks_of_one_kind_in_index_order() {
        let title = DiscDbTitle {
            duration: Some("1:41:13".to_string()),
            tracks: vec![
                track(2, "Audio", "DTS"),
                track(0, "Video", "Mpeg2"),
                track(3, "Subtitles", "HDMV PGS Subtitles"),
                track(1, "Audio", "DTS-HD Master Audio"),
            ],
            ..bare_title(0)
        };
        assert_eq!(title.seconds(), Some(6073));
        let audio = title.tracks_of(StreamKind::Audio);
        assert_eq!(audio.iter().map(|track| track.index).collect::<Vec<u32>>(), vec![1, 2]);
        let video = title.tracks_of(StreamKind::Video);
        assert_eq!(video.iter().map(|track| track.index).collect::<Vec<u32>>(), vec![0]);
        let subtitles = title.tracks_of(StreamKind::Subtitles);
        assert_eq!(subtitles.iter().map(|track| track.index).collect::<Vec<u32>>(), vec![3]);
        assert_eq!(DiscDbTitle { duration: Some("101:13".to_string()), ..bare_title(0) }.seconds(), None);
        assert_eq!(bare_title(0).seconds(), None);
    }

    #[test]
    fn the_record_gives_its_kind_tmdb_identity_title_and_year_and_disc_number() {
        let record: DiscDbRecord = serde_json::from_str(FIREFLY_API_EXCERPT).unwrap();
        assert_eq!(record.kind(), Some(Kind::Tv));
        assert_eq!(record.tmdb_identity(), Some(TmdbIdentity { kind: TmdbKind::Tv, id: 1437 }));
        assert_eq!(record.title_and_year(), Some(("Firefly".to_string(), Some(2002))));
        assert_eq!(record.disc_number(), Some(2));

        let media = |media_type: &str, tmdb: Option<&str>| DiscDbMedia {
            title: Some("Knowing".to_string()),
            full_title: None,
            year: Some(2009),
            media_type: Some(media_type.to_string()),
            external_ids: DiscDbExternalIds { tmdb: tmdb.map(str::to_string), imdb: None },
        };
        let with = |media: Option<DiscDbMedia>| DiscDbRecord { media, ..bare_record() };
        let movie = with(Some(media("Movie", Some("13811"))));
        assert_eq!(movie.kind(), Some(Kind::Movie));
        assert_eq!(movie.tmdb_identity(), Some(TmdbIdentity { kind: TmdbKind::Movie, id: 13811 }));
        assert_eq!(movie.title_and_year(), Some(("Knowing".to_string(), Some(2009))));
        // A type other than Movie or Series states no kind, so its TMDb ID cannot be paired with one.
        let other = with(Some(media("Collection", Some("13811"))));
        assert_eq!((other.kind(), other.tmdb_identity()), (None, None));
        // An absent, empty, non-numeric or zero TMDb ID gives no identity; the kind still holds.
        for tmdb in [None, Some(""), Some("tt0448011"), Some("0")] {
            let record = with(Some(media("Movie", tmdb)));
            assert_eq!((record.kind(), record.tmdb_identity()), (Some(Kind::Movie), None), "{tmdb:?}");
        }
        // No media (the pinned form): no kind, no identity, no title for 8.1's search; the top-level Index is the disc.
        let pinned = DiscDbRecord { pinned_index: Some(1), ..bare_record() };
        assert_eq!(
            (pinned.kind(), pinned.tmdb_identity(), pinned.title_and_year(), pinned.disc_number()),
            (None, None, None, Some(1))
        );
        // disc.index wins over a pinned Index; a media without a title gives no title and year.
        let both = DiscDbRecord {
            disc: Some(DiscDbDisc { index: Some(3), name: None, slug: None }),
            pinned_index: Some(1),
            media: Some(DiscDbMedia { title: None, ..media("Movie", None) }),
            ..bare_record()
        };
        assert_eq!((both.disc_number(), both.title_and_year()), (Some(3), None));
        assert_eq!(bare_record().disc_number(), None);
    }

    #[test]
    fn main_movies_and_episodes_are_listed_in_index_order() {
        let item = |item_type: ItemType| Some(DiscDbItem { title: "x".to_string(), item_type, season: None, episode: None });
        let record = DiscDbRecord {
            titles: vec![
                DiscDbTitle { item: item(ItemType::Episode), ..bare_title(5) },
                DiscDbTitle { item: item(ItemType::Episode), ..bare_title(2) },
                DiscDbTitle { item: item(ItemType::MainMovie), ..bare_title(7) },
                DiscDbTitle { item: item(ItemType::Extra), ..bare_title(1) },
                bare_title(3),
                DiscDbTitle { item: item(ItemType::MainMovie), ..bare_title(0) },
            ],
            ..bare_record()
        };
        assert_eq!(indices(record.main_movies()), vec![0, 7]);
        assert_eq!(indices(record.episodes()), vec![2, 5]);
    }

    /// A path under the repository's corpus folder (tracked, so on every OS and in CI).
    fn corpus(path: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus").join(path)
    }

    /// A file's text, read lossily as `crates/core/tests/thediscdb_corpus.rs` reads the pinned logs.
    fn read(path: &Path) -> String {
        let bytes = fs::read(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        String::from_utf8_lossy(&bytes).into_owned()
    }

    /// Every answer of corpus/thediscdb-api in name order, as (file name, records): a dischash file's one record, a
    /// discid file's array.
    fn api_answers() -> Vec<(String, Vec<DiscDbRecord>)> {
        let dir = corpus("thediscdb-api");
        let mut names: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".json"))
            .collect();
        names.sort();
        names
            .into_iter()
            .map(|name| {
                let text = read(&dir.join(&name));
                let records = if name.starts_with("dischash-") {
                    parse_dischash(&text, &name).unwrap().into_iter().collect()
                } else {
                    parse_discid(&text, &name).unwrap()
                };
                (name, records)
            })
            .collect()
    }

    /// The dischash answer of one content hash.
    fn api_record(hash: &str) -> DiscDbRecord {
        let name = format!("dischash-{hash}.json");
        parse_dischash(&read(&corpus("thediscdb-api").join(&name)), &name).unwrap().unwrap()
    }

    /// The nine pinned logs, by stem under corpus/thediscdb, with their discNN.json's top-level Index, Name and
    /// ContentHash and their title count.
    const PINNED: [(&str, i64, &str, &str, usize); 9] = [
        ("movie/John Wick (2014)/4k-ultra-hd--blu-ray/disc01", 1, "4K Ultra HD", "76E63EBB5B66F73B0C37BFAB59B8ABA5", 10),
        ("movie/John Wick (2014)/4k-ultra-hd--blu-ray/disc02", 2, "Blu-ray", "BB9E6C963A4E2EAAB46C748CD4C00B69", 9),
        ("movie/Knowing (2009)/2010-dvd/disc01", 1, "DVD", "631BD77872C157D7C436E6F48F06A478", 7),
        ("movie/Smile (2022)/2022-4k/disc01", 1, "4K", "E826F0C64B04079EA83094AC046E8709", 236),
        (
            "movie/The Expendables 3 (2014)/the-expendables-3-film-collection/disc01",
            1,
            "The Expendables 3",
            "545411B47060B18FD7F8D94599F24708",
            64,
        ),
        ("series/Chernobyl (2019)/2020-4k-US/disc01", 1, "4K Disc One", "D11592C9B2D252937C22EDF01DBD1E05", 33),
        ("series/Chernobyl (2019)/2020-4k-US/disc02", 2, "4K Disc Two", "59796E40C7ABE16ACD9B259E732D392B", 31),
        ("series/Firefly (2002)/2003-dvd-the-complete-series/disc01", 1, "Disc 1", "5A961BC67EB34171F24852D3A7688020", 5),
        ("series/Firefly (2002)/2003-dvd-the-complete-series/disc02", 2, "Disc 2", "294A3E64C457A1BC9A0833335C090568", 6),
    ];

    /// A pinned discNN.json through parse_dischash (an object with a non-empty `Titles` is a hit).
    fn pinned_record(stem: &str) -> DiscDbRecord {
        let path = corpus("thediscdb").join(format!("{stem}.json"));
        parse_dischash(&read(&path), &path.display().to_string()).unwrap().unwrap()
    }

    #[test]
    fn parse_dischash_reads_one_object_and_returns_none_for_an_empty_body_or_a_non_hit() {
        // A miss is HTTP 404 with an empty body, which the recorder saves as an empty file (spec 8.1).
        for empty in ["", "\u{feff}", " \r\n"] {
            assert_eq!(parse_dischash(empty, "hash.json"), Ok(None), "{empty:?}");
        }
        assert_eq!(parse_dischash(r#"{"titles": []}"#, "hash.json"), Ok(None));
        assert_eq!(parse_dischash(r#"{"error": "not found"}"#, "hash.json"), Ok(None));
        assert_eq!(parse_dischash(r#""error""#, "hash.json"), Ok(None));
        assert_eq!(parse_dischash("[]", "hash.json"), Ok(None));
        let hit = parse_dischash("\u{feff}{\"content_hash\": \"ABC\", \"titles\": [{\"index\": 0}]}", "hash.json")
            .unwrap()
            .unwrap();
        assert_eq!((hit.content_hash.as_deref(), hit.titles.len(), hit.titles[0].index), (Some("ABC"), 1, 0));
        // The pinned form's capitalised Titles makes a hit too; an empty list of them does not.
        let pinned = parse_dischash(r#"{"Index": 1, "Name": "DVD", "Titles": [{"Index": 0}]}"#, "disc01.json")
            .unwrap()
            .unwrap();
        assert_eq!((pinned.pinned_index, pinned.pinned_name.as_deref(), pinned.titles.len()), (Some(1), Some("DVD"), 1));
        assert_eq!(parse_dischash(r#"{"Titles": []}"#, "disc01.json"), Ok(None));
        assert_eq!(
            parse_dischash(r#"[{"titles": [{"index": 0}]}]"#, "hash.json"),
            Err("hash.json: a dischash answer is one object, not an array".to_string())
        );
        let broken = parse_dischash("{\"titles\": [", "hash.json").unwrap_err();
        assert!(broken.starts_with("hash.json: EOF while parsing"), "{broken}");
        let mistyped = parse_dischash(r#"{"titles": [{"index": "zero"}]}"#, "hash.json").unwrap_err();
        assert!(mistyped.starts_with("hash.json: invalid type: string \"zero\""), "{mistyped}");
    }

    #[test]
    fn parse_discid_reads_an_array_and_an_empty_body_as_no_records() {
        assert_eq!(parse_discid("", "discid.json"), Ok(Vec::new()));
        assert_eq!(parse_discid("\u{feff}", "discid.json"), Ok(Vec::new()));
        assert_eq!(parse_discid("[]", "discid.json"), Ok(Vec::new()));
        let two = parse_discid(
            r#"[{"disc": {"index": 1}, "titles": [{"index": 0}]}, {"disc": {"index": 3}, "titles": []}]"#,
            "discid.json",
        )
        .unwrap();
        assert_eq!(two.iter().map(DiscDbRecord::disc_number).collect::<Vec<_>>(), vec![Some(1), Some(3)]);
        let object = parse_discid(r#"{"titles": [{"index": 0}]}"#, "discid.json").unwrap_err();
        assert!(object.starts_with("discid.json: invalid type: map"), "{object}");
    }

    #[test]
    fn every_api_answer_parses_and_names_its_own_hash_or_disc_id() {
        let answers = api_answers();
        assert_eq!(answers.len(), 18);
        for (name, records) in &answers {
            let key = name.trim_end_matches(".json");
            if let Some(hash) = key.strip_prefix("dischash-") {
                assert_eq!(records.len(), 1, "{name}");
                assert_eq!(records[0].content_hash.as_deref(), Some(hash), "{name}");
            } else {
                let id = key.strip_prefix("discid-").unwrap();
                let expected = if id == "B8C1624CF3BFCB2B49697266B06D308E29746B4A" { 2 } else { 1 };
                assert_eq!(records.len(), expected, "{name}");
                for record in records {
                    assert_eq!(record.global_disc_id.as_deref(), Some(id), "{name}");
                    // Each disc ID's record is the record of one of the nine fingerprints.
                    let hash = record.content_hash.as_deref().unwrap();
                    assert!(answers.iter().any(|(other, _)| *other == format!("dischash-{hash}.json")), "{name}");
                }
            }
        }
        let records: Vec<&DiscDbRecord> = answers.iter().flat_map(|(_, records)| records).collect();
        assert_eq!(records.len(), 19);
        assert_eq!(records.iter().map(|record| record.titles.len()).sum::<usize>(), 866);
    }

    #[test]
    fn the_api_answers_hold_the_field_values_spec_8_1_lists() {
        let answers = api_answers();
        let records: Vec<&DiscDbRecord> = answers.iter().flat_map(|(_, records)| records).collect();
        let formats: BTreeSet<&str> = records.iter().filter_map(|record| record.format.as_deref()).collect();
        assert_eq!(formats, BTreeSet::from(["Blu-Ray", "Blu-ray", "DVD", "UHD"]));
        let media_types: BTreeSet<&str> =
            records.iter().filter_map(|record| record.media.as_ref()?.media_type.as_deref()).collect();
        assert_eq!(media_types, BTreeSet::from(["Movie", "Series"]));
        let locales: BTreeSet<&str> =
            records.iter().filter_map(|record| record.release.as_ref()?.locale.as_deref()).collect();
        assert_eq!(locales, BTreeSet::from(["en-us"]));
        for record in &records {
            assert!(record.kind().is_some() && record.tmdb_identity().is_some(), "{:?}", record.content_hash);
            assert!(record.disc.is_some() && record.pinned_index.is_none(), "{:?}", record.content_hash);
        }

        let titles: Vec<&DiscDbTitle> = records.iter().flat_map(|record| &record.titles).collect();
        // file_name and item "only when TheDiscDB has described the title".
        assert!(titles.iter().all(|title| title.file_name.is_some() == title.item.is_some()));
        let mut item_types: BTreeMap<&str, usize> = BTreeMap::new();
        for item in titles.iter().filter_map(|title| title.item.as_ref()) {
            *item_types.entry(item.item_type.as_str()).or_default() += 1;
            // season and episode "where the item belongs to an episode": every one here is an Episode.
            let episode = item.item_type == ItemType::Episode;
            assert_eq!((item.season.is_some(), item.episode.is_some()), (episode, episode), "{}", item.title);
        }
        assert_eq!(
            item_types,
            BTreeMap::from([
                ("DeletedScene", 5),
                ("Episode", 24),
                ("Extra", 68),
                ("Featurette", 4),
                ("MainMovie", 11),
                ("Other", 6),
                ("Trailer", 2),
            ])
        );
        assert_eq!(titles.iter().map(|title| title.chapters.len()).sum::<usize>(), 224);

        let tracks: Vec<&DiscDbTrack> = titles.iter().flat_map(|title| &title.tracks).collect();
        let mut by_type: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
        for track in &tracks {
            let entry = by_type.entry(track.track_type.as_str()).or_default();
            entry.0 += 1;
            entry.1 += usize::from(track.description.is_some());
            // The fields of each kind of track as spec 8.1 lists them; every language code has three letters.
            let fields_hold = match track.kind() {
                Some(StreamKind::Video) => {
                    track.resolution.is_some()
                        && track.aspect_ratio.is_some()
                        && track.audio_type.is_none()
                        && track.language_code.is_none()
                }
                Some(StreamKind::Audio) => {
                    track.channels().is_some() && track.language.is_some() && track.resolution.is_none()
                }
                Some(StreamKind::Subtitles) => {
                    track.language.is_some() && track.audio_type.is_none() && track.resolution.is_none()
                }
                None => false,
            };
            assert!(fields_hold, "{track:?}");
            if track.kind() != Some(StreamKind::Video) {
                assert_eq!(track.language_code.as_deref().map(str::len), Some(3), "{track:?}");
            }
        }
        // (tracks, tracks with a description) per type: 58 of 837 audio and 30 of 2002 subtitle tracks here; the
        // owner's 2 recorded answers bring them to spec 8.1's 877 and 2018 (crates/dev/tests/lookup_answers.rs).
        assert_eq!(by_type, BTreeMap::from([("Audio", (837, 58)), ("Subtitles", (2002, 30)), ("Video", (866, 0))]));
    }

    #[test]
    fn the_api_answers_identify_the_nine_pinned_discs() {
        let expected: [(&str, Kind, i64, i64, &str, i32, &[u32], &[u32]); 9] = [
            ("294A3E64C457A1BC9A0833335C090568", Kind::Tv, 1437, 2, "Firefly", 2002, &[], &[0, 1, 2, 5]),
            ("545411B47060B18FD7F8D94599F24708", Kind::Movie, 138103, 1, "The Expendables 3", 2014, &[41], &[]),
            ("59796E40C7ABE16ACD9B259E732D392B", Kind::Tv, 87108, 2, "Chernobyl", 2019, &[], &[6, 8]),
            ("5A961BC67EB34171F24852D3A7688020", Kind::Tv, 1437, 1, "Firefly", 2002, &[], &[0, 1, 2]),
            ("631BD77872C157D7C436E6F48F06A478", Kind::Movie, 13811, 1, "Knowing", 2009, &[0], &[]),
            ("76E63EBB5B66F73B0C37BFAB59B8ABA5", Kind::Movie, 245891, 1, "John Wick", 2014, &[0], &[]),
            ("BB9E6C963A4E2EAAB46C748CD4C00B69", Kind::Movie, 245891, 2, "John Wick", 2014, &[0], &[]),
            ("D11592C9B2D252937C22EDF01DBD1E05", Kind::Tv, 87108, 1, "Chernobyl", 2019, &[], &[1, 7, 8]),
            ("E826F0C64B04079EA83094AC046E8709", Kind::Movie, 882598, 1, "Smile", 2022, &[14], &[]),
        ];
        for (hash, kind, tmdb, disc, title, year, main_movies, episodes) in expected {
            let record = api_record(hash);
            assert_eq!(record.kind(), Some(kind), "{hash}");
            assert_eq!(record.tmdb_identity(), Some(TmdbIdentity { kind: TmdbKind::from(kind), id: tmdb }), "{hash}");
            assert_eq!(record.disc_number(), Some(disc), "{hash}");
            assert_eq!(record.title_and_year(), Some((title.to_string(), Some(year))), "{hash}");
            assert_eq!(indices(record.main_movies()), main_movies, "{hash}");
            assert_eq!(indices(record.episodes()), episodes, "{hash}");
        }
    }

    #[test]
    fn the_expendables_disc_id_answer_holds_two_records_of_one_disc() {
        let name = "discid-B8C1624CF3BFCB2B49697266B06D308E29746B4A.json";
        let records = parse_discid(&read(&corpus("thediscdb-api").join(name)), name).unwrap();
        assert_eq!(records.len(), 2);
        for record in &records {
            assert_eq!(record.content_hash.as_deref(), Some("545411B47060B18FD7F8D94599F24708"));
            assert_eq!(record.titles.len(), 64);
        }
        // The same pressing listed twice in the film collection: disc 1 and disc 3, with the same titles.
        assert_eq!(records.iter().map(DiscDbRecord::disc_number).collect::<Vec<_>>(), vec![Some(1), Some(3)]);
        assert_eq!(records[0].titles, records[1].titles);
    }

    #[test]
    fn every_pinned_file_parses_with_its_disc_number_from_the_top_level_index() {
        let mut items = 0;
        for (stem, index, name, hash, titles) in PINNED {
            let record = pinned_record(stem);
            assert_eq!(record.disc_number(), Some(index), "{stem}");
            assert_eq!(record.pinned_name.as_deref(), Some(name), "{stem}");
            assert_eq!(record.content_hash.as_deref(), Some(hash), "{stem}");
            assert_eq!(record.titles.len(), titles, "{stem}");
            assert!(record.media.is_none() && record.release.is_none() && record.disc.is_none(), "{stem}");
            assert_eq!((record.kind(), record.tmdb_identity()), (None, None), "{stem}");
            assert!(corpus("thediscdb-api").join(format!("dischash-{hash}.json")).is_file(), "{stem}");
            for item in record.titles.iter().filter_map(|title| title.item.as_ref()) {
                assert_ne!(item.item_type, ItemType::Unknown, "{stem}: {}", item.title);
                items += 1;
            }
        }
        assert_eq!(items, 57);
    }

    #[test]
    fn each_pinned_file_agrees_with_its_api_answer_title_by_title() {
        // Without what only one form carries: the API's file_name, title resolution and chapters, the pinned comment.
        fn shared(mut title: DiscDbTitle) -> DiscDbTitle {
            title.comment = None;
            title.file_name = None;
            title.resolution = None;
            title.chapters = Vec::new();
            title
        }
        for (stem, _, _, hash, _) in PINNED {
            let pinned = pinned_record(stem);
            let api = api_record(hash);
            assert_eq!(pinned.format, api.format, "{stem}");
            assert_eq!(pinned.global_disc_id, api.global_disc_id, "{stem}");
            assert_eq!(pinned.disc_number(), api.disc_number(), "{stem}");
            assert_eq!(pinned.pinned_name, api.disc.as_ref().and_then(|disc| disc.name.clone()), "{stem}");
            let left: Vec<DiscDbTitle> = pinned.titles.into_iter().map(shared).collect();
            let right: Vec<DiscDbTitle> = api.titles.into_iter().map(shared).collect();
            assert_eq!(left, right, "{stem}");
        }
    }

    #[test]
    fn knowing_title_0_reads_its_flagged_commentary_track_and_its_chapters() {
        let record = api_record("631BD77872C157D7C436E6F48F06A478");
        let title = &record.titles[0];
        assert_eq!(
            (title.index, title.source_file.as_deref(), title.segment_map.as_deref(), title.seconds(), title.size),
            (0, Some("01"), Some("1-19"), Some(7274), Some(7098400768))
        );
        assert_eq!(
            (title.file_name.as_deref(), title.resolution.as_deref(), title.display_size.as_deref()),
            (Some("Knowing (2009) [480p].mkv"), Some("480p"), Some("6.6 GB"))
        );
        assert_eq!(title.item.as_ref().map(|item| (item.title.as_str(), item.item_type)), Some(("Knowing", ItemType::MainMovie)));
        assert_eq!((title.chapters.len(), title.chapters[0].title.as_deref()), (18, Some("50 Years Ago")));
        let video = title.tracks_of(StreamKind::Video);
        assert_eq!(
            (video.len(), video[0].name.as_deref(), video[0].resolution.as_deref(), video[0].aspect_ratio.as_deref()),
            (1, Some("Mpeg2"), Some("720x480"), Some("16:9"))
        );
        let audio_tracks = title.tracks_of(StreamKind::Audio);
        let audio: Vec<(u32, Option<u32>, Option<&str>, Option<&str>)> = audio_tracks
            .iter()
            .map(|track| (track.index, track.channels(), track.language_code.as_deref(), track.description.as_deref()))
            .collect();
        assert_eq!(
            audio,
            vec![
                (1, Some(6), Some("eng"), Some("ENglish")),
                (2, Some(6), Some("spa"), Some("Spanish")),
                (3, Some(2), Some("eng"), Some("Director Commentary")),
            ]
        );
        let subtitle_tracks = title.tracks_of(StreamKind::Subtitles);
        let subtitles: Vec<(u32, Option<&str>, Option<&str>)> = subtitle_tracks
            .iter()
            .map(|track| (track.index, track.name.as_deref(), track.language_code.as_deref()))
            .collect();
        assert_eq!(
            subtitles,
            vec![
                (4, Some("Dvd Subtitles"), Some("eng")),
                (5, Some("Dvd Subtitles"), Some("spa")),
                (6, Some("Dvd Subtitles"), Some("eng")),
                (7, Some("Dvd Subtitles"), Some("spa")),
            ]
        );
    }

    /// A database title with a source file, segment map, duration and size, and nothing else.
    fn db_title(index: u32, source_file: Option<&str>, segment_map: &str, duration: &str, size: Option<u64>) -> DiscDbTitle {
        DiscDbTitle {
            source_file: source_file.map(str::to_string),
            segment_map: Some(segment_map.to_string()),
            duration: Some(duration.to_string()),
            size,
            ..bare_title(index)
        }
    }

    /// A Blu-ray disc from (title number, TINFO attributes) rows, through `testsupport::scan_text`, `robot::parse` and
    /// `disc::disc`; no streams.
    fn scan_disc(titles: &[(u32, Vec<(u32, &str)>)]) -> Disc {
        let no_streams: &[(u32, &[(u32, &str)])] = &[];
        let rows: Vec<(u32, &[(u32, &str)], &[(u32, &[(u32, &str)])])> =
            titles.iter().map(|(number, attrs)| (*number, attrs.as_slice(), no_streams)).collect();
        disc(&parse(&scan_text(&[(1, "Blu-ray disc")], &rows))).unwrap()
    }

    /// The typed disc of a pinned log.
    fn pinned_disc(stem: &str) -> Disc {
        disc(&parse(&read(&corpus("thediscdb").join(format!("{stem}.txt"))))).unwrap()
    }

    #[test]
    fn a_blu_ray_title_matches_on_source_file_segment_map_size_and_duration() {
        let base: Vec<(u32, &'static str)> = vec![(9, "1:59:10"), (11, "55613952000"), (16, "00800.mpls"), (26, "6")];
        let with = |id: u32, value: &'static str| -> Vec<(u32, &'static str)> {
            base.iter().map(|&(key, old)| if key == id { (key, value) } else { (key, old) }).collect()
        };
        let scan = scan_disc(&[
            (0, base.clone()),
            (1, with(16, "00801.mpls")),
            (2, with(26, "7")),
            (3, with(11, "55613952001")),
            (4, with(9, "1:59:12")),
            (5, with(9, "1:59:08")),
            (6, with(9, "1:59:13")),
            (7, with(9, "1:59:07")),
        ]);
        let db = db_title(0, Some("00800.mpls"), "6", "1:59:10", Some(55613952000));
        let matching = |db: &DiscDbTitle| -> Vec<u32> {
            scan.titles.values().filter(|title| same_title(db, title)).map(|title| title.number).collect()
        };
        // Exact source file, segment map and size; the duration within 2 s either way (DURATION_MATCH_S).
        assert_eq!(matching(&db), vec![0, 4, 5]);
        // A database title without a size is matched on the other three.
        assert_eq!(matching(&DiscDbTitle { size: None, ..db.clone() }), vec![0, 3, 4, 5]);
        // A database title without a segment map compares as "": it matches a scan title without attribute 26 only.
        let unmapped = scan_disc(&[(0, vec![(9, "1:59:10"), (11, "55613952000"), (16, "00800.mpls")])]);
        let no_map = DiscDbTitle { segment_map: None, ..db.clone() };
        assert!(same_title(&no_map, &unmapped.titles[&0]));
        assert!(!same_title(&no_map, &scan.titles[&0]));
    }

    #[test]
    fn a_title_without_a_duration_on_either_side_never_matches() {
        let scan = scan_disc(&[
            (0, vec![(11, "100"), (16, "00001.m2ts"), (26, "1")]),
            (1, vec![(9, "0:10:00"), (11, "100"), (16, "00001.m2ts"), (26, "1")]),
        ]);
        let db = db_title(0, Some("00001.m2ts"), "1", "0:10:00", Some(100));
        assert!(!same_title(&db, &scan.titles[&0]));
        assert!(same_title(&db, &scan.titles[&1]));
        assert!(!same_title(&DiscDbTitle { duration: None, ..db.clone() }, &scan.titles[&1]));
        assert!(!same_title(&DiscDbTitle { duration: Some("10 min".to_string()), ..db }, &scan.titles[&1]));
    }

    #[test]
    fn a_truncated_segment_map_is_compared_by_its_untruncated_prefix() {
        let full = "280,284,299,277,292,276";
        let scan = scan_disc(&[
            (0, vec![(9, "2:11:31"), (11, "28831358976"), (16, "00756.mpls"), (26, full)]),
            (1, vec![(9, "2:11:31"), (11, "28831358976"), (16, "00756.mpls"), (26, "280,284,299,27...")]),
        ]);
        let truncated = db_title(41, Some("00756.mpls"), "280,284,299,27...", "2:11:31", Some(28831358976));
        assert!(same_title(&truncated, &scan.titles[&0]), "the database side truncated");
        let untruncated = db_title(41, Some("00756.mpls"), full, "2:11:31", Some(28831358976));
        assert!(same_title(&untruncated, &scan.titles[&1]), "the scan side truncated");
        let other_prefix = db_title(41, Some("00756.mpls"), "280,285,29...", "2:11:31", Some(28831358976));
        assert!(!same_title(&other_prefix, &scan.titles[&0]), "another prefix");
        // Source file, size and duration are still compared.
        assert!(!same_title(&DiscDbTitle { size: Some(28831358977), ..truncated.clone() }, &scan.titles[&0]));
        assert!(!same_title(&DiscDbTitle { source_file: Some("00757.mpls".to_string()), ..truncated.clone() }, &scan.titles[&0]));
        assert!(!same_title(&DiscDbTitle { duration: Some("2:11:34".to_string()), ..truncated }, &scan.titles[&0]));
    }

    #[test]
    fn a_dvd_title_matches_thediscdb_by_its_attribute_24() {
        let stem = "movie/Knowing (2009)/2010-dvd/disc01";
        let knowing = pinned_disc(stem);
        assert!(knowing.titles.values().all(|title| title.source_file.is_none()));
        for record in [pinned_record(stem), api_record("631BD77872C157D7C436E6F48F06A478")] {
            let matches = match_titles(&record, &knowing);
            assert_eq!(matches.get(&0), Some(&0));
            let three_to_six: Vec<Option<u32>> = (3..=6).map(|index| matches.get(&index).copied()).collect();
            assert_eq!(three_to_six, vec![Some(3), Some(4), Some(5), Some(6)]);
            // Titles 3 to 6 all carry segment map "1"; with the same map, size and duration, the DVD title number alone
            // decides.
            assert!(same_title(&record.titles[3], &knowing.titles[&3]));
            let renumbered = DiscDbTitle { source_file: Some("05".to_string()), ..record.titles[3].clone() };
            assert!(!same_title(&renumbered, &knowing.titles[&3]), "same map, size and duration, another DVD title number");
        }
        // Compared as numbers: TheDiscDB's "01" and "1" are MakeMKV's attribute 24 "01"; "02" is another title, and a
        // Blu-ray playlist name never matches a DVD title.
        let scan = scan_disc(&[(0, vec![(9, "2:01:14"), (11, "7098400768"), (24, "01"), (26, "1-19")])]);
        let cases = [(Some("01"), true), (Some("1"), true), (Some("02"), false), (Some("00001.mpls"), false), (None, false)];
        for (source, expected) in cases {
            let db = db_title(0, source, "1-19", "2:01:14", Some(7098400768));
            assert_eq!(same_title(&db, &scan.titles[&0]), expected, "{source:?}");
        }
    }

    #[test]
    fn a_title_with_neither_a_source_file_nor_a_dvd_title_never_matches() {
        let scan = scan_disc(&[(0, vec![(9, "0:43:48"), (11, "1966837760"), (26, "1-4,5-9,10-14,15")])]);
        assert!(scan.titles[&0].source_file.is_none() && scan.titles[&0].dvd_title.is_none());
        for source in [Some("11"), Some("00011.mpls"), None] {
            let db = db_title(5, source, "1-4,5-9,10-14,15", "0:43:48", Some(1966837760));
            assert!(!same_title(&db, &scan.titles[&0]), "{source:?}");
        }
        // A scan title with a source file never matches a database title without one.
        let blu_ray = scan_disc(&[(0, vec![(9, "0:43:48"), (11, "1966837760"), (16, "00011.mpls"), (26, "1-4,5-9,10-14,15")])]);
        assert!(!same_title(&db_title(5, None, "1-4,5-9,10-14,15", "0:43:48", Some(1966837760)), &blu_ray.titles[&0]));
    }

    #[test]
    fn match_titles_never_uses_the_title_numbers() {
        let scan = scan_disc(&[
            (0, vec![(9, "0:21:49"), (11, "4686133248"), (16, "00005.mpls"), (26, "7")]),
            (7, vec![(9, "0:21:18"), (11, "5168762880"), (16, "00006.mpls"), (26, "8")]),
        ]);
        let record = DiscDbRecord {
            titles: vec![
                db_title(0, Some("00006.mpls"), "8", "0:21:18", Some(5168762880)),
                db_title(7, Some("00009.mpls"), "9", "0:29:35", Some(7141834752)),
            ],
            ..bare_record()
        };
        // Database index 0 is scan title 7; database index 7 matches no scan title, though title 7 exists.
        assert_eq!(match_titles(&record, &scan), BTreeMap::from([(0, 7)]));
    }

    #[test]
    fn match_titles_takes_each_scan_title_once_in_database_index_order() {
        let twin: Vec<(u32, &str)> = vec![(9, "0:05:00"), (11, "1000"), (16, "00010.m2ts"), (26, "10")];
        let scan = scan_disc(&[(3, twin.clone()), (4, twin)]);
        let same = |index: u32| db_title(index, Some("00010.m2ts"), "10", "0:05:00", Some(1000));
        // Listed out of order: index 0 is matched first and takes title 3, index 1 takes title 4, index 2 finds none left.
        let record = DiscDbRecord { titles: vec![same(2), same(1), same(0)], ..bare_record() };
        assert_eq!(match_titles(&record, &scan), BTreeMap::from([(0, 3), (1, 4)]));
        // A database index the record repeats keeps its first match and takes no second scan title.
        let repeated = DiscDbRecord { titles: vec![same(0), same(0)], ..bare_record() };
        assert_eq!(match_titles(&repeated, &scan), BTreeMap::from([(0, 3)]));
    }

    #[test]
    fn john_wick_title_0_and_title_8_share_a_segment_map_and_are_told_apart_by_source_file() {
        let scan = pinned_disc("movie/John Wick (2014)/4k-ultra-hd--blu-ray/disc01");
        let (zero, eight) = (&scan.titles[&0], &scan.titles[&8]);
        assert_eq!((zero.segment_map.as_str(), zero.seconds, zero.size), ("1", Some(6073), Some(67757930496)));
        assert_eq!((eight.segment_map.as_str(), eight.seconds, eight.size), ("1", Some(6073), Some(67757930496)));
        assert_eq!((zero.source_file.as_deref(), eight.source_file.as_deref()), (Some("00001.mpls"), Some("00001.m2ts")));
        let record = api_record("76E63EBB5B66F73B0C37BFAB59B8ABA5");
        assert!(same_title(&record.titles[0], zero));
        assert!(!same_title(&record.titles[0], eight));
        let matches = match_titles(&record, &scan);
        assert_eq!((matches.get(&0), matches.get(&8)), (Some(&0), Some(&8)));
    }

    #[test]
    fn every_pinned_file_and_its_api_answer_match_every_title_to_the_scan_title_of_the_same_number() {
        // TheDiscDB's index is MakeMKV's title number on these logs, so the rule must find exactly that pairing; on the
        // owner's Office disc they differ (crates/dev/tests/lookup_answers.rs). Firefly disc 2's title 5 is DVD title "11".
        for (stem, _, _, hash, titles) in PINNED {
            let scan = pinned_disc(stem);
            for record in [pinned_record(stem), api_record(hash)] {
                let matches = match_titles(&record, &scan);
                assert_eq!(matches.len(), titles, "{stem}");
                assert!(matches.iter().all(|(index, number)| index == number), "{stem}: {matches:?}");
            }
        }
    }
}
