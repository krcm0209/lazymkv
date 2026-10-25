//! The transport that replays recorded answers keyed by URL (design part 8.1), which gives spec 15's recorded HTTP
//! responses with no server and no network. Answers come from `answer` and `fail`, or from an `http.json` file (S63).
//! Every request is logged when it arrives, before its gate is waited on (S64), so a test can see a request in flight.
//! The log holds each request's URL and headers in memory only and nothing writes it out, so a bearer token sent in a
//! header never reaches a file or a log line. Used by Tasks 16, 22, 23, 24, 26 and 29.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};

use serde_json::Value;

use crate::transport::{HttpTransport, Response, TransportError};

/// A held request's gate: the request waits in `get` until a test calls [`Gate::open`] (S64). Design part 8.1.
/// Def: T15. Use: T23, T24.
#[derive(Clone)]
pub struct Gate(Arc<(Mutex<bool>, Condvar)>);

impl Gate {
    fn closed() -> Gate {
        Gate(Arc::new((Mutex::new(false), Condvar::new())))
    }

    /// Releases every request waiting on this gate, and any that arrives later. Design part 8.1. Def: T15. Use: T23,
    /// T24.
    pub fn open(&self) {
        let (opened, changed) = &*self.0;
        *locked(opened) = true;
        changed.notify_all();
    }

    fn wait(&self) {
        let (opened, changed) = &*self.0;
        let mut opened = locked(opened);
        while !*opened {
            opened = changed.wait(opened).unwrap_or_else(PoisonError::into_inner);
        }
    }
}

/// A panic in a test thread must not hide the transport's state from the assertions that follow, so a poisoned lock is
/// read as it stands.
fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

type Answer = Result<Response, TransportError>;

/// The answers recorded for one URL, in the order they are served, and how many have been served.
#[derive(Default)]
struct Slot {
    answers: Vec<Answer>,
    served: usize,
}

#[derive(Default)]
struct State {
    slots: HashMap<String, Slot>,
    requests: Vec<(String, Vec<(String, String)>)>,
    unrecorded: Vec<String>,
    gates: HashMap<String, Gate>,
}

/// Replays recorded answers by URL. Several answers for one URL are served in the order they were added, and the last
/// one repeats once the others are used. A URL with no answer gives `Unrecorded` and is listed by `unrecorded`.
/// Design part 8.1. Def: T15. Use: T16, T22, T23, T24, T26, T29.
#[derive(Default)]
pub struct RecordedTransport {
    state: Mutex<State>,
}

impl RecordedTransport {
    /// A transport with no answers. Design part 8.1. Def: T15. Use: T22, T26.
    pub fn new() -> RecordedTransport {
        RecordedTransport::default()
    }

    /// Adds an answer for `url`. It takes `&self`, so a test adds answers to the transport the engine already holds
    /// (S63). Design part 8.1. Def: T15. Use: T16, T24, T26.
    pub fn answer(&self, url: &str, status: u16, body: &[u8]) {
        self.push(url, Ok(Response { status, body: body.to_vec() }));
    }

    /// Adds a failure for `url`, served in turn with its answers. Design part 8.1. Def: T15. Use: T16, T24.
    pub fn fail(&self, url: &str, error: TransportError) {
        self.push(url, Err(error));
    }

    fn push(&self, url: &str, answer: Answer) {
        locked(&self.state).slots.entry(url.to_string()).or_default().answers.push(answer);
    }

    /// Reads `{"answers":[..]}` from an `http.json` file (S63). Each entry has a `url`, in which every `{name}` is
    /// replaced from `vars`, and either a `status` or an `error` (`"timeout"` gives `Timeout`, any other text gives
    /// `Unreachable` with that text; an `error` wins over a `status`). The body is `body`, a string's UTF-8 bytes, else
    /// the bytes of `body_file`, read relative to the folder of the file, else empty. Entries for one URL are served in
    /// file order. Err names the file, and the entry's index where an entry is at fault. Design part 8.1. Def: T15.
    /// Use: T23, T29.
    pub fn from_file(path: &Path, vars: &[(&str, &str)]) -> Result<RecordedTransport, String> {
        let shown = path.display();
        let text = std::fs::read_to_string(path).map_err(|error| format!("{shown}: {error}"))?;
        let document: Value = serde_json::from_str(&text).map_err(|error| format!("{shown}: {error}"))?;
        let entries = document
            .get("answers")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("{shown}: there is no \"answers\" array"))?;
        let folder = path.parent().unwrap_or_else(|| Path::new(""));
        let transport = RecordedTransport::new();
        for (index, entry) in entries.iter().enumerate() {
            let url = entry
                .get("url")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("{shown}: answer {index} has no url"))?;
            let url = vars
                .iter()
                .fold(url.to_string(), |url, (name, value)| url.replace(&format!("{{{name}}}"), value));
            if let Some(error) = entry.get("error").and_then(Value::as_str) {
                let error = match error {
                    "timeout" => TransportError::Timeout,
                    other => TransportError::Unreachable(other.to_string()),
                };
                transport.fail(&url, error);
                continue;
            }
            let status = entry
                .get("status")
                .and_then(Value::as_u64)
                .and_then(|status| u16::try_from(status).ok())
                .ok_or_else(|| {
                    format!("{shown}: answer {index} has neither a status (a number up to 65535) nor an error text")
                })?;
            let body = entry.get("body").and_then(Value::as_str);
            let body = match (body, entry.get("body_file").and_then(Value::as_str)) {
                (Some(body), _) => body.as_bytes().to_vec(),
                (None, Some(name)) => {
                    let file = folder.join(name);
                    std::fs::read(&file).map_err(|error| format!("{}: {error}", file.display()))?
                }
                (None, None) => Vec::new(),
            };
            transport.answer(&url, status, &body);
        }
        Ok(transport)
    }

    /// Holds every request for `url` until the returned gate is opened. Requests already waiting on an earlier gate
    /// for the same URL stay with that one. Design part 8.1. Def: T15. Use: T23, T24.
    pub fn gate(&self, url: &str) -> Gate {
        let gate = Gate::closed();
        locked(&self.state).gates.insert(url.to_string(), gate.clone());
        gate
    }

    /// Every request so far, in arrival order: the URL and the headers as sent, a request still waiting on its gate
    /// included (S64). Design part 8.1. Def: T15. Use: T23, T24, T26, T29.
    pub fn requests(&self) -> Vec<(String, Vec<(String, String)>)> {
        locked(&self.state).requests.clone()
    }

    /// The URL of each request that had no recorded answer, in arrival order. Design part 8.1. Def: T15. Use: T29.
    pub fn unrecorded(&self) -> Vec<String> {
        locked(&self.state).unrecorded.clone()
    }
}

/// Design part 8.1. Def: T15. Use: T16.
impl HttpTransport for RecordedTransport {
    fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<Response, TransportError> {
        let gate = {
            let mut state = locked(&self.state);
            let sent = headers.iter().map(|(name, value)| (name.to_string(), value.to_string())).collect();
            state.requests.push((url.to_string(), sent));
            state.gates.get(url).cloned()
        };
        if let Some(gate) = gate {
            gate.wait();
        }
        let mut state = locked(&self.state);
        let State { slots, unrecorded, .. } = &mut *state;
        match slots.get_mut(url).filter(|slot| !slot.answers.is_empty()) {
            Some(slot) => {
                let answer = slot.answers[slot.served.min(slot.answers.len() - 1)].clone();
                slot.served += 1;
                answer
            }
            None => {
                unrecorded.push(url.to_string());
                Err(TransportError::Unrecorded(url.to_string()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lookups::{discdb_by_disc_id, discdb_by_fingerprint, dischash_url, discid_url, search_url, tmdb_search, Outcome, PendingReason};
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn several_answers_for_one_url_are_served_in_order_then_the_last_repeats() {
        let t = RecordedTransport::new();
        t.answer(&dischash_url("ABC"), 503, b"");
        t.answer(&dischash_url("ABC"), 404, b"");
        assert!(matches!(discdb_by_fingerprint(&t, "ABC"),
            Outcome::Pending { reason: PendingReason::Status(503), fetched: Some(_) }));
        assert!(matches!(discdb_by_fingerprint(&t, "ABC"), Outcome::Miss { .. }));
        assert!(matches!(discdb_by_fingerprint(&t, "ABC"), Outcome::Miss { .. }));
    }

    #[test]
    fn an_unrecorded_url_is_pending_and_listed() {
        let t = RecordedTransport::new();
        let url = discid_url("D1");
        assert_eq!(discdb_by_disc_id(&t, "D1"),
            Outcome::Pending { reason: PendingReason::Unreachable(format!("no recorded answer for {url}")), fetched: None });
        assert_eq!(t.unrecorded(), vec![url]);
    }

    #[test]
    fn a_gated_request_waits_until_opened() {
        let t = std::sync::Arc::new(RecordedTransport::new());
        t.answer(&dischash_url("ABC"), 404, b"");
        let gate = t.gate(&dischash_url("ABC"));
        let (sent, got) = mpsc::channel();
        let asking = t.clone();
        std::thread::spawn(move || sent.send(discdb_by_fingerprint(&*asking, "ABC")).unwrap());
        assert!(got.recv_timeout(Duration::from_millis(200)).is_err());
        assert_eq!(t.requests().len(), 1);
        gate.open();
        assert!(matches!(got.recv_timeout(Duration::from_secs(5)).unwrap(), Outcome::Miss { .. }));
    }

    #[test]
    fn http_json_answers_load_with_body_files_and_vars() {
        let dir = std::env::temp_dir().join(format!("lazymkv-clients-http-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("search.json"), r#"{"page":1,"results":[{"media_type":"movie","id":900001,"title":"Synthetic Movie","original_title":"Synthetic Movie","release_date":"2001-02-03"}]}"#).unwrap();
        std::fs::write(dir.join("http.json"), r#"{"answers":[
            {"url":"https://thediscdb.com/api/dischash/{fingerprint}","status":404,"body":""},
            {"url":"https://thediscdb.com/api/discid/{disc_id}","error":"timeout"},
            {"url":"https://api.themoviedb.org/3/search/multi?query=SYNTHETIC%20MOVIE","status":200,"body_file":"search.json"}]}"#).unwrap();
        let t = RecordedTransport::from_file(&dir.join("http.json"), &[("fingerprint", "ABC"), ("disc_id", "D1")]).unwrap();
        assert!(matches!(discdb_by_fingerprint(&t, "ABC"), Outcome::Miss { .. }));
        assert!(matches!(discdb_by_disc_id(&t, "D1"), Outcome::Pending { reason: PendingReason::Timeout, fetched: None }));
        match tmdb_search(&t, Some("token"), "SYNTHETIC MOVIE") {
            Outcome::Answered { value, fetched } => {
                assert_eq!(value.results.len(), 1);
                assert_eq!(fetched.url, search_url("SYNTHETIC MOVIE"));
            }
            other => panic!("expected Answered, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
