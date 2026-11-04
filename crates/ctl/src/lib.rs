//! lazymkv-ctl: lazymkvctl's commands. plan, decide and retune run locally through lazymkv_dev::dispatch; the rest go to the engine over IPC (design parts 4.1, 9.5, 9.6).

use lazymkv_dev::cmd::{decide, plan};
use lazymkv_dev::evidence::{default_spec, Ctx};
use lazymkv_dev::forms::read_forms;
use lazymkv_dev::{Args, COMMANDS};
use lazymkv_platform::IpcConnector;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

mod engine_cmds;

/// What lazymkvctl reads and writes: the dev context, the engine connector (the named pipe's client on Windows, design
/// part 3.3; None on Linux and macOS, S38), and the three streams (design parts 9.5, 9.6). Def: T27, T3-12.
/// Use: T27, T28, T3-12.
pub struct Env<'a> {
    /// The working folder, environment and runner that the local commands run over.
    pub ctx: &'a Ctx,
    /// The way to the engine; None when the build has no transport, as on Linux and macOS (S38).
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

/// The first part of `lazymkvctl --help`, the commands run here; `write_usage` adds the engine commands (S88, PF-33,
/// design parts 9.5, 9.6).
const USAGE: &str = "\
usage: lazymkvctl <command> [arguments]

commands run here (design part 9.5):
  plan <slug|recording dir|scan file> [lazymkv-dev plan's flags]   or   plan --job ID
  decide (<slug> <label> | --rec DIR) [lazymkv-dev decide's flags]   or   decide --job ID --title KEY
  retune [lazymkv-dev retune's flags]   or   retune --jobs [--spec FILE]
";

/// Writes `lazymkvctl --help`'s words: USAGE, then one indented line per `ENGINE_COMMANDS` synopsis (PF-33).
fn write_usage(w: &mut dyn Write) {
    let _ = w.write_all(USAGE.as_bytes());
    let _ = writeln!(w, "\ncommands sent to the engine (design part 9.6):");
    for (_, synopsis) in engine_cmds::ENGINE_COMMANDS {
        let _ = writeln!(w, "  {synopsis}");
    }
}

/// lazymkvctl's entry: `argv` excludes the program name; returns the exit code of design part 9.6. Def: T27. Use: T27, T28.
pub fn run(argv: &[String], env: Env) -> i32 {
    let Some(name) = argv.first() else {
        write_usage(env.err);
        return 2;
    };
    if name == "--help" || name == "-h" {
        write_usage(env.out);
        return 0;
    }
    if LOCAL_COMMANDS.contains(&name.as_str()) {
        return local(argv, env);
    }
    if engine_cmds::ENGINE_COMMANDS.iter().any(|(command, _)| command == name) {
        return engine_cmds::run(argv, env);
    }
    if COMMANDS.iter().any(|(command, _, _)| *command == name.as_str()) {
        let _ = writeln!(env.err, "lazymkvctl: {name} is a lazymkv-dev command, not a lazymkvctl command");
        return 2;
    }
    let _ = writeln!(env.err, "lazymkvctl: unknown command {name}");
    write_usage(env.err);
    2
}

/// `lazymkvctl <name>: error: <message>` on `err` and exit 2, the form of every argument error (design part 9.6).
/// Def: T27. Use: T27, T28.
pub(crate) fn argument_error(err: &mut dyn Write, name: &str, message: &str) -> i32 {
    let _ = writeln!(err, "lazymkvctl {name}: error: {message}");
    2
}

/// A command of `LOCAL_COMMANDS` (`argv[0]`): with no `--job` and no `--jobs` it runs through lazymkv-dev's
/// dispatcher, so the documents, the exit codes and dev's stderr are identical (design part 9.5, P23, S37); otherwise
/// the engine form (`--job ID`, `--job ID --title KEY`, `--jobs`) goes to the engine through `engine_cmds::relay`.
fn local(argv: &[String], env: Env) -> i32 {
    let name = argv[0].as_str();
    let forms = match read_forms(name, &argv[1..]) {
        Ok(forms) => forms,
        Err(message) => return argument_error(env.err, name, &message),
    };
    if forms.job.is_none() && !forms.jobs {
        let mut words = vec![name.to_string()];
        words.extend(forms.words);
        return lazymkv_dev::dispatch(env.ctx, &words, env.out, env.err);
    }
    engine_cmds::relay(argv, env)
}

/// The spec path of `retune --jobs` (design part 9.6): `--spec`'s value through `ctx.abs`, else
/// `evidence::default_spec(ctx)`; Err is an `Args` error text (an ambiguous prefix such as `--s` included, T27-r1), or
/// "the following arguments are required: --spec" (retune.rs:83's sentence) when neither gives a path. Def: T27.
/// Use: T27, T28.
pub(crate) fn retune_spec(ctx: &Ctx, words: &[String]) -> Result<PathBuf, String> {
    let mut args = Args::new(words, &RETUNE_RELAY_FLAGS);
    let spec = args.flag("--spec")?;
    args.rest()?;
    match spec {
        Some(given) => Ok(ctx.abs(&given)),
        None => default_spec(ctx).ok_or_else(|| "the following arguments are required: --spec".to_string()),
    }
}

/// decide's path flags: `decide::FLAGS` without `--selection`, whose value is a choice, not a path (V10's ruling).
/// plan's path flags are all of `plan::FLAGS`. Def: PR #14 review round 1 (V10). Use: PR #14 review round 1 (V10).
pub(crate) const DECIDE_PATH_FLAGS: [&str; 7] =
    ["--rec", "--text", "--settings", "--tmdb-search", "--tmdb-record", "--remembered", "--evidence"];

/// The words of a relayed plan or decide (`argv[0]`), with each path flag whose value is relative sent again, made
/// absolute with `ctx.abs` against lazymkvctl's working folder (V10's ruling), since the child runs in the engine's.
/// The words after the name are read with the child's own declared flags (`plan::FLAGS`, `decide::FLAGS`), so a
/// prefix, the `=` form and a repeat resolve as they do in the child; the added words go at the first `--`, else at the
/// end, in the path list's order (`plan::FLAGS`, `DECIDE_PATH_FLAGS`), after the values they override, which the
/// child's last-wins reads. An absolute value, or a path flag not given, adds nothing. Any other name, or a path flag
/// with no value, gives `argv` unchanged, and the child prints dev's own error and exits 2. `--job` and `--title`
/// cannot confuse the reading: `read_forms` accepted them, their values never start with `-`, and `--t` is ambiguous
/// in decide's list, so `Args::flag` skips it. A drive-relative value such as `D:alt.json` stays drive-relative.
/// Def: PR #14 review round 1 (V10). Use: PR #14 review round 1 (V10).
pub(crate) fn relayed_paths(ctx: &Ctx, argv: &[String]) -> Vec<String> {
    let (declared, paths): (&[&str], &[&str]) = match argv.first().map(String::as_str) {
        Some("plan") => (&plan::FLAGS[..], &plan::FLAGS[..]),
        Some("decide") => (&decide::FLAGS[..], &DECIDE_PATH_FLAGS[..]),
        _ => return argv.to_vec(),
    };
    let mut args = Args::new(&argv[1..], declared);
    let mut added = Vec::new();
    for flag in paths {
        match args.flag(flag) {
            Ok(Some(value)) if !Path::new(&value).is_absolute() => {
                added.extend([flag.to_string(), ctx.abs(&value).to_string_lossy().into_owned()]);
            }
            Ok(_) => {}
            Err(_) => return argv.to_vec(),
        }
    }
    let at = argv.iter().position(|word| word == "--").unwrap_or(argv.len());
    let mut words = argv[..at].to_vec();
    words.extend(added);
    words.extend_from_slice(&argv[at..]);
    words
}

#[cfg(test)]
mod tests;
