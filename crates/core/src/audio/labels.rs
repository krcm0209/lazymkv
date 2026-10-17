//! The owner's labels for the audio tracks of one disc (decision D16), `<slug>/audio-labels.json`, and the discs and
//! tracks the thresholds were fitted on, `audio-fitted.json` beside the recordings. Both files are `serde_json::Value`s
//! with insertion order kept, so an unknown key and every character of a note survive an edit. Validation runs over
//! the value and gives the Python's sentences verbatim (`{x!r}` through `py_repr`, Python's `==` through `py_eq`),
//! except that the one naming `audio_labels.py kinds` names `lazymkv-dev labels kinds`; the typed views
//! (`LabelEntry`, `Labels`) are made only after it passes. The edits are functions from the value to the new value;
//! `crates/dev` writes the result with `pyjson::write_labels`. Whether a label counts towards experiment 13 is worked
//! out each time (`not_counted_because`, D23), never stored. No I/O: files come through a `Tree`.
//! Py: scripts/phase0/audio_labels.py (commit 75fa76a).

use std::collections::{BTreeMap, BTreeSet};

use regex::Regex;
use serde_json::{json, Map, Value};

use crate::audio::levels::read_manifest;
use crate::recording::{
    decode_text, py_int, py_repr, py_repr_str, py_splitlines, py_str, read_text, Encoding, Errors, Ordered, PathKey, Tree,
};
use crate::robot::{int_attr, parse};

/// The labels file's format line. Py: FORMAT.
pub const FORMAT: &str = "lazymkv audio labels v1";
/// The fitted file's format line. Py: FITTED_FORMAT.
pub const FITTED_FORMAT: &str = "lazymkv audio fitted v1";
/// Scan flag 2048 (attribute 22): a stream MakeMKV derived from another, such as a DTS core. Py: DERIVED.
pub const DERIVED: i64 = 2048;

/// (kind, group, what the owner wants in the final file, meaning), in the Python's order. Groups: programme audio (a
/// drop verdict on it is a failure), a duplicate (dropped by MD5, D13), unwanted (dropped when confirmed: R6, D12,
/// D14), ask (the owner has not decided: EXP-13 questions 3 to 6), unknown. Py: KINDS.
pub const KINDS: [(&str, &str, &str, &str); 18] = [
    ("main", "programme", "keep", "the programme's main soundtrack; as a reference it is never judged"),
    ("lossy-companion", "programme", "keep", "a separately encoded copy of the programme mix (AC-3/AAC/DTS companion, 2.0 downmix)"),
    ("alternate-mix", "programme", "keep", "another mix or master of the programme (original or broadcast mix, remaster, night mix)"),
    ("laugh-track-version", "programme", "keep", "the programme with or without its laugh track"),
    (
        "dub",
        "programme",
        "keep",
        concat!(
            "the programme in another language, tagged with it (an English dub of a Spanish film); kept (R3, R4); ",
            "content never drops a candidate in another language than the reference (EXP-13 addendum 9.2)",
        ),
    ),
    ("derived-core", "programme", "keep", "a stream MakeMKV derived from another (scan flag 2048); the app drops it by that flag, never by content"),
    ("duplicate", "duplicate", "drop", "an exact copy of another track of the title (same packet MD5); one copy is kept (D13)"),
    ("commentary", "unwanted", "drop", "an audio commentary (R6, D12)"),
    ("partial-commentary", "unwanted", "drop", "a commentary over only some scenes"),
    ("audio-description", "unwanted", "drop", "audio description (R6)"),
    ("isolated-score", "unwanted", "drop", "the score alone (D14)"),
    ("music-effects", "unwanted", "drop", "music and effects without dialogue (D14)"),
    ("same-language-dub", "ask", "ask", "a second dub or alternate dialogue in the same language (EXP-13 question 3)"),
    ("alternate-narration", "ask", "ask", "a second narrator (EXP-13 question 3)"),
    ("foreign-dub", "ask", "ask", "a dub in another language than its tag says, tagged with a wanted one (EXP-13 question 4)"),
    ("other-speech", "ask", "ask", "trivia, interview or picture-in-picture speech (EXP-13 question 5)"),
    ("sing-along", "ask", "ask", "a sing-along or karaoke track (EXP-13 question 6)"),
    ("unknown", "unknown", "unknown", "the owner does not know"),
];

/// (group, wanted, meaning) of a kind; None for a kind outside the vocabulary. Py: KINDS[kind].
pub fn kind_of(kind: &str) -> Option<(&'static str, &'static str, &'static str)> {
    KINDS.iter().find(|(name, _, _, _)| *name == kind).map(|&(_, group, wanted, meaning)| (group, wanted, meaning))
}

/// (how, meaning): where a label comes from, in the Python's order. Py: HOW.
pub const HOW: [(&str, &str); 7] = [
    ("listened", "a person listened to it: the track's content was checked (counts)"),
    ("packaging", "the packaging says so (an observation: does not count on its own, D23)"),
    ("menu", "the disc's menu says so (an observation: does not count on its own, D23)"),
    ("flag", "a flag in MakeMKV's scan (an observation: does not count on its own, D23)"),
    ("thediscdb", "TheDiscDB's track description (an observation: does not count on its own, D23)"),
    ("md5", "identical packet MD5 (an exact duplicate; no listening needed)"),
    ("inferred", "inferred, for example from the disc's layout; never counts"),
];
/// Spec 17 row 13 and D23: the one source that makes a label count. Py: COUNTING_HOW.
pub const COUNTING_HOW: [&str; 1] = ["listened"];
/// Sources kept with a label as observations, never counted on their own. Py: OBSERVATION_HOW.
pub const OBSERVATION_HOW: [&str; 4] = ["packaging", "menu", "flag", "thediscdb"];
/// Kinds that never count. Py: NOT_COUNTED.
pub const NOT_COUNTED: [&str; 3] = ["main", "duplicate", "unknown"];
/// Py: COMMENTARY_KINDS.
pub const COMMENTARY_KINDS: [&str; 2] = ["commentary", "partial-commentary"];
/// Py: TITLE_KINDS.
pub const TITLE_KINDS: [&str; 4] = ["feature", "episode", "extra", "other"];

/// Every key a track entry has. Py: TRACK_KEYS.
const TRACK_KEYS: [&str; 8] = ["label", "stream", "kind", "how", "sure", "date", "saw_verdict", "note"];
/// The other keys a track may have. Py: OPTIONAL_KEYS.
const OPTIONAL_KEYS: [&str; 2] = ["film_audible", "history"];
/// The other keys a history or removed entry may have (strict=False).
const LOOSE_KEYS: [&str; 5] = ["film_audible", "history", "replaced", "removed", "why"];
/// What `set` compares to decide that a label is unchanged. Py: SUBSTANCE.
const SUBSTANCE: [&str; 6] = ["kind", "how", "sure", "saw_verdict", "film_audible", "note"];
/// The labels file's top-level keys.
const TOP_KEYS: [&str; 5] = ["format", "slug", "titles", "tracks", "removed"];
/// A title's keys.
const TITLE_KEYS: [&str; 3] = ["title_kind", "dialogue_heavy", "note"];
/// The fitted file's top-level keys.
const FITTED_TOP_KEYS: [&str; 2] = ["format", "fitted"];
/// A fitted entry's keys.
const FITTED_KEYS: [&str; 6] = ["slug", "label", "stream", "date", "rule", "note"];
/// Numbers row 13 may spell out. Py: WORDS.
const WORDS: [(&str, i64); 10] = [
    ("one", 1), ("two", 2), ("three", 3), ("four", 4), ("five", 5), ("six", 6), ("seven", 7), ("eight", 8), ("nine", 9), ("ten", 10),
];
/// A number in row 13. Py: exp13_requirements' `num`.
const NUM: &str = r"(\d+|one|two|three|four|five|six|seven|eight|nine|ten)";
/// Row 13's phrases with the keys their numbers fill, in the Python's order; `{num}` stands for NUM. Py: `phrases`.
const PHRASES: [(&[&str], &str); 5] = [
    (
        &["commentaries", "commentary_discs", "commentary_features", "commentary_audible"],
        "at least {num} commentaries from at least {num} discs, at least {num} of them on feature films and at least {num} with the film clearly audible",
    ),
    (
        &["ad_min", "ad_max"],
        r"at least {num} audio-description track if the owner has a disc with one \(all that the owner has, up to {num}\)",
    ),
    (&["programme", "programme_separate"], "at least {num} programme tracks other than the reference, at least {num} of them separately encoded"),
    (
        &["no_centre_titles", "dialogue_heavy_titles"],
        "at least {num} title whose reference has no centre channel .{0,40}?and at least {num} dialogue-heavy or speech-only title",
    ),
    (&["verdict_discs"], "at least one commentary on each of at least {num} different discs"),
];
/// The optional dubs phrase (EXP-13 addendum 9.5).
const DUBS: &str = "at least {num} dubs in a wanted language from at least {num} discs, at least {num} of them on a dialogue-heavy title";
/// The fitting disc's phrase.
const FITTING: &str = r"Every threshold was fitted on .{0,40}?\(`([a-z0-9-]+)`";

/// SLUG.match: `^[a-z0-9][a-z0-9-]*$` with Python's `$`, which also matches before one final "\n". Py: SLUG.
pub fn is_slug(text: &str) -> bool {
    let body = text.strip_suffix('\n').unwrap_or(text);
    let mut chars = body.chars();
    let first = matches!(chars.next(), Some(c) if c.is_ascii_lowercase() || c.is_ascii_digit());
    first && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Experiment 13's totals and fitting disc from row 13 of the spec's section 17, keys in the Python's order (the
/// snapshot's "spec_row13"); the dub totals are None while row 13 lacks their phrase. Py: exp13_requirements' dict.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Requirements {
    pub commentaries: i64,
    pub commentary_discs: i64,
    pub commentary_features: i64,
    pub commentary_audible: i64,
    pub ad_min: i64,
    pub ad_max: i64,
    pub programme: i64,
    pub programme_separate: i64,
    pub no_centre_titles: i64,
    pub dialogue_heavy_titles: i64,
    pub verdict_discs: i64,
    pub fitting_slug: String,
    pub dubs: Option<i64>,
    pub dub_discs: Option<i64>,
    pub dub_heavy: Option<i64>,
}

/// Reads row 13 (the first line starting "| 13 |", lines by py_splitlines) with the Python's patterns. Err names the
/// phrase not found, verbatim, so a reworded row is noticed instead of silently counted against old numbers (the
/// Python's "update audio_labels.py" names this module instead: "update core::audio::labels"). Py: exp13_requirements.
pub fn exp13_requirements(spec_text: &str) -> Result<Requirements, String> {
    let row = py_splitlines(spec_text)
        .into_iter()
        .find(|line| line.starts_with("| 13 |"))
        .ok_or_else(|| "spec section 17 has no row starting '| 13 |'".to_string())?;
    let mut found: BTreeMap<&str, i64> = BTreeMap::new();
    for (keys, template) in PHRASES {
        let pattern = template.replace("{num}", NUM);
        let caps = Regex::new(&pattern)
            .expect("row 13's patterns are valid")
            .captures(row)
            .ok_or_else(|| format!("spec section 17 row 13 no longer contains the phrase /{pattern}/; update core::audio::labels"))?;
        for (k, key) in keys.iter().enumerate() {
            found.insert(*key, number(&caps[k + 1])?);
        }
    }
    let fitting_slug = Regex::new(FITTING)
        .expect("the fitting pattern is valid")
        .captures(row)
        .map(|caps| caps[1].to_string())
        .ok_or_else(|| "spec section 17 row 13 no longer names the disc the thresholds were fitted on".to_string())?;
    let dubs = match Regex::new(&DUBS.replace("{num}", NUM)).expect("the dubs pattern is valid").captures(row) {
        Some(caps) => [Some(number(&caps[1])?), Some(number(&caps[2])?), Some(number(&caps[3])?)],
        None => [None, None, None],
    };
    Ok(Requirements {
        commentaries: found["commentaries"],
        commentary_discs: found["commentary_discs"],
        commentary_features: found["commentary_features"],
        commentary_audible: found["commentary_audible"],
        ad_min: found["ad_min"],
        ad_max: found["ad_max"],
        programme: found["programme"],
        programme_separate: found["programme_separate"],
        no_centre_titles: found["no_centre_titles"],
        dialogue_heavy_titles: found["dialogue_heavy_titles"],
        verdict_discs: found["verdict_discs"],
        fitting_slug,
        dubs: dubs[0],
        dub_discs: dubs[1],
        dub_heavy: dubs[2],
    })
}

/// YYYY-MM-DD (ASCII digits, as date.fromisoformat takes them) and a real date of years 1 to 9999. Any value that is
/// not a string is false. Py: date_ok.
pub fn date_ok(value: &Value) -> bool {
    let Some(text) = value.as_str() else { return false };
    let bytes = text.as_bytes();
    let shape = bytes.len() == 10
        && bytes.iter().enumerate().all(|(i, c)| if i == 4 || i == 7 { *c == b'-' } else { c.is_ascii_digit() });
    if !shape {
        return false;
    }
    let field = |range: std::ops::Range<usize>| text[range].parse::<u32>().unwrap_or(0);
    let (year, month, day) = (field(0..4), field(5..7), field(8..10));
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => 0,
    };
    year >= 1 && day >= 1 && day <= days
}

/// A fitted entry: (slug, label, stream), label and stream None for a whole disc. Py: load_fitted's tuples.
pub type FittedKey = (String, Option<String>, Option<i64>);
/// The discs and tracks the thresholds were fitted on.
pub type Fitted = BTreeSet<FittedKey>;

/// Every problem of a parsed audio-fitted.json, one sentence each, verbatim; empty when it is valid. Py: fitted_problems.
pub fn fitted_problems(data: &Value) -> Vec<String> {
    let Some(d) = data.as_object() else { return vec!["the file is not a JSON object".to_string()] };
    let mut out = Vec::new();
    let extra: Vec<&str> = d.keys().map(String::as_str).filter(|k| !FITTED_TOP_KEYS.contains(k)).collect();
    if !extra.is_empty() {
        out.push(format!("unknown top-level keys {}", sorted_list(extra)));
    }
    if get(d, "format").as_str() != Some(FITTED_FORMAT) {
        out.push(format!("format is {}, not {}", repr_of(d, "format"), py_repr_str(FITTED_FORMAT)));
    }
    let Some(fitted) = get(d, "fitted").as_array() else {
        out.push("fitted is not a list".to_string());
        return out;
    };
    for (k, f) in fitted.iter().enumerate() {
        let where_ = format!("entry {k}");
        let Some(fm) = f.as_object() else {
            out.push(format!("{where_}: not an object"));
            continue;
        };
        let keys: Vec<&str> = fm.keys().map(String::as_str).collect();
        if keys.len() != FITTED_KEYS.len() || !keys.iter().all(|k| FITTED_KEYS.contains(k)) {
            out.push(format!("{where_}: keys must be {}, not {}", sorted_list(FITTED_KEYS.to_vec()), sorted_list(keys)));
        }
        if !get(fm, "slug").as_str().map_or(false, is_slug) {
            out.push(format!("{where_}: slug {} is not a disc slug", repr_of(fm, "slug")));
        }
        let label = get(fm, "label");
        if label.is_null() {
            if !get(fm, "stream").is_null() {
                out.push(format!("{where_}: a stream needs a label"));
            }
        } else if !label.as_str().map_or(false, is_slug) {
            out.push(format!("{where_}: label {} is not a rip label", py_repr(label)));
        } else if !is_stream_number(get(fm, "stream")) {
            out.push(format!("{where_}: a label needs ffprobe's stream number"));
        }
        if !date_ok(get(fm, "date")) {
            out.push(format!("{where_}: date {} is not YYYY-MM-DD", repr_of(fm, "date")));
        }
        for key in ["rule", "note"] {
            if !get(fm, key).is_string() {
                out.push(format!("{where_}: {key} is not text"));
            }
        }
    }
    out
}

/// The fitted set: row 13's disc (when given and not empty) plus every entry of `<corpus>/audio-fitted.json` (none
/// when the file is absent). Err "<path>: not valid JSON (...)" or "<path>: <problems joined by '; '>", and the
/// OSError text when the file cannot be read. Py: load_fitted.
pub fn load_fitted(tree: &dyn Tree, corpus: &PathKey, fitting: Option<&str>) -> Result<Fitted, String> {
    let mut out = Fitted::new();
    if let Some(slug) = fitting.filter(|slug| !slug.is_empty()) {
        out.insert((slug.to_string(), None, None));
    }
    let path = corpus.join("audio-fitted.json");
    if !tree.exists(&path) {
        return Ok(out);
    }
    let shown = tree.display(&path);
    let data = read_value(tree, &path)?.map_err(|e| format!("{shown}: not valid JSON ({e})"))?;
    let found = fitted_problems(&data);
    if !found.is_empty() {
        return Err(format!("{shown}: {}", found.join("; ")));
    }
    for f in data["fitted"].as_array().into_iter().flatten() {
        out.insert((f["slug"].as_str().unwrap_or_default().to_string(), f["label"].as_str().map(str::to_string), f["stream"].as_i64()));
    }
    Ok(out)
}

/// The disc or the track is fitted. Py: is_fitted.
pub fn is_fitted(fitted: &Fitted, slug: &str, label: &str, stream: i64) -> bool {
    fitted.contains(&(slug.to_string(), None, None)) || fitted.contains(&(slug.to_string(), Some(label.to_string()), Some(stream)))
}

/// One validated track label, with the raw entry kept for the snapshot's "owner_label".
#[derive(Clone, Debug, PartialEq)]
pub struct LabelEntry {
    pub raw: Value,
    pub label: String,
    pub stream: i64,
    pub kind: String,
    pub how: String,
    pub sure: bool,
    pub saw_verdict: bool,
    /// Some(b) only for a JSON boolean, so `== Some(true)` is the Python's `.get("film_audible") is True` (a 1 passes
    /// validation, being `== True`, but is None here).
    pub film_audible: Option<bool>,
}

impl LabelEntry {
    /// The typed view of an entry with no entry_problems; None otherwise.
    pub fn from_value(value: &Value) -> Option<LabelEntry> {
        if !entry_problems(value, "", true).is_empty() {
            return None;
        }
        let m = value.as_object()?;
        Some(LabelEntry {
            raw: value.clone(),
            label: m.get("label")?.as_str()?.to_string(),
            stream: m.get("stream")?.as_i64()?,
            kind: m.get("kind")?.as_str()?.to_string(),
            how: m.get("how")?.as_str()?.to_string(),
            sure: m.get("sure")?.as_bool()?,
            saw_verdict: m.get("saw_verdict")?.as_bool()?,
            film_audible: m.get("film_audible").and_then(Value::as_bool),
        })
    }
}

/// "" when the label counts towards experiment 13 (spec section 17 row 13), otherwise why it does not, verbatim.
/// Py: not_counted_because.
pub fn not_counted_because(entry: &LabelEntry, slug: &str, fitted: &Fitted) -> String {
    if NOT_COUNTED.contains(&entry.kind.as_str()) {
        return format!("a {} track never counts", entry.kind);
    }
    if OBSERVATION_HOW.contains(&entry.how.as_str()) {
        return format!("'{}' is an observation, not a check of the track's content (row 13, D23)", entry.how);
    }
    if !COUNTING_HOW.contains(&entry.how.as_str()) {
        return format!("'{}' is not a check of the track's content (row 13, D23)", entry.how);
    }
    if entry.saw_verdict {
        return "the owner saw the verdict first".to_string();
    }
    if !entry.sure {
        return "the owner is not sure".to_string();
    }
    if is_fitted(fitted, slug, &entry.label, entry.stream) {
        return "the thresholds were fitted on it: spec row 13 or audio-fitted.json".to_string();
    }
    String::new()
}

/// The problems of one track entry. strict=false (history and removed entries) checks only that the entry can be
/// read: its kind and how may come from an older vocabulary. A value that is not an object is read as an empty one.
/// Py: entry_problems.
pub fn entry_problems(t: &Value, where_: &str, strict: bool) -> Vec<String> {
    let empty = Map::new();
    let m = t.as_object().unwrap_or(&empty);
    let allowed: &[&str] = if strict { &OPTIONAL_KEYS } else { &LOOSE_KEYS };
    let mut out = base_entry_problems(m, where_, &TRACK_KEYS, allowed);
    if !strict {
        return out;
    }
    let kind = get(m, "kind").as_str();
    let how = get(m, "how").as_str();
    if !kind.map_or(false, |k| kind_of(k).is_some()) {
        out.push(format!("{where_}: kind {} is not in the vocabulary (lazymkv-dev labels kinds)", repr_of(m, "kind")));
    }
    if !how.map_or(false, |h| HOW.iter().any(|(name, _)| *name == h)) {
        out.push(format!("{where_}: how {} is not one of {}", repr_of(m, "how"), names(&HOW).join(", ")));
    }
    if how == Some("md5") && kind != Some("duplicate") {
        out.push(format!("{where_}: how md5 only says that a track is a duplicate"));
    }
    for key in ["saw_verdict", "sure"] {
        if !get(m, key).is_boolean() {
            out.push(format!("{where_}: {key} must be true or false"));
        }
    }
    if !m.get("note").map_or(true, Value::is_string) {
        out.push(format!("{where_}: note is not text"));
    }
    if m.contains_key("film_audible") {
        if !kind.map_or(false, |k| COMMENTARY_KINDS.contains(&k)) {
            out.push(format!("{where_}: film_audible is only for {}", COMMENTARY_KINDS.join(" and ")));
        } else if !true_false_none(m.get("film_audible")) {
            out.push(format!("{where_}: film_audible must be true, false or null"));
        }
    }
    out.extend(history_problems(m, where_, |h, w| entry_problems(h, w, false)));
    out
}

/// Every problem of a parsed labels file, one sentence each, verbatim; empty when it is valid. `slug` is the folder
/// the file is in (None: not compared). Py: problems.
pub fn problems(data: &Value, slug: Option<&str>) -> Vec<String> {
    let Some(d) = data.as_object() else { return vec!["the file is not a JSON object".to_string()] };
    let mut out = head_problems(d, &TOP_KEYS, FORMAT, slug);
    let no_titles = Map::new();
    let titles = match d.get("titles") {
        Some(Value::Object(titles)) => titles,
        _ => {
            out.push("titles is not an object".to_string());
            &no_titles
        }
    };
    for (label, t) in titles {
        let where_ = format!("title {label}");
        if !is_slug(label) {
            out.push(format!("{where_}: not a rip label"));
        }
        let Some(tm) = t.as_object() else {
            out.push(format!("{where_}: not an object"));
            continue;
        };
        let unknown: Vec<&str> = tm.keys().map(String::as_str).filter(|k| !TITLE_KEYS.contains(k)).collect();
        if !unknown.is_empty() {
            out.push(format!("{where_}: unknown keys {}", sorted_list(unknown)));
        }
        if !get(tm, "title_kind").as_str().map_or(false, |k| TITLE_KINDS.contains(&k)) {
            out.push(format!("{where_}: title_kind {} is not one of {}", repr_of(tm, "title_kind"), TITLE_KINDS.join(", ")));
        }
        if !true_false_none(tm.get("dialogue_heavy")) {
            out.push(format!("{where_}: dialogue_heavy must be true, false or null"));
        }
        if !tm.get("note").map_or(true, Value::is_string) {
            out.push(format!("{where_}: note is not text"));
        }
    }
    out.extend(removed_problems(d, |r, where_| entry_problems(r, where_, false)));
    let Some(tracks) = get(d, "tracks").as_array() else {
        out.push("tracks is not a list".to_string());
        return out;
    };
    let mut seen: Vec<(Value, Value)> = Vec::new();
    for (k, t) in tracks.iter().enumerate() {
        let Some(tm) = t.as_object() else {
            out.push(format!("track {k}: not an object"));
            continue;
        };
        let (label, stream) = (get(tm, "label").clone(), get(tm, "stream").clone());
        let where_ = format!("track {} stream {}", py_str(&label), py_str(&stream));
        out.extend(entry_problems(t, &where_, true));
        if seen.iter().any(|(l, s)| py_eq(l, &label) && py_eq(s, &stream)) {
            out.push(format!("{where_}: labelled twice"));
        }
        seen.push((label, stream));
    }
    out
}

/// A title's facts. `dialogue_heavy` is Some(b) only for a JSON boolean (see LabelEntry::film_audible); `note` is ""
/// when absent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TitleFacts {
    pub title_kind: String,
    pub dialogue_heavy: Option<bool>,
    pub note: String,
}

/// A validated labels file: the raw value, its slug, its titles in file order, its tracks in file order.
#[derive(Clone, Debug, PartialEq)]
pub struct Labels {
    pub raw: Value,
    pub slug: String,
    pub titles: Ordered<TitleFacts>,
    pub tracks: Vec<LabelEntry>,
}

/// Reads and validates `<path>` against the name of the folder it is in (lexical, as `path.parent.name`); Err
/// "<shown>: not valid JSON (...)" or "<shown>: <problems joined by '; '>", or the OSError text. Py: load.
pub fn load(tree: &dyn Tree, path: &PathKey) -> Result<Labels, String> {
    let shown = tree.display(path);
    let data = read_value(tree, path)?.map_err(|e| format!("{shown}: not valid JSON ({e})"))?;
    let found = problems(&data, Some(lexical_parent_name(&shown)));
    if !found.is_empty() {
        return Err(format!("{shown}: {}", found.join("; ")));
    }
    let mut titles = Ordered::new();
    for (label, t) in data["titles"].as_object().into_iter().flatten() {
        titles.insert(
            label.clone(),
            TitleFacts {
                title_kind: t["title_kind"].as_str().unwrap_or_default().to_string(),
                dialogue_heavy: t.get("dialogue_heavy").and_then(Value::as_bool),
                note: t.get("note").and_then(Value::as_str).unwrap_or_default().to_string(),
            },
        );
    }
    let tracks = data["tracks"].as_array().into_iter().flatten().filter_map(LabelEntry::from_value).collect();
    let slug = data["slug"].as_str().unwrap_or_default().to_string();
    Ok(Labels { raw: data, slug, titles, tracks })
}

/// A labels file opened for a change: as printed (<slug_dir>/audio-labels.json), the name of its folder (lexical,
/// what `problems` compares the slug with), the value (a new file when absent: format, the resolved folder name,
/// empty titles and tracks) and the problems it has now. Py: open_for_change's tuple.
#[derive(Clone, Debug, PartialEq)]
pub struct Opened {
    pub shown: String,
    pub folder_name: String,
    pub data: Value,
    pub before: Vec<String>,
}

/// A file that does not validate can still be changed, but it must be JSON with a 'tracks' list and a 'titles'
/// object, and the change may not add a problem. The root of a tree is a folder by construction. Err with the
/// Python's sentences, or the OSError text. Py: open_for_change.
pub fn open_for_change(tree: &dyn Tree, slug_dir: &PathKey) -> Result<Opened, String> {
    if !slug_dir.components().is_empty() && !tree.is_dir(slug_dir) {
        return Err(format!("{}: no such recording folder", tree.display(slug_dir)));
    }
    let path = slug_dir.join("audio-labels.json");
    let shown = tree.display(&path);
    let folder_name = lexical_parent_name(&shown).to_string();
    if !tree.exists(&path) {
        let data = json!({"format": FORMAT, "slug": tree.resolved_name(slug_dir), "titles": {}, "tracks": []});
        return Ok(Opened { shown, folder_name, data, before: Vec::new() });
    }
    let data = match read_value(tree, &path)? {
        Ok(data) => data,
        Err(e) => return Err(format!("{shown}: not valid JSON ({e}); restore it from git (git checkout -- {shown})")),
    };
    let shaped = data.as_object().map_or(false, |d| get(d, "tracks").is_array() && get(d, "titles").is_object());
    if !shaped {
        return Err(format!("{shown}: has no 'tracks' list or 'titles' object; restore it from git (git checkout -- {shown})"));
    }
    let before = problems(&data, Some(&folder_name));
    Ok(Opened { shown, folder_name, data, before })
}

/// The value to write and the problems still left: Err "<shown>: not written: <the new problems>" when the change
/// adds a sentence the file did not have; otherwise tracks sorted (stably) by (str(label), the stream when an int or
/// a boolean, else -1), an entry that is not an object as ("", -1), and titles sorted by key. Py: save, without the write.
pub fn prepare_save(opened: &Opened, data: Value) -> Result<(Value, Vec<String>), String> {
    let mut data = data;
    let found = problems(&data, Some(&opened.folder_name));
    let new: Vec<&str> = found.iter().filter(|p| !opened.before.contains(*p)).map(String::as_str).collect();
    if !new.is_empty() {
        return Err(format!("{}: not written: {}", opened.shown, new.join("; ")));
    }
    if let Some(tracks) = data.get_mut("tracks").and_then(Value::as_array_mut) {
        tracks.sort_by(|a, b| track_sort_key(a).cmp(&track_sort_key(b)));
    }
    if let Some(titles) = data.get("titles").and_then(Value::as_object) {
        let mut items: Vec<(String, Value)> = titles.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        items.sort_by(|a, b| a.0.cmp(&b.0));
        data["titles"] = Value::Object(items.into_iter().collect());
    }
    Ok((data, found))
}

/// --film-audible: not given, or given as yes, no or unknown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilmAudible {
    Absent,
    Given(Option<bool>),
}

/// The arguments of `labels set`.
#[derive(Clone, Debug, PartialEq)]
pub struct SetTrack {
    pub label: String,
    pub stream: i64,
    pub kind: String,
    pub how: String,
    pub saw_verdict: bool,
    pub date: String,
    pub sure: bool,
    pub film_audible: FilmAudible,
    pub note: String,
}

/// The entry written or kept, whether it changed, why it does not count ("" when it does, "invalid" for an entry
/// with problems), the problems left, and the file to write (None when unchanged). Py: set_track's tuple.
#[derive(Clone, Debug, PartialEq)]
pub struct SetOutcome {
    pub entry: Value,
    pub changed: bool,
    pub why: String,
    pub left: Vec<String>,
    pub write: Option<Value>,
}

/// Adds or replaces the label of (label, stream): one old label with the same substance is kept as it is; otherwise
/// every old label of the track (Python's `==`) goes into the new entry's "history" (their own history first), with
/// "replaced" set to the new date. `fitted` is what `why` is worked out against. Py: set_track.
pub fn set_track(opened: &Opened, args: &SetTrack, fitted: &Fitted) -> Result<SetOutcome, String> {
    let mut entry = json!({"label": args.label, "stream": args.stream, "kind": args.kind, "how": args.how, "sure": args.sure,
                           "date": args.date, "saw_verdict": args.saw_verdict, "note": args.note});
    match args.film_audible {
        FilmAudible::Given(value) => entry["film_audible"] = json!(value),
        FilmAudible::Absent if COMMENTARY_KINDS.contains(&args.kind.as_str()) => entry["film_audible"] = Value::Null,
        FilmAudible::Absent => {}
    }
    let tracks: Vec<Value> = opened.data["tracks"].as_array().cloned().unwrap_or_default();
    let is_old = |t: &Value| t.as_object().map_or(false, |m| same_track(m, &args.label, args.stream));
    let old: Vec<&Value> = tracks.iter().filter(|t| is_old(*t)).collect();
    let slug = opened.data["slug"].as_str().unwrap_or_default();
    if old.len() == 1 && SUBSTANCE.iter().all(|k| py_eq(&absent_or(old[0], k), &absent_or(&entry, k))) {
        let kept = old[0].clone();
        let why = why_not(&kept, slug, fitted);
        return Ok(SetOutcome { entry: kept, changed: false, why, left: opened.before.clone(), write: None });
    }
    if !old.is_empty() {
        let mut history = Vec::new();
        for o in &old {
            if let Some(Value::Array(earlier)) = o.get("history") {
                history.extend(earlier.iter().cloned());
            }
            let mut copy: Map<String, Value> =
                o.as_object().into_iter().flatten().filter(|(k, _)| k.as_str() != "history").map(|(k, v)| (k.clone(), v.clone())).collect();
            copy.insert("replaced".to_string(), json!(args.date));
            history.push(Value::Object(copy));
        }
        entry["history"] = Value::Array(history);
    }
    let mut kept: Vec<Value> = tracks.iter().filter(|t| !is_old(*t)).cloned().collect();
    kept.push(entry.clone());
    let mut data = opened.data.clone();
    data["tracks"] = Value::Array(kept);
    let (write, left) = prepare_save(opened, data)?;
    let why = why_not(&entry, slug, fitted);
    Ok(SetOutcome { entry, changed: true, why, left, write: Some(write) })
}

/// Moves every label of (label, stream) to the file's "removed" list (added at the end of the file when absent), each
/// with "removed" and "why"; returns the file to write and the problems left. Py: unset_track.
pub fn unset_track(opened: &Opened, label: &str, stream: i64, why: &str, date: &str) -> Result<(Value, Vec<String>), String> {
    let tracks: Vec<Value> = opened.data["tracks"].as_array().cloned().unwrap_or_default();
    let is_old = |t: &Value| t.as_object().map_or(false, |m| same_track(m, label, stream));
    let old: Vec<Value> = tracks.iter().filter(|t| is_old(*t)).cloned().collect();
    if old.is_empty() {
        return Err(format!("{}: {label} stream {stream} has no label", opened.shown));
    }
    if why.is_empty() {
        return Err("unset needs --note: why the label is taken away".to_string());
    }
    let mut data = opened.data.clone();
    data["tracks"] = Value::Array(tracks.iter().filter(|t| !is_old(*t)).cloned().collect());
    let moved: Vec<Value> = old
        .into_iter()
        .map(|o| {
            let mut m = o.as_object().cloned().unwrap_or_default();
            m.insert("removed".to_string(), json!(date));
            m.insert("why".to_string(), json!(why));
            Value::Object(m)
        })
        .collect();
    let d = data.as_object_mut().expect("an opened file is an object");
    match d.get_mut("removed") {
        None => {
            d.insert("removed".to_string(), Value::Array(moved));
        }
        Some(Value::Array(list)) => list.extend(moved),
        Some(_) => return Err(format!("{}: 'removed' is not a list, so the label cannot be moved there", opened.shown)),
    }
    prepare_save(opened, data)
}

/// Sets one title's facts (replacing them in place when the title has some); returns the title's entry, the file to
/// write and the problems left. Py: set_title.
pub fn set_title(
    opened: &Opened,
    label: &str,
    title_kind: &str,
    dialogue_heavy: Option<bool>,
    note: &str,
) -> Result<(Value, Value, Vec<String>), String> {
    let mut data = opened.data.clone();
    let facts = json!({"title_kind": title_kind, "dialogue_heavy": dialogue_heavy, "note": note});
    data["titles"][label] = facts;
    let (written, left) = prepare_save(opened, data)?;
    let facts = written["titles"][label].clone();
    Ok((facts, written, left))
}

/// Appends one entry to the fitted file's value (a new file when None); returns the entry and the file to write.
/// Err "<shown>: does not validate, so nothing is added: ..." or "<shown>: not written: ...". Py: add_fitted, without
/// its folder check and its read (the command does both).
#[allow(clippy::too_many_arguments)]
pub fn add_fitted(
    existing: Option<&Value>,
    shown: &str,
    slug: &str,
    label: Option<&str>,
    stream: Option<i64>,
    rule: &str,
    note: &str,
    date: &str,
) -> Result<(Value, Value), String> {
    let mut data = existing.cloned().unwrap_or_else(|| json!({"format": FITTED_FORMAT, "fitted": []}));
    let before = fitted_problems(&data);
    if !before.is_empty() {
        return Err(format!("{shown}: does not validate, so nothing is added: {}", before.join("; ")));
    }
    let entry = json!({"slug": slug, "label": label, "stream": stream, "date": date, "rule": rule, "note": note});
    data["fitted"].as_array_mut().expect("a valid fitted file has a list").push(entry.clone());
    let found = fitted_problems(&data);
    if !found.is_empty() {
        return Err(format!("{shown}: not written: {}", found.join("; ")));
    }
    Ok((entry, data))
}

/// {ffprobe stream number: scan flags} of a rip's audio streams, the k-th audio stream of the file (the first
/// `audio-levels-20ms-*/streams.json` of the label) matched with the k-th of the scan's title (the rip summary's
/// "title"), as Task 12 Step 15 of the phase 0 plan does; Err with the Python's reason. Py: _scan_flags.
pub fn scan_flags(tree: &dyn Tree, slug_dir: &PathKey, label: &str) -> Result<BTreeMap<i64, i64>, String> {
    scan_flags_checked(tree, slug_dir, label).and_then(|flags| flags)
}

/// Labels every measured candidate that needs no listening: an exact duplicate (its packet MD5 equals the main
/// stream's or an earlier candidate's) and a derived core (scan flag 2048); a track that has a label already is never
/// changed. The labels are applied in the Python's order to one value, each as `set_track` does with the problems
/// of the value so far, so the file written once is what the Python's last save left. Returns the lines to print and
/// the file to write (None when nothing changed). Err where the Python stopped with an exception (a change that would
/// add a problem, a manifest or streams.json that cannot be read, a cand= that is not an int). Py: auto.
pub fn auto(tree: &dyn Tree, slug_dir: &PathKey, opened: &Opened, date: &str, fitted: &Fitted) -> Result<(Vec<String>, Option<Value>), String> {
    let mut current = opened.data.clone();
    let mut changed = false;
    let mut have: Vec<(Value, Value)> = opened.data["tracks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
        .map(|m| (get(m, "label").clone(), get(m, "stream").clone()))
        .collect();
    let mut lines = Vec::new();
    for label_dir in tree.list(slug_dir) {
        if !tree.is_dir(&label_dir) {
            continue;
        }
        let folders: Vec<PathKey> =
            tree.glob(&label_dir, "audio-levels-20ms-*").into_iter().filter(|f| tree.is_file(&f.join("manifest.txt"))).collect();
        if folders.is_empty() {
            continue;
        }
        let label = label_dir.name().to_string();
        let flags = match scan_flags_checked(tree, slug_dir, &label)? {
            Ok(flags) => Some(flags),
            Err(why) => {
                lines.push(format!("{label}: derived cores not labelled: {why}"));
                None
            }
        };
        for folder in &folders {
            let manifest = read_manifest(tree, folder)?;
            let Some(main) = manifest.main.as_ref().filter(|main| !main.is_empty()) else { continue };
            let mut seen: Vec<(Option<String>, Option<String>)> = vec![(main.get("md5").cloned(), main.get("main").cloned())];
            for cand in &manifest.cands {
                let number_text = cand.get("cand").cloned().unwrap_or_default();
                let n = py_int(&number_text)
                    .ok_or_else(|| format!("invalid literal for int() with base 10: {}", py_repr_str(&number_text)))?;
                let md5 = cand.get("md5").cloned();
                let mut what: Option<(&str, &str, String)> = None;
                if let Some((_, first)) = seen.iter().find(|(m, _)| *m == md5) {
                    what = Some(("duplicate", "md5", format!("auto: same packets as stream {} ({})", or_none(first), or_none(&md5))));
                } else if let Some(known) = flags.as_ref().filter(|known| !known.is_empty()) {
                    let bits = known.get(&n).copied().unwrap_or(0);
                    if bits & DERIVED != 0 {
                        what = Some(("derived-core", "flag", format!("auto: scan flags {bits} (2048: derived by MakeMKV)")));
                    }
                }
                if !seen.iter().any(|(m, _)| *m == md5) {
                    seen.push((md5.clone(), Some(number_text.clone())));
                }
                let Some((kind, how, note)) = what else { continue };
                let key = (Value::from(label.clone()), Value::from(n));
                if have.iter().any(|(l, s)| py_eq(l, &key.0) && py_eq(s, &key.1)) {
                    continue;
                }
                let step = Opened {
                    shown: opened.shown.clone(),
                    folder_name: opened.folder_name.clone(),
                    before: problems(&current, Some(&opened.folder_name)),
                    data: current.clone(),
                };
                let args = SetTrack {
                    label: label.clone(),
                    stream: n,
                    kind: kind.to_string(),
                    how: how.to_string(),
                    saw_verdict: false,
                    date: date.to_string(),
                    sure: true,
                    film_audible: FilmAudible::Absent,
                    note,
                };
                let outcome = set_track(&step, &args, fitted)?;
                if let Some(written) = outcome.write {
                    current = written;
                    changed = true;
                }
                have.push(key);
                let counts = if outcome.why.is_empty() { "True" } else { "False" };
                let why = if outcome.why.is_empty() { String::new() } else { format!(" ({})", outcome.why) };
                lines.push(format!(
                    "labelled {label} stream {n}: {} ({}), counts={counts}{why}",
                    py_str(&outcome.entry["kind"]),
                    py_str(&outcome.entry["how"])
                ));
            }
        }
    }
    if lines.is_empty() {
        lines.push("nothing to label automatically".to_string());
    }
    Ok((lines, if changed { Some(current) } else { None }))
}

/// `labels kinds`' lines: each kind, its group, what the owner wants and its meaning (padded to 20, 10 and 8
/// characters), then the sources. Py: main's kinds branch.
pub fn kinds_lines() -> Vec<String> {
    let mut lines: Vec<String> =
        KINDS.iter().map(|(kind, group, wanted, meaning)| format!("{kind:<20} {group:<10} {wanted:<8} {meaning}")).collect();
    let how: Vec<String> = HOW.iter().map(|(name, meaning)| format!("{name} = {meaning}")).collect();
    lines.push(format!("how: {}", how.join("; ")));
    lines
}

/// "warning: the file still has <n> problem(s): ..." or nothing. Py: left_lines.
pub fn left_lines(left: &[String]) -> Vec<String> {
    if left.is_empty() {
        Vec::new()
    } else {
        vec![format!("warning: the file still has {} problem(s): {}", left.len(), left.join("; "))]
    }
}

/// The labels part of `labels check <root>`, shared with `labels --subtitles check` (RP15): root/<file_name> when it
/// exists, else every */<file_name> (never one under golden/), each "ok <file> (<what load_count says of it>)" or the
/// refusal `load_count` gives, which makes the file a bad one; "<root>: no <file_name>" when there is none. Returns
/// the lines and the number of bad files. Public because the dev crate's subtitle `check` calls it. Py: main's check
/// branch.
pub fn labels_check_lines(
    tree: &dyn Tree,
    root: &PathKey,
    file_name: &str,
    load_count: &dyn Fn(&dyn Tree, &PathKey) -> Result<String, String>,
) -> (Vec<String>, usize) {
    let mut lines = Vec::new();
    let mut bad = 0;
    let own = root.join(file_name);
    let files = if tree.exists(&own) { vec![own] } else { tree.glob(root, &format!("*/{file_name}")) };
    if files.is_empty() {
        lines.push(format!("{}: no {file_name}", tree.display(root)));
    }
    for path in &files {
        match load_count(tree, path) {
            Ok(count) => lines.push(format!("ok {} ({count})", tree.display(path))),
            Err(e) => {
                lines.push(e);
                bad += 1;
            }
        }
    }
    (lines, bad)
}

/// `labels check <root>`'s lines for one root and the number of bad files: root/audio-labels.json when it exists,
/// else every */audio-labels.json (never one under golden/), then root/audio-fitted.json when it exists.
/// Py: main's check branch.
pub fn check_lines(tree: &dyn Tree, root: &PathKey) -> (Vec<String>, usize) {
    let (mut lines, mut bad) = labels_check_lines(tree, root, "audio-labels.json", &|tree, path| {
        load(tree, path).map(|labels| format!("{} tracks, {} titles", labels.tracks.len(), labels.titles.len()))
    });
    let fitted_path = root.join("audio-fitted.json");
    if tree.exists(&fitted_path) {
        match load_fitted(tree, root, None) {
            Ok(fitted) => lines.push(format!("ok {} ({} entries)", tree.display(&fitted_path), fitted.len())),
            Err(e) => {
                lines.push(e);
                bad += 1;
            }
        }
    }
    (lines, bad)
}

/// yes/no (and unknown) as the Python's _yes: the text lower-cased; Err "'<text>': say yes or no[ or unknown]".
pub fn yes_no_unknown(text: &str, allow_unknown: bool) -> Result<Option<bool>, String> {
    match text.to_lowercase().as_str() {
        "yes" => Ok(Some(true)),
        "no" => Ok(Some(false)),
        "unknown" if allow_unknown => Ok(None),
        _ => Err(format!("{}: say yes or no{}", py_repr_str(text), if allow_unknown { " or unknown" } else { "" })),
    }
}

/// labels.json (spec section 5): `labels check` of the recording and whether each labelled track counts.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LabelsJson {
    pub check: Vec<String>,
    pub exit_code: i32,
    pub tracks: Vec<LabelCount>,
}

/// One labelled track and why it does not count ("" when it does).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LabelCount {
    pub label: String,
    pub stream: i64,
    pub kind: String,
    pub how: String,
    pub not_counted_because: String,
}

/// check_lines of the recording folder (exit_code 2 when a file is bad, else 0), and for a labels file that
/// validates, not_counted_because of every track in file order against the file's slug and `fitted` (row 13's disc
/// plus the corpus's audio-fitted.json, which the caller loads); no tracks when the file is absent or bad.
pub fn labels_json(tree: &dyn Tree, slug_dir: &PathKey, fitted: &Fitted) -> LabelsJson {
    let (check, bad) = check_lines(tree, slug_dir);
    let tracks = match load(tree, &slug_dir.join("audio-labels.json")) {
        Ok(labels) => labels
            .tracks
            .iter()
            .map(|e| LabelCount {
                label: e.label.clone(),
                stream: e.stream,
                kind: e.kind.clone(),
                how: e.how.clone(),
                not_counted_because: not_counted_because(e, &labels.slug, fitted),
            })
            .collect(),
        Err(_) => Vec::new(),
    };
    LabelsJson { check, exit_code: if bad > 0 { 2 } else { 0 }, tracks }
}

/// `_scan_flags` with the two ways it can fail apart: the outer Err where the Python raised (an attribute 22 that is
/// not an int, a streams.json that cannot be read or is not JSON, a rip summary or streams.json of the wrong shape),
/// the inner Err for the reasons it returned.
fn scan_flags_checked(tree: &dyn Tree, slug_dir: &PathKey, label: &str) -> Result<Result<BTreeMap<i64, i64>, String>, String> {
    let summary = slug_dir.join(label).join("rip-summary.json");
    let title = match read_value(tree, &summary) {
        Err(e) | Ok(Err(e)) => return Ok(Err(format!("no scan or rip summary ({e})"))),
        Ok(Ok(Value::Object(m))) => match m.get("title") {
            Some(title) => title.clone(),
            None => return Ok(Err("no scan or rip summary ('title')".to_string())),
        },
        Ok(Ok(_)) => return Err(format!("{}: not a JSON object", tree.display(&summary))),
    };
    let text = match read_text(tree, &slug_dir.join("scan.txt"), Encoding::Utf8Sig, Errors::Replace) {
        Ok(text) => text,
        Err(e) => return Ok(Err(format!("no scan or rip summary ({e})"))),
    };
    let scan = parse(&text);
    let mut flags = Vec::new();
    for ((t, _), attrs) in &scan.streams {
        if py_eq(&Value::from(*t), &title) && attrs.get(&1).map(String::as_str) == Some("Audio") {
            flags.push(int_attr(attrs, 22)?);
        }
    }
    let Some(first) = tree.glob(&slug_dir.join(label), "audio-levels-20ms-*/streams.json").into_iter().next() else {
        return Ok(Err("no streams.json".to_string()));
    };
    let shown = tree.display(&first);
    let value = read_value(tree, &first)??;
    let streams: &[Value] = match value.as_object() {
        Some(m) => match m.get("streams") {
            None => &[],
            Some(Value::Array(list)) => list,
            Some(_) => return Err(format!("{shown}: streams is not a list")),
        },
        None => return Err(format!("{shown}: not a JSON object")),
    };
    let mut audio = Vec::new();
    for s in streams {
        let Some(sm) = s.as_object() else { return Err(format!("{shown}: a stream is not an object")) };
        if get(sm, "codec_type").as_str() == Some("audio") {
            let Some(index) = get(sm, "index").as_i64() else { return Err(format!("{shown}: an audio stream has no integer index")) };
            audio.push(index);
        }
    }
    if audio.len() != flags.len() {
        return Ok(Err(format!(
            "the file has {} audio streams and the scan's title {} has {}",
            audio.len(),
            py_str(&title),
            flags.len()
        )));
    }
    Ok(Ok(audio.into_iter().zip(flags).collect()))
}

/// json.loads(path.read_text(encoding="utf-8-sig")): Err(text) where the read failed (Python's OSError, which no
/// caller catches), Ok(Err(text)) where the bytes are not UTF-8 or not JSON (Python's ValueError, which callers catch).
pub(crate) fn read_value(tree: &dyn Tree, path: &PathKey) -> Result<Result<Value, String>, String> {
    let bytes = tree.read(path).map_err(|e| e.to_string())?;
    Ok(decode_text(&bytes, Encoding::Utf8Sig, Errors::Strict)
        .and_then(|text| serde_json::from_str::<Value>(&text).map_err(|e| e.to_string())))
}

/// Path(shown).name: the last component of a path as pathlib prints it, "" for "." and for a root.
fn lexical_name(shown: &str) -> &str {
    let last = shown.rsplit(|c: char| c == '/' || c == '\\').next().unwrap_or("");
    if last == "." {
        ""
    } else {
        last
    }
}

/// Path(shown).parent.name: "" when the path has one component (its parent is ".").
fn lexical_parent_name(shown: &str) -> &str {
    match shown.rfind(|c: char| c == '/' || c == '\\') {
        Some(i) => lexical_name(&shown[..i]),
        None => "",
    }
}

/// Python's `==` between values json.loads gives: bool, int and float compare as numbers (True == 1 == 1.0, ints and
/// floats by exact value), str with str, None with None, lists item by item, dicts key by key.
fn py_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Null, Value::Null) => true,
        (Value::String(x), Value::String(y)) => x == y,
        (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| py_eq(p, q)),
        (Value::Object(x), Value::Object(y)) => x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).map_or(false, |w| py_eq(v, w))),
        _ => match (py_number(a), py_number(b)) {
            (Some(x), Some(y)) => num_eq(x, y),
            _ => false,
        },
    }
}

/// A Python number: an int (bool included) or a float.
#[derive(Clone, Copy)]
enum PyNum {
    Int(i128),
    Float(f64),
}

fn py_number(value: &Value) -> Option<PyNum> {
    match value {
        Value::Bool(b) => Some(PyNum::Int(i128::from(*b))),
        Value::Number(n) => n
            .as_i64()
            .map(|i| PyNum::Int(i128::from(i)))
            .or_else(|| n.as_u64().map(|u| PyNum::Int(i128::from(u))))
            .or_else(|| n.as_f64().map(PyNum::Float)),
        _ => None,
    }
}

fn num_eq(a: PyNum, b: PyNum) -> bool {
    match (a, b) {
        (PyNum::Int(x), PyNum::Int(y)) => x == y,
        (PyNum::Float(x), PyNum::Float(y)) => x == y,
        (PyNum::Int(i), PyNum::Float(f)) | (PyNum::Float(f), PyNum::Int(i)) => {
            f.is_finite() && f.fract() == 0.0 && f.abs() < 1e38 && f as i128 == i
        }
    }
}

/// Python's None for a key that is absent.
static NULL: Value = Value::Null;

/// `m.get(key)`, None (JSON null) when absent.
pub(crate) fn get<'a>(m: &'a Map<String, Value>, key: &str) -> &'a Value {
    m.get(key).unwrap_or(&NULL)
}

/// `{m.get(key)!r}`.
pub(crate) fn repr_of(m: &Map<String, Value>, key: &str) -> String {
    py_repr(get(m, key))
}

/// `sorted(names)` as Python prints a list of str: ['a', 'b'].
fn sorted_list(mut names: Vec<&str>) -> String {
    names.sort_unstable();
    let items: Vec<String> = names.iter().map(|name| py_repr_str(name)).collect();
    format!("[{}]", items.join(", "))
}

/// `value in (True, False, None)` (absent is None; 0, 1, 0.0 and 1.0 are `==` to False or True).
fn true_false_none(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) | Some(Value::Bool(_)) => true,
        Some(other) => py_eq(other, &Value::from(0)) || py_eq(other, &Value::from(1)),
    }
}

/// An int (not a bool, not a float) of 0 or more that fits an i64. Py: the stream checks.
fn is_stream_number(value: &Value) -> bool {
    value.as_i64().map_or(false, |n| n >= 0)
}

/// `(t.get("label"), t.get("stream")) == (label, stream)` with Python's `==`.
fn same_track(t: &Map<String, Value>, label: &str, stream: i64) -> bool {
    py_eq(get(t, "label"), &Value::from(label)) && py_eq(get(t, "stream"), &Value::from(stream))
}

/// `t.get(key, "absent")`.
fn absent_or(t: &Value, key: &str) -> Value {
    t.get(key).cloned().unwrap_or_else(|| Value::from("absent"))
}

/// save's sort key: (str(label), the stream when isinstance(stream, int), a bool as 0 or 1, else -1); ("", -1) for
/// an entry that is not an object.
pub(crate) fn track_sort_key(t: &Value) -> (String, i64) {
    match t.as_object() {
        Some(m) => {
            let label = py_str(get(m, "label"));
            let stream = match get(m, "stream") {
                Value::Bool(b) => i64::from(*b),
                other => other.as_i64().unwrap_or(-1),
            };
            (label, stream)
        }
        None => (String::new(), -1),
    }
}

/// not_counted_because of an entry that validates, "invalid" otherwise. Py: set_track's `why`.
fn why_not(entry: &Value, slug: &str, fitted: &Fitted) -> String {
    match LabelEntry::from_value(entry) {
        Some(e) => not_counted_because(&e, slug, fitted),
        None => "invalid".to_string(),
    }
}

/// A number row 13 states in digits or in words. Py: `int(v) if v.isdigit() else WORDS[v]`.
fn number(text: &str) -> Result<i64, String> {
    if let Some(n) = py_int(text) {
        return Ok(n);
    }
    WORDS
        .iter()
        .find(|(word, _)| *word == text)
        .map(|&(_, n)| n)
        .ok_or_else(|| format!("invalid literal for int() with base 10: {}", py_repr_str(text)))
}

/// The names of a (name, meaning) table, in order: the `how` vocabulary of these labels and of the subtitle labels
/// (`subtitle::labels`, RP15).
pub(crate) fn names<'a>(table: &[(&'a str, &'a str)]) -> Vec<&'a str> {
    table.iter().map(|&(name, _)| name).collect()
}

/// The first checks of a labels file, shared with the subtitle labels (RP15): the top-level keys that are not in
/// `top_keys`, the format line against `format`, and the slug, compared with `folder` when it is given. Py: problems'
/// first checks.
pub(crate) fn head_problems(d: &Map<String, Value>, top_keys: &[&str], format: &str, folder: Option<&str>) -> Vec<String> {
    let mut out = Vec::new();
    let extra: Vec<&str> = d.keys().map(String::as_str).filter(|k| !top_keys.contains(k)).collect();
    if !extra.is_empty() {
        out.push(format!("unknown top-level keys {}", sorted_list(extra)));
    }
    if get(d, "format").as_str() != Some(format) {
        out.push(format!("format is {}, not {}", repr_of(d, "format"), py_repr_str(format)));
    }
    match get(d, "slug").as_str() {
        Some(own) if is_slug(own) => {
            if let Some(folder) = folder {
                if own != folder {
                    out.push(format!("slug is {} but the file is in corpus/owner/{folder}", py_repr_str(own)));
                }
            }
        }
        _ => out.push(format!("slug {} is not lowercase letters, digits and hyphens", repr_of(d, "slug"))),
    }
    out
}

/// The problems of a labels file's `removed` list, shared with the subtitle labels (RP15): each entry read with
/// `loose` (the file's own entry check with strict false) and its `removed` date. Py: problems' removed loop.
pub(crate) fn removed_problems(d: &Map<String, Value>, loose: impl Fn(&Value, &str) -> Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    match d.get("removed") {
        None => {}
        Some(Value::Array(removed)) => {
            for (k, r) in removed.iter().enumerate() {
                let Some(rm) = r.as_object() else {
                    out.push(format!("removed {k}: not an object"));
                    continue;
                };
                out.extend(loose(r, &format!("removed {k}")));
                if !date_ok(get(rm, "removed")) {
                    out.push(format!("removed {k}: removed {} is not YYYY-MM-DD", repr_of(rm, "removed")));
                }
            }
        }
        Some(_) => out.push("removed is not a list".to_string()),
    }
    out
}

/// The checks every entry of a labels file has, strict or not, shared with the subtitle labels (RP15): the keys of
/// `track_keys` that are missing, the keys in neither `track_keys` nor `allowed`, the label, the stream and the date,
/// each sentence prefixed with `where_`. Py: entry_problems' first checks.
pub(crate) fn base_entry_problems(m: &Map<String, Value>, where_: &str, track_keys: &[&str], allowed: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    let missing: Vec<&str> = track_keys.iter().copied().filter(|k| !m.contains_key(*k)).collect();
    let unknown: Vec<&str> = m.keys().map(String::as_str).filter(|k| !track_keys.contains(k) && !allowed.contains(k)).collect();
    if !missing.is_empty() {
        out.push(format!("{where_}: missing {}", sorted_list(missing)));
    }
    if !unknown.is_empty() {
        out.push(format!("{where_}: unknown keys {}", sorted_list(unknown)));
    }
    if !get(m, "label").as_str().map_or(false, is_slug) {
        out.push(format!("{where_}: label is not a rip label"));
    }
    if !is_stream_number(get(m, "stream")) {
        out.push(format!("{where_}: stream is not ffprobe's stream number (an integer, 0 or more)"));
    }
    if !date_ok(get(m, "date")) {
        out.push(format!("{where_}: date {} is not YYYY-MM-DD", repr_of(m, "date")));
    }
    out
}

/// The problems of a strict entry's `history` list, shared with the subtitle labels (RP15): each earlier label read
/// with `loose` (the file's own entry check with strict false) and its `replaced` date. Py: entry_problems' history
/// loop.
pub(crate) fn history_problems(m: &Map<String, Value>, where_: &str, loose: impl Fn(&Value, &str) -> Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    match m.get("history") {
        None => {}
        Some(Value::Array(history)) => {
            for (k, h) in history.iter().enumerate() {
                let Some(hm) = h.as_object() else {
                    out.push(format!("{where_}: history {k} is not an object"));
                    continue;
                };
                out.extend(loose(h, &format!("{where_} history {k}")));
                if !date_ok(get(hm, "replaced")) {
                    out.push(format!("{where_} history {k}: replaced {} is not YYYY-MM-DD", repr_of(hm, "replaced")));
                }
            }
        }
        Some(_) => out.push(format!("{where_}: history is not a list")),
    }
    out
}

/// str() of an optional manifest value: the text, or "None".
fn or_none(value: &Option<String>) -> &str {
    value.as_deref().unwrap_or("None")
}

#[cfg(test)]
mod tests;
