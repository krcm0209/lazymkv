//! `lazymkv-dev scan <slug|dir> [--title N | --label L]` (spec 3.4): what a recorded scan says about a disc, line
//! for line as the Python `summarize_scan` printed it; with `--title N`, the subtitle streams of title N as the scan lists
//! them (the phase 0 plan's Task 12 Step 5 listing); with `--label L`, the audio streams of rip L's file against
//! the scan's flags (Task 12 Step 15). Exit 0; 2 for an argument error or an unreadable input (the Python crashed
//! with 1); 3 when a slug is given and the evidence clone is absent.

use std::collections::BTreeSet;
use std::io::Write;

use lazymkv_core::recording::files::{FfprobeStream, FfprobeStreams, ProbeSummary, Summary};
use lazymkv_core::recording::{py_int, py_repr_str, read_json, Encoding, Errors, Tree};
use lazymkv_core::robot::{int_attr, Attrs, Scan};

use crate::evidence::{read_scan_in, resolve_recording, Ctx, Recording};
use crate::{usage_error, Args};

/// The usage line, after "usage: lazymkv-dev ".
pub const USAGE: &str = "scan [--evidence DIR] [--title N | --label L] <slug|dir>";
/// summarize_scan's Java line when no MSG:3344 names a runtime.
pub const JAVA_ABSENT: &str = "no (MSG:3344 absent)";
/// summarize_scan's table header, as its f-string padded it.
pub const TABLE_HEADER: &str =
    "  # source          duration chap         bytes segs aud sub  comment / segment map";
/// The Step 5 listing's first line after "<slug> title <n>". The Python named the alignment script, whose
/// command is now `lazymkv-dev align` (spec 3.4 rewrites lines that name a script).
pub const SUBTITLE_HEADER: &str = ": subtitle streams in scan order; s<k> in lazymkv-dev align is line s<k>";
/// Appended to the line of a derived forced-only stream (flags 4096 and 2048 together, spec 8.7).
pub const DERIVED_FORCED_NOTE: &str = "  derived forced-only: set aside, not counted";
/// The Step 5 count line, before its number.
pub const COUNTED_LINE: &str =
    "English or no-language subtitle streams, derived forced-only streams not counted:";
/// The Step 15 names of the scan's audio flags (attribute 22), in the Python's order.
pub const AUDIO_FLAG_NAMES: [(i64, &str); 5] = [
    (1, "director-comments"),
    (2, "alt-director-comments"),
    (4, "visually-impaired"),
    (512, "secondary"),
    (2048, "derived"),
];

/// The command, registered in COMMANDS as "scan".
pub fn run(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let mut parsed = Args::new(args, &["--evidence", "--title", "--label"]);
    let evidence = match parsed.flag("--evidence") {
        Ok(value) => value,
        Err(message) => return usage_error(err, "scan", USAGE, &message),
    };
    let title = match parsed.flag("--title") {
        Ok(value) => value,
        Err(message) => return usage_error(err, "scan", USAGE, &message),
    };
    let label = match parsed.flag("--label") {
        Ok(value) => value,
        Err(message) => return usage_error(err, "scan", USAGE, &message),
    };
    let words = match parsed.positionals() {
        Ok(words) => words,
        Err(message) => return usage_error(err, "scan", USAGE, &message),
    };
    let recording = match words.as_slice() {
        [] => return usage_error(err, "scan", USAGE, "the following arguments are required: recording"),
        [one] => one.clone(),
        [_, rest @ ..] => {
            return usage_error(err, "scan", USAGE, &format!("unrecognized arguments: {}", rest.join(" ")))
        }
    };
    if title.is_some() && label.is_some() {
        return usage_error(err, "scan", USAGE, "argument --label: not allowed with argument --title");
    }
    let title = match title {
        None => None,
        Some(text) => match py_int(&text) {
            Some(number) => Some(number),
            None => {
                let message = format!("argument --title: invalid int value: {}", py_repr_str(&text));
                return usage_error(err, "scan", USAGE, &message);
            }
        },
    };
    let rec = match resolve_recording(ctx, evidence.as_deref(), &recording) {
        Ok(rec) => rec,
        Err(refused) => {
            let _ = writeln!(err, "{}", refused.line);
            return refused.code;
        }
    };
    let lines = match (title, label) {
        (Some(title), _) => title_listing(&rec, title),
        (None, Some(label)) => label_listing(&rec, &label),
        (None, None) => summary(&rec),
    };
    match lines {
        Ok(lines) => {
            for line in lines {
                let _ = writeln!(out, "{line}");
            }
            0
        }
        Err(message) => {
            let _ = writeln!(err, "lazymkv-dev scan: error: {message}");
            2
        }
    }
}

/// summarize_scan's lines for a parsed scan.
pub fn summary_lines(scan: &Scan) -> Vec<String> {
    let version = first_param(scan, 1005).unwrap_or("?");
    let java = match first_param(scan, 3344) {
        Some(java) if !java.is_empty() => java,
        _ => JAVA_ABSENT,
    };
    let codes: BTreeSet<u32> = scan.messages.iter().map(|m| m.code).collect();
    let codes: Vec<String> = codes.iter().map(|code| code.to_string()).collect();
    let mut lines = vec![
        format!("MakeMKV: {version}"),
        format!("Java in use: {java}"),
        format!(
            "Disc: type={} name={} volume={}",
            or_none(scan.disc.get(&1)),
            or_none(scan.disc.get(&2)),
            or_none(scan.disc.get(&32))
        ),
        format!("Message codes seen: [{}]", codes.join(", ")),
        String::new(),
        TABLE_HEADER.to_string(),
    ];
    for (title, a) in &scan.titles {
        let (mut audio, mut subtitles) = (0usize, 0usize);
        for ((t, _), stream) in &scan.streams {
            if t != title {
                continue;
            }
            match stream.get(&1).map(String::as_str) {
                Some("Audio") => audio += 1,
                Some("Subtitles") => subtitles += 1,
                _ => {}
            }
        }
        let segment_map: String = attr(a, 26, "").chars().take(60).collect();
        lines.push(format!(
            "{:>3} {:<14} {:>9} {:>4} {:>13} {:>4} {:>3} {:>3}  {} [{}]",
            title,
            attr(a, 16, ""),
            attr(a, 9, ""),
            attr(a, 8, "0"),
            attr(a, 11, ""),
            attr(a, 25, ""),
            audio,
            subtitles,
            attr(a, 49, ""),
            segment_map
        ));
    }
    lines
}

/// The Step 5 listing of `title`; `probe` is None when all-tracks/probe-summary.json does not exist, else its
/// "subtitle_tracks" (None when the key is absent). Err with Python's int() text when a flag is not a number.
pub fn subtitle_lines(slug: &str, title: i64, scan: &Scan, probe: Option<Option<i64>>) -> Result<Vec<String>, String> {
    let mut lines = vec![format!("{slug} title {title}{SUBTITLE_HEADER}")];
    let (mut k, mut counted) = (0usize, 0usize);
    for ((t, s), a) in &scan.streams {
        if i64::from(*t) != title || a.get(&1).map(String::as_str) != Some("Subtitles") {
            continue;
        }
        let flags = int_attr(a, 22)?;
        let lang = match a.get(&3) {
            Some(lang) if !lang.is_empty() => lang.as_str(),
            _ => "und",
        };
        let derived_forced = (flags & 4096) != 0 && (flags & 2048) != 0;
        if (lang == "eng" || lang == "und") && !derived_forced {
            counted += 1;
        }
        let note = if derived_forced { DERIVED_FORCED_NOTE } else { "" };
        lines.push(format!(
            "s{:<3} stream {:>2}  lang={:<4} codec={:<10} flags={:<5} conversion={}{}",
            k,
            s,
            lang,
            attr(a, 6, ""),
            flags,
            attr(a, 42, ""),
            note
        ));
        k += 1;
    }
    lines.push(format!("{COUNTED_LINE} {counted}"));
    if let Some(in_file) = probe {
        lines.push(format!(
            "subtitle streams in the scan: {k}; subtitle tracks in the ripped file (probe-summary.json): {}",
            or_none(in_file)
        ));
    }
    Ok(lines)
}

/// The Step 15 listing: the audio streams of `probe` (the label's ffprobe-streams.json), the k-th against the scan's
/// k-th audio stream of `title`. Err with Python's int() text when a flag is not a number.
pub fn audio_lines(slug: &str, label: &str, title: i64, scan: &Scan, probe: &FfprobeStreams) -> Result<Vec<String>, String> {
    let mut flags = Vec::new();
    for ((t, _), a) in &scan.streams {
        if i64::from(*t) == title && a.get(&1).map(String::as_str) == Some("Audio") {
            flags.push(int_attr(a, 22)?);
        }
    }
    let audio: Vec<&FfprobeStream> =
        probe.streams.iter().filter(|s| s.codec_type.as_deref() == Some("audio")).collect();
    let mut lines = vec![format!(
        "{slug}/{label}: title {title}, {} audio streams in the file, {} in the scan",
        audio.len(),
        flags.len()
    )];
    for (k, s) in audio.iter().enumerate() {
        let f = flags.get(k).copied();
        let marks: Vec<&str> = match f {
            Some(f) => AUDIO_FLAG_NAMES.iter().filter(|(bit, _)| (f & *bit) != 0).map(|(_, name)| *name).collect(),
            None => Vec::new(),
        };
        let track_title = match tag(s, "title") {
            Some(title) => py_repr_str(title),
            None => "None".to_string(),
        };
        lines.push(format!(
            "  stream {}: {} profile={} {} ch {} lang={} title={} scan_flags={} {}",
            s.index,
            or_none(s.codec_name.as_deref()),
            or_none(s.profile.as_deref()),
            or_none(s.channels),
            or_none(s.channel_layout.as_deref()),
            tag(s, "language").map(String::as_str).unwrap_or("und"),
            track_title,
            or_none(f),
            marks.join(" ")
        ));
    }
    Ok(lines)
}

/// The first parameter of the first `code` message that has one.
/// Py: next((p[0] for code, _, p in scan.messages if code == c and p), default).
fn first_param(scan: &Scan, code: u32) -> Option<&str> {
    scan.messages.iter().find(|m| m.code == code && !m.params.is_empty()).map(|m| m.params[0].as_str())
}

/// `a.get(id, default)` of a title's or stream's attributes.
fn attr<'a>(a: &'a Attrs, id: u32, default: &'a str) -> &'a str {
    a.get(&id).map(String::as_str).unwrap_or(default)
}

/// `s.get("tags", {}).get(key)` of an ffprobe stream.
fn tag<'a>(stream: &'a FfprobeStream, key: &str) -> Option<&'a String> {
    stream.tags.as_ref().and_then(|tags| tags.get(key))
}

/// An f-string of a value that may be None: the value, or "None".
fn or_none<T: std::fmt::Display>(value: Option<T>) -> String {
    value.map_or_else(|| "None".to_string(), |value| value.to_string())
}

/// The recording's scan.txt, decoded with errors="replace" in `encoding` (summarize_scan: utf-8; the heredocs:
/// utf-8-sig), newlines as text mode gives them, parsed.
fn read_scan(rec: &Recording, encoding: Encoding) -> Result<Scan, String> {
    read_scan_in(&rec.tree, &rec.dir.join("scan.txt"), encoding, Errors::Replace)
}

/// summarize_scan's main.
fn summary(rec: &Recording) -> Result<Vec<String>, String> {
    Ok(summary_lines(&read_scan(rec, Encoding::Utf8)?))
}

/// The Step 5 heredoc with a title argument: the scan, then all-tracks/probe-summary.json when it exists.
fn title_listing(rec: &Recording, title: i64) -> Result<Vec<String>, String> {
    let scan = read_scan(rec, Encoding::Utf8Sig)?;
    let probe_path = rec.dir.join("all-tracks").join("probe-summary.json");
    let probe = if rec.tree.exists(&probe_path) {
        Some(read_json::<ProbeSummary>(&rec.tree, &probe_path)?.subtitle_tracks)
    } else {
        None
    };
    subtitle_lines(&rec.slug, title, &scan, probe)
}

/// The Step 15 heredoc: <label>/rip-summary.json, <label>/ffprobe-streams.json, scan.txt, in the Python's order.
fn label_listing(rec: &Recording, label: &str) -> Result<Vec<String>, String> {
    let folder = rec.dir.join(label);
    let summary_path = folder.join("rip-summary.json");
    let rip: Summary = read_json(&rec.tree, &summary_path)?;
    let probe: FfprobeStreams = read_json(&rec.tree, &folder.join("ffprobe-streams.json"))?;
    let scan = read_scan(rec, Encoding::Utf8Sig)?;
    let title = rip
        .title
        .ok_or_else(|| format!("{}: the summary names no title", rec.tree.display(&summary_path)))?;
    audio_lines(&rec.slug, label, title, &scan, &probe)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use lazymkv_core::recording::files::FfprobeStreams;
    use lazymkv_core::robot::parse;

    use super::{audio_lines, run, subtitle_lines, summary_lines};
    use crate::evidence::Ctx;

    /// A made-up robot-mode scan (LF here; written to disk with CRLF and a byte-order mark).
    const SCAN: &str = r##"MSG:1005,0,1,"MakeMKV v1.18.2 win(x64-release) started","%1 started","MakeMKV v1.18.2 win(x64-release)"
DRV:0,2,999,1,"BD-ROM DRIVE","TEST DISC","D:"
DRV:1,256,999,0,"","",""
MSG:3344,0,1,"Using Java runtime from C:\\Java\\bin\\java.exe","Using Java runtime from %1","C:\\Java\\bin\\java.exe"
MSG:3307,0,2,"File 00001.mpls was added as title #0","File %1 was added as title #%2","00001.mpls","0"
CINFO:1,6209,"Blu-ray disc"
CINFO:2,0,"Test Disc"
CINFO:32,0,"TEST_DISC"
TCOUNT:2
TINFO:0,8,0,"16"
TINFO:0,9,0,"1:41:13"
TINFO:0,11,0,"67757930496"
TINFO:0,16,0,"00001.mpls"
TINFO:0,25,0,"1"
TINFO:0,26,0,"1"
TINFO:0,49,0,"B1"
SINFO:0,0,1,6201,"Video"
SINFO:0,0,6,0,"MpegH"
SINFO:0,1,1,6202,"Audio"
SINFO:0,1,3,0,"eng"
SINFO:0,1,6,0,"TrueHD"
SINFO:0,1,22,0,"0"
SINFO:0,2,1,6202,"Audio"
SINFO:0,2,3,0,"eng"
SINFO:0,2,6,0,"DD"
SINFO:0,2,22,0,"2561"
SINFO:0,3,1,6203,"Subtitles"
SINFO:0,3,3,0,"eng"
SINFO:0,3,6,0,"PGS"
SINFO:0,3,42,0,"( Lossless conversion )"
SINFO:0,9,1,6203,"Subtitles"
SINFO:0,9,3,0,"eng"
SINFO:0,9,6,0,"PGS"
SINFO:0,9,22,0,"6144"
SINFO:0,9,42,0,"( Lossless conversion )"
SINFO:0,10,1,6203,"Subtitles"
SINFO:0,10,3,0,"fra"
SINFO:0,10,6,0,"PGS"
SINFO:0,10,22,0,"0"
SINFO:0,11,1,6203,"Subtitles"
SINFO:0,11,6,0,"CC608"
SINFO:0,11,22,0,""
SINFO:0,11,42,0,"( Converted to UTF-8 )"
TINFO:2,9,0,"0:06:00"
TINFO:2,16,0,"00007.m2ts"
TINFO:2,26,0,"7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30"
SINFO:2,0,1,6201,"Video"
SINFO:2,1,1,6202,"Audio"
SINFO:2,2,1,6203,"Subtitles"
"##;

    /// A scan of a disc MakeMKV could not open: no version, no Java line, no disc facts, no title.
    const BARE_SCAN: &str = "MSG:5010,0,0,\"Failed to open disc\",\"Failed to open disc\"\nTCOUNT:0\n";

    /// A made-up ffprobe-streams.json: video, three audio streams, a subtitle stream.
    const FFPROBE: &str = r##"{"streams": [
 {"index": 0, "codec_name": "hevc", "profile": "Main 10", "codec_type": "video", "tags": {"language": "eng"}},
 {"index": 1, "codec_name": "truehd", "codec_type": "audio", "channels": 8, "channel_layout": "7.1", "tags": {"language": "eng", "title": "Surround 7.1"}},
 {"index": 2, "codec_name": "ac3", "codec_type": "audio", "channels": 2, "channel_layout": "stereo", "tags": {"language": "eng", "title": "Director's commentary"}},
 {"index": 3, "codec_name": "dts", "profile": "DTS", "codec_type": "audio", "channels": 6, "channel_layout": "5.1(side)"},
 {"index": 4, "codec_name": "hdmv_pgs_subtitle", "codec_type": "subtitle", "tags": {"language": "eng"}}
], "format": {"filename": "title_t00.mkv"}}
"##;

    /// The Python `summarize_scan` over SCAN.
    const SUMMARY: &str = concat!(
        "MakeMKV: MakeMKV v1.18.2 win(x64-release)\n",
        "Java in use: C:\\Java\\bin\\java.exe\n",
        "Disc: type=Blu-ray disc name=Test Disc volume=TEST_DISC\n",
        "Message codes seen: [1005, 3307, 3344]\n",
        "\n",
        "  # source          duration chap         bytes segs aud sub  comment / segment map\n",
        "  0 00001.mpls       1:41:13   16   67757930496    1   2   4  B1 [1]\n",
        "  2 00007.m2ts       0:06:00    0                      1   1   [7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,]\n",
    );

    /// The Python `summarize_scan` over BARE_SCAN.
    const BARE_SUMMARY: &str = concat!(
        "MakeMKV: ?\n",
        "Java in use: no (MSG:3344 absent)\n",
        "Disc: type=None name=None volume=None\n",
        "Message codes seen: [5010]\n",
        "\n",
        "  # source          duration chap         bytes segs aud sub  comment / segment map\n",
    );

    /// The Step 5 heredoc, `test-disc-2000-bluray 0`, with all-tracks/probe-summary.json saying 4.
    const TITLE_0: &str = concat!(
        "test-disc-2000-bluray title 0: subtitle streams in scan order; s<k> in lazymkv-dev align is line s<k>\n",
        "s0   stream  3  lang=eng  codec=PGS        flags=0     conversion=( Lossless conversion )\n",
        "s1   stream  9  lang=eng  codec=PGS        flags=6144  conversion=( Lossless conversion )  derived forced-only: set aside, not counted\n",
        "s2   stream 10  lang=fra  codec=PGS        flags=0     conversion=\n",
        "s3   stream 11  lang=und  codec=CC608      flags=0     conversion=( Converted to UTF-8 )\n",
        "English or no-language subtitle streams, derived forced-only streams not counted: 2\n",
        "subtitle streams in the scan: 4; subtitle tracks in the ripped file (probe-summary.json): 4\n",
    );

    /// The Step 5 heredoc, `test-disc-2000-bluray 2`.
    const TITLE_2: &str = concat!(
        "test-disc-2000-bluray title 2: subtitle streams in scan order; s<k> in lazymkv-dev align is line s<k>\n",
        "s0   stream  2  lang=und  codec=           flags=0     conversion=\n",
        "English or no-language subtitle streams, derived forced-only streams not counted: 1\n",
        "subtitle streams in the scan: 1; subtitle tracks in the ripped file (probe-summary.json): 4\n",
    );

    /// The Step 5 heredoc, `bare-disc 0` (no all-tracks folder: no totals line).
    const BARE_TITLE_0: &str = concat!(
        "bare-disc title 0: subtitle streams in scan order; s<k> in lazymkv-dev align is line s<k>\n",
        "English or no-language subtitle streams, derived forced-only streams not counted: 0\n",
    );

    /// The Step 15 heredoc, `test-disc-2000-bluray all-tracks`.
    const LABEL_ALL_TRACKS: &str = concat!(
        "test-disc-2000-bluray/all-tracks: title 0, 3 audio streams in the file, 2 in the scan\n",
        "  stream 1: truehd profile=None 8 ch 7.1 lang=eng title='Surround 7.1' scan_flags=0 \n",
        "  stream 2: ac3 profile=None 2 ch stereo lang=eng title=\"Director's commentary\" scan_flags=2561 director-comments secondary derived\n",
        "  stream 3: dts profile=DTS 6 ch 5.1(side) lang=und title=None scan_flags=None \n",
    );

    /// Lines as print() writes them.
    fn text(lines: Vec<String>) -> String {
        lines.iter().map(|line| format!("{line}\n")).collect()
    }

    /// A folder under the system's temporary directory, removed when dropped.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> TempDir {
            let path = std::env::temp_dir().join(format!("lazymkv-dev-scan-{}-{name}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            TempDir(path)
        }

        fn write(&self, rel: &str, bytes: &[u8]) {
            let path = self.0.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, bytes).unwrap();
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// The fixture recordings on disk, as the mini PC writes them.
    fn fixture(name: &str) -> TempDir {
        let tmp = TempDir::new(name);
        let mut scan = b"\xef\xbb\xbf".to_vec();
        scan.extend_from_slice(SCAN.replace('\n', "\r\n").as_bytes());
        tmp.write("test-disc-2000-bluray/scan.txt", &scan);
        tmp.write(
            "test-disc-2000-bluray/all-tracks/probe-summary.json",
            "\u{feff}{\"ok\": true, \"result\": \"ok\", \"subtitle_tracks\": 4}\r\n".as_bytes(),
        );
        tmp.write(
            "test-disc-2000-bluray/all-tracks/rip-summary.json",
            "\u{feff}{\"ok\": true, \"result\": \"ok\", \"title\": 0}\r\n".as_bytes(),
        );
        tmp.write("test-disc-2000-bluray/all-tracks/ffprobe-streams.json", FFPROBE.as_bytes());
        tmp.write("test-disc-2000-bluray/eng-only/notes.txt", b"no summary here\n");
        tmp.write("bare-disc/scan.txt", BARE_SCAN.as_bytes());
        tmp
    }

    /// `run` with `dir` as the current directory: (exit code, stdout, stderr).
    fn run_in(dir: &Path, args: &[&str]) -> (i32, String, String) {
        let ctx = Ctx::for_test(dir);
        let args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run(&ctx, &args, &mut out, &mut err);
        (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
    }

    #[test]
    fn summary_lines_match_summarize_scan() {
        let scan = parse(&SCAN.replace('\n', "\r\n"));
        assert_eq!(text(summary_lines(&scan)), SUMMARY);
    }

    #[test]
    fn summary_lines_of_a_scan_without_disc_facts() {
        assert_eq!(text(summary_lines(&parse(BARE_SCAN))), BARE_SUMMARY);
    }

    #[test]
    fn subtitle_lines_match_the_step_5_listing() {
        let lines = subtitle_lines("test-disc-2000-bluray", 0, &parse(SCAN), Some(Some(4))).unwrap();
        assert_eq!(text(lines), TITLE_0);
    }

    #[test]
    fn subtitle_lines_of_another_title_and_without_a_probe_summary() {
        let lines = subtitle_lines("test-disc-2000-bluray", 2, &parse(SCAN), Some(Some(4))).unwrap();
        assert_eq!(text(lines), TITLE_2);
        let lines = subtitle_lines("bare-disc", 0, &parse(BARE_SCAN), None).unwrap();
        assert_eq!(text(lines), BARE_TITLE_0);
        // A probe summary without "subtitle_tracks": the Python's .get() gave None.
        let lines = subtitle_lines("bare-disc", 0, &parse(BARE_SCAN), Some(None)).unwrap();
        assert_eq!(
            lines.last().unwrap(),
            "subtitle streams in the scan: 0; subtitle tracks in the ripped file (probe-summary.json): None"
        );
    }

    #[test]
    fn audio_lines_match_the_step_15_listing() {
        let probe: FfprobeStreams = serde_json::from_str(FFPROBE).unwrap();
        let lines = audio_lines("test-disc-2000-bluray", "all-tracks", 0, &parse(SCAN), &probe).unwrap();
        assert_eq!(text(lines), LABEL_ALL_TRACKS);
    }

    #[test]
    fn run_prints_the_summary_from_disk() {
        let tmp = fixture("summary");
        let (code, out, err) = run_in(&tmp.0, &["test-disc-2000-bluray"]);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(out, SUMMARY);
        let (code, out, err) = run_in(&tmp.0, &["bare-disc"]);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(out, BARE_SUMMARY);
    }

    #[test]
    fn run_prints_both_listings_from_disk() {
        let tmp = fixture("listings");
        let (code, out, err) = run_in(&tmp.0, &["test-disc-2000-bluray", "--title", "0"]);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(out, TITLE_0);
        let (code, out, err) = run_in(&tmp.0, &["test-disc-2000-bluray", "--title=2"]);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(out, TITLE_2);
        let (code, out, err) = run_in(&tmp.0, &["bare-disc", "--title", "0"]);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(out, BARE_TITLE_0);
        let (code, out, err) = run_in(&tmp.0, &["--label", "all-tracks", "test-disc-2000-bluray"]);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(out, LABEL_ALL_TRACKS);
    }

    #[test]
    fn run_refuses_without_the_evidence_clone() {
        let tmp = TempDir::new("no-clone");
        let missing = tmp.0.join("corpus-owner-missing");
        let (code, out, err) = run_in(&tmp.0, &["no-such-slug", "--evidence", missing.to_str().unwrap()]);
        assert_eq!((code, out.as_str()), (3, ""));
        assert!(err.contains("the evidence repository lazymkv-evidence is not cloned there"), "{err}");
    }

    #[test]
    fn run_refuses_a_missing_path_with_or_without_the_clone() {
        // A path is never taken for a slug: "<arg>: no such folder", exit 2, with no evidence check (summarize_scan
        // crashed on it; before the fix the port exited 3 without the clone, or named corpus/owner/<arg> with it).
        let tmp = fixture("missing-path");
        let (evidence, missing) = (tmp.0.join("evidence"), tmp.0.join("no-evidence"));
        std::fs::create_dir_all(&evidence).unwrap();
        for flag in [&evidence, &missing] {
            let (code, out, err) = run_in(&tmp.0, &["recs/nope", "--evidence", flag.to_str().unwrap()]);
            assert_eq!((code, out.as_str(), err.as_str()), (2, "", "recs/nope: no such folder\n"), "{flag:?}");
        }
    }

    #[test]
    fn run_exits_2_when_a_file_is_missing() {
        let tmp = fixture("missing");
        tmp.write("empty-disc/meta.json", b"{}");
        let (code, out, err) = run_in(&tmp.0, &["empty-disc"]);
        assert_eq!((code, out.as_str()), (2, ""));
        assert!(err.starts_with("lazymkv-dev scan: error: "), "{err}");
        assert!(err.contains("No such file or directory") && err.contains("scan.txt"), "{err}");
        let (code, out, err) = run_in(&tmp.0, &["test-disc-2000-bluray", "--label", "eng-only"]);
        assert_eq!((code, out.as_str()), (2, ""));
        assert!(err.contains("No such file or directory") && err.contains("rip-summary.json"), "{err}");
    }

    #[test]
    fn run_reports_usage_errors() {
        let tmp = fixture("usage");
        let (code, _, err) = run_in(&tmp.0, &["test-disc-2000-bluray", "--title", "x"]);
        assert_eq!(code, 2);
        assert!(err.contains("lazymkv-dev scan: error: argument --title: invalid int value: 'x'"), "{err}");
        let (code, _, err) = run_in(&tmp.0, &["test-disc-2000-bluray", "--title", "0", "--label", "all-tracks"]);
        assert_eq!(code, 2);
        assert!(err.contains("lazymkv-dev scan: error: argument --label: not allowed with argument --title"), "{err}");
        let (code, _, err) = run_in(&tmp.0, &[]);
        assert_eq!(code, 2);
        assert!(err.contains("lazymkv-dev scan: error: the following arguments are required: recording"), "{err}");
        let (code, _, err) = run_in(&tmp.0, &["test-disc-2000-bluray", "bare-disc"]);
        assert_eq!(code, 2);
        assert!(err.contains("lazymkv-dev scan: error: unrecognized arguments: bare-disc"), "{err}");
    }

    #[test]
    fn run_exits_2_on_a_flag_python_could_not_read() {
        let tmp = TempDir::new("bad-flag");
        tmp.write("bad-flags/scan.txt", b"SINFO:0,1,1,6203,\"Subtitles\"\nSINFO:0,1,22,0,\"x\"\n");
        let (code, out, err) = run_in(&tmp.0, &["bad-flags", "--title", "0"]);
        assert_eq!((code, out.as_str()), (2, ""));
        assert_eq!(err, "lazymkv-dev scan: error: invalid literal for int() with base 10: 'x'\n");
    }

    #[test]
    fn scan_is_in_the_command_table() {
        assert!(crate::COMMANDS.iter().any(|(name, _, _)| *name == "scan"));
    }
}
