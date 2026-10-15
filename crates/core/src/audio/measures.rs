//! The measures of experiment 13 (spec 8.6's content rule): `analyse` compares a candidate's level series with the
//! main track's, frame by frame, as audio_similarity.py does, in the Python's loop order and in f64, so every measure
//! agrees with the Python's within 1e-9 (SMI within 1e-6, spec section 4). `Constants` are the module constants a
//! tuning run may change (`lazymkv-dev retune --set NAME=VALUE`).

use super::levels::{FLOOR, FRAME_S};
use crate::recording::py_float_repr;

/// 2 s windows; fixed by the level files, never set. Py: WIN.
pub const WIN: usize = 100;
/// 60 s blocks for the lag; never set. Py: BLOCK.
pub const BLOCK: usize = 3000;
/// 1.28 s modulation windows of 64 frames, fixed by the precomputed transform. Py: N_SMI.
pub const N_SMI: usize = 64;

/// The reverse run's measures kept as "rev_<name>", in this order. Py: REVERSE_KEYS.
pub const REVERSE_KEYS: [&str; 15] = [
    "overlap", "main_silent", "gap_s", "dc", "ag", "ag_s", "adl", "tenths", "tenths_comm", "tenths_ad", "smi",
    "smi_windows", "ls_db", "added_s", "gain_db",
];

/// A Python number that is an int or a float (a constant, a rule threshold, a --set value). Display is Python's
/// str() (py_float_repr for a float); serde keeps the kind (10 stays 10, 10.0 stays 10.0).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum Num {
    Int(i64),
    Float(f64),
}

impl Num {
    pub fn as_f64(self) -> f64 {
        match self {
            Num::Int(i) => i as f64,
            Num::Float(x) => x,
        }
    }
}

impl std::fmt::Display for Num {
    /// Python's str(): an int's digits, a float's repr ("0.7", "20.0", "1e-05").
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Num::Int(i) => write!(f, "{i}"),
            Num::Float(x) => f.write_str(&py_float_repr(*x)),
        }
    }
}

/// Whether a key holds an int or a float.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumKind {
    Int,
    Float,
}

/// Why `set` refused: the name is not one of NAMES, or an Int key was given a value that is not a whole number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetError {
    Unknown,
    NotWhole,
}

/// The measure constants a tuning run may change, with the Python's defaults; fields in CONSTANTS order, so the serde
/// form is the snapshot's "constants". Py: the module constants named by CONSTANTS.
#[allow(non_snake_case)]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Constants {
    /// A window whose levels correlate at least this much follows the main.
    pub MATCH_R: f64,
    /// Added sound must sit in a window that correlates less than this.
    pub MISMATCH_R: f64,
    /// Candidate active: within this many dB of its own 95th percentile.
    pub X_ACTIVE_BELOW: f64,
    /// Dialogue frame: the dialogue reference within this many dB of its own 95th percentile.
    pub DLG_BELOW: f64,
    /// A dialogue gap lasts at least this many frames ...
    pub GAP_MIN: i64,
    /// ... and loses this many frames at each end.
    pub GAP_TRIM: i64,
    /// Added: at least this many dB above the level the main predicts.
    pub ADDED_DB: f64,
    /// The main is digitally silent at or below this level.
    pub SILENT_DB: f64,
    /// Coarse lag search, +- this many 100 ms steps.
    pub COARSE_LAG: i64,
    /// Per-block lag search, +- this many frames around the global lag.
    pub BLOCK_LAG: i64,
    /// Per-window lag search, +- this many frames.
    pub WIN_LAG: i64,
    /// A lag (global or per block) is used only when the envelopes correlate at least this much.
    pub LAG_MIN_R: f64,
    /// The gain is this percentile of the candidate's level over the main's in matched windows.
    pub GAIN_PCT: f64,
    /// A tenth of the running time counts when it holds this many seconds of dialogue gaps.
    pub TENTH_GAP_S: f64,
    /// A counting tenth has commentary-like added sound at this AG or more (tenths_comm).
    pub TENTH_COMM_AG: f64,
    /// ... and audio-description-like added sound at this AG or more (tenths_ad).
    pub TENTH_AD_AG: f64,
}

impl Default for Constants {
    fn default() -> Constants {
        Constants {
            MATCH_R: 0.70,
            MISMATCH_R: 0.40,
            X_ACTIVE_BELOW: 20.0,
            DLG_BELOW: 25.0,
            GAP_MIN: 25,
            GAP_TRIM: 5,
            ADDED_DB: 10.0,
            SILENT_DB: -90.0,
            COARSE_LAG: 100,
            BLOCK_LAG: 100,
            WIN_LAG: 5,
            LAG_MIN_R: 0.5,
            GAIN_PCT: 0.75,
            TENTH_GAP_S: 10.0,
            TENTH_COMM_AG: 0.40,
            TENTH_AD_AG: 0.05,
        }
    }
}

/// 2**63, the first f64 above every i64.
const I64_END: f64 = 9_223_372_036_854_775_808.0;

impl Constants {
    /// The 16 names in CONSTANTS order, each with its kind. Py: CONSTANTS.
    pub const NAMES: [(&'static str, NumKind); 16] = [
        ("MATCH_R", NumKind::Float),
        ("MISMATCH_R", NumKind::Float),
        ("X_ACTIVE_BELOW", NumKind::Float),
        ("DLG_BELOW", NumKind::Float),
        ("GAP_MIN", NumKind::Int),
        ("GAP_TRIM", NumKind::Int),
        ("ADDED_DB", NumKind::Float),
        ("SILENT_DB", NumKind::Float),
        ("COARSE_LAG", NumKind::Int),
        ("BLOCK_LAG", NumKind::Int),
        ("WIN_LAG", NumKind::Int),
        ("LAG_MIN_R", NumKind::Float),
        ("GAIN_PCT", NumKind::Float),
        ("TENTH_GAP_S", NumKind::Float),
        ("TENTH_COMM_AG", NumKind::Float),
        ("TENTH_AD_AG", NumKind::Float),
    ];

    /// The value of a name exactly as NAMES spells it. Py: getattr(sim, name).
    pub fn get(&self, name: &str) -> Option<Num> {
        Some(match name {
            "MATCH_R" => Num::Float(self.MATCH_R),
            "MISMATCH_R" => Num::Float(self.MISMATCH_R),
            "X_ACTIVE_BELOW" => Num::Float(self.X_ACTIVE_BELOW),
            "DLG_BELOW" => Num::Float(self.DLG_BELOW),
            "GAP_MIN" => Num::Int(self.GAP_MIN),
            "GAP_TRIM" => Num::Int(self.GAP_TRIM),
            "ADDED_DB" => Num::Float(self.ADDED_DB),
            "SILENT_DB" => Num::Float(self.SILENT_DB),
            "COARSE_LAG" => Num::Int(self.COARSE_LAG),
            "BLOCK_LAG" => Num::Int(self.BLOCK_LAG),
            "WIN_LAG" => Num::Int(self.WIN_LAG),
            "LAG_MIN_R" => Num::Float(self.LAG_MIN_R),
            "GAIN_PCT" => Num::Float(self.GAIN_PCT),
            "TENTH_GAP_S" => Num::Float(self.TENTH_GAP_S),
            "TENTH_COMM_AG" => Num::Float(self.TENTH_COMM_AG),
            "TENTH_AD_AG" => Num::Float(self.TENTH_AD_AG),
            _ => return None,
        })
    }

    /// Sets a name exactly as NAMES spells it (the caller upper-cases and has refused a non-finite value, as
    /// parse_sets does). A Float key keeps the value as a float, whole or not ("SILENT_DB=-80" stores -80.0); an Int
    /// key takes int(value) of a whole value and refuses a fraction with NotWhole (a non-finite value, or one outside
    /// i64, is NotWhole too). Returns the value stored. Py: parse_sets' constant branch.
    pub fn set(&mut self, name: &str, value: f64) -> Result<Num, SetError> {
        let kind = Self::NAMES
            .iter()
            .find(|entry| entry.0 == name)
            .map(|entry| entry.1)
            .ok_or(SetError::Unknown)?;
        let num = match kind {
            NumKind::Float => Num::Float(value),
            NumKind::Int => {
                if !value.is_finite() || value != value.trunc() || !(-I64_END..I64_END).contains(&value) {
                    return Err(SetError::NotWhole);
                }
                Num::Int(value as i64)
            }
        };
        match (name, num) {
            ("MATCH_R", Num::Float(v)) => self.MATCH_R = v,
            ("MISMATCH_R", Num::Float(v)) => self.MISMATCH_R = v,
            ("X_ACTIVE_BELOW", Num::Float(v)) => self.X_ACTIVE_BELOW = v,
            ("DLG_BELOW", Num::Float(v)) => self.DLG_BELOW = v,
            ("GAP_MIN", Num::Int(v)) => self.GAP_MIN = v,
            ("GAP_TRIM", Num::Int(v)) => self.GAP_TRIM = v,
            ("ADDED_DB", Num::Float(v)) => self.ADDED_DB = v,
            ("SILENT_DB", Num::Float(v)) => self.SILENT_DB = v,
            ("COARSE_LAG", Num::Int(v)) => self.COARSE_LAG = v,
            ("BLOCK_LAG", Num::Int(v)) => self.BLOCK_LAG = v,
            ("WIN_LAG", Num::Int(v)) => self.WIN_LAG = v,
            ("LAG_MIN_R", Num::Float(v)) => self.LAG_MIN_R = v,
            ("GAIN_PCT", Num::Float(v)) => self.GAIN_PCT = v,
            ("TENTH_GAP_S", Num::Float(v)) => self.TENTH_GAP_S = v,
            ("TENTH_COMM_AG", Num::Float(v)) => self.TENTH_COMM_AG = v,
            ("TENTH_AD_AG", Num::Float(v)) => self.TENTH_AD_AG = v,
            _ => unreachable!("Constants::NAMES gives every name the kind of its field"),
        }
        Ok(num)
    }

    /// json.dumps({name: value for name in CONSTANTS}, sort_keys=True): the cache key's constants part, with
    /// Python's default separators ", " and ": " and Python's number spelling.
    pub fn canonical_json(&self) -> String {
        let mut names: Vec<&str> = Self::NAMES.iter().map(|entry| entry.0).collect();
        names.sort_unstable();
        let parts: Vec<String> = names
            .iter()
            .map(|name| format!("\"{name}\": {}", json_number(self.get(name).expect("a name of NAMES"))))
            .collect();
        format!("{{{}}}", parts.join(", "))
    }
}

/// A number as json.dumps spells it: an int's digits, a float's repr, and NaN and the infinities as json.dumps'
/// NaN, Infinity and -Infinity.
fn json_number(num: Num) -> String {
    match num {
        Num::Int(i) => i.to_string(),
        Num::Float(x) if x.is_nan() => "NaN".to_string(),
        Num::Float(x) if x.is_infinite() => (if x > 0.0 { "Infinity" } else { "-Infinity" }).to_string(),
        Num::Float(x) => py_float_repr(x),
    }
}

// ------------------------------------------------------------------------------------------------------------------
// The measures of one pair
// ------------------------------------------------------------------------------------------------------------------

/// The measures of one pair, every key of the Python dict as a typed field. The serde form writes the keys in the
/// Python's insertion order (frames, lag_ms, r_global, overlap, main_silent, dc, gain_db, gap_s, ag, ag_s, adl,
/// tenths, tenths_comm, tenths_ad, smi, smi_windows, ls_db, added_s, then rev_error and the 15 rev_ keys), NaN as
/// null, ints as ints; it reads null as NaN, "error" as the Error form, a "rev_tenths" key as a Block, "rev_ag"
/// without "rev_tenths" as Earlier. Py: analyse's dict.
#[derive(Clone, Debug, PartialEq)]
pub enum Measures {
    /// `{"frames": n, "error": "too short"}` from analyse; `{"error": e}` (frames None) from reverse_measures and
    /// swapped when the reverse run had one.
    Error { frames: Option<i64>, error: String },
    /// A measured pair.
    Run(Run),
}

impl Measures {
    /// The "error" key. Py: `"error" in m`, `m["error"]`.
    pub fn error(&self) -> Option<&str> {
        match self {
            Measures::Error { error, .. } => Some(error.as_str()),
            Measures::Run(_) => None,
        }
    }

    /// The measured pair, None for an Error.
    pub fn run(&self) -> Option<&Run> {
        match self {
            Measures::Run(run) => Some(run),
            Measures::Error { .. } => None,
        }
    }
}

/// The forward measures; a NaN f64 is the Python's float("nan").
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    /// Absent (None) in the dicts reverse_measures and swapped build.
    pub frames: Option<i64>,
    pub lag_ms: Option<i64>,
    pub r_global: Option<f64>,
    pub overlap: f64,
    pub main_silent: f64,
    /// NaN when the main track has no dialogue windows.
    pub dc: f64,
    pub gain_db: f64,
    pub gap_s: f64,
    pub ag: f64,
    pub ag_s: f64,
    pub adl: f64,
    pub tenths: i64,
    pub tenths_comm: i64,
    pub tenths_ad: i64,
    pub smi: f64,
    pub smi_windows: i64,
    pub ls_db: f64,
    pub added_s: f64,
    pub rev: Reverse,
}

impl Run {
    /// Every float NaN, every int 0, frames/lag_ms/r_global None, rev Absent: the base the ported Rule, ReverseRun and
    /// Report tests fill with struct update syntax, as the Python tests build partial dicts.
    pub fn blank() -> Run {
        let nan = f64::NAN;
        Run {
            frames: None,
            lag_ms: None,
            r_global: None,
            overlap: nan,
            main_silent: nan,
            dc: nan,
            gain_db: nan,
            gap_s: nan,
            ag: nan,
            ag_s: nan,
            adl: nan,
            tenths: 0,
            tenths_comm: 0,
            tenths_ad: 0,
            smi: nan,
            smi_windows: 0,
            ls_db: nan,
            added_s: nan,
            rev: Reverse::Absent,
        }
    }
}

/// The rev_ keys a measures dict carries.
#[derive(Clone, Debug, PartialEq)]
pub enum Reverse {
    /// analyse(reverse=False), and the dicts reverse_measures builds: no rev_ key.
    Absent,
    /// Measures written before 2026-10-06: rev_gap_s and rev_ag alone (the earlier reverse test of verdict).
    Earlier { rev_gap_s: f64, rev_ag: f64 },
    /// rev_error and the 15 REVERSE_KEYS.
    Block(RevBlock),
}

/// The reverse run as stored. When it had an error, rev_gap_s is 0.0 and every other value is NaN (the ints None), as
/// `back.get(k, 0.0 if k == "gap_s" else nan)` gives; with an empty rev_error every int is Some.
#[derive(Clone, Debug, PartialEq)]
pub struct RevBlock {
    pub rev_error: String,
    pub rev_overlap: f64,
    pub rev_main_silent: f64,
    pub rev_gap_s: f64,
    pub rev_dc: f64,
    pub rev_ag: f64,
    pub rev_ag_s: f64,
    pub rev_adl: f64,
    pub rev_tenths: Option<i64>,
    pub rev_tenths_comm: Option<i64>,
    pub rev_tenths_ad: Option<i64>,
    pub rev_smi: f64,
    pub rev_smi_windows: Option<i64>,
    pub rev_ls_db: f64,
    pub rev_added_s: f64,
    pub rev_gain_db: f64,
}

/// NaN as None, so that serde writes null (nan_to_none).
fn nan_null(x: f64) -> Option<f64> {
    if x.is_nan() {
        None
    } else {
        Some(x)
    }
}

impl serde::Serialize for Measures {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(None)?;
        match self {
            Measures::Error { frames, error } => {
                if let Some(n) = frames {
                    map.serialize_entry("frames", n)?;
                }
                map.serialize_entry("error", error)?;
            }
            Measures::Run(r) => {
                if let Some(v) = r.frames {
                    map.serialize_entry("frames", &v)?;
                }
                if let Some(v) = r.lag_ms {
                    map.serialize_entry("lag_ms", &v)?;
                }
                if let Some(v) = r.r_global {
                    map.serialize_entry("r_global", &nan_null(v))?;
                }
                map.serialize_entry("overlap", &nan_null(r.overlap))?;
                map.serialize_entry("main_silent", &nan_null(r.main_silent))?;
                map.serialize_entry("dc", &nan_null(r.dc))?;
                map.serialize_entry("gain_db", &nan_null(r.gain_db))?;
                map.serialize_entry("gap_s", &nan_null(r.gap_s))?;
                map.serialize_entry("ag", &nan_null(r.ag))?;
                map.serialize_entry("ag_s", &nan_null(r.ag_s))?;
                map.serialize_entry("adl", &nan_null(r.adl))?;
                map.serialize_entry("tenths", &r.tenths)?;
                map.serialize_entry("tenths_comm", &r.tenths_comm)?;
                map.serialize_entry("tenths_ad", &r.tenths_ad)?;
                map.serialize_entry("smi", &nan_null(r.smi))?;
                map.serialize_entry("smi_windows", &r.smi_windows)?;
                map.serialize_entry("ls_db", &nan_null(r.ls_db))?;
                map.serialize_entry("added_s", &nan_null(r.added_s))?;
                match &r.rev {
                    Reverse::Absent => {}
                    Reverse::Earlier { rev_gap_s, rev_ag } => {
                        map.serialize_entry("rev_gap_s", &nan_null(*rev_gap_s))?;
                        map.serialize_entry("rev_ag", &nan_null(*rev_ag))?;
                    }
                    Reverse::Block(b) => {
                        map.serialize_entry("rev_error", &b.rev_error)?;
                        map.serialize_entry("rev_overlap", &nan_null(b.rev_overlap))?;
                        map.serialize_entry("rev_main_silent", &nan_null(b.rev_main_silent))?;
                        map.serialize_entry("rev_gap_s", &nan_null(b.rev_gap_s))?;
                        map.serialize_entry("rev_dc", &nan_null(b.rev_dc))?;
                        map.serialize_entry("rev_ag", &nan_null(b.rev_ag))?;
                        map.serialize_entry("rev_ag_s", &nan_null(b.rev_ag_s))?;
                        map.serialize_entry("rev_adl", &nan_null(b.rev_adl))?;
                        map.serialize_entry("rev_tenths", &b.rev_tenths)?;
                        map.serialize_entry("rev_tenths_comm", &b.rev_tenths_comm)?;
                        map.serialize_entry("rev_tenths_ad", &b.rev_tenths_ad)?;
                        map.serialize_entry("rev_smi", &nan_null(b.rev_smi))?;
                        map.serialize_entry("rev_smi_windows", &b.rev_smi_windows)?;
                        map.serialize_entry("rev_ls_db", &nan_null(b.rev_ls_db))?;
                        map.serialize_entry("rev_added_s", &nan_null(b.rev_added_s))?;
                        map.serialize_entry("rev_gain_db", &nan_null(b.rev_gain_db))?;
                    }
                }
            }
        }
        map.end()
    }
}

impl<'de> serde::Deserialize<'de> for Measures {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Measures, D::Error> {
        let map = <serde_json::Map<String, serde_json::Value> as serde::Deserialize>::deserialize(deserializer)?;
        measures_from_map(&map).map_err(serde::de::Error::custom)
    }
}

type JsonMap = serde_json::Map<String, serde_json::Value>;

/// A float key's value: null is NaN.
fn float_value(key: &str, value: &serde_json::Value) -> Result<f64, String> {
    if value.is_null() {
        return Ok(f64::NAN);
    }
    value.as_f64().ok_or_else(|| format!("field `{key}`: expected a number or null, got {value}"))
}

/// A float key that must be there (Python's m[key]).
fn float_at(map: &JsonMap, key: &str) -> Result<f64, String> {
    match map.get(key) {
        None => Err(format!("missing field `{key}`")),
        Some(value) => float_value(key, value),
    }
}

/// A float key read with a default (Python's m.get(key, default)).
fn float_or(map: &JsonMap, key: &str, default: f64) -> Result<f64, String> {
    match map.get(key) {
        None => Ok(default),
        Some(value) => float_value(key, value),
    }
}

/// An int key that must be there and be an int.
fn int_at(map: &JsonMap, key: &str) -> Result<i64, String> {
    match map.get(key) {
        None => Err(format!("missing field `{key}`")),
        Some(value) => value.as_i64().ok_or_else(|| format!("field `{key}`: expected an integer, got {value}")),
    }
}

/// An int key that must be there and may be null (a reverse run's int after an error).
fn nullable_int_at(map: &JsonMap, key: &str) -> Result<Option<i64>, String> {
    match map.get(key) {
        None => Err(format!("missing field `{key}`")),
        Some(serde_json::Value::Null) => Ok(None),
        Some(value) => {
            value.as_i64().map(Some).ok_or_else(|| format!("field `{key}`: expected an integer or null, got {value}"))
        }
    }
}

/// An int key that may be absent or null (frames and lag_ms of the reverse_measures and swapped dicts).
fn optional_int_at(map: &JsonMap, key: &str) -> Result<Option<i64>, String> {
    match map.get(key) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(value) => value.as_i64().map(Some).ok_or_else(|| format!("field `{key}`: expected an integer, got {value}")),
    }
}

fn measures_from_map(map: &JsonMap) -> Result<Measures, String> {
    if let Some(error) = map.get("error") {
        let error = error.as_str().ok_or("field `error`: expected a string")?.to_string();
        return Ok(Measures::Error { frames: optional_int_at(map, "frames")?, error });
    }
    let rev = if map.contains_key("rev_tenths") {
        // m.get("rev_error") is falsy when it is absent or null
        let rev_error = match map.get("rev_error") {
            None | Some(serde_json::Value::Null) => String::new(),
            Some(value) => value.as_str().ok_or("field `rev_error`: expected a string")?.to_string(),
        };
        Reverse::Block(RevBlock {
            rev_error,
            rev_overlap: float_at(map, "rev_overlap")?,
            rev_main_silent: float_at(map, "rev_main_silent")?,
            rev_gap_s: float_at(map, "rev_gap_s")?,
            rev_dc: float_at(map, "rev_dc")?,
            rev_ag: float_at(map, "rev_ag")?,
            rev_ag_s: float_at(map, "rev_ag_s")?,
            rev_adl: float_at(map, "rev_adl")?,
            rev_tenths: nullable_int_at(map, "rev_tenths")?,
            rev_tenths_comm: nullable_int_at(map, "rev_tenths_comm")?,
            rev_tenths_ad: nullable_int_at(map, "rev_tenths_ad")?,
            rev_smi: float_at(map, "rev_smi")?,
            rev_smi_windows: nullable_int_at(map, "rev_smi_windows")?,
            rev_ls_db: float_at(map, "rev_ls_db")?,
            rev_added_s: float_at(map, "rev_added_s")?,
            rev_gain_db: float_at(map, "rev_gain_db")?,
        })
    } else if map.contains_key("rev_ag") {
        // verdict reads m.get("rev_gap_s", 0.0) and m["rev_ag"]
        Reverse::Earlier { rev_gap_s: float_or(map, "rev_gap_s", 0.0)?, rev_ag: float_at(map, "rev_ag")? }
    } else {
        Reverse::Absent
    };
    let r_global = match map.get("r_global") {
        None => None,
        Some(value) => Some(float_value("r_global", value)?),
    };
    Ok(Measures::Run(Run {
        frames: optional_int_at(map, "frames")?,
        lag_ms: optional_int_at(map, "lag_ms")?,
        r_global,
        overlap: float_at(map, "overlap")?,
        main_silent: float_at(map, "main_silent")?,
        dc: float_at(map, "dc")?,
        gain_db: float_at(map, "gain_db")?,
        gap_s: float_at(map, "gap_s")?,
        ag: float_at(map, "ag")?,
        ag_s: float_at(map, "ag_s")?,
        adl: float_at(map, "adl")?,
        tenths: int_at(map, "tenths")?,
        tenths_comm: int_at(map, "tenths_comm")?,
        tenths_ad: int_at(map, "tenths_ad")?,
        smi: float_at(map, "smi")?,
        smi_windows: int_at(map, "smi_windows")?,
        ls_db: float_at(map, "ls_db")?,
        added_s: float_at(map, "added_s")?,
        rev,
    }))
}

// ------------------------------------------------------------------------------------------------------------------
// Python's arithmetic, spelt out
// ------------------------------------------------------------------------------------------------------------------

/// Python 3.9's sum() of floats: left to right from 0.0.
fn py_sum(values: &[f64]) -> f64 {
    values.iter().fold(0.0, |acc, &v| acc + v)
}

/// Python's sorted() of floats: a stable ascending sort (equal values keep their order, -0.0 equals 0.0); a NaN,
/// which the level series never hold, sorts last.
fn py_sorted(values: &[f64]) -> Vec<f64> {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| match (a.is_nan(), b.is_nan()) {
        (false, false) => a.partial_cmp(b).expect("two numbers compare"),
        (false, true) => std::cmp::Ordering::Less,
        (true, false) => std::cmp::Ordering::Greater,
        (true, true) => std::cmp::Ordering::Equal,
    });
    sorted
}

/// int() of a float: truncation toward zero (saturating beyond i64, which min() and py_index treat as Python treats
/// the big int); Python's ValueError and OverflowError texts for NaN and the infinities.
fn py_trunc(x: f64) -> i64 {
    if x.is_nan() {
        panic!("cannot convert float NaN to integer");
    }
    if x.is_infinite() {
        panic!("cannot convert float infinity to integer");
    }
    x.trunc() as i64
}

/// A Python list index: a negative one counts from the end; None where Python raises IndexError.
fn py_index(i: i64, len: usize) -> Option<usize> {
    let j = if i < 0 { i.checked_add(len as i64)? } else { i };
    if j >= 0 && (j as usize) < len {
        Some(j as usize)
    } else {
        None
    }
}

/// Python's tuple `a > b` for (r, lag): r decides, an equal r is decided by the lag.
fn pair_gt(a: (f64, i64), b: (f64, i64)) -> bool {
    if a.0 == b.0 {
        a.1 > b.1
    } else {
        a.0 > b.0
    }
}

/// Python's max() of (r, lag) tuples: the first item, replaced by every later item that is greater.
fn py_max_pair(items: impl IntoIterator<Item = (f64, i64)>) -> (f64, i64) {
    let mut items = items.into_iter();
    let mut best = items.next().expect("max() arg is an empty sequence");
    for item in items {
        if pair_gt(item, best) {
            best = item;
        }
    }
    best
}

/// Python's max() of floats: the first item, replaced by every later item that is greater.
fn py_max(items: impl IntoIterator<Item = f64>) -> f64 {
    let mut items = items.into_iter();
    let mut best = items.next().expect("max() arg is an empty sequence");
    for item in items {
        if item > best {
            best = item;
        }
    }
    best
}

/// statistics.median of Python 3.9: the middle of the sorted values, or the mean of the two middle ones.
fn py_median(data: &[f64]) -> f64 {
    let sorted = py_sorted(data);
    let n = sorted.len();
    assert!(n > 0, "no median for empty data");
    if n % 2 == 1 {
        sorted[n / 2]
    } else {
        let i = n / 2;
        (sorted[i - 1] + sorted[i]) / 2.0
    }
}

/// C pow() as CPython's float `**` calls it. black_box keeps LLVM from rewriting pow(10, x) as exp10(x) and
/// pow(x, 2.0) as x * x; on the Mac the C library's pow(x, 2.0) differs from x * x in about 1 case in 750.
fn py_pow(base: f64, exponent: f64) -> f64 {
    std::hint::black_box(base).powf(std::hint::black_box(exponent))
}

// ------------------------------------------------------------------------------------------------------------------
// The measure helpers
// ------------------------------------------------------------------------------------------------------------------

/// Not NaN. Py: ok.
pub fn ok(v: f64) -> bool {
    !v.is_nan()
}

/// `ordered[min(len - 1, int(share * len))]` of the sorted values (a negative index counts from the end, as in
/// Python), NaN for none. Py: percentile.
pub fn percentile(values: &[f64], share: f64) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    let ordered = py_sorted(values);
    let len = ordered.len();
    let i = py_trunc(share * len as f64).min(len as i64 - 1);
    ordered[py_index(i, len).expect("list index out of range")]
}

/// Every level raised to at least the 95th percentile minus 45 dB. Py: clamp(values, below=45.0).
fn clamp(values: &[f64]) -> Vec<f64> {
    let floor = percentile(values, 0.95) - 45.0;
    // Python's max(v, floor): v unless floor is greater
    values.iter().map(|&v| if floor > v { floor } else { v }).collect()
}

/// Pearson's r of two equally long series; 0.0 for fewer than two values or a constant series. Py: pearson.
fn pearson(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len();
    if n < 2 {
        return 0.0;
    }
    let ma = py_sum(a) / n as f64;
    let mb = py_sum(b) / n as f64;
    let (mut cov, mut va, mut vb) = (0.0f64, 0.0f64, 0.0f64);
    for (x, y) in a.iter().zip(b) {
        let dx = x - ma;
        let dy = y - mb;
        cov += dx * dy;
        va += dx * dx;
        vb += dy * dy;
    }
    if va > 0.0 && vb > 0.0 {
        cov / (va * vb).sqrt()
    } else {
        0.0
    }
}

/// Correlation of a[i] and b[i + lag] for i in [i0, i1) in steps of `step`; -2.0 when fewer than 10 pairs exist.
/// Py: corr_at.
fn corr_at(a: &[f64], b: &[f64], i0: i64, i1: i64, lag: i64, step: usize) -> f64 {
    let lo = i0.max(-lag);
    let hi = i1.min(b.len() as i64 - lag);
    if hi - lo < 10 {
        return -2.0;
    }
    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<f64> = Vec::new();
    for i in (lo..hi).step_by(step) {
        xs.push(a[i as usize]);
        ys.push(b[(i + lag) as usize]);
    }
    pearson(&xs, &ys)
}

/// _TW[k][n] = cmath.exp(-2j * math.pi * k * n / N_SMI) for k in 0..33, n in 0..64, as (re, im). CPython 3.9 forms
/// the exponent as (0.0, theta) with theta = (0.0 + (0.0 + (-2 * pi) * k) * n) / 64 (complex times int and complex
/// over int go through full complex arithmetic, which is where the `0.0 +` come from), and exp((0.0, theta)) is
/// (cos theta, sin theta). Py: _TW.
fn twiddles() -> &'static [[(f64, f64); N_SMI]; 33] {
    static TW: std::sync::OnceLock<[[(f64, f64); N_SMI]; 33]> = std::sync::OnceLock::new();
    TW.get_or_init(|| {
        let mut tw = [[(0.0f64, 0.0f64); N_SMI]; 33];
        for (k, row) in tw.iter_mut().enumerate() {
            for (n, entry) in row.iter_mut().enumerate() {
                let theta = (0.0 + (0.0 + (-2.0 * std::f64::consts::PI) * k as f64) * n as f64) / N_SMI as f64;
                *entry = (theta.cos(), theta.sin());
            }
        }
        tw
    })
}

/// Share of the linear envelope's modulation energy at 2.3 to 7.8 Hz: DFT bins 3 to 10 of 1 to 32 over 64 frames of
/// 20 ms, each bin summed n = 0..63 in order with the precomputed twiddles, |X_k| ** 2 by C pow; 0.0 when the total
/// is not positive. The mean is the sum of every given level over 64 and the first 64 are transformed, as the Python
/// does. Py: smi.
pub fn smi(levels_db: &[f64]) -> f64 {
    let lin: Vec<f64> = levels_db.iter().map(|&v| py_pow(10.0, v / 20.0)).collect();
    let mean = py_sum(&lin) / N_SMI as f64;
    let x: Vec<f64> = lin.iter().map(|&v| v - mean).collect();
    assert!(x.len() >= N_SMI, "list index out of range");
    let tw = twiddles();
    let mut e = [0.0f64; 33];
    for (k, energy) in e.iter_mut().enumerate() {
        // sum() of complex numbers from (0.0, 0.0); each term is CPython 3.9's float * complex, (x + 0j) * (c + sj)
        let (mut re, mut im) = (0.0f64, 0.0f64);
        for n in 0..N_SMI {
            let (c, s) = tw[k][n];
            re += x[n] * c - 0.0 * s;
            im += x[n] * s + 0.0 * c;
        }
        *energy = py_pow(re.hypot(im), 2.0);
    }
    let total = py_sum(&e[1..33]);
    if total > 0.0 {
        py_sum(&e[3..11]) / total
    } else {
        0.0
    }
}

// ------------------------------------------------------------------------------------------------------------------
// analyse
// ------------------------------------------------------------------------------------------------------------------

/// The measures of `cand` against `main` (levels in dB per 20 ms frame), in the Python's loop order: clamp, coarse
/// and fine lag, block lags, gaps, windows, DC, gain, added, tenths, SMI, LS, ADL; with `reverse`, the reverse run
/// analyse(cand, main, None, false) as the rev_ block. `dialogue` is the centre channel; None or Some(empty) means
/// `main` (`dialogue or main`). Error "too short" when either series has fewer than 10 windows. Panics with the
/// Python's exception text where the Python raised (only for --set values outside their sense: a negative lag
/// constant, a negative GAP_TRIM, a GAIN_PCT outside 0 to 1, a TENTH_GAP_S at or below 0). Py: analyse.
pub fn analyse(main: &[f64], cand: &[f64], dialogue: Option<&[f64]>, reverse: bool, consts: &Constants) -> Measures {
    let c = consts;
    let dialogue: &[f64] = match dialogue {
        Some(d) if !d.is_empty() => d,
        _ => main,
    };
    let n = main.len();
    if n < 10 * WIN || cand.len() < 10 * WIN {
        return Measures::Error { frames: Some(n as i64), error: "too short".to_string() };
    }
    let mc = clamp(main);
    let xc = clamp(cand);
    // Lag: tracks of one title share a timeline. Coarse +-10 s on 100 ms means, fine +-100 ms; kept only when the two
    // envelopes follow each other (r >= LAG_MIN_R), otherwise 0. Then per 60 s block +-2 s, kept when r >= LAG_MIN_R.
    let mk: Vec<f64> = (0..n - 4).step_by(5).map(|i| py_sum(&mc[i..i + 5]) / 5.0).collect();
    let xk: Vec<f64> = (0..xc.len() - 4).step_by(5).map(|i| py_sum(&xc[i..i + 5]) / 5.0).collect();
    let (_, lag_k) = py_max_pair(
        (-c.COARSE_LAG..c.COARSE_LAG + 1).map(|l| (corr_at(&mk, &xk, 0, mk.len() as i64, l, 2), l)),
    );
    let (r_glob, fine) =
        py_max_pair((5 * lag_k - 5..5 * lag_k + 6).map(|l| (corr_at(&mc, &xc, 0, n as i64, l, 5), l)));
    let lag = if r_glob < c.LAG_MIN_R { 0 } else { fine };
    let n_blocks = n.div_ceil(BLOCK);
    let mut block_lag: Vec<i64> = Vec::with_capacity(n_blocks);
    for b in 0..n_blocks {
        let (b0, b1) = (b * BLOCK, n.min((b + 1) * BLOCK));
        let (r, lb) = py_max_pair(
            (lag - c.BLOCK_LAG..lag + c.BLOCK_LAG + 1)
                .step_by(2)
                .map(|l| (corr_at(&mc, &xc, b0 as i64, b1 as i64, l, 4), l)),
        );
        block_lag.push(if r >= c.LAG_MIN_R { lb } else { lag });
    }
    let lag_of = |t: usize| block_lag[t / BLOCK];

    let n_cand = cand.len() as i64;
    let inside: Vec<bool> = (0..n).map(|t| (0..n_cand).contains(&(t as i64 + lag_of(t)))).collect();
    let frames: Vec<usize> = (0..n).filter(|&t| inside[t]).collect();
    let xa: Vec<f64> =
        (0..n).map(|t| if inside[t] { cand[(t as i64 + lag_of(t)) as usize] } else { FLOOR }).collect();
    let xa_inside: Vec<f64> = frames.iter().map(|&t| xa[t]).collect();
    let x95 = percentile(&xa_inside, 0.95);
    let m95 = percentile(&frames.iter().map(|&t| main[t]).collect::<Vec<f64>>(), 0.95);
    let d95 = percentile(
        &frames.iter().filter(|&&t| t < dialogue.len()).map(|&t| dialogue[t]).collect::<Vec<f64>>(),
        0.95,
    );
    let silent: Vec<bool> = main.iter().map(|&v| v <= c.SILENT_DB).collect();
    let active: Vec<bool> = (0..n).map(|t| inside[t] && xa[t] >= x95 - c.X_ACTIVE_BELOW).collect();
    let dlg: Vec<bool> =
        (0..n).map(|t| inside[t] && t < dialogue.len() && dialogue[t] >= d95 - c.DLG_BELOW).collect();

    let mut gap = vec![false; n];
    let mut t = 0usize;
    while t < n {
        if inside[t] && !dlg[t] {
            let mut u = t;
            while u < n && inside[u] && !dlg[u] {
                u += 1;
            }
            if (u - t) as i64 >= c.GAP_MIN {
                for k in (t as i64 + c.GAP_TRIM)..(u as i64 - c.GAP_TRIM) {
                    // gap[k] = not silent[k], with Python's negative indices
                    let i = py_index(k, n).expect("list index out of range");
                    gap[i] = !silent[i];
                }
            }
            t = u;
        } else {
            t += 1;
        }
    }

    let wins: Vec<usize> = (0..n - WIN + 1).step_by(WIN).filter(|&w| inside[w..w + WIN].iter().all(|&b| b)).collect();
    // rw: the best window correlation over +-WIN_LAG frames, by window number (w / WIN); None for a window not in wins
    let mut rw: Vec<Option<f64>> = vec![None; n / WIN + 1];
    for &w in &wins {
        rw[w / WIN] = Some(py_max(
            (-c.WIN_LAG..c.WIN_LAG + 1).map(|d| corr_at(&mc, &xc, w as i64, (w + WIN) as i64, lag_of(w) + d, 1)),
        ));
    }
    let rw_at = |w: usize| rw[w / WIN].expect("a window of wins has its correlation");
    let dwins: Vec<usize> =
        wins.iter().copied().filter(|&w| dlg[w..w + WIN].iter().filter(|&&b| b).count() >= WIN / 2).collect();
    let dc = if dwins.is_empty() {
        f64::NAN
    } else {
        dwins.iter().filter(|&&w| rw_at(w) >= c.MATCH_R).count() as f64 / dwins.len() as f64
    };
    let matched: Vec<usize> = wins.iter().copied().filter(|&w| rw_at(w) >= c.MATCH_R).collect();
    let mframes: Vec<usize> = matched
        .iter()
        .flat_map(|&w| w..w + WIN)
        .filter(|&u| main[u] >= m95 - 30.0 && xa[u] > FLOOR)
        .collect();
    let gain = if matched.len() >= std::cmp::max(10, wins.len() / 20) && !mframes.is_empty() {
        percentile(&mframes.iter().map(|&u| xa[u] - main[u]).collect::<Vec<f64>>(), c.GAIN_PCT)
    } else {
        x95 - m95
    };
    let added: Vec<bool> = (0..n)
        .map(|u| active[u] && xa[u] - gain - main[u] >= c.ADDED_DB && rw[u / WIN].unwrap_or(1.0) < c.MISMATCH_R)
        .collect();

    let n_gap = gap.iter().filter(|&&g| g).count();
    let n_ag = (0..n).filter(|&u| gap[u] && added[u]).count();
    let n_dlg = dlg.iter().filter(|&&b| b).count();
    let lo = *frames.first().expect("list index out of range");
    let hi = frames[frames.len() - 1] + 1;
    let mut tenths: Vec<f64> = Vec::new();
    for k in 0..10usize {
        let a = lo + k * (hi - lo) / 10;
        let b = lo + (k + 1) * (hi - lo) / 10;
        let g = gap[a..b].iter().filter(|&&x| x).count();
        if g as f64 * FRAME_S >= c.TENTH_GAP_S {
            if g == 0 {
                panic!("division by zero");
            }
            tenths.push((a..b).filter(|&u| gap[u] && added[u]).count() as f64 / g as f64);
        }
    }
    let s: Vec<f64> = (0..n - N_SMI)
        .step_by(N_SMI / 2)
        .filter(|&t0| added[t0..t0 + N_SMI].iter().filter(|&&b| b).count() >= N_SMI / 2)
        .map(|t0| smi(&xa[t0..t0 + N_SMI]))
        .collect();

    let rev = if reverse {
        Reverse::Block(rev_block(&analyse(cand, main, None, false, consts)))
    } else {
        Reverse::Absent
    };
    Measures::Run(Run {
        frames: Some(n as i64),
        lag_ms: Some(lag * 20),
        r_global: Some(r_glob),
        overlap: frames.len() as f64 / n as f64,
        main_silent: frames.iter().filter(|&&u| silent[u]).count() as f64 / frames.len() as f64,
        dc,
        gain_db: gain,
        gap_s: n_gap as f64 * FRAME_S,
        ag: if n_gap > 0 { n_ag as f64 / n_gap as f64 } else { f64::NAN },
        ag_s: n_ag as f64 * FRAME_S,
        adl: if n_dlg > 0 {
            (0..n).filter(|&u| dlg[u] && added[u]).count() as f64 / n_dlg as f64
        } else {
            f64::NAN
        },
        tenths: tenths.len() as i64,
        tenths_comm: tenths.iter().filter(|&&v| v >= c.TENTH_COMM_AG).count() as i64,
        tenths_ad: tenths.iter().filter(|&&v| v >= c.TENTH_AD_AG).count() as i64,
        smi: if s.is_empty() { f64::NAN } else { py_median(&s) },
        smi_windows: s.len() as i64,
        ls_db: x95 - percentile(&xa_inside, 0.50),
        added_s: added.iter().filter(|&&b| b).count() as f64 * FRAME_S,
        rev,
    })
}

/// A forward run's values as a reverse block (rev_error "").
fn block_of(r: &Run) -> RevBlock {
    RevBlock {
        rev_error: String::new(),
        rev_overlap: r.overlap,
        rev_main_silent: r.main_silent,
        rev_gap_s: r.gap_s,
        rev_dc: r.dc,
        rev_ag: r.ag,
        rev_ag_s: r.ag_s,
        rev_adl: r.adl,
        rev_tenths: Some(r.tenths),
        rev_tenths_comm: Some(r.tenths_comm),
        rev_tenths_ad: Some(r.tenths_ad),
        rev_smi: r.smi,
        rev_smi_windows: Some(r.smi_windows),
        rev_ls_db: r.ls_db,
        rev_added_s: r.added_s,
        rev_gain_db: r.gain_db,
    }
}

/// analyse's rev_ keys from the reverse run: `m["rev_error"] = back.get("error", "")` and
/// `m["rev_" + k] = back.get(k, 0.0 if k == "gap_s" else nan)`.
fn rev_block(back: &Measures) -> RevBlock {
    match back {
        Measures::Run(b) => block_of(b),
        Measures::Error { error, .. } => {
            let nan = f64::NAN;
            RevBlock {
                rev_error: error.clone(),
                rev_overlap: nan,
                rev_main_silent: nan,
                rev_gap_s: 0.0,
                rev_dc: nan,
                rev_ag: nan,
                rev_ag_s: nan,
                rev_adl: nan,
                rev_tenths: None,
                rev_tenths_comm: None,
                rev_tenths_ad: None,
                rev_smi: nan,
                rev_smi_windows: None,
                rev_ls_db: nan,
                rev_added_s: nan,
                rev_gain_db: nan,
            }
        }
    }
}

/// A reverse block as its own forward measures (the 15 keys, no frames, lag_ms or r_global). An int the block lacks
/// with an empty rev_error, which analyse never writes, reads as 0.
fn run_of_block(b: &RevBlock) -> Run {
    Run {
        frames: None,
        lag_ms: None,
        r_global: None,
        overlap: b.rev_overlap,
        main_silent: b.rev_main_silent,
        dc: b.rev_dc,
        gain_db: b.rev_gain_db,
        gap_s: b.rev_gap_s,
        ag: b.rev_ag,
        ag_s: b.rev_ag_s,
        adl: b.rev_adl,
        tenths: b.rev_tenths.unwrap_or(0),
        tenths_comm: b.rev_tenths_comm.unwrap_or(0),
        tenths_ad: b.rev_tenths_ad.unwrap_or(0),
        smi: b.rev_smi,
        smi_windows: b.rev_smi_windows.unwrap_or(0),
        ls_db: b.rev_ls_db,
        added_s: b.rev_added_s,
        rev: Reverse::Absent,
    }
}

/// The reverse run (the candidate as reference, the main stream as candidate) as its own measures: None when `m`
/// carries no reverse block ("rev_tenths" not in m: an Error, analyse(reverse=False), the earlier form); the Error
/// form `{"error": rev_error}` when the reverse run had one. Py: reverse_measures.
pub fn reverse_measures(m: &Measures) -> Option<Measures> {
    let block = match m {
        Measures::Run(Run { rev: Reverse::Block(block), .. }) => block,
        _ => return None,
    };
    if !block.rev_error.is_empty() {
        return Some(Measures::Error { frames: None, error: block.rev_error.clone() });
    }
    Some(Measures::Run(run_of_block(block)))
}

/// The measures of the same pair with the roles exchanged: the reverse run becomes the forward one and the forward
/// run the reverse block (frames, lag_ms and r_global left out). None for an Error or a measures without a reverse
/// block; the Error form when the reverse run had one. Py: swapped.
pub fn swapped(m: &Measures) -> Option<Measures> {
    let back = reverse_measures(m)?;
    let forward = m.run()?;
    match back {
        Measures::Error { error, .. } => Some(Measures::Error { frames: None, error }),
        Measures::Run(mut run) => {
            run.rev = Reverse::Block(block_of(forward));
            Some(Measures::Run(run))
        }
    }
}

#[cfg(test)]
mod constants_tests {
    //! New tests: the measure constants, their --set semantics (audio_dataset.parse_sets' constant branch) and the
    //! cache key's constants JSON (Measurer's json.dumps(..., sort_keys=True)), against Python 3.9.6's output.
    use super::*;

    #[test]
    fn constants_have_the_pythons_defaults_kinds_and_order() {
        let c = Constants::default();
        assert_eq!(
            serde_json::to_string(&c).unwrap(),
            r#"{"MATCH_R":0.7,"MISMATCH_R":0.4,"X_ACTIVE_BELOW":20.0,"DLG_BELOW":25.0,"GAP_MIN":25,"GAP_TRIM":5,"ADDED_DB":10.0,"SILENT_DB":-90.0,"COARSE_LAG":100,"BLOCK_LAG":100,"WIN_LAG":5,"LAG_MIN_R":0.5,"GAIN_PCT":0.75,"TENTH_GAP_S":10.0,"TENTH_COMM_AG":0.4,"TENTH_AD_AG":0.05}"#
        );
        let names: Vec<&str> = Constants::NAMES.iter().map(|entry| entry.0).collect();
        assert_eq!(
            names,
            vec![
                "MATCH_R", "MISMATCH_R", "X_ACTIVE_BELOW", "DLG_BELOW", "GAP_MIN", "GAP_TRIM", "ADDED_DB", "SILENT_DB",
                "COARSE_LAG", "BLOCK_LAG", "WIN_LAG", "LAG_MIN_R", "GAIN_PCT", "TENTH_GAP_S", "TENTH_COMM_AG",
                "TENTH_AD_AG",
            ]
        );
        for (name, kind) in Constants::NAMES {
            match (kind, c.get(name)) {
                (NumKind::Int, Some(Num::Int(_))) | (NumKind::Float, Some(Num::Float(_))) => {}
                other => panic!("{name}: {other:?}"),
            }
        }
        assert_eq!(c.get("FRAME_MS"), None);
        assert_eq!(serde_json::from_str::<Constants>(&serde_json::to_string(&c).unwrap()).unwrap(), c);
    }

    #[test]
    fn set_takes_whole_numbers_for_int_constants() {
        let mut c = Constants::default();
        assert_eq!(c.set("GAP_MIN", 30.0), Ok(Num::Int(30)));
        assert_eq!(c.GAP_MIN, 30);
        assert_eq!(c.set("GAP_MIN", 30.5), Err(SetError::NotWhole));
        assert_eq!(c.GAP_MIN, 30);
        assert_eq!(c.set("GAP_TRIM", -0.0), Ok(Num::Int(0)));
        assert_eq!(c.set("WIN_LAG", f64::INFINITY), Err(SetError::NotWhole));
        assert_eq!(c.set("BLOCK_LAG", 1e30), Err(SetError::NotWhole));
        assert_eq!(c.set("SILENT_DB", -80.0), Ok(Num::Float(-80.0)));
        assert_eq!(c.SILENT_DB, -80.0);
        assert_eq!(c.set("MATCH_R", 1.0), Ok(Num::Float(1.0)));
        assert_eq!(c.get("MATCH_R"), Some(Num::Float(1.0)));
        // the caller upper-cases; FRAME_MS, WIN, BLOCK and N_SMI are fixed; rule keys are Rule's
        assert_eq!(c.set("silent_db", -80.0), Err(SetError::Unknown));
        assert_eq!(c.set("FRAME_MS", 20.0), Err(SetError::Unknown));
        assert_eq!(c.set("dc_max", 0.25), Err(SetError::Unknown));
    }

    #[test]
    fn canonical_json_is_json_dumps_with_sort_keys() {
        let mut c = Constants::default();
        assert_eq!(
            c.canonical_json(),
            r#"{"ADDED_DB": 10.0, "BLOCK_LAG": 100, "COARSE_LAG": 100, "DLG_BELOW": 25.0, "GAIN_PCT": 0.75, "GAP_MIN": 25, "GAP_TRIM": 5, "LAG_MIN_R": 0.5, "MATCH_R": 0.7, "MISMATCH_R": 0.4, "SILENT_DB": -90.0, "TENTH_AD_AG": 0.05, "TENTH_COMM_AG": 0.4, "TENTH_GAP_S": 10.0, "WIN_LAG": 5, "X_ACTIVE_BELOW": 20.0}"#
        );
        c.set("GAP_MIN", 30.0).unwrap();
        c.set("SILENT_DB", -80.0).unwrap();
        c.set("TENTH_AD_AG", 0.00001).unwrap();
        let text = c.canonical_json();
        assert!(text.contains(r#""GAP_MIN": 30, "#), "{text}");
        assert!(text.contains(r#""SILENT_DB": -80.0, "#), "{text}");
        assert!(text.contains(r#""TENTH_AD_AG": 1e-05, "#), "{text}");
    }

    #[test]
    fn num_keeps_its_kind_and_prints_as_python() {
        assert_eq!(serde_json::to_string(&Num::Int(10)).unwrap(), "10");
        assert_eq!(serde_json::to_string(&Num::Float(10.0)).unwrap(), "10.0");
        assert_eq!(serde_json::from_str::<Num>("10").unwrap(), Num::Int(10));
        assert_eq!(serde_json::from_str::<Num>("10.0").unwrap(), Num::Float(10.0));
        assert_eq!(serde_json::from_str::<Num>("-3").unwrap(), Num::Int(-3));
        assert_eq!(Num::Int(25).to_string(), "25");
        assert_eq!(Num::Float(0.7).to_string(), "0.7");
        assert_eq!(Num::Float(20.0).to_string(), "20.0");
        assert_eq!(Num::Float(1e-5).to_string(), "1e-05");
        assert_eq!((Num::Int(25).as_f64(), Num::Float(0.5).as_f64()), (25.0, 0.5));
    }
}

/// MIX, CENTRE = programme(): built once, as test_audio_similarity.py builds them at import.
#[cfg(test)]
fn mix_centre() -> &'static (Vec<f64>, Vec<f64>) {
    static PROGRAMME: std::sync::OnceLock<(Vec<f64>, Vec<f64>)> = std::sync::OnceLock::new();
    PROGRAMME.get_or_init(|| crate::testsupport::programme(420, 1))
}

#[cfg(test)]
#[allow(clippy::module_inception)] // named after the Python class it ports
mod measures {
    //! Ported from test_audio_similarity.py::Measures (6 tests): every assertion on the measures. The tests' verdict()
    //! and not_judged() assertions need core::audio::rule; Task 5 appends them to these six functions.
    use super::*;
    use crate::testsupport::{commentary, FPS};

    #[test]
    fn a_quieter_delayed_copy_of_the_programme_is_kept() {
        let (mix, centre) = mix_centre();
        // 60 ms late, 6 dB down
        let mut copy = vec![-100.0; 3];
        copy.extend(mix[..mix.len() - 3].iter().map(|v| v - 6.0));
        let m = analyse(mix, &copy, Some(centre.as_slice()), true, &Constants::default());
        let r = m.run().expect("measured");
        assert_eq!(r.lag_ms, Some(60));
        assert!(r.dc >= 0.9, "dc {}", r.dc);
        assert_eq!(r.ag, 0.0);
    }

    #[test]
    fn a_voice_talking_through_the_programme_has_the_commentary_pattern() {
        let (mix, centre) = mix_centre();
        let m = analyse(mix, &commentary(mix, 7), Some(centre.as_slice()), true, &Constants::default());
        let r = m.run().expect("measured");
        assert!(r.dc <= 0.30, "dc {}", r.dc);
        assert!(r.ag >= 0.50, "ag {}", r.ag);
        assert_eq!(r.tenths_comm, r.tenths);
        assert!(r.smi >= 0.50, "smi {}", r.smi);
        assert!(r.ls_db <= 13.0, "ls_db {}", r.ls_db);
    }

    #[test]
    fn the_same_voice_is_not_confirmed_when_the_main_is_not_continuous_programme_sound() {
        let (mix0, centre0) = mix_centre();
        let (mut mix, mut centre) = (mix0.clone(), centre0.clone());
        // 3 s of digital silence in every 10 s: an isolated score, say
        for t in (0..mix.len()).step_by(10 * FPS) {
            for u in t..mix.len().min(t + 3 * FPS) {
                mix[u] = -100.0;
                centre[u] = -100.0;
            }
        }
        let m = analyse(&mix, &commentary(mix0, 7), Some(centre.as_slice()), true, &Constants::default());
        let r = m.run().expect("measured");
        // what not_judged() reads to give its "digitally silent" reason: the overlap passes, main_silent does not
        assert!(r.overlap >= 0.90, "overlap {}", r.overlap);
        assert!(r.main_silent > 0.10, "main_silent {}", r.main_silent);
    }

    #[test]
    fn steady_music_added_in_the_gaps_is_not_a_commentary() {
        let (mix, centre) = mix_centre();
        let cand: Vec<f64> =
            mix.iter().zip(centre.iter()).map(|(&m, &c)| if c > -50.0 { m } else { -35.0 }).collect();
        let m = analyse(mix, &cand, Some(centre.as_slice()), true, &Constants::default());
        let r = m.run().expect("measured");
        assert!(r.dc >= 0.8, "dc {}", r.dc);
    }

    #[test]
    fn a_candidate_covering_only_part_of_the_main_is_not_confirmed() {
        let (mix, centre) = mix_centre();
        let comm = commentary(mix, 7);
        let m = analyse(mix, &comm[..mix.len() * 7 / 10], Some(centre.as_slice()), true, &Constants::default());
        let r = m.run().expect("measured");
        assert!(r.overlap < 0.9, "overlap {}", r.overlap);
    }

    #[test]
    fn too_little_audio_gives_no_verdict() {
        let (mix, _) = mix_centre();
        let m = analyse(&mix[..500], &mix[..500], None, true, &Constants::default());
        assert_eq!(m, Measures::Error { frames: Some(500), error: "too short".to_string() });
    }
}

#[cfg(test)]
mod analysis_tests {
    //! New tests: the measure helpers and analyse against the values Python 3.9.6 gave for the same synthetic inputs,
    //! reverse_measures and swapped, and the measures' JSON form.
    use super::*;
    use crate::testsupport::commentary;

    /// json.dumps of sim.analyse(MIX, [-100.0] * 3 + [v - 6 for v in MIX[:-3]], CENTRE), NaN as null.
    const COPY_PY: &str = r#"{"frames": 21000, "lag_ms": 60, "r_global": 1.0, "overlap": 0.9998571428571429, "main_silent": 0.0, "dc": 1.0, "gain_db": -6.0, "gap_s": 161.72, "ag": 0.0, "ag_s": 0.0, "adl": 0.0, "tenths": 10, "tenths_comm": 0, "tenths_ad": 0, "smi": null, "smi_windows": 0, "ls_db": 11.147648202020317, "added_s": 0.0, "rev_error": "", "rev_overlap": 0.9998571428571429, "rev_main_silent": 0.0, "rev_gap_s": 161.72, "rev_dc": 1.0, "rev_ag": 0.0, "rev_ag_s": 0.0, "rev_adl": 0.0, "rev_tenths": 10, "rev_tenths_comm": 0, "rev_tenths_ad": 0, "rev_smi": null, "rev_smi_windows": 0, "rev_ls_db": 11.147648202020317, "rev_added_s": 0.0, "rev_gain_db": 6.0}"#;
    /// json.dumps of sim.analyse(MIX, commentary(MIX), CENTRE), NaN as null.
    const COMMENTARY_PY: &str = r#"{"frames": 21000, "lag_ms": 0, "r_global": 0.06417622940791069, "overlap": 1.0, "main_silent": 0.0, "dc": 0.024193548387096774, "gain_db": 2.1551622753224002, "gap_s": 161.78, "ag": 0.8684633452837186, "ag_s": 140.5, "adl": 0.07821136869895223, "tenths": 10, "tenths_comm": 10, "tenths_ad": 10, "smi": 0.771334137329481, "smi_windows": 262, "ls_db": 9.034490204267868, "added_s": 173.1, "rev_error": "", "rev_overlap": 1.0, "rev_main_silent": 0.009761904761904762, "rev_gap_s": 11.14, "rev_dc": 0.004761904761904762, "rev_ag": 0.7414721723518851, "rev_ag_s": 8.26, "rev_adl": 0.02035583464154893, "rev_tenths": 0, "rev_tenths_comm": 0, "rev_tenths_ad": 0, "rev_smi": 0.7614099122810714, "rev_smi_windows": 4, "rev_ls_db": 11.147648202020317, "rev_added_s": 29.76, "rev_gain_db": -2.1551622753224002}"#;
    /// Measures::Run(pans()) as serde_json writes it.
    const PANS_JSON: &str = r#"{"overlap":1.0,"main_silent":0.0,"dc":0.08,"gain_db":10.5,"gap_s":929.0,"ag":0.54,"ag_s":505.0,"adl":0.18,"tenths":10,"tenths_comm":9,"tenths_ad":10,"smi":0.51,"smi_windows":0,"ls_db":10.8,"added_s":2016.0,"rev_error":"","rev_overlap":1.0,"rev_main_silent":0.001,"rev_gap_s":470.0,"rev_dc":0.06,"rev_ag":0.31,"rev_ag_s":144.0,"rev_adl":0.04,"rev_tenths":10,"rev_tenths_comm":1,"rev_tenths_ad":10,"rev_smi":0.39,"rev_smi_windows":526,"rev_ls_db":17.1,"rev_added_s":796.0,"rev_gain_db":-6.7}"#;
    /// The same forward run with a reverse run that had an error, as analyse would store it.
    const FAILED_JSON: &str = r#"{"overlap":1.0,"main_silent":0.0,"dc":0.08,"gain_db":10.5,"gap_s":929.0,"ag":0.54,"ag_s":505.0,"adl":0.18,"tenths":10,"tenths_comm":9,"tenths_ad":10,"smi":0.51,"smi_windows":0,"ls_db":10.8,"added_s":2016.0,"rev_error":"too short","rev_overlap":null,"rev_main_silent":null,"rev_gap_s":0.0,"rev_dc":null,"rev_ag":null,"rev_ag_s":null,"rev_adl":null,"rev_tenths":null,"rev_tenths_comm":null,"rev_tenths_ad":null,"rev_smi":null,"rev_smi_windows":null,"rev_ls_db":null,"rev_added_s":null,"rev_gain_db":null}"#;

    /// Every key of the Python's measures dict against the Rust's serde form: the same keys in the same order, ints
    /// equal as ints, floats within 1e-9 (SMI within 1e-6, spec section 4), null where the Python had NaN.
    fn assert_like_python(expected: &str, m: &Measures) {
        let want: serde_json::Value = serde_json::from_str(expected).expect("the Python's JSON parses");
        let got = serde_json::to_value(m).expect("measures serialise");
        let (want, got) = (want.as_object().unwrap(), got.as_object().unwrap());
        let keys = |o: &serde_json::Map<String, serde_json::Value>| o.keys().cloned().collect::<Vec<String>>();
        assert_eq!(keys(got), keys(want));
        for (key, w) in want {
            let g = got.get(key).expect("the same keys");
            let tol = if key == "smi" || key == "rev_smi" { 1e-6 } else { 1e-9 };
            match (w, g) {
                (serde_json::Value::Number(a), serde_json::Value::Number(b)) if a.is_f64() => {
                    assert!(b.is_f64(), "{key}: {g} is not a float");
                    let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
                    assert!((a - b).abs() <= tol, "{key}: python {a}, rust {b}");
                }
                _ => assert_eq!(g, w, "{key}"),
            }
        }
    }

    /// A measured commentary and its reverse run (test_audio_similarity.py's ReverseRun numbers, gain_db 10.5).
    fn pans() -> Run {
        Run {
            overlap: 1.0,
            main_silent: 0.0,
            gap_s: 929.0,
            dc: 0.08,
            ag: 0.54,
            ag_s: 505.0,
            adl: 0.18,
            tenths: 10,
            tenths_comm: 9,
            tenths_ad: 10,
            smi: 0.51,
            ls_db: 10.8,
            added_s: 2016.0,
            gain_db: 10.5,
            rev: Reverse::Block(RevBlock {
                rev_error: String::new(),
                rev_overlap: 1.0,
                rev_main_silent: 0.001,
                rev_gap_s: 470.0,
                rev_dc: 0.06,
                rev_ag: 0.31,
                rev_ag_s: 144.0,
                rev_adl: 0.04,
                rev_tenths: Some(10),
                rev_tenths_comm: Some(1),
                rev_tenths_ad: Some(10),
                rev_smi: 0.39,
                rev_smi_windows: Some(526),
                rev_ls_db: 17.1,
                rev_added_s: 796.0,
                rev_gain_db: -6.7,
            }),
            ..Run::blank()
        }
    }

    #[test]
    fn percentile_clamp_pearson_and_corr_at_are_the_pythons() {
        assert_eq!(percentile(&[3.0, 1.0, 2.0], 0.5), 2.0);
        assert_eq!(percentile(&[3.0, 1.0, 2.0], 1.0), 3.0);
        assert!(percentile(&[], 0.5).is_nan());
        assert_eq!(percentile(&[5.0, 4.0, 3.0, 2.0, 1.0], 0.95), 5.0);
        assert_eq!(clamp(&[0.0, -10.0, -50.0, -100.0]), vec![0.0, -10.0, -45.0, -45.0]);
        assert!((pearson(&[1.0, 2.0, 3.0, 4.0], &[2.0, 4.0, 5.0, 9.0]) - 0.9647638212377322).abs() <= 1e-12);
        assert_eq!(pearson(&[1.0], &[1.0]), 0.0);
        assert_eq!(pearson(&[1.0, 1.0], &[2.0, 3.0]), 0.0);
        let (mix, _) = mix_centre();
        assert!((corr_at(mix, mix, 0, 100, 3, 1) - (-0.04127718262826115)).abs() <= 1e-12);
        assert_eq!(corr_at(&mix[..20], &mix[..20], 0, 20, 15, 1), -2.0);
        assert!(ok(0.0) && !ok(f64::NAN));
    }

    #[test]
    fn smi_matches_the_python() {
        let square: Vec<f64> = (0..64).map(|i| if (i / 6) % 2 == 0 { -20.0 } else { -40.0 }).collect();
        assert!((smi(&square) - 0.8200324498810588).abs() <= 1e-9, "{}", smi(&square));
        let (mix, _) = mix_centre();
        assert!((smi(&mix[..64]) - 0.7435025680117952).abs() <= 1e-9, "{}", smi(&mix[..64]));
        assert!((smi(&mix[1000..1064]) - 0.41598795760788587).abs() <= 1e-9, "{}", smi(&mix[1000..1064]));
    }

    #[test]
    fn analyse_matches_the_python_on_two_synthetic_pairs() {
        let (mix, centre) = mix_centre();
        let mut copy = vec![-100.0; 3];
        copy.extend(mix[..mix.len() - 3].iter().map(|v| v - 6.0));
        let consts = Constants::default();
        assert_like_python(COPY_PY, &analyse(mix, &copy, Some(centre.as_slice()), true, &consts));
        assert_like_python(COMMENTARY_PY, &analyse(mix, &commentary(mix, 7), Some(centre.as_slice()), true, &consts));
    }

    #[test]
    fn an_empty_dialogue_is_the_main_track_and_too_short_is_either_track() {
        let (mix, _) = mix_centre();
        let consts = Constants::default();
        let empty: &[f64] = &[];
        let with_empty = analyse(&mix[..3000], &mix[..3000], Some(empty), true, &consts);
        let with_none = analyse(&mix[..3000], &mix[..3000], None, true, &consts);
        assert_eq!(serde_json::to_value(&with_empty).unwrap(), serde_json::to_value(&with_none).unwrap());
        assert_eq!(
            analyse(&mix[..999], mix, None, true, &consts),
            Measures::Error { frames: Some(999), error: "too short".to_string() }
        );
        assert_eq!(
            analyse(mix, &mix[..999], None, true, &consts),
            Measures::Error { frames: Some(21000), error: "too short".to_string() }
        );
    }

    #[test]
    fn reverse_measures_and_swapped_exchange_the_runs() {
        let m = Measures::Run(pans());
        let back = Run {
            overlap: 1.0,
            main_silent: 0.001,
            gap_s: 470.0,
            dc: 0.06,
            ag: 0.31,
            ag_s: 144.0,
            adl: 0.04,
            tenths: 10,
            tenths_comm: 1,
            tenths_ad: 10,
            smi: 0.39,
            smi_windows: 526,
            ls_db: 17.1,
            added_s: 796.0,
            gain_db: -6.7,
            ..Run::blank()
        };
        assert_eq!(reverse_measures(&m), Some(Measures::Run(back.clone())));
        let sw = swapped(&m).expect("a reverse block");
        let want = Run {
            rev: Reverse::Block(RevBlock {
                rev_error: String::new(),
                rev_overlap: 1.0,
                rev_main_silent: 0.0,
                rev_gap_s: 929.0,
                rev_dc: 0.08,
                rev_ag: 0.54,
                rev_ag_s: 505.0,
                rev_adl: 0.18,
                rev_tenths: Some(10),
                rev_tenths_comm: Some(9),
                rev_tenths_ad: Some(10),
                rev_smi: 0.51,
                rev_smi_windows: Some(0),
                rev_ls_db: 10.8,
                rev_added_s: 2016.0,
                rev_gain_db: 10.5,
            }),
            ..back
        };
        assert_eq!(sw, Measures::Run(want));
        assert_eq!(swapped(&sw), Some(m.clone()));
        // a reverse run with an error
        let mut failed = pans();
        if let Reverse::Block(block) = &mut failed.rev {
            block.rev_error = "too short".to_string();
        }
        let failed = Measures::Run(failed);
        let error = Some(Measures::Error { frames: None, error: "too short".to_string() });
        assert_eq!(reverse_measures(&failed), error);
        assert_eq!(swapped(&failed), error);
        // no reverse block: an error, the earlier form, analyse(reverse=False)
        let short = Measures::Error { frames: Some(500), error: "too short".to_string() };
        assert_eq!((reverse_measures(&short), swapped(&short)), (None, None));
        let earlier = Measures::Run(Run { rev: Reverse::Earlier { rev_gap_s: 29.0, rev_ag: 0.10 }, ..pans() });
        assert_eq!((reverse_measures(&earlier), swapped(&earlier)), (None, None));
        let absent = Measures::Run(Run { rev: Reverse::Absent, ..pans() });
        assert_eq!((reverse_measures(&absent), swapped(&absent)), (None, None));
    }

    #[test]
    fn measures_json_has_the_python_keys_kinds_and_null() {
        let short = Measures::Error { frames: Some(500), error: "too short".to_string() };
        assert_eq!(serde_json::to_string(&short).unwrap(), r#"{"frames":500,"error":"too short"}"#);
        let bare = Measures::Error { frames: None, error: "too short".to_string() };
        assert_eq!(serde_json::to_string(&bare).unwrap(), r#"{"error":"too short"}"#);
        assert_eq!(serde_json::from_str::<Measures>(r#"{"error":"too short"}"#).unwrap(), bare);
        // a block: the forward keys, rev_error, then the REVERSE_KEYS in order; ints as ints
        let m = Measures::Run(pans());
        assert_eq!(serde_json::to_string(&m).unwrap(), PANS_JSON);
        assert_eq!(serde_json::from_str::<Measures>(PANS_JSON).unwrap(), m);
        let value = serde_json::to_value(&m).unwrap();
        let keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
        let rev_keys: Vec<String> = REVERSE_KEYS.iter().map(|k| format!("rev_{k}")).collect();
        assert_eq!((keys.len(), keys[15].as_str()), (31, "rev_error"));
        assert_eq!(keys[16..].to_vec(), rev_keys);
        // NaN is written as null and read back as NaN
        let blank = serde_json::to_value(Measures::Run(Run::blank())).unwrap();
        assert_eq!(blank["dc"], serde_json::Value::Null);
        assert_eq!(blank["tenths"], serde_json::json!(0));
        assert!(blank.get("frames").is_none());
        let read: Measures = serde_json::from_value(blank).unwrap();
        assert!(read.run().unwrap().dc.is_nan());
        // a reverse run with an error: rev_gap_s 0.0, the other values null
        let read: Measures = serde_json::from_str(FAILED_JSON).unwrap();
        match &read.run().unwrap().rev {
            Reverse::Block(block) => {
                assert_eq!((block.rev_error.as_str(), block.rev_gap_s, block.rev_tenths), ("too short", 0.0, None));
                assert!(block.rev_dc.is_nan());
            }
            other => panic!("not a block: {other:?}"),
        }
        assert_eq!(serde_json::to_string(&read).unwrap(), FAILED_JSON);
        // rev_ag without rev_tenths is the earlier form; a missing rev_gap_s reads as 0.0
        let earlier = Run { rev: Reverse::Earlier { rev_gap_s: 29.0, rev_ag: 0.1 }, ..pans() };
        let mut value = serde_json::to_value(Measures::Run(earlier.clone())).unwrap();
        assert_eq!(serde_json::from_value::<Measures>(value.clone()).unwrap(), Measures::Run(earlier));
        value.as_object_mut().unwrap().remove("rev_gap_s");
        let read: Measures = serde_json::from_value(value).unwrap();
        assert_eq!(read.run().unwrap().rev, Reverse::Earlier { rev_gap_s: 0.0, rev_ag: 0.1 });
        // a forward key that is missing is an error, not a silent default
        assert!(serde_json::from_str::<Measures>(r#"{"overlap": 1.0}"#).is_err());
    }
}
