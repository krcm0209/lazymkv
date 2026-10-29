//! What the engine's IPC tests share (Ruling PF3-S21-22-2): `Client`, one client over the harness's in-memory pipe,
//! and `WAIT`, the bound of its reads. engine_ipc.rs and engine_command.rs each declare `mod common;` and use a part
//! of it, so the parts one test crate leaves unused are allowed.

#![allow(dead_code)]

use std::collections::VecDeque;
use std::io::{BufRead, Write};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::time::Duration;

use lazymkv_core::ipc::{read_reply, Envelope, Reply};
use lazymkv_engine::testkit::{ci_factor, Harness};
use lazymkv_platform::IpcConnector;
use serde_json::{json, Value};

/// The tests' own waits: 20 s, times `ci_factor` wherever a test waits for something to happen (Rulings R29.c2,
/// R29.c5).
pub const WAIT: Duration = Duration::from_secs(20);

/// One client over the harness's in-memory pipe: a reader thread hands each line the engine writes to `lines`; `next`
/// is the id of the next request, and `pushes` keeps the pushes a read for a reply passed over.
pub struct Client {
    pub lines: Receiver<String>,
    pub writer: Box<dyn Write + Send>,
    pub next: u64,
    pub pushes: VecDeque<Envelope>,
}

/// A `command_output` push's (stream, line) (design part 9.3).
fn output_of(envelope: &Envelope) -> (String, String) {
    let text = |key: &str| {
        envelope.payload[key].as_str().unwrap_or_else(|| panic!("no {key} in {envelope:?}")).to_string()
    };
    (text("stream"), text("line"))
}

impl Client {
    pub fn open(h: &Harness) -> Client {
        let stream = h.client().connect().unwrap_or_else(|_| panic!("the engine accepts the client"));
        let (send, lines) = channel();
        let mut reader = stream.reader;
        std::thread::spawn(move || loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => if send.send(line).is_err() { break },
            }
        });
        Client { lines, writer: stream.writer, next: 1, pushes: VecDeque::new() }
    }

    /// The next line, or None once the engine closed the connection.
    pub fn line(&mut self) -> Option<String> {
        // 20 s, 80 s under CI (Ruling R29.c2).
        let wait = WAIT * ci_factor();
        match self.lines.recv_timeout(wait) {
            Ok(line) => Some(line),
            Err(RecvTimeoutError::Disconnected) => None,
            Err(RecvTimeoutError::Timeout) => panic!("no line from the engine within {wait:?}"),
        }
    }

    /// The next line as a reply or a push; a panic names the timeout (`line`) or the closed connection.
    pub fn read(&mut self) -> Reply {
        let line = self.line().unwrap_or_else(|| panic!("the engine closed the connection"));
        read_reply(line.trim_end()).unwrap()
    }

    /// Writes `line` and its newline in one write: the engine answers a line over 4 MiB as soon as it has read that
    /// much and then drops the client, so a second write could meet a closed pipe.
    pub fn raw(&mut self, line: &str) {
        self.writer.write_all(format!("{line}\n").as_bytes()).unwrap();
        self.writer.flush().unwrap();
    }

    /// Sends `kind` with the next id (`next`, then incremented); returns that id.
    pub fn send(&mut self, kind: &str, payload: Value) -> u64 {
        let id = self.next;
        self.next += 1;
        self.raw(&json!({"v": 1, "type": kind, "id": id, "payload": payload}).to_string());
        id
    }

    /// Sends `kind` and returns the reply to it (`reply_to`).
    pub fn request(&mut self, kind: &str, payload: Value) -> Reply {
        let id = self.send(kind, payload);
        self.reply_to(id).1
    }

    /// Each `command_output` of request `id` as (stream, line), in order, and the reply to `id`. A `command_output`
    /// carrying another id fails the test; other pushes go to `pushes`.
    pub fn reply_to(&mut self, id: u64) -> (Vec<(String, String)>, Reply) {
        let mut outputs = Vec::new();
        loop {
            match self.read() {
                Reply::Push(envelope) if envelope.kind == "command_output" => {
                    assert_eq!(envelope.id, Some(id), "a command_output of another request: {envelope:?}");
                    outputs.push(output_of(&envelope));
                }
                Reply::Push(envelope) => self.pushes.push_back(envelope),
                reply @ (Reply::Ok { id: Some(n), .. } | Reply::Error { id: Some(n), .. }) if n == id => {
                    return (outputs, reply)
                }
                other => panic!("unexpected {other:?}"),
            }
        }
    }

    /// `send("command", {"words": words})`, then `reply_to` its id.
    pub fn command(&mut self, words: &[&str]) -> (Vec<(String, String)>, Reply) {
        let id = self.send("command", json!({ "words": words }));
        self.reply_to(id)
    }

    pub fn ok(&mut self, kind: &str, payload: Value) -> Value {
        match self.request(kind, payload) { Reply::Ok { result, .. } => result, other => panic!("{kind}: {other:?}") }
    }

    pub fn err(&mut self, kind: &str, payload: Value) -> String {
        match self.request(kind, payload) { Reply::Error { message, .. } => message, other => panic!("{kind}: {other:?}") }
    }

    pub fn push(&mut self) -> Envelope {
        if let Some(envelope) = self.pushes.pop_front() { return envelope; }
        match self.read() {
            Reply::Push(envelope) => envelope,
            other => panic!("expected a push, got {other:?}"),
        }
    }

    pub fn snapshot(&mut self) -> Value {
        let envelope = self.push();
        assert_eq!(envelope.kind, "snapshot");
        envelope.payload
    }

    /// job_update pushes until one whose state is `state` (for `job` when given); returns its payload.
    pub fn until_state(&mut self, job: Option<&str>, state: &str) -> Value {
        loop {
            let envelope = self.push();
            if envelope.kind == "job_update" && envelope.payload["state"]["state"] == json!(state)
                && job.map_or(true, |id| envelope.payload["id"] == json!(id)) {
                return envelope.payload;
            }
        }
    }

    /// The next `command_output` as (stream, line); other pushes go to `pushes`.
    pub fn output(&mut self) -> (String, String) {
        loop {
            match self.read() {
                Reply::Push(envelope) if envelope.kind == "command_output" => return output_of(&envelope),
                Reply::Push(envelope) => self.pushes.push_back(envelope),
                other => panic!("expected a command_output, got {other:?}"),
            }
        }
    }
}
