//! lazymkvctl's engine commands (design part 9.6) and the relay of plan, decide and retune's engine forms as the
//! `command` request (design part 9.5). A run checks its words, reads its settings file or its standard input, then
//! opens one connection through the `IpcConnector`, sends at most one request with id 1 and reads the engine's lines
//! until its answer (Rulings T28-a to T28-j).

use std::io::{BufRead, Read, Write};

use lazymkv_core::ipc::{read_reply, Envelope, OutputStream, Reply, PROTOCOL_VERSION};
use lazymkv_core::settings::SettingsProblem;
use lazymkv_dev::evidence::Ctx;
use lazymkv_dev::pyjson::{dumps, Style};
use lazymkv_dev::Args;
use lazymkv_platform::{ConnectError, IpcConnector, IpcStream};
use serde_json::{json, Value};

use crate::{argument_error, retune_spec, Env, NOT_ELEVATED, NOT_RUNNING, NO_TRANSPORT, REFUSED};

/// Design part 9.6's engine commands as (name, synopsis after "usage: lazymkvctl "), in its table order.
/// Def: T28. Use: T28.
pub(crate) const ENGINE_COMMANDS: [(&str, &str); 11] = [
    ("status", "status"), ("watch", "watch"), ("history", "history [--before JOB]"),
    ("settings", "settings get | settings set FILE"), ("secret", "secret set NAME"), ("selftest", "selftest"),
    ("retry", "retry JOB"), ("cancel", "cancel JOB"), ("rip-again", "rip-again JOB"), ("approve", "approve JOB"),
    ("stop", "stop [--force]"),
];

/// The message of exit 4 when the connection closes before the answer (T28-d).
const CLOSED: &str = "lazymkvctl: the engine closed the connection before replying";
/// The message of exit 1 when `selftest`'s `ok` came with no `selftest_result` push before it (T28-i).
const NO_SELFTEST: &str = "lazymkvctl: the engine replied without a self-test result";
/// The message of exit 4 when a relayed command's `ok` carries no integer `exit_code`.
const NO_EXIT_CODE: &str = "lazymkvctl: the engine's reply has no exit_code";
/// The message of exit 4 when a `command_output` push of the request lacks a known `stream` or a `line`.
const BAD_OUTPUT: &str = "lazymkvctl: the engine's command_output has no stream and line";

/// Runs one engine command of design part 9.6; argv[0] is an ENGINE_COMMANDS name. Exits 0 on `ok`, 1 on `error`,
/// 2 on an argument error, 4 when no engine answered (T28-d). Def: T28. Use: T28.
pub(crate) fn run(argv: &[String], env: Env) -> i32 {
    let name = argv[0].as_str();
    match exchange_of(name, &argv[1..], env.ctx, &mut *env.stdin) {
        Ok(exchange) => talk(env, exchange),
        Err(message) => {
            let synopsis = ENGINE_COMMANDS.iter().find(|(command, _)| *command == name).map_or(name, |(_, text)| text);
            let _ = writeln!(env.err, "usage: lazymkvctl {synopsis}");
            argument_error(env.err, name, &message)
        }
    }
}

/// Sends the engine form of `plan --job`, `decide --job --title` or `retune --jobs` as `command` (design part 9.5),
/// prints the relayed lines and exits with the child's code; an `error` reply exits 2 (RP24). Def: T28. Use: T28.
pub(crate) fn relay(argv: &[String], env: Env) -> i32 {
    let mut words = argv.to_vec();
    if argv[0] == "retune" {
        match retune_spec(env.ctx, &argv[1..]) {
            Ok(spec) => words.extend(["--spec".to_string(), spec.to_string_lossy().into_owned()]),
            Err(message) => return argument_error(env.err, "retune", &message),
        }
    }
    talk(env, Exchange::ask("command", json!({ "words": words }), Answer::ExitCode))
}

/// What one exchange sends and waits for.
struct Exchange {
    /// The request's type and payload, sent with id 1 (T28-c); None sends nothing.
    request: Option<(&'static str, Value)>,
    /// What ends the exchange.
    wait: Wait,
}

impl Exchange {
    /// An exchange that sends `kind` with `payload` and waits for its reply.
    fn ask(kind: &'static str, payload: Value, answer: Answer) -> Exchange {
        Exchange { request: Some((kind, payload)), wait: Wait::Reply(answer) }
    }

    /// An exchange that sends nothing and waits for `wait`.
    fn listen(wait: Wait) -> Exchange {
        Exchange { request: None, wait }
    }
}

/// What ends an exchange.
enum Wait {
    /// The first `snapshot` push, whose payload is printed, or with Some(key) that key of it (status, settings get;
    /// T28-a); a missing key prints `null`.
    Snapshot(Option<&'static str>),
    /// The connection's close; every line is printed as received (watch, T28-b).
    Close,
    /// The `ok` or `error` reply to request 1.
    Reply(Answer),
}

/// What an `ok` reply prints and the exit code it gives.
enum Answer {
    /// Nothing; exit 0.
    Nothing,
    /// The reply's `result` (history, T28-a); exit 0.
    Result,
    /// The last `selftest_result` push's payload, exit 0; without one NO_SELFTEST, exit 1 (T28-i).
    Selftest,
    /// The relayed command's `exit_code`; an `error` reply exits 2 instead of 1 (RP24).
    ExitCode,
}

/// The checked exchange of engine command `name` over `words` (design part 9.6's table). Every check, the settings
/// file read and the standard-input read happen here, before any connection (T28-h); Err is the argument error's text.
fn exchange_of(name: &str, words: &[String], ctx: &Ctx, stdin: &mut dyn Read) -> Result<Exchange, String> {
    let job_request = |kind: &'static str| -> Result<Exchange, String> {
        let job = one_job(words)?;
        Ok(Exchange::ask(kind, json!({ "job": job }), Answer::Nothing))
    };
    match name {
        "status" => {
            nothing_left(Args::new(words, &[]))?;
            Ok(Exchange::listen(Wait::Snapshot(None)))
        }
        "watch" => {
            nothing_left(Args::new(words, &[]))?;
            Ok(Exchange::listen(Wait::Close))
        }
        "history" => {
            let mut args = Args::new(words, &["--before"]);
            let before = args.flag("--before")?;
            nothing_left(args)?;
            Ok(Exchange::ask("history", json!({ "before": before }), Answer::Result))
        }
        "settings" => settings(words, ctx),
        "secret" => secret(words, stdin),
        "selftest" => {
            nothing_left(Args::new(words, &[]))?;
            Ok(Exchange::ask("run_selftest", Value::Null, Answer::Selftest))
        }
        "retry" => job_request("retry"),
        "cancel" => job_request("cancel"),
        "rip-again" => job_request("rip_again"),
        "approve" => {
            let job = one_job(words)?;
            Ok(Exchange::ask("review_action", json!({ "job": job, "action": "approve" }), Answer::Nothing))
        }
        "stop" => {
            let mut args = Args::new(words, &["--force"]);
            let force = args.switch("--force");
            nothing_left(args)?;
            Ok(Exchange::ask("shutdown", json!({ "force": force }), Answer::Nothing))
        }
        _ => Err(format!("{name} is not an engine command")),
    }
}

/// Err("unrecognized arguments: ...") when `args` holds any word no call took, as argparse words it.
fn nothing_left(args: Args) -> Result<(), String> {
    unrecognized(&args.positionals()?)
}

/// Err("unrecognized arguments: ...") naming `left`, or Ok when it is empty.
fn unrecognized(left: &[String]) -> Result<(), String> {
    if left.is_empty() {
        Ok(())
    } else {
        Err(format!("unrecognized arguments: {}", left.join(" ")))
    }
}

/// The one JOB of `retry`, `cancel`, `rip-again` and `approve`.
fn one_job(words: &[String]) -> Result<String, String> {
    let mut left = Args::new(words, &[]).positionals()?;
    if left.is_empty() {
        return Err("the following arguments are required: JOB".to_string());
    }
    let job = left.remove(0);
    unrecognized(&left)?;
    Ok(job)
}

/// `settings get` waits for the snapshot's `settings`; `settings set FILE` sends FILE's object as `settings_set`.
fn settings(words: &[String], ctx: &Ctx) -> Result<Exchange, String> {
    let words = Args::new(words, &[]).positionals()?;
    match words.split_first() {
        None => Err("the following arguments are required: get or set".to_string()),
        Some((action, rest)) if action == "get" => {
            unrecognized(rest)?;
            Ok(Exchange::listen(Wait::Snapshot(Some("settings"))))
        }
        Some((action, rest)) if action == "set" => {
            let (file, rest) = rest.split_first().ok_or("the following arguments are required: FILE")?;
            unrecognized(rest)?;
            Ok(Exchange::ask("settings_set", settings_file(ctx, file)?, Answer::Nothing))
        }
        Some((action, _)) => Err(format!("invalid choice: '{action}' (choose from 'get', 'set')")),
    }
}

/// The JSON object in `file` (taken against the working folder); Err "<FILE>: <io error>" or "<FILE>: not a JSON
/// object".
fn settings_file(ctx: &Ctx, file: &str) -> Result<Value, String> {
    let text = std::fs::read_to_string(ctx.abs(file)).map_err(|error| format!("{file}: {error}"))?;
    match serde_json::from_str::<Value>(&text) {
        Ok(object @ Value::Object(_)) => Ok(object),
        _ => Err(format!("{file}: not a JSON object")),
    }
}

/// `secret set NAME` sends NAME and standard input's value (T28-f) as `secret_set`.
fn secret(words: &[String], stdin: &mut dyn Read) -> Result<Exchange, String> {
    let words = Args::new(words, &[]).positionals()?;
    let name = match words.split_first() {
        None => return Err("the following arguments are required: set".to_string()),
        Some((action, rest)) if action == "set" => {
            let (name, rest) = rest.split_first().ok_or("the following arguments are required: NAME")?;
            unrecognized(rest)?;
            name
        }
        Some((action, _)) => return Err(format!("invalid choice: '{action}' (choose from 'set')")),
    };
    let value = stdin_value(stdin)?;
    Ok(Exchange::ask("secret_set", json!({ "name": name, "value": value }), Answer::Nothing))
}

/// The whole of standard input with one trailing "\n" or "\r\n" removed (T28-f); Err when it cannot be read as text
/// or the value is empty.
fn stdin_value(stdin: &mut dyn Read) -> Result<String, String> {
    let mut text = String::new();
    stdin.read_to_string(&mut text).map_err(|error| format!("standard input: {error}"))?;
    let value = text.strip_suffix("\r\n").or_else(|| text.strip_suffix('\n')).unwrap_or(&text);
    if value.is_empty() {
        return Err("no value on standard input".to_string());
    }
    Ok(value.to_string())
}

/// The connection to the engine, or the message of exit 4 when there is none.
fn open(connector: Option<&dyn IpcConnector>) -> Result<IpcStream, &'static str> {
    let connector = connector.ok_or(NO_TRANSPORT)?;
    connector.connect().map_err(|error| match error {
        ConnectError::NotRunning => NOT_RUNNING,
        ConnectError::Refused => REFUSED,
        ConnectError::NotElevated => NOT_ELEVATED,
        ConnectError::NoTransport => NO_TRANSPORT,
    })
}

/// Opens one connection, sends the exchange's request with id 1 (T28-c), then reads the engine's lines until the
/// exchange's wait is met, and returns the exit code. Each line goes through `read_reply`:
/// - a `command_output` push of request 1 writes its line to `out` or `err`, chosen by its stream;
/// - a `selftest_result` push is kept for `Answer::Selftest`;
/// - a `snapshot` push ends `Wait::Snapshot`;
/// - the `ok` or `error` of request 1 ends `Wait::Reply`;
/// - anything else is skipped, except under `Wait::Close`, where every line is printed as received.
///
/// The connection's close exits 0 under `Wait::Close` and 4 with CLOSED otherwise; a line that cannot be read or that
/// `read_reply` refuses exits 4 (T28-d).
fn talk(env: Env, exchange: Exchange) -> i32 {
    let Exchange { request, wait } = exchange;
    let IpcStream { mut reader, mut writer } = match open(env.connector) {
        Ok(stream) => stream,
        Err(message) => {
            let _ = writeln!(env.err, "{message}");
            return 4;
        }
    };
    if let Some((kind, payload)) = request {
        let envelope = Envelope { v: PROTOCOL_VERSION, kind: kind.to_string(), id: Some(1), payload };
        let line = serde_json::to_string(&envelope).expect("an envelope of JSON values serialises") + "\n";
        if writer.write_all(line.as_bytes()).and_then(|()| writer.flush()).is_err() {
            let _ = writeln!(env.err, "{CLOSED}");
            return 4;
        }
    }
    let mut selftest = None;
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) if matches!(wait, Wait::Close) => return 0,
            Ok(0) => {
                let _ = writeln!(env.err, "{CLOSED}");
                return 4;
            }
            Ok(_) => {}
            Err(error) => {
                let _ = writeln!(env.err, "lazymkvctl: {error}");
                return 4;
            }
        }
        let reply = match read_reply(line.trim_end()) {
            Ok(reply) => reply,
            Err(message) => {
                let _ = writeln!(env.err, "lazymkvctl: {message}");
                return 4;
            }
        };
        match (reply, &wait) {
            (_, Wait::Close) => {
                let text = match line.strip_suffix('\n') {
                    Some(text) => text.strip_suffix('\r').unwrap_or(text),
                    None => &line,
                };
                // A standard output nobody reads any more ends the watch rather than leaving it running unseen.
                if writeln!(env.out, "{text}").and_then(|()| env.out.flush()).is_err() {
                    return 1;
                }
            }
            (Reply::Push(envelope), Wait::Snapshot(key)) if envelope.kind == "snapshot" => {
                let value = match key {
                    Some(key) => envelope.payload.get(*key).cloned().unwrap_or(Value::Null),
                    None => envelope.payload,
                };
                print_json(env.out, &value);
                return 0;
            }
            (Reply::Push(envelope), _) if envelope.kind == "selftest_result" => selftest = Some(envelope.payload),
            (Reply::Push(envelope), _) if envelope.kind == "command_output" && envelope.id == Some(1) => {
                let stream = envelope.payload.get("stream").cloned().map(serde_json::from_value::<OutputStream>);
                let text = envelope.payload.get("line").and_then(Value::as_str);
                let _ = match (stream, text) {
                    (Some(Ok(OutputStream::Stdout)), Some(text)) => writeln!(env.out, "{text}"),
                    (Some(Ok(OutputStream::Stderr)), Some(text)) => writeln!(env.err, "{text}"),
                    _ => {
                        let _ = writeln!(env.err, "{BAD_OUTPUT}");
                        return 4;
                    }
                };
            }
            (Reply::Ok { id: Some(1), result }, Wait::Reply(answer)) => {
                return answered(answer, result, selftest, env.out, env.err);
            }
            (Reply::Error { id: Some(1), message, problems }, Wait::Reply(answer)) => {
                print_refusal(env.err, &message, &problems);
                return if matches!(answer, Answer::ExitCode) { 2 } else { 1 };
            }
            _ => {}
        }
    }
}

/// What an `ok` of request 1 prints, and its exit code.
fn answered(answer: &Answer, result: Value, selftest: Option<Value>, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    match answer {
        Answer::Nothing => 0,
        Answer::Result => {
            print_json(out, &result);
            0
        }
        Answer::Selftest => match selftest {
            Some(payload) => {
                print_json(out, &payload);
                0
            }
            None => {
                let _ = writeln!(err, "{NO_SELFTEST}");
                1
            }
        },
        Answer::ExitCode => match result.get("exit_code").and_then(Value::as_i64).map(i32::try_from) {
            Some(Ok(code)) => code,
            _ => {
                let _ = writeln!(err, "{NO_EXIT_CODE}");
                4
            }
        },
    }
}

/// An `error` reply as T28-e prints it: `lazymkvctl: <message>`, then `lazymkvctl: <key>: <problem>` per problem.
fn print_refusal(err: &mut dyn Write, message: &str, problems: &[SettingsProblem]) {
    let _ = writeln!(err, "lazymkvctl: {message}");
    for problem in problems {
        let _ = writeln!(err, "lazymkvctl: {}: {}", problem.key, problem.problem);
    }
}

/// `value` in lazymkvctl's one JSON style, indent 1 and ASCII, and a newline (T28-a).
fn print_json(out: &mut dyn Write, value: &Value) {
    let _ = writeln!(out, "{}", dumps(value, Style::INDENT1_ASCII));
}

#[cfg(test)]
mod tests {
    use crate::{run, Env, NOT_ELEVATED, NOT_RUNNING, NO_TRANSPORT, REFUSED};
    use lazymkv_core::ipc::{push_line, reply_error, reply_ok, OutputStream, Push};
    use lazymkv_core::settings::SettingsProblem;
    use lazymkv_core::testsupport::{Scratch, ScratchExt};
    use lazymkv_dev::evidence::Ctx;
    use lazymkv_platform::fakes::{MemoryConnector, MemoryIpc};
    use lazymkv_platform::{ConnectError, IpcConnector, IpcTransport};
    use serde_json::{json, Value};
    use std::io::{BufRead, Write};
    use std::path::Path;
    use std::thread::{self, JoinHandle};

    const SNAPSHOT: &str = r#"{"v":1,"type":"snapshot","id":null,"payload":{"jobs":[]}}"#;

    fn call(argv: &[&str], connector: Option<&dyn IpcConnector>, stdin: &[u8], cwd: &Path) -> (i32, String, String) {
        let ctx = Ctx::for_test(cwd);
        let mut input = stdin;
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let argv: Vec<String> = argv.iter().map(|word| word.to_string()).collect();
        let code = run(&argv, Env { ctx: &ctx, connector, stdin: &mut input, out: &mut out, err: &mut err });
        (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
    }

    /// Accepts one connection, writes `before`; with `answer` Some, reads one request line and writes the answer;
    /// then closes the connection. Returns the request line, or "" when none was read; None when nothing connected
    /// before the test dropped its connector (`finish`), which makes `accept` fail instead of waiting forever.
    fn peer(ipc: MemoryIpc, before: Vec<String>, answer: Option<Vec<String>>) -> JoinHandle<Option<String>> {
        thread::spawn(move || {
            let Ok(mut stream) = ipc.accept() else {
                return None;
            };
            let send = |lines: &[String], stream: &mut lazymkv_platform::IpcStream| {
                for line in lines {
                    write!(stream.writer, "{}\n", line.trim_end_matches('\n')).unwrap();
                }
                stream.writer.flush().unwrap();
            };
            send(&before, &mut stream);
            let mut request = String::new();
            if let Some(lines) = answer {
                stream.reader.read_line(&mut request).unwrap();
                send(&lines, &mut stream);
            }
            Some(request)
        })
    }

    /// Drops the test's only connector, so a peer still waiting in `accept` fails at once, then returns the peer's
    /// request line. The test fails here, rather than hanging, when lazymkvctl never connected.
    fn finish(server: JoinHandle<Option<String>>, connector: MemoryConnector) -> String {
        drop(connector);
        let request = server.join().expect("the scripted peer's thread panicked; its message is printed above");
        request.expect("lazymkvctl never connected to the scripted peer")
    }

    fn request_of(line: &str) -> Value {
        serde_json::from_str(line.trim_end()).unwrap()
    }

    #[test]
    fn no_connector_exits_4_with_the_no_transport_message() {
        let scratch = Scratch::new("ctl-none");
        for argv in [&["status"][..], &["retry", "a-1"], &["plan", "--job", "a-1"]] {
            let (code, out, err) = call(argv, None, b"", &scratch.0);
            assert_eq!((code, out.as_str(), err), (4, "", format!("{NO_TRANSPORT}\n")), "{argv:?}");
        }
    }

    #[test]
    fn each_connect_error_exits_4_with_its_message() {
        let scratch = Scratch::new("ctl-connect");
        let cases = [
            (ConnectError::NotRunning, NOT_RUNNING),
            (ConnectError::Refused, REFUSED),
            (ConnectError::NotElevated, NOT_ELEVATED),
            (ConnectError::NoTransport, NO_TRANSPORT),
        ];
        for (error, message) in cases {
            let connector = MemoryConnector::failing(error);
            let (code, out, err) = call(&["status"], Some(&connector), b"", &scratch.0);
            assert_eq!((code, out.as_str(), err), (4, "", format!("{message}\n")));
        }
    }

    #[test]
    fn engine_command_argument_errors_exit_2_without_connecting() {
        let scratch = Scratch::new("ctl-args");
        let list = scratch.write_path("list.json", "[1]");
        let missing = scratch.0.join("missing.json");
        let (list, missing) = (list.to_str().unwrap(), missing.to_str().unwrap());
        let connector = MemoryConnector::failing(ConnectError::NotRunning);
        let cases: Vec<(Vec<&str>, &[u8], &str)> = vec![
            (vec!["status", "x"], b"", "status"),
            (vec!["watch", "x"], b"", "watch"),
            (vec!["history", "--before"], b"", "history"),
            (vec!["settings"], b"", "settings"),
            (vec!["settings", "put"], b"", "settings"),
            (vec!["settings", "set"], b"", "settings"),
            (vec!["settings", "set", missing], b"", "settings"),
            (vec!["settings", "set", list], b"", "settings"),
            (vec!["secret", "set"], b"", "secret"),
            (vec!["secret", "set", "tmdb_token"], b"", "secret"),
            (vec!["selftest", "x"], b"", "selftest"),
            (vec!["retry"], b"", "retry"),
            (vec!["cancel", "a-1", "b-1"], b"", "cancel"),
            (vec!["approve"], b"", "approve"),
            (vec!["stop", "--now"], b"", "stop"),
        ];
        for (argv, stdin, name) in cases {
            let (code, out, err) = call(&argv, Some(&connector), stdin, &scratch.0);
            assert_eq!((code, out.as_str()), (2, ""), "{argv:?}");
            assert!(err.starts_with("usage: lazymkvctl "), "{argv:?}: {err}");
            assert!(err.contains(&format!("lazymkvctl {name}: error: ")), "{argv:?}: {err}");
            assert!(!err.contains(NOT_RUNNING), "{argv:?} connected: {err}");
        }
    }

    #[test]
    fn retune_jobs_without_a_spec_exits_2() {
        let scratch = Scratch::new("ctl-retune-nospec");
        let connector = MemoryConnector::failing(ConnectError::NotRunning);
        let (code, out, err) = call(&["retune", "--jobs"], Some(&connector), b"", &scratch.0);
        assert_eq!((code, out.as_str()), (2, ""));
        assert!(err.contains("the following arguments are required: --spec"), "{err}");
        assert!(!err.contains(NOT_RUNNING), "{err}");
    }

    #[test]
    fn status_prints_the_first_snapshots_payload() {
        let scratch = Scratch::new("ctl-status");
        let (ipc, connector) = MemoryIpc::new();
        let server = peer(ipc, vec![SNAPSHOT.to_string()], None);
        let (code, out, err) = call(&["status"], Some(&connector), b"", &scratch.0);
        finish(server, connector);
        assert_eq!((code, out.as_str(), err.as_str()), (0, "{\n \"jobs\": []\n}\n", ""));
    }

    #[test]
    fn watch_prints_pushes_until_the_connection_closes() {
        let scratch = Scratch::new("ctl-watch");
        let (ipc, connector) = MemoryIpc::new();
        let lines = vec![
            SNAPSHOT.to_string(),
            r#"{"v":1,"type":"job_update","id":null,"payload":{"id":"a-1"}}"#.to_string(),
            r#"{"v":1,"type":"progress","id":null,"payload":{"job":"a-1","step":"RipTitle","title":"t00-00800","fraction":0.5}}"#
                .to_string(),
        ];
        let server = peer(ipc, lines.clone(), None);
        let (code, out, err) = call(&["watch"], Some(&connector), b"", &scratch.0);
        assert_eq!(finish(server, connector), "");
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(out, lines.join("\n") + "\n");
    }

    #[test]
    fn each_engine_command_sends_its_design_part_9_3_request() {
        let scratch = Scratch::new("ctl-requests");
        let selftest_push = r#"{"v":1,"type":"selftest_result","id":null,"payload":{"checks":[]}}"#.to_string();
        let cases: Vec<(Vec<&str>, &str, Value, &str)> = vec![
            (vec!["history"], "history", json!({"before": null}), "[]\n"),
            (vec!["history", "--before", "a-1"], "history", json!({"before": "a-1"}), "[]\n"),
            (vec!["selftest"], "run_selftest", Value::Null, "{\n \"checks\": []\n}\n"),
            (vec!["retry", "a-1"], "retry", json!({"job": "a-1"}), ""),
            (vec!["cancel", "a-1"], "cancel", json!({"job": "a-1"}), ""),
            (vec!["rip-again", "a-1"], "rip_again", json!({"job": "a-1"}), ""),
            (vec!["approve", "a-1"], "review_action", json!({"job": "a-1", "action": "approve"}), ""),
            (vec!["stop"], "shutdown", json!({"force": false}), ""),
            (vec!["stop", "--force"], "shutdown", json!({"force": true}), ""),
        ];
        for (argv, kind, payload, printed) in cases {
            let (ipc, connector) = MemoryIpc::new();
            let answer = vec![selftest_push.clone(), reply_ok(Some(1), json!([]))];
            let server = peer(ipc, vec![SNAPSHOT.to_string()], Some(answer));
            let (code, out, err) = call(&argv, Some(&connector), b"", &scratch.0);
            let request = request_of(&finish(server, connector));
            assert_eq!(request, json!({"v": 1, "type": kind, "id": 1, "payload": payload}), "{argv:?}");
            assert_eq!((code, out.as_str(), err.as_str()), (0, printed, ""), "{argv:?}");
        }
    }

    #[test]
    fn secret_set_sends_stdins_text_without_its_last_newline() {
        let scratch = Scratch::new("ctl-secret");
        for (name, stdin, value) in [
            ("tmdb_token", &b"tok-123\r\n"[..], "tok-123"),
            (r"share:\\nas\media", &b"user\npass\n"[..], "user\npass"),
        ] {
            let (ipc, connector) = MemoryIpc::new();
            let server = peer(ipc, vec![SNAPSHOT.to_string()], Some(vec![reply_ok(Some(1), Value::Null)]));
            let (code, out, err) = call(&["secret", "set", name], Some(&connector), stdin, &scratch.0);
            let request = request_of(&finish(server, connector));
            assert_eq!(request["type"], "secret_set");
            assert_eq!(request["payload"], json!({"name": name, "value": value}));
            assert_eq!((code, out.as_str(), err.as_str()), (0, "", ""));
        }
    }

    #[test]
    fn an_error_reply_exits_1_with_its_message_and_problems() {
        let scratch = Scratch::new("ctl-error");
        let file = scratch.write_path("change.json", r#"{"cache_mb": "big"}"#);
        let (ipc, connector) = MemoryIpc::new();
        let problem = SettingsProblem { key: "cache_mb".to_string(), problem: "not a number".to_string() };
        let server = peer(
            ipc,
            vec![SNAPSHOT.to_string()],
            Some(vec![reply_error(Some(1), "the settings were refused", &[problem])]),
        );
        let (code, out, err) = call(&["settings", "set", file.to_str().unwrap()], Some(&connector), b"", &scratch.0);
        let request = request_of(&finish(server, connector));
        assert_eq!(request, json!({"v": 1, "type": "settings_set", "id": 1, "payload": {"cache_mb": "big"}}));
        assert_eq!(
            (code, out.as_str(), err.as_str()),
            (1, "", "lazymkvctl: the settings were refused\nlazymkvctl: cache_mb: not a number\n")
        );
    }

    #[test]
    fn a_connection_closed_before_the_reply_exits_4() {
        let scratch = Scratch::new("ctl-closed");
        let (ipc, connector) = MemoryIpc::new();
        let server = peer(ipc, vec![SNAPSHOT.to_string()], Some(vec![]));
        let (code, out, err) = call(&["retry", "a-1"], Some(&connector), b"", &scratch.0);
        finish(server, connector);
        assert_eq!(
            (code, out.as_str(), err.as_str()),
            (4, "", "lazymkvctl: the engine closed the connection before replying\n")
        );
    }

    #[test]
    fn a_relayed_command_prints_its_output_lines_and_exits_with_the_childs_code() {
        let scratch = Scratch::new("ctl-relay");
        let (ipc, connector) = MemoryIpc::new();
        let stdout = Push::CommandOutput { stream: OutputStream::Stdout, line: "{".to_string() };
        let stderr = Push::CommandOutput { stream: OutputStream::Stderr, line: "warning: x".to_string() };
        let server = peer(
            ipc,
            vec![SNAPSHOT.to_string()],
            Some(vec![
                push_line(&stdout, Some(1)),
                push_line(&stderr, Some(1)),
                r#"{"v":1,"type":"job_update","id":null,"payload":{"id":"a-1"}}"#.to_string(),
                reply_ok(Some(1), json!({"exit_code": 3})),
            ]),
        );
        let argv = ["decide", "--job", "a-1", "--title", "t00-00800"];
        let (code, out, err) = call(&argv, Some(&connector), b"", &scratch.0);
        let request = request_of(&finish(server, connector));
        assert_eq!(request, json!({"v": 1, "type": "command", "id": 1, "payload": {"words": argv}}));
        assert_eq!((code, out.as_str(), err.as_str()), (3, "{\n", "warning: x\n"));
    }

    #[test]
    fn a_refused_relayed_command_exits_2() {
        let scratch = Scratch::new("ctl-relay-refused");
        let (ipc, connector) = MemoryIpc::new();
        let server = peer(
            ipc,
            vec![SNAPSHOT.to_string()],
            Some(vec![reply_error(Some(1), "a command is already running", &[])]),
        );
        let (code, out, err) = call(&["plan", "--job", "a-1"], Some(&connector), b"", &scratch.0);
        finish(server, connector);
        assert_eq!((code, out.as_str(), err.as_str()), (2, "", "lazymkvctl: a command is already running\n"));
    }

    #[test]
    fn retune_jobs_sends_the_spec_path_made_absolute() {
        let scratch = Scratch::new("ctl-retune-spec");
        let (ipc, connector) = MemoryIpc::new();
        let server = peer(ipc, vec![SNAPSHOT.to_string()], Some(vec![reply_ok(Some(1), json!({"exit_code": 0}))]));
        let (code, _, err) = call(&["retune", "--jobs", "--spec", "spec.md"], Some(&connector), b"", &scratch.0);
        let request = request_of(&finish(server, connector));
        let absolute = scratch.0.join("spec.md").to_str().unwrap().to_string();
        assert_eq!(request["payload"]["words"], json!(["retune", "--jobs", "--spec", "spec.md", "--spec", absolute]));
        assert_eq!((code, err.as_str()), (0, ""));
    }
}
