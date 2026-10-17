//! `lazymkv-dev labels [--spec FILE] [--subtitles] kinds|check|set|unset|title|auto|fitted ...`: the owner's audio
//! labels, `<slug_dir>/audio-labels.json`, and the fitted list, `<corpus>/audio-fitted.json` (decision D16); with
//! `--subtitles` (RP15), the owner's subtitle labels, `<slug_dir>/subtitle-labels.json`, through `kinds`, `check`,
//! `set` and `unset` (`core::subtitle::labels`). Paths are taken as given and no evidence check is made (spec 3.4).
//! For the audio labels the sentences, the files written and the exit codes are the Python's: 0, or 2 for an argument
//! error, an unreadable spec, a file that does not validate and a change that would add a problem; where the Python
//! crashed with a traceback the command prints one line and exits 2 (spec 3.4). `auto` applies its labels to one value
//! and writes the file once (the Python saved after each label). `--spec` (default: the design spec under the
//! repository root) and `--subtitles` come before the command word, as argparse's main options. The subtitle words
//! are new code (RP20) with the same exit codes; they read no spec, since what counts towards row 14 of spec section
//! 17 is `retune --subtitles`' (Task 18). Py: audio_labels.main.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use lazymkv_core::audio::labels::{
    add_fitted, auto, check_lines, exp13_requirements, kinds_lines, labels_check_lines, left_lines, load_fitted,
    open_for_change, set_title, set_track, unset_track, yes_no_unknown, FilmAudible, Fitted, Opened, SetOutcome,
    SetTrack, HOW, KINDS, TITLE_KINDS,
};
use lazymkv_core::audio::rule::RULE_VERSION;
use lazymkv_core::recording::{
    decode_text, pathlib_under, py_int, py_repr_str, py_str, py_truthy, Encoding, Errors, MemTree, PathKey, Tree,
};
use lazymkv_core::subtitle::labels::{self as subtitle_labels, SubtitleKind, SubtitleLabel};
use serde_json::{json, Value};

use crate::evidence::{default_spec, os_error_text, pathlib_str, Ctx, DiskTree};
use crate::pyjson::write_labels;
use crate::{usage_error, Args};

/// The synopsis after "usage: lazymkv-dev ". The COMMANDS row (crates/dev/src/lib.rs) holds it without the leading "labels".
const USAGE: &str = "labels [--spec FILE] [--subtitles] kinds|check|set|unset|title|auto|fitted ...";
/// The command words, in the Python's order (argparse's choices for `cmd`).
const SUBCOMMANDS: [&str; 7] = ["kinds", "check", "set", "unset", "title", "auto", "fitted"];
/// The command words `--subtitles` takes (RP15): the audio labels' `title`, `auto` and `fitted` have no subtitle form.
const SUBTITLE_SUBCOMMANDS: [&str; 4] = ["kinds", "check", "set", "unset"];
/// The main parser's long options, for argparse's prefix matching.
const MAIN_OPTIONS: [&str; 2] = ["--spec", "--subtitles"];

/// The synopsis of one command word (after "usage: lazymkv-dev ").
fn sub_usage(cmd: &str) -> &'static str {
    match cmd {
        "kinds" => "labels kinds",
        "check" => "labels check <path> ...",
        "set" => concat!(
            "labels set <slug_dir> <label> <stream> <kind> --how HOW --saw-verdict yes|no [--sure yes|no] ",
            "[--date YYYY-MM-DD] [--film-audible yes|no|unknown] [--note TEXT]",
        ),
        "unset" => "labels unset <slug_dir> <label> <stream> --note TEXT [--date YYYY-MM-DD]",
        "title" => "labels title <slug_dir> <label> --title-kind feature|episode|extra|other [--dialogue-heavy yes|no|unknown] [--note TEXT]",
        "auto" => "labels auto <slug_dir> [--date YYYY-MM-DD]",
        _ => "labels fitted <corpus> <slug> [<label> <stream>] --rule TEXT [--note TEXT] [--date YYYY-MM-DD]",
    }
}

/// An argparse error: from the command word's parser ("labels <cmd>: error:"), or from the main parser ("labels:
/// error:"), which is where argparse reports unrecognized arguments.
struct Usage {
    sub: bool,
    message: String,
}

fn sub(message: String) -> Usage {
    Usage { sub: true, message }
}

fn top(message: String) -> Usage {
    Usage { sub: false, message }
}

/// A command line, parsed and converted as argparse converts it.
enum Parsed {
    Kinds,
    Check { paths: Vec<String> },
    Set { slug_dir: String, args: SetTrack },
    Unset { slug_dir: String, label: String, stream: i64, note: String, date: String },
    Title { slug_dir: String, label: String, title_kind: String, dialogue_heavy: Option<bool>, note: String },
    Auto { slug_dir: String, date: String },
    Fitted { corpus: String, slug: String, label: Option<String>, stream: Option<i64>, rule: String, note: String, date: String },
}

/// Why the spec gave no fitting disc: an argument error (no --spec and no repository root), or the line to print.
enum Fail {
    Usage(String),
    Line(String),
}

pub fn run(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let Main { spec, subtitles, cmd, rest } = match split_main(args) {
        Ok(Some(main)) => main,
        Ok(None) => {
            let _ = writeln!(out, "usage: lazymkv-dev {USAGE}");
            return 0;
        }
        Err(message) => return usage_error(err, "labels", USAGE, &message),
    };
    if subtitles {
        return run_subtitles(ctx, &cmd, &rest, out, err);
    }
    if rest.iter().any(|word| word == "-h" || word == "--help") {
        let _ = writeln!(out, "usage: lazymkv-dev {}", sub_usage(&cmd));
        if cmd == "fitted" {
            let _ = writeln!(out, "  --rule TEXT  what was fitted: a commit, or the rule's version (now \"version {RULE_VERSION}\")");
        }
        return 0;
    }
    let parsed = match parse(&cmd, &rest, &ctx.today()) {
        Ok(parsed) => parsed,
        Err(Usage { sub: true, message }) => return usage_error(err, &format!("labels {cmd}"), sub_usage(&cmd), &message),
        Err(Usage { sub: false, message }) => return usage_error(err, "labels", USAGE, &message),
    };
    if matches!(parsed, Parsed::Kinds) {
        for line in kinds_lines() {
            let _ = writeln!(out, "{line}");
        }
        return 0;
    }
    let fitting = match fitting_disc(ctx, spec.as_deref()) {
        Ok(slug) => slug,
        Err(Fail::Usage(message)) => return usage_error(err, "labels", USAGE, &message),
        Err(Fail::Line(line)) => {
            let _ = writeln!(err, "{line}");
            return 2;
        }
    };
    match execute(ctx, parsed, &fitting, out) {
        Ok(code) => code,
        Err(line) => {
            let _ = writeln!(err, "{line}");
            2
        }
    }
}

/// The main parser's options and the command word, with what follows it.
struct Main {
    spec: Option<String>,
    subtitles: bool,
    cmd: String,
    rest: Vec<String>,
}

/// The main parser's part of the command line: `--spec FILE` (also `--spec=FILE`) and the switch `--subtitles`
/// (RP15), each also as a prefix of at least three characters that names only it (argparse's abbreviations: `--su`,
/// `--sp`; `--s` is ambiguous), and `-h`/`--help`, before the command word; then the command word, which with
/// `--subtitles` must be one of SUBTITLE_SUBCOMMANDS. Ok(None) asks for the help; Err is argparse's message.
fn split_main(args: &[String]) -> Result<Option<Main>, String> {
    let mut spec = None;
    let mut subtitles = false;
    let mut i = 0;
    while let Some(word) = args.get(i) {
        if !word.starts_with('-') || word == "-" {
            break;
        }
        let (name, inline) = match word.split_once('=') {
            Some((name, value)) => (name, Some(value.to_string())),
            None => (word.as_str(), None),
        };
        if word == "-h" || (name.len() >= 3 && "--help".starts_with(name) && inline.is_none()) {
            return Ok(None);
        }
        let matches: Vec<&str> =
            if name.len() >= 3 { MAIN_OPTIONS.iter().copied().filter(|option| option.starts_with(name)).collect() } else { Vec::new() };
        match matches.as_slice() {
            ["--spec"] => {
                spec = Some(match inline {
                    Some(value) => value,
                    None => {
                        i += 1;
                        args.get(i).cloned().ok_or_else(|| "argument --spec: expected one argument".to_string())?
                    }
                });
            }
            ["--subtitles"] => {
                if let Some(value) = inline {
                    return Err(format!("argument --subtitles: ignored explicit argument {}", py_repr_str(&value)));
                }
                subtitles = true;
            }
            [] => return Err(format!("unrecognized arguments: {word}")),
            _ => return Err(format!("ambiguous option: {name} could match {}", matches.join(", "))),
        }
        i += 1;
    }
    let cmd = args.get(i).cloned().ok_or_else(|| "the following arguments are required: cmd".to_string())?;
    if !SUBCOMMANDS.contains(&cmd.as_str()) {
        return Err(format!("argument cmd: invalid choice: {} (choose from {})", py_repr_str(&cmd), choices(&SUBCOMMANDS)));
    }
    if subtitles && !SUBTITLE_SUBCOMMANDS.contains(&cmd.as_str()) {
        return Err(format!(
            "argument cmd: {} does not take --subtitles (with --subtitles, choose from {})",
            py_repr_str(&cmd),
            choices(&SUBTITLE_SUBCOMMANDS)
        ));
    }
    Ok(Some(Main { spec, subtitles, cmd, rest: args[i + 1..].to_vec() }))
}

/// The command word's arguments. Order of the errors: type and choice errors (positionals, then options, in the
/// Python's declaration order), then the missing required arguments, then unrecognized arguments.
fn parse(cmd: &str, rest: &[String], today: &str) -> Result<Parsed, Usage> {
    match cmd {
        "kinds" => {
            extra(rest)?;
            Ok(Parsed::Kinds)
        }
        "check" => {
            let paths = Args::new(rest, &[]).positionals().map_err(top)?;
            required(&missing_words(&paths, &["paths"]))?;
            Ok(Parsed::Check { paths })
        }
        "set" => parse_set(rest, today),
        "unset" => parse_unset(rest, today),
        "title" => parse_title(rest),
        "auto" => parse_auto(rest, today),
        _ => parse_fitted(rest, today),
    }
}

fn parse_set(rest: &[String], today: &str) -> Result<Parsed, Usage> {
    let mut a = Args::new(rest, &["--how", "--saw-verdict", "--sure", "--date", "--film-audible", "--note"]);
    let how = a.flag("--how").map_err(sub)?;
    let saw = a.flag("--saw-verdict").map_err(sub)?;
    let sure = a.flag("--sure").map_err(sub)?;
    let date = a.flag("--date").map_err(sub)?;
    let film = a.flag("--film-audible").map_err(sub)?;
    let note = a.flag("--note").map_err(sub)?;
    let words = a.positionals().map_err(top)?;
    let kinds: Vec<&str> = KINDS.iter().map(|k| k.0).collect();
    let hows: Vec<&str> = HOW.iter().map(|h| h.0).collect();
    let stream = words.get(2).map(|w| int_arg("stream", w)).transpose()?;
    let kind = words.get(3).map(|w| choose("kind", w, &kinds)).transpose()?;
    let how = how.map(|h| choose("--how", &h, &hows)).transpose()?;
    let saw = saw.map(|s| yes("--saw-verdict", &s, false)).transpose()?;
    let sure = sure.map(|s| yes("--sure", &s, false)).transpose()?;
    let film_audible = match film {
        Some(f) => FilmAudible::Given(yes("--film-audible", &f, true)?),
        None => FilmAudible::Absent,
    };
    let mut missing = missing_words(&words, &["slug_dir", "label", "stream", "kind"]);
    if how.is_none() {
        missing.push("--how");
    }
    if saw.is_none() {
        missing.push("--saw-verdict");
    }
    required(&missing)?;
    extra(words.get(4..).unwrap_or(&[]))?;
    Ok(Parsed::Set {
        slug_dir: words[0].clone(),
        args: SetTrack {
            label: words[1].clone(),
            stream: stream.unwrap_or_default(),
            kind: kind.unwrap_or_default(),
            how: how.unwrap_or_default(),
            saw_verdict: saw == Some(Some(true)),
            date: date.unwrap_or_else(|| today.to_string()),
            sure: sure.map_or(true, |s| s == Some(true)),
            film_audible,
            note: note.unwrap_or_default(),
        },
    })
}

fn parse_unset(rest: &[String], today: &str) -> Result<Parsed, Usage> {
    let mut a = Args::new(rest, &["--note", "--date"]);
    let note = a.flag("--note").map_err(sub)?;
    let date = a.flag("--date").map_err(sub)?;
    let words = a.positionals().map_err(top)?;
    let stream = words.get(2).map(|w| int_arg("stream", w)).transpose()?;
    let mut missing = missing_words(&words, &["slug_dir", "label", "stream"]);
    if note.is_none() {
        missing.push("--note");
    }
    required(&missing)?;
    extra(words.get(3..).unwrap_or(&[]))?;
    Ok(Parsed::Unset {
        slug_dir: words[0].clone(),
        label: words[1].clone(),
        stream: stream.unwrap_or_default(),
        note: note.unwrap_or_default(),
        date: date.unwrap_or_else(|| today.to_string()),
    })
}

fn parse_title(rest: &[String]) -> Result<Parsed, Usage> {
    let mut a = Args::new(rest, &["--title-kind", "--dialogue-heavy", "--note"]);
    let title_kind = a.flag("--title-kind").map_err(sub)?;
    let heavy = a.flag("--dialogue-heavy").map_err(sub)?;
    let note = a.flag("--note").map_err(sub)?;
    let words = a.positionals().map_err(top)?;
    let title_kind = title_kind.map(|k| choose("--title-kind", &k, &TITLE_KINDS)).transpose()?;
    let dialogue_heavy = heavy.map(|h| yes("--dialogue-heavy", &h, true)).transpose()?.flatten();
    let mut missing = missing_words(&words, &["slug_dir", "label"]);
    if title_kind.is_none() {
        missing.push("--title-kind");
    }
    required(&missing)?;
    extra(words.get(2..).unwrap_or(&[]))?;
    Ok(Parsed::Title {
        slug_dir: words[0].clone(),
        label: words[1].clone(),
        title_kind: title_kind.unwrap_or_default(),
        dialogue_heavy,
        note: note.unwrap_or_default(),
    })
}

fn parse_auto(rest: &[String], today: &str) -> Result<Parsed, Usage> {
    let mut a = Args::new(rest, &["--date"]);
    let date = a.flag("--date").map_err(sub)?;
    let words = a.positionals().map_err(top)?;
    required(&missing_words(&words, &["slug_dir"]))?;
    extra(words.get(1..).unwrap_or(&[]))?;
    Ok(Parsed::Auto { slug_dir: words[0].clone(), date: date.unwrap_or_else(|| today.to_string()) })
}

fn parse_fitted(rest: &[String], today: &str) -> Result<Parsed, Usage> {
    let mut a = Args::new(rest, &["--rule", "--note", "--date"]);
    let rule = a.flag("--rule").map_err(sub)?;
    let note = a.flag("--note").map_err(sub)?;
    let date = a.flag("--date").map_err(sub)?;
    let words = a.positionals().map_err(top)?;
    let stream = words.get(3).map(|w| int_arg("stream", w)).transpose()?;
    let mut missing = missing_words(&words, &["corpus", "slug"]);
    if rule.is_none() {
        missing.push("--rule");
    }
    required(&missing)?;
    extra(words.get(4..).unwrap_or(&[]))?;
    Ok(Parsed::Fitted {
        corpus: words[0].clone(),
        slug: words[1].clone(),
        label: words.get(2).cloned(),
        stream,
        rule: rule.unwrap_or_default(),
        note: note.unwrap_or_default(),
        date: date.unwrap_or_else(|| today.to_string()),
    })
}

/// Runs one command word. Ok(exit code) after printing its lines; Err(the line for stderr), exit 2.
fn execute(ctx: &Ctx, parsed: Parsed, fitting: &str, out: &mut dyn Write) -> Result<i32, String> {
    match parsed {
        Parsed::Kinds => Ok(0),
        Parsed::Check { paths } => check_paths(ctx, &paths, out, check_lines),
        Parsed::Set { slug_dir, args } => {
            let (tree, file) = open_dir(ctx, &slug_dir, &slug_dir)?;
            let opened = open_for_change(&tree, &PathKey::root())?;
            let fitted = corpus_fitted(ctx, &slug_dir, fitting);
            let none = Fitted::new();
            let outcome = set_track(&opened, &args, fitted.as_ref().unwrap_or(&none))?;
            if let Some(data) = &outcome.write {
                save(&file, data, &opened)?;
            }
            // The Python read the fitted list after writing, and only for a valid entry.
            if outcome.why != "invalid" {
                fitted?;
            }
            let _ = writeln!(out, "{}", set_line(&outcome)?);
            print_left(out, &outcome.left);
            Ok(0)
        }
        Parsed::Unset { slug_dir, label, stream, note, date } => {
            let (tree, file) = open_dir(ctx, &slug_dir, &slug_dir)?;
            let opened = open_for_change(&tree, &PathKey::root())?;
            let (data, left) = unset_track(&opened, &label, stream, &note, &date)?;
            save(&file, &data, &opened)?;
            let _ = writeln!(out, "unset {label} stream {stream}: the label is kept under 'removed'");
            print_left(out, &left);
            Ok(0)
        }
        Parsed::Title { slug_dir, label, title_kind, dialogue_heavy, note } => {
            let (tree, file) = open_dir(ctx, &slug_dir, &slug_dir)?;
            let opened = open_for_change(&tree, &PathKey::root())?;
            let (facts, data, left) = set_title(&opened, &label, &title_kind, dialogue_heavy, &note)?;
            save(&file, &data, &opened)?;
            let _ = writeln!(out, "title {label}: {}, dialogue_heavy={}", py_str(&facts["title_kind"]), py_str(&facts["dialogue_heavy"]));
            print_left(out, &left);
            Ok(0)
        }
        Parsed::Auto { slug_dir, date } => {
            // auto names the folder as pathlib prints it (Path(slug_dir)); set, unset and title as typed.
            let (tree, file) = open_dir(ctx, &slug_dir, &pathlib_str(&slug_dir))?;
            let opened = open_for_change(&tree, &PathKey::root())?;
            let fitted = corpus_fitted(ctx, &slug_dir, fitting);
            let none = Fitted::new();
            let (lines, data) = auto(&tree, &PathKey::root(), &opened, &date, fitted.as_ref().unwrap_or(&none))?;
            if let Some(data) = &data {
                // A fitted list that does not validate stops the run before anything is written (Open point 14).
                fitted?;
                save(&file, data, &opened)?;
            }
            for line in lines {
                let _ = writeln!(out, "{line}");
            }
            Ok(0)
        }
        Parsed::Fitted { corpus, slug, label, stream, rule, note, date } => {
            if label.is_some() != stream.is_some() {
                return Err("fitted: give both <label> and <stream>, or neither (the whole disc)".to_string());
            }
            let dir = ctx.abs(&corpus);
            if !dir.is_dir() {
                return Err(format!("{corpus}: no such folder"));
            }
            let file = dir.join("audio-fitted.json");
            let shown = pathlib_under(&pathlib_str(&corpus), "audio-fitted.json");
            let existing = if file.exists() {
                let bytes = std::fs::read(&file).map_err(|e| os_error_text(&e, &shown))?;
                let text = decode_text(&bytes, Encoding::Utf8Sig, Errors::Strict)?;
                Some(serde_json::from_str::<Value>(&text).map_err(|e| e.to_string())?)
            } else {
                None
            };
            let (entry, data) = add_fitted(existing.as_ref(), &shown, &slug, label.as_deref(), stream, &rule, &note, &date)?;
            write_labels(&file, &data).map_err(|e| os_error_text(&e, &shown))?;
            let what = if py_truthy(&entry["label"]) {
                format!("{}/{} stream {}", py_str(&entry["slug"]), py_str(&entry["label"]), py_str(&entry["stream"]))
            } else {
                format!("{} (the whole disc)", py_str(&entry["slug"]))
            };
            let _ = writeln!(out, "fitted: {what}: no longer counts towards experiment 13");
            Ok(0)
        }
    }
}

/// The recording folder `arg` as a tree printed as pathlib prints the argument, and its labels file's path. Err
/// "<missing_shown>: no such recording folder" (set, unset and title print the argument as typed; auto as pathlib
/// prints it), or the OSError text.
fn open_dir(ctx: &Ctx, arg: &str, missing_shown: &str) -> Result<(DiskTree, PathBuf), String> {
    let dir = ctx.abs(arg);
    if !dir.is_dir() {
        return Err(format!("{missing_shown}: no such recording folder"));
    }
    let shown = pathlib_str(arg);
    let tree = DiskTree::open(&dir, &shown).map_err(|e| os_error_text(&e, &shown))?;
    Ok((tree, dir.join("audio-labels.json")))
}

/// Writes a labels file (indent 2, ensure_ascii off, through a .tmp file and a rename). Py: write_json(path, data, 2).
fn save(file: &Path, data: &Value, opened: &Opened) -> Result<(), String> {
    write_labels(file, data).map_err(|e| format!("{}: {e}", opened.shown))
}

/// "warning: ..." when problems are left.
fn print_left(out: &mut dyn Write, left: &[String]) {
    for line in left_lines(left) {
        let _ = writeln!(out, "{line}");
    }
}

/// The fitted list `set` and `auto` judge against: row 13's disc plus `Path(slug_dir).resolve().parent /
/// "audio-fitted.json"`. Only that one file is read (the parent folder is never walked). Py: corpus_fitted.
fn corpus_fitted(ctx: &Ctx, slug_dir: &str, fitting: &str) -> Result<Fitted, String> {
    let resolved = std::fs::canonicalize(ctx.abs(slug_dir)).map_err(|e| os_error_text(&e, &pathlib_str(slug_dir)))?;
    let corpus = resolved.parent().map_or_else(|| resolved.clone(), Path::to_path_buf);
    let name = corpus.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut tree = MemTree::new(&corpus.display().to_string(), &name);
    let file = corpus.join("audio-fitted.json");
    if file.exists() {
        let bytes = std::fs::read(&file).map_err(|e| os_error_text(&e, &file.display().to_string()))?;
        tree.insert_file("audio-fitted.json", bytes);
    }
    load_fitted(&tree, &PathKey::root(), Some(fitting))
}

/// Row 13's fitting disc from --spec (default: the design spec under the repository root); its errors are the
/// Python's line "cannot read the fitting disc from <spec>: <why>". Py: fitting_slug and main's except.
fn fitting_disc(ctx: &Ctx, flag: Option<&str>) -> Result<String, Fail> {
    let (path, shown) = match flag {
        Some(given) => (ctx.abs(given), given.to_string()),
        None => match default_spec(ctx) {
            Some(path) => {
                let shown = path.display().to_string();
                (path, shown)
            }
            None => return Err(Fail::Usage("the following arguments are required: --spec".to_string())),
        },
    };
    let cannot = |why: String| Fail::Line(format!("cannot read the fitting disc from {shown}: {why}"));
    let bytes = std::fs::read(&path).map_err(|e| cannot(os_error_text(&e, &pathlib_str(&shown))))?;
    let text = decode_text(&bytes, Encoding::Utf8Sig, Errors::Strict).map_err(&cannot)?;
    exp13_requirements(&text).map(|req| req.fitting_slug).map_err(&cannot)
}

/// The line `set` prints, from the entry written or kept: its values as Python's str(), the two flags by truth.
/// Err "KeyError: '<key>'" where the Python's line raised (an unchanged entry without a key it prints).
fn set_line(outcome: &SetOutcome) -> Result<String, String> {
    let empty = serde_json::Map::new();
    let e = outcome.entry.as_object().unwrap_or(&empty);
    let field = |key: &str| e.get(key).ok_or_else(|| format!("KeyError: {}", py_repr_str(key)));
    let yes_no = |value: &Value| if py_truthy(value) { "yes" } else { "no" };
    let mut line = format!(
        "{} {} stream {}: {} ({}, {}, saw verdict {}, sure {}), counts={}",
        if outcome.changed { "labelled" } else { "unchanged" },
        py_str(field("label")?),
        py_str(field("stream")?),
        py_str(field("kind")?),
        py_str(field("how")?),
        py_str(field("date")?),
        yes_no(field("saw_verdict")?),
        yes_no(field("sure")?),
        if outcome.why.is_empty() { "True" } else { "False" },
    );
    if !outcome.why.is_empty() {
        line.push_str(&format!(" ({})", outcome.why));
    }
    if outcome.changed {
        if let Some(history) = e.get("history").filter(|h| py_truthy(h)) {
            line.push_str(&format!("; replaces {} earlier label(s), kept in its history", history.as_array().map_or(0, Vec::len)));
        }
    }
    Ok(line)
}

/// argparse's list of choices: each repr'd, joined by ", ".
fn choices(list: &[&str]) -> String {
    list.iter().map(|choice| py_repr_str(choice)).collect::<Vec<_>>().join(", ")
}

/// A value that must be one of `list`. Py: argparse's choices.
fn choose(name: &str, value: &str, list: &[&str]) -> Result<String, Usage> {
    if list.contains(&value) {
        Ok(value.to_string())
    } else {
        Err(sub(format!("argument {name}: invalid choice: {} (choose from {})", py_repr_str(value), choices(list))))
    }
}

/// `type=int`. Py: argparse's "invalid int value".
fn int_arg(name: &str, value: &str) -> Result<i64, Usage> {
    py_int(value).ok_or_else(|| sub(format!("argument {name}: invalid int value: {}", py_repr_str(value))))
}

/// `type=_yes`. Py: argparse's ArgumentTypeError message.
fn yes(name: &str, value: &str, allow_unknown: bool) -> Result<Option<bool>, Usage> {
    yes_no_unknown(value, allow_unknown).map_err(|message| sub(format!("argument {name}: {message}")))
}

/// argparse's "the following arguments are required: ...".
fn required(missing: &[&str]) -> Result<(), Usage> {
    if missing.is_empty() {
        Ok(())
    } else {
        Err(sub(format!("the following arguments are required: {}", missing.join(", "))))
    }
}

/// argparse's "unrecognized arguments: ...", from the main parser.
fn extra(words: &[String]) -> Result<(), Usage> {
    if words.is_empty() {
        Ok(())
    } else {
        Err(top(format!("unrecognized arguments: {}", words.join(" "))))
    }
}

/// The positional names `words` does not reach.
fn missing_words<'a>(words: &[String], names: &[&'a str]) -> Vec<&'a str> {
    names.iter().skip(words.len()).copied().collect()
}

/// `check`'s loop over its paths, for the audio labels (`execute`) and the subtitle labels (`execute_subtitles`): each
/// path is read through a DiskTree when it is a folder, else through an empty tree (so `lines` finds no file), and
/// `lines`' lines for it are printed. Ok(2) when a file is bad, else Ok(0); Err(the OSError text) when a folder cannot
/// be opened.
fn check_paths(
    ctx: &Ctx,
    paths: &[String],
    out: &mut dyn Write,
    lines: fn(&dyn Tree, &PathKey) -> (Vec<String>, usize),
) -> Result<i32, String> {
    let mut bad = 0;
    for path in paths {
        let shown = pathlib_str(path);
        let dir = ctx.abs(path);
        let (printed, found) = if dir.is_dir() {
            let tree = DiskTree::open(&dir, &shown).map_err(|e| os_error_text(&e, &shown))?;
            lines(&tree, &PathKey::root())
        } else {
            lines(&MemTree::new(&shown, ""), &PathKey::root())
        };
        for line in printed {
            let _ = writeln!(out, "{line}");
        }
        bad += found;
    }
    Ok(if bad > 0 { 2 } else { 0 })
}

/// `labels --subtitles <cmd> <arguments>` (RP15): the owner's subtitle labels through `core::subtitle::labels`. No spec is
/// read. Exit 0, or 2 for an argument error, a recording folder that does not exist, a file that is not JSON or does
/// not validate (`check`), and a change that would add a problem.
fn run_subtitles(ctx: &Ctx, cmd: &str, rest: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    if rest.iter().any(|word| word == "-h" || word == "--help") {
        let _ = writeln!(out, "usage: lazymkv-dev {}", subtitle_usage(cmd));
        return 0;
    }
    let parsed = match parse_subtitles(cmd, rest, &ctx.today()) {
        Ok(parsed) => parsed,
        Err(Usage { sub: true, message }) => return usage_error(err, &format!("labels {cmd}"), subtitle_usage(cmd), &message),
        Err(Usage { sub: false, message }) => return usage_error(err, "labels", USAGE, &message),
    };
    match execute_subtitles(ctx, parsed, out) {
        Ok(code) => code,
        Err(line) => {
            let _ = writeln!(err, "{line}");
            2
        }
    }
}

/// A `--subtitles` command line, parsed and converted as argparse converts it.
enum SubtitleParsed {
    Kinds,
    Check { paths: Vec<String> },
    Set { slug_dir: String, label: SubtitleLabel },
    Unset { slug_dir: String, label: String, stream: i64, note: String, date: String },
}

/// The command word's arguments under `--subtitles`; `unset` takes the audio `unset`'s arguments.
fn parse_subtitles(cmd: &str, rest: &[String], today: &str) -> Result<SubtitleParsed, Usage> {
    match cmd {
        "kinds" => {
            extra(rest)?;
            Ok(SubtitleParsed::Kinds)
        }
        "check" => {
            let paths = Args::new(rest, &[]).positionals().map_err(top)?;
            required(&missing_words(&paths, &["paths"]))?;
            Ok(SubtitleParsed::Check { paths })
        }
        "set" => parse_subtitle_set(rest, today),
        _ => match parse_unset(rest, today)? {
            Parsed::Unset { slug_dir, label, stream, note, date } => Ok(SubtitleParsed::Unset { slug_dir, label, stream, note, date }),
            _ => unreachable!("parse_unset gives only Parsed::Unset"),
        },
    }
}

/// `set <slug_dir> <label> <stream> <kind> --how HOW --saw-verdict yes|no [--sure yes|no] [--date YYYY-MM-DD] [--note
/// TEXT] [--style uk]`: `--saw-verdict` is required, as the audio `set`'s (section 17 row 14 (a)); `--sure` defaults to
/// yes, `--date` to today, `--note` and `--style` to none (null in the file; `--style uk` marks a UK-style SDH track,
/// RP25). Errors in the order of the audio `set`:
/// type and choice errors (positionals, then options), the missing required arguments, unrecognized arguments.
fn parse_subtitle_set(rest: &[String], today: &str) -> Result<SubtitleParsed, Usage> {
    let mut a = Args::new(rest, &["--how", "--saw-verdict", "--sure", "--date", "--note", "--style"]);
    let how = a.flag("--how").map_err(sub)?;
    let saw = a.flag("--saw-verdict").map_err(sub)?;
    let sure = a.flag("--sure").map_err(sub)?;
    let date = a.flag("--date").map_err(sub)?;
    let note = a.flag("--note").map_err(sub)?;
    let style = a.flag("--style").map_err(sub)?;
    let words = a.positionals().map_err(top)?;
    let kinds: Vec<&str> = SubtitleKind::ALL.iter().map(|kind| kind.as_str()).collect();
    let hows: Vec<&str> = subtitle_labels::HOW.iter().map(|(name, _)| *name).collect();
    let stream = words.get(2).map(|w| int_arg("stream", w)).transpose()?;
    let kind = words.get(3).map(|w| choose("kind", w, &kinds)).transpose()?;
    let how = how.map(|h| choose("--how", &h, &hows)).transpose()?;
    let saw = saw.map(|s| yes("--saw-verdict", &s, false)).transpose()?;
    let sure = sure.map(|s| yes("--sure", &s, false)).transpose()?;
    let style = style.map(|s| choose("--style", &s, &["uk"])).transpose()?;
    let mut missing = missing_words(&words, &["slug_dir", "label", "stream", "kind"]);
    if how.is_none() {
        missing.push("--how");
    }
    if saw.is_none() {
        missing.push("--saw-verdict");
    }
    required(&missing)?;
    extra(words.get(4..).unwrap_or(&[]))?;
    let kind = SubtitleKind::from_str(&kind.unwrap_or_default()).map_err(sub)?;
    Ok(SubtitleParsed::Set {
        slug_dir: words[0].clone(),
        label: SubtitleLabel {
            label: words[1].clone(),
            stream: stream.unwrap_or_default(),
            kind,
            how: how.unwrap_or_default(),
            sure: sure.map_or(true, |s| s == Some(true)),
            date: date.unwrap_or_else(|| today.to_string()),
            saw_verdict: saw == Some(Some(true)),
            note,
            style,
        },
    })
}

/// Runs one `--subtitles` command word. Ok(exit code) after printing its lines; Err(the line for stderr), exit 2.
fn execute_subtitles(ctx: &Ctx, parsed: SubtitleParsed, out: &mut dyn Write) -> Result<i32, String> {
    match parsed {
        SubtitleParsed::Kinds => {
            for line in subtitle_kinds_lines() {
                let _ = writeln!(out, "{line}");
            }
            Ok(0)
        }
        SubtitleParsed::Check { paths } => check_paths(ctx, &paths, out, subtitle_check_lines),
        SubtitleParsed::Set { slug_dir, label } => {
            let SubtitleFile { path, shown, data } = open_subtitles(ctx, &slug_dir)?;
            let (written, left) = subtitle_labels::set_track(data.clone(), &label).map_err(|e| format!("{shown}: {e}"))?;
            let changed = written != data;
            if changed {
                write_labels(&path, &written).map_err(|e| format!("{shown}: {e}"))?;
            }
            let _ = writeln!(out, "{}", subtitle_set_line(&written, &label, changed));
            print_left(out, &left);
            Ok(0)
        }
        SubtitleParsed::Unset { slug_dir, label, stream, note, date } => {
            let SubtitleFile { path, shown, data } = open_subtitles(ctx, &slug_dir)?;
            let (written, left) =
                subtitle_labels::unset_track(data, &label, stream, &note, &date).map_err(|e| format!("{shown}: {e}"))?;
            write_labels(&path, &written).map_err(|e| format!("{shown}: {e}"))?;
            let _ = writeln!(out, "unset {label} stream {stream}: the label is kept under 'removed'");
            print_left(out, &left);
            Ok(0)
        }
    }
}

/// A recording's subtitle labels file opened for a change: its path on disk, how it is printed, and its value (a new
/// file when absent: the format line, the folder's resolved name as slug, no tracks).
struct SubtitleFile {
    path: PathBuf,
    shown: String,
    data: Value,
}

/// Opens `<slug_dir>/subtitle-labels.json` for a change. A file that does not validate can still be changed (the
/// edit may not add a problem), but it must be JSON. Err "<slug_dir>: no such recording folder", "<shown>: not valid
/// JSON (<the reason>); restore it from git (git checkout -- <shown>)", or the OSError text.
fn open_subtitles(ctx: &Ctx, slug_dir: &str) -> Result<SubtitleFile, String> {
    let dir = ctx.abs(slug_dir);
    if !dir.is_dir() {
        return Err(format!("{slug_dir}: no such recording folder"));
    }
    let folder = pathlib_str(slug_dir);
    let shown = pathlib_under(&folder, subtitle_labels::FILE_NAME);
    let path = dir.join(subtitle_labels::FILE_NAME);
    if !path.exists() {
        let tree = DiskTree::open(&dir, &folder).map_err(|e| os_error_text(&e, &folder))?;
        let data = json!({"format": subtitle_labels::FORMAT, "slug": tree.resolved_name(&PathKey::root()), "tracks": []});
        return Ok(SubtitleFile { path, shown, data });
    }
    let bytes = std::fs::read(&path).map_err(|e| os_error_text(&e, &shown))?;
    let parsed = decode_text(&bytes, Encoding::Utf8Sig, Errors::Strict)
        .and_then(|text| serde_json::from_str::<Value>(&text).map_err(|e| e.to_string()));
    match parsed {
        Ok(data) => Ok(SubtitleFile { path, shown, data }),
        Err(why) => Err(format!("{shown}: not valid JSON ({why}); restore it from git (git checkout -- {shown})")),
    }
}

/// The line `--subtitles set` prints, from the entry written or kept: `labelled|unchanged <label> stream <n>: <kind>
/// (<how>, <date>, saw verdict yes|no, sure yes|no[, style <style>])` (the style only when it is not null, RP25), then for
/// an observation (D23) why it does not count, then after a replacement how
/// many earlier labels its history keeps.
fn subtitle_set_line(written: &Value, label: &SubtitleLabel, changed: bool) -> String {
    let entry = written["tracks"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|t| t["label"].as_str() == Some(label.label.as_str()) && t["stream"].as_i64() == Some(label.stream))
        .cloned()
        .unwrap_or(Value::Null);
    let how = py_str(&entry["how"]);
    let style = entry["style"].as_str().map_or(String::new(), |style| format!(", style {style}"));
    let mut line = format!(
        "{} {} stream {}: {} ({how}, {}, saw verdict {}, sure {}{style})",
        if changed { "labelled" } else { "unchanged" },
        label.label,
        label.stream,
        py_str(&entry["kind"]),
        py_str(&entry["date"]),
        if py_truthy(&entry["saw_verdict"]) { "yes" } else { "no" },
        if py_truthy(&entry["sure"]) { "yes" } else { "no" },
    );
    if subtitle_labels::OBSERVATION_HOW.contains(&how.as_str()) {
        line.push_str(&format!(" ('{how}' is an observation, not a check of the track's text (row 14, D23))"));
    }
    let replaced = entry.get("history").and_then(Value::as_array).map_or(0, Vec::len);
    if changed && replaced > 0 {
        line.push_str(&format!("; replaces {replaced} earlier label(s), kept in its history"));
    }
    line
}

/// `labels --subtitles kinds`' lines: each kind, what the owner wants and its meaning (padded to 16 and 8
/// characters), then the sources.
fn subtitle_kinds_lines() -> Vec<String> {
    let mut lines: Vec<String> =
        subtitle_labels::KINDS.iter().map(|(kind, wanted, meaning)| format!("{kind:<16} {wanted:<8} {meaning}")).collect();
    let how: Vec<String> = subtitle_labels::HOW.iter().map(|(name, meaning)| format!("{name} = {meaning}")).collect();
    lines.push(format!("how: {}", how.join("; ")));
    lines
}

/// `labels --subtitles check <root>`'s lines for one root and the number of bad files: root/subtitle-labels.json when
/// it exists, else every */subtitle-labels.json (never one under golden/), each "ok <file> (<n> tracks)" or `load`'s
/// refusal; "<root>: no subtitle-labels.json" when there is none. The audio labels' `labels_check_lines`, with the
/// subtitle file's name and count (RP15).
fn subtitle_check_lines(tree: &dyn Tree, root: &PathKey) -> (Vec<String>, usize) {
    labels_check_lines(tree, root, subtitle_labels::FILE_NAME, &|tree, path| {
        subtitle_labels::load(tree, path).map(|labels| format!("{} tracks", labels.len()))
    })
}

/// The synopsis of one `--subtitles` command word (after "usage: lazymkv-dev ").
fn subtitle_usage(cmd: &str) -> &'static str {
    match cmd {
        "kinds" => "labels --subtitles kinds",
        "check" => "labels --subtitles check <path> ...",
        "set" => {
            "labels --subtitles set <slug_dir> <label> <stream> <kind> --how HOW --saw-verdict yes|no [--sure yes|no] [--date YYYY-MM-DD] [--note TEXT] [--style uk]"
        }
        _ => "labels --subtitles unset <slug_dir> <label> <stream> --note TEXT [--date YYYY-MM-DD]",
    }
}

#[cfg(test)]
mod tests;
