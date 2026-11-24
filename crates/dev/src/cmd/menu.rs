//! `lazymkv-dev menu show <map> | check <map> --scan <scan> [--evidence DIR]`: menu maps at the command line (menu maps
//! design 5.4; the format and rulings M3-1 to M3-8 are docs/menu-maps/format.md). `show` prints a map, labels as they
//! are; `check` ties it to a scanned disc and prints ids, playlists and numbers only, never a label. Exit: show 0, or
//! 2; check 0 when nothing failed, 1 otherwise; 2 for an argument, map or scan problem; 3 for a slug without the
//! evidence clone. Refusals print `lazymkv-dev menu: error: <line>`, one per problem. The command changes nothing;
//! `decide --menu` reads a map with `read_map`, the off-decisions beside it with `read_off` and the recording's
//! playlists with `read_playlists`, and its decision drops a commentary or audio description the map confirms
//! (design 5.4a).

use crate::evidence::{read_file_text, resolve_recording, Ctx, Recording, Refused};
use crate::{usage_error, Args};
use lazymkv_core::disc::{disc, Disc};
use lazymkv_core::menu::{
    self, parse_off, parse_playlist, DiscId, Effect, MenuMap, OffDecisions, Playlist, FILE_NAME, META_WITHOUT_ID,
    NO_ID_IN_SCAN_FILE, NO_META, OFF_FILE_NAME,
};
use lazymkv_core::recording::files::Meta;
use lazymkv_core::recording::{py_float_repr, py_repr_str, read_json, read_text, text_mode, Encoding, Errors, Tree};
use lazymkv_core::robot::parse;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::PathBuf;

/// The command's name as typed.
const NAME: &str = "menu";
/// argparse's usage lines, under the command's name.
const USAGE: &str = "menu [-h] {show,check} ...";
const SHOW_USAGE: &str = "menu show [-h] map";
const CHECK_USAGE: &str = "menu check [-h] map --scan SCAN [--evidence DIR]";
/// A recording folder's files this command reads.
const SCAN_FILE: &str = "scan.txt";
const META_FILE: &str = "meta.json";
/// Why `read_playlists` read no file for a name: it is not five ASCII digits (MR-13), or the recording has no such
/// playlist file.
const NOT_A_PLAYLIST_NAME: &str = "not a playlist name";
const NO_SUCH_PLAYLIST: &str = "no such file";

/// What -h prints after the usage line.
const HELP: &str = "
Menu maps (menu maps design 5.4): what a disc's menu calls each choice, and the playlist and streams it selects.

  show <map>                 print a map, one line for each part of it, labels as they are
  check <map> --scan SCAN    check a map against a scanned disc; prints ids, playlists and numbers, never a label

positional arguments:
  {show,check}

optional arguments:
  -h, --help  show this help message and exit
";
/// What `show -h` prints after the usage line.
const SHOW_HELP: &str = "
positional arguments:
  map         a map folder (its menu-map.json is read) or a map file

optional arguments:
  -h, --help  show this help message and exit

exit status: 0 when the map is printed; 2 for an argument error, or a map that cannot be read or does not pass the
format's rules (every problem is printed on its own line).
";
/// What `check -h` prints after the usage line.
const CHECK_HELP: &str = "
positional arguments:
  map             a map folder (its menu-map.json is read) or a map file

optional arguments:
  -h, --help      show this help message and exit
  --scan SCAN     the scanned disc: a recording folder (its scan.txt and meta.json), a slug under the evidence clone,
                  or a bare scan file (which carries no disc ID, so the map's is not checked)
  --evidence DIR  the evidence clone (default: LAZYMKV_EVIDENCE, else corpus/owner)

exit status: 0 when nothing failed; 1 when an audio or shown subtitle number is outside its scan title's streams, or
the map is of another disc (a playlist the scan does not list is reported, not failed); 2 for an argument error, or a
map or scan that cannot be used; 3 when a slug is given and the evidence clone is absent.
";

/// A refusal with exit code 2.
fn refused(line: String) -> Refused {
    Refused { code: 2, line }
}

/// The map at `given`: the file `<given>/menu-map.json` when `given` is a folder, else the file itself; the text of the
/// map's path as it is shown (the path as given; for a folder, the path as given without its trailing separators,
/// `/` and the file name, `/` on every system as the crate's other shown paths, or the file name alone for an empty
/// path, the working folder) and the map. The file is UTF-8 with an optional byte-order mark, read strictly. Every
/// problem of `menu::read` becomes one line `<shown>: <problem>` in the refusal (code 2), the lines joined with '\n'.
pub(crate) fn read_map(ctx: &Ctx, given: &str) -> Result<(String, MenuMap), Refused> {
    let path = ctx.abs(given);
    let (path, shown) = if path.is_dir() {
        let folder = given.trim_end_matches(['/', '\\']);
        // An empty folder is the working folder, shown as the file name alone; a folder of separators alone is a root.
        let shown = if given.is_empty() { FILE_NAME.to_string() } else { format!("{folder}/{FILE_NAME}") };
        (path.join(FILE_NAME), shown)
    } else {
        (path, given.to_string())
    };
    let text = read_file_text(&path, &shown, Encoding::Utf8Sig, Errors::Strict)
        .map_err(|problem| refused(format!("{shown}: {problem}")))?;
    match menu::read(&text) {
        Ok(map) => Ok((shown, map)),
        Err(problems) => {
            let lines: Vec<String> = problems.iter().map(|problem| format!("{shown}: {problem}")).collect();
            Err(refused(lines.join("\n")))
        }
    }
}

/// The off-decisions beside the map `given` names (`<folder>/menu-off.tsv`; for a map file, its folder's): none
/// when the file is absent; Refused { code: 2 } with one line `<shown>: <problem>` per problem otherwise (MR-6). The
/// file's shown path is built as `read_map` builds the map's, with `menu-off.tsv` as the file name (for a map file,
/// the given path's folder part, or the file name alone when it has none), and the file is read as the map is:
/// UTF-8 with an optional byte-order mark, strictly (`read_file_text` with `Utf8Sig`), then `menu::parse_off`.
pub(crate) fn read_off(ctx: &Ctx, given: &str) -> Result<OffDecisions, Refused> {
    let path = ctx.abs(given);
    let (path, shown) = if path.is_dir() {
        let folder = given.trim_end_matches(['/', '\\']);
        let shown = if given.is_empty() { OFF_FILE_NAME.to_string() } else { format!("{folder}/{OFF_FILE_NAME}") };
        (path.join(OFF_FILE_NAME), shown)
    } else {
        let shown = match given.rfind(['/', '\\']) {
            Some(at) => format!("{}/{OFF_FILE_NAME}", &given[..at]),
            None => OFF_FILE_NAME.to_string(),
        };
        (path.parent().map_or_else(|| PathBuf::from(OFF_FILE_NAME), |folder| folder.join(OFF_FILE_NAME)), shown)
    };
    if !path.exists() {
        return Ok(OffDecisions::default());
    }
    let text = read_file_text(&path, &shown, Encoding::Utf8Sig, Errors::Strict)
        .map_err(|problem| refused(format!("{shown}: {problem}")))?;
    parse_off(&text).map_err(|problems| {
        let lines: Vec<String> = problems.iter().map(|problem| format!("{shown}: {problem}")).collect();
        refused(lines.join("\n"))
    })
}

/// Each named playlist of the recording, `nav/BDMV/PLAYLIST/<name>.mpls`: parsed, or why not (`not a playlist
/// name` for a name that is not five ASCII digits, which is not looked up (MR-13); `no such file`; the read error;
/// `parse_playlist`'s sentence). Every name requested has an entry, keyed as requested.
pub(crate) fn read_playlists(rec: &Recording, names: &BTreeSet<String>) -> BTreeMap<String, Result<Playlist, String>> {
    let folder = rec.dir.join("nav").join("BDMV").join("PLAYLIST");
    let read = |name: &str| -> Result<Playlist, String> {
        // MR-13: a map's playlist string reaches the file system only as five ASCII digits.
        if name.len() != 5 || !name.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(NOT_A_PLAYLIST_NAME.to_string());
        }
        let key = folder.join(&format!("{name}.mpls"));
        if !rec.tree.is_file(&key) {
            return Err(NO_SUCH_PLAYLIST.to_string());
        }
        let bytes = rec.tree.read(&key).map_err(|error| error.to_string())?;
        parse_playlist(&bytes)
    };
    names.iter().map(|name| (name.clone(), read(name))).collect()
}

/// A recording's disc ID, its meta.json `disc_id` (M3-6): `Absent(NO_META)` when the recording has no meta.json,
/// `Absent(META_WITHOUT_ID)` when `disc_id` is missing, null or empty (surrounding white space left off), else
/// `Known`. A meta.json that cannot be read or decoded is Refused { code: 2, "<path>: <problem>" }.
pub(crate) fn disc_id_of(rec: &Recording) -> Result<DiscId, Refused> {
    let key = rec.dir.join(META_FILE);
    if !rec.tree.is_file(&key) {
        return Ok(DiscId::Absent(NO_META.to_string()));
    }
    let meta: Meta =
        read_json(&rec.tree, &key).map_err(|problem| refused(format!("{}: {problem}", rec.tree.display(&key))))?;
    Ok(match meta.disc_id.as_deref().map(str::trim).filter(|id| !id.is_empty()) {
        Some(id) => DiscId::Known(id.to_string()),
        None => DiscId::Absent(META_WITHOUT_ID.to_string()),
    })
}

/// The typed disc of a scan's text (spec 9.4), read through text mode; `disc::disc`'s refusal is prefixed by `shown`,
/// the sentence as `menu::line_text` shows it (it may quote a scan's text, such as attribute 19's, which a scan line
/// can carry with a character that breaks a printed line). A disc with no title is refused too, `<shown>: the scan lists no title`: the robot reader skips every line it does
/// not know, so a file that is not a scan, or an empty scan.txt, would otherwise pass with nothing checked.
fn typed_disc(text: &str, shown: &str) -> Result<Disc, Refused> {
    let refusal = |sentence: String| refused(format!("{shown}: {}", menu::line_text(&sentence)));
    let typed = disc(&parse(&text_mode(text))).map_err(refusal)?;
    if typed.titles.is_empty() {
        return Err(refused(format!("{shown}: the scan lists no title")));
    }
    Ok(typed)
}

/// The scanned disc `--scan` names and its ID: a bare scan file (`ctx.abs(scan).is_file()`; its ID is absent, with the
/// reason `NO_ID_IN_SCAN_FILE`), else a recording (`resolve_recording`: a folder, or a slug under the evidence clone,
/// code 3 when the clone is absent) read as its scan.txt and meta.json. No stale-copy gate: the check reads no TMDb
/// copy.
fn scanned(ctx: &Ctx, scan: &str, evidence: Option<&str>) -> Result<(Disc, DiscId), Refused> {
    let file = ctx.abs(scan);
    if file.is_file() {
        let text = read_file_text(&file, scan, Encoding::Utf8, Errors::Replace).map_err(refused)?;
        return Ok((typed_disc(&text, scan)?, DiscId::Absent(NO_ID_IN_SCAN_FILE.to_string())));
    }
    let rec = resolve_recording(ctx, evidence, scan)?;
    let key = rec.dir.join(SCAN_FILE);
    let text = read_text(&rec.tree, &key, Encoding::Utf8, Errors::Replace).map_err(refused)?;
    let typed = typed_disc(&text, &rec.tree.display(&key))?;
    Ok((typed, disc_id_of(&rec)?))
}

/// Prints a refusal, one `lazymkv-dev menu: error: <line>` for each of its lines; its exit code.
fn refuse(err: &mut dyn Write, refusal: &Refused) -> i32 {
    for line in refusal.line.split('\n') {
        let _ = writeln!(err, "lazymkv-dev {NAME}: error: {line}");
    }
    refusal.code
}

/// A number of an effect: the digits, or `none`.
fn number(n: Option<i64>) -> String {
    n.map_or_else(|| "none".to_string(), |n| n.to_string())
}

/// What an entry selects: `playlist P, audio A, subtitle S shown|off` (P is `menu::playlist_text`), or `no effect
/// (<note>)` (the note as `menu::line_text` shows it).
fn effect_text(effect: Option<&Effect>, note: &str) -> String {
    match effect {
        Some(e) => format!(
            "playlist {}, audio {}, subtitle {} {}",
            menu::playlist_text(&e.playlist),
            number(e.audio),
            number(e.subtitle),
            if e.subtitle_shown { "shown" } else { "off" }
        ),
        None => format!("no effect ({})", menu::line_text(note)),
    }
}

/// The lines `menu show` prints, the map's text as it is, except that free text holding a character that breaks a
/// line (a control character, U+2028 or U+2029: the writer keeps them out of a label, not out of a section, a note, a
/// script, a `made` value or a `not_mapped` item) is quoted and escaped (`menu::line_text`), and the IDs and playlists
/// print as `menu::id_text` and `menu::playlist_text` show them, so that no text of a map can break a line or forge
/// one.
fn show_lines(map: &MenuMap) -> Vec<String> {
    let text = |value: Option<&str>, absent: &str| value.map_or_else(|| absent.to_string(), menu::line_text);
    let id = |value: Option<&str>| value.map_or_else(|| "none".to_string(), menu::id_text);
    let made = |key: &str| text(map.made.get(key).and_then(serde_json::Value::as_str), "unknown");
    let seconds = map.start.seconds_to_menu.map_or_else(|| "unknown".to_string(), py_float_repr);
    let mut lines = vec![
        format!(
            "map {}: format {}, level {}, {} choices",
            map.disc.slug,
            text(map.disc.format.as_deref(), "unknown"),
            menu::map_level(map).word(),
            map.choices.len()
        ),
        format!("made by {}, checked by {}, {}", made("by"), made("checked_by"), made("date")),
        format!("disc_id_sha1 {}", id(map.disc.disc_id_sha1.as_deref())),
        format!("listing_sig {}", id(map.disc.listing_sig.as_deref())),
        format!(
            "start: {}, {} passed, {} s to the menu",
            text(map.start.script.as_deref(), "none"),
            map.start.passed.len(),
            seconds
        ),
        format!(
            "play: {} -> {} [{}]",
            menu::line_text(&map.play.label),
            effect_text(map.play.effect.as_ref(), &map.play.check.note),
            map.play.check.level.word()
        ),
    ];
    for choice in &map.choices {
        let same = if choice.differs_from_play == Some(false) { ", same as play" } else { "" };
        lines.push(format!(
            "{} {} {} {} {} -> {}{} [{}]",
            choice.id,
            choice.kind.word(),
            choice.surface.word(),
            choice.applies.word(),
            menu::line_text(&choice.shown_label()),
            effect_text(choice.effect.as_ref(), &choice.check.note),
            same,
            choice.check.level.word()
        ));
    }
    lines.extend(map.not_mapped.iter().map(|item| format!("not mapped: {}", menu::line_text(item))));
    lines.push(format!("quirks: {}", map.quirks.len()));
    lines
}

/// `menu show <map>`: the map's lines on `out`, exit 0; a refusal as `refuse` prints it.
fn run_show(ctx: &Ctx, rest: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let words = match Args::new(rest, &[]).positionals() {
        Ok(words) => words,
        Err(message) => return usage_error(err, NAME, USAGE, &message),
    };
    let given = match words.as_slice() {
        [] => return usage_error(err, "menu show", SHOW_USAGE, "the following arguments are required: map"),
        [given] => given,
        [_, extra @ ..] => {
            return usage_error(err, NAME, USAGE, &format!("unrecognized arguments: {}", extra.join(" ")));
        }
    };
    match read_map(ctx, given) {
        Ok((_, map)) => {
            for line in show_lines(&map) {
                let _ = writeln!(out, "{line}");
            }
            0
        }
        Err(refusal) => refuse(err, &refusal),
    }
}

/// `menu check <map> --scan <scan> [--evidence DIR]`: the check's lines on `out`; exit 0 when nothing failed, else 1.
fn run_check(ctx: &Ctx, rest: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let mut parsed = Args::new(rest, &["--scan", "--evidence"]);
    let (scan, evidence) = match (parsed.flag("--scan"), parsed.flag("--evidence")) {
        (Ok(scan), Ok(evidence)) => (scan, evidence),
        (Err(message), _) | (_, Err(message)) => return usage_error(err, "menu check", CHECK_USAGE, &message),
    };
    let words = match parsed.positionals() {
        Ok(words) => words,
        Err(message) => return usage_error(err, NAME, USAGE, &message),
    };
    let mut missing = Vec::new();
    if words.is_empty() {
        missing.push("map");
    }
    if scan.is_none() {
        missing.push("--scan");
    }
    if !missing.is_empty() {
        let message = format!("the following arguments are required: {}", missing.join(", "));
        return usage_error(err, "menu check", CHECK_USAGE, &message);
    }
    if words.len() > 1 {
        return usage_error(err, NAME, USAGE, &format!("unrecognized arguments: {}", words[1..].join(" ")));
    }
    let scan = scan.unwrap_or_default();
    let found = read_map(ctx, &words[0])
        .and_then(|(_, map)| scanned(ctx, &scan, evidence.as_deref()).map(|scanned| (map, scanned)));
    let (map, (disc, disc_id)) = match found {
        Ok(found) => found,
        Err(refusal) => return refuse(err, &refusal),
    };
    let report = menu::check(&map, &disc, menu::identity(&map, &disc_id));
    for line in &report.lines {
        let _ = writeln!(out, "{line}");
    }
    i32::from(report.failed != 0)
}

/// `lazymkv-dev menu`: `show <map>` and `check <map> --scan <scan> [--evidence DIR]`. `-h` before or after a verb
/// prints that usage and exits 0; an unknown verb is argparse's invalid-choice line, exit 2.
pub fn run(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let Some(command) = args.first() else {
        return usage_error(err, NAME, USAGE, "the following arguments are required: command");
    };
    let rest = &args[1..];
    let help = rest.iter().take_while(|word| word.as_str() != "--").any(|word| word == "-h" || word == "--help");
    let (usage, text) = match command.as_str() {
        "-h" | "--help" => (USAGE, HELP),
        "show" if help => (SHOW_USAGE, SHOW_HELP),
        "check" if help => (CHECK_USAGE, CHECK_HELP),
        "show" => return run_show(ctx, rest, out, err),
        "check" => return run_check(ctx, rest, out, err),
        other => {
            let message =
                format!("argument command: invalid choice: {} (choose from 'show', 'check')", py_repr_str(other));
            return usage_error(err, NAME, USAGE, &message);
        }
    };
    let _ = writeln!(out, "usage: lazymkv-dev {usage}");
    let _ = write!(out, "{text}");
    0
}

#[cfg(test)]
mod tests {
    use super::{disc_id_of, read_map, read_off, read_playlists, run};
    use crate::evidence::{resolve_recording, Ctx, Refused};
    use crate::testutil::{call, printed, Scratch, ScratchExt};
    use lazymkv_core::disc::disc;
    use lazymkv_core::menu::{
        self, made_up_playlist, parse_off, parse_playlist, DiscId, MadeUpItem, OffDecisions, FILE_NAME, FIXTURE,
        META_WITHOUT_ID, NO_META, OFF_FILE_NAME,
    };
    use std::collections::BTreeSet;
    use lazymkv_core::robot::parse;
    use lazymkv_core::testsupport::{menu_scan_text, scan_text};

    /// The made-up map's `disc.disc_id_sha1`.
    const FIXTURE_ID: &str = "0123456789ABCDEF0123456789ABCDEF01234567";
    /// The map folder and the recording folder of `world`, as the command line gives them (relative to the context).
    const MAP_DIR: &str = "maps/made-up-1";
    const REC_DIR: &str = "rec/made-up-1";

    /// A scratch folder holding the made-up map in `MAP_DIR` and the made-up scan as `REC_DIR`'s scan.txt, with a
    /// meta.json holding `disc_id` when one is given.
    fn world(tag: &str, disc_id: Option<&str>) -> Scratch {
        let tmp = Scratch::new(tag);
        tmp.write(&format!("{MAP_DIR}/{FILE_NAME}"), FIXTURE);
        tmp.write(&format!("{REC_DIR}/scan.txt"), menu_scan_text());
        if let Some(id) = disc_id {
            tmp.write(&format!("{REC_DIR}/meta.json"), format!("{{\"disc_id\": \"{id}\"}}"));
        }
        tmp
    }

    /// One run of `menu` with the arguments, in `tmp`; (exit code, stdout, stderr).
    fn menu_run(tmp: &Scratch, args: &[&str]) -> (i32, String, String) {
        call(run, &Ctx::for_test(&tmp.0), args)
    }

    /// The path of the map file as `read_map` shows it for the folder `dir`: joined with `/` on every system.
    fn shown(dir: &str) -> String {
        format!("{dir}/{FILE_NAME}")
    }

    #[test]
    fn menu_show_prints_the_made_up_map_line_by_line() {
        let tmp = world("menu-show", None);
        let (code, out, err) = menu_run(&tmp, &["show", MAP_DIR]);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(
            out,
            printed(&[
                "map made-up-1: format uhd, level read, 9 choices",
                "made by agent-explorer, checked by agent-checker, 2026-11-17",
                "disc_id_sha1 0123456789ABCDEF0123456789ABCDEF01234567",
                "listing_sig 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                "start: scripts/start.txt, 1 passed, 12.5 s to the menu",
                "play: PLAY -> playlist 00800, audio 1, subtitle 1 off [replayed]",
                "c001 audio main at-play SETUP > AUDIO: Language A 5.1 -> playlist 00800, audio 1, subtitle 1 off, same as play [replayed]",
                "c002 audio main at-play SETUP > AUDIO: Language B 5.1 -> playlist 00800, audio 2, subtitle 3 shown [replayed]",
                "c003 audio main at-play SETUP > AUDIO: COMMENTARY 1 -> playlist 00800, audio 3, subtitle 1 off [checked]",
                "c004 subtitle main at-play SETUP > SUBTITLES: Language A -> playlist 00800, audio 1, subtitle 2 shown [replayed]",
                "c005 subtitle main at-play SETUP > SUBTITLES: OFF -> playlist 00800, audio 1, subtitle 2 off, same as play [replayed]",
                "c006 subtitle popup immediately POP-UP > SUBTITLES: Langue C — ÉTÉ -> playlist 00800, audio 1, subtitle 3 shown [read]",
                "c007 bonus main at-play EXTRAS: Bonus 1 -> playlist 01001, audio 1, subtitle none off [replayed]",
                "c008 version main at-play PLAY > VERSIONS: Version 2 -> playlist 00801, audio 1, subtitle 1 off [replayed]",
                "c009 audio main at-play SETUP > AUDIO: Language D -> no effect (no final state) [read]",
                "not mapped: SCENES",
                "quirks: 1",
            ])
        );
    }

    #[test]
    fn menu_show_prints_unknown_for_what_the_map_does_not_say_and_seconds_as_python_does() {
        let tmp = world("menu-show-unknown", None);
        let whole = FIXTURE.replacen("\"seconds_to_menu\": 12.5", "\"seconds_to_menu\": 13", 1);
        let bare = whole.replacen("\"seconds_to_menu\": 13", "\"seconds_to_menu\": null", 1);
        let bare = bare
            .replacen("\"format\": \"uhd\"", "\"format\": null", 1)
            .replacen("\"script\": \"scripts/start.txt\"", "\"script\": null", 1);
        let mut bare: serde_json::Value = serde_json::from_str(&bare).unwrap();
        bare["made"] = serde_json::json!({});
        let bare = bare.to_string();
        assert!(menu::read(&whole).is_ok() && menu::read(&bare).is_ok());
        tmp.write("whole.json", whole);
        tmp.write("bare.json", bare);
        let (code, out, _) = menu_run(&tmp, &["show", "whole.json"]);
        assert_eq!(code, 0);
        assert!(out.contains("\nstart: scripts/start.txt, 1 passed, 13.0 s to the menu\n"), "{out}");
        let (code, out, _) = menu_run(&tmp, &["show", "bare.json"]);
        assert_eq!(code, 0);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "map made-up-1: format unknown, level read, 9 choices");
        assert_eq!(lines[1], "made by unknown, checked by unknown, unknown");
        assert_eq!(lines[4], "start: none, 1 passed, unknown s to the menu");
    }

    #[test]
    fn menu_show_escapes_map_text_that_would_break_a_line() {
        let tmp = world("menu-show-escaped", None);
        let mut map: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
        map["choices"][0]["section"] = "SETUP\nAUDIO".into();
        map["choices"][1]["section"] = "SETUP\u{2028}AUDIO".into();
        map["choices"][7]["effect"]["playlist"] = "00801\nquirks: 0".into();
        map["choices"][8]["check"]["note"] = "no final\nstate".into();
        map["disc"]["format"] = "uhd\nmap x".into();
        map["disc"]["disc_id_sha1"] = "ABC\rDEF".into();
        map["disc"]["listing_sig"] = "not hex".into();
        map["made"]["by"] = "agent\u{85}explorer".into();
        map["start"]["script"] = "scripts/start\n.txt".into();
        map["not_mapped"][0] = "SCENES\nquirks: 0".into();
        assert!(menu::read(&map.to_string()).is_ok());
        tmp.write("odd.json", map.to_string());
        let (code, out, err) = menu_run(&tmp, &["show", "odd.json"]);
        assert_eq!((code, err.as_str()), (0, ""));
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 17, "{out}");
        assert_eq!(lines[0], "map made-up-1: format 'uhd\\nmap x', level read, 9 choices");
        assert_eq!(lines[1], "made by 'agent\\x85explorer', checked by agent-checker, 2026-11-17");
        assert_eq!(lines[2..4], ["disc_id_sha1 'ABC\\rDEF'", "listing_sig 'not hex'"]);
        assert_eq!(lines[4], "start: 'scripts/start\\n.txt', 1 passed, 12.5 s to the menu");
        assert_eq!(
            lines[6],
            "c001 audio main at-play 'SETUP\\nAUDIO: Language A 5.1' -> playlist 00800, audio 1, subtitle 1 off, \
             same as play [replayed]"
        );
        assert_eq!(
            lines[7],
            "c002 audio main at-play 'SETUP\\u2028AUDIO: Language B 5.1' -> playlist 00800, audio 2, subtitle 3 shown \
             [replayed]"
        );
        assert_eq!(
            lines[13],
            "c008 version main at-play PLAY > VERSIONS: Version 2 -> playlist '00801\\nquirks: 0', audio 1, subtitle 1 \
             off [replayed]"
        );
        assert_eq!(lines[14], "c009 audio main at-play SETUP > AUDIO: Language D -> no effect ('no final\\nstate') [read]");
        assert_eq!(lines[15], "not mapped: 'SCENES\\nquirks: 0'");
        assert_eq!(out.lines().filter(|line| line.starts_with("quirks: ")).count(), 1, "{out}");
        // Split as Python's `str.splitlines` splits (U+2028, U+0085 and a lone '\r' end a line too), the output has
        // the same 17 lines.
        let breaks = ['\n', '\r', '\u{b}', '\u{c}', '\u{1c}', '\u{1d}', '\u{1e}', '\u{85}', '\u{2028}', '\u{2029}'];
        assert_eq!(out.trim_end_matches('\n').split(breaks).count(), 17, "{out}");
    }

    #[test]
    fn menu_show_takes_the_map_folder_or_the_file() {
        let tmp = world("menu-show-both", None);
        let by_folder = menu_run(&tmp, &["show", MAP_DIR]);
        let by_file = menu_run(&tmp, &["show", &format!("{MAP_DIR}/{FILE_NAME}")]);
        assert_eq!(by_folder.0, 0);
        assert_eq!(by_folder, by_file);
        assert!(by_folder.1.contains("Langue C — ÉTÉ"), "labels are printed as they are: {}", by_folder.1);
    }

    #[test]
    fn menu_show_lists_every_problem_of_an_invalid_map_and_exits_2() {
        let tmp = world("menu-show-invalid", None);
        let mut map: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
        map["format"] = serde_json::Value::Null;
        map["choices"][0]["kind"] = "video".into();
        map["choices"][0]["script"] = "../x".into();
        tmp.write("bad/menu-map.json", map.to_string());
        let (code, out, err) = menu_run(&tmp, &["show", "bad"]);
        let at = shown("bad");
        assert_eq!(code, 2);
        assert_eq!(out, "");
        assert_eq!(
            err,
            printed(&[
                &format!("lazymkv-dev menu: error: {at}: format is None, not 'lazymkv menu map v1'"),
                &format!("lazymkv-dev menu: error: {at}: choices[0] (c001): kind 'video' is not one of audio, subtitle, version, bonus, other"),
                &format!("lazymkv-dev menu: error: {at}: choices[0] (c001): script has a \"..\" part"),
            ])
        );
    }

    #[test]
    fn menu_show_refuses_a_newer_format_with_one_line() {
        let tmp = world("menu-show-v2", None);
        tmp.write("v2.json", FIXTURE.replacen("\"lazymkv menu map v1\"", "\"lazymkv menu map v2\"", 1));
        let (code, out, err) = menu_run(&tmp, &["show", "v2.json"]);
        assert_eq!((code, out.as_str()), (2, ""));
        assert_eq!(
            err,
            "lazymkv-dev menu: error: v2.json: this lazymkv reads only 'lazymkv menu map v1'; the map says it is 'lazymkv menu map v2'\n"
        );
    }

    #[test]
    fn menu_show_of_a_missing_map_or_a_bad_text_exits_2() {
        let tmp = world("menu-show-missing", None);
        let (code, out, err) = menu_run(&tmp, &["show", "nowhere/menu-map.json"]);
        assert_eq!((code, out.as_str()), (2, ""));
        assert!(err.starts_with("lazymkv-dev menu: error: nowhere/menu-map.json: "), "{err}");
        assert_eq!(err.lines().count(), 1, "{err}");
        tmp.write("not-text.json", [0xffu8, 0xfe, 0x41]);
        let (code, _, err) = menu_run(&tmp, &["show", "not-text.json"]);
        assert_eq!(code, 2);
        assert!(err.starts_with("lazymkv-dev menu: error: not-text.json: "), "{err}");
        tmp.write("not-json.json", "{");
        let (code, _, err) = menu_run(&tmp, &["show", "not-json.json"]);
        assert_eq!(code, 2);
        assert!(err.starts_with("lazymkv-dev menu: error: not-json.json: not JSON: "), "{err}");
    }

    #[test]
    fn menu_check_on_a_recording_folder_matches_the_disc_id_and_exits_0() {
        let tmp = world("menu-check-match", Some(FIXTURE_ID));
        let (code, out, err) = menu_run(&tmp, &["check", MAP_DIR, "--scan", REC_DIR]);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(
            out,
            printed(&[
                &format!("identity: disc_id matches ({FIXTURE_ID})"),
                "listing_sig: not checked (lazymkv does not recompute it)",
                "play: playlist 00800, title 0: audio 1 = stream 1, subtitle 1 off",
                "c001: playlist 00800, title 0: audio 1 = stream 1, subtitle 1 off",
                "c002: playlist 00800, title 0: audio 2 = stream 3, subtitle 3 = stream 8",
                "c003: playlist 00800, title 0: audio 3 = stream 4, subtitle 1 off",
                "c004: playlist 00800, title 0: audio 1 = stream 1, subtitle 2 = stream 7",
                "c005: playlist 00800, title 0: audio 1 = stream 1, subtitle 2 off",
                "c006: playlist 00800, title 0: audio 1 = stream 1, subtitle 3 = stream 8",
                "c007: playlist 01001, title 1: audio 1 = stream 1, subtitle none",
                "c008: playlist 00801 not in the scan",
                "c009: no effect recorded",
                "CHECK ok entries=10 on_scan=8 not_in_scan=1 no_effect=1 warnings=0 failed=0; \
                 streams of not_in_scan entries were not checked",
            ])
        );
    }

    #[test]
    fn menu_check_on_a_bare_scan_file_says_identity_was_not_checked() {
        let tmp = world("menu-check-bare", Some(FIXTURE_ID));
        let (code, out, err) = menu_run(&tmp, &["check", MAP_DIR, "--scan", &format!("{REC_DIR}/scan.txt")]);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(
            out.lines().next(),
            Some("identity: not checked: a scan file carries no disc ID; give the recording folder")
        );
        assert!(out.lines().last().unwrap().starts_with("CHECK ok "), "{out}");
    }

    #[test]
    fn menu_check_of_another_discs_map_exits_1_naming_both_ids() {
        let tmp = world("menu-check-other", Some("FFFF"));
        let (code, out, err) = menu_run(&tmp, &["check", MAP_DIR, "--scan", REC_DIR]);
        assert_eq!((code, err.as_str()), (1, ""));
        assert_eq!(out.lines().next(), Some(format!("identity: disc_id differs: the map's is {FIXTURE_ID}, the disc's is FFFF").as_str()));
        assert_eq!(
            out.lines().last(),
            Some(
                "CHECK FAILED entries=10 on_scan=8 not_in_scan=1 no_effect=1 warnings=0 failed=1; \
                 streams of not_in_scan entries were not checked"
            )
        );
        // A meta.json `disc_id` that would break the line is quoted, so it cannot forge a CHECK line.
        let tmp = world("menu-check-forged", Some("FFFF\\nCHECK ok entries=0"));
        let (code, out, err) = menu_run(&tmp, &["check", MAP_DIR, "--scan", REC_DIR]);
        assert_eq!((code, err.as_str()), (1, ""));
        let first = format!("identity: disc_id differs: the map's is {FIXTURE_ID}, the disc's is 'FFFF\\nCHECK ok entries=0'");
        assert_eq!(out.lines().next(), Some(first.as_str()));
        assert_eq!(out.lines().count(), 13, "{out}");
        assert_eq!(out.lines().filter(|line| line.starts_with("CHECK ")).count(), 1, "{out}");
    }

    #[test]
    fn menu_check_reports_a_playlist_not_in_the_scan_and_exits_0() {
        let tmp = world("menu-check-not-in-scan", Some(FIXTURE_ID));
        let (code, out, _) = menu_run(&tmp, &["check", MAP_DIR, "--scan", REC_DIR]);
        assert_eq!(code, 0);
        assert!(out.lines().any(|line| line == "c008: playlist 00801 not in the scan"), "{out}");
        assert!(out.lines().any(|line| line == "c009: no effect recorded"), "{out}");
    }

    #[test]
    fn menu_check_prints_no_label_text() {
        let tmp = world("menu-check-labels", Some(FIXTURE_ID));
        let (_, out, err) = menu_run(&tmp, &["check", MAP_DIR, "--scan", REC_DIR]);
        let map = menu::read(FIXTURE).unwrap();
        let mut words = vec![map.play.label.clone()];
        for choice in &map.choices {
            words.push(choice.label.clone());
            words.push(choice.section.clone());
            words.push(choice.shown_label());
        }
        assert!(words.len() > 20);
        for word in words {
            assert!(!out.contains(&word) && !err.contains(&word), "{word:?} is printed: {out}");
        }
        // The ids and the numbers are there.
        assert!(out.contains("c002: playlist 00800, title 0: audio 2 = stream 3, subtitle 3 = stream 8"), "{out}");
    }

    #[test]
    fn menu_check_with_a_slug_and_no_evidence_clone_exits_3() {
        let tmp = world("menu-check-slug", Some(FIXTURE_ID));
        tmp.write("Cargo.toml", "[workspace]\nmembers = []\n");
        let (code, out, err) = menu_run(&tmp, &["check", MAP_DIR, "--scan", "made-up-1"]);
        assert_eq!((code, out.as_str()), (3, ""));
        assert!(err.starts_with("lazymkv-dev menu: error: corpus/owner: no such folder; the evidence repository"), "{err}");
        assert_eq!(err.lines().count(), 1, "{err}");
        // A slug under --evidence is found there.
        tmp.write("clone/made-up-1/scan.txt", menu_scan_text());
        let (code, out, _) = menu_run(&tmp, &["check", MAP_DIR, "--scan", "made-up-1", "--evidence", "clone"]);
        assert_eq!(code, 0);
        assert!(out.starts_with("identity: not checked: the recording has no meta.json\n"), "{out}");
    }

    #[test]
    fn menu_check_refuses_a_map_or_a_scan_it_cannot_use_with_exit_2() {
        let tmp = world("menu-check-refused", Some(FIXTURE_ID));
        let (code, out, err) = menu_run(&tmp, &["check", "nowhere", "--scan", REC_DIR]);
        assert_eq!((code, out.as_str()), (2, ""));
        // The rest of the line is the operating system's.
        assert!(err.starts_with("lazymkv-dev menu: error: nowhere: "), "{err}");
        assert_eq!(err.lines().count(), 1, "{err}");
        let (code, out, err) = menu_run(&tmp, &["check", MAP_DIR, "--scan", "rec/nowhere"]);
        assert_eq!((code, out.as_str()), (2, ""));
        assert_eq!(err, "lazymkv-dev menu: error: rec/nowhere: no such folder\n");
        // A scan `disc::disc` refuses: an audio stream whose flags are not a number.
        let broken = scan_text(&[], &[(0, &[(16, "00800.mpls")], &[(1, &[(1, "Audio"), (22, "x")])])]);
        let sentence = disc(&parse(&broken)).unwrap_err();
        tmp.write("rec/broken/scan.txt", broken);
        let (code, out, err) = menu_run(&tmp, &["check", MAP_DIR, "--scan", "rec/broken"]);
        assert_eq!((code, out.as_str()), (2, ""));
        assert_eq!(err, format!("lazymkv-dev menu: error: rec/broken/scan.txt: {sentence}\n"));
        // A scan's text in the sentence that would break the line is quoted with it, so it cannot forge a line.
        let size = "1x\u{2028}CHECK ok entries=10";
        let forged = scan_text(&[], &[(0, &[(16, "00800.mpls")], &[(1, &[(1, "Video"), (19, size)])])]);
        let sentence = disc(&parse(&forged)).unwrap_err();
        assert!(sentence.contains(size), "{sentence}");
        tmp.write("scan-forged.txt", forged);
        let (code, out, err) = menu_run(&tmp, &["check", MAP_DIR, "--scan", "scan-forged.txt"]);
        assert_eq!((code, out.as_str()), (2, ""));
        assert_eq!(err, format!("lazymkv-dev menu: error: scan-forged.txt: {}\n", menu::line_text(&sentence)));
        assert!(err.contains("\\u2028CHECK ok") && !err.contains('\u{2028}'), "{err}");
        // A file that is not a scan, and a recording whose scan.txt is empty, list no title: refused, not passed.
        tmp.write("scan-not-a-disc.txt", "this is not a robot log\n");
        let (code, out, err) = menu_run(&tmp, &["check", MAP_DIR, "--scan", "scan-not-a-disc.txt"]);
        assert_eq!((code, out.as_str()), (2, ""));
        assert_eq!(err, "lazymkv-dev menu: error: scan-not-a-disc.txt: the scan lists no title\n");
        tmp.write("rec/empty/scan.txt", "");
        tmp.write("rec/empty/meta.json", format!("{{\"disc_id\": \"{FIXTURE_ID}\"}}"));
        let (code, out, err) = menu_run(&tmp, &["check", MAP_DIR, "--scan", "rec/empty"]);
        assert_eq!((code, out.as_str()), (2, ""));
        assert_eq!(err, "lazymkv-dev menu: error: rec/empty/scan.txt: the scan lists no title\n");
    }

    #[test]
    fn the_disc_id_of_a_recording_is_known_or_says_why_not() {
        let tmp = Scratch::new("menu-disc-id");
        let ctx = Ctx::for_test(&tmp.0);
        let of = |dir: &str| disc_id_of(&resolve_recording(&ctx, None, dir).unwrap());
        tmp.write("a/scan.txt", "");
        assert_eq!(of("a"), Ok(DiscId::Absent(NO_META.to_string())));
        tmp.write("b/meta.json", "{\"fingerprint\": \"x\"}");
        assert_eq!(of("b"), Ok(DiscId::Absent(META_WITHOUT_ID.to_string())));
        tmp.write("c/meta.json", "{\"disc_id\": null}");
        assert_eq!(of("c"), Ok(DiscId::Absent(META_WITHOUT_ID.to_string())));
        tmp.write("d/meta.json", "{\"disc_id\": \"\"}");
        assert_eq!(of("d"), Ok(DiscId::Absent(META_WITHOUT_ID.to_string())));
        tmp.write("e/meta.json", "{\"disc_id\": \"ABCD\"}");
        assert_eq!(of("e"), Ok(DiscId::Known("ABCD".to_string())));
        tmp.write("g/meta.json", "{\"disc_id\": \" ABCD \"}");
        assert_eq!(of("g"), Ok(DiscId::Known("ABCD".to_string())));
        tmp.write("f/meta.json", "{\"disc_id\": ");
        let refusal: Refused = of("f").unwrap_err();
        assert_eq!(refusal.code, 2);
        assert!(refusal.line.contains("meta.json: "), "{}", refusal.line);
    }

    #[test]
    fn read_map_shows_the_path_it_was_given_and_the_map() {
        let tmp = world("menu-read-map", None);
        let ctx = Ctx::for_test(&tmp.0);
        let (at, map) = read_map(&ctx, MAP_DIR).unwrap();
        assert_eq!(at, shown(MAP_DIR));
        assert_eq!(map, menu::read(FIXTURE).unwrap());
        let (at, _) = read_map(&ctx, &format!("{MAP_DIR}/{FILE_NAME}")).unwrap();
        assert_eq!(at, format!("{MAP_DIR}/{FILE_NAME}"));
        // A folder given with trailing separators is shown without them, joined with one `/`.
        let (at, _) = read_map(&ctx, &format!("{MAP_DIR}//")).unwrap();
        assert_eq!(at, "maps/made-up-1/menu-map.json");
        // A leading byte-order mark is dropped.
        tmp.write("bom.json", format!("\u{feff}{FIXTURE}"));
        assert_eq!(read_map(&ctx, "bom.json").unwrap().1, map);
        // An empty path is the working folder: its map is shown by the file name alone, not under the root.
        let refusal = read_map(&ctx, "").unwrap_err();
        assert!(refusal.line.starts_with(&format!("{FILE_NAME}: ")), "{}", refusal.line);
        tmp.write(FILE_NAME, FIXTURE);
        assert_eq!(read_map(&ctx, "").unwrap(), (FILE_NAME.to_string(), map));
    }

    #[test]
    fn read_off_reads_the_file_beside_the_map() {
        let tmp = world("menu-read-off", None);
        let ctx = Ctx::for_test(&tmp.0);
        let map_file = format!("{MAP_DIR}/{FILE_NAME}");
        // An absent file is no decision.
        assert_eq!(read_off(&ctx, MAP_DIR).unwrap(), OffDecisions::default());
        assert_eq!(read_off(&ctx, &map_file).unwrap(), OffDecisions::default());
        // The folder form and the file form read the same file, the map folder's.
        let text = "# made-up decisions\noff\tc003\tAUDIO: COMMENTARY 1\nrole\tc002\tAUDIO: Language B 5.1\n";
        tmp.write(&format!("{MAP_DIR}/{OFF_FILE_NAME}"), text);
        let expected = parse_off(text).unwrap();
        assert_ne!(expected, OffDecisions::default());
        assert_eq!(read_off(&ctx, MAP_DIR).unwrap(), expected);
        assert_eq!(read_off(&ctx, &map_file).unwrap(), expected);
        // A malformed file is refused with one `<shown>: <problem>` line per problem, either form.
        let bad = "of\tc003\tA\noff\tx1\tA\n";
        tmp.write(&format!("{MAP_DIR}/{OFF_FILE_NAME}"), bad);
        let at = format!("{MAP_DIR}/{OFF_FILE_NAME}");
        let lines: Vec<String> = parse_off(bad).unwrap_err().iter().map(|problem| format!("{at}: {problem}")).collect();
        assert_eq!(lines.len(), 2);
        for given in [MAP_DIR, map_file.as_str()] {
            let refusal = read_off(&ctx, given).unwrap_err();
            assert_eq!((refusal.code, refusal.line.clone()), (2, lines.join("\n")), "{given}");
            assert!(refusal.line.starts_with(&format!("{at}: line 1: ")), "{}", refusal.line);
        }
        // A map file named without a folder: the off-decisions of the working folder, shown by the file name alone.
        tmp.write("loose.json", FIXTURE);
        assert_eq!(read_off(&ctx, "loose.json").unwrap(), OffDecisions::default());
        tmp.write(OFF_FILE_NAME, "off c003\n");
        let refusal = read_off(&ctx, "loose.json").unwrap_err();
        assert!(refusal.line.starts_with(&format!("{OFF_FILE_NAME}: line 1: ")), "{}", refusal.line);
        // A file that is not UTF-8 is refused, read strictly as the map is.
        tmp.write(&format!("{MAP_DIR}/{OFF_FILE_NAME}"), [0xffu8, 0xfe, 0x41]);
        let refusal = read_off(&ctx, MAP_DIR).unwrap_err();
        assert!(refusal.line.starts_with(&format!("{at}: ")), "{}", refusal.line);
        assert_eq!(refusal.line.lines().count(), 1, "{}", refusal.line);
    }

    #[test]
    fn read_playlists_reads_only_five_digit_names() {
        let tmp = world("menu-read-playlists", None);
        let item = |audio: Vec<(u16, u8, &'static str)>| MadeUpItem {
            clip: "00055",
            in_time: 0,
            out_time: 90_000,
            audio,
            subtitles: vec![(0x1200, "eng")],
        };
        let bytes = made_up_playlist(&[item(vec![(0x1100, 0x86, "eng"), (0x1101, 0x81, "spa")])]);
        let folder = format!("{REC_DIR}/nav/BDMV/PLAYLIST");
        tmp.write(&format!("{folder}/00800.mpls"), &bytes);
        tmp.write(&format!("{folder}/00802.mpls"), &bytes[..20]);
        // A playlist where `../x` would lead: never looked up.
        tmp.write(&format!("{REC_DIR}/nav/BDMV/x.mpls"), &bytes);
        tmp.write(&format!("{folder}/0080.mpls"), &bytes);
        let ctx = Ctx::for_test(&tmp.0);
        let rec = resolve_recording(&ctx, None, REC_DIR).unwrap();
        let names: BTreeSet<String> = ["00800", "../x", "0080", "00801", "00802"].map(String::from).into();
        let read = read_playlists(&rec, &names);
        assert_eq!(read.keys().collect::<Vec<_>>(), names.iter().collect::<Vec<_>>());
        assert_eq!(read["00800"], parse_playlist(&bytes));
        assert!(read["00800"].is_ok());
        assert_eq!(read["../x"], Err("not a playlist name".to_string()));
        assert_eq!(read["0080"], Err("not a playlist name".to_string()));
        assert_eq!(read["00801"], Err("no such file".to_string()));
        let cut = parse_playlist(&bytes[..20]).unwrap_err();
        assert_eq!(read["00802"], Err(cut));
    }

    #[test]
    fn an_unknown_verb_is_a_usage_error_with_exit_2() {
        let tmp = Scratch::new("menu-verb");
        let (code, out, err) = menu_run(&tmp, &["frob", "x"]);
        assert_eq!((code, out.as_str()), (2, ""));
        assert_eq!(
            err,
            "usage: lazymkv-dev menu [-h] {show,check} ...\n\
             lazymkv-dev menu: error: argument command: invalid choice: 'frob' (choose from 'show', 'check')\n"
        );
        let (code, _, err) = menu_run(&tmp, &[]);
        assert_eq!(code, 2);
        assert!(err.ends_with("lazymkv-dev menu: error: the following arguments are required: command\n"), "{err}");
    }

    #[test]
    fn missing_and_extra_arguments_are_usage_errors_with_exit_2() {
        let tmp = world("menu-usage", Some(FIXTURE_ID));
        let (code, _, err) = menu_run(&tmp, &["show"]);
        assert_eq!(code, 2);
        assert_eq!(
            err,
            "usage: lazymkv-dev menu show [-h] map\nlazymkv-dev menu show: error: the following arguments are required: map\n"
        );
        let (code, _, err) = menu_run(&tmp, &["show", MAP_DIR, "extra"]);
        assert_eq!(code, 2);
        assert!(err.ends_with("lazymkv-dev menu: error: unrecognized arguments: extra\n"), "{err}");
        let usage = "usage: lazymkv-dev menu check [-h] map --scan SCAN [--evidence DIR]\n";
        let (code, _, err) = menu_run(&tmp, &["check", MAP_DIR]);
        assert_eq!(code, 2);
        assert_eq!(err, format!("{usage}lazymkv-dev menu check: error: the following arguments are required: --scan\n"));
        let (code, _, err) = menu_run(&tmp, &["check"]);
        assert_eq!(code, 2);
        assert_eq!(err, format!("{usage}lazymkv-dev menu check: error: the following arguments are required: map, --scan\n"));
        let (code, _, err) = menu_run(&tmp, &["check", MAP_DIR, "--scan"]);
        assert_eq!(code, 2);
        assert_eq!(err, format!("{usage}lazymkv-dev menu check: error: argument --scan: expected one argument\n"));
    }

    #[test]
    fn help_before_or_after_a_verb_prints_that_usage_and_exits_0() {
        let tmp = Scratch::new("menu-help");
        for (args, usage) in [
            (&["-h"][..], "usage: lazymkv-dev menu [-h] {show,check} ...\n"),
            (&["--help"][..], "usage: lazymkv-dev menu [-h] {show,check} ...\n"),
            (&["show", "-h"][..], "usage: lazymkv-dev menu show [-h] map\n"),
            (&["show", "x", "--help"][..], "usage: lazymkv-dev menu show [-h] map\n"),
            (&["check", "-h"][..], "usage: lazymkv-dev menu check [-h] map --scan SCAN [--evidence DIR]\n"),
            (&["check", "x", "--scan", "y", "-h"][..], "usage: lazymkv-dev menu check [-h] map --scan SCAN [--evidence DIR]\n"),
        ] {
            let (code, out, err) = menu_run(&tmp, args);
            assert_eq!((code, err.as_str()), (0, ""), "{args:?}");
            assert!(out.starts_with(usage), "{args:?}: {out}");
        }
    }
}
