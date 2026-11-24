//! Menu maps, `core::menu` (menu maps design 5.4): the types of a `lazymkv menu map v1` file, its reader and the
//! writer's validator; `scan.rs` ties a map to a scanned disc (stream numbers, identity, the check, decide's per-title
//! block); `mpls.rs` reads a Blu-ray playlist's play items and stream tables; `role.rs` reads the role words of a
//! choice's label and the per-choice off-decisions file (design 5.4a); `tie.rs` ties a map's audio choices to a title's
//! scan streams through the title's playlist and reads each stream's role (design 5.4a). The format, its rules and this
//! increment's rulings M3-1 to M3-8 are `docs/menu-maps/format.md`.
//! A map says only what the menu calls a choice and which playlist and streams choosing it selects. The decision reads
//! only the confirmed roles of `tie.rs` (a commentary or an audio description with a strong tie, on a map whose
//! identity matches), and only when `decide --menu` gives a map (design 5.4a); nothing else here changes a verdict,
//! plan or decision. No I/O: `read` takes the file's text.

mod mpls;
mod role;
mod scan;
mod tie;
mod validate;

pub use mpls::*;
pub use role::*;
pub use scan::*;
pub use tie::*;
pub use validate::validate;

/// The format name a v1 map's `format` holds (design 5.1).
pub const FORMAT: &str = "lazymkv menu map v1";

/// The map's file name inside its map folder (design 5.1).
pub const FILE_NAME: &str = "menu-map.json";

/// Level `replayed` needs at least this many agreeing replays (design 5.1; the writer's `REPLAYS_NEEDED`).
pub const REPLAYS_NEEDED: i64 = 3;

/// The made-up map both readers test against (`fixtures/menu-map-v1.json`, a byte copy of the writer's fixture).
#[cfg(any(test, feature = "testsupport"))]
pub const FIXTURE: &str = include_str!("fixtures/menu-map-v1.json");

/// The shared role-word cases (`fixtures/menu-role-cases.json`, made-up texts with the measurement's own answers).
#[cfg(any(test, feature = "testsupport"))]
pub const ROLE_CASES: &str = include_str!("fixtures/menu-role-cases.json");

/// What a choice is for: a choice's `kind` (design 5.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Audio,
    Subtitle,
    Version,
    Bonus,
    Other,
}

impl Kind {
    /// Every kind in the format's order.
    pub const ALL: [Kind; 5] = [Kind::Audio, Kind::Subtitle, Kind::Version, Kind::Bonus, Kind::Other];

    /// The format's word for the kind (the serde spelling).
    pub fn word(self) -> &'static str {
        match self {
            Kind::Audio => "audio",
            Kind::Subtitle => "subtitle",
            Kind::Version => "version",
            Kind::Bonus => "bonus",
            Kind::Other => "other",
        }
    }
}

/// Where a choice sits: the main menu, the pop-up menu, or anywhere else (a choice's `surface`; design 5.1 and 5.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Surface {
    Main,
    Popup,
    Other,
}

impl Surface {
    /// Every surface in the format's order.
    pub const ALL: [Surface; 3] = [Surface::Main, Surface::Popup, Surface::Other];

    /// The format's word for the surface (the serde spelling).
    pub fn word(self) -> &'static str {
        match self {
            Surface::Main => "main",
            Surface::Popup => "popup",
            Surface::Other => "other",
        }
    }
}

/// How far a choice is confirmed, the ladder of design 5.1: `read` (one reader), `checked` (a second reader and its
/// replay agree), `replayed` (checked, and the deterministic replay agreed at least `REPLAYS_NEEDED` times). Ordered up
/// the ladder, so the lowest of several is their `min`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Read,
    Checked,
    Replayed,
}

impl Level {
    /// Every level up the ladder.
    pub const ALL: [Level; 3] = [Level::Read, Level::Checked, Level::Replayed];

    /// The format's word for the level (the serde spelling).
    pub fn word(self) -> &'static str {
        match self {
            Level::Read => "read",
            Level::Checked => "checked",
            Level::Replayed => "replayed",
        }
    }
}

/// When a choice takes effect: when Play is chosen afterwards, or at once (a choice's `applies`; design 5.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Applies {
    AtPlay,
    Immediately,
}

impl Applies {
    /// Both values in the format's order.
    pub const ALL: [Applies; 2] = [Applies::AtPlay, Applies::Immediately];

    /// The format's word for the value (the serde spelling).
    pub fn word(self) -> &'static str {
        match self {
            Applies::AtPlay => "at-play",
            Applies::Immediately => "immediately",
        }
    }
}

/// What playing after a choice selects (design 5.1): the playlist (five digits, as named on the disc), the audio
/// stream and the subtitle stream as the player numbers them (from 1; null for none), and whether the subtitle is
/// shown. With subtitles off the subtitle number is a leftover and names no stream (M3-4).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Effect {
    pub playlist: String,
    pub audio: Option<i64>,
    pub subtitle: Option<i64>,
    pub subtitle_shown: bool,
}

/// The check of play or of a choice (design 5.1): the second reading of the label (`same`, or what the second reader
/// read), the level, the replays and whether they agreed (null when there were none), whether the owner saw it on a
/// real player, and a note (empty when there is nothing to say).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Check {
    pub label_reread: String,
    pub level: Level,
    pub replays: i64,
    pub replays_equal: Option<bool>,
    pub player: bool,
    pub note: String,
}

/// The disc a map is of (design 5.1): its slug, the two identity values of the capture (`disc_id_sha1` is checked
/// against a recording, `listing_sig` only shown; M3-6), and its format (uhd, bd or dvd).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MapDisc {
    pub slug: String,
    pub disc_id_sha1: Option<String>,
    pub listing_sig: Option<String>,
    pub format: Option<String>,
}

/// How the menu was reached (design 5.1): the key-press script (null when not scripted), what had to be passed on the
/// way, and the seconds it took (null when not measured).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Start {
    pub script: Option<String>,
    pub passed: Vec<String>,
    pub seconds_to_menu: Option<f64>,
}

/// What plain Play does with no choice made (design 5.1). Script and effect are null only at level `read` with a
/// note.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Play {
    pub label: String,
    pub script: Option<String>,
    pub effect: Option<Effect>,
    pub check: Check,
}

/// One menu choice (design 5.1): its id (`c` and three digits), the section it sits in, the label as read and the
/// crop of it as drawn (null only at level `read`), its kind, surface, key-press script and when it applies, its
/// effect (null only at level `read` with a note), whether that effect differs from plain Play (null when the writer
/// could not compare; lazymkv checks it only when both effects are objects), and its check.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Choice {
    pub id: String,
    pub section: String,
    pub label: String,
    pub label_image: Option<String>,
    pub kind: Kind,
    pub surface: Surface,
    pub script: String,
    pub applies: Applies,
    pub effect: Option<Effect>,
    pub differs_from_play: Option<bool>,
    pub check: Check,
}

impl Choice {
    /// The label as a person is shown it: `SECTION: label`, or the label alone when the section is empty (design 5.3,
    /// the whole section; not the measurement's innermost heading).
    pub fn shown_label(&self) -> String {
        if self.section.is_empty() {
            self.label.clone()
        } else {
            format!("{}: {}", self.section, self.label)
        }
    }
}

/// A `lazymkv menu map v1` file (design 5.1). `made` and `quirks` are free-form for this reader; unknown keys are
/// read and dropped, so a later writer may add some.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MenuMap {
    pub format: String,
    pub disc: MapDisc,
    pub made: serde_json::Value,
    pub start: Start,
    pub play: Play,
    pub choices: Vec<Choice>,
    pub not_mapped: Vec<String>,
    pub quirks: Vec<serde_json::Value>,
}

/// The map in `text` (a file's whole text). A leading byte-order mark is dropped. Err is every sentence the reader
/// has: `not JSON: ...`; a map of another format version gets one sentence before any v1 rule speaks (M3-8); then
/// `validate`'s list, the writer's sentences; then, for a value the writer's rules leave open but this reader's types
/// do not (an ID or `disc.format` that is not a string or null, `label_reread` not a string, `replays` not an
/// integer, `replays_equal` or `differs_from_play` not true, false or null), one sentence naming serde's error.
pub fn read(text: &str) -> Result<MenuMap, Vec<String>> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let value: serde_json::Value = serde_json::from_str(text).map_err(|error| vec![format!("not JSON: {error}")])?;
    if let Some(format) = value.get("format").and_then(serde_json::Value::as_str) {
        if format != FORMAT {
            return Err(vec![format!(
                "this lazymkv reads only {}; the map says it is {}",
                crate::recording::py_repr_str(FORMAT),
                crate::recording::py_repr_str(format)
            )]);
        }
    }
    let errors = validate(&value);
    if !errors.is_empty() {
        return Err(errors);
    }
    serde_json::from_value(value)
        .map_err(|error| vec![format!("the map passes the format's rules but not this reader's: {error}")])
}

/// The writer's rule for two effects: the same playlist, the same audio and the same `subtitle_shown`; the subtitle
/// stream counts only when both show it (with subtitles off its number is a leftover). Design 5.1.
pub fn effects_equal(a: &Effect, b: &Effect) -> bool {
    a.playlist == b.playlist
        && a.audio == b.audio
        && a.subtitle_shown == b.subtitle_shown
        && (!a.subtitle_shown || a.subtitle == b.subtitle)
}

/// The map's own level, a summary only: the lowest of play's and every choice's; `Read` for a map with no choices. As
/// the writer's `map_level` counts it (design 5.1 names the choices only).
pub fn map_level(map: &MenuMap) -> Level {
    if map.choices.is_empty() {
        return Level::Read;
    }
    map.choices.iter().map(|choice| choice.check.level).fold(map.play.check.level, Level::min)
}

#[cfg(test)]
mod tests;
