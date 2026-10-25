//! Notifications (design part 8.6, spec 12): the sink they go to, the message each event carries and the notifier
//! thread that delivers them in order.

use std::sync::mpsc::{channel, Sender};
use std::thread;

use lazymkv_core::ipc::Notification;
use lazymkv_core::job::{FailureClass, NotifyEvent};
use lazymkv_core::record::JobRecord;

/// What a `failed` notification of class `MediaReadError` adds to its message (spec 12): the disc may read in the other
/// drive.
const TRY_THE_OTHER_DRIVE: &str = "; try the other drive";

/// Where notifications go (design part 8.6, S36): PR 2's sink; PR 4 adds webhook delivery. Def: T22. Use: T22, T26,
/// T29.
pub trait NotifySink: Send {
    /// Delivers one notification. A sink that cannot deliver logs and drops it; it never blocks a job (spec 12).
    fn deliver(&mut self, notification: &Notification);
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

/// The notifier thread (design part 4.4): it drains the channel in order into the sink, and ends once every sender is
/// gone. Def: T22. Use: T22.
pub(crate) fn spawn_notifier(sink: Box<dyn NotifySink>) -> Sender<Notification> {
    let (sender, notifications) = channel::<Notification>();
    thread::spawn(move || {
        let mut sink = sink;
        for notification in notifications {
            sink.deliver(&notification);
        }
    });
    sender
}
