//! lazymkv-dev: the phase 0 analysers as commands over recordings (port spec section 3.4). The library holds every
//! command and what they share, so that the integration tests under crates/dev/tests can call them; src/main.rs
//! hands the process's arguments, environment and streams to `dispatch`.

pub mod cmd;
pub mod evidence;
pub mod pyjson;
pub mod runner;

use evidence::Ctx;
use lazymkv_core::recording::py_repr_str;
use std::io::Write;

/// Every command, in every cmd/<name>.rs: `pub fn run(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut
/// dyn Write) -> i32`. `args` excludes the command name; the return value is the exit code of spec 3.4; stdout lines
/// go to `out`, error lines to `err`.
pub type Command = fn(&Ctx, &[String], &mut dyn Write, &mut dyn Write) -> i32;

/// (name as typed, the command, its arguments as spec 3.4 writes them after the name), in spec 3.4's order. Each of
/// Tasks 2 to 14 adds its row, for example `("scan", cmd::scan::run as Command, "<slug|dir> [--title N] [--label L]")`.
pub const COMMANDS: &[(&str, Command, &str)] = &[];

const USAGE: &str = "usage: lazymkv-dev <command> [arguments]";

/// `argv[0]` picks the command (`argv` excludes the program name); `--help` or `-h` prints the table to `out` and
/// returns 0; no argument prints it to `err` and returns 2; an unknown name prints "lazymkv-dev: unknown command
/// '<name>'" and the table to `err` and returns 2.
pub fn dispatch(ctx: &Ctx, argv: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let Some(name) = argv.first() else {
        help(err);
        return 2;
    };
    if name == "--help" || name == "-h" {
        help(out);
        return 0;
    }
    match COMMANDS.iter().find(|(command, _, _)| *command == name.as_str()) {
        Some((_, run, _)) => run(ctx, &argv[1..], out, err),
        None => {
            let _ = writeln!(err, "lazymkv-dev: unknown command {}", py_repr_str(name));
            help(err);
            2
        }
    }
}

/// The usage line, a blank line, "commands:" and one "  <name> <arguments>" line per command.
fn help(w: &mut dyn Write) {
    let _ = writeln!(w, "{}", USAGE);
    let _ = writeln!(w);
    let _ = writeln!(w, "commands:");
    if COMMANDS.is_empty() {
        let _ = writeln!(w, "  (none yet)");
    }
    for (name, _, synopsis) in COMMANDS {
        let _ = writeln!(w, "  {} {}", name, synopsis);
    }
}

/// Hand-written argument parsing with argparse's spellings: `--name VALUE`, `--name=VALUE`, a unique prefix of a
/// declared long flag (argparse's allow_abbrev), repeatable flags, switches, positionals, and `--` before words that
/// would look like flags. A command declares its flags, takes each with `flag`, `flags` or `switch`, then the rest
/// with `positionals`. A value may be a negative number ("-5", "-2.5") or "-", as argparse allows.
pub struct Args {
    words: Vec<String>,
    declared: Vec<String>,
}

/// What one word is to the parser.
enum Resolved {
    /// A positional word (or a flag-like word no declared flag matches).
    Word,
    /// A declared flag (its full name) and the value written after '=', if any.
    Flag(String, Option<String>),
    /// A prefix of several declared flags (the word as written and the flags it matches).
    Ambiguous(String, Vec<String>),
}

impl Args {
    /// The words after the command name; `flags` declares the long flags the command knows (with "--").
    pub fn new(args: &[String], flags: &[&str]) -> Args {
        Args { words: args.to_vec(), declared: flags.iter().map(|flag| flag.to_string()).collect() }
    }

    /// The value of a single-use flag (the last one given wins, as argparse's store); Err("argument --name: expected
    /// one argument") when it has none.
    pub fn flag(&mut self, name: &str) -> Result<Option<String>, String> {
        Ok(self.take(name)?.pop())
    }

    /// Every value of a repeatable flag (--set), in order.
    pub fn flags(&mut self, name: &str) -> Result<Vec<String>, String> {
        self.take(name)
    }

    /// Whether a switch (--allow-unmeasured) was given.
    pub fn switch(&mut self, name: &str) -> bool {
        let mut found = false;
        let mut at = 0;
        while at < self.words.len() && self.words[at] != "--" {
            match self.resolve(&self.words[at]) {
                Resolved::Flag(flag, None) if flag == name => {
                    self.words.remove(at);
                    found = true;
                }
                _ => at += 1,
            }
        }
        found
    }

    /// The words left, in order; a lone "--" is dropped and every word after it is positional. Err("ambiguous
    /// option: --s could match --set, --snapshot") for an ambiguous prefix, else Err("unrecognized arguments: ...")
    /// for flag-like words no call took.
    pub fn positionals(self) -> Result<Vec<String>, String> {
        let mut out = Vec::new();
        let mut unknown = Vec::new();
        let mut after_dashes = false;
        for word in &self.words {
            if after_dashes {
                out.push(word.clone());
            } else if word == "--" {
                after_dashes = true;
            } else if is_value(word) {
                out.push(word.clone());
            } else if let Resolved::Ambiguous(option, matches) = self.resolve(word) {
                return Err(format!("ambiguous option: {} could match {}", option, matches.join(", ")));
            } else {
                unknown.push(word.clone());
            }
        }
        if unknown.is_empty() {
            Ok(out)
        } else {
            Err(format!("unrecognized arguments: {}", unknown.join(" ")))
        }
    }

    /// Removes every occurrence of flag `name` before a "--", with its value, and returns the values in order.
    fn take(&mut self, name: &str) -> Result<Vec<String>, String> {
        let mut values = Vec::new();
        let mut at = 0;
        while at < self.words.len() && self.words[at] != "--" {
            match self.resolve(&self.words[at]) {
                Resolved::Flag(flag, inline) if flag == name => {
                    self.words.remove(at);
                    match inline {
                        Some(value) => values.push(value),
                        None if at < self.words.len() && is_value(&self.words[at]) => values.push(self.words.remove(at)),
                        None => return Err(format!("argument {}: expected one argument", name)),
                    }
                }
                _ => at += 1,
            }
        }
        Ok(values)
    }

    fn resolve(&self, word: &str) -> Resolved {
        if !word.starts_with("--") || word == "--" {
            return Resolved::Word;
        }
        let (option, inline) = match word.split_once('=') {
            Some((option, value)) => (option, Some(value.to_string())),
            None => (word, None),
        };
        if self.declared.iter().any(|flag| flag == option) {
            return Resolved::Flag(option.to_string(), inline);
        }
        let matches: Vec<String> = self.declared.iter().filter(|flag| flag.starts_with(option)).cloned().collect();
        match matches.len() {
            0 => Resolved::Word,
            1 => Resolved::Flag(matches[0].clone(), inline),
            _ => Resolved::Ambiguous(option.to_string(), matches),
        }
    }
}

/// Whether argparse takes `word` as a value: it does not start with '-', is "-" alone, or is a negative number
/// ("-5", "-.5", "-2.5").
fn is_value(word: &str) -> bool {
    if !word.starts_with('-') || word == "-" {
        return true;
    }
    let rest = &word[1..];
    match rest.split_once('.') {
        None => !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()),
        Some((whole, fraction)) => {
            !fraction.is_empty() && whole.bytes().all(|b| b.is_ascii_digit()) && fraction.bytes().all(|b| b.is_ascii_digit())
        }
    }
}

/// Prints "usage: lazymkv-dev <usage>" and "lazymkv-dev <cmd>: error: <message>" to `err`, returns 2 (argparse's
/// exit). `usage` starts with the command name, as argparse's usage line does.
pub fn usage_error(err: &mut dyn Write, cmd: &str, usage: &str, message: &str) -> i32 {
    let _ = writeln!(err, "usage: lazymkv-dev {}", usage);
    let _ = writeln!(err, "lazymkv-dev {}: error: {}", cmd, message);
    2
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn words(text: &[&str]) -> Vec<String> {
        text.iter().map(|word| word.to_string()).collect()
    }

    #[test]
    fn cli_args_take_values_in_argparse_spellings() {
        let mut args = Args::new(&words(&["slug", "--title", "3", "--label=all-tracks"]), &["--title", "--label"]);
        assert_eq!(args.flag("--title"), Ok(Some("3".to_string())));
        assert_eq!(args.flag("--label"), Ok(Some("all-tracks".to_string())));
        assert_eq!(args.positionals(), Ok(vec!["slug".to_string()]));
    }

    #[test]
    fn cli_args_accept_a_unique_prefix_repeats_and_switches() {
        let mut args = Args::new(
            &words(&["--set", "DC_MAX=0.25", "--set=SILENT_DB=-80", "--allow", "--snap", "out.json"]),
            &["--set", "--snapshot", "--allow-unmeasured"],
        );
        assert_eq!(args.flags("--set"), Ok(vec!["DC_MAX=0.25".to_string(), "SILENT_DB=-80".to_string()]));
        assert!(args.switch("--allow-unmeasured"));
        assert_eq!(args.flag("--snapshot"), Ok(Some("out.json".to_string())));
        assert_eq!(args.positionals(), Ok(Vec::<String>::new()));
        let mut twice = Args::new(&words(&["--title", "1", "--title", "2"]), &["--title"]);
        assert_eq!(twice.flag("--title"), Ok(Some("2".to_string())));
        let mut none = Args::new(&words(&["x"]), &["--title", "--allow-unmeasured"]);
        assert_eq!(none.flag("--title"), Ok(None));
        assert!(!none.switch("--allow-unmeasured"));
    }

    #[test]
    fn cli_args_report_missing_values_unknown_words_and_ambiguity() {
        let mut args = Args::new(&words(&["--title"]), &["--title"]);
        assert_eq!(args.flag("--title"), Err("argument --title: expected one argument".to_string()));
        let mut args = Args::new(&words(&["--title", "--label", "x"]), &["--title", "--label"]);
        assert_eq!(args.flag("--title"), Err("argument --title: expected one argument".to_string()));
        let args = Args::new(&words(&["a", "--bogus", "-x"]), &["--title"]);
        assert_eq!(args.positionals(), Err("unrecognized arguments: --bogus -x".to_string()));
        let args = Args::new(&words(&["--s", "1"]), &["--set", "--snapshot"]);
        assert_eq!(args.positionals(), Err("ambiguous option: --s could match --set, --snapshot".to_string()));
    }

    #[test]
    fn cli_args_take_negative_numbers_and_words_after_a_double_dash() {
        let mut args = Args::new(&words(&["--main", "-5", "-", "--", "--title"]), &["--main", "--title"]);
        assert_eq!(args.flag("--main"), Ok(Some("-5".to_string())));
        assert_eq!(args.flag("--title"), Ok(None));
        assert_eq!(args.positionals(), Ok(vec!["-".to_string(), "--title".to_string()]));
    }

    #[test]
    fn cli_help_prints_the_command_table_to_stdout() {
        let ctx = evidence::Ctx::for_test(Path::new("."));
        let (mut out, mut err): (Vec<u8>, Vec<u8>) = (Vec::new(), Vec::new());
        assert_eq!(dispatch(&ctx, &words(&["--help"]), &mut out, &mut err), 0);
        let text = String::from_utf8(out).unwrap();
        assert!(text.starts_with("usage: lazymkv-dev <command> [arguments]\n\ncommands:\n"), "{}", text);
        if COMMANDS.is_empty() {
            assert!(text.ends_with("commands:\n  (none yet)\n"), "{}", text);
        }
        for (name, _, synopsis) in COMMANDS {
            assert!(text.contains(&format!("  {} {}\n", name, synopsis)), "{}", text);
        }
        assert!(err.is_empty());
    }

    #[test]
    fn cli_no_or_an_unknown_command_prints_the_table_to_stderr_and_exits_2() {
        let ctx = evidence::Ctx::for_test(Path::new("."));
        let (mut out, mut err): (Vec<u8>, Vec<u8>) = (Vec::new(), Vec::new());
        assert_eq!(dispatch(&ctx, &[], &mut out, &mut err), 2);
        assert!(String::from_utf8(err).unwrap().starts_with("usage: lazymkv-dev <command> [arguments]\n"));
        let (mut out2, mut err2): (Vec<u8>, Vec<u8>) = (Vec::new(), Vec::new());
        assert_eq!(dispatch(&ctx, &words(&["nonsense"]), &mut out2, &mut err2), 2);
        assert!(String::from_utf8(err2)
            .unwrap()
            .starts_with("lazymkv-dev: unknown command 'nonsense'\nusage: lazymkv-dev <command> [arguments]\n"));
        assert!(out.is_empty() && out2.is_empty());
    }

    #[test]
    fn cli_usage_error_prints_the_argparse_lines_and_returns_2() {
        let mut err: Vec<u8> = Vec::new();
        let code = usage_error(
            &mut err,
            "scan",
            "scan <slug|dir> [--title N] [--label L]",
            "the following arguments are required: slug",
        );
        assert_eq!(code, 2);
        assert_eq!(
            String::from_utf8(err).unwrap(),
            "usage: lazymkv-dev scan <slug|dir> [--title N] [--label L]\n\
             lazymkv-dev scan: error: the following arguments are required: slug\n"
        );
    }
}
