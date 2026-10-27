//! The engine's IPC (inter-process communication) messages and the notification payload: the envelope every line of
//! the engine's pipe carries (design part 9.1), the requests a client sends and the replies and pushes the engine
//! sends back (design part 9.3), the self-test (spec 4.5) and the notification of spec 12. Pure serde types and
//! string functions with no I/O (RP19): the engine's server (Task 26) and `lazymkvctl` (Task 28) share them, so both
//! read and write one JSON shape.

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use crate::job::{FailureClass, NotifyEvent, State};
use crate::plan::Plan;
use crate::record::JobRecord;
use crate::review::{sorted_unique, Noted, ReviewReason};
use crate::settings::{Settings, SettingsProblem};

/// The protocol version every envelope carries in `v` (design part 9.1). Def: T7. Use: T26, T28.
pub const PROTOCOL_VERSION: u32 = 1;
/// The longest line the engine reads, newline excluded (design part 9.1). Def: T7. Use: T26.
pub const MAX_LINE_BYTES: usize = 4 * 1024 * 1024;
/// The error text for a line over MAX_LINE_BYTES, from parse_request and from the engine's capped reader (design part
/// 9.1). Def: T7. Use: T7, T26.
pub const LINE_TOO_LONG: &str = "the line is longer than 4 MiB";
/// How many finished jobs one `history` reply holds, and how many the snapshot lists after the unfinished ones (design
/// parts 9.2, 9.3). Def: T7. Use: T26.
pub const HISTORY_PAGE: usize = 50;
/// The message of a job the owner cancelled (design part 9.3), also the drivers' message for a run the cancel killed.
/// Def: T7. Use: T13, T14, T22, T23, T26.
pub const CANCELLED: &str = "cancelled by the owner";
/// The names `secret_set` takes besides a share's (design part 9.4). Def: T7. Use: T26.
pub const SECRET_NAMES: [&str; 3] = ["tmdb_token", "ha_webhook_url", "webhook_url"];

/// Whether `name` is a secret `secret_set` takes: one of SECRET_NAMES, or a share's `share:\\<server>\<share>`, with
/// both parts non-empty and no further backslash, because design part 9.4 names a share and not a folder (Ruling
/// T7-f). Def: T7. Use: T26.
pub fn is_secret_name(name: &str) -> bool {
    if SECRET_NAMES.contains(&name) {
        return true;
    }
    name.strip_prefix(r"share:\\")
        .and_then(|rest| rest.split_once('\\'))
        .is_some_and(|(server, share)| !server.is_empty() && !share.is_empty() && !share.contains('\\'))
}

/// One line of the pipe (design part 9.1): the protocol version, the message type, the request's id (None for a push
/// that answers no request) and the payload. A missing `id` or `payload` reads as None or `null`; unknown keys are
/// ignored. Def: T7. Use: T26, T28.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Envelope {
    /// The protocol version.
    pub v: u32,
    /// The message type, in snake case (`type` on the wire).
    #[serde(rename = "type")]
    pub kind: String,
    /// The id of the request this line answers, or None.
    #[serde(default)]
    pub id: Option<u64>,
    /// The message's payload.
    #[serde(default)]
    pub payload: Value,
}

/// A request a client sends (design part 9.3). `type` is the variant in snake case. `SettingsSet`'s `changes` is the
/// whole payload object, with no `changes` key on the wire. Def: T7. Use: T26, T28.
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    /// `settings_set`: change settings keys; the engine validates them and refuses all of them on a problem.
    SettingsSet {
        /// The changed keys.
        changes: Map<String, Value>,
    },
    /// `secret_set`: store a secret (a name accepted by `is_secret_name`).
    SecretSet {
        /// The secret's name.
        name: String,
        /// The secret's value.
        value: String,
    },
    /// `retry`: run a failed or waiting job again.
    Retry {
        /// The job's ID.
        job: String,
    },
    /// `cancel`: cancel a job.
    Cancel {
        /// The job's ID.
        job: String,
    },
    /// `discard`: discard a job.
    Discard {
        /// The job's ID.
        job: String,
    },
    /// `rip_again`: arm Rip again on a job.
    RipAgain {
        /// The job's ID.
        job: String,
    },
    /// `run_selftest`: run the self-test again.
    RunSelftest,
    /// `history`: one page of finished jobs, newest first.
    History {
        /// The ID of the oldest job of the page before, or None for the first page.
        before: Option<String>,
    },
    /// `shutdown`: stop the engine.
    Shutdown {
        /// Whether to stop with a job still running.
        force: bool,
    },
    /// `command`: run an ordinary `lazymkvctl` command in the engine.
    Command {
        /// The command's words.
        words: Vec<String>,
    },
    /// `review_action`: the owner's answer to a held job.
    ReviewAction {
        /// The job's ID.
        job: String,
        /// The action's name.
        action: String,
        /// The owner's overrides, or None.
        overrides: Option<Value>,
    },
    /// `ui_quit`: the UI closed its window.
    UiQuit,
}

/// The payload of `retry`, `cancel`, `discard` and `rip_again`.
#[derive(serde::Deserialize)]
struct JobPayload {
    job: String,
}

/// The payload of `secret_set`.
#[derive(serde::Deserialize)]
struct SecretPayload {
    name: String,
    value: String,
}

/// The payload of `history`.
#[derive(serde::Deserialize)]
struct HistoryPayload {
    #[serde(default)]
    before: Option<String>,
}

/// The payload of `shutdown`: `force` is required (Ruling T7-i).
#[derive(serde::Deserialize)]
struct ShutdownPayload {
    force: bool,
}

/// The payload of `command`.
#[derive(serde::Deserialize)]
struct CommandPayload {
    words: Vec<String>,
}

/// The payload of `review_action`.
#[derive(serde::Deserialize)]
struct ReviewPayload {
    job: String,
    action: String,
    #[serde(default)]
    overrides: Option<Value>,
}

/// Decodes `payload` as the request type `kind`'s payload `T`; Err is the refusal text.
fn decode<T: serde::de::DeserializeOwned>(kind: &str, payload: Value) -> Result<T, String> {
    serde_json::from_value(payload).map_err(|e| format!("the {kind} payload does not fit: {e}"))
}

/// Reads one request line (design parts 9.1, 9.3). Err is the error reply's line (without its newline), carrying the
/// request's id when the line has a readable one. The checks run in this order: the length, JSON and an object, the
/// id, the version, the type, then the payload. A `null` or absent payload reads as `{}` (Ruling T7-e), except for
/// `settings_set`, whose payload is the object of changed keys. Unknown keys are ignored. Def: T7. Use: T26.
pub fn parse_request(line: &str) -> Result<(Option<u64>, Request), String> {
    if line.len() > MAX_LINE_BYTES {
        return Err(reply_error(None, LINE_TOO_LONG, &[]));
    }
    let value: Value = serde_json::from_str(line)
        .map_err(|e| reply_error(None, &format!("the line is not a JSON message: {e}"), &[]))?;
    let Value::Object(mut message) = value else {
        return Err(reply_error(None, "the line is not a JSON message: it is not an object", &[]));
    };
    let id = message.get("id").and_then(Value::as_u64);
    let refuse = |text: &str| reply_error(id, text, &[]);
    let Some(v) = message.get("v").and_then(Value::as_u64) else {
        return Err(refuse("the message has no protocol version"));
    };
    if v != u64::from(PROTOCOL_VERSION) {
        return Err(refuse(&format!(
            "protocol version {v} is not supported; this engine speaks version {PROTOCOL_VERSION}"
        )));
    }
    let Some(kind) = message.get("type").and_then(Value::as_str).map(str::to_string) else {
        return Err(refuse("the message has no type"));
    };
    let payload = message.remove("payload").unwrap_or(Value::Null);
    let request = match kind.as_str() {
        "run_selftest" => Request::RunSelftest,
        "ui_quit" => Request::UiQuit,
        "settings_set" | "secret_set" | "retry" | "cancel" | "discard" | "rip_again" | "history" | "shutdown"
        | "command" | "review_action" => {
            let payload = match payload {
                Value::Null if kind != "settings_set" => Value::Object(Map::new()),
                other => other,
            };
            if !payload.is_object() {
                return Err(refuse(&format!("the {kind} payload is not an object")));
            }
            decode_request(&kind, payload).map_err(|text| refuse(&text))?
        }
        _ => return Err(refuse(&format!("unknown message type \"{kind}\""))),
    };
    Ok((id, request))
}

/// The request of a payload-carrying type `kind`, from its object `payload`; Err is the refusal text.
fn decode_request(kind: &str, payload: Value) -> Result<Request, String> {
    Ok(match kind {
        "settings_set" => match payload {
            Value::Object(changes) => Request::SettingsSet { changes },
            _ => return Err(format!("the {kind} payload is not an object")),
        },
        "secret_set" => {
            let SecretPayload { name, value } = decode(kind, payload)?;
            Request::SecretSet { name, value }
        }
        "retry" => Request::Retry { job: decode::<JobPayload>(kind, payload)?.job },
        "cancel" => Request::Cancel { job: decode::<JobPayload>(kind, payload)?.job },
        "discard" => Request::Discard { job: decode::<JobPayload>(kind, payload)?.job },
        "rip_again" => Request::RipAgain { job: decode::<JobPayload>(kind, payload)?.job },
        "history" => Request::History { before: decode::<HistoryPayload>(kind, payload)?.before },
        "shutdown" => Request::Shutdown { force: decode::<ShutdownPayload>(kind, payload)?.force },
        "command" => Request::Command { words: decode::<CommandPayload>(kind, payload)?.words },
        "review_action" => {
            let ReviewPayload { job, action, overrides } = decode(kind, payload)?;
            Request::ReviewAction { job, action, overrides }
        }
        _ => return Err(format!("unknown message type \"{kind}\"")),
    })
}

/// The line of an envelope (without its newline, Ruling T7-h). Its keys are in the order v, type, id, payload.
fn envelope_line(kind: &str, id: Option<u64>, payload: Value) -> String {
    let envelope = json!({"v": PROTOCOL_VERSION, "type": kind, "id": id, "payload": payload});
    serde_json::to_string(&envelope).expect("an envelope of JSON values serialises")
}

/// The `ok` reply to request `id`: `{"result": result}` (design part 9.3), without its newline (Ruling T7-h).
/// Def: T7. Use: T26.
pub fn reply_ok(id: Option<u64>, result: Value) -> String {
    envelope_line("ok", id, json!({"result": result}))
}

/// The `error` reply to request `id` (design part 9.3), without its newline (Ruling T7-h). `problems` is always
/// present, an empty list unless a `settings_set` was refused (Ruling T7-b). Def: T7. Use: T26.
pub fn reply_error(id: Option<u64>, message: &str, problems: &[SettingsProblem]) -> String {
    envelope_line("error", id, json!({"message": message, "problems": problems}))
}

/// Which stream a `command_output` line came from (design part 9.3). Def: T7. Use: T26, T28.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputStream {
    /// The command's standard output.
    Stdout,
    /// The command's standard error.
    Stderr,
}

/// A message the engine sends without being asked (design part 9.3). Def: T7. Use: T26, T28.
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)] // built, written and dropped at once; the sheet fixes the unboxed variants
pub enum Push {
    /// `snapshot`: everything a client shows, sent when it connects.
    Snapshot(Snapshot),
    /// `job_update`: a job changed.
    JobUpdate(JobUpdate),
    /// `progress`: a running step's progress.
    Progress(Progress),
    /// `selftest_result`: the self-test finished.
    SelftestResult(SelfTest),
    /// `command_output`: one line of a `command` request's output, tagged with that request's id.
    CommandOutput {
        /// The stream the line came from.
        stream: OutputStream,
        /// The line, without its newline.
        line: String,
    },
}

/// The line of `push`, without its newline (Ruling T7-h). `id` is the request's id for `CommandOutput` and is ignored
/// for every other push, which carries `null` (design part 9.3). Def: T7. Use: T26.
pub fn push_line(push: &Push, id: Option<u64>) -> String {
    fn payload<T: serde::Serialize>(value: &T) -> Value {
        serde_json::to_value(value).expect("a push payload serialises")
    }
    match push {
        Push::Snapshot(snapshot) => envelope_line("snapshot", None, payload(snapshot)),
        Push::JobUpdate(update) => envelope_line("job_update", None, payload(update)),
        Push::Progress(progress) => envelope_line("progress", None, payload(progress)),
        Push::SelftestResult(selftest) => envelope_line("selftest_result", None, payload(selftest)),
        Push::CommandOutput { stream, line } => {
            envelope_line("command_output", id, json!({"stream": stream, "line": line}))
        }
    }
}

/// A line the engine sent, as a client reads it (design part 9.3). Def: T7. Use: T28.
#[derive(Clone, Debug, PartialEq)]
pub enum Reply {
    /// An `ok` reply to request `id`.
    Ok {
        /// The request's id.
        id: Option<u64>,
        /// The reply's `result`, `null` when it has none.
        result: Value,
    },
    /// An `error` reply to request `id`.
    Error {
        /// The request's id.
        id: Option<u64>,
        /// The error text.
        message: String,
        /// The refused settings, empty unless a `settings_set` was refused.
        problems: Vec<SettingsProblem>,
    },
    /// Any other type: a push, left as its envelope.
    Push(Envelope),
}

/// Reads one line of the engine as a `Reply` (design part 9.3). Err is a plain sentence: the line is not an
/// envelope, the engine speaks another protocol version, or an `error` reply is malformed. Def: T7. Use: T28.
pub fn read_reply(line: &str) -> Result<Reply, String> {
    let mut envelope: Envelope =
        serde_json::from_str(line).map_err(|e| format!("the engine's line is not a message: {e}"))?;
    if envelope.v != PROTOCOL_VERSION {
        return Err(format!(
            "the engine speaks protocol version {}; this client speaks version {PROTOCOL_VERSION}",
            envelope.v
        ));
    }
    match envelope.kind.as_str() {
        "ok" => {
            let result = envelope.payload.get_mut("result").map(Value::take).unwrap_or(Value::Null);
            Ok(Reply::Ok { id: envelope.id, result })
        }
        "error" => {
            let Some(message) = envelope.payload.get("message").and_then(Value::as_str) else {
                return Err("the engine's error reply has no message".to_string());
            };
            let problems = match envelope.payload.get("problems") {
                None | Some(Value::Null) => Vec::new(),
                Some(list) => serde_json::from_value(list.clone())
                    .map_err(|e| format!("the engine's error reply has a problems list that does not fit: {e}"))?,
            };
            Ok(Reply::Error { id: envelope.id, message: message.to_string(), problems })
        }
        _ => Ok(Reply::Push(envelope)),
    }
}

/// A job as the job list shows it (design part 9.3). `state` nests as `State` serialises, a `{"state": ...}` object.
/// Every `Option` serialises as `null` and is never omitted (Ruling T7-a). Def: T7. Use: T26, T28.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct JobSummary {
    /// The job's ID.
    pub id: String,
    /// The job's state.
    pub state: State,
    /// The plan's identity title, None without a plan (Ruling T7-d).
    pub title: Option<String>,
    /// The disc's fingerprint.
    pub fingerprint: String,
    /// The drive's ID the job runs on.
    pub drive: String,
    /// When the job was created, in Unix seconds.
    pub created_unix: u64,
    /// When the record was last written, in Unix seconds.
    pub updated_unix: u64,
    /// The review reasons of the plan and the titles, sorted and unique (Ruling T7-c).
    pub review_reasons: Vec<ReviewReason>,
    /// The record's notes.
    pub notes: Vec<Noted>,
    /// The failure class, Some only for a `Failed` job (Ruling T7-c).
    pub class: Option<FailureClass>,
    /// The state's message, Some only for a `Failed` job (Ruling T7-c).
    pub message: Option<String>,
}

/// The summary of `record` (design part 9.3). `review_reasons` is `sorted_unique` over the plan's `reviews`, each
/// title's `rip_reviews` and each title's `decision.reviews`; `class` and `message` are Some only when the state is
/// `Failed` (Ruling T7-c); `title` is the plan's identity title (Ruling T7-d). Def: T7. Use: T26.
pub fn job_summary(record: &JobRecord) -> JobSummary {
    let mut reasons: Vec<ReviewReason> = record.plan.iter().flat_map(|plan| plan.reviews.iter().copied()).collect();
    for title in &record.titles {
        reasons.extend(title.rip_reviews.iter().copied());
        reasons.extend(title.decision.iter().flat_map(|decision| decision.reviews.iter().copied()));
    }
    let (class, message) = match &record.state {
        State::Failed { class, .. } => (Some(*class), record.message.clone()),
        _ => (None, None),
    };
    JobSummary {
        id: record.id.clone(),
        state: record.state.clone(),
        title: record.plan.as_ref().and_then(|plan| plan.identity.title.clone()),
        fingerprint: record.fingerprint.clone(),
        drive: record.drive.clone(),
        created_unix: record.created_unix,
        updated_unix: record.updated_unix,
        review_reasons: sorted_unique(reasons),
        notes: record.notes.clone(),
        class,
        message,
    }
}

/// The `job_update` push (design part 9.3): the job's summary flattened beside its plan and the collisions found at
/// filing, None when it has none. Def: T7. Use: T26, T28.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct JobUpdate {
    /// The job's summary, whose keys sit at the top level.
    #[serde(flatten)]
    pub summary: JobSummary,
    /// The plan, once planned.
    pub plan: Option<Plan>,
    /// The collisions found at filing, if any were looked for.
    pub collisions: Option<Vec<String>>,
}

/// The `snapshot` push (design part 9.3): everything a client shows when it connects. Def: T7. Use: T26, T28.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Snapshot {
    /// The jobs, as summaries.
    pub jobs: Vec<JobSummary>,
    /// The settings in force.
    pub settings: Settings,
    /// The problems `Settings::validate` finds in them, then the engine's own when `staging_root` is unset (design
    /// part 9.4: the engine starts no job until it is set).
    pub problems: Vec<SettingsProblem>,
    /// Which secrets are set, by name; a value is never sent.
    pub secrets: BTreeMap<String, SecretState>,
    /// The last self-test.
    pub selftest: SelfTest,
}

/// Whether a secret is set, and for a webhook URL its host, never the secret itself (design parts 9.3, 9.4).
/// Def: T7. Use: T26, T28.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SecretState {
    /// Whether a value is stored.
    pub set: bool,
    /// The host of a stored URL, or None.
    pub host: Option<String>,
}

/// The `progress` push (design part 9.3). `step` is the name of the running `Action`, as its serde tag spells it
/// (Ruling S7). Def: T7. Use: T26, T28.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Progress {
    /// The job's ID.
    pub job: String,
    /// The running step.
    pub step: String,
    /// The title key the step concerns, or None.
    pub title: Option<String>,
    /// How far the step is, from 0.0 to 1.0.
    pub fraction: f64,
}

/// The self-test's result (spec 4.5; design part 9.3; Ruling S8): one check per required program and place. Def: T7.
/// Use: T26, T28.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SelfTest {
    /// The checks, in the order they ran.
    pub checks: Vec<SelfTestCheck>,
}

/// One check of the self-test (spec 4.5). Def: T7. Use: T26, T28.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SelfTestCheck {
    /// What was checked.
    pub name: String,
    /// Whether it passed.
    pub ok: bool,
    /// One plain sentence of detail.
    pub detail: String,
    /// The path the check used, or None.
    pub path: Option<String>,
    /// The version found, or None.
    pub version: Option<String>,
    /// Where the tool was found: `settings`, `resources`, `install_folder` or `path` (design parts 3.6, 9.2); None for
    /// a check that is not a tool's, or a tool that was not found.
    pub found: Option<String>,
    /// Whether the check could not run because a job was using the resource.
    pub busy: bool,
}

/// Spec 12's notification: the event flattened (its `event` tag and fields) beside the job, the message and the time
/// the engine adds. Every `Option` serialises as `null` (Ruling T7-a); `time` is Unix seconds (Ruling S42).
/// Def: T7. Use: T22.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Notification {
    /// The event.
    #[serde(flatten)]
    pub event: NotifyEvent,
    /// The job's ID, or None for an event of no job.
    pub job_id: Option<String>,
    /// The disc's fingerprint, or None.
    pub fingerprint: Option<String>,
    /// The title the job is filed under, or None.
    pub title: Option<String>,
    /// The drive's ID, or None.
    pub drive: Option<String>,
    /// One plain sentence for the owner.
    pub message: String,
    /// When the event happened, in Unix seconds.
    pub time: u64,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;
    use crate::job::{FailureClass, NotifyEvent, State, WaitReason};
    use crate::record::{JobRecord, TitleRecord, JOB_FORMAT};
    use crate::review::ReviewReason;
    use crate::settings::{Settings, SettingsProblem};

    fn record(state: State, message: Option<&str>) -> JobRecord {
        JobRecord {
            format: JOB_FORMAT.to_string(),
            id: "abc-1".to_string(),
            fingerprint: "ABC".to_string(),
            disc_id: None,
            drive: "tok0".to_string(),
            created_unix: 1_800_000_000,
            updated_unix: 1_800_000_060,
            state,
            message: message.map(str::to_string),
            rip_again: false,
            profile: "job.mmcp.xml".to_string(),
            plan: None,
            titles: vec![],
            filed_unix: None,
            collisions: vec![],
            notes: vec![],
        }
    }

    fn title(key: &str, rip_reviews: Vec<ReviewReason>) -> TitleRecord {
        TitleRecord { key: key.to_string(), scan_title: 0, rip_index: Some(0), attempts: 2, verified: true,
            verify_failures: 0, read_errors: vec![], rip_reviews, decision: None, filed: None }
    }

    /// The error reply `parse_request` refused `line` with, read back.
    fn refusal(line: &str) -> Reply {
        read_reply(&parse_request(line).unwrap_err()).unwrap()
    }

    fn error(id: Option<u64>, message: &str) -> Reply {
        Reply::Error { id, message: message.to_string(), problems: vec![] }
    }

    #[test]
    fn a_request_with_v_2_is_an_error_reply_with_its_id() {
        assert_eq!(refusal(r#"{"v":2,"type":"retry","id":7,"payload":{"job":"a-1"}}"#),
            error(Some(7), "protocol version 2 is not supported; this engine speaks version 1"));
        assert_eq!(refusal(r#"{"type":"retry","id":8,"payload":{"job":"a-1"}}"#),
            error(Some(8), "the message has no protocol version"));
    }

    #[test]
    fn an_unknown_type_is_an_error_reply() {
        assert_eq!(refusal(r#"{"v":1,"type":"frobnicate","id":5,"payload":null}"#),
            error(Some(5), "unknown message type \"frobnicate\""));
        assert_eq!(refusal(r#"{"v":1,"id":6,"payload":null}"#), error(Some(6), "the message has no type"));
    }

    #[test]
    fn a_line_over_4_mib_is_refused_before_parsing() {
        let long = format!(r#"{{"v":1,"type":"retry","id":9,"payload":{{"job":"{}"}}}}"#, "a".repeat(MAX_LINE_BYTES));
        assert_eq!(refusal(&long), error(None, LINE_TOO_LONG));
        assert_eq!(LINE_TOO_LONG, "the line is longer than 4 MiB");
        let short = r#"{"v":1,"type":"run_selftest","id":1,"payload":null}"#;
        let exact = format!("{short}{}", " ".repeat(MAX_LINE_BYTES - short.len()));
        assert_eq!(exact.len(), MAX_LINE_BYTES);
        assert_eq!(parse_request(&exact), Ok((Some(1), Request::RunSelftest)));
    }

    #[test]
    fn a_line_that_is_not_json_is_an_error_reply_with_no_id() {
        let Reply::Error { id, message, problems } = refusal("not json") else { panic!("not an error reply") };
        assert_eq!(id, None);
        assert!(message.starts_with("the line is not a JSON message: "), "{message}");
        assert!(problems.is_empty());
    }

    #[test]
    fn unknown_payload_fields_are_ignored() {
        assert_eq!(parse_request(r#"{"v":1,"type":"retry","id":3,"payload":{"job":"a-1","extra":1},"later":true}"#),
            Ok((Some(3), Request::Retry { job: "a-1".to_string() })));
    }

    #[test]
    fn a_payload_missing_a_field_is_an_error_reply_naming_the_type() {
        assert_eq!(refusal(r#"{"v":1,"type":"retry","id":4,"payload":{}}"#),
            error(Some(4), "the retry payload does not fit: missing field `job`"));
        assert_eq!(refusal(r#"{"v":1,"type":"retry","id":4,"payload":null}"#),
            error(Some(4), "the retry payload does not fit: missing field `job`"));
        assert_eq!(refusal(r#"{"v":1,"type":"settings_set","id":2,"payload":[1]}"#),
            error(Some(2), "the settings_set payload is not an object"));
    }

    #[test]
    fn a_settings_set_payload_is_the_changed_keys_themselves() {
        let (id, request) = parse_request(r#"{"v":1,"type":"settings_set","id":1,"payload":{"cache_mb":512}}"#).unwrap();
        assert_eq!(id, Some(1));
        let Request::SettingsSet { changes } = request else { panic!("not settings_set") };
        assert_eq!(serde_json::Value::Object(changes), json!({"cache_mb": 512}));
    }

    #[test]
    fn every_request_type_of_design_part_9_3_parses() {
        let job = || "a-1".to_string();
        let cases: Vec<(&str, Request)> = vec![
            (r#"{"v":1,"type":"secret_set","id":2,"payload":{"name":"tmdb_token","value":"t0k"}}"#,
                Request::SecretSet { name: "tmdb_token".to_string(), value: "t0k".to_string() }),
            (r#"{"v":1,"type":"retry","id":2,"payload":{"job":"a-1"}}"#, Request::Retry { job: job() }),
            (r#"{"v":1,"type":"cancel","id":2,"payload":{"job":"a-1"}}"#, Request::Cancel { job: job() }),
            (r#"{"v":1,"type":"discard","id":2,"payload":{"job":"a-1"}}"#, Request::Discard { job: job() }),
            (r#"{"v":1,"type":"rip_again","id":2,"payload":{"job":"a-1"}}"#, Request::RipAgain { job: job() }),
            (r#"{"v":1,"type":"run_selftest","id":2,"payload":null}"#, Request::RunSelftest),
            (r#"{"v":1,"type":"history","id":2,"payload":{"before":"a-1"}}"#, Request::History { before: Some(job()) }),
            (r#"{"v":1,"type":"history","id":2,"payload":{}}"#, Request::History { before: None }),
            (r#"{"v":1,"type":"shutdown","id":2,"payload":{"force":true}}"#, Request::Shutdown { force: true }),
            (r#"{"v":1,"type":"command","id":2,"payload":{"words":["plan","--job","a-1"]}}"#,
                Request::Command { words: vec!["plan".to_string(), "--job".to_string(), job()] }),
            (r#"{"v":1,"type":"review_action","id":2,"payload":{"job":"a-1","action":"approve"}}"#,
                Request::ReviewAction { job: job(), action: "approve".to_string(), overrides: None }),
            (r#"{"v":1,"type":"review_action","id":2,"payload":{"job":"a-1","action":"approve","overrides":{}}}"#,
                Request::ReviewAction { job: job(), action: "approve".to_string(), overrides: Some(json!({})) }),
            (r#"{"v":1,"type":"ui_quit","id":2,"payload":null}"#, Request::UiQuit),
        ];
        for (line, expected) in cases {
            assert_eq!(parse_request(line), Ok((Some(2), expected)), "{line}");
        }
        assert_eq!(parse_request(r#"{"v":1,"type":"ui_quit"}"#), Ok((None, Request::UiQuit)));
    }

    #[test]
    fn ok_and_error_replies_read_back() {
        let ok = reply_ok(Some(3), json!({"exit_code": 0}));
        assert_eq!(ok, r#"{"v":1,"type":"ok","id":3,"payload":{"result":{"exit_code":0}}}"#);
        assert_eq!(read_reply(&ok), Ok(Reply::Ok { id: Some(3), result: json!({"exit_code": 0}) }));
        assert_eq!(read_reply(&reply_ok(Some(4), serde_json::Value::Null)),
            Ok(Reply::Ok { id: Some(4), result: serde_json::Value::Null }));
        assert_eq!(read_reply(r#"{"v":2,"type":"ok","id":3,"payload":{"result":null}}"#),
            Err("the engine speaks protocol version 2; this client speaks version 1".to_string()));
    }

    #[test]
    fn pushes_carry_a_null_id_except_command_output() {
        let progress = Push::Progress(Progress { job: "abc-1".to_string(), step: "RipTitle".to_string(),
            title: Some("t00-00800".to_string()), fraction: 0.5 });
        assert_eq!(push_line(&progress, Some(9)),
            r#"{"v":1,"type":"progress","id":null,"payload":{"job":"abc-1","step":"RipTitle","title":"t00-00800","fraction":0.5}}"#);
        let output = Push::CommandOutput { stream: OutputStream::Stderr, line: "plan: refused".to_string() };
        let line = push_line(&output, Some(9));
        assert_eq!(line, r#"{"v":1,"type":"command_output","id":9,"payload":{"stream":"stderr","line":"plan: refused"}}"#);
        let Ok(Reply::Push(envelope)) = read_reply(&line) else { panic!("not a push") };
        assert_eq!((envelope.kind.as_str(), envelope.id), ("command_output", Some(9)));
        assert_eq!(push_line(&Push::SelftestResult(SelfTest::default()), Some(9)),
            r#"{"v":1,"type":"selftest_result","id":null,"payload":{"checks":[]}}"#);
    }

    #[test]
    fn a_summary_nests_the_state_as_it_serialises() {
        let failed = State::Failed { class: FailureClass::Unknown, from: Box::new(State::Scanned) };
        let summary = job_summary(&record(failed, Some(CANCELLED)));
        assert_eq!(serde_json::to_string(&summary).unwrap(), concat!(
            r#"{"id":"abc-1","state":{"state":"Failed","class":"Unknown","from":{"state":"Scanned"}},"title":null,"#,
            r#""fingerprint":"ABC","drive":"tok0","created_unix":1800000000,"updated_unix":1800000060,"#,
            r#""review_reasons":[],"notes":[],"class":"Unknown","message":"cancelled by the owner"}"#));
        let running = job_summary(&record(State::Ripping, Some("ripping t00-00800")));
        assert_eq!((running.class, running.message), (None, None));
    }

    #[test]
    fn a_summary_gathers_the_review_reasons_sorted_and_unique() {
        let mut rec = record(State::Ripping, None);
        rec.titles = vec![
            title("t00-00800", vec![ReviewReason::ReadErrors, ReviewReason::VerifyFailed]),
            title("t01-00801", vec![ReviewReason::VerifyFailed]),
        ];
        assert_eq!(job_summary(&rec).review_reasons, vec![ReviewReason::VerifyFailed, ReviewReason::ReadErrors]);
    }

    #[test]
    fn a_job_update_flattens_its_summary_beside_plan_and_collisions() {
        let update = JobUpdate { summary: job_summary(&record(State::Verified, None)), plan: None,
            collisions: Some(vec!["/lib/Movies/A (2001)/A (2001).mkv".to_string()]) };
        let value = serde_json::to_value(&update).unwrap();
        let keys: Vec<&str> = value.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys, ["id", "state", "title", "fingerprint", "drive", "created_unix", "updated_unix",
            "review_reasons", "notes", "class", "message", "plan", "collisions"]);
        assert_eq!(serde_json::from_value::<JobUpdate>(value).unwrap(), update);
    }

    #[test]
    fn a_snapshot_push_has_its_five_keys_in_order() {
        let snapshot = Snapshot { jobs: vec![job_summary(&record(State::Filed, None))], settings: Settings::default(),
            problems: Settings::default().validate(),
            secrets: BTreeMap::from([("tmdb_token".to_string(), SecretState { set: false, host: None })]),
            selftest: SelfTest::default() };
        let Ok(Reply::Push(envelope)) = read_reply(&push_line(&Push::Snapshot(snapshot.clone()), Some(1))) else {
            panic!("not a push")
        };
        assert_eq!((envelope.kind.as_str(), envelope.id), ("snapshot", None));
        let keys: Vec<&str> = envelope.payload.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys, ["jobs", "settings", "problems", "secrets", "selftest"]);
        assert_eq!(serde_json::from_value::<Snapshot>(envelope.payload).unwrap(), snapshot);
    }

    #[test]
    fn a_refused_settings_set_lists_key_and_problem() {
        let problems = [SettingsProblem { key: "tuning_root".to_string(), problem: "tuning_root is not set".to_string() }];
        let line = reply_error(Some(4), "the settings were refused", &problems);
        assert_eq!(line, concat!(r#"{"v":1,"type":"error","id":4,"payload":{"message":"the settings were refused","#,
            r#""problems":[{"key":"tuning_root","problem":"tuning_root is not set"}]}}"#));
        assert_eq!(read_reply(&line), Ok(Reply::Error { id: Some(4), message: "the settings were refused".to_string(),
            problems: problems.to_vec() }));
        assert_eq!(reply_error(None, "x", &[]), r#"{"v":1,"type":"error","id":null,"payload":{"message":"x","problems":[]}}"#);
    }

    #[test]
    fn a_notification_flattens_its_event() {
        let failed = Notification { event: NotifyEvent::Failed { class: FailureClass::Unknown }, job_id: None,
            fingerprint: None, title: None, drive: Some("tok0".to_string()),
            message: "the volume could not be read".to_string(), time: 1_800_000_000 };
        let text = serde_json::to_string(&failed).unwrap();
        assert_eq!(text, concat!(r#"{"event":"failed","class":"Unknown","job_id":null,"fingerprint":null,"title":null,"#,
            r#""drive":"tok0","message":"the volume could not be read","time":1800000000}"#));
        assert_eq!(serde_json::from_str::<Notification>(&text).unwrap(), failed);
        let waiting = Notification { event: NotifyEvent::Waiting { reason: WaitReason::Lookup },
            job_id: Some("abc-1".to_string()), fingerprint: Some("ABC".to_string()), title: None,
            drive: Some("tok0".to_string()), message: "a lookup is pending".to_string(), time: 1 };
        let text = serde_json::to_string(&waiting).unwrap();
        assert!(text.starts_with(r#"{"event":"waiting","reason":"lookup","job_id":"abc-1""#), "{text}");
        assert_eq!(serde_json::from_str::<Notification>(&text).unwrap(), waiting);
    }

    #[test]
    fn share_secret_names_are_accepted() {
        for name in SECRET_NAMES {
            assert!(is_secret_name(name), "{name}");
        }
        assert!(is_secret_name(r"share:\\nas\media"));
        for name in ["tmdb", "", r"share:\\nas", r"share:\\\media", r"share:\\nas\", r"share:\\nas\media\films", r"share:nas\media"] {
            assert!(!is_secret_name(name), "{name}");
        }
    }
}
