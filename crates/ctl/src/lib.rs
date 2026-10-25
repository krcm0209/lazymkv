//! lazymkv-ctl: lazymkvctl's commands. plan, decide and retune run locally through lazymkv_dev::dispatch; the rest go to the engine over IPC (design parts 4.1, 9.5, 9.6).

use lazymkv_dev::evidence::{default_spec, Ctx};
use lazymkv_dev::{Args, COMMANDS};
use lazymkv_platform::IpcConnector;
use std::io::{Read, Write};
use std::path::PathBuf;

/// What lazymkvctl reads and writes: the dev context, the engine connector (None in PR 2's binary, S38), and the
/// three streams (design parts 9.5, 9.6). Def: T27. Use: T27, T28.
pub struct Env<'a> {
    /// The working folder, environment and runner that the local commands run over.
    pub ctx: &'a Ctx,
    /// The way to the engine; None when the build has no transport, which is every build in PR 2.
    pub connector: Option<&'a dyn IpcConnector>,
    /// Standard input, which `secret set` reads its value from.
    pub stdin: &'a mut dyn Read,
    /// Standard output.
    pub out: &'a mut dyn Write,
    /// Standard error.
    pub err: &'a mut dyn Write,
}

/// The commands lazymkvctl runs itself through lazymkv-dev's dispatcher (design part 9.5, P23). Def: T27. Use: T27, T28.
pub const LOCAL_COMMANDS: [&str; 3] = ["plan", "decide", "retune"];

/// The message of exit 4 when this build has no engine transport (design part 4.2, S38). Def: T27. Use: T27, T28.
pub const NO_TRANSPORT: &str = "lazymkvctl: no engine transport exists in this build";

/// The message of exit 4 when no engine is running (design part 9.6). Def: T27. Use: T28.
pub const NOT_RUNNING: &str = "lazymkvctl: no engine is running";

/// The message of exit 4 when the engine turned the connection away (design part 9.6). Def: T27. Use: T28.
pub const REFUSED: &str = "lazymkvctl: the engine refused the connection";

/// The message of exit 4 when only an elevated client may reach the engine (design part 9.6). Def: T27. Use: T28.
pub const NOT_ELEVATED: &str = "lazymkvctl: run lazymkvctl elevated to reach the engine";

/// retune's eight flags (crates/dev/src/cmd/retune.rs:38) plus "--jobs", for reading a relayed retune's words.
/// Design part 9.6. Def: T27. Use: T27, T28.
pub(crate) const RETUNE_RELAY_FLAGS: [&str; 9] = [
    "--evidence",
    "--set",
    "--snapshot",
    "--compare",
    "--spec",
    "--allow-unmeasured",
    "--subtitles",
    "--text",
    "--jobs",
];

/// The words of `lazymkvctl --help`; Task 28 adds the engine commands (S88, design parts 9.5, 9.6).
const USAGE: &str = "\
usage: lazymkvctl <command> [arguments]

commands run here (design part 9.5):
  plan <slug|recording dir|scan file> [lazymkv-dev plan's flags]   or   plan --job ID
  decide (<slug> <label> | --rec DIR) [lazymkv-dev decide's flags]   or   decide --job ID --title KEY
  retune [lazymkv-dev retune's flags]   or   retune --jobs [--spec FILE]
";

/// lazymkvctl's entry: `argv` excludes the program name; returns the exit code of design part 9.6. Def: T27. Use: T27, T28.
pub fn run(argv: &[String], env: Env) -> i32 {
    let Some(name) = argv.first() else {
        let _ = env.err.write_all(USAGE.as_bytes());
        return 2;
    };
    if name == "--help" || name == "-h" {
        let _ = env.out.write_all(USAGE.as_bytes());
        return 0;
    }
    if LOCAL_COMMANDS.contains(&name.as_str()) {
        return local(name, &argv[1..], env);
    }
    if COMMANDS.iter().any(|(command, _, _)| *command == name.as_str()) {
        let _ = writeln!(env.err, "lazymkvctl: {name} is a lazymkv-dev command, not a lazymkvctl command");
        return 2;
    }
    let _ = writeln!(env.err, "lazymkvctl: unknown command {name}");
    let _ = env.err.write_all(USAGE.as_bytes());
    2
}

/// `lazymkvctl <name>: error: <message>` on `err` and exit 2, the form of every argument error (design part 9.6).
pub(crate) fn argument_error(err: &mut dyn Write, name: &str, message: &str) -> i32 {
    let _ = writeln!(err, "lazymkvctl {name}: error: {message}");
    2
}

/// What `--job`, `--jobs` and `--title` took out of a local command's words, and the words left for dev's parser.
struct Forms {
    job: Option<String>,
    jobs: bool,
    title: Option<String>,
    words: Vec<String>,
}

/// Takes `--job`, `--jobs` and `--title` out of `rest` and checks which of them `name` allows (Ruling S87). Err is the
/// text of an argument error.
fn read_forms(name: &str, rest: &[String]) -> Result<Forms, String> {
    let mut args = Args::new(rest, &["--job", "--jobs", "--title"]);
    let job = args.flag("--job")?;
    let jobs = args.switch("--jobs");
    let title = args.flag("--title")?;
    let words = args.rest()?;
    if name == "retune" {
        if job.is_some() {
            return Err("argument --job: not allowed with retune; use --jobs".to_string());
        }
    } else if jobs {
        return Err("argument --jobs: only with retune".to_string());
    }
    if title.is_some() && job.is_none() {
        return Err("argument --title: only with --job".to_string());
    }
    if job.is_some() {
        match (name, &title) {
            ("decide", None) => return Err("argument --title: required with decide --job".to_string()),
            ("decide", Some(_)) => {}
            (_, Some(_)) => return Err(format!("argument --title: not allowed with {name}")),
            (_, None) => {}
        }
    }
    Ok(Forms { job, jobs, title, words })
}

/// A command of `LOCAL_COMMANDS`: with no `--job` and no `--jobs` it runs through lazymkv-dev's dispatcher, so the
/// documents, the exit codes and dev's stderr are identical (design part 9.5, P23, S37); otherwise it is the engine form.
fn local(name: &str, rest: &[String], env: Env) -> i32 {
    let forms = match read_forms(name, rest) {
        Ok(forms) => forms,
        Err(message) => return argument_error(env.err, name, &message),
    };
    if forms.job.is_none() && !forms.jobs {
        let mut words = vec![name.to_string()];
        words.extend(forms.words);
        return lazymkv_dev::dispatch(env.ctx, &words, env.out, env.err);
    }
    engine_form(name, forms.job.as_deref(), forms.title.as_deref(), &forms.words, env)
}

/// The engine form of plan, decide or retune (`--job ID`, `--job ID --title KEY`, `--jobs`). PR 2's binary has no
/// transport, so after the argument checks it exits 4 with `NO_TRANSPORT` (S38); Task 28 replaces the body with `relay`.
fn engine_form(name: &str, job: Option<&str>, title: Option<&str>, words: &[String], env: Env) -> i32 {
    let _ = (job, title); // Task 28's relay sends them in the `command` request.
    if name == "retune" {
        if let Err(message) = retune_spec(env.ctx, words) {
            return argument_error(env.err, name, &message);
        }
    }
    let _ = writeln!(env.err, "{NO_TRANSPORT}");
    4
}

/// The spec path of `retune --jobs` (design part 9.6): `--spec`'s value through `ctx.abs`, else
/// `evidence::default_spec(ctx)`; Err is an `Args` error text, or "the following arguments are required: --spec"
/// (retune.rs:83's sentence) when neither gives a path. Def: T27. Use: T27, T28.
pub(crate) fn retune_spec(ctx: &Ctx, words: &[String]) -> Result<PathBuf, String> {
    match Args::new(words, &RETUNE_RELAY_FLAGS).flag("--spec")? {
        Some(given) => Ok(ctx.abs(&given)),
        None => default_spec(ctx).ok_or_else(|| "the following arguments are required: --spec".to_string()),
    }
}

#[cfg(test)]
mod tests;
