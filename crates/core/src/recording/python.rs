//! Python's spellings of strings and numbers (port spec section 4), so that every printed or written value reads as
//! the Python's did: str.isspace, str.split(), str.strip(), str.isdigit(), int() and float() of a str or of a JSON
//! value, repr() of a float, a str and a JSON value, '%.Nf' % x, format(x, "g"), str() and bool().

use super::printable::is_printable;
use serde_json::Value;

/// Python's str.isspace for one character: Unicode White_Space plus the bidi classes B, S and WS, so U+001C to
/// U+001F count (char::is_whitespace does not count them): 29 characters in Python 3.9.
pub fn py_is_space(c: char) -> bool {
    matches!(
        c,
        '\t'..='\r'
            | '\x1c'..=' '
            | '\u{85}'
            | '\u{a0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200a}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202f}'
            | '\u{205f}'
            | '\u{3000}'
    )
}

/// str.split() with no argument: runs of py_is_space separate, no empty pieces.
pub fn py_split(text: &str) -> Vec<&str> {
    text.split(py_is_space).filter(|piece| !piece.is_empty()).collect()
}

/// str.strip() with no argument.
pub fn py_strip(text: &str) -> &str {
    text.trim_matches(py_is_space)
}

/// str.isdigit(), ASCII digits only. Python's isdigit also accepts other Unicode digits; no recorded or pinned robot
/// log holds one (checked while planning: the 41 robot logs under corpus/ hold no non-ASCII digit), so no verdict
/// depends on the difference (02-interfaces.md, open point 9).
pub fn py_isdigit(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit())
}

/// A leading '-' or '+' taken off: (negative, the rest).
fn split_sign(text: &str) -> (bool, &str) {
    match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..]),
        Some(b'+') => (false, &text[1..]),
        _ => (false, text),
    }
}

/// ASCII digits with single underscores between digits (PEP 515), at least one digit.
fn valid_digits(text: &str) -> bool {
    let bytes = text.as_bytes();
    match (bytes.first(), bytes.last()) {
        (Some(first), Some(last)) => {
            first.is_ascii_digit()
                && last.is_ascii_digit()
                && bytes.iter().all(|b| b.is_ascii_digit() || *b == b'_')
                && !text.contains("__")
        }
        _ => false,
    }
}

/// int() of a str: surrounding whitespace, one sign, ASCII digits with single underscores between them; None where
/// Python raises ValueError. A value beyond i64 is None too (Python would give a big int; no recorded value is near).
pub fn py_int(text: &str) -> Option<i64> {
    let (negative, digits) = split_sign(py_strip(text));
    if !valid_digits(digits) {
        return None;
    }
    let cleaned: String = digits.chars().filter(|c| *c != '_').collect();
    let signed = if negative { format!("-{}", cleaned) } else { cleaned };
    signed.parse().ok()
}

/// The decimal form float() accepts: digits [. [digits]] or . digits, then optionally e or E, a sign and digits;
/// every digit run as valid_digits.
fn valid_float(text: &str) -> bool {
    let (mantissa, exponent) = match text.find(|c: char| c == 'e' || c == 'E') {
        Some(at) => (&text[..at], Some(&text[at + 1..])),
        None => (text, None),
    };
    let mantissa_ok = match mantissa.split_once('.') {
        Some((whole, fraction)) => {
            (!whole.is_empty() || !fraction.is_empty())
                && (whole.is_empty() || valid_digits(whole))
                && (fraction.is_empty() || valid_digits(fraction))
        }
        None => valid_digits(mantissa),
    };
    let exponent_ok = match exponent {
        None => true,
        Some(digits) => valid_digits(split_sign(digits).1),
    };
    mantissa_ok && exponent_ok
}

/// float() of a str: whitespace, sign, digits with underscores, exponent, "inf"/"infinity"/"nan" in any case; None
/// where Python raises ValueError. Non-ASCII digits are refused (Python accepts them; no recorded value has one).
pub fn py_float(text: &str) -> Option<f64> {
    let (negative, rest) = split_sign(py_strip(text));
    let magnitude = match rest.to_ascii_lowercase().as_str() {
        "inf" | "infinity" => f64::INFINITY,
        "nan" => f64::NAN,
        _ => {
            if !valid_float(rest) {
                return None;
            }
            let cleaned: String = rest.chars().filter(|c| *c != '_').collect();
            cleaned.parse::<f64>().ok()?
        }
    };
    Some(if negative { -magnitude } else { magnitude })
}

/// type(value).__name__ of the Python value json.loads gives.
fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "NoneType",
        Value::Bool(_) => "bool",
        Value::Number(number) if number.is_f64() => "float",
        Value::Number(_) => "int",
        Value::String(_) => "str",
        Value::Array(_) => "list",
        Value::Object(_) => "dict",
    }
}

/// int() of a JSON value: an int as is, a float truncated, a str by py_int, a bool as 0 or 1; Err is Python's
/// TypeError or ValueError text (Python 3.9's wording).
pub fn py_int_value(value: &Value) -> Result<i64, String> {
    match value {
        Value::Bool(flag) => Ok(i64::from(*flag)),
        Value::Number(number) => {
            if let Some(int) = number.as_i64() {
                Ok(int)
            } else if number.is_u64() {
                Err(format!("int too large for 64 bits: {}", number))
            } else {
                let truncated = number.as_f64().unwrap_or(f64::NAN).trunc();
                if (-9.223_372_036_854_776e18..9.223_372_036_854_776e18).contains(&truncated) {
                    Ok(truncated as i64)
                } else {
                    Err(format!("float too large for 64 bits: {}", number))
                }
            }
        }
        Value::String(text) => {
            py_int(text).ok_or_else(|| format!("invalid literal for int() with base 10: {}", py_repr_str(text)))
        }
        other => Err(format!(
            "int() argument must be a string, a bytes-like object or a number, not '{}'",
            type_name(other)
        )),
    }
}

/// float() of a JSON value: an int or float as f64, a str by py_float, a bool as 0.0 or 1.0; Err as py_int_value.
pub fn py_float_value(value: &Value) -> Result<f64, String> {
    match value {
        Value::Bool(flag) => Ok(if *flag { 1.0 } else { 0.0 }),
        Value::Number(number) => Ok(number.as_f64().unwrap_or(f64::NAN)),
        Value::String(text) => {
            py_float(text).ok_or_else(|| format!("could not convert string to float: {}", py_repr_str(text)))
        }
        other => Err(format!("float() argument must be a string or a number, not '{}'", type_name(other))),
    }
}

/// Python's spelling of a NaN or an infinity; None for a finite number.
fn non_finite(x: f64) -> Option<String> {
    if x.is_nan() {
        Some("nan".to_string())
    } else if x == f64::INFINITY {
        Some("inf".to_string())
    } else if x == f64::NEG_INFINITY {
        Some("-inf".to_string())
    } else {
        None
    }
}

/// repr(float): the shortest digits that read back as x, in exponent form below 1e-4 and from 1e16 ("1e-05",
/// "1e+16", at least two exponent digits), else positional with ".0" on whole numbers; "nan", "inf", "-inf".
pub fn py_float_repr(x: f64) -> String {
    if let Some(text) = non_finite(x) {
        return text;
    }
    if x == 0.0 {
        let text = if x.is_sign_negative() { "-0.0" } else { "0.0" };
        return text.to_string();
    }
    // {:e} without a precision writes the shortest digits that read back as x, the digits Python's repr chooses.
    let scientific = format!("{:e}", x.abs());
    let (mantissa, exponent) = scientific.split_once('e').expect("{:e} writes an exponent");
    let exponent: i32 = exponent.parse().expect("{:e} writes a whole exponent");
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let sign = if x < 0.0 { "-" } else { "" };
    let point = exponent + 1;
    let body = if !(-4..16).contains(&exponent) {
        let head = if digits.len() > 1 { format!("{}.{}", &digits[..1], &digits[1..]) } else { digits.clone() };
        format!("{}e{}{:02}", head, if exponent < 0 { '-' } else { '+' }, exponent.abs())
    } else if point <= 0 {
        format!("0.{}{}", "0".repeat((-point) as usize), digits)
    } else if (point as usize) < digits.len() {
        format!("{}.{}", &digits[..point as usize], &digits[point as usize..])
    } else {
        format!("{}{}.0", digits, "0".repeat(point as usize - digits.len()))
    };
    format!("{}{}", sign, body)
}

/// '%.Nf' % x: rounded half to even on the exact binary value (Rust's {:.N} rounds the same way), "-0.0" kept,
/// "nan", "inf", "-inf".
pub fn py_fixed(x: f64, digits: usize) -> String {
    match non_finite(x) {
        Some(text) => text,
        None => format!("{:.*}", digits, x),
    }
}

/// The text without trailing zeros after a decimal point, and without the point when nothing follows it.
fn without_trailing_zeros(text: &str) -> String {
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        text.to_string()
    }
}

/// format(x, "g"): six significant digits; positional when the exponent of the rounded value is from -4 to 5, else
/// exponent form with at least two exponent digits; trailing zeros dropped; "-0", "nan", "inf", "-inf".
pub fn py_g(x: f64) -> String {
    if let Some(text) = non_finite(x) {
        return text;
    }
    if x == 0.0 {
        let text = if x.is_sign_negative() { "-0" } else { "0" };
        return text.to_string();
    }
    let scientific = format!("{:.5e}", x);
    let (mantissa, exponent) = scientific.split_once('e').expect("{:e} writes an exponent");
    let exponent: i32 = exponent.parse().expect("{:e} writes a whole exponent");
    if (-4..6).contains(&exponent) {
        without_trailing_zeros(&format!("{:.*}", (5 - exponent) as usize, x))
    } else {
        format!("{}e{}{:02}", without_trailing_zeros(mantissa), if exponent < 0 { '-' } else { '+' }, exponent.abs())
    }
}

/// repr() of a str: single quotes unless the text holds ' and no ", backslash escapes for \\ \t \n \r and the quote,
/// \xNN for the other controls and DEL, the characters str.isprintable accepts as they are, and \xNN / \uNNNN /
/// \UNNNNNNNN (lowercase hex) for the rest. Py: unicode_repr.
pub fn py_repr_str(text: &str) -> String {
    let quote = if text.contains('\'') && !text.contains('"') { '"' } else { '\'' };
    let mut out = String::with_capacity(text.len() + 2);
    out.push(quote);
    for c in text.chars() {
        let code = c as u32;
        match c {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ if c == quote => {
                out.push('\\');
                out.push(c);
            }
            _ if code < 0x20 || code == 0x7f => out.push_str(&format!("\\x{:02x}", code)),
            _ if code < 0x7f => out.push(c),
            _ if is_printable(c) => out.push(c),
            _ if code <= 0xff => out.push_str(&format!("\\x{:02x}", code)),
            _ if code <= 0xffff => out.push_str(&format!("\\u{:04x}", code)),
            _ => out.push_str(&format!("\\U{:08x}", code)),
        }
    }
    out.push(quote);
    out
}

/// repr() of the Python value json.loads gives: None, True, False, ints, py_float_repr, py_repr_str, "[a, b]",
/// "{'k': v}" in the object's order.
pub fn py_repr(value: &Value) -> String {
    match value {
        Value::Null => "None".to_string(),
        Value::Bool(true) => "True".to_string(),
        Value::Bool(false) => "False".to_string(),
        Value::Number(number) => {
            if let Some(int) = number.as_i64() {
                int.to_string()
            } else if let Some(int) = number.as_u64() {
                int.to_string()
            } else {
                py_float_repr(number.as_f64().unwrap_or(f64::NAN))
            }
        }
        Value::String(text) => py_repr_str(text),
        Value::Array(items) => format!("[{}]", items.iter().map(py_repr).collect::<Vec<_>>().join(", ")),
        Value::Object(map) => format!(
            "{{{}}}",
            map.iter().map(|(key, item)| format!("{}: {}", py_repr_str(key), py_repr(item))).collect::<Vec<_>>().join(", ")
        ),
    }
}

/// str() of that value: a str as it is, anything else as py_repr.
pub fn py_str(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => py_repr(other),
    }
}

/// bool() of that value: false for None, False, 0, 0.0, "", [] and {}.
pub fn py_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_f64().map_or(false, |x| x != 0.0),
        Value::String(text) => !text.is_empty(),
        Value::Array(items) => !items.is_empty(),
        Value::Object(map) => !map.is_empty(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn is_space_is_python_isspace() {
        let count = (0..=0x10ffffu32).filter_map(char::from_u32).filter(|c| py_is_space(*c)).count();
        assert_eq!(count, 29);
        for c in [
            '\t', '\n', '\x0b', '\x0c', '\r', '\x1c', '\x1d', '\x1e', '\x1f', ' ', '\u{85}', '\u{a0}', '\u{1680}',
            '\u{2000}', '\u{200a}', '\u{2028}', '\u{2029}', '\u{202f}', '\u{205f}', '\u{3000}',
        ] {
            assert!(py_is_space(c), "{:?}", c);
        }
        for c in ['\x08', '\x1b', '!', 'a', '\u{200b}', '\u{feff}', '\u{180e}'] {
            assert!(!py_is_space(c), "{:?}", c);
        }
    }

    #[test]
    fn split_strip_and_isdigit_follow_python() {
        assert_eq!(py_split(" a\x1cb  c\u{3000}"), vec!["a", "b", "c"]);
        assert_eq!(py_split(""), Vec::<&str>::new());
        assert_eq!(py_split("   "), Vec::<&str>::new());
        assert_eq!(py_strip("\x1f x y \u{85}"), "x y");
        assert!(py_isdigit("0123"));
        assert!(!py_isdigit(""));
        assert!(!py_isdigit("1a"));
        assert!(!py_isdigit("-1"));
        assert!(!py_isdigit("\u{663}"));
    }

    #[test]
    fn int_of_a_string_follows_python() {
        assert_eq!(py_int(" 1_0 "), Some(10));
        assert_eq!(py_int("007"), Some(7));
        assert_eq!(py_int("-42"), Some(-42));
        assert_eq!(py_int("+5"), Some(5));
        assert_eq!(py_int("-9223372036854775808"), Some(i64::MIN));
        for bad in ["", "x", "1__0", "_1", "1_", "+-1", "0x10", "1.0", "1 0", "99999999999999999999"] {
            assert_eq!(py_int(bad), None, "{:?}", bad);
        }
    }

    #[test]
    fn float_of_a_string_follows_python() {
        assert_eq!(py_float("1_0.5"), Some(10.5));
        assert_eq!(py_float(".5"), Some(0.5));
        assert_eq!(py_float("1."), Some(1.0));
        assert_eq!(py_float(" -Infinity "), Some(f64::NEG_INFINITY));
        assert_eq!(py_float("+inf"), Some(f64::INFINITY));
        assert!(py_float("nAn").unwrap().is_nan());
        assert!(py_float("-nan").unwrap().is_nan());
        assert_eq!(py_float("1e5"), Some(100000.0));
        assert_eq!(py_float("1e5_0"), Some(1e50));
        assert_eq!(py_float("-2.5E-3"), Some(-0.0025));
        for bad in ["", ".", "x", "1_.5", "1e", "e5", "1e_5", "1_e5", "0x10", "in f", "1.2.3"] {
            assert_eq!(py_float(bad), None, "{:?}", bad);
        }
    }

    #[test]
    fn int_and_float_of_json_values_follow_python() {
        assert_eq!(py_int_value(&json!(7)), Ok(7));
        assert_eq!(py_int_value(&json!(-2.9)), Ok(-2));
        assert_eq!(py_int_value(&json!("12")), Ok(12));
        assert_eq!(py_int_value(&json!(true)), Ok(1));
        assert_eq!(py_int_value(&json!("x")), Err("invalid literal for int() with base 10: 'x'".to_string()));
        assert_eq!(
            py_int_value(&json!(null)),
            Err("int() argument must be a string, a bytes-like object or a number, not 'NoneType'".to_string())
        );
        assert_eq!(
            py_int_value(&json!([])),
            Err("int() argument must be a string, a bytes-like object or a number, not 'list'".to_string())
        );
        assert_eq!(py_float_value(&json!(3)), Ok(3.0));
        assert_eq!(py_float_value(&json!("0.25")), Ok(0.25));
        assert_eq!(py_float_value(&json!(false)), Ok(0.0));
        assert_eq!(py_float_value(&json!("x")), Err("could not convert string to float: 'x'".to_string()));
        assert_eq!(
            py_float_value(&json!(null)),
            Err("float() argument must be a string or a number, not 'NoneType'".to_string())
        );
        assert_eq!(
            py_float_value(&json!({})),
            Err("float() argument must be a string or a number, not 'dict'".to_string())
        );
    }

    #[test]
    fn float_repr_is_python_repr() {
        let cases = [
            (0.0, "0.0"),
            (-0.0, "-0.0"),
            (1.0, "1.0"),
            (0.1, "0.1"),
            (1e16, "1e+16"),
            (1e-05, "1e-05"),
            (0.0001, "0.0001"),
            (9999999999999998.0, "9999999999999998.0"),
            (1.5e-07, "1.5e-07"),
            (1e22, "1e+22"),
            (5e-324, "5e-324"),
            (f64::MAX, "1.7976931348623157e+308"),
            (123456789.0, "123456789.0"),
            (100.0, "100.0"),
            (2.5, "2.5"),
            (1.0 / 3.0, "0.3333333333333333"),
            (-1e-05, "-1e-05"),
            (1e15, "1000000000000000.0"),
            (12345678901234568.0, "1.2345678901234568e+16"),
            (0.1 + 0.2, "0.30000000000000004"),
            (1e100, "1e+100"),
            (-123.456, "-123.456"),
            (0.000123, "0.000123"),
        ];
        for (x, text) in cases {
            assert_eq!(py_float_repr(x), text, "{:e}", x);
        }
        assert_eq!(py_float_repr(f64::NAN), "nan");
        assert_eq!(py_float_repr(f64::INFINITY), "inf");
        assert_eq!(py_float_repr(f64::NEG_INFINITY), "-inf");
    }

    #[test]
    fn fixed_point_is_python_percent_f() {
        let cases = [
            (0.5, 0, "0"),
            (1.5, 0, "2"),
            (2.5, 0, "2"),
            (0.125, 2, "0.12"),
            (0.375, 2, "0.38"),
            (-0.04, 1, "-0.0"),
            (-0.0, 1, "-0.0"),
            (2.675, 2, "2.67"),
            (1.005, 2, "1.00"),
            (-23.45, 1, "-23.4"),
            (1e22, 1, "10000000000000000000000.0"),
            (0.05, 1, "0.1"),
        ];
        for (x, digits, text) in cases {
            assert_eq!(py_fixed(x, digits), text, "{} to {} places", x, digits);
        }
        assert_eq!(py_fixed(f64::NAN, 1), "nan");
        assert_eq!(py_fixed(f64::NEG_INFINITY, 2), "-inf");
    }

    #[test]
    fn general_format_is_python_format_g() {
        let cases = [
            (-18.0, "-18"),
            (0.5, "0.5"),
            (1e-05, "1e-05"),
            (123456789.0, "1.23457e+08"),
            (0.0001, "0.0001"),
            (100000.0, "100000"),
            (1000000.0, "1e+06"),
            (2.5, "2.5"),
            (-0.0, "-0"),
            (0.0, "0"),
            (1234567.0, "1.23457e+06"),
            (0.00012345678, "0.000123457"),
            (999999.5, "1e+06"),
            (9.999995, "10"),
            (-2.5e-07, "-2.5e-07"),
        ];
        for (x, text) in cases {
            assert_eq!(py_g(x), text, "{:e}", x);
        }
        assert_eq!(py_g(f64::NAN), "nan");
        assert_eq!(py_g(f64::INFINITY), "inf");
    }

    #[test]
    fn string_repr_is_python_repr() {
        let cases = [
            ("abc", "'abc'"),
            ("it's", "\"it's\""),
            ("say \"hi\"", "'say \"hi\"'"),
            ("both ' and \"", "'both \\' and \"'"),
            ("tab\there", "'tab\\there'"),
            ("nl\n", "'nl\\n'"),
            ("cr\r", "'cr\\r'"),
            ("back\\slash", "'back\\\\slash'"),
            ("\x00\x1f\x7f", "'\\x00\\x1f\\x7f'"),
            ("caf\u{e9}", "'caf\u{e9}'"),
            ("\u{a0}", "'\\xa0'"),
            ("\u{ad}", "'\\xad'"),
            ("\u{200b}", "'\\u200b'"),
            ("\u{feff}", "'\\ufeff'"),
            ("\u{2028}", "'\\u2028'"),
            ("\u{1f600}", "'\u{1f600}'"),
            ("\u{e0001}", "'\\U000e0001'"),
            ("\u{e000}", "'\\ue000'"),
            ("\u{f0000}", "'\\U000f0000'"),
            ("\u{378}", "'\\u0378'"),
            ("\u{85}", "'\\x85'"),
            ("\u{ff}", "'\u{ff}'"),
            ("\u{3000}", "'\\u3000'"),
            ("\u{fffe}", "'\\ufffe'"),
        ];
        for (text, repr) in cases {
            assert_eq!(py_repr_str(text), repr, "{:?}", text);
        }
    }

    #[test]
    fn repr_str_and_truth_of_json_values_follow_python() {
        let value = json!({"a": [1, 2.5, null, true, false], "b": "x'y", "c": {}, "d": -0.0, "e": 1e16});
        assert_eq!(py_repr(&value), "{'a': [1, 2.5, None, True, False], 'b': \"x'y\", 'c': {}, 'd': -0.0, 'e': 1e+16}");
        assert_eq!(py_str(&json!("plain")), "plain");
        assert_eq!(py_str(&json!(6)), "6");
        assert_eq!(py_str(&json!(6.0)), "6.0");
        assert_eq!(py_str(&json!(null)), "None");
        for falsy in [json!(null), json!(false), json!(0), json!(0.0), json!(""), json!([]), json!({})] {
            assert!(!py_truthy(&falsy), "{}", falsy);
        }
        for truthy in [json!(true), json!(1), json!(-0.5), json!("0"), json!([0]), json!({"k": null})] {
            assert!(py_truthy(&truthy), "{}", truthy);
        }
    }
}
