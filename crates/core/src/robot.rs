//! The parser for makemkvcon's robot-mode output (spec 9.4), the port of phase 0's Python module `robot`:
//! `split_fields`, `parse` and `duration_seconds`, with the flag read every caller of the Python repeated
//! (`int_attr`) and the `scan.json` golden form (`scan_json`). No I/O: callers hand in the decoded text.

use std::collections::BTreeMap;

use crate::recording::{py_int, py_repr_str, strip_cr_lines, text_mode, Ordered};

pub mod lines;

/// A title's or stream's attributes: attribute number -> value (the last line wins). Py: the inner dicts of Scan.
pub type Attrs = BTreeMap<u32, String>;

/// One MSG line: (code, text, params) = (int(f[0]), f[3], f[5:]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub code: u32,
    pub text: String,
    pub params: Vec<String>,
}

/// One DRV line's fields. Never written to a golden (it can carry a drive serial).
pub type Drive = Vec<String>;

/// A parsed robot-mode log. Sparse title numbering kept. Py: robot.Scan.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scan {
    pub messages: Vec<Message>,
    pub drives: Vec<Drive>,
    /// CINFO: attribute -> value.
    pub disc: BTreeMap<u32, String>,
    /// TINFO: title -> attributes.
    pub titles: BTreeMap<u32, Attrs>,
    /// SINFO: (title, stream) -> attributes.
    pub streams: BTreeMap<(u32, u32), Attrs>,
    /// Lines the parser skipped and why ("field <i> is missing", "field <i> is not a number: '<text>'"); a line
    /// without ':' is not recorded. For diagnostics only: never printed by a report, never written.
    pub skipped: Vec<(String, String)>,
}

/// The fields after "KIND:": quoted fields keep commas, a backslash takes the next character, "" is a quote.
/// Py: robot.split_fields (indexing by character, as Python indexes a str).
pub fn split_fields(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut i = 0;
    while i < n {
        let c = chars[i];
        if quoted {
            if c == '\\' && i + 1 < n {
                current.push(chars[i + 1]);
                i += 2;
                continue;
            }
            if c == '"' {
                if i + 1 < n && chars[i + 1] == '"' {
                    current.push('"');
                    i += 2;
                    continue;
                }
                quoted = false;
            } else {
                current.push(c);
            }
        } else if c == '"' {
            quoted = true;
        } else if c == ',' {
            fields.push(std::mem::take(&mut current));
        } else {
            current.push(c);
        }
        i += 1;
    }
    fields.push(current);
    fields
}

/// Parses a robot-mode log, never failing. Every Python caller read the log with `read_text` (universal newlines), so
/// parse first translates "\r\n" and a lone "\r" itself (`text_mode`, idempotent), and every caller parses alike.
/// Py: robot.parse(path.read_text(...)).
pub fn parse(text: &str) -> Scan {
    let mut scan = Scan::default();
    let text = text_mode(text);
    for raw in strip_cr_lines(&text) {
        let line = raw.trim_start_matches('\u{feff}');
        let Some((kind, rest)) = line.split_once(':') else {
            continue;
        };
        let fields = split_fields(rest);
        let outcome = match kind {
            "MSG" => message(&mut scan, &fields),
            "DRV" => {
                scan.drives.push(fields);
                Ok(())
            }
            "CINFO" => disc_attribute(&mut scan, &fields),
            "TINFO" => title_attribute(&mut scan, &fields),
            "SINFO" => stream_attribute(&mut scan, &fields),
            _ => Ok(()),
        };
        if let Err(reason) = outcome {
            scan.skipped.push((line.to_string(), reason));
        }
    }
    scan
}

/// `f[i]`, or why the Python's f[i] raised IndexError.
fn field(fields: &[String], i: usize) -> Result<&str, String> {
    fields.get(i).map(String::as_str).ok_or_else(|| format!("field {i} is missing"))
}

/// `int(f[i])` as a u32, or why not (ValueError in the Python; a negative or too large number here too).
fn number(fields: &[String], i: usize) -> Result<u32, String> {
    let text = field(fields, i)?;
    py_int(text)
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(|| format!("field {i} is not a number: {}", py_repr_str(text)))
}

/// `scan.messages.append((int(f[0]), f[3], f[5:]))`: the tuple is built left to right.
pub(crate) fn message(scan: &mut Scan, fields: &[String]) -> Result<(), String> {
    let code = number(fields, 0)?;
    let text = field(fields, 3)?.to_string();
    let params = fields.get(5..).unwrap_or_default().to_vec();
    scan.messages.push(Message { code, text, params });
    Ok(())
}

/// `scan.disc[int(f[0])] = f[2]`: the value is evaluated first.
fn disc_attribute(scan: &mut Scan, fields: &[String]) -> Result<(), String> {
    let value = field(fields, 2)?.to_string();
    let attribute = number(fields, 0)?;
    scan.disc.insert(attribute, value);
    Ok(())
}

/// `scan.titles.setdefault(int(f[0]), {})[int(f[1])] = f[3]`: the value, then the title (created here and kept
/// when the attribute number then fails, as setdefault kept it), then the attribute.
fn title_attribute(scan: &mut Scan, fields: &[String]) -> Result<(), String> {
    let value = field(fields, 3)?.to_string();
    let title = number(fields, 0)?;
    let attrs = scan.titles.entry(title).or_default();
    let attribute = number(fields, 1)?;
    attrs.insert(attribute, value);
    Ok(())
}

/// `scan.streams.setdefault((int(f[0]), int(f[1])), {})[int(f[2])] = f[4]`, in the same order.
fn stream_attribute(scan: &mut Scan, fields: &[String]) -> Result<(), String> {
    let value = field(fields, 4)?.to_string();
    let key = (number(fields, 0)?, number(fields, 1)?);
    let attrs = scan.streams.entry(key).or_default();
    let attribute = number(fields, 2)?;
    attrs.insert(attribute, value);
    Ok(())
}

/// "H:MM:SS" to seconds; None where the Python raised ValueError (not three parts, a part not an int) and for a
/// negative total (which the Python returned; no recorded or pinned scan has one). Py: robot.duration_seconds.
pub fn duration_seconds(text: &str) -> Option<u32> {
    let parts: Vec<&str> = text.split(':').collect();
    let [hours, minutes, seconds] = parts.as_slice() else {
        return None;
    };
    let total = py_int(hours)?
        .checked_mul(3600)?
        .checked_add(py_int(minutes)?.checked_mul(60)?)?
        .checked_add(py_int(seconds)?)?;
    u32::try_from(total).ok()
}

/// `int(a.get(id, "0") or 0)`: 0 for an absent or empty attribute; Err with Python's ValueError text (its repr of
/// the text cut at 200 characters, as `%.200R`) where int() raised.
pub fn int_attr(a: &Attrs, id: u32) -> Result<i64, String> {
    match a.get(&id).map(String::as_str) {
        None | Some("") => Ok(0),
        Some(text) => py_int(text).ok_or_else(|| {
            let shown: String = py_repr_str(text).chars().take(200).collect();
            format!("invalid literal for int() with base 10: {shown}")
        }),
    }
}

/// scan.json's content (section 5): titles, streams and disc with decimal-string keys in numeric order (a stream's
/// key is "<title>,<stream>"), and duration_seconds of every title's attribute 9 (null when absent or unreadable).
/// No drives, no messages.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ScanJson {
    pub titles: Ordered<Ordered<String>>,
    pub streams: Ordered<Ordered<String>>,
    pub disc: Ordered<String>,
    pub duration_seconds: Ordered<Option<u32>>,
}

/// Builds ScanJson from a Scan.
pub fn scan_json(scan: &Scan) -> ScanJson {
    let mut titles = Ordered::new();
    let mut durations = Ordered::new();
    for (title, attrs) in &scan.titles {
        titles.insert(title.to_string(), attrs_json(attrs));
        durations.insert(title.to_string(), attrs.get(&9).and_then(|text| duration_seconds(text)));
    }
    let mut streams = Ordered::new();
    for ((title, stream), attrs) in &scan.streams {
        streams.insert(format!("{title},{stream}"), attrs_json(attrs));
    }
    ScanJson { titles, streams, disc: attrs_json(&scan.disc), duration_seconds: durations }
}

/// Attribute numbers as decimal strings, in numeric order.
fn attrs_json(attrs: &BTreeMap<u32, String>) -> Ordered<String> {
    let mut out = Ordered::new();
    for (attribute, value) in attrs {
        out.insert(attribute.to_string(), value.clone());
    }
    out
}

#[cfg(test)]
mod tests {
    use crate::recording::{decode_text, Encoding, Errors};
    use crate::robot::{parse, Scan};

    /// TheDiscDB's John Wick UHD log (`corpus/thediscdb`), compiled into the test binary.
    const JOHN_WICK: &[u8] =
        include_bytes!("../../../corpus/thediscdb/movie/John Wick (2014)/4k-ultra-hd--blu-ray/disc01.txt");

    /// Py: Parse.setUp, `robot.parse(JOHN_WICK.read_text(encoding="utf-8", errors="replace"))`.
    fn john_wick() -> Scan {
        parse(&decode_text(JOHN_WICK, Encoding::Utf8, Errors::Replace).unwrap())
    }

    /// Py: test_analysis.SplitFields.
    mod split_fields {
        use crate::robot::split_fields;

        #[test]
        fn quoted_fields_keep_commas_and_unescape_backslashes() {
            let line = concat!(
                r#"3344,0,1,"Using Java runtime from C:\\Program Files\\Java\\bin\\java.exe","#,
                r#""Using Java runtime from %1","C:\\Program Files\\Java\\bin\\java.exe""#,
            );
            let fields = split_fields(line);
            assert_eq!(fields[0..3], ["3344", "0", "1"]);
            assert_eq!(fields[5], r"C:\Program Files\Java\bin\java.exe");
        }

        #[test]
        fn both_quote_escapes_are_accepted() {
            assert_eq!(split_fields(r#"1,"a \"b\", c","d ""e""""#), ["1", r#"a "b", c"#, r#"d "e""#]);
        }
    }

    /// Py: test_analysis.Parse.
    mod parse {
        use super::john_wick;

        #[test]
        fn titles_and_streams() {
            let scan = john_wick();
            assert_eq!(scan.titles[&0][&16], "00001.mpls");
            assert_eq!(scan.titles[&0][&9], "1:41:13");
            assert_eq!(scan.titles[&0][&8], "16");
            assert_eq!(scan.streams[&(0, 1)][&1], "Audio");
            assert_eq!(scan.streams[&(0, 1)][&6], "TrueHD");
            assert_eq!(scan.streams[&(0, 1)][&3], "eng");
        }

        #[test]
        fn messages_carry_their_parameters() {
            let scan = john_wick();
            let added: Vec<&Vec<String>> =
                scan.messages.iter().filter(|m| m.code == 3307).map(|m| &m.params).collect();
            assert_eq!(*added[0], ["00001.mpls", "0"]);
            let version: Vec<&Vec<String>> =
                scan.messages.iter().filter(|m| m.code == 1005).map(|m| &m.params).collect();
            assert!(version[0][0].starts_with("MakeMKV v"));
        }

        #[test]
        fn drives() {
            let scan = john_wick();
            assert_eq!(scan.drives.len(), 16);
            assert_eq!(scan.drives[0][1], "2");
        }

        #[test]
        fn duration_seconds() {
            assert_eq!(crate::robot::duration_seconds("1:41:13"), Some(6073));
            assert_eq!(crate::robot::duration_seconds("0:06:00"), Some(360));
        }
    }

    /// Py: test_decisions.ParseLines.
    mod parse_lines {
        use crate::robot::parse;

        #[test]
        fn only_a_newline_ends_a_line() {
            let scan = parse("\u{feff}TINFO:0,2,0,\"A\u{2028}B\"\r\nTINFO:0,9,0,\"0:10:00\"\r\n");
            assert_eq!(scan.titles[&0][&2], "A\u{2028}B");
            assert_eq!(scan.titles[&0][&9], "0:10:00");
        }
    }

    /// New: Review Focus 1, robot.parse's evaluation order, and the helpers' edges.
    mod added {
        use std::collections::BTreeMap;

        use crate::robot::{duration_seconds, int_attr, parse, scan_json, split_fields, Attrs, Message, ScanJson};

        /// A scan as the mini PC records it: UTF-8 with a byte-order mark, CRLF line ends. Made-up values.
        const MINI_PC_SCAN: &str = "\u{feff}MSG:1005,0,1,\"MakeMKV v1.18.2 win(x64-release) started\",\
            \"%1 started\",\"MakeMKV v1.18.2 win(x64-release)\"\r\n\
            DRV:0,2,999,1,\"BD-ROM DRIVE\",\"TEST DISC\",\"D:\"\r\n\
            CINFO:1,6209,\"Blu-ray disc\"\r\n\
            CINFO:2,0,\"Test Disc\"\r\n\
            TCOUNT:1\r\n\
            TINFO:0,9,0,\"0:10:00\"\r\n\
            TINFO:0,16,0,\"00001.mpls\"\r\n\
            SINFO:0,0,1,6201,\"Video\"\r\n\
            SINFO:0,1,1,6202,\"Audio\"\r\n\
            SINFO:0,1,3,0,\"eng\"\r\n\
            PRGV:0,0,65536\r\n";

        #[test]
        fn bom_and_crlf_lines_all_parse() {
            let scan = parse(MINI_PC_SCAN);
            assert!(scan.skipped.is_empty(), "{:?}", scan.skipped);
            assert_eq!(
                scan.messages,
                [Message {
                    code: 1005,
                    text: "MakeMKV v1.18.2 win(x64-release) started".to_string(),
                    params: vec!["MakeMKV v1.18.2 win(x64-release)".to_string()],
                }]
            );
            assert_eq!(scan.drives, [["0", "2", "999", "1", "BD-ROM DRIVE", "TEST DISC", "D:"]]);
            assert_eq!(
                scan.disc,
                BTreeMap::from([(1, "Blu-ray disc".to_string()), (2, "Test Disc".to_string())])
            );
            let title: Attrs = BTreeMap::from([(9, "0:10:00".to_string()), (16, "00001.mpls".to_string())]);
            assert_eq!(scan.titles, BTreeMap::from([(0, title)]));
            assert_eq!(scan.streams[&(0, 0)][&1], "Video");
            assert_eq!(scan.streams[&(0, 1)][&1], "Audio");
            assert_eq!(scan.streams[&(0, 1)][&3], "eng");
            // The same scan with LF line ends and no mark parses to the same Scan: no value keeps a '\r'.
            let unix = MINI_PC_SCAN.trim_start_matches('\u{feff}').replace("\r\n", "\n");
            assert_eq!(scan, parse(&unix));
        }

        #[test]
        fn a_skipped_line_keeps_the_side_effects_robot_parse_had() {
            let scan = parse(concat!(
                "TINFO:3,x,0,\"v\"\n",   // setdefault created title 3 before int(f[1]) raised
                "SINFO:1,2,y,0,\"v\"\n", // and stream (1, 2) before int(f[2]) raised
                "MSG:x,0,0,\"t\"\n",
                "CINFO:1,0\n",           // f[2] missing
                "TINFO:4,9,0\n",         // f[3] is evaluated first: title 4 is never created
                "no colon here\n",
                "PRGV:1,2,3\n",
                "TCOUNT:5\n",
                "CINFO:x,0,\"v\"\n",
                "TINFO:5,9,0,\"0:01:00\"\n",
            ));
            assert_eq!(scan.titles.keys().copied().collect::<Vec<u32>>(), [3, 5]);
            assert!(scan.titles[&3].is_empty());
            assert_eq!(scan.titles[&5][&9], "0:01:00");
            assert_eq!(scan.streams.keys().copied().collect::<Vec<(u32, u32)>>(), [(1, 2)]);
            assert!(scan.streams[&(1, 2)].is_empty());
            assert!(scan.messages.is_empty());
            assert!(scan.disc.is_empty());
            let skipped: Vec<(&str, &str)> =
                scan.skipped.iter().map(|(line, why)| (line.as_str(), why.as_str())).collect();
            assert_eq!(
                skipped,
                [
                    ("TINFO:3,x,0,\"v\"", "field 1 is not a number: 'x'"),
                    ("SINFO:1,2,y,0,\"v\"", "field 2 is not a number: 'y'"),
                    ("MSG:x,0,0,\"t\"", "field 0 is not a number: 'x'"),
                    ("CINFO:1,0", "field 2 is missing"),
                    ("TINFO:4,9,0", "field 3 is missing"),
                    ("CINFO:x,0,\"v\"", "field 0 is not a number: 'x'"),
                ]
            );
        }

        #[test]
        fn a_lone_cr_ends_a_line_as_read_text_made_it() {
            // Pinned to d3a754d's robot.parse(path.read_text(encoding="utf-8", errors="replace")) on these bytes:
            // {0: {9: '0:10:00', 2: 'A'}}, {(0, 1): {1: 'Audio'}}, [(5010, 'x', [])]. The CR inside the quoted value
            // ends that line too; its tail 'B"' has no ':' and is not recorded.
            let scan = parse(concat!(
                "\u{feff}TINFO:0,9,0,\"0:10:00\"\rTINFO:0,2,0,\"A\rB\"\r\n",
                "SINFO:0,1,1,6202,\"Audio\"\rMSG:5010,0,0,\"x\",\"y\"\r",
            ));
            let title: Attrs = BTreeMap::from([(9, "0:10:00".to_string()), (2, "A".to_string())]);
            assert_eq!(scan.titles, BTreeMap::from([(0, title)]));
            let stream: Attrs = BTreeMap::from([(1, "Audio".to_string())]);
            assert_eq!(scan.streams, BTreeMap::from([((0, 1), stream)]));
            assert_eq!(scan.messages, [Message { code: 5010, text: "x".to_string(), params: vec![] }]);
            assert!(scan.skipped.is_empty(), "{:?}", scan.skipped);
        }

        #[test]
        fn split_fields_edges() {
            assert_eq!(split_fields(""), [""]);
            assert_eq!(split_fields(r#""a\"#), [r"a\"]);
            assert_eq!(split_fields(r"a\,b"), [r"a\", "b"]);
            assert_eq!(split_fields(r#""x,y"z,"",w"#), ["x,yz", "", "w"]);
        }

        #[test]
        fn duration_seconds_is_none_where_python_raised() {
            assert_eq!(duration_seconds("1:41"), None);
            assert_eq!(duration_seconds("a:00:00"), None);
            assert_eq!(duration_seconds("1:2:3:4"), None);
            assert_eq!(duration_seconds(""), None);
            assert_eq!(duration_seconds(" 1:02:03"), Some(3723));
            assert_eq!(duration_seconds("1_0:00:00"), Some(36000));
            // Python returned -3600 and -30 here; no recorded or pinned scan has a negative duration.
            assert_eq!(duration_seconds("-1:00:00"), None);
            assert_eq!(duration_seconds("0:-1:30"), None);
        }

        #[test]
        fn int_attr_reads_a_flag_as_python_int_did() {
            let a: Attrs = BTreeMap::from([
                (22, "4096".to_string()),
                (14, String::new()),
                (3, " 12 ".to_string()),
                (6, "x".to_string()),
                (7, "1.5".to_string()),
            ]);
            assert_eq!(int_attr(&a, 22), Ok(4096));
            assert_eq!(int_attr(&a, 14), Ok(0));
            assert_eq!(int_attr(&a, 99), Ok(0));
            assert_eq!(int_attr(&a, 3), Ok(12));
            assert_eq!(int_attr(&a, 6), Err("invalid literal for int() with base 10: 'x'".to_string()));
            assert_eq!(int_attr(&a, 7), Err("invalid literal for int() with base 10: '1.5'".to_string()));
        }

        #[test]
        fn scan_json_orders_keys_by_number_and_leaves_out_drives_and_messages() {
            let scan = parse(concat!(
                "MSG:1005,0,1,\"m\",\"%1\",\"m\"\n",
                "DRV:0,2,999,1,\"a\",\"b\",\"c\"\n",
                "CINFO:32,0,\"V\"\n",
                "CINFO:2,0,\"N\"\n",
                "TINFO:10,9,0,\"0:01:00\"\n",
                "TINFO:2,16,0,\"00002.mpls\"\n",
                "TINFO:2,9,0,\"1:00:00\"\n",
                "TINFO:3,9,0,\"x\"\n",
                "SINFO:10,1,1,6202,\"Audio\"\n",
                "SINFO:2,10,1,6203,\"Subtitles\"\n",
                "SINFO:2,9,1,6202,\"Audio\"\n",
            ));
            let json = scan_json(&scan);
            let text = serde_json::to_string(&json).unwrap();
            assert_eq!(
                text,
                concat!(
                    r#"{"titles":{"2":{"9":"1:00:00","16":"00002.mpls"},"3":{"9":"x"},"10":{"9":"0:01:00"}},"#,
                    r#""streams":{"2,9":{"1":"Audio"},"2,10":{"1":"Subtitles"},"10,1":{"1":"Audio"}},"#,
                    r#""disc":{"2":"N","32":"V"},"#,
                    r#""duration_seconds":{"2":3600,"3":null,"10":60}}"#,
                )
            );
            let back: ScanJson = serde_json::from_str(&text).unwrap();
            assert_eq!(back, json);
        }
    }
}
