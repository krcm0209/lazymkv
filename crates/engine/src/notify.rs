//! Notifications (design part 8.6, spec 12): the sink they go to, the message each event carries, the notifier thread
//! that delivers them in order and engine.log's line for a notification the sink refuses (spec 12).

use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;
use std::thread;

use lazymkv_core::ipc::Notification;
use lazymkv_core::job::{FailureClass, NotifyEvent};
use lazymkv_core::record::JobRecord;

use crate::store::Store;

/// What a `failed` notification of class `MediaReadError` adds to its message (spec 12): the disc may read in the other
/// drive.
const TRY_THE_OTHER_DRIVE: &str = "; try the other drive";

/// Where notifications go (design part 8.6, S36; spec 12): pull request 3's binary passes `NoDelivery` (Ruling W26),
/// and pull request 4 adds webhook delivery. Def: T22; T3-21 makes `deliver` return a Result. Use: T22, T26, T29,
/// T3-21, T3-23.
pub trait NotifySink: Send {
    /// Delivers one notification, or gives why it could not. The notifier then writes `undelivered_line` to engine.log
    /// and goes on, so a notification never blocks a job (spec 12, design part 3.5). The reason never holds a secret,
    /// such as a webhook URL (spec 12).
    fn deliver(&mut self, notification: &Notification) -> Result<(), String>;
}

/// The reason `NoDelivery` gives for every notification (Ruling W26). Def: T3-21. Use: T3-21.
pub const NO_DELIVERY: &str = "no notification delivery in this build; it arrives in pull request 4";

/// The sink of pull request 3's engine binary (Ruling W26): it delivers nothing and refuses every notification with
/// `NO_DELIVERY`, so each one is logged undelivered in engine.log (spec 12, design part 3.5) until pull request 4's
/// webhooks. Def: T3-21. Use: T3-21, T3-23.
pub struct NoDelivery;

impl NotifySink for NoDelivery {
    fn deliver(&mut self, _notification: &Notification) -> Result<(), String> {
        Err(NO_DELIVERY.to_string())
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
    use std::sync::mpsc::{channel, Sender};
    use std::sync::Arc;

    use lazymkv_core::ipc::Notification;
    use lazymkv_core::job::{FailureClass, NotifyEvent};
    use lazymkv_core::testsupport::Scratch;

    use super::{spawn_notifier, undelivered_line, NoDelivery, NotifySink, NO_DELIVERY};
    use crate::store::Store;

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
    fn no_delivery_refuses_every_notification_with_its_sentence() {
        assert_eq!(NO_DELIVERY, "no notification delivery in this build; it arrives in pull request 4");
        let mut sink = NoDelivery;
        for event in [NotifyEvent::DiscDone, NotifyEvent::Filed, NotifyEvent::EjectFailed] {
            assert_eq!(sink.deliver(&notification(event, Some("abc-1"))), Err(NO_DELIVERY.to_string()));
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
}
