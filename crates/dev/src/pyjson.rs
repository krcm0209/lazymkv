//! Python's json.dumps byte for byte, the files the Python wrote with it, the level files of
//! proxy_levels.convert_levels, and the structural comparison the goldens use (port spec 3.3 "Writers", section 4).

use lazymkv_core::recording::{py_float_repr, py_is_space, universal_lines};
use serde_json::{Number, Value};
use std::path::{Path, PathBuf};

/// How json.dumps was called.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Style {
    /// None: one line with Python's default separators ", " and ": "; Some(n): n spaces per level with "," and ": ".
    pub indent: Option<usize>,
    /// true: every character outside ' ' to '~' as \uXXXX (lowercase hex, surrogate pairs above U+FFFF); false:
    /// kept. Both escape \" \\ \n \r \t \b \f by name and the other controls below 0x20 as \u00XX, as json.dumps does.
    pub ensure_ascii: bool,
    /// json.dumps(sort_keys=True): object keys by code point.
    pub sort_keys: bool,
}

impl Style {
    /// json.dumps(x): indent None, ASCII, insertion order.
    pub const PY_DEFAULT: Style = Style { indent: None, ensure_ascii: true, sort_keys: false };
    /// The snapshot and streams.json: indent 1, ASCII.
    pub const INDENT1_ASCII: Style = Style { indent: Some(1), ensure_ascii: true, sort_keys: false };
    /// The labels and fitted files: indent 2, non-ASCII kept.
    pub const INDENT2_UTF8: Style = Style { indent: Some(2), ensure_ascii: false, sort_keys: false };
}

/// json.dumps(value, ...) byte for byte: insertion order (serde_json `preserve_order`), ints and floats distinct
/// (floats spelt by py_float_repr), true/false/null, empty containers as [] and {}. A NaN never reaches it:
/// serde_json::Value holds none (serde_json turns a NaN f64 into null, which is nan_to_none).
pub fn dumps(value: &Value, style: Style) -> String {
    let mut out = String::new();
    write_value(&mut out, value, style, 0);
    out
}

fn write_value(out: &mut String, value: &Value, style: Style, level: usize) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(number) => out.push_str(&number_text(number)),
        Value::String(text) => write_string(out, text, style.ensure_ascii),
        Value::Array(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            for (at, item) in items.iter().enumerate() {
                if at > 0 {
                    out.push_str(item_separator(style));
                }
                newline(out, style, level + 1);
                write_value(out, item, style, level + 1);
            }
            newline(out, style, level);
            out.push(']');
        }
        Value::Object(map) => {
            if map.is_empty() {
                out.push_str("{}");
                return;
            }
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            if style.sort_keys {
                entries.sort_by(|a, b| a.0.cmp(b.0));
            }
            out.push('{');
            for (at, (key, item)) in entries.into_iter().enumerate() {
                if at > 0 {
                    out.push_str(item_separator(style));
                }
                newline(out, style, level + 1);
                write_string(out, key, style.ensure_ascii);
                out.push_str(": ");
                write_value(out, item, style, level + 1);
            }
            newline(out, style, level);
            out.push('}');
        }
    }
}

/// ", " on one line, "," when indented (json.dumps' default separators).
fn item_separator(style: Style) -> &'static str {
    if style.indent.is_some() {
        ","
    } else {
        ", "
    }
}

/// "\n" and indent * level spaces when indented; nothing on one line.
fn newline(out: &mut String, style: Style, level: usize) {
    if let Some(indent) = style.indent {
        out.push('\n');
        out.push_str(&" ".repeat(indent * level));
    }
}

/// An int as its digits, a float as Python's repr.
fn number_text(number: &Number) -> String {
    if let Some(int) = number.as_i64() {
        int.to_string()
    } else if let Some(int) = number.as_u64() {
        int.to_string()
    } else {
        py_float_repr(number.as_f64().unwrap_or(f64::NAN))
    }
}

fn write_string(out: &mut String, text: &str, ensure_ascii: bool) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            _ if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            _ if ensure_ascii && (c as u32) > 0x7e => {
                let mut units = [0u16; 2];
                for unit in c.encode_utf16(&mut units).iter() {
                    out.push_str(&format!("\\u{:04x}", unit));
                }
            }
            _ => out.push(c),
        }
    }
    out.push('"');
}

/// Writes `text` to `<path>.tmp`, then renames it over `path` (os.replace).
pub fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    let mut tmp = path.as_os_str().to_os_string();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

/// The snapshot: dumps(INDENT1_ASCII) + "\n", through `.tmp` and a rename. Py: audio_dataset.write_snapshot (whose
/// "unchanged but for its time" check stays with the caller, Task 8).
pub fn write_snapshot(path: &Path, value: &Value) -> std::io::Result<()> {
    write_atomic(path, &format!("{}\n", dumps(value, Style::INDENT1_ASCII)))
}

/// A labels or fitted file: dumps(INDENT2_UTF8) + "\n", UTF-8, through `.tmp` and a rename. Py: audio_labels.write_json.
pub fn write_labels(path: &Path, value: &Value) -> std::io::Result<()> {
    write_atomic(path, &format!("{}\n", dumps(value, Style::INDENT2_UTF8)))
}

/// streams.json: dumps(INDENT1_ASCII) + "\n", written in place. Py: proxy_levels.build.
pub fn write_streams_json(path: &Path, value: &Value) -> std::io::Result<()> {
    std::fs::write(path, format!("{}\n", dumps(value, Style::INDENT1_ASCII)))
}

/// The ametadata key of a level line. Py: proxy_levels.KEY.
pub const KEY: &str = "lavfi.astats.Overall.RMS_level=";

const FLOOR: &str = "-100.0";
const CONVERSION_SYNTAX: &str = "[<class 'decimal.ConversionSyntax'>]";
const INVALID_OPERATION: &str = "[<class 'decimal.InvalidOperation'>]";

/// A number as Python's decimal module reads it.
enum Decimal {
    NaN,
    Infinity { negative: bool },
    /// The value is (-1 if negative) * digits * 10**exponent; `digits` holds ASCII digits, leading zeros allowed.
    Finite { negative: bool, digits: String, exponent: i64 },
}

/// One level as audio-levels.ps1 and proxy_levels write it: Decimal("-100.0") when the text holds "inf" or "nan",
/// else max(Decimal("-100.0"), Decimal(text)) (the first wins a tie), quantized to 0.1 with ROUND_HALF_UP (half away
/// from zero), spelt as str(Decimal) ("-23.5", "-0.0", "-100.0"). Err is str() of the decimal exception the Python
/// would raise: "[<class 'decimal.ConversionSyntax'>]" for text Decimal() refuses, "[<class
/// 'decimal.InvalidOperation'>]" for +Infinity, a NaN (max() compares with it) or a result of more than 28 digits.
/// Non-ASCII digits are refused (Decimal() accepts them; ffmpeg never writes one). Py: convert_levels' loop body.
pub fn round_level(text: &str) -> Result<String, String> {
    if text.contains("inf") || text.contains("nan") {
        return Ok(FLOOR.to_string());
    }
    match parse_decimal(text).ok_or_else(|| CONVERSION_SYNTAX.to_string())? {
        Decimal::NaN => Err(INVALID_OPERATION.to_string()),
        Decimal::Infinity { negative: true } => Ok(FLOOR.to_string()),
        Decimal::Infinity { negative: false } => Err(INVALID_OPERATION.to_string()),
        Decimal::Finite { negative, digits, exponent } => {
            if negative && !below_hundred(&digits, exponent) {
                return Ok(FLOOR.to_string());
            }
            quantize_tenth(negative, &digits, exponent)
        }
    }
}

/// Decimal(text) of Python 3.9's C decimal: underscores dropped anywhere, surrounding whitespace stripped, then
/// [sign] (digits [. [digits]] | . digits) [e|E [sign] digits], or [sign] Inf / Infinity, or [sign] NaN / sNaN with
/// optional digits (letters in any case). None where Decimal() raises ConversionSyntax.
fn parse_decimal(text: &str) -> Option<Decimal> {
    let cleaned: String = text
        .chars()
        .filter(|c| *c != '_')
        .map(|c| if !c.is_ascii() && py_is_space(c) { ' ' } else { c })
        .collect();
    let body = cleaned.trim_matches(|c: char| matches!(c, ' ' | '\t' | '\n' | '\x0b' | '\x0c' | '\r'));
    let (negative, rest) = match body.as_bytes().first() {
        Some(b'-') => (true, &body[1..]),
        Some(b'+') => (false, &body[1..]),
        _ => (false, body),
    };
    let lower = rest.to_ascii_lowercase();
    if lower == "inf" || lower == "infinity" {
        return Some(Decimal::Infinity { negative });
    }
    if let Some(payload) = lower.strip_prefix("nan").or_else(|| lower.strip_prefix("snan")) {
        return if payload.bytes().all(|b| b.is_ascii_digit()) { Some(Decimal::NaN) } else { None };
    }
    let (mantissa, exponent_text) = match rest.find(|c: char| c == 'e' || c == 'E') {
        Some(at) => (&rest[..at], Some(&rest[at + 1..])),
        None => (rest, None),
    };
    let (whole, fraction) = match mantissa.split_once('.') {
        Some((whole, fraction)) => (whole, fraction),
        None => (mantissa, ""),
    };
    if whole.is_empty() && fraction.is_empty() {
        return None;
    }
    if !whole.bytes().all(|b| b.is_ascii_digit()) || !fraction.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let exponent = match exponent_text {
        None => 0,
        Some(text) => {
            let (sign, digits) = match text.as_bytes().first() {
                Some(b'-') => (-1, &text[1..]),
                Some(b'+') => (1, &text[1..]),
                _ => (1, text),
            };
            if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            let significant = digits.trim_start_matches('0');
            let magnitude: i64 = if significant.is_empty() {
                0
            } else if significant.len() > 15 {
                1_000_000_000_000_000
            } else {
                significant.parse().unwrap_or(0)
            };
            sign * magnitude
        }
    };
    Some(Decimal::Finite { negative, digits: format!("{}{}", whole, fraction), exponent: exponent - fraction.len() as i64 })
}

/// Whether digits * 10**exponent is below 100 (so that a negative value is above the floor of -100).
fn below_hundred(digits: &str, exponent: i64) -> bool {
    let significant = digits.trim_start_matches('0');
    significant.is_empty() || (significant.len() as i64 - 1 + exponent) < 2
}

/// Decimal.quantize(Decimal("0.1"), rounding=ROUND_HALF_UP) and str() of the result.
fn quantize_tenth(negative: bool, digits: &str, exponent: i64) -> Result<String, String> {
    let digits = digits.trim_start_matches('0');
    let kept: String = if exponent >= -1 {
        if digits.is_empty() {
            "0".to_string()
        } else {
            if digits.len() as i64 + exponent + 1 > 28 {
                return Err(INVALID_OPERATION.to_string());
            }
            format!("{}{}", digits, "0".repeat((exponent + 1) as usize))
        }
    } else {
        let dropped = (-1 - exponent) as usize;
        let (head, first_dropped) = if digits.len() > dropped {
            (digits[..digits.len() - dropped].to_string(), digits.as_bytes()[digits.len() - dropped])
        } else if digits.len() == dropped {
            ("0".to_string(), digits.as_bytes()[0])
        } else {
            ("0".to_string(), b'0')
        };
        if first_dropped >= b'5' {
            increment(&head)
        } else {
            head
        }
    };
    let kept = kept.trim_start_matches('0');
    let kept = if kept.is_empty() { "0" } else { kept };
    if kept.len() > 28 {
        return Err(INVALID_OPERATION.to_string());
    }
    let (whole, tenth) = kept.split_at(kept.len() - 1);
    Ok(format!("{}{}.{}", if negative { "-" } else { "" }, if whole.is_empty() { "0" } else { whole }, tenth))
}

/// A string of ASCII digits plus one.
fn increment(digits: &str) -> String {
    let mut bytes = digits.as_bytes().to_vec();
    let mut at = bytes.len();
    loop {
        if at == 0 {
            bytes.insert(0, b'1');
            break;
        }
        at -= 1;
        if bytes[at] == b'9' {
            bytes[at] = b'0';
        } else {
            bytes[at] += 1;
            break;
        }
    }
    String::from_utf8(bytes).expect("ASCII digits")
}

/// A level file's text from ffmpeg's raw ametadata output: `header`, then one round_level per line starting with KEY
/// (lines from universal_lines, which already drops the "\r\n" convert_levels strips), every line ended by "\r\n";
/// and the number of frames. Py: proxy_levels.convert_levels.
pub fn level_file_text(raw: &str, header: &str) -> Result<(String, usize), String> {
    let mut text = header.replace('\n', "\r\n");
    text.push_str("\r\n");
    let mut frames = 0;
    for line in universal_lines(raw) {
        if let Some(value) = line.strip_prefix(KEY) {
            text.push_str(&round_level(value)?);
            text.push_str("\r\n");
            frames += 1;
        }
    }
    Ok((text, frames))
}

/// Writes level_file_text(raw, header) to `path` (CRLF line ends, UTF-8) and returns the frame count; Err with the
/// I/O or decimal text (nothing is written when a level is refused). Py: proxy_levels.convert_levels.
pub fn write_level_file(path: &Path, raw: &str, header: &str) -> Result<usize, String> {
    let (text, frames) = level_file_text(raw, header)?;
    std::fs::write(path, text).map_err(|error| error.to_string())?;
    Ok(frames)
}

/// manifest.txt: the lines joined with "\n" plus a final "\n", UTF-8. Py: proxy_levels.build.
pub fn write_manifest(path: &Path, lines: &[String]) -> std::io::Result<()> {
    std::fs::write(path, format!("{}\n", lines.join("\n")))
}

/// How two parsed JSON documents may differ and still be equal: floats within `abs` (1e-9), keys named in
/// `loose_keys` ("smi", "rev_smi") within `loose` (1e-6), null equal to null (NaN as null), ints equal to equal floats
/// never (1 and 1.0 differ, as Python's text does); paths in `ignore` are JSON pointers with `*` for any key or
/// index ("/created", "/tracks/*/key").
pub struct Tolerance {
    pub abs: f64,
    pub loose: f64,
    pub loose_keys: &'static [&'static str],
    pub ignore: Vec<String>,
}

impl Tolerance {
    /// abs 1e-9, loose 1e-6 for "smi" and "rev_smi", nothing ignored (spec section 4).
    pub fn spec() -> Tolerance {
        Tolerance { abs: 1e-9, loose: 1e-6, loose_keys: &["smi", "rev_smi"], ignore: Vec::new() }
    }
}

/// Every difference between `expected` and `actual`, one line each ("<pointer>: <expected> != <actual>", values as
/// dumps(PY_DEFAULT), "(absent)" for a missing key or element, "(root)" for the whole document); empty when they are
/// equal under `tol`. Object keys are compared as sets (expected's keys first, in its order, then actual's own),
/// arrays in order. A float inside an array takes the tolerance of the key that holds the array.
pub fn structural_diff(expected: &Value, actual: &Value, tol: &Tolerance) -> Vec<String> {
    let mut out = Vec::new();
    diff_at(&mut Vec::new(), None, Some(expected), Some(actual), tol, &mut out);
    out
}

fn diff_at(
    pointer: &mut Vec<String>,
    key: Option<&str>,
    expected: Option<&Value>,
    actual: Option<&Value>,
    tol: &Tolerance,
    out: &mut Vec<String>,
) {
    if tol.ignore.iter().any(|pattern| pointer_matches(pattern, pointer.as_slice())) {
        return;
    }
    match (expected, actual) {
        (Some(Value::Object(left)), Some(Value::Object(right))) => {
            let mut names: Vec<&String> = left.keys().collect();
            names.extend(right.keys().filter(|name| !left.contains_key(name.as_str())));
            for name in names {
                pointer.push(name.clone());
                diff_at(pointer, Some(name.as_str()), left.get(name.as_str()), right.get(name.as_str()), tol, out);
                pointer.pop();
            }
        }
        (Some(Value::Array(left)), Some(Value::Array(right))) => {
            for at in 0..left.len().max(right.len()) {
                pointer.push(at.to_string());
                diff_at(pointer, key, left.get(at), right.get(at), tol, out);
                pointer.pop();
            }
        }
        (Some(left), Some(right)) if same(left, right, key, tol) => {}
        _ => out.push(format!("{}: {} != {}", pointer_text(pointer), shown(expected), shown(actual))),
    }
}

/// Equal under `tol`: numbers of the same kind (int or float), floats within the key's tolerance; anything else
/// exactly.
fn same(expected: &Value, actual: &Value, key: Option<&str>, tol: &Tolerance) -> bool {
    match (expected, actual) {
        (Value::Number(left), Value::Number(right)) => {
            if left.is_f64() != right.is_f64() {
                return false;
            }
            if !left.is_f64() {
                return left == right;
            }
            let (left, right) = (left.as_f64().unwrap_or(f64::NAN), right.as_f64().unwrap_or(f64::NAN));
            let loose = key.map_or(false, |name| tol.loose_keys.iter().any(|loose_key| *loose_key == name));
            let limit = if loose { tol.loose } else { tol.abs };
            left == right || (left - right).abs() <= limit
        }
        _ => expected == actual,
    }
}

fn shown(value: Option<&Value>) -> String {
    match value {
        Some(value) => dumps(value, Style::PY_DEFAULT),
        None => "(absent)".to_string(),
    }
}

/// A JSON pointer ("/a~1b/0"), or "(root)" for the whole document.
fn pointer_text(pointer: &[String]) -> String {
    if pointer.is_empty() {
        return "(root)".to_string();
    }
    pointer.iter().map(|part| format!("/{}", part.replace('~', "~0").replace('/', "~1"))).collect()
}

/// Whether `pattern` ("/tracks/*/key") names `pointer`: as many parts, each equal or "*".
fn pointer_matches(pattern: &str, pointer: &[String]) -> bool {
    let parts: Vec<String> = pattern.split('/').skip(1).map(|part| part.replace("~1", "/").replace("~0", "~")).collect();
    parts.len() == pointer.len() && parts.iter().zip(pointer).all(|(part, name)| part == "*" || part == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lazymkv-dev-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn dumps_matches_json_dumps_with_its_default_arguments() {
        let value = json!({"a": 1, "b": [1.0, 2.5, null, true, false], "c": {}, "d": [], "e": "x\u{e9}\n"});
        assert_eq!(
            dumps(&value, Style::PY_DEFAULT),
            r#"{"a": 1, "b": [1.0, 2.5, null, true, false], "c": {}, "d": [], "e": "x\u00e9\n"}"#
        );
    }

    #[test]
    fn dumps_indents_as_json_dumps_with_indent_1_and_2() {
        let value = json!({"a": 1, "b": [1.0, 2.5, null, true, false], "c": {}, "d": [], "e": "x\u{e9}\n"});
        assert_eq!(
            dumps(&value, Style::INDENT1_ASCII),
            "{\n \"a\": 1,\n \"b\": [\n  1.0,\n  2.5,\n  null,\n  true,\n  false\n ],\n \"c\": {},\n \"d\": [],\n \"e\": \"x\\u00e9\\n\"\n}"
        );
        let nested = json!({"z": 1, "a": [{"k": []}, 2]});
        assert_eq!(
            dumps(&nested, Style::INDENT2_UTF8),
            "{\n  \"z\": 1,\n  \"a\": [\n    {\n      \"k\": []\n    },\n    2\n  ]\n}"
        );
        let note = json!({"note": "caf\u{e9} \u{fc}ber", "n": -0.0});
        assert_eq!(dumps(&note, Style::INDENT2_UTF8), "{\n  \"note\": \"caf\u{e9} \u{fc}ber\",\n  \"n\": -0.0\n}");
    }

    #[test]
    fn dumps_escapes_as_json_dumps_with_and_without_ensure_ascii() {
        let text = json!("\u{0}\u{1f}\u{7f}\u{8}\u{c}\t\"\\/\u{e9}\u{2028}\u{1f600}");
        assert_eq!(
            dumps(&text, Style::PY_DEFAULT),
            r#""\u0000\u001f\u007f\b\f\t\"\\/\u00e9\u2028\ud83d\ude00""#
        );
        let utf8 = Style { indent: None, ensure_ascii: false, sort_keys: false };
        assert_eq!(dumps(&text, utf8), "\"\\u0000\\u001f\u{7f}\\b\\f\\t\\\"\\\\/\u{e9}\u{2028}\u{1f600}\"");
    }

    #[test]
    fn dumps_sorts_keys_and_spells_numbers_as_python() {
        let sorted = Style { indent: None, ensure_ascii: true, sort_keys: true };
        assert_eq!(dumps(&json!({"b": 1, "a": {"d": 2, "c": 3}}), sorted), r#"{"a": {"c": 3, "d": 2}, "b": 1}"#);
        let numbers: Value = serde_json::from_str(
            "[9223372036854775808, -9223372036854775808, 18446744073709551615, 1e16, 1E-5, 0.1, 100.0, 5e-324, -0.0]",
        )
        .unwrap();
        assert_eq!(
            dumps(&numbers, Style::PY_DEFAULT),
            "[9223372036854775808, -9223372036854775808, 18446744073709551615, 1e+16, 1e-05, 0.1, 100.0, 5e-324, -0.0]"
        );
        assert_eq!(dumps(&json!(f64::NAN), Style::PY_DEFAULT), "null");
    }

    #[test]
    fn writers_add_the_final_newline_and_replace_through_a_tmp_file() {
        let dir = scratch("writers");
        let value = json!({"format": "x", "note": "caf\u{e9}"});
        write_snapshot(&dir.join("snapshot.json"), &value).unwrap();
        assert_eq!(
            fs::read_to_string(dir.join("snapshot.json")).unwrap(),
            "{\n \"format\": \"x\",\n \"note\": \"caf\\u00e9\"\n}\n"
        );
        write_labels(&dir.join("audio-labels.json"), &value).unwrap();
        assert_eq!(
            fs::read_to_string(dir.join("audio-labels.json")).unwrap(),
            "{\n  \"format\": \"x\",\n  \"note\": \"caf\u{e9}\"\n}\n"
        );
        write_streams_json(&dir.join("streams.json"), &json!({"streams": []})).unwrap();
        assert_eq!(fs::read_to_string(dir.join("streams.json")).unwrap(), "{\n \"streams\": []\n}\n");
        let lines = vec!["# lazymkv audio-levels v2 (experiment 13)".to_string(), "main=1".to_string()];
        write_manifest(&dir.join("manifest.txt"), &lines).unwrap();
        assert_eq!(fs::read(dir.join("manifest.txt")).unwrap(), b"# lazymkv audio-levels v2 (experiment 13)\nmain=1\n");
        write_atomic(&dir.join("snapshot.json"), "replaced\n").unwrap();
        assert_eq!(fs::read_to_string(dir.join("snapshot.json")).unwrap(), "replaced\n");
        let leftovers: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "{:?}", leftovers);
    }

    #[test]
    fn round_level_is_the_decimal_rounding_of_convert_levels() {
        let cases = [
            ("-23.456789", "-23.5"),
            ("-23.450000", "-23.5"),
            ("-23.449999", "-23.4"),
            ("-0.04", "-0.0"),
            ("-0.05", "-0.1"),
            ("0.05", "0.1"),
            ("0.04", "0.0"),
            ("-100.000000", "-100.0"),
            ("-100", "-100.0"),
            ("-150.5", "-100.0"),
            ("-inf", "-100.0"),
            ("inf", "-100.0"),
            ("nan", "-100.0"),
            ("-nan", "-100.0"),
            ("-99.95", "-100.0"),
            ("-99.949", "-99.9"),
            ("1_0", "10.0"),
            (" -5.25 ", "-5.3"),
            ("1_.5", "1.5"),
            ("_1", "1.0"),
            ("1__0", "10.0"),
            ("+1.25", "1.3"),
            (".05", "0.1"),
            ("5.", "5.0"),
            ("1e1", "10.0"),
            ("-1E-1", "-0.1"),
            ("-Inf", "-100.0"),
            ("-Infinity", "-100.0"),
            ("-0", "-0.0"),
            ("0", "0.0"),
            ("-9999999999999999999999999999.95", "-100.0"),
            ("1.25e-3", "0.0"),
            ("-12.3456e1", "-100.0"),
            ("-23.45\t", "-23.5"),
        ];
        for (text, level) in cases {
            assert_eq!(round_level(text), Ok(level.to_string()), "{:?}", text);
        }
        for text in ["Infinity", "INF", "NaN", "sNaN", "12345678901234567890123456789.5"] {
            assert_eq!(round_level(text), Err("[<class 'decimal.InvalidOperation'>]".to_string()), "{:?}", text);
        }
        for text in ["", "abc", "1 0", "0x10", "1e", "e5", ".", "Nano"] {
            assert_eq!(round_level(text), Err("[<class 'decimal.ConversionSyntax'>]".to_string()), "{:?}", text);
        }
    }

    #[test]
    fn level_file_text_keeps_the_key_lines_rounded_with_crlf() {
        let raw = "frame:0    pts:0       pts_time:0\nlavfi.astats.Overall.RMS_level=-23.456789\r\n\
                   frame:1    pts:160     pts_time:0.02\rlavfi.astats.Overall.RMS_level=-inf\n\
                   lavfi.astats.Overall.RMS_level=-0.04";
        let (text, frames) = level_file_text(raw, "# lazymkv audio levels v2: frame_ms=20 series=main-mix").unwrap();
        assert_eq!(frames, 3);
        assert_eq!(text, "# lazymkv audio levels v2: frame_ms=20 series=main-mix\r\n-23.5\r\n-100.0\r\n-0.0\r\n");
        assert_eq!(
            level_file_text("lavfi.astats.Overall.RMS_level=abc\n", "h"),
            Err("[<class 'decimal.ConversionSyntax'>]".to_string())
        );
        let dir = scratch("level-file");
        assert_eq!(write_level_file(&dir.join("main-mix.txt"), raw, "h"), Ok(3));
        assert_eq!(fs::read(dir.join("main-mix.txt")).unwrap(), b"h\r\n-23.5\r\n-100.0\r\n-0.0\r\n");
    }

    #[test]
    fn structural_diff_compares_floats_within_the_spec_tolerance() {
        let tol = Tolerance::spec();
        let expected = json!({"r": 0.5, "smi": 0.25, "n": 3, "v": null, "list": [1, "a"]});
        let close = json!({"list": [1, "a"], "v": null, "n": 3, "smi": 0.2500009, "r": 0.5000000005});
        assert_eq!(structural_diff(&expected, &close, &tol), Vec::<String>::new());
        let far = json!({"r": 0.500001, "smi": 0.25001, "n": 3.0, "v": 0, "list": [1]});
        assert_eq!(
            structural_diff(&expected, &far, &tol),
            vec![
                "/r: 0.5 != 0.500001".to_string(),
                "/smi: 0.25 != 0.25001".to_string(),
                "/n: 3 != 3.0".to_string(),
                "/v: null != 0".to_string(),
                "/list/1: \"a\" != (absent)".to_string(),
            ]
        );
    }

    #[test]
    fn structural_diff_ignores_wildcard_pointers_and_names_missing_keys() {
        let mut tol = Tolerance::spec();
        tol.ignore = vec!["/created".to_string(), "/tracks/*/key".to_string()];
        let expected = json!({
            "created": "2026-10-01T10:00:00",
            "tracks": [{"key": "a", "verdict": "keep"}, {"key": "b", "rev_smi": 0.1}],
            "only_here": 1
        });
        let actual = json!({
            "created": "2026-10-13T12:00:00",
            "tracks": [{"key": "x", "verdict": "keep"}, {"rev_smi": 0.1000005}],
            "only_there": {"k/~": 2}
        });
        assert_eq!(
            structural_diff(&expected, &actual, &tol),
            vec!["/only_here: 1 != (absent)".to_string(), "/only_there: (absent) != {\"k/~\": 2}".to_string()]
        );
        assert_eq!(structural_diff(&json!(1), &json!("1"), &tol), vec!["(root): 1 != \"1\"".to_string()]);
        assert_eq!(
            structural_diff(&json!({"a/b": {"c~d": 1}}), &json!({"a/b": {"c~d": 2}}), &tol),
            vec!["/a~1b/c~0d: 1 != 2".to_string()]
        );
    }
}
