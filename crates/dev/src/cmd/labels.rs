//! `lazymkv-dev labels [--spec FILE] kinds|check|set|unset|title|auto|fitted ...`: the owner's audio labels,
//! `<slug_dir>/audio-labels.json`, and the fitted list, `<corpus>/audio-fitted.json` (decision D16). Paths are taken
//! as given and no evidence check is made (spec 3.4). The sentences, the files written and the exit codes are the
//! Python's: 0, or 2 for an argument error, an unreadable spec, a file that does not validate and a change that would
//! add a problem; where the Python crashed with a traceback the command prints one line and exits 2 (spec 3.4).
//! `auto` applies its labels to one value and writes the file once (the Python saved after each label). `--spec`
//! (default: the design spec under the repository root) comes before the command word, as argparse's main option.
//! Py: audio_labels.main.

use std::io::Write;
use std::path::{Path, PathBuf};

use lazymkv_core::audio::labels::{
    add_fitted, auto, check_lines, exp13_requirements, kinds_lines, left_lines, load_fitted, open_for_change, set_title,
    set_track, unset_track, yes_no_unknown, FilmAudible, Fitted, Opened, SetOutcome, SetTrack, HOW, KINDS, TITLE_KINDS,
};
use lazymkv_core::audio::rule::RULE_VERSION;
use lazymkv_core::recording::{
    decode_text, pathlib_under, py_int, py_repr_str, py_str, py_truthy, Encoding, Errors, MemTree, PathKey,
};
use serde_json::Value;

use crate::evidence::{default_spec, pathlib_str, Ctx, DiskTree};
use crate::pyjson::write_labels;
use crate::{usage_error, Args};

/// The synopsis after "usage: lazymkv-dev ". The COMMANDS row (crates/dev/src/lib.rs) holds it without the leading "labels".
const USAGE: &str = "labels [--spec FILE] kinds|check|set|unset|title|auto|fitted ...";
/// The command words, in the Python's order (argparse's choices for `cmd`).
const SUBCOMMANDS: [&str; 7] = ["kinds", "check", "set", "unset", "title", "auto", "fitted"];

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
    let (spec, cmd, rest) = match split_main(args) {
        Ok(Some(parts)) => parts,
        Ok(None) => {
            let _ = writeln!(out, "usage: lazymkv-dev {USAGE}");
            return 0;
        }
        Err(message) => return usage_error(err, "labels", USAGE, &message),
    };
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

/// The main parser's part of the command line: `--spec FILE` (also `--spec=FILE`, or a prefix of it of at least
/// three characters, as argparse's abbreviations) and `-h`/`--help` before the command word, then the command word.
/// Ok(None) asks for the help; Err is argparse's message.
fn split_main(args: &[String]) -> Result<Option<(Option<String>, String, Vec<String>)>, String> {
    let mut spec = None;
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
        if name.len() < 3 || !"--spec".starts_with(name) {
            return Err(format!("unrecognized arguments: {word}"));
        }
        spec = Some(match inline {
            Some(value) => value,
            None => {
                i += 1;
                args.get(i).cloned().ok_or_else(|| "argument --spec: expected one argument".to_string())?
            }
        });
        i += 1;
    }
    let cmd = args.get(i).cloned().ok_or_else(|| "the following arguments are required: cmd".to_string())?;
    if !SUBCOMMANDS.contains(&cmd.as_str()) {
        return Err(format!("argument cmd: invalid choice: {} (choose from {})", py_repr_str(&cmd), choices(&SUBCOMMANDS)));
    }
    Ok(Some((spec, cmd, args[i + 1..].to_vec())))
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
        Parsed::Check { paths } => {
            let mut bad = 0;
            for path in &paths {
                let shown = pathlib_str(path);
                let dir = ctx.abs(path);
                let (lines, found) = if dir.is_dir() {
                    let tree = DiskTree::open(&dir, &shown).map_err(|e| os_error_text(&e, &shown))?;
                    check_lines(&tree, &PathKey::root())
                } else {
                    check_lines(&MemTree::new(&shown, ""), &PathKey::root())
                };
                for line in lines {
                    let _ = writeln!(out, "{line}");
                }
                bad += found;
            }
            Ok(if bad > 0 { 2 } else { 0 })
        }
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

/// str(OSError) as Python prints it: "[Errno 2] No such file or directory: 'x'" (the strerror text without Rust's
/// " (os error N)"); the plain text when the error has no OS code.
fn os_error_text(e: &std::io::Error, shown: &str) -> String {
    match e.raw_os_error() {
        Some(code) => {
            let text = e.to_string();
            let suffix = format!(" (os error {code})");
            let reason = text.strip_suffix(&suffix).unwrap_or(&text);
            format!("[Errno {code}] {reason}: {}", py_repr_str(shown))
        }
        None => e.to_string(),
    }
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

#[cfg(test)]
mod tests;
