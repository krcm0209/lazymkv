//! Spec 8.1's lookups, `core::lookup`. Task 3 holds the answers: TheDiscDB's in `discdb` (spec 8.1, last bullet), with
//! the match of database titles to scan titles (spec 8.3, first paragraph), and TMDb's in `tmdb` (spec 8.1: the search,
//! the record with its external IDs, a season, the recording's slim `tmdb.json`). This file holds `Kind`, the disc kind
//! of spec 8.2, here because 8.1's identification and runtime checks are aware of it, and the two private helpers every
//! parser shares. Task 4 adds `name` (the disc's own name) and the identification of spec 8.1. No I/O: every parser
//! takes the answer's text and a name to show in its errors. Used by Tasks 4 to 7, 11, 13 and 15.

pub mod discdb;
pub mod tmdb;

use std::str::FromStr;

/// The disc kind of spec 8.2 (TheDiscDB's "Movie" or "Series", or the structural decision); defined here because 8.1's
/// identification and runtime checks are kind-aware; `plan::kind` re-exports it. Spec 8.2. Def: T3. Use: T4 to T7,
/// T11, T13, T15.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Movie,
    Tv,
}

impl Kind {
    /// Both kinds in spec 8.2's order: a movie (the default), then TV.
    pub const ALL: [Kind; 2] = [Kind::Movie, Kind::Tv];

    /// "movie" or "tv", the serde spelling. Spec 8.2.
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Movie => "movie",
            Kind::Tv => "tv",
        }
    }
}

/// "movie" or "tv" exactly; any other text is refused with a sentence naming it. Spec 8.2.
impl FromStr for Kind {
    type Err = String;

    fn from_str(text: &str) -> Result<Kind, String> {
        Kind::ALL
            .into_iter()
            .find(|kind| kind.as_str() == text)
            .ok_or_else(|| format!("{text:?} is not a disc kind (movie or tv)"))
    }
}

/// Movie to Movie, Tv to Tv (the two enums name the same two kinds; `TmdbKind` is TMDb's numbering space). Spec 8.1.
/// Def: T3. Use: T4, T7, T13.
impl From<tmdb::TmdbKind> for Kind {
    fn from(kind: tmdb::TmdbKind) -> Kind {
        match kind {
            tmdb::TmdbKind::Movie => Kind::Movie,
            tmdb::TmdbKind::Tv => Kind::Tv,
        }
    }
}

/// Movie to Movie, Tv to Tv: the TMDb kind a disc of this kind is searched and paired as. Spec 8.1. Def: T3. Use: T4,
/// T7, T13.
impl From<Kind> for tmdb::TmdbKind {
    fn from(kind: Kind) -> tmdb::TmdbKind {
        match kind {
            Kind::Movie => tmdb::TmdbKind::Movie,
            Kind::Tv => tmdb::TmdbKind::Tv,
        }
    }
}

/// An answer's text without the one byte-order mark a Windows recorder may write before it (the recorder's PowerShell
/// jobs write UTF-8 with one, as `recording::read_json` allows for).
fn without_bom(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

/// An answer's JSON (after `without_bom`) as `T`; Err is serde_json's message after `shown` and ": ".
fn parse_json<T: serde::de::DeserializeOwned>(text: &str, shown: &str) -> Result<T, String> {
    serde_json::from_str(without_bom(text)).map_err(|error| format!("{shown}: {error}"))
}

#[cfg(test)]
mod tests;
