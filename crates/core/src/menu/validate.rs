//! The writer's validator over a parsed map (design 5.1): the Python writer's rules, in its order, with its sentences
//! (`%r` is `py_repr`). `docs/menu-maps/format.md` lists the rules with the name of the shared case that pins each.
//! The writer's last rule, "map cannot be saved", has no Rust case: a lone surrogate, NaN or an infinity never reaches
//! a parsed `Value` (M3-7).

use serde_json::{Map, Value};

use super::{Applies, Kind, Level, Surface, FORMAT, REPLAYS_NEEDED};
use crate::recording::{py_repr, py_repr_str, py_truthy};

/// What a missing key reads as, as Python's `dict.get` gives None.
static NULL: Value = Value::Null;

/// The keys every effect and every check holds, in the format's order.
const EFFECT_KEYS: [&str; 4] = ["playlist", "audio", "subtitle", "subtitle_shown"];
const CHECK_KEYS: [&str; 6] = ["label_reread", "level", "replays", "replays_equal", "player", "note"];

/// The slug's pattern as the sentences spell it.
const SLUG: &str = "^[a-z0-9][a-z0-9-]*$";

/// The sentence of play and of a choice whose effect is null above level `read` or without a note.
const EFFECT_NULL_UNLESS_READ: &str = "effect may be null only at level 'read' with a note";

/// An integer, never a float or a bool (Python's `isinstance(v, int) and not isinstance(v, bool)`).
fn is_int(value: &Value) -> bool {
    matches!(value, Value::Number(number) if number.is_i64() || number.is_u64())
}

fn slug_fits(text: &str) -> bool {
    let mut chars = text.chars();
    let lower_or_digit = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit();
    chars.next().map_or(false, lower_or_digit) && chars.all(|c| lower_or_digit(c) || c == '-')
}

/// `c` and three ASCII digits.
fn id_fits(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 4 && bytes[0] == b'c' && bytes[1..].iter().all(u8::is_ascii_digit)
}

/// What is wrong with a relative path held as a string, on the string alone (never `std::path`, which differs on
/// Windows): a leading `/` or `\`, a drive letter, or a `..` part split on both separators.
fn path_problem(value: &Value) -> Option<&'static str> {
    let Value::String(text) = value else {
        return Some("is not a string");
    };
    let bytes = text.as_bytes();
    if text.is_empty() {
        Some("is empty")
    } else if text.starts_with('/')
        || text.starts_with('\\')
        || (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
    {
        Some("is not a relative path")
    } else if text.split(['/', '\\']).any(|part| part == "..") {
        Some("has a \"..\" part")
    } else {
        None
    }
}

/// What is wrong with a label: not a string, empty, or holding a control character (Cc) or a line or paragraph
/// separator (U+2028, U+2029). A lone surrogate cannot reach a Rust string.
fn label_problem(value: &Value) -> Option<&'static str> {
    let Value::String(text) = value else {
        return Some("is not a string");
    };
    if text.is_empty() {
        Some("is empty")
    } else if text.chars().any(super::scan::breaks_a_line) {
        Some("has a tab, line break, control character or lone surrogate")
    } else {
        None
    }
}

/// `d[key]`, or a sentence when it is missing.
fn need<'a>(d: &'a Map<String, Value>, key: &str, place: &str, errors: &mut Vec<String>) -> Option<&'a Value> {
    let value = d.get(key);
    if value.is_none() {
        errors.push(format!("{place}: \"{key}\" is missing"));
    }
    value
}

/// The top-level section `m[key]` when `fits` it; otherwise a sentence and None.
fn section<'a>(
    m: &'a Map<String, Value>,
    key: &str,
    fits: fn(&Value) -> bool,
    errors: &mut Vec<String>,
) -> Option<&'a Value> {
    match m.get(key) {
        None => {
            errors.push(format!("\"{key}\" is missing"));
            None
        }
        Some(value) if !fits(value) => {
            errors.push(format!("\"{key}\" has the wrong type"));
            None
        }
        Some(value) => Some(value),
    }
}

fn check_effect(effect: &Value, place: &str, errors: &mut Vec<String>) {
    let Value::Object(effect) = effect else {
        errors.push(format!("{place}: effect is not an object"));
        return;
    };
    for key in EFFECT_KEYS {
        if !effect.contains_key(key) {
            errors.push(format!("{place}: effect is missing \"{key}\""));
        }
    }
    if effect.get("playlist").map_or(false, |value| !value.is_string()) {
        errors.push(format!("{place}: effect playlist is not a string"));
    }
    for key in ["audio", "subtitle"] {
        if effect.get(key).map_or(false, |value| !value.is_null() && !is_int(value)) {
            errors.push(format!("{place}: effect {key} is not a number or null"));
        }
    }
    if effect.get("subtitle_shown").map_or(false, |value| !value.is_boolean()) {
        errors.push(format!("{place}: effect subtitle_shown is not true or false"));
    }
}

/// The check block's sentences; returns its level (any value, None when missing) and its note when a string.
fn check_check<'a>(check: &'a Value, place: &str, errors: &mut Vec<String>) -> (Option<&'a Value>, Option<&'a str>) {
    let Value::Object(check) = check else {
        errors.push(format!("{place}: check is not an object"));
        return (None, None);
    };
    for key in CHECK_KEYS {
        if !check.contains_key(key) {
            errors.push(format!("{place}: check is missing \"{key}\""));
        }
    }
    let level = check.get("level");
    if let Some(level) = level {
        if !Level::ALL.iter().any(|known| level.as_str() == Some(known.word())) {
            let words = Level::ALL.map(Level::word).join(", ");
            errors.push(format!("{place}: level {} is not one of {words}", py_repr(level)));
        }
    }
    if is_level(level, Level::Replayed) {
        let replays = check.get("replays").unwrap_or(&NULL);
        let enough = is_int(replays) && replays.as_i64().map_or(true, |count| count >= REPLAYS_NEEDED);
        if !(enough && check.get("replays_equal") == Some(&Value::Bool(true))) {
            errors.push(format!("{place}: level replayed needs replays >= {REPLAYS_NEEDED} and replays_equal true"));
        }
    }
    if check.get("player").map_or(false, |value| !value.is_boolean()) {
        errors.push(format!("{place}: check.player is not true or false"));
    }
    let note = check.get("note");
    if note.map_or(false, |value| !value.is_string()) {
        errors.push(format!("{place}: check.note is not a string"));
    }
    (level, note.and_then(Value::as_str))
}

fn is_level(level: Option<&Value>, want: Level) -> bool {
    level.and_then(Value::as_str) == Some(want.word())
}

fn read_with_note(level: Option<&Value>, note: Option<&str>) -> bool {
    is_level(level, Level::Read) && note.map_or(false, |note| !note.is_empty())
}

fn label(d: &Map<String, Value>, place: &str, errors: &mut Vec<String>) {
    if let Some(value) = need(d, "label", place, errors) {
        if let Some(problem) = label_problem(value) {
            errors.push(format!("{place}: label {problem}"));
        }
    }
}

fn script(d: &Map<String, Value>, place: &str, errors: &mut Vec<String>, nullable: bool) {
    let Some(value) = need(d, "script", place, errors) else {
        return;
    };
    if value.is_null() && nullable {
        return;
    }
    if let Some(problem) = path_problem(value) {
        errors.push(format!("{place}: script {problem}"));
    }
}

fn validate_play(play: &Map<String, Value>, errors: &mut Vec<String>) {
    let place = "play";
    label(play, place, errors);
    let (level, note) = match play.get("check") {
        Some(check) => check_check(check, place, errors),
        None => {
            errors.push(format!("{place}: \"check\" is missing"));
            (None, None)
        }
    };
    let script = need(play, "script", place, errors);
    let effect = need(play, "effect", place, errors);
    if let Some(script) = script.filter(|script| !script.is_null()) {
        if let Some(problem) = path_problem(script) {
            errors.push(format!("{place}: script {problem}"));
        }
    }
    match (script, effect) {
        (Some(script), Some(effect)) => {
            if script.is_null() && effect.is_null() {
                if !read_with_note(level, note) {
                    errors.push(format!(
                        "{place}: script and effect may be null together only at level 'read' with a note"
                    ));
                }
            } else if script.is_null() {
                errors.push(format!("{place}: script is null but the effect is not"));
            } else if effect.is_null() && !read_with_note(level, note) {
                errors.push(format!("{place}: {EFFECT_NULL_UNLESS_READ}"));
            }
        }
        (None, Some(effect)) if effect.is_null() && !read_with_note(level, note) => {
            errors.push(format!("{place}: {EFFECT_NULL_UNLESS_READ}"));
        }
        _ => {}
    }
    if let Some(effect) = effect.filter(|effect| !effect.is_null()) {
        check_effect(effect, place, errors);
    }
}

fn validate_choice(
    choice: &Value,
    index: usize,
    play_effect: Option<&Value>,
    seen: &mut Vec<String>,
    errors: &mut Vec<String>,
) {
    let mut place = format!("choices[{index}]");
    let Value::Object(choice) = choice else {
        errors.push(format!("{place}: not an object"));
        return;
    };
    if let Some(id) = need(choice, "id", &place, errors) {
        match id.as_str().filter(|id| id_fits(id)) {
            None => errors.push(format!("{place}: id {} does not match c000", py_repr(id))),
            Some(id) => {
                place = format!("{place} ({id})");
                if seen.iter().any(|known| known == id) {
                    errors.push(format!("{place}: id is used twice"));
                } else {
                    seen.push(id.to_string());
                }
            }
        }
    }
    if need(choice, "section", &place, errors).map_or(false, |value| !value.is_string()) {
        errors.push(format!("{place}: section is not a string"));
    }
    label(choice, &place, errors);
    let kinds = Kind::ALL.map(Kind::word);
    let surfaces = Surface::ALL.map(Surface::word);
    let applies = Applies::ALL.map(Applies::word);
    for (key, allowed) in [("kind", &kinds[..]), ("surface", &surfaces[..]), ("applies", &applies[..])] {
        if let Some(value) = need(choice, key, &place, errors) {
            if !allowed.iter().any(|word| value.as_str() == Some(*word)) {
                errors.push(format!("{place}: {key} {} is not one of {}", py_repr(value), allowed.join(", ")));
            }
        }
    }
    let (level, note) = match choice.get("check") {
        Some(check) => check_check(check, &place, errors),
        None => {
            errors.push(format!("{place}: \"check\" is missing"));
            (None, None)
        }
    };
    if let Some(image) = need(choice, "label_image", &place, errors) {
        if image.is_null() {
            if !is_level(level, Level::Read) {
                errors.push(format!("{place}: label_image may be null only at level 'read'"));
            }
        } else if let Some(problem) = path_problem(image) {
            errors.push(format!("{place}: label_image {problem}"));
        }
    }
    script(choice, &place, errors, false);
    let effect = need(choice, "effect", &place, errors);
    if let Some(effect) = effect {
        if effect.is_null() {
            if !read_with_note(level, note) {
                errors.push(format!("{place}: {EFFECT_NULL_UNLESS_READ}"));
            }
        } else {
            check_effect(effect, &place, errors);
        }
    }
    let differs = need(choice, "differs_from_play", &place, errors);
    // As the writer does: whenever both effects are objects, whatever their keys' types.
    let both = (differs, effect, play_effect);
    if let (Some(differs), Some(Value::Object(effect)), Some(Value::Object(play_effect))) = both {
        match differs {
            Value::Bool(differs) => {
                let say = !py_effects_equal(effect, play_effect);
                if *differs != say {
                    errors.push(format!("{place}: differs_from_play is {differs} but the effects say {say}"));
                }
            }
            _ => errors.push(format!("{place}: differs_from_play is not true or false")),
        }
    }
}

/// Every problem of the parsed map as the writer's sentences, in its order (empty when the map is valid). Never panics
/// on a malformed map. Design 5.1; the rules are listed in `docs/menu-maps/format.md`.
pub fn validate(map: &Value) -> Vec<String> {
    let Value::Object(m) = map else {
        return vec!["the map is not an object".to_string()];
    };
    let mut errors = Vec::new();
    let format = m.get("format").unwrap_or(&NULL);
    if format.as_str() != Some(FORMAT) {
        errors.push(format!("format is {}, not {}", py_repr(format), py_repr_str(FORMAT)));
    }
    if let Some(Value::Object(disc)) = section(m, "disc", Value::is_object, &mut errors) {
        if let Some(slug) = need(disc, "slug", "disc", &mut errors) {
            if !slug.as_str().map_or(false, slug_fits) {
                errors.push(format!("disc: slug {} does not match {SLUG}", py_repr(slug)));
            }
        }
    }
    section(m, "made", Value::is_object, &mut errors);
    if let Some(Value::Object(start)) = section(m, "start", Value::is_object, &mut errors) {
        script(start, "start", &mut errors, true);
        if let Some(passed) = need(start, "passed", "start", &mut errors) {
            if !passed.as_array().map_or(false, |items| items.iter().all(Value::is_string)) {
                errors.push("start: passed is not a list of strings".to_string());
            }
        }
        // Every number a parsed Value holds is finite; a bool is not a number here.
        if let Some(seconds) = need(start, "seconds_to_menu", "start", &mut errors) {
            if !seconds.is_null() && !seconds.is_number() {
                errors.push("start: seconds_to_menu is not a finite number or null".to_string());
            }
        }
    }
    let mut play_effect = None;
    if let Some(Value::Object(play)) = section(m, "play", Value::is_object, &mut errors) {
        validate_play(play, &mut errors);
        play_effect = play.get("effect");
    }
    if let Some(Value::Array(choices)) = section(m, "choices", Value::is_array, &mut errors) {
        let mut seen = Vec::new();
        for (index, choice) in choices.iter().enumerate() {
            validate_choice(choice, index, play_effect, &mut seen, &mut errors);
        }
    }
    if let Some(Value::Array(items)) = section(m, "not_mapped", Value::is_array, &mut errors) {
        if !items.iter().all(Value::is_string) {
            errors.push("not_mapped is not a list of strings".to_string());
        }
    }
    // The items of quirks are free-form: a later version may add structured quirks.
    section(m, "quirks", Value::is_array, &mut errors);
    errors
}

/// The writer's `effects_equal` over two raw effect objects, with Python's `==` (`py_eq`): the same playlist, audio and
/// `subtitle_shown`, and the same subtitle when `subtitle_shown` is truthy (`py_truthy`). A missing key is null.
fn py_effects_equal(a: &Map<String, Value>, b: &Map<String, Value>) -> bool {
    let same = |key: &str| py_eq(a.get(key).unwrap_or(&NULL), b.get(key).unwrap_or(&NULL));
    if !same("playlist") || !same("audio") || !same("subtitle_shown") {
        return false;
    }
    !py_truthy(a.get("subtitle_shown").unwrap_or(&NULL)) || same("subtitle")
}

/// A number or a bool as Python compares it: an exact integer, or a float.
enum Numeric {
    Int(i128),
    Float(f64),
}

fn numeric(value: &Value) -> Option<Numeric> {
    match value {
        Value::Bool(flag) => Some(Numeric::Int(i128::from(*flag))),
        Value::Number(number) => Some(if let Some(int) = number.as_i64() {
            Numeric::Int(i128::from(int))
        } else if let Some(int) = number.as_u64() {
            Numeric::Int(i128::from(int))
        } else {
            Numeric::Float(number.as_f64().unwrap_or(f64::NAN))
        }),
        _ => None,
    }
}

/// An integer and a float are equal, as in Python, only when the float is exactly that integer.
fn int_equals_float(int: i128, float: f64) -> bool {
    // Every integer a Value holds is below 2^64 in size, so a float of 1e30 or more in size equals none.
    float.fract() == 0.0 && float.abs() < 1e30 && float as i128 == int
}

/// Python's `==` between two values `json.loads` gives: numbers and bools by numeric value (`1 == 1.0 == True`),
/// null only null, strings by text, arrays element by element, objects key by key in any order.
fn py_eq(a: &Value, b: &Value) -> bool {
    if let (Some(x), Some(y)) = (numeric(a), numeric(b)) {
        return match (x, y) {
            (Numeric::Int(x), Numeric::Int(y)) => x == y,
            (Numeric::Float(x), Numeric::Float(y)) => x == y,
            (Numeric::Int(x), Numeric::Float(y)) | (Numeric::Float(y), Numeric::Int(x)) => int_equals_float(x, y),
        };
    }
    match (a, b) {
        (Value::Null, Value::Null) => true,
        (Value::String(x), Value::String(y)) => x == y,
        (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(x, y)| py_eq(x, y)),
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len() && x.iter().all(|(key, x)| y.get(key).map_or(false, |y| py_eq(x, y)))
        }
        _ => false,
    }
}
