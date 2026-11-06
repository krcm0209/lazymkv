//! Notifications (design part 8.6, spec 12): the sink they go to, the message each event carries, the notifier thread
//! that delivers them in order and engine.log's line for a notification the sink refuses (spec 12).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use lazymkv_clients::{user_agent, HttpTransport, Response, TransportError, TIMEOUT_S};
use lazymkv_core::ipc::{Notification, WEBHOOK_SECRETS};
use lazymkv_core::job::{FailureClass, NotifyEvent};
use lazymkv_core::record::JobRecord;
use lazymkv_platform::SecretStore;

use crate::store::Store;

/// What a `failed` notification of class `MediaReadError` adds to its message (spec 12): the disc may read in the other
/// drive.
const TRY_THE_OTHER_DRIVE: &str = "; try the other drive";

/// Where notifications go (design part 8.6, S36; spec 12): the engine's notifier is `WebhookSink`, which the binary
/// passes (design part 8.6). Def: T22; T3-21 makes `deliver` return a Result; T4-12 adds `stop_flag`. Use: T22, T26,
/// T29, T3-21, T3-23, T4-12, T4-17.
pub trait NotifySink: Send {
    /// Delivers one notification, or gives why it could not. The notifier then writes `undelivered_line` to engine.log
    /// and goes on, so a notification never blocks a job (spec 12, design part 3.5). The reason never holds a secret,
    /// such as a webhook URL (spec 12).
    fn deliver(&mut self, notification: &Notification) -> Result<(), String>;
    /// The flag `EngineHandle::stop` raises so the sink skips its waits (design part 8.6; Ruling T4-R13); None for
    /// a sink that never waits. Def: T4-12. Use: T4-12.
    fn stop_flag(&self) -> Option<Arc<AtomicBool>> {
        None
    }
}

/// The wait before each attempt of one channel, counted from the previous failure (design part 8.6: the second 1 s
/// after the first fails, the third 4 s after the second). Def: T4-12. Use: T4-12.
pub const ATTEMPT_WAITS_S: [u64; 3] = [0, 1, 4];

/// The longest `EngineHandle::stop` waits for the notifier (spec 12: "a restart loses a notification still being
/// retried"; Ruling T4-R13). Def: T4-12. Use: T4-12.
pub const NOTIFY_DRAIN_S: u64 = 15;

/// The production pause's step: how often it looks at the stop flag.
const PAUSE_STEP: Duration = Duration::from_millis(50);

/// The engine's notifier (design part 8.6, spec 12; Rulings T4-R12, T4-R13): each notification POSTed as the JSON of
/// `Notification` to `ha_webhook_url`, then `webhook_url`, each read from the secrets at delivery and skipped when
/// unset, with `Content-Type: application/json` and `User-Agent: lazymkv/<version>`; any 2xx delivers; three attempts
/// per channel. A failed channel's reason names the channel, never its URL. Def: T4-12. Use: T4-12, T4-17, T4-18.
pub struct WebhookSink {
    http: Arc<dyn HttpTransport>,
    secrets: Arc<dyn SecretStore>,
    pause: Arc<dyn Fn(Duration) + Send + Sync>,
    stopping: Arc<AtomicBool>,
}

impl WebhookSink {
    /// The production pause: sleeps in steps of 50 ms until the wait has passed or `stopping` is raised.
    /// Def: T4-12. Use: T4-17.
    pub fn new(http: Arc<dyn HttpTransport>, secrets: Arc<dyn SecretStore>) -> WebhookSink {
        let stopping = Arc::new(AtomicBool::new(false));
        let raised = Arc::clone(&stopping);
        let pause = Arc::new(move |wait: Duration| {
            let end = Instant::now() + wait;
            while !raised.load(Ordering::SeqCst) {
                let rest = end.saturating_duration_since(Instant::now());
                if rest.is_zero() {
                    break;
                }
                thread::sleep(rest.min(PAUSE_STEP));
            }
        });
        WebhookSink { http, secrets, pause, stopping }
    }

    /// `self` with `pause` in place of the production one (tests: a recorder or a no-op). Def: T4-12. Use: T4-12, T4-18.
    pub fn with_pause(self, pause: Arc<dyn Fn(Duration) + Send + Sync>) -> WebhookSink {
        WebhookSink { pause, ..self }
    }

    fn stopping(&self) -> bool {
        self.stopping.load(Ordering::SeqCst)
    }

    /// One channel's attempts: None once a 2xx answers, else the channel's reason (design part 8.6).
    fn try_channel(&self, name: &str, url: &str, headers: &[(&str, &str)], body: &[u8]) -> Option<String> {
        let mut last = String::new();
        for (attempt, wait) in ATTEMPT_WAITS_S.iter().enumerate() {
            if attempt > 0 {
                if !self.stopping() && *wait > 0 {
                    (self.pause)(Duration::from_secs(*wait));
                }
                if self.stopping() {
                    let unit = if attempt == 1 { "attempt" } else { "attempts" };
                    return Some(format!("{name}: {last} after {attempt} {unit} (the engine was stopping)"));
                }
            }
            let outcome = self.http.post(url, headers, body);
            if matches!(&outcome, Ok(response) if (200..300).contains(&response.status)) {
                return None;
            }
            last = outcome_text(&outcome);
        }
        Some(format!("{name}: {last} after {} attempts", ATTEMPT_WAITS_S.len()))
    }
}

impl NotifySink for WebhookSink {
    fn deliver(&mut self, notification: &Notification) -> Result<(), String> {
        let body = serde_json::to_vec(notification).map_err(|error| format!("the notification is not JSON: {error}"))?;
        let agent = user_agent();
        let headers = [("Content-Type", "application/json"), ("User-Agent", agent.as_str())];
        let mut reasons = Vec::new();
        for name in WEBHOOK_SECRETS {
            let Some(url) = self.secrets.get(name).filter(|url| !url.is_empty()) else {
                continue;
            };
            if let Some(reason) = self.try_channel(name, &url, &headers, &body) {
                reasons.push(reason);
            }
        }
        if reasons.is_empty() {
            Ok(())
        } else {
            Err(reasons.join("; "))
        }
    }

    fn stop_flag(&self) -> Option<Arc<AtomicBool>> {
        Some(Arc::clone(&self.stopping))
    }
}

/// One attempt's outcome in words (design part 8.6), never a URL: a non-2xx answer is `HTTP <n>`.
fn outcome_text(outcome: &Result<Response, TransportError>) -> String {
    match outcome {
        Ok(response) => format!("HTTP {}", response.status),
        Err(TransportError::Timeout) => format!("no answer within {TIMEOUT_S} s"),
        Err(TransportError::Unreachable(words)) => words.clone(),
        Err(TransportError::Unrecorded(_)) => "no recorded answer".to_string(),
    }
}

/// engine.log's line for a notification the sink refused (spec 12, design part 3.5):
/// `notification <event> for <job id> was not delivered: <reason>`, with `no job` for a notification of no job and
/// `<event>` from `NotifyEvent::as_str`. It holds neither the notification's message nor any other field.
/// Def: T3-21. Use: T3-21.
pub(crate) fn undelivered_line(notification: &Notification, reason: &str) -> String {
    let job = notification.job_id.as_deref().unwrap_or("no job");
    format!("notification {} for {job} was not delivered: {reason}", notification.event.as_str())
}

/// Spec 12's `message` for each event (design part 8.6), from the job's record when the event has a job:
/// - `disc_done`: "the disc phase is done and the disc was ejected";
/// - `already_ripped`: "the disc was already ripped as job <id> and was ejected";
/// - `filed`: "filed at <first title's filed path>", or "filed";
/// - `needs_review`: "held for review: <reasons, snake_case, joined by ", ">";
/// - `waiting`: "waiting: <reason>", plus ": <record.message>" when the record has one;
/// - `resumed`: "resumed";
/// - `failed`: the record's message, else "failed with class <class>", followed by "; try the other drive" for class
///   `MediaReadError` (spec 12);
/// - `eject_failed`: "the eject failed; remove the disc by hand".
///
/// Def: T22. Use: T22, T24, T26.
pub(crate) fn message(event: &NotifyEvent, record: Option<&JobRecord>) -> String {
    let state_message = record.and_then(|record| record.message.clone());
    match event {
        NotifyEvent::DiscDone => "the disc phase is done and the disc was ejected".to_string(),
        NotifyEvent::AlreadyRipped => match record {
            Some(record) => format!("the disc was already ripped as job {} and was ejected", record.id),
            None => "the disc was already ripped and was ejected".to_string(),
        },
        NotifyEvent::Filed => {
            match record.and_then(|record| record.titles.first()).and_then(|title| title.filed.as_ref()) {
                Some(filed) => format!("filed at {}", filed.path),
                None => "filed".to_string(),
            }
        }
        NotifyEvent::NeedsReview { reasons } => {
            let reasons: Vec<&str> = reasons.iter().map(|reason| reason.as_str()).collect();
            format!("held for review: {}", reasons.join(", "))
        }
        NotifyEvent::Waiting { reason } => match state_message {
            Some(message) => format!("waiting: {}: {message}", reason.as_str()),
            None => format!("waiting: {}", reason.as_str()),
        },
        NotifyEvent::Resumed => "resumed".to_string(),
        NotifyEvent::Failed { class } => {
            let message = state_message.unwrap_or_else(|| format!("failed with class {}", class.as_str()));
            match class {
                FailureClass::MediaReadError => format!("{message}{TRY_THE_OTHER_DRIVE}"),
                _ => message,
            }
        }
        NotifyEvent::EjectFailed => "the eject failed; remove the disc by hand".to_string(),
    }
}

/// The notifier thread (design part 4.4): it drains the channel in order into the sink, writes `undelivered_line` of
/// each notification the sink refuses through `store`, the one writer of engine.log (design part 3.5), and ends once
/// every sender is gone. Def: T22; T3-21 adds `store`. Use: T22, T3-21.
pub(crate) fn spawn_notifier(sink: Box<dyn NotifySink>, store: Arc<Store>) -> Sender<Notification> {
    let (sender, notifications) = channel::<Notification>();
    thread::spawn(move || {
        let mut sink = sink;
        for notification in notifications {
            if let Err(reason) = sink.deliver(&notification) {
                store.log(&undelivered_line(&notification, &reason));
            }
        }
    });
    sender
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::Ordering;
    use std::sync::mpsc::{channel, Sender};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use lazymkv_clients::{user_agent, HttpTransport, RecordedTransport, TransportError};
    use lazymkv_core::ipc::Notification;
    use lazymkv_core::job::{FailureClass, NotifyEvent};
    use lazymkv_core::testsupport::Scratch;
    use lazymkv_platform::fakes::MemorySecrets;
    use lazymkv_platform::SecretStore;

    use super::{spawn_notifier, undelivered_line, NotifySink, WebhookSink};
    use crate::store::Store;

    const HA: &str = "http://ha.local:8123/api/webhook/hook-sentinel-7f3a";
    const HOOK: &str = "http://hooks.local/hook-sentinel-1c4b";

    /// A sink over `http` and `secrets` whose pause records each wait instead of sleeping.
    fn sink(http: &Arc<RecordedTransport>, secrets: &Arc<MemorySecrets>) -> (WebhookSink, Arc<Mutex<Vec<Duration>>>) {
        let pauses = Arc::new(Mutex::new(Vec::new()));
        let recorder = Arc::clone(&pauses);
        let sink = WebhookSink::new(Arc::clone(http) as Arc<dyn HttpTransport>, Arc::clone(secrets) as _)
            .with_pause(Arc::new(move |wait| recorder.lock().unwrap().push(wait)));
        (sink, pauses)
    }

    fn notification(event: NotifyEvent, job: Option<&str>) -> Notification {
        Notification { event, job_id: job.map(str::to_string), fingerprint: None, title: None,
            drive: Some("tok0".to_string()), message: "m".to_string(), time: 7 }
    }

    /// Delivers the first notification and refuses every later one; its channel closes when the notifier drops it.
    struct FirstOnly {
        delivered: bool,
        _done: Sender<()>,
    }

    impl NotifySink for FirstOnly {
        fn deliver(&mut self, _: &Notification) -> Result<(), String> {
            if std::mem::replace(&mut self.delivered, true) {
                Err("refused by the test sink".to_string())
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn the_undelivered_line_names_the_event_and_the_job_or_no_job() {
        assert_eq!(undelivered_line(&notification(NotifyEvent::DiscDone, Some("abc-1")), "why"),
                   "notification disc_done for abc-1 was not delivered: why");
        let failed = notification(NotifyEvent::Failed { class: FailureClass::Unknown }, None);
        assert_eq!(undelivered_line(&failed, "why"), "notification failed for no job was not delivered: why");
    }

    /// Spec 12: each refused notification is one engine.log line, in order, and a delivered one writes none.
    #[test]
    fn the_notifier_logs_each_refused_notification_and_nothing_for_a_delivered_one() {
        let scratch = Scratch::new("notifier");
        let store = Arc::new(Store::new(scratch.0.clone(), Arc::new(|| 7), None));
        let (done, ended) = channel::<()>();
        let sender = spawn_notifier(Box::new(FirstOnly { delivered: false, _done: done }), Arc::clone(&store));
        sender.send(notification(NotifyEvent::DiscDone, Some("abc-1"))).unwrap();
        sender.send(notification(NotifyEvent::Filed, Some("abc-1"))).unwrap();
        sender.send(notification(NotifyEvent::Failed { class: FailureClass::Unknown }, None)).unwrap();
        drop(sender);
        let _ = ended.recv(); // Err once the notifier has dropped the sink, after its last line
        assert_eq!(fs::read_to_string(scratch.0.join("engine.log")).unwrap(),
                   "7 notification filed for abc-1 was not delivered: refused by the test sink\n\
                    7 notification failed for no job was not delivered: refused by the test sink\n");
    }

    #[test]
    fn a_notification_is_posted_as_json_to_each_set_channel_in_order() {
        let http = Arc::new(RecordedTransport::new());
        http.answer(HA, 200, b"");
        http.answer(HOOK, 200, b"");
        let secrets = Arc::new(MemorySecrets::new().with("ha_webhook_url", HA).with("webhook_url", HOOK));
        let (mut sink, pauses) = sink(&http, &secrets);
        let n = notification(NotifyEvent::Filed, Some("abc-1"));
        assert_eq!(sink.deliver(&n), Ok(()));
        let posts = http.posts();
        assert_eq!(posts.iter().map(|(url, _, _)| url.as_str()).collect::<Vec<_>>(), [HA, HOOK]);
        for (_, headers, body) in &posts {
            assert_eq!(body, &serde_json::to_vec(&n).unwrap());
            assert!(headers.contains(&("Content-Type".to_string(), "application/json".to_string())));
            assert!(headers.contains(&("User-Agent".to_string(), user_agent())));
        }
        assert!(pauses.lock().unwrap().is_empty());
    }

    #[test]
    fn a_failing_channel_is_tried_three_times_with_waits_of_1_and_4_seconds() {
        let http = Arc::new(RecordedTransport::new());
        http.answer(HA, 500, b"");
        let secrets = Arc::new(MemorySecrets::new().with("ha_webhook_url", HA));
        let (mut sink, pauses) = sink(&http, &secrets);
        assert_eq!(sink.deliver(&notification(NotifyEvent::DiscDone, Some("abc-1"))),
                   Err("ha_webhook_url: HTTP 500 after 3 attempts".to_string()));
        assert_eq!(http.posts().len(), 3);
        assert_eq!(*pauses.lock().unwrap(), [Duration::from_secs(1), Duration::from_secs(4)]);
    }

    #[test]
    fn an_undelivered_notification_names_its_channel_and_never_its_url() {
        let url = "http://user-sentinel-7c1e:pass-sentinel-4b9d@ha.local:8123/api/webhook/hook-sentinel-7f3a?x=1#f";
        for fault in [None, Some(TransportError::Timeout)] {
            let http = Arc::new(RecordedTransport::new());
            if let Some(error) = fault.clone() { http.fail(url, error); }
            let secrets = Arc::new(MemorySecrets::new().with("ha_webhook_url", url));
            let (mut sink, _) = sink(&http, &secrets);
            let n = notification(NotifyEvent::DiscDone, Some("abc-1"));
            let reason = sink.deliver(&n).unwrap_err();
            let expected = if fault.is_some() { "no answer within 10 s" } else { "no recorded answer" };
            assert_eq!(reason, format!("ha_webhook_url: {expected} after 3 attempts"));
            let line = undelivered_line(&n, &reason);
            for needle in ["hook-sentinel-7f3a", "pass-sentinel-4b9d", "user-sentinel-7c1e", "x=1", "/api/webhook"] {
                assert!(!line.contains(needle), "{line}");
            }
        }
    }

    #[test]
    fn a_stop_skips_the_waits_and_gives_each_queued_notification_one_attempt() {
        let http = Arc::new(RecordedTransport::new());
        http.answer(HA, 500, b"");
        let secrets = Arc::new(MemorySecrets::new().with("ha_webhook_url", HA));
        let (mut sink, pauses) = sink(&http, &secrets);
        sink.stop_flag().expect("the sink has a stop flag").store(true, Ordering::SeqCst);
        for job in ["abc-1", "abc-2"] {
            assert_eq!(sink.deliver(&notification(NotifyEvent::Filed, Some(job))),
                       Err("ha_webhook_url: HTTP 500 after 1 attempt (the engine was stopping)".to_string()));
        }
        assert_eq!(http.posts().len(), 2);
        assert!(pauses.lock().unwrap().is_empty());
    }

    #[test]
    fn any_2xx_delivers_and_a_204_is_not_retried() {
        for status in [204, 299] {
            let http = Arc::new(RecordedTransport::new());
            http.answer(HA, status, b"");
            let secrets = Arc::new(MemorySecrets::new().with("ha_webhook_url", HA));
            let (mut sink, _) = sink(&http, &secrets);
            assert_eq!(sink.deliver(&notification(NotifyEvent::Filed, Some("abc-1"))), Ok(()), "{status}");
            assert_eq!(http.posts().len(), 1, "{status}");
        }
        let http = Arc::new(RecordedTransport::new());
        http.answer(HA, 300, b"");
        let secrets = Arc::new(MemorySecrets::new().with("ha_webhook_url", HA));
        let (mut sink, _) = sink(&http, &secrets);
        assert_eq!(sink.deliver(&notification(NotifyEvent::Filed, Some("abc-1"))),
                   Err("ha_webhook_url: HTTP 300 after 3 attempts".to_string()));
    }

    #[test]
    fn a_channel_whose_url_is_unset_is_skipped_and_both_unset_log_nothing() {
        let http = Arc::new(RecordedTransport::new());
        let secrets = Arc::new(MemorySecrets::new());
        let (mut sink, _) = sink(&http, &secrets);
        assert_eq!(sink.deliver(&notification(NotifyEvent::Filed, Some("abc-1"))), Ok(()));
        assert!(http.posts().is_empty());
        let scratch = Scratch::new("notifier-unset");
        let store = Arc::new(Store::new(scratch.0.clone(), Arc::new(|| 7), None));
        let (done, ended) = channel::<()>();
        let sink = WebhookSink::new(Arc::clone(&http) as Arc<dyn HttpTransport>, Arc::clone(&secrets) as _)
            .with_pause(Arc::new(move |_| {
                let _held = &done; // the channel closes when the notifier drops the sink
            }));
        let sender = spawn_notifier(Box::new(sink), Arc::clone(&store));
        sender.send(notification(NotifyEvent::DiscDone, Some("abc-1"))).unwrap();
        sender.send(notification(NotifyEvent::Failed { class: FailureClass::Unknown }, None)).unwrap();
        drop(sender);
        let _ = ended.recv(); // Err once the notifier has dropped the sink
        let log = fs::read_to_string(scratch.0.join("engine.log")).unwrap_or_default();
        assert!(!log.contains("was not delivered"), "{log}");
        assert!(http.posts().is_empty());
    }

    #[test]
    fn one_failing_channel_does_not_stop_the_other() {
        let http = Arc::new(RecordedTransport::new());
        http.answer(HA, 200, b"");
        http.answer(HOOK, 500, b"");
        let secrets = Arc::new(MemorySecrets::new().with("ha_webhook_url", HA).with("webhook_url", HOOK));
        let (mut sink, _) = sink(&http, &secrets);
        assert_eq!(sink.deliver(&notification(NotifyEvent::Filed, Some("abc-1"))),
                   Err("webhook_url: HTTP 500 after 3 attempts".to_string()));
        assert_eq!(http.posts().iter().map(|(url, _, _)| url.as_str()).collect::<Vec<_>>(), [HA, HOOK, HOOK, HOOK]);
    }

    #[test]
    fn a_url_set_after_start_is_used_by_the_next_notification() {
        let http = Arc::new(RecordedTransport::new());
        http.answer(HA, 200, b"");
        let secrets = Arc::new(MemorySecrets::new());
        let (mut sink, _) = sink(&http, &secrets);
        assert_eq!(sink.deliver(&notification(NotifyEvent::DiscDone, Some("abc-1"))), Ok(()));
        assert!(http.posts().is_empty());
        secrets.set("ha_webhook_url", HA).unwrap();
        assert_eq!(sink.deliver(&notification(NotifyEvent::Filed, Some("abc-1"))), Ok(()));
        assert_eq!(http.posts().iter().map(|(url, _, _)| url.as_str()).collect::<Vec<_>>(), [HA]);
    }

    #[test]
    fn a_stop_during_the_retries_ends_them_after_the_attempts_made() {
        let http = Arc::new(RecordedTransport::new());
        http.answer(HA, 500, b"");
        let secrets = Arc::new(MemorySecrets::new().with("ha_webhook_url", HA));
        let sink = WebhookSink::new(Arc::clone(&http) as Arc<dyn HttpTransport>, Arc::clone(&secrets) as _);
        let flag = sink.stop_flag().expect("the sink has a stop flag");
        let mut sink = sink.with_pause(Arc::new(move |_| flag.store(true, Ordering::SeqCst)));
        assert_eq!(sink.deliver(&notification(NotifyEvent::DiscDone, Some("abc-1"))),
                   Err("ha_webhook_url: HTTP 500 after 1 attempt (the engine was stopping)".to_string()));
        assert_eq!(http.posts().len(), 1);
    }
}
