//! Bytes to text and text to lines, as the Python's readers do it (port spec 3.2): `bytes.decode` with "utf-8" or
//! "utf-8-sig" and errors="strict" or "replace", `str.splitlines`, text-mode file iteration, `robot.parse`'s
//! `split("\n")`, and `bytes.splitlines`.

/// Python's "utf-8" (a byte-order mark stays as U+FEFF) and "utf-8-sig" (one leading mark removed).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    Utf8,
    Utf8Sig,
}

/// Python's errors="strict" and errors="replace" (U+FFFD per maximal invalid subpart, as `from_utf8_lossy`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Errors {
    Strict,
    Replace,
}

const BOM: &[u8] = b"\xef\xbb\xbf";

/// Bytes to text as `bytes.decode(encoding, errors)`. Err is Python's UnicodeDecodeError text ("'utf-8' codec can't
/// decode byte 0xff in position 0: invalid start byte"), positions counted after a removed byte-order mark, as
/// Python counts them. Py: Path.read_text.
pub fn decode_text(bytes: &[u8], encoding: Encoding, errors: Errors) -> Result<String, String> {
    let body = match encoding {
        Encoding::Utf8Sig => bytes.strip_prefix(BOM).unwrap_or(bytes),
        Encoding::Utf8 => bytes,
    };
    match errors {
        Errors::Replace => Ok(String::from_utf8_lossy(body).into_owned()),
        Errors::Strict => match std::str::from_utf8(body) {
            Ok(text) => Ok(text.to_string()),
            Err(error) => Err(decode_error(body, &error)),
        },
    }
}

/// CPython's UnicodeDecodeError text for the first invalid sequence: one byte is "byte 0x.. in position n", more are
/// "bytes in position n-m"; a lead byte C2 to F4 makes the reason "invalid continuation byte", any other "invalid
/// start byte"; a sequence cut by the end of the input is "unexpected end of data".
fn decode_error(body: &[u8], error: &std::str::Utf8Error) -> String {
    let start = error.valid_up_to();
    let (count, reason) = match error.error_len() {
        None => (body.len() - start, "unexpected end of data"),
        Some(count) if (0xc2..=0xf4).contains(&body[start]) => (count, "invalid continuation byte"),
        Some(count) => (count, "invalid start byte"),
    };
    if count == 1 {
        format!("'utf-8' codec can't decode byte 0x{:02x} in position {}: {}", body[start], start, reason)
    } else {
        format!("'utf-8' codec can't decode bytes in position {}-{}: {}", start, start + count - 1, reason)
    }
}

/// Splits `text` after every character `is_boundary` accepts ("\r\n" counts as one), without the terminators and
/// without a final empty line.
fn split_lines(text: &str, is_boundary: fn(char) -> bool) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((at, c)) = chars.next() {
        if !is_boundary(c) {
            continue;
        }
        lines.push(&text[start..at]);
        start = at + c.len_utf8();
        if c == '\r' {
            if let Some(&(next_at, '\n')) = chars.peek() {
                chars.next();
                start = next_at + 1;
            }
        }
    }
    if start < text.len() {
        lines.push(&text[start..]);
    }
    lines
}

/// Lines on Python's `str.splitlines` boundaries: \n, \r, \r\n, \v, \f, \x1c, \x1d, \x1e, \x85, \u{2028}, \u{2029}; no
/// terminators; no final empty line. Py: str.splitlines.
pub fn py_splitlines(text: &str) -> Vec<&str> {
    split_lines(text, |c| {
        matches!(c, '\n' | '\r' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\u{85}' | '\u{2028}' | '\u{2029}')
    })
}

/// Python's text-mode read (universal newlines): "\r\n" and a lone "\r" become "\n", as `Path.read_text` gives them.
pub fn text_mode(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// Lines as Python's text-mode file iteration gives them (universal newlines: \n, \r, \r\n only), without
/// terminators. Py: `for line in open(path)`.
pub fn universal_lines(text: &str) -> Vec<&str> {
    split_lines(text, |c| c == '\n' || c == '\r')
}

/// `text.split("\n")`, each line with its trailing "\r"s removed (rstrip("\r")); a final empty piece kept, as split
/// keeps it. Py: robot.parse's loop.
pub fn strip_cr_lines(text: &str) -> Vec<&str> {
    text.split('\n').map(|line| line.trim_end_matches('\r')).collect()
}

/// Lines of bytes on `bytes.splitlines` boundaries (\n, \r, \r\n only), without terminators and without a final
/// empty line. Py: check_encoding's data.splitlines().
pub fn py_bytes_splitlines(data: &[u8]) -> Vec<&[u8]> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut at = 0;
    while at < data.len() {
        match data[at] {
            b'\n' => {
                lines.push(&data[start..at]);
                at += 1;
                start = at;
            }
            b'\r' => {
                lines.push(&data[start..at]);
                at += if data.get(at + 1) == Some(&b'\n') { 2 } else { 1 };
                start = at;
            }
            _ => at += 1,
        }
    }
    if start < data.len() {
        lines.push(&data[start..]);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strict(bytes: &[u8]) -> Result<String, String> {
        decode_text(bytes, Encoding::Utf8, Errors::Strict)
    }

    fn sig(bytes: &[u8], errors: Errors) -> Result<String, String> {
        decode_text(bytes, Encoding::Utf8Sig, errors)
    }

    fn replaced(bytes: &[u8]) -> Result<String, String> {
        decode_text(bytes, Encoding::Utf8, Errors::Replace)
    }

    fn error(text: &str) -> Result<String, String> {
        Err(text.to_string())
    }

    #[test]
    fn strict_decoding_fails_with_the_python_texts() {
        assert_eq!(strict(b"\xff"), error("'utf-8' codec can't decode byte 0xff in position 0: invalid start byte"));
        assert_eq!(strict(b"ab\xff"), error("'utf-8' codec can't decode byte 0xff in position 2: invalid start byte"));
        assert_eq!(strict(b"\xe2\x82"), error("'utf-8' codec can't decode bytes in position 0-1: unexpected end of data"));
        assert_eq!(strict(b"\xe2A"), error("'utf-8' codec can't decode byte 0xe2 in position 0: invalid continuation byte"));
        assert_eq!(strict(b"\xe2\x82A"), error("'utf-8' codec can't decode bytes in position 0-1: invalid continuation byte"));
        assert_eq!(strict(b"\xf0\x9f\x98"), error("'utf-8' codec can't decode bytes in position 0-2: unexpected end of data"));
        assert_eq!(strict(b"\xc3"), error("'utf-8' codec can't decode byte 0xc3 in position 0: unexpected end of data"));
        assert_eq!(strict(b"\xc0\x80"), error("'utf-8' codec can't decode byte 0xc0 in position 0: invalid start byte"));
        assert_eq!(strict(b"\xed\xa0\x80"), error("'utf-8' codec can't decode byte 0xed in position 0: invalid continuation byte"));
        assert_eq!(strict(b"plain"), Ok("plain".to_string()));
    }

    #[test]
    fn utf8_sig_removes_one_mark_and_counts_positions_after_it() {
        assert_eq!(sig(b"\xef\xbb\xbfMSG", Errors::Strict), Ok("MSG".to_string()));
        assert_eq!(strict(b"\xef\xbb\xbfMSG"), Ok("\u{feff}MSG".to_string()));
        assert_eq!(sig(b"\xef\xbb\xbf\xef\xbb\xbfx", Errors::Strict), Ok("\u{feff}x".to_string()));
        assert_eq!(sig(b"\xef\xbb\xbf\xff", Errors::Strict), error("'utf-8' codec can't decode byte 0xff in position 0: invalid start byte"));
        assert_eq!(strict(b"\xef\xbb\xbf\xff"), error("'utf-8' codec can't decode byte 0xff in position 3: invalid start byte"));
    }

    #[test]
    fn replace_decoding_gives_one_replacement_per_maximal_subpart() {
        assert_eq!(replaced(b"x\x80y"), Ok("x\u{fffd}y".to_string()));
        assert_eq!(replaced(b"\xe2\x82A"), Ok("\u{fffd}A".to_string()));
        assert_eq!(replaced(b"\xc0\x80"), Ok("\u{fffd}\u{fffd}".to_string()));
        assert_eq!(sig(b"\xef\xbb\xbf\xff", Errors::Replace), Ok("\u{fffd}".to_string()));
        assert_eq!(replaced(b"\xef\xbb\xbf\xff"), Ok("\u{feff}\u{fffd}".to_string()));
    }

    // One test per boundary of Python's str.splitlines (port spec 3.2).
    macro_rules! boundary {
        ($name:ident, $separator:expr) => {
            #[test]
            fn $name() {
                let text = format!("one{}two{}", $separator, $separator);
                assert_eq!(py_splitlines(&text), vec!["one", "two"]);
            }
        };
    }
    boundary!(splitlines_boundary_lf, "\n");
    boundary!(splitlines_boundary_cr, "\r");
    boundary!(splitlines_boundary_crlf, "\r\n");
    boundary!(splitlines_boundary_vt, "\x0b");
    boundary!(splitlines_boundary_ff, "\x0c");
    boundary!(splitlines_boundary_fs, "\x1c");
    boundary!(splitlines_boundary_gs, "\x1d");
    boundary!(splitlines_boundary_rs, "\x1e");
    boundary!(splitlines_boundary_nel, "\u{85}");
    boundary!(splitlines_boundary_line_separator, "\u{2028}");
    boundary!(splitlines_boundary_paragraph_separator, "\u{2029}");

    #[test]
    fn splitlines_keeps_empty_lines_but_no_final_one() {
        assert_eq!(py_splitlines(""), Vec::<&str>::new());
        assert_eq!(py_splitlines("a"), vec!["a"]);
        assert_eq!(py_splitlines("a\n\nb"), vec!["a", "", "b"]);
        assert_eq!(py_splitlines("a\r\r\nb"), vec!["a", "", "b"]);
        assert_eq!(py_splitlines("a\n\n"), vec!["a", ""]);
        assert_eq!(py_splitlines("a\x1fb\tc"), vec!["a\x1fb\tc"]);
    }

    #[test]
    fn text_mode_turns_crlf_and_lone_cr_into_lf() {
        assert_eq!(text_mode("a\r\nb\rc\nd\r\r\n"), "a\nb\nc\nd\n\n");
    }

    #[test]
    fn universal_lines_split_on_lf_cr_and_crlf_only() {
        assert_eq!(universal_lines("a\x0bb\x0cc\u{2028}d\r\ne\rf\n"), vec!["a\x0bb\x0cc\u{2028}d", "e", "f"]);
        assert_eq!(universal_lines("x\n\n"), vec!["x", ""]);
        assert_eq!(universal_lines(""), Vec::<&str>::new());
    }

    #[test]
    fn strip_cr_lines_split_on_lf_and_drop_trailing_cr() {
        assert_eq!(strip_cr_lines("\u{feff}MSG:1\r\nCINFO:2\r\r\n\r\n"), vec!["\u{feff}MSG:1", "CINFO:2", "", ""]);
        assert_eq!(strip_cr_lines("a\rb\n"), vec!["a\rb", ""]);
        assert_eq!(strip_cr_lines(""), vec![""]);
    }

    #[test]
    fn bytes_splitlines_split_on_lf_cr_and_crlf_only() {
        let expected = vec![&b"a"[..], &b"b"[..], &b"c"[..], &b"d\x0be"[..]];
        assert_eq!(py_bytes_splitlines(b"a\r\nb\rc\nd\x0be"), expected);
        assert_eq!(py_bytes_splitlines(b"x\n"), vec![&b"x"[..]]);
        assert!(py_bytes_splitlines(b"").is_empty());
    }
}
