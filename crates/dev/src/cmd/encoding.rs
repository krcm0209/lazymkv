//! `lazymkv-dev encoding <file>`: experiment 8, which encoding does makemkvcon use for robot output?
//! Py: check_encoding, its lines verbatim. Exit 0; 2 where the Python crashed (no argument, the file unreadable).

use crate::evidence::{first_word, os_error_text, pathlib_str, Ctx};
use lazymkv_core::recording::{decode_text, py_bytes_splitlines, Encoding, Errors};
use std::io::Write;

/// The command's name as typed.
const NAME: &str = "encoding";
/// Its usage after "usage: lazymkv-dev ". The Python read sys.argv[1] and ignored any further word; so does this.
const USAGE: &str = "encoding <file>";
/// How many lines with a byte above 127 are shown, and how many characters of each shown line.
const SHOW_NON_ASCII: usize = 5;
const SHOW_CHARS: usize = 160;
/// How many lines with a backslash-escaped quote are shown.
const SHOW_QUOTED: usize = 3;

/// Windows-1252 for the bytes 0x80 to 0x9F, and U+FFFD for the five it leaves undefined (0x81, 0x8D, 0x8F, 0x90,
/// 0x9D), as Python's decode("cp1252", errors="replace") gives them. Every other byte is Latin-1 (U+0000 to U+007F,
/// U+00A0 to U+00FF).
const CP1252_80_9F: [char; 32] = [
    '\u{20ac}', '\u{fffd}', '\u{201a}', '\u{0192}', '\u{201e}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{02c6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{fffd}', '\u{017d}', '\u{fffd}',
    '\u{fffd}', '\u{2018}', '\u{2019}', '\u{201c}', '\u{201d}', '\u{2022}', '\u{2013}', '\u{2014}',
    '\u{02dc}', '\u{2122}', '\u{0161}', '\u{203a}', '\u{0153}', '\u{fffd}', '\u{017e}', '\u{0178}',
];

/// The lines check_encoding.main prints for the bytes of a robot log. Py: check_encoding.main.
pub fn report_lines(data: &[u8]) -> Vec<String> {
    let mut lines = Vec::new();
    match decode_text(data, Encoding::Utf8, Errors::Strict) {
        Ok(_) => lines.push("valid UTF-8: yes".to_string()),
        Err(error) => lines.push(format!("valid UTF-8: NO ({error})")),
    }
    let mut shown = 0;
    for raw in py_bytes_splitlines(data) {
        if raw.iter().any(|byte| *byte > 127) {
            lines.push(format!("utf-8 : {}", first_chars(&utf8_replaced(raw), SHOW_CHARS)));
            lines.push(format!("cp1252: {}", first_chars(&cp1252(raw), SHOW_CHARS)));
            shown += 1;
            if shown == SHOW_NON_ASCII {
                break;
            }
        }
    }
    lines.push(format!("lines with non-ASCII bytes shown: {shown}"));
    let quoted: Vec<&[u8]> = py_bytes_splitlines(data)
        .into_iter()
        .filter(|raw| raw.windows(2).any(|pair| pair == b"\\\""))
        .collect();
    lines.push(format!("lines with a backslash-escaped quote: {}", quoted.len()));
    for raw in quoted.iter().take(SHOW_QUOTED) {
        lines.push(format!("   {}", first_chars(&utf8_replaced(raw), SHOW_CHARS)));
    }
    lines
}

/// bytes.decode("utf-8", errors="replace").
fn utf8_replaced(raw: &[u8]) -> String {
    decode_text(raw, Encoding::Utf8, Errors::Replace).unwrap_or_default()
}

/// bytes.decode("cp1252", errors="replace").
fn cp1252(raw: &[u8]) -> String {
    raw.iter()
        .map(|&byte| if (0x80..=0x9f).contains(&byte) { CP1252_80_9F[usize::from(byte - 0x80)] } else { char::from(byte) })
        .collect()
}

/// text[:n]: the first `n` characters.
fn first_chars(text: &str, n: usize) -> String {
    text.chars().take(n).collect()
}

/// `lazymkv-dev encoding <file>`. Py: check_encoding's `main(sys.argv[1])`.
pub fn run(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let file = match first_word(args, out, err, NAME, USAGE, "file") {
        Ok(file) => file,
        Err(code) => return code,
    };
    match std::fs::read(ctx.abs(file)).map_err(|error| os_error_text(&error, &pathlib_str(file))) {
        Ok(data) => {
            for line in report_lines(&data) {
                let _ = writeln!(out, "{line}");
            }
            0
        }
        Err(error) => {
            let _ = writeln!(err, "lazymkv-dev {NAME}: error: {error}");
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{report_lines, run};
    use crate::evidence::Ctx;
    use std::path::Path;

    #[test]
    fn encoding_reports_invalid_utf8_and_both_decodings() {
        // A CRLF line, a line with a Latin-1 byte and a byte cp1252 leaves undefined, a quoted line ended by a lone
        // "\r", and a UTF-8 line.
        let data = b"plain\r\nMSG:1,\"caf\xe9 \x81\"\nTINFO:0,2,0,\"Disc \\\"A\\\"\"\rsecond \xc3\xa9 line\n";
        assert_eq!(
            report_lines(data),
            [
                "valid UTF-8: NO ('utf-8' codec can't decode byte 0xe9 in position 17: invalid continuation byte)",
                "utf-8 : MSG:1,\"caf\u{fffd} \u{fffd}\"",
                "cp1252: MSG:1,\"caf\u{e9} \u{fffd}\"",
                "utf-8 : second \u{e9} line",
                "cp1252: second \u{c3}\u{a9} line",
                "lines with non-ASCII bytes shown: 2",
                "lines with a backslash-escaped quote: 1",
                "   TINFO:0,2,0,\"Disc \\\"A\\\"\"",
            ]
        );
    }

    #[test]
    fn encoding_shows_five_lines_of_160_characters_and_three_quoted_lines() {
        let mut data = b"\xef\xbb\xbfok\n".to_vec();
        for k in 0..6 {
            data.extend_from_slice(format!("line {k} \u{e9} {}\n", "x".repeat(200)).as_bytes());
        }
        for k in 0..4 {
            data.extend_from_slice(format!("Q{k} \\\"q\\\"\n").as_bytes());
        }
        let mut want = vec![
            "valid UTF-8: yes".to_string(),
            "utf-8 : \u{feff}ok".to_string(),
            "cp1252: \u{ef}\u{bb}\u{bf}ok".to_string(),
        ];
        for k in 0..4 {
            want.push(format!("utf-8 : line {k} \u{e9} {}", "x".repeat(151)));
            want.push(format!("cp1252: line {k} \u{c3}\u{a9} {}", "x".repeat(150)));
        }
        want.push("lines with non-ASCII bytes shown: 5".to_string());
        want.push("lines with a backslash-escaped quote: 4".to_string());
        for k in 0..3 {
            want.push(format!("   Q{k} \\\"q\\\""));
        }
        assert_eq!(report_lines(&data), want);
    }

    #[test]
    fn encoding_maps_every_byte_as_cp1252_does() {
        let mut data: Vec<u8> = (0x80..=0x9f).collect();
        data.extend_from_slice(b"\n\xa0\xe9\xff\n");
        assert_eq!(
            report_lines(&data),
            [
                "valid UTF-8: NO ('utf-8' codec can't decode byte 0x80 in position 0: invalid start byte)".to_string(),
                format!("utf-8 : {}", "\u{fffd}".repeat(32)),
                "cp1252: \u{20ac}\u{fffd}\u{201a}\u{192}\u{201e}\u{2026}\u{2020}\u{2021}\u{2c6}\u{2030}\u{160}\u{2039}\u{152}\u{fffd}\u{17d}\u{fffd}\u{fffd}\u{2018}\u{2019}\u{201c}\u{201d}\u{2022}\u{2013}\u{2014}\u{2dc}\u{2122}\u{161}\u{203a}\u{153}\u{fffd}\u{17e}\u{178}".to_string(),
                "utf-8 : \u{fffd}\u{fffd}\u{fffd}".to_string(),
                "cp1252: \u{a0}\u{e9}\u{ff}".to_string(),
                "lines with non-ASCII bytes shown: 2".to_string(),
                "lines with a backslash-escaped quote: 0".to_string(),
            ]
        );
    }

    #[test]
    fn encoding_without_a_readable_file_exits_2() {
        let ctx = Ctx::for_test(Path::new("."));
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(run(&ctx, &[], &mut out, &mut err), 2);
        assert_eq!(
            String::from_utf8(err).unwrap(),
            "usage: lazymkv-dev encoding <file>\nlazymkv-dev encoding: error: the following arguments are required: file\n"
        );
        let missing = std::env::temp_dir().join(format!("lazymkv-test-encoding-{}-missing.txt", std::process::id()));
        let args = vec![missing.to_string_lossy().into_owned()];
        let (mut out2, mut err2) = (Vec::new(), Vec::new());
        assert_eq!(run(&ctx, &args, &mut out2, &mut err2), 2);
        assert!(out.is_empty() && out2.is_empty());
        assert!(String::from_utf8(err2).unwrap().starts_with("lazymkv-dev encoding: error: "));
    }

    #[test]
    fn encoding_names_a_missing_file_as_python_does() {
        let ctx = Ctx::for_test(Path::new("."));
        let args = vec!["./lazymkv-no-such-folder//x.txt".to_string()];
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(run(&ctx, &args, &mut out, &mut err), 2);
        assert!(out.is_empty());
        assert_eq!(
            String::from_utf8(err).unwrap(),
            "lazymkv-dev encoding: error: [Errno 2] No such file or directory: 'lazymkv-no-such-folder/x.txt'\n"
        );
    }
}
