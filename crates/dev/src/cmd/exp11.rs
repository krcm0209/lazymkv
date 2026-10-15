//! `lazymkv-dev exp11 coverage <where>... | match <folder> <title> <label>`: experiment 11 (spec section 17, row 11),
//! which streams of a title put the eng-only selection to the test, and which of the title's streams a rip made with
//! that selection kept. Py: exp11_tracks, its lines verbatim. Exit 0; 1 when the scan lists no such title (the
//! Python's sys.exit message, on stderr); 2 for an argument error (argparse's lines) and where the Python crashed (a
//! scan, mkvmerge.json or rip-summary.json unreadable, a title word that is not a number, a flag that is not one).

use crate::evidence::{read_file_text, read_scan_file, Ctx};
use crate::{usage_error, Args};
use lazymkv_core::recording::files::{MkvTrack, MkvmergeIdent, Selection, Summary};
use lazymkv_core::recording::{pathlib_str, pathlib_under, py_int, py_repr, py_repr_str, py_str, Encoding, Errors};
use lazymkv_core::reconcile::{expected_rows, file_rows, same_track, ExpectedRow, FileRow};
use lazymkv_core::robot::{duration_seconds, int_attr, Attrs, Scan};
use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};

/// The command's name as typed.
const NAME: &str = "exp11";
/// argparse's usage lines, under the command's name.
const USAGE: &str = "exp11 [-h] {coverage,match} ...";
const COVERAGE_USAGE: &str = "exp11 coverage [-h] where [where ...]";
const MATCH_USAGE: &str = "exp11 match [-h] folder title label";
/// match's positional arguments, as argparse names them.
const MATCH_POSITIONALS: [&str; 3] = ["folder", "title", "label"];

/// What -h prints: argparse's help of exp11_tracks (its module text) at 80 columns.
const HELP: &str = "usage: lazymkv-dev exp11 [-h] {coverage,match} ...

Experiment 11 (spec section 17, row 11): which streams of a title put the eng-only selection to the
test, and which of the title's streams a rip made with that selection kept.

  lazymkv-dev exp11 coverage <recording dir or scan file>[@<title>] ...
      With @<title>: every stream of that title (attributes 3 and 28, codec, channels, flags, and whether
      compare_tracks' eng prediction keeps it), then its COVERAGE line. Without @<title>: one COVERAGE
      line for each title of 20 minutes or more. A recording dir is read as <dir>/scan.txt.
  lazymkv-dev exp11 match <recording dir> <title> <label>
      Pairs the tracks of <dir>/<label>/mkvmerge.json, in order, with the scan streams they can be
      (type, language, audio channels, as compare_tracks does), and names each predicted stream that is
      missing and each unpredicted one that is present. Then it lists the file's tracks, and those of
      <dir>/all-tracks/mkvmerge.json when that rip is of the same title, for checking by eye.

COVERAGE counts the streams that put each part of the selection to the test: audio-lang (audio with a
language code other than English), nolang (audio with no language code, or und), sub-lang (subtitles
not in English), core (audio flag 256), special (audio flags 1, 2 or 4). Two more are only observed:
sub-flagged (subtitles with flags 1, 2 or 4) and audio-512 (secondary audio, flag 512).
Both commands exit 0 unless their input is missing or broken.

positional arguments:
  {coverage,match}

optional arguments:
  -h, --help        show this help message and exit
";
/// What `coverage -h` prints.
const COVERAGE_HELP: &str = "usage: lazymkv-dev exp11 coverage [-h] where [where ...]

positional arguments:
  where

optional arguments:
  -h, --help  show this help message and exit
";
/// What `match -h` prints.
const MATCH_HELP: &str = "usage: lazymkv-dev exp11 match [-h] folder title label

positional arguments:
  folder
  title
  label

optional arguments:
  -h, --help  show this help message and exit
";

/// What COVERAGE counts, in the Python's order. Py: TOKENS.
pub const TOKENS: [&str; 7] = ["audio-lang", "nolang", "sub-lang", "core", "special", "sub-flagged", "audio-512"];
/// A title this long or longer gets a COVERAGE line when no title is named. Py: LONG_TITLE_S.
pub const LONG_TITLE_S: u32 = 20 * 60;
/// The line after the file's tracks when there is no all-tracks rip of the title.
const NO_ALL_TRACKS: &str = "no all-tracks rip of this title to compare with";
/// The two lines before the all-tracks rip's tracks.
const ALL_TRACKS_HEAD: [&str; 2] = [
    "all-tracks tracks of the same title; 'scan' is the stream at the same position, which is that",
    "track when compare_tracks gave the all-tracks rip count=yes and order=yes:",
];

/// How a file track or a predicted stream fared. Py: match's tags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tag {
    /// A file track that is a predicted stream.
    Predicted,
    /// A file track that is a stream the prediction drops.
    Extra,
    /// A predicted stream no file track is.
    Missing,
    /// A file track no later scan stream can be.
    Unmatched,
}

impl Tag {
    /// The Python's spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Tag::Predicted => "predicted",
            Tag::Extra => "EXTRA",
            Tag::Missing => "MISSING",
            Tag::Unmatched => "UNMATCHED",
        }
    }
}

/// One row of match: the tag, the scan stream (None for UNMATCHED), the file track (None for MISSING).
pub type Row = (Tag, Option<ExpectedRow>, Option<FileRow>);

/// How many of the title's streams test each token (a stream's language is its attribute 3), in TOKENS order. Err
/// where a flag is not a number (the Python crashed). Py: exp11_tracks.coverage.
pub fn coverage(scan: &Scan, title: u32) -> Result<Vec<(&'static str, i64)>, String> {
    let mut n = [0i64; 7];
    for ((t, _), a) in &scan.streams {
        if *t != title {
            continue;
        }
        let kind = attr_or(a, 1, "");
        let lang = attr_or(a, 3, "").to_lowercase();
        let flags = int_attr(a, 22)?;
        if kind == "Audio" {
            if lang.is_empty() || lang == "und" {
                n[1] += 1;
            } else if lang != "eng" {
                n[0] += 1;
            }
            n[3] += i64::from(flags & 256 != 0);
            n[4] += i64::from(flags & 7 != 0);
            n[6] += i64::from(flags & 512 != 0);
        } else if kind == "Subtitles" {
            n[2] += i64::from(lang != "eng");
            n[5] += i64::from(flags & 7 != 0);
        }
    }
    Ok(TOKENS.iter().copied().zip(n).collect())
}

/// Every file track, in order, takes the next scan stream it can be (type, language, audio channels, as
/// reconcile's same_track); the predicted streams it skips are MISSING, and so are those after the last match.
/// Py: exp11_tracks.match.
pub fn match_tracks(scan: &Scan, title: u32, ident: &MkvmergeIdent) -> Result<Vec<Row>, String> {
    let every = expected_rows(scan, title, Selection::All)?;
    let predicted: BTreeSet<u32> = expected_rows(scan, title, Selection::Eng)?.iter().map(|row| row.stream).collect();
    let mut rows: Vec<Row> = Vec::new();
    let mut i = 0;
    for g in file_rows(ident)? {
        let Some(j) = (i..every.len()).find(|k| same_track(&every[*k], &g)) else {
            rows.push((Tag::Unmatched, None, Some(g)));
            continue;
        };
        for r in &every[i..j] {
            if predicted.contains(&r.stream) {
                rows.push((Tag::Missing, Some(r.clone()), None));
            }
        }
        let tag = if predicted.contains(&every[j].stream) { Tag::Predicted } else { Tag::Extra };
        rows.push((tag, Some(every[j].clone()), Some(g)));
        i = j + 1;
    }
    for r in &every[i..] {
        if predicted.contains(&r.stream) {
            rows.push((Tag::Missing, Some(r.clone()), None));
        }
    }
    Ok(rows)
}

/// Attribute `id`, or `default` when the stream or title lacks it. Py: a.get(id, default).
fn attr_or<'a>(a: &'a Attrs, id: u32, default: &'a str) -> &'a str {
    a.get(&id).map_or(default, String::as_str)
}

/// str() of an optional attribute: the value, or "None".
fn or_none(value: Option<&String>) -> &str {
    value.map_or("None", String::as_str)
}

/// repr() of an optional attribute: 'eng', or None.
fn repr_or_none(value: Option<&String>) -> String {
    value.map_or_else(|| "None".to_string(), |text| py_repr_str(text))
}

/// "audio-lang=0 nolang=0 ...". Py: " ".join(f"{k}={v}" for k, v in n.items()).
fn counts_text(n: &[(&str, i64)]) -> String {
    n.iter().map(|(token, count)| format!("{token}={count}")).collect::<Vec<_>>().join(" ")
}

/// One mkvmerge track for checking by eye. Py: exp11_tracks.describe.
fn describe(track: &MkvTrack) -> String {
    let p = &track.properties;
    let language = p.get("language").map_or_else(|| "und".to_string(), py_str);
    let channels = p.get("audio_channels").map_or_else(String::new, py_str);
    let forced = p.get("forced_track").map_or_else(|| "None".to_string(), py_str);
    let bytes = p.get("tag_number_of_bytes").map_or_else(|| "-".to_string(), py_str);
    let name = p.get("track_name").map_or_else(|| py_repr_str(""), py_repr);
    format!(
        "{:>2} {:<9} {:<4} {:<22} ch={:<2} forced={} bytes={} name={}",
        track.id, track.r#type, language, track.codec, channels, forced, bytes, name
    )
}

/// (the file on disk, the path as str(Path) shows it) of the scan `place` names: `<place>/scan.txt` when `place` is a
/// folder, else `place` itself. Py: exp11_tracks.scan_path.
fn scan_target(ctx: &Ctx, place: &str) -> (PathBuf, String) {
    let path = ctx.abs(place);
    let shown = pathlib_str(place);
    if path.is_dir() {
        (path.join("scan.txt"), pathlib_under(&shown, "scan.txt"))
    } else {
        (path, shown)
    }
}

/// A scan read as Path.read_text(encoding="utf-8-sig", errors="replace") gives it, a read error naming the path as
/// `shown`. Py: exp11_tracks.load_scan.
fn load_scan_shown(path: &Path, shown: &str) -> Result<Scan, String> {
    read_scan_file(path, shown, Encoding::Utf8Sig, Errors::Replace)
}

/// `load_scan_shown` with the path shown as given (the tests' scans are named by their full path).
#[cfg(test)]
fn load_scan(path: &Path) -> Result<Scan, String> {
    load_scan_shown(path, &path.to_string_lossy())
}

/// mkvmerge -J output, read as json.loads and then ["tracks"]: Err where the Python crashed (unreadable, not JSON,
/// no "tracks").
fn load_ident(path: &Path, shown: &str) -> Result<MkvmergeIdent, String> {
    let value: serde_json::Value = serde_json::from_str(&read_file_text(path, shown, Encoding::Utf8Sig, Errors::Strict)?)
        .map_err(|error| format!("{shown}: {error}"))?;
    if value.get("tracks").is_none() {
        return Err(format!("{shown}: KeyError: 'tracks'"));
    }
    serde_json::from_value(value).map_err(|error| format!("{shown}: {error}"))
}

/// rip-summary.json, read as json.loads.
fn load_summary(path: &Path, shown: &str) -> Result<Summary, String> {
    serde_json::from_str(&read_file_text(path, shown, Encoding::Utf8Sig, Errors::Strict)?)
        .map_err(|error| format!("{shown}: {error}"))
}

/// `exp11 coverage <where>[@<title>] ...`: Ok(0), or Ok(1) after the sys.exit line on stderr. Py: run_coverage.
fn run_coverage(ctx: &Ctx, places: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Result<i32, String> {
    for arg in places {
        let (place, title) = arg.rsplit_once('@').unwrap_or((arg.as_str(), ""));
        let (path, shown) = scan_target(ctx, place);
        if !path.is_file() {
            let _ = writeln!(out, "{place}: no scan.txt");
            continue;
        }
        let scan = load_scan_shown(&path, &shown)?;
        if title.is_empty() {
            for (t, a) in &scan.titles {
                // duration_seconds raising ValueError (None here) is "not long enough".
                let long_enough = duration_seconds(attr_or(a, 9, "0:00:00")).map_or(false, |seconds| seconds >= LONG_TITLE_S);
                if long_enough {
                    let n = coverage(&scan, *t)?;
                    let _ = writeln!(out, "COVERAGE {place} title {t} ({}): {}", or_none(a.get(&9)), counts_text(&n));
                }
            }
            continue;
        }
        let t = py_int(title).ok_or_else(|| format!("invalid literal for int() with base 10: {}", py_repr_str(title)))?;
        let Some(number) = u32::try_from(t).ok().filter(|number| scan.titles.contains_key(number)) else {
            let _ = writeln!(err, "{place}: the scan lists no title {t}");
            return Ok(1);
        };
        let kept: BTreeSet<u32> = expected_rows(&scan, number, Selection::Eng)?.iter().map(|row| row.stream).collect();
        let _ = writeln!(out, "{place} title {t} ({}), every stream:", attr_or(&scan.titles[&number], 9, "?"));
        for ((tt, s), a) in &scan.streams {
            if *tt != number {
                continue;
            }
            let flags = int_attr(a, 22)?;
            let _ = writeln!(
                out,
                "  stream {:>2} {:<9} attr3={:<7} attr28={:<7} {:<8} ch={:<2} flags={:<5} predicted: {}",
                s,
                attr_or(a, 1, ""),
                repr_or_none(a.get(&3)),
                repr_or_none(a.get(&28)),
                attr_or(a, 6, ""),
                attr_or(a, 14, ""),
                flags,
                if kept.contains(s) { "keep" } else { "drop" }
            );
        }
        let n = coverage(&scan, number)?;
        let _ = writeln!(out, "COVERAGE {place} title {t}: {}", counts_text(&n));
    }
    Ok(0)
}

/// `exp11 match <folder> <title> <label>`: Ok(0), or Ok(1) after the sys.exit line on stderr; the lines are written
/// as they are made, so an Err (the Python's crash) comes after the lines before it. Py: run_match.
fn run_match(ctx: &Ctx, folder: &str, title: i64, label: &str, out: &mut dyn Write, err: &mut dyn Write) -> Result<i32, String> {
    let d = ctx.abs(folder);
    let (scan_file, scan_shown) = scan_target(ctx, folder);
    let scan = load_scan_shown(&scan_file, &scan_shown)?;
    let Some(number) = u32::try_from(title).ok().filter(|number| scan.titles.contains_key(number)) else {
        let _ = writeln!(err, "{} lists no title {title}", pathlib_under(&pathlib_str(folder), "scan.txt"));
        return Ok(1);
    };
    let ident_shown = pathlib_str(&format!("{folder}/{label}/mkvmerge.json"));
    let ident = load_ident(&d.join(label).join("mkvmerge.json"), &ident_shown)?;
    let every = expected_rows(&scan, number, Selection::All)?;
    let rows = match_tracks(&scan, number, &ident)?;
    let predicted = expected_rows(&scan, number, Selection::Eng)?.len();
    let _ = writeln!(
        out,
        "title {title}: {} streams without 3D video, {predicted} predicted; {} tracks in {label}/mkvmerge.json",
        every.len(),
        ident.tracks.len()
    );
    for (tag, r, g) in &rows {
        let line = match (r, g) {
            (Some(r), g) => {
                // f"{None:<9}" raised TypeError in the Python: a stream without attribute 1.
                let kind = r.r#type.as_deref().ok_or_else(|| "unsupported format string passed to NoneType.__format__".to_string())?;
                let mut line = format!("scan {:>2} {:<9} {:<4} {:<10} ch={:<2} f={}", r.stream, kind, r.lang_raw, r.codec, r.ch, r.flags);
                if let Some(g) = g {
                    line.push_str(&format!("  <- file {:>2} {}", g.id, g.codec));
                }
                line
            }
            (None, Some(g)) => format!("file {:>2} {:<9} {:<4} {} ch={}", g.id, g.r#type, g.lang_raw, g.codec, py_str(&g.ch)),
            (None, None) => continue,
        };
        let _ = writeln!(out, "{:<10} {line}", tag.as_str());
    }
    let count = |wanted: Tag| rows.iter().filter(|(tag, _, _)| *tag == wanted).count();
    let _ = writeln!(
        out,
        "DIFFERENCES missing={} extra={} unmatched={}",
        count(Tag::Missing),
        count(Tag::Extra),
        count(Tag::Unmatched)
    );
    let _ = writeln!(out);
    let _ = writeln!(out, "{label} tracks, for checking the lines above by eye:");
    for track in &ident.tracks {
        let _ = writeln!(out, "  {}", describe(track));
    }
    let base = d.join("all-tracks").join("mkvmerge.json");
    let summary = d.join("all-tracks").join("rip-summary.json");
    let all_tracks = pathlib_str(&format!("{folder}/all-tracks"));
    let summary_shown = pathlib_under(&all_tracks, "rip-summary.json");
    if !(base.is_file() && summary.is_file() && load_summary(&summary, &summary_shown)?.title == Some(title)) {
        let _ = writeln!(out, "{NO_ALL_TRACKS}");
        return Ok(0);
    }
    let tracks = load_ident(&base, &pathlib_under(&all_tracks, "mkvmerge.json"))?.tracks;
    for line in ALL_TRACKS_HEAD {
        let _ = writeln!(out, "{line}");
    }
    for (k, track) in tracks.iter().enumerate() {
        let stream = if tracks.len() == every.len() { every[k].stream.to_string() } else { "?".to_string() };
        let _ = writeln!(out, "  scan {stream:>2}  {}", describe(track));
    }
    Ok(0)
}

/// `lazymkv-dev exp11 coverage <where>... | match <folder> <title> <label>`. Py: exp11_tracks.main and its argparse.
pub fn run(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let Some(command) = args.first() else {
        return usage_error(err, NAME, USAGE, "the following arguments are required: command");
    };
    let rest = &args[1..];
    let help = rest.iter().take_while(|word| word.as_str() != "--").any(|word| word == "-h" || word == "--help");
    let result = match command.as_str() {
        "-h" | "--help" => {
            let _ = write!(out, "{HELP}");
            return 0;
        }
        "coverage" if help => {
            let _ = write!(out, "{COVERAGE_HELP}");
            return 0;
        }
        "match" if help => {
            let _ = write!(out, "{MATCH_HELP}");
            return 0;
        }
        "coverage" => {
            let places = match Args::new(rest, &[]).positionals() {
                Ok(words) => words,
                Err(message) => return usage_error(err, NAME, USAGE, &message),
            };
            if places.is_empty() {
                return usage_error(err, "exp11 coverage", COVERAGE_USAGE, "the following arguments are required: where");
            }
            run_coverage(ctx, &places, out, err)
        }
        "match" => {
            let words = match Args::new(rest, &[]).positionals() {
                Ok(words) => words,
                Err(message) => return usage_error(err, NAME, USAGE, &message),
            };
            // argparse converts the title as it takes it, so a bad title is reported before a missing argument.
            let title = match words.get(1) {
                None => 0,
                Some(word) => match py_int(word) {
                    Some(title) => title,
                    None => {
                        let message = format!("argument title: invalid int value: {}", py_repr_str(word));
                        return usage_error(err, "exp11 match", MATCH_USAGE, &message);
                    }
                },
            };
            if words.len() < MATCH_POSITIONALS.len() {
                let message = format!("the following arguments are required: {}", MATCH_POSITIONALS[words.len()..].join(", "));
                return usage_error(err, "exp11 match", MATCH_USAGE, &message);
            }
            if words.len() > MATCH_POSITIONALS.len() {
                let message = format!("unrecognized arguments: {}", words[MATCH_POSITIONALS.len()..].join(" "));
                return usage_error(err, NAME, USAGE, &message);
            }
            run_match(ctx, &words[0], title, &words[2], out, err)
        }
        other => {
            let message = format!("argument command: invalid choice: {} (choose from 'coverage', 'match')", py_repr_str(other));
            return usage_error(err, NAME, USAGE, &message);
        }
    };
    match result {
        Ok(code) => code,
        Err(message) => {
            let _ = writeln!(err, "lazymkv-dev {NAME}: error: {message}");
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{coverage, load_scan, match_tracks, run, Row};
    use crate::evidence::Ctx;
    use crate::testutil::{printed, Scratch as TempDir, ScratchExt};
    use lazymkv_core::recording::files::MkvmergeIdent;
    use serde_json::json;
    use std::path::{Path, PathBuf};

    /// A pinned public scan under corpus/thediscdb/movie.
    fn movie(parts: &[&str]) -> PathBuf {
        let mut path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/thediscdb/movie");
        for part in parts {
            path.push(part);
        }
        path
    }

    /// Py: JOHN_WICK.
    fn john_wick() -> PathBuf {
        movie(&["John Wick (2014)", "4k-ultra-hd--blu-ray", "disc01.txt"])
    }

    /// Py: KNOWING.
    fn knowing() -> PathBuf {
        movie(&["Knowing (2009)", "2010-dvd", "disc01.txt"])
    }

    /// A stand-in for `mkvmerge -J`: tracks are (type, language, audio channels or None). Py: ident.
    fn ident(tracks: &[(&str, &str, Option<i64>)]) -> MkvmergeIdent {
        let tracks: Vec<serde_json::Value> = tracks
            .iter()
            .enumerate()
            .map(|(id, (kind, lang, ch))| {
                let mut properties = serde_json::Map::new();
                properties.insert("language".to_string(), json!(lang));
                if let Some(ch) = ch {
                    properties.insert("audio_channels".to_string(), json!(ch));
                }
                json!({"id": id, "type": kind, "codec": "x", "properties": properties})
            })
            .collect();
        serde_json::from_value(json!({ "tracks": tracks })).unwrap()
    }

    /// (tag, scan stream) of every row. Py: tags.
    fn tags(rows: &[Row]) -> Vec<(&'static str, Option<u32>)> {
        rows.iter().map(|(tag, r, _)| (tag.as_str(), r.as_ref().map(|r| r.stream))).collect()
    }

    /// The made-up recording folder `m`: title 0 (1:30:00) with video, two English audio streams (the second a core,
    /// flag 256) and English and French subtitles; title 1 (0:05:00) with no stream. eng-only kept video, the
    /// TrueHD stream and the French subtitles; all-tracks is a rip of title 0 with every stream.
    fn write_m(tmp: &TempDir) {
        tmp.write(
            "m/scan.txt",
            "TINFO:0,9,0,\"1:30:00\"\nTINFO:1,9,0,\"0:05:00\"\n\
             SINFO:0,0,1,6201,\"Video\"\nSINFO:0,0,6,0,\"MpegH\"\nSINFO:0,0,28,0,\"eng\"\n\
             SINFO:0,1,1,6202,\"Audio\"\nSINFO:0,1,3,0,\"eng\"\nSINFO:0,1,6,0,\"TrueHD\"\nSINFO:0,1,14,0,\"8\"\n\
             SINFO:0,2,1,6202,\"Audio\"\nSINFO:0,2,3,0,\"eng\"\nSINFO:0,2,6,0,\"DD\"\nSINFO:0,2,14,0,\"2\"\nSINFO:0,2,22,0,\"256\"\n\
             SINFO:0,3,1,6203,\"Subtitles\"\nSINFO:0,3,3,0,\"eng\"\nSINFO:0,3,6,0,\"PGS\"\n\
             SINFO:0,4,1,6203,\"Subtitles\"\nSINFO:0,4,3,0,\"fra\"\nSINFO:0,4,6,0,\"PGS\"\n",
        );
        tmp.write(
            "m/eng-only/mkvmerge.json",
            r#"{"tracks": [
 {"id": 0, "type": "video", "codec": "HEVC/H.265/MPEG-H", "properties": {"language": "und", "forced_track": false}},
 {"id": 1, "type": "audio", "codec": "TrueHD Atmos", "properties": {"language": "eng", "audio_channels": 8, "forced_track": false, "tag_number_of_bytes": "1234", "track_name": "Main mix"}},
 {"id": 2, "type": "subtitles", "codec": "HDMV PGS", "properties": {"language": "fra"}}
]}
"#,
        );
        tmp.write(
            "m/all-tracks/mkvmerge.json",
            r#"{"tracks": [
 {"id": 0, "type": "video", "codec": "HEVC/H.265/MPEG-H", "properties": {"language": "und"}},
 {"id": 1, "type": "audio", "codec": "TrueHD Atmos", "properties": {"language": "eng", "audio_channels": 8}},
 {"id": 2, "type": "audio", "codec": "AC-3", "properties": {"language": "eng", "audio_channels": 2}},
 {"id": 3, "type": "subtitles", "codec": "HDMV PGS", "properties": {"language": "eng"}},
 {"id": 4, "type": "subtitles", "codec": "HDMV PGS", "properties": {"language": "fre", "track_name": "it's"}}
]}
"#,
        );
        tmp.write("m/all-tracks/rip-summary.json", "{\"ok\": true, \"title\": 0}\n");
    }

    /// The command with `cwd` as the current directory: (exit code, stdout, stderr).
    fn run_in(cwd: &Path, args: &[&str]) -> (i32, String, String) {
        let ctx = Ctx::for_test(cwd);
        let args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run(&ctx, &args, &mut out, &mut err);
        (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
    }

    #[test]
    fn coverage_a_uhd_with_only_english_audio_tests_neither_special_nor_nolang() {
        let n = coverage(&load_scan(&john_wick()).unwrap(), 0).unwrap();
        assert_eq!(
            n,
            vec![("audio-lang", 0), ("nolang", 0), ("sub-lang", 2), ("core", 1), ("special", 0), ("sub-flagged", 0), ("audio-512", 0)]
        );
    }

    #[test]
    fn coverage_a_dvd_with_a_flagged_commentary_tests_special() {
        let n = coverage(&load_scan(&knowing()).unwrap(), 0).unwrap();
        assert_eq!(
            n,
            vec![("audio-lang", 1), ("nolang", 0), ("sub-lang", 2), ("core", 0), ("special", 1), ("sub-flagged", 2), ("audio-512", 0)]
        );
    }

    // Knowing, title 0: 0 video; audio 1 eng 6ch, 2 spa 6ch, 3 eng 2ch flag 1; subtitles 4 eng, 5 spa, 6 eng flag 1,
    // 7 spa flag 1. The eng prediction keeps 0, 1, 4 and 6. Py: Match.

    #[test]
    fn match_a_file_as_predicted_has_no_difference() {
        let scan = load_scan(&knowing()).unwrap();
        let file = ident(&[("video", "und", None), ("audio", "eng", Some(6)), ("subtitles", "eng", None), ("subtitles", "eng", None)]);
        let rows = match_tracks(&scan, 0, &file).unwrap();
        assert_eq!(tags(&rows), vec![("predicted", Some(0)), ("predicted", Some(1)), ("predicted", Some(4)), ("predicted", Some(6))]);
    }

    #[test]
    fn match_a_kept_commentary_and_a_dropped_subtitle_are_named() {
        let scan = load_scan(&knowing()).unwrap();
        let file = ident(&[("video", "und", None), ("audio", "eng", Some(6)), ("audio", "eng", Some(2)), ("subtitles", "eng", None)]);
        let rows = match_tracks(&scan, 0, &file).unwrap();
        assert_eq!(
            tags(&rows),
            vec![("predicted", Some(0)), ("predicted", Some(1)), ("EXTRA", Some(3)), ("predicted", Some(4)), ("MISSING", Some(6))]
        );
    }

    #[test]
    fn match_a_track_out_of_scan_order_is_unmatched() {
        let scan = load_scan(&knowing()).unwrap();
        let file = ident(&[("video", "und", None), ("subtitles", "eng", None), ("audio", "eng", Some(6))]);
        let rows = match_tracks(&scan, 0, &file).unwrap();
        assert_eq!(
            tags(&rows),
            vec![("predicted", Some(0)), ("MISSING", Some(1)), ("predicted", Some(4)), ("UNMATCHED", None), ("MISSING", Some(6))]
        );
    }

    #[test]
    fn exp11_match_prints_the_pythons_lines() {
        let tmp = TempDir::new("match");
        write_m(&tmp);
        let (code, out, err) = run_in(&tmp.0, &["match", "m", "0", "eng-only"]);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(
            out,
            printed(&[
                "title 0: 5 streams without 3D video, 3 predicted; 3 tracks in eng-only/mkvmerge.json",
                "predicted  scan  0 video     eng  MpegH      ch=   f=0  <- file  0 HEVC/H.265/MPEG-H",
                "predicted  scan  1 audio     eng  TrueHD     ch=8  f=0  <- file  1 TrueHD Atmos",
                "MISSING    scan  3 subtitles eng  PGS        ch=   f=0",
                "EXTRA      scan  4 subtitles fra  PGS        ch=   f=0  <- file  2 HDMV PGS",
                "DIFFERENCES missing=1 extra=1 unmatched=0",
                "",
                "eng-only tracks, for checking the lines above by eye:",
                "   0 video     und  HEVC/H.265/MPEG-H      ch=   forced=False bytes=- name=''",
                "   1 audio     eng  TrueHD Atmos           ch=8  forced=False bytes=1234 name='Main mix'",
                "   2 subtitles fra  HDMV PGS               ch=   forced=None bytes=- name=''",
                "all-tracks tracks of the same title; 'scan' is the stream at the same position, which is that",
                "track when compare_tracks gave the all-tracks rip count=yes and order=yes:",
                "  scan  0   0 video     und  HEVC/H.265/MPEG-H      ch=   forced=None bytes=- name=''",
                "  scan  1   1 audio     eng  TrueHD Atmos           ch=8  forced=None bytes=- name=''",
                "  scan  2   2 audio     eng  AC-3                   ch=2  forced=None bytes=- name=''",
                "  scan  3   3 subtitles eng  HDMV PGS               ch=   forced=None bytes=- name=''",
                "  scan  4   4 subtitles fre  HDMV PGS               ch=   forced=None bytes=- name=\"it's\"",
            ])
        );
    }

    #[test]
    fn exp11_coverage_prints_the_pythons_lines() {
        let tmp = TempDir::new("coverage");
        write_m(&tmp);
        let (code, out, err) = run_in(&tmp.0, &["coverage", "m@0", "m", "nowhere", "m/scan.txt@1", "m@"]);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(
            out,
            printed(&[
                "m title 0 (1:30:00), every stream:",
                "  stream  0 Video     attr3=None    attr28='eng'   MpegH    ch=   flags=0     predicted: keep",
                "  stream  1 Audio     attr3='eng'   attr28=None    TrueHD   ch=8  flags=0     predicted: keep",
                "  stream  2 Audio     attr3='eng'   attr28=None    DD       ch=2  flags=256   predicted: drop",
                "  stream  3 Subtitles attr3='eng'   attr28=None    PGS      ch=   flags=0     predicted: keep",
                "  stream  4 Subtitles attr3='fra'   attr28=None    PGS      ch=   flags=0     predicted: drop",
                "COVERAGE m title 0: audio-lang=0 nolang=0 sub-lang=1 core=1 special=0 sub-flagged=0 audio-512=0",
                "COVERAGE m title 0 (1:30:00): audio-lang=0 nolang=0 sub-lang=1 core=1 special=0 sub-flagged=0 audio-512=0",
                "nowhere: no scan.txt",
                "m/scan.txt title 1 (0:05:00), every stream:",
                "COVERAGE m/scan.txt title 1: audio-lang=0 nolang=0 sub-lang=0 core=0 special=0 sub-flagged=0 audio-512=0",
                "COVERAGE m title 0 (1:30:00): audio-lang=0 nolang=0 sub-lang=1 core=1 special=0 sub-flagged=0 audio-512=0",
            ])
        );
    }

    #[test]
    fn exp11_a_title_the_scan_does_not_list_exits_1() {
        let tmp = TempDir::new("unlisted");
        write_m(&tmp);
        // The Python's sys.exit(message): stderr, exit 1, nothing after it (the second argument is not read).
        assert_eq!(run_in(&tmp.0, &["coverage", "m@7", "m"]), (1, String::new(), "m: the scan lists no title 7\n".to_string()));
        // str(Path("./m/") / "scan.txt") is "m/scan.txt"; the title is checked before mkvmerge.json is read.
        assert_eq!(run_in(&tmp.0, &["match", "./m/", "3", "eng-only"]), (1, String::new(), "m/scan.txt lists no title 3\n".to_string()));
        assert_eq!(run_in(&tmp.0, &["match", "m", "-1", "no-such-label"]), (1, String::new(), "m/scan.txt lists no title -1\n".to_string()));
        // Where the Python crashed: a title word that is not a number, a label without mkvmerge.json.
        assert_eq!(run_in(&tmp.0, &["coverage", "m@x"]).0, 2);
        assert_eq!(run_in(&tmp.0, &["match", "m", "0", "no-such-label"]).0, 2);
    }

    #[test]
    fn exp11_reads_its_arguments_as_argparse_did() {
        let tmp = TempDir::new("args");
        let top = "usage: lazymkv-dev exp11 [-h] {coverage,match} ...\n";
        let cases: [(&[&str], String); 6] = [
            (&[], format!("{top}lazymkv-dev exp11: error: the following arguments are required: command\n")),
            (
                &["bogus"],
                format!("{top}lazymkv-dev exp11: error: argument command: invalid choice: 'bogus' (choose from 'coverage', 'match')\n"),
            ),
            (
                &["coverage"],
                "usage: lazymkv-dev exp11 coverage [-h] where [where ...]\n\
                 lazymkv-dev exp11 coverage: error: the following arguments are required: where\n"
                    .to_string(),
            ),
            (
                &["match", "a"],
                "usage: lazymkv-dev exp11 match [-h] folder title label\n\
                 lazymkv-dev exp11 match: error: the following arguments are required: title, label\n"
                    .to_string(),
            ),
            (
                &["match", "a", "x", "b"],
                "usage: lazymkv-dev exp11 match [-h] folder title label\n\
                 lazymkv-dev exp11 match: error: argument title: invalid int value: 'x'\n"
                    .to_string(),
            ),
            (&["match", "a", "1", "b", "c"], format!("{top}lazymkv-dev exp11: error: unrecognized arguments: c\n")),
        ];
        for (args, stderr) in cases {
            assert_eq!(run_in(&tmp.0, args), (2, String::new(), stderr), "{args:?}");
        }
        let (code, out, err) = run_in(&tmp.0, &["-h"]);
        assert_eq!((code, err.as_str()), (0, ""));
        assert!(out.starts_with(&format!("{top}\nExperiment 11 (spec section 17, row 11)")), "{out}");
        let (code, out, _) = run_in(&tmp.0, &["match", "a", "-h"]);
        assert_eq!(code, 0);
        assert!(out.starts_with("usage: lazymkv-dev exp11 match [-h] folder title label\n"), "{out}");
    }

    #[test]
    fn exp11_names_a_missing_file_as_python_does() {
        let tmp = TempDir::new("missing-file");
        write_m(&tmp);
        // The path as str(Path) shows it: the scan's folder missing, then the label's mkvmerge.json missing.
        let (code, out, err) = run_in(&tmp.0, &["match", "./nope//", "0", "x"]);
        assert_eq!((code, out.as_str()), (2, ""));
        assert_eq!(err, "lazymkv-dev exp11: error: [Errno 2] No such file or directory: 'nope'\n");
        let (code, out, err) = run_in(&tmp.0, &["match", "./m//", "0", "./no-such-label/"]);
        assert_eq!((code, out.as_str()), (2, ""));
        assert_eq!(err, "lazymkv-dev exp11: error: [Errno 2] No such file or directory: 'm/no-such-label/mkvmerge.json'\n");
    }
}
