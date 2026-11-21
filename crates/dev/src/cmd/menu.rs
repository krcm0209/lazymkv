//! `lazymkv-dev menu show <map> | check <map> --scan <scan> [--evidence DIR]`: menu maps at the command line (menu maps
//! design 5.4; the format and rulings M3-1 to M3-8 are docs/menu-maps/format.md). `show` prints a map, labels as they
//! are; `check` ties it to a scanned disc and prints ids, playlists and numbers only, never a label. Exit: show 0, or 2;
//! check 0 when nothing failed, 1 otherwise; 2 for an argument, map or scan problem; 3 for a slug without the evidence
//! clone. Refusals print `lazymkv-dev menu: error: <line>`, one per problem. Evidence only: no decision reads a map.

use crate::evidence::{read_file_text, resolve_recording, Ctx, Recording, Refused};
use crate::{usage_error, Args};
use lazymkv_core::disc::{disc, Disc};
use lazymkv_core::menu::{
    self, DiscId, Effect, MenuMap, FILE_NAME, META_WITHOUT_ID, NO_ID_IN_SCAN_FILE, NO_META,
};
use lazymkv_core::recording::files::Meta;
use lazymkv_core::recording::{py_float_repr, py_repr_str, read_json, read_text, text_mode, Encoding, Errors, Tree};
use lazymkv_core::robot::parse;
use std::io::Write;
use std::path::Path;

/// The command's name as typed.
const NAME: &str = "menu";
/// argparse's usage lines, under the command's name.
const USAGE: &str = "menu [-h] {show,check} ...";
const SHOW_USAGE: &str = "menu show [-h] map";
const CHECK_USAGE: &str = "menu check [-h] map --scan SCAN [--evidence DIR]";
/// A recording folder's files this command reads.
const SCAN_FILE: &str = "scan.txt";
const META_FILE: &str = "meta.json";

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

exit status: 0 when nothing failed; 1 when a stream the map names is not on the scanned disc or the map is of another
disc; 2 for an argument error, or a map or scan that cannot be used; 3 when a slug is given and the evidence clone is
absent.
";

/// A refusal with exit code 2.
fn refused(line: String) -> Refused {
    Refused { code: 2, line }
}

/// The map at `given`: the file `<given>/menu-map.json` when `given` is a folder, else the file itself; the text of the
/// map's path as it is shown (the path as given, joined to the file name for a folder) and the map. The file is UTF-8
/// with an optional byte-order mark, read strictly. Every problem of `menu::read` becomes one line `<shown>: <problem>`
/// in the refusal (code 2), the lines joined with '\n'.
pub(crate) fn read_map(ctx: &Ctx, given: &str) -> Result<(String, MenuMap), Refused> {
    let path = ctx.abs(given);
    let (path, shown) = if path.is_dir() {
        (path.join(FILE_NAME), Path::new(given).join(FILE_NAME).to_string_lossy().into_owned())
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

/// A recording's disc ID, its meta.json `disc_id` (M3-6): `Absent(NO_META)` when the recording has no meta.json,
/// `Absent(META_WITHOUT_ID)` when `disc_id` is missing, null or empty (surrounding white space left off), else
/// `Known`. A meta.json that cannot be read or decoded is Refused { code: 2, "<path>: <problem>" }.
pub(crate) fn disc_id_of(rec: &Recording) -> Result<DiscId, Refused> {
    let key = rec.dir.join(META_FILE);
    if !rec.tree.is_file(&key) {
        return Ok(DiscId::Absent(NO_META.to_string()));
    }
    let meta: Meta = read_json(&rec.tree, &key).map_err(|problem| refused(format!("{}: {problem}", rec.tree.display(&key))))?;
    Ok(match meta.disc_id.as_deref().map(str::trim).filter(|id| !id.is_empty()) {
        Some(id) => DiscId::Known(id.to_string()),
        None => DiscId::Absent(META_WITHOUT_ID.to_string()),
    })
}

/// The typed disc of a scan's text (spec 9.4), read through text mode; `disc::disc`'s refusal is prefixed by `shown`.
fn typed_disc(text: &str, shown: &str) -> Result<Disc, Refused> {
    disc(&parse(&text_mode(text))).map_err(|sentence| refused(format!("{shown}: {sentence}")))
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

/// What an entry selects: `playlist P, audio A, subtitle S shown|off`, or `no effect (<note>)`.
fn effect_text(effect: Option<&Effect>, note: &str) -> String {
    match effect {
        Some(e) => format!(
            "playlist {}, audio {}, subtitle {} {}",
            e.playlist,
            number(e.audio),
            number(e.subtitle),
            if e.subtitle_shown { "shown" } else { "off" }
        ),
        None => format!("no effect ({note})"),
    }
}

/// The lines `menu show` prints, labels as they are.
fn show_lines(map: &MenuMap) -> Vec<String> {
    let made = |key: &str| map.made.get(key).and_then(serde_json::Value::as_str).unwrap_or("unknown");
    let seconds = map.start.seconds_to_menu.map_or_else(|| "unknown".to_string(), py_float_repr);
    let mut lines = vec![
        format!(
            "map {}: format {}, level {}, {} choices",
            map.disc.slug,
            map.disc.format.as_deref().unwrap_or("unknown"),
            menu::map_level(map).word(),
            map.choices.len()
        ),
        format!("made by {}, checked by {}, {}", made("by"), made("checked_by"), made("date")),
        format!("disc_id_sha1 {}", map.disc.disc_id_sha1.as_deref().unwrap_or("none")),
        format!("listing_sig {}", map.disc.listing_sig.as_deref().unwrap_or("none")),
        format!(
            "start: {}, {} passed, {} s to the menu",
            map.start.script.as_deref().unwrap_or("none"),
            map.start.passed.len(),
            seconds
        ),
        format!(
            "play: {} -> {} [{}]",
            map.play.label,
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
            choice.shown_label(),
            effect_text(choice.effect.as_ref(), &choice.check.note),
            same,
            choice.check.level.word()
        ));
    }
    lines.extend(map.not_mapped.iter().map(|item| format!("not mapped: {item}")));
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
        [_, extra @ ..] => return usage_error(err, NAME, USAGE, &format!("unrecognized arguments: {}", extra.join(" "))),
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
            let message = format!("argument command: invalid choice: {} (choose from 'show', 'check')", py_repr_str(other));
            return usage_error(err, NAME, USAGE, &message);
        }
    };
    let _ = writeln!(out, "usage: lazymkv-dev {usage}");
    let _ = write!(out, "{text}");
    0
}

#[cfg(test)]
mod tests {
    use super::{disc_id_of, read_map, run};
    use crate::evidence::{resolve_recording, Ctx, Refused};
    use crate::testutil::{call, printed, Scratch, ScratchExt};
    use lazymkv_core::menu::{self, DiscId, FILE_NAME, FIXTURE, META_WITHOUT_ID, NO_META};
    use lazymkv_core::testsupport::{menu_scan_text, scan_text};
    use std::path::Path;

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

    /// The path of the map file as `read_map` shows it for the folder `dir`.
    fn shown(dir: &str) -> String {
        Path::new(dir).join(FILE_NAME).to_string_lossy().into_owned()
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
        let bare = bare.replacen("\"format\": \"uhd\"", "\"format\": null", 1).replacen("\"script\": \"scripts/start.txt\"", "\"script\": null", 1);
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
        assert_eq!(lines[4], "start: none, 1 passed, unknown s to the menu");
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
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], format!("identity: disc_id matches ({FIXTURE_ID})"));
        assert_eq!(lines[1], "listing_sig: not checked (lazymkv does not recompute it)");
        assert_eq!(
            lines.last().copied(),
            Some(
                "CHECK ok entries=10 on_scan=8 not_in_scan=1 no_effect=1 warnings=0 failed=0; \
                 streams of not_in_scan entries were not checked"
            )
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
        tmp.write("scan-not-a-disc.txt", "this is not a robot log\n");
        let (code, out, err) = menu_run(&tmp, &["check", "nowhere", "--scan", REC_DIR]);
        assert_eq!((code, out.as_str()), (2, ""));
        assert!(err.starts_with("lazymkv-dev menu: error: "), "{err}");
        let (code, out, err) = menu_run(&tmp, &["check", MAP_DIR, "--scan", "rec/nowhere"]);
        assert_eq!((code, out.as_str()), (2, ""));
        assert_eq!(err, "lazymkv-dev menu: error: rec/nowhere: no such folder\n");
        // A scan `disc::disc` refuses: an audio stream whose flags are not a number.
        let broken = scan_text(&[], &[(0, &[(16, "00800.mpls")], &[(1, &[(1, "Audio"), (22, "x")])])]);
        tmp.write("rec/broken/scan.txt", broken);
        let (code, out, err) = menu_run(&tmp, &["check", MAP_DIR, "--scan", "rec/broken"]);
        assert_eq!((code, out.as_str()), (2, ""));
        assert!(err.starts_with("lazymkv-dev menu: error: "), "{err}");
        assert_eq!(err.lines().count(), 1, "{err}");
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
        // A leading byte-order mark is dropped.
        tmp.write("bom.json", format!("\u{feff}{FIXTURE}"));
        assert_eq!(read_map(&ctx, "bom.json").unwrap().1, map);
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
