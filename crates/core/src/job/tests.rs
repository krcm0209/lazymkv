//! Tests of core::job (design spec 5, 6.1 step 2, 10.1, 10.2, 12; RP12): the closed sets and their spellings, the
//! failure outcomes, the resume rules, one test per row of the reducer table (Shared interfaces, `core::job`), the job
//! ID and the job lookup. Every input is built here; nothing is read from a file.

use crate::lookup::tmdb::{TmdbIdentity, TmdbKind};
use crate::remembered::{AudioOverride, AudioTrackOverride, Collision, CollisionAnswer, TitleOverrideEntry, TitleRole};

use super::*;

/// A boxed copy of `state`, for the `return_to` and `from` fields.
fn boxed(state: State) -> Box<State> {
    Box::new(state)
}

/// A failure of `class` with no Output cause.
fn failure(class: FailureClass) -> Failure {
    Failure { class, output: None }
}

/// An Output failure with its cause.
fn output(cause: OutputCause) -> Failure {
    Failure { class: FailureClass::Output, output: Some(cause) }
}

/// The seven disc-phase states, with both Planned modes and both RipVerified values.
fn disc_phase_states() -> Vec<State> {
    vec![
        State::Detected,
        State::Fingerprinted,
        State::Scanned,
        State::Planned { mode: Mode::Direct },
        State::Planned { mode: Mode::Copy },
        State::Ripping,
        State::Copying,
        State::RipVerified { after_copy: false },
        State::RipVerified { after_copy: true },
    ]
}

/// The six file-phase states, with both Ejected values and both Decided values.
fn file_phase_states() -> Vec<State> {
    vec![
        State::Ejected { after_copy: false },
        State::Ejected { after_copy: true },
        State::Extracting,
        State::Inspecting,
        State::Decided { approved: None },
        State::Decided { approved: Some(vec![ReviewReason::IdentityAmbiguous]) },
        State::Remuxing,
        State::Verified,
    ]
}

/// The six waiting states, each returning to `back`.
fn waiting_states(back: State) -> Vec<State> {
    vec![
        State::AwaitingShare { return_to: boxed(back.clone()) },
        State::AwaitingSpace { return_to: boxed(back.clone()) },
        State::AwaitingMakeMkv { return_to: boxed(back.clone()) },
        State::AwaitingNetwork { return_to: boxed(back.clone()) },
        State::AwaitingLookup { return_to: boxed(back.clone()) },
        State::AwaitingTool { return_to: boxed(back) },
    ]
}

/// The states that are neither disc phase, file phase nor waiting: both holds, Filed, Discarded and Failed of every
/// class.
fn resting_states() -> Vec<State> {
    let mut states = vec![
        State::NeedsReview { held: Held::AfterCopy },
        State::NeedsReview { held: Held::DispositionGate },
        State::Filed,
        State::Discarded,
    ];
    for class in FailureClass::ALL {
        states.push(State::Failed { class, from: boxed(State::Ripping) });
    }
    states
}

/// One state of every variant: every disc-phase and file-phase value, every waiting state returning to Ripping and to
/// Inspecting, both holds, Filed, Discarded and Failed of every class (42 states).
fn sample_states() -> Vec<State> {
    let mut states = disc_phase_states();
    states.extend(file_phase_states());
    states.extend(waiting_states(State::Ripping));
    states.extend(waiting_states(State::Inspecting));
    states.extend(resting_states());
    states
}

/// One event of every variant, with every value of each boolean, mode, disposition and tool field, the failure events
/// with a MediaReadError failure (CopyFailed also with Unknown), DecisionDone also with a path collision and with no
/// final path, and ReviewApproved also with entries in the lists of `Overrides` (74 events).
fn sample_events() -> Vec<Event> {
    let read = failure(FailureClass::MediaReadError);
    let mut events = vec![
        Event::MediaArrived,
        Event::MediaRemoved,
        Event::FingerprintDone,
        Event::FingerprintFailed { failure: read },
        Event::ScanDone,
        Event::ScanFailed { failure: read },
        Event::PlanDone { mode: Mode::Direct },
        Event::PlanDone { mode: Mode::Copy },
        Event::PlanFailed { failure: read },
        Event::SpaceOk,
        Event::SpaceShort,
        Event::SpaceFreed,
        Event::TitleRipped,
        Event::TitleRipFailed { failure: read },
    ];
    for last in [false, true] {
        events.push(Event::TitleVerified { last });
        for second in [false, true] {
            events.push(Event::TitleVerifyFailed { second, last });
        }
    }
    for scan_listed_titles in [false, true] {
        for hash_errors in [false, true] {
            events.push(Event::CopyDone { scan_listed_titles, hash_errors });
        }
    }
    for second in [false, true] {
        events.push(Event::CopyFailed { failure: read, second });
        events.push(Event::CopyFailed { failure: failure(FailureClass::Unknown), second });
    }
    events.extend([
        Event::TitleExtracted,
        Event::ExtractFailed { failure: read },
        Event::Ejected,
        Event::EjectFailed,
        Event::HoldForReview { reasons: vec![ReviewReason::TitleAmbiguous] },
        Event::FilePhaseSlot,
        Event::ProbeDone,
        Event::ProbeFailed { failure: read },
        Event::DecisionDone { disposition: Disposition::AutoFile, reasons: Vec::new(), has_path: true },
        Event::DecisionDone { disposition: Disposition::NeedsReview, reasons: vec![ReviewReason::SubtitleAmbiguous], has_path: true },
        Event::DecisionDone { disposition: Disposition::NeedsReview, reasons: vec![ReviewReason::PathCollision], has_path: true },
        Event::DecisionDone { disposition: Disposition::NeedsReview, reasons: Vec::new(), has_path: false },
        Event::DecisionFailed { failure: read },
        Event::RemuxDone,
        Event::RemuxFailed { failure: read },
        Event::FileVerified,
        Event::FileVerifyFailed { failure: read },
        Event::FileDone,
        Event::FileFailed { failure: read },
        Event::ShareLost,
        Event::ShareBack,
        Event::LicenceMessage,
        Event::VersionMessage,
        Event::MakeMkvChanged,
        Event::NetworkNeeded,
        Event::NetworkRetry,
        Event::LookupPending,
        Event::LookupAnswered,
    ]);
    for tool in Tool::ALL {
        events.push(Event::ToolMissing { tool });
        events.push(Event::ToolFound { tool });
    }
    events.extend([
        Event::ReviewApproved { overrides: Overrides::default(), approved_reasons: Vec::new() },
        Event::ReviewApproved {
            overrides: Overrides {
                titles: vec![TitleOverrideEntry { title: 1, role: TitleRole::Feature }],
                audio: vec![AudioTrackOverride { title: 1, stream: 2, action: AudioOverride::Drop }],
                ..Overrides::default()
            },
            approved_reasons: vec![ReviewReason::TitleAmbiguous],
        },
        Event::ReviewDiscarded,
        Event::Retry,
        Event::RipAgain,
        Event::ReFinalise { approved_reasons: Vec::new() },
        Event::EngineRestart { disc_present: false },
        Event::EngineRestart { disc_present: true },
    ]);
    events
}

/// `value` as compact JSON.
fn json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap()
}

#[test]
fn the_retry_waits_are_10_minutes_for_the_network_and_hourly_for_a_lookup() {
    assert_eq!(NETWORK_RETRY_MIN, 10);
    assert_eq!(LOOKUP_RETRY_MIN, 60);
}

#[test]
fn the_failure_classes_are_spelt_as_in_spec_10_2_in_its_order() {
    let spelt: Vec<&str> = FailureClass::ALL.into_iter().map(FailureClass::as_str).collect();
    assert_eq!(
        spelt,
        [
            "DriveIncapable",
            "MediaReadError",
            "Licence",
            "NeedsMakeMkvUpdate",
            "NeedsNetwork",
            "Output",
            "LookupUnavailable",
            "ToolMissing",
            "Unknown"
        ]
    );
    for class in FailureClass::ALL {
        assert_eq!(json(&class), format!("\"{}\"", class.as_str()));
        assert_eq!(serde_json::from_str::<FailureClass>(&json(&class)).unwrap(), class);
        assert_eq!(class.as_str().parse::<FailureClass>(), Ok(class));
    }
}

#[test]
fn a_failure_class_outside_spec_10_2_is_refused() {
    assert_eq!(
        "mediareaderror".parse::<FailureClass>(),
        Err("\"mediareaderror\" is not a failure class (spec 10.2)".to_string())
    );
    assert!(serde_json::from_str::<FailureClass>("\"Timeout\"").is_err());
}

#[test]
fn the_output_causes_are_share_unreachable_out_of_space_and_write_error() {
    let spelt: Vec<&str> = OutputCause::ALL.into_iter().map(OutputCause::as_str).collect();
    assert_eq!(spelt, ["share_unreachable", "out_of_space", "write_error"]);
    for cause in OutputCause::ALL {
        assert_eq!(json(&cause), format!("\"{}\"", cause.as_str()));
        assert_eq!(cause.as_str().parse::<OutputCause>(), Ok(cause));
    }
    assert_eq!(
        "full".parse::<OutputCause>(),
        Err("\"full\" is not an output failure cause (spec 10.2)".to_string())
    );
}

#[test]
fn a_failure_is_written_with_its_class_and_its_output_cause_or_null() {
    assert_eq!(json(&output(OutputCause::OutOfSpace)), r#"{"class":"Output","output":"out_of_space"}"#);
    assert_eq!(json(&failure(FailureClass::MediaReadError)), r#"{"class":"MediaReadError","output":null}"#);
    let back: Failure = serde_json::from_str(r#"{"class":"Output","output":"write_error"}"#).unwrap();
    assert_eq!(back, output(OutputCause::WriteError));
}

#[test]
fn the_tools_are_spelt_as_their_program_names() {
    let spelt: Vec<&str> = Tool::ALL.into_iter().map(Tool::as_str).collect();
    assert_eq!(spelt, ["makemkvcon", "mkvmerge", "ffprobe", "ffmpeg", "java"]);
    for tool in Tool::ALL {
        assert_eq!(json(&tool), format!("\"{}\"", tool.as_str()));
        assert_eq!(tool.as_str().parse::<Tool>(), Ok(tool));
    }
    assert_eq!("makemkvcon64".parse::<Tool>(), Err("\"makemkvcon64\" is not a tool (spec 4.4, 10.2)".to_string()));
}

#[test]
fn the_two_holds_are_after_copy_and_disposition_gate() {
    let spelt: Vec<&str> = Held::ALL.into_iter().map(Held::as_str).collect();
    assert_eq!(spelt, ["after_copy", "disposition_gate"]);
    for held in Held::ALL {
        assert_eq!(json(&held), format!("\"{}\"", held.as_str()));
        assert_eq!(held.as_str().parse::<Held>(), Ok(held));
    }
    assert_eq!("review".parse::<Held>(), Err("\"review\" is not a hold (spec 10.1)".to_string()));
}

#[test]
fn the_waiting_reasons_are_spec_12s_six_in_its_order() {
    let spelt: Vec<&str> = WaitReason::ALL.into_iter().map(WaitReason::as_str).collect();
    assert_eq!(spelt, ["share", "space", "makemkv", "network", "lookup", "tool"]);
    for reason in WaitReason::ALL {
        assert_eq!(json(&reason), format!("\"{}\"", reason.as_str()));
        assert_eq!(reason.as_str().parse::<WaitReason>(), Ok(reason));
    }
    assert_eq!("disc".parse::<WaitReason>(), Err("\"disc\" is not a waiting reason (spec 12)".to_string()));
}

#[test]
fn the_notification_events_are_spec_12s_eight_with_their_fields() {
    let cases = [
        (NotifyEvent::DiscDone, "disc_done", r#"{"event":"disc_done"}"#),
        (NotifyEvent::AlreadyRipped, "already_ripped", r#"{"event":"already_ripped"}"#),
        (NotifyEvent::Filed, "filed", r#"{"event":"filed"}"#),
        (
            NotifyEvent::NeedsReview { reasons: vec![ReviewReason::CopyHashErrors, ReviewReason::TitleAmbiguous] },
            "needs_review",
            r#"{"event":"needs_review","reasons":["copy_hash_errors","title_ambiguous"]}"#,
        ),
        (NotifyEvent::Waiting { reason: WaitReason::Network }, "waiting", r#"{"event":"waiting","reason":"network"}"#),
        (NotifyEvent::Resumed, "resumed", r#"{"event":"resumed"}"#),
        (
            NotifyEvent::Failed { class: FailureClass::MediaReadError },
            "failed",
            r#"{"event":"failed","class":"MediaReadError"}"#,
        ),
        (NotifyEvent::EjectFailed, "eject_failed", r#"{"event":"eject_failed"}"#),
    ];
    for (event, name, written) in cases {
        assert_eq!(event.as_str(), name);
        assert_eq!(json(&event), written);
        assert_eq!(serde_json::from_str::<NotifyEvent>(written).unwrap(), event);
    }
}

#[test]
fn every_state_is_named_as_in_spec_10_1() {
    let names: Vec<&str> = sample_states().iter().map(State::as_str).collect();
    assert_eq!(
        names,
        [
            "Detected",
            "Fingerprinted",
            "Scanned",
            "Planned",
            "Planned",
            "Ripping",
            "Copying",
            "RipVerified",
            "RipVerified",
            "Ejected",
            "Ejected",
            "Extracting",
            "Inspecting",
            "Decided",
            "Decided",
            "Remuxing",
            "Verified",
            "AwaitingShare",
            "AwaitingSpace",
            "AwaitingMakeMkv",
            "AwaitingNetwork",
            "AwaitingLookup",
            "AwaitingTool",
            "AwaitingShare",
            "AwaitingSpace",
            "AwaitingMakeMkv",
            "AwaitingNetwork",
            "AwaitingLookup",
            "AwaitingTool",
            "NeedsReview",
            "NeedsReview",
            "Filed",
            "Discarded",
            "Failed",
            "Failed",
            "Failed",
            "Failed",
            "Failed",
            "Failed",
            "Failed",
            "Failed",
            "Failed"
        ]
    );
}

#[test]
fn the_disc_phase_states_are_detected_to_rip_verified() {
    for state in sample_states() {
        let expected = disc_phase_states().contains(&state);
        assert_eq!(state.is_disc_phase(), expected, "{state:?}");
    }
}

#[test]
fn the_file_phase_states_are_ejected_extracting_inspecting_decided_remuxing_and_verified() {
    for state in sample_states() {
        let expected = file_phase_states().contains(&state);
        assert_eq!(state.is_file_phase(), expected, "{state:?}");
    }
}

#[test]
fn the_waiting_states_are_the_six_awaiting_states() {
    for state in sample_states() {
        let expected = state.as_str().starts_with("Awaiting");
        assert_eq!(state.is_waiting(), expected, "{state:?}");
    }
}

#[test]
fn held_filed_discarded_and_failed_states_are_in_no_phase_and_not_waiting() {
    for state in resting_states() {
        assert!(!state.is_disc_phase() && !state.is_file_phase() && !state.is_waiting(), "{state:?}");
    }
}

#[test]
fn a_state_is_written_with_its_name_under_state_and_its_fields_beside_it() {
    assert_eq!(json(&State::Ripping), r#"{"state":"Ripping"}"#);
    assert_eq!(json(&State::RipVerified { after_copy: false }), r#"{"state":"RipVerified","after_copy":false}"#);
    assert_eq!(json(&State::Planned { mode: Mode::Copy }), r#"{"state":"Planned","mode":"copy"}"#);
    assert_eq!(json(&State::Ejected { after_copy: true }), r#"{"state":"Ejected","after_copy":true}"#);
    assert_eq!(
        json(&State::Decided { approved: Some(vec![ReviewReason::IdentityAmbiguous]) }),
        r#"{"state":"Decided","approved":["identity_ambiguous"]}"#
    );
    assert_eq!(json(&State::Decided { approved: None }), r#"{"state":"Decided","approved":null}"#);
    assert_eq!(
        json(&State::NeedsReview { held: Held::DispositionGate }),
        r#"{"state":"NeedsReview","held":"disposition_gate"}"#
    );
}

#[test]
fn a_waiting_state_stores_the_state_it_returns_to() {
    let waiting = State::AwaitingShare { return_to: boxed(State::Planned { mode: Mode::Direct }) };
    assert_eq!(json(&waiting), r#"{"state":"AwaitingShare","return_to":{"state":"Planned","mode":"direct"}}"#);
}

#[test]
fn a_failed_state_carries_its_class_and_the_state_that_failed() {
    let failed = State::Failed { class: FailureClass::MediaReadError, from: boxed(State::Copying) };
    assert_eq!(json(&failed), r#"{"state":"Failed","class":"MediaReadError","from":{"state":"Copying"}}"#);
}

#[test]
fn every_sample_state_round_trips_through_json() {
    for state in sample_states() {
        assert_eq!(serde_json::from_str::<State>(&json(&state)).unwrap(), state);
    }
}

#[test]
fn every_sample_event_round_trips_through_json() {
    for event in sample_events() {
        assert_eq!(serde_json::from_str::<Event>(&json(&event)).unwrap(), event, "{}", json(&event));
    }
}

#[test]
fn an_event_is_written_with_its_name_under_event_and_its_fields_beside_it() {
    assert_eq!(json(&Event::ShareLost), r#"{"event":"ShareLost"}"#);
    assert_eq!(
        json(&Event::TitleVerifyFailed { second: true, last: false }),
        r#"{"event":"TitleVerifyFailed","second":true,"last":false}"#
    );
    assert_eq!(json(&Event::ToolMissing { tool: Tool::Ffprobe }), r#"{"event":"ToolMissing","tool":"ffprobe"}"#);
    let decided = Event::DecisionDone {
        disposition: Disposition::NeedsReview,
        reasons: vec![ReviewReason::SubtitleAmbiguous],
        has_path: true,
    };
    assert_eq!(
        json(&decided),
        r#"{"event":"DecisionDone","disposition":"needs_review","reasons":["subtitle_ambiguous"],"has_path":true}"#
    );
    assert_eq!(
        json(&Event::FingerprintFailed { failure: output(OutputCause::OutOfSpace) }),
        r#"{"event":"FingerprintFailed","failure":{"class":"Output","output":"out_of_space"}}"#
    );
}

#[test]
fn an_action_is_written_with_its_name_under_action_and_its_fields_beside_it() {
    assert_eq!(json(&Action::RipTitle), r#"{"action":"RipTitle"}"#);
    assert_eq!(json(&Action::StartTimer { minutes: NETWORK_RETRY_MIN }), r#"{"action":"StartTimer","minutes":10}"#);
    assert_eq!(
        json(&Action::RecordNote { note: Note::DirectModeFallback }),
        r#"{"action":"RecordNote","note":"direct_mode_fallback"}"#
    );
    assert_eq!(
        json(&Action::Notify { event: NotifyEvent::Waiting { reason: WaitReason::Lookup } }),
        r#"{"action":"Notify","event":{"event":"waiting","reason":"lookup"}}"#
    );
    let back: Action = serde_json::from_str(r#"{"action":"Notify","event":{"event":"disc_done"}}"#).unwrap();
    assert_eq!(back, Action::Notify { event: NotifyEvent::DiscDone });
}

// Spec 10.2's Result column (`failure_outcome`) and its last paragraph (`classify_title_failure`).

#[test]
fn a_drive_incapable_failure_fails_the_job() {
    assert_eq!(
        failure_outcome(failure(FailureClass::DriveIncapable), &State::Scanned),
        State::Failed { class: FailureClass::DriveIncapable, from: boxed(State::Scanned) }
    );
}

#[test]
fn a_media_read_error_fails_the_job_and_keeps_the_state_that_failed() {
    assert_eq!(
        failure_outcome(failure(FailureClass::MediaReadError), &State::Ripping),
        State::Failed { class: FailureClass::MediaReadError, from: boxed(State::Ripping) }
    );
}

#[test]
fn an_unknown_failure_fails_the_job() {
    assert_eq!(
        failure_outcome(failure(FailureClass::Unknown), &State::Remuxing),
        State::Failed { class: FailureClass::Unknown, from: boxed(State::Remuxing) }
    );
}

#[test]
fn a_licence_failure_waits_for_makemkv() {
    assert_eq!(
        failure_outcome(failure(FailureClass::Licence), &State::Ripping),
        State::AwaitingMakeMkv { return_to: boxed(State::Ripping) }
    );
}

#[test]
fn a_failure_needing_a_newer_makemkv_waits_for_makemkv() {
    assert_eq!(
        failure_outcome(failure(FailureClass::NeedsMakeMkvUpdate), &State::Fingerprinted),
        State::AwaitingMakeMkv { return_to: boxed(State::Fingerprinted) }
    );
}

#[test]
fn a_failure_needing_the_network_waits_for_the_network() {
    assert_eq!(
        failure_outcome(failure(FailureClass::NeedsNetwork), &State::Copying),
        State::AwaitingNetwork { return_to: boxed(State::Copying) }
    );
}

#[test]
fn an_output_failure_out_of_space_waits_for_space() {
    assert_eq!(
        failure_outcome(output(OutputCause::OutOfSpace), &State::Ripping),
        State::AwaitingSpace { return_to: boxed(State::Ripping) }
    );
}

#[test]
fn an_output_failure_with_the_share_unreachable_waits_for_the_share() {
    assert_eq!(
        failure_outcome(output(OutputCause::ShareUnreachable), &State::Ripping),
        State::AwaitingShare { return_to: boxed(State::Ripping) }
    );
}

#[test]
fn an_output_write_error_is_treated_as_a_share_fault() {
    assert_eq!(
        failure_outcome(output(OutputCause::WriteError), &State::Remuxing),
        State::AwaitingShare { return_to: boxed(State::Remuxing) }
    );
}

#[test]
fn an_output_failure_without_a_cause_waits_for_the_share() {
    assert_eq!(
        failure_outcome(failure(FailureClass::Output), &State::Extracting),
        State::AwaitingShare { return_to: boxed(State::Extracting) }
    );
}

#[test]
fn a_lookup_unavailable_failure_waits_for_the_lookup() {
    assert_eq!(
        failure_outcome(failure(FailureClass::LookupUnavailable), &State::Inspecting),
        State::AwaitingLookup { return_to: boxed(State::Inspecting) }
    );
}

#[test]
fn a_tool_missing_failure_waits_for_the_tool() {
    assert_eq!(
        failure_outcome(failure(FailureClass::ToolMissing), &State::Detected),
        State::AwaitingTool { return_to: boxed(State::Detected) }
    );
}

#[test]
fn every_outcome_keeps_the_state_that_failed_whatever_the_class() {
    let from = State::Planned { mode: Mode::Copy };
    for class in FailureClass::ALL {
        let kept = match failure_outcome(failure(class), &from) {
            State::Failed { from: kept, .. } => *kept,
            State::AwaitingShare { return_to }
            | State::AwaitingSpace { return_to }
            | State::AwaitingMakeMkv { return_to }
            | State::AwaitingNetwork { return_to }
            | State::AwaitingLookup { return_to }
            | State::AwaitingTool { return_to } => *return_to,
            other => panic!("{class:?} gave {other:?}"),
        };
        assert_eq!(kept, from, "{class:?}");
    }
}

#[test]
fn a_title_failure_with_the_share_unreachable_is_an_output_failure_whatever_else_was_seen() {
    for (out_of_space, write_error, read_errors) in [(false, false, false), (true, true, true), (false, false, true)] {
        assert_eq!(
            classify_title_failure(false, out_of_space, write_error, read_errors),
            output(OutputCause::ShareUnreachable)
        );
    }
}

#[test]
fn a_title_failure_out_of_space_with_the_share_healthy_is_an_output_failure_before_a_write_or_read_error() {
    assert_eq!(classify_title_failure(true, true, false, false), output(OutputCause::OutOfSpace));
    assert_eq!(classify_title_failure(true, true, true, true), output(OutputCause::OutOfSpace));
}

#[test]
fn a_title_failure_with_a_write_error_is_an_output_failure_before_a_read_error() {
    assert_eq!(classify_title_failure(true, false, true, false), output(OutputCause::WriteError));
    assert_eq!(classify_title_failure(true, false, true, true), output(OutputCause::WriteError));
}

#[test]
fn a_title_failure_with_read_errors_and_the_share_healthy_is_a_media_read_error() {
    assert_eq!(classify_title_failure(true, false, false, true), failure(FailureClass::MediaReadError));
}

#[test]
fn a_title_failure_with_nothing_seen_is_unknown() {
    assert_eq!(classify_title_failure(true, false, false, false), failure(FailureClass::Unknown));
}

// `new_job` and spec 10.1's "Resuming after a restart" (`resume`).

/// The actions `resume` gives `state` with the disc present.
fn resumed(state: State) -> Vec<Action> {
    let (kept, actions) = resume(&state, true);
    assert_eq!(kept, state, "resume changed the state");
    actions
}

#[test]
fn a_new_job_starts_detected_and_fingerprints_the_disc() {
    assert_eq!(new_job(), (State::Detected, vec![Action::Fingerprint]));
}

#[test]
fn resuming_detected_fingerprints_again() {
    assert_eq!(resumed(State::Detected), [Action::Fingerprint]);
}

#[test]
fn resuming_fingerprinted_runs_the_scan_again() {
    assert_eq!(resumed(State::Fingerprinted), [Action::RunScan]);
}

#[test]
fn resuming_scanned_plans_again() {
    assert_eq!(resumed(State::Scanned), [Action::Plan]);
}

#[test]
fn resuming_planned_checks_space_in_either_mode() {
    assert_eq!(resumed(State::Planned { mode: Mode::Direct }), [Action::CheckSpace]);
    assert_eq!(resumed(State::Planned { mode: Mode::Copy }), [Action::CheckSpace]);
}

#[test]
fn resuming_decided_decides_again() {
    assert_eq!(resumed(State::Decided { approved: None }), [Action::Decide]);
    assert_eq!(resumed(State::Decided { approved: Some(Vec::new()) }), [Action::Decide]);
}

#[test]
fn resuming_ripping_empties_the_title_directory_and_rips_again() {
    assert_eq!(resumed(State::Ripping), [Action::EmptyTitleDirectory, Action::RipTitle]);
}

#[test]
fn resuming_extracting_empties_the_title_directory_and_extracts_again() {
    assert_eq!(resumed(State::Extracting), [Action::EmptyTitleDirectory, Action::ExtractTitle]);
}

#[test]
fn resuming_copying_empties_the_copy_directory_and_copies_again() {
    assert_eq!(resumed(State::Copying), [Action::EmptyCopyDirectory, Action::CopyDisc]);
}

#[test]
fn resuming_rip_verified_ejects_after_a_rip_or_a_copy() {
    assert_eq!(resumed(State::RipVerified { after_copy: false }), [Action::Eject]);
    assert_eq!(resumed(State::RipVerified { after_copy: true }), [Action::Eject]);
}

#[test]
fn resuming_inspecting_probes_again() {
    assert_eq!(resumed(State::Inspecting), [Action::Probe]);
}

#[test]
fn resuming_remuxing_deletes_the_partial_file_and_remuxes_again() {
    assert_eq!(resumed(State::Remuxing), [Action::DeletePartialFile, Action::Remux]);
}

#[test]
fn resuming_verified_files_again() {
    assert_eq!(resumed(State::Verified), [Action::File]);
}

#[test]
fn resuming_ejected_does_nothing_until_the_file_phase_slot_or_the_hold() {
    assert_eq!(resumed(State::Ejected { after_copy: false }), Vec::<Action>::new());
    assert_eq!(resumed(State::Ejected { after_copy: true }), Vec::<Action>::new());
}

#[test]
fn a_disc_phase_state_whose_disc_is_not_in_the_drive_waits_for_the_disc() {
    for state in disc_phase_states() {
        assert_eq!(resume(&state, false), (state.clone(), vec![Action::WaitForDisc]), "{state:?}");
    }
}

#[test]
fn a_file_phase_state_resumes_the_same_whether_or_not_the_disc_is_in() {
    for state in file_phase_states() {
        assert_eq!(resume(&state, false), resume(&state, true), "{state:?}");
    }
}

#[test]
fn resuming_a_held_filed_discarded_or_failed_job_does_nothing() {
    for state in resting_states() {
        assert_eq!(resume(&state, true), (state.clone(), Vec::new()), "{state:?}");
        assert_eq!(resume(&state, false), (state.clone(), Vec::new()), "{state:?}");
    }
}

#[test]
fn resuming_a_wait_for_the_share_space_makemkv_or_a_tool_does_nothing() {
    for state in waiting_states(State::Ripping) {
        if matches!(state, State::AwaitingNetwork { .. } | State::AwaitingLookup { .. }) {
            continue;
        }
        assert_eq!(resume(&state, true), (state.clone(), Vec::new()), "{state:?}");
        assert_eq!(resume(&state, false), (state.clone(), Vec::new()), "{state:?}");
    }
}

#[test]
fn resuming_a_wait_for_the_network_or_a_lookup_starts_its_retry_timer_again() {
    let network = State::AwaitingNetwork { return_to: boxed(State::Scanned) };
    assert_eq!(resume(&network, false), (network.clone(), vec![Action::StartTimer { minutes: 10 }]));
    let lookup = State::AwaitingLookup { return_to: boxed(State::Inspecting) };
    assert_eq!(resume(&lookup, true), (lookup.clone(), vec![Action::StartTimer { minutes: 60 }]));
}

#[test]
fn resume_never_changes_the_state() {
    for state in sample_states() {
        for disc_present in [false, true] {
            assert_eq!(resume(&state, disc_present).0, state);
        }
    }
}

// `reduce`: the rows for any state, the Direct-mode main line, rejection and the disjointness of the rules.

/// reduce's answer for a pair the table holds; panics with "rejected: <state> + <event>" for one it does not.
fn step(state: &State, event: &Event) -> (State, Vec<Action>) {
    match reduce(state, event) {
        Ok(next) => next,
        Err(rejected) => panic!("rejected: {:?} + {:?}", rejected.state, rejected.event),
    }
}

/// reduce over `events` from `start`, every step's actions in order.
fn walk(start: State, events: &[Event]) -> (State, Vec<Vec<Action>>) {
    let mut state = start;
    let mut actions = Vec::new();
    for event in events {
        let (next, done) = step(&state, event);
        state = next;
        actions.push(done);
    }
    (state, actions)
}

/// True when reduce refuses the pair, with the pair in the refusal.
fn rejects(state: &State, event: &Event) -> bool {
    reduce(state, event) == Err(Rejected { state: state.clone(), event: event.clone() })
}

#[test]
fn no_state_and_event_pair_is_held_by_two_rules() {
    for state in sample_states() {
        for event in sample_events() {
            let holding = RULES.iter().filter(|rule| rule(&state, &event).is_some()).count();
            assert!(holding <= 1, "{state:?} + {event:?} is held by {holding} rules");
        }
    }
}

#[test]
fn an_event_the_state_does_not_take_is_rejected_with_the_state_and_the_event() {
    assert_eq!(
        reduce(&State::Detected, &Event::ScanDone),
        Err(Rejected { state: State::Detected, event: Event::ScanDone })
    );
    assert!(rejects(&State::Ripping, &Event::FileDone));
    assert!(rejects(&State::Filed, &Event::SpaceOk));
}

#[test]
fn a_discarded_job_takes_only_an_engine_restart_and_the_java_note() {
    for event in sample_events() {
        let any_state_row = matches!(event, Event::EngineRestart { .. } | Event::ToolMissing { tool: Tool::Java });
        assert_eq!(rejects(&State::Discarded, &event), !any_state_row, "{event:?}");
    }
}

#[test]
fn an_engine_restart_resumes_any_state_with_the_disc_present_or_not() {
    for state in sample_states() {
        for disc_present in [false, true] {
            assert_eq!(step(&state, &Event::EngineRestart { disc_present }), resume(&state, disc_present), "{state:?}");
        }
    }
}

#[test]
fn a_missing_java_leaves_any_state_and_records_that_the_label_rule_is_unavailable() {
    for state in sample_states() {
        assert_eq!(
            step(&state, &Event::ToolMissing { tool: Tool::Java }),
            (state.clone(), vec![Action::RecordNote { note: Note::LabelRuleUnavailable }])
        );
    }
}

#[test]
fn detected_takes_the_fingerprint_to_fingerprinted_and_runs_the_scan() {
    assert_eq!(step(&State::Detected, &Event::FingerprintDone), (State::Fingerprinted, vec![Action::RunScan]));
}

#[test]
fn fingerprinted_takes_the_scan_to_scanned_and_plans() {
    assert_eq!(step(&State::Fingerprinted, &Event::ScanDone), (State::Scanned, vec![Action::Plan]));
}

#[test]
fn scanned_takes_the_plan_to_planned_with_its_mode_and_checks_space() {
    for mode in [Mode::Direct, Mode::Copy] {
        assert_eq!(
            step(&State::Scanned, &Event::PlanDone { mode }),
            (State::Planned { mode }, vec![Action::CheckSpace])
        );
    }
}

#[test]
fn a_direct_plan_with_space_rips_the_first_title() {
    assert_eq!(step(&State::Planned { mode: Mode::Direct }, &Event::SpaceOk), (State::Ripping, vec![Action::RipTitle]));
}

#[test]
fn a_ripped_title_is_verified() {
    assert_eq!(step(&State::Ripping, &Event::TitleRipped), (State::Ripping, vec![Action::VerifyFile]));
}

#[test]
fn a_verified_title_that_is_not_the_last_rips_the_next() {
    assert_eq!(step(&State::Ripping, &Event::TitleVerified { last: false }), (State::Ripping, vec![Action::RipTitle]));
}

#[test]
fn a_second_verification_failure_that_is_not_the_last_title_rips_the_next() {
    assert_eq!(
        step(&State::Ripping, &Event::TitleVerifyFailed { second: true, last: false }),
        (State::Ripping, vec![Action::RipTitle])
    );
}

#[test]
fn a_first_verification_failure_rips_the_same_title_again_even_for_the_last_title() {
    for last in [false, true] {
        assert_eq!(
            step(&State::Ripping, &Event::TitleVerifyFailed { second: false, last }),
            (State::Ripping, vec![Action::RipTitle])
        );
    }
}

#[test]
fn the_last_title_verified_ends_the_rip_and_ejects() {
    assert_eq!(
        step(&State::Ripping, &Event::TitleVerified { last: true }),
        (State::RipVerified { after_copy: false }, vec![Action::Eject])
    );
}

#[test]
fn the_last_title_failing_verification_a_second_time_ends_the_rip_and_ejects() {
    assert_eq!(
        step(&State::Ripping, &Event::TitleVerifyFailed { second: true, last: true }),
        (State::RipVerified { after_copy: false }, vec![Action::Eject])
    );
}

#[test]
fn a_verified_rip_ejected_sends_disc_done() {
    assert_eq!(
        step(&State::RipVerified { after_copy: false }, &Event::Ejected),
        (State::Ejected { after_copy: false }, vec![notify(NotifyEvent::DiscDone)])
    );
}

#[test]
fn a_verified_rip_whose_eject_failed_sends_eject_failed_and_leaves_the_disc_phase() {
    assert_eq!(
        step(&State::RipVerified { after_copy: false }, &Event::EjectFailed),
        (State::Ejected { after_copy: false }, vec![notify(NotifyEvent::EjectFailed)])
    );
}

#[test]
fn a_direct_job_ejected_takes_the_file_phase_slot_and_probes() {
    assert_eq!(
        step(&State::Ejected { after_copy: false }, &Event::FilePhaseSlot),
        (State::Inspecting, vec![Action::Probe])
    );
}

#[test]
fn a_direct_job_ejected_is_not_held_for_review() {
    assert!(rejects(&State::Ejected { after_copy: false }, &Event::HoldForReview { reasons: Vec::new() }));
}

#[test]
fn a_finished_probe_extracts_the_subtitle_evidence_then_decides() {
    assert_eq!(
        step(&State::Inspecting, &Event::ProbeDone),
        (State::Inspecting, vec![Action::ExtractSubtitleEvidence, Action::Decide])
    );
}

#[test]
fn an_auto_file_decision_without_a_final_path_is_held_at_the_gate_whether_first_or_made_again() {
    // Ruling T14: a job with no final path is never AutoFile. `decide` never pairs AutoFile with no path, but the event
    // is the job's, combined from its titles (`decision_done`), so the reducer holds it in both unapproved states too.
    let pathless = Event::DecisionDone { disposition: Disposition::AutoFile, reasons: Vec::new(), has_path: false };
    for state in [State::Inspecting, State::Decided { approved: None }] {
        assert_eq!(
            step(&state, &pathless),
            (
                State::NeedsReview { held: Held::DispositionGate },
                vec![notify(NotifyEvent::NeedsReview { reasons: Vec::new() })]
            ),
            "{state:?}"
        );
    }
    let reasons = vec![ReviewReason::TvUnmapped];
    let unmapped = Event::DecisionDone { disposition: Disposition::NeedsReview, reasons: reasons.clone(), has_path: false };
    assert_eq!(
        step(&State::Decided { approved: None }, &unmapped),
        (State::NeedsReview { held: Held::DispositionGate }, vec![notify(NotifyEvent::NeedsReview { reasons })])
    );
}

/// One title's decision of a job: keeping nothing, with this disposition, these reviews and a final path or none
/// (only these three fields reach `decision_done`).
fn title_decision(disposition: Disposition, reviews: Vec<ReviewReason>, path: bool) -> FileDecision {
    use crate::decision::Confidences;
    use crate::naming::NamedPath;
    use crate::tracks::audio::AudioDecision;
    use crate::tracks::layout::Layout;
    use crate::tracks::subtitle::{SubtitleCase, SubtitleDecision};
    FileDecision {
        title_key: "t00-00800".to_string(),
        audio: AudioDecision {
            kept: Vec::new(),
            dropped: Vec::new(),
            reference: None,
            verdict_reference: None,
            notes: Vec::new(),
            reviews: Vec::new(),
            confidence: 0.95,
            description_alignment: None,
        },
        subtitles: SubtitleDecision {
            kept: Vec::new(),
            dropped: Vec::new(),
            set_aside: Vec::new(),
            full: Vec::new(),
            groups: Vec::new(),
            dialogue_group: None,
            dialogue_pick: None,
            pairs: Vec::new(),
            case: SubtitleCase::NoEnglish,
            confidence: 0.95,
            notes: Vec::new(),
            reviews: Vec::new(),
        },
        layout: Layout { tracks: Vec::new(), chapters_kept: true },
        resolution: Some("1080p".to_string()),
        edition: None,
        path: path.then(|| NamedPath {
            root: "/library".to_string(),
            components: vec!["Show".to_string()],
            path: "/library/Show".to_string(),
        }),
        confidences: Confidences { identity: 0.95, title: 0.98, track: 0.95 },
        reviews,
        notes: Vec::new(),
        read_errors: Vec::new(),
        disposition,
    }
}

#[test]
fn a_job_of_several_titles_needs_review_when_any_title_does_and_has_a_path_only_when_every_title_has_one() {
    let mut decisions = vec![title_decision(Disposition::AutoFile, Vec::new(), true); 3];
    decisions.push(title_decision(Disposition::NeedsReview, vec![ReviewReason::TvUnmapped], false));
    let done = decision_done(&decisions);
    assert_eq!(
        done,
        Event::DecisionDone {
            disposition: Disposition::NeedsReview,
            reasons: vec![ReviewReason::TvUnmapped],
            has_path: false
        }
    );
    // Even an approved job is held: one title has no final path (Open point 38).
    assert_eq!(
        step(&State::Decided { approved: Some(vec![ReviewReason::TvUnmapped]) }, &done),
        (
            State::NeedsReview { held: Held::DispositionGate },
            vec![notify(NotifyEvent::NeedsReview { reasons: vec![ReviewReason::TvUnmapped] })]
        )
    );
}

#[test]
fn every_title_filing_automatically_gives_an_auto_file_job() {
    let decisions = vec![title_decision(Disposition::AutoFile, Vec::new(), true); 4];
    assert_eq!(
        decision_done(&decisions),
        Event::DecisionDone { disposition: Disposition::AutoFile, reasons: Vec::new(), has_path: true }
    );
    assert_eq!(
        step(&State::Inspecting, &decision_done(&decisions)),
        (State::Decided { approved: None }, vec![Action::CheckSpace])
    );
}

#[test]
fn reasons_from_several_titles_are_merged_sorted_and_unique() {
    let decisions = [
        title_decision(Disposition::NeedsReview, vec![ReviewReason::SubtitleAmbiguous, ReviewReason::PathCollision], true),
        title_decision(Disposition::NeedsReview, vec![ReviewReason::CommentaryDetected, ReviewReason::PathCollision], true),
        title_decision(Disposition::AutoFile, Vec::new(), true),
    ];
    assert_eq!(
        decision_done(&decisions),
        Event::DecisionDone {
            disposition: Disposition::NeedsReview,
            reasons: vec![ReviewReason::CommentaryDetected, ReviewReason::SubtitleAmbiguous, ReviewReason::PathCollision],
            has_path: true
        }
    );
}

#[test]
fn an_empty_decision_list_is_held_with_no_path() {
    assert_eq!(
        decision_done(&[]),
        Event::DecisionDone { disposition: Disposition::NeedsReview, reasons: Vec::new(), has_path: false }
    );
}

#[test]
fn an_auto_file_decision_checks_space_for_the_remux() {
    assert_eq!(
        step(&State::Inspecting, &Event::DecisionDone { disposition: Disposition::AutoFile, reasons: Vec::new(), has_path: true }),
        (State::Decided { approved: None }, vec![Action::CheckSpace])
    );
}

#[test]
fn a_needs_review_decision_holds_the_job_at_the_disposition_gate_and_sends_its_reasons() {
    let reasons = vec![ReviewReason::AudioReferenceUnclear, ReviewReason::SubtitleAmbiguous];
    let decided = Event::DecisionDone { disposition: Disposition::NeedsReview, reasons: reasons.clone(), has_path: true };
    assert_eq!(
        step(&State::Inspecting, &decided),
        (State::NeedsReview { held: Held::DispositionGate }, vec![notify(NotifyEvent::NeedsReview { reasons })])
    );
}

#[test]
fn a_decided_job_with_space_remuxes() {
    for approved in [None, Some(vec![ReviewReason::IdentityAmbiguous])] {
        assert_eq!(step(&State::Decided { approved: approved.clone() }, &Event::SpaceOk), (State::Remuxing, vec![Action::Remux]));
    }
}

#[test]
fn a_remuxed_file_is_verified() {
    assert_eq!(step(&State::Remuxing, &Event::RemuxDone), (State::Remuxing, vec![Action::VerifyFile]));
}

#[test]
fn a_verified_file_is_filed() {
    assert_eq!(step(&State::Remuxing, &Event::FileVerified), (State::Verified, vec![Action::File]));
}

#[test]
fn the_rename_into_the_library_files_the_job_and_sends_filed() {
    assert_eq!(step(&State::Verified, &Event::FileDone), (State::Filed, vec![notify(NotifyEvent::Filed)]));
}

#[test]
fn a_direct_job_walks_the_main_line_from_detected_to_filed() {
    let (start, first) = new_job();
    assert_eq!(first, [Action::Fingerprint]);
    let (end, actions) = walk(
        start,
        &[
            Event::FingerprintDone,
            Event::ScanDone,
            Event::PlanDone { mode: Mode::Direct },
            Event::SpaceOk,
            Event::TitleRipped,
            Event::TitleVerified { last: false },
            Event::TitleRipped,
            Event::TitleVerifyFailed { second: false, last: true },
            Event::TitleRipped,
            Event::TitleVerified { last: true },
            Event::Ejected,
            Event::FilePhaseSlot,
            Event::ProbeDone,
            Event::DecisionDone { disposition: Disposition::AutoFile, reasons: Vec::new(), has_path: true },
            Event::SpaceOk,
            Event::RemuxDone,
            Event::FileVerified,
            Event::FileDone,
        ],
    );
    assert_eq!(end, State::Filed);
    assert_eq!(
        actions,
        [
            vec![Action::RunScan],
            vec![Action::Plan],
            vec![Action::CheckSpace],
            vec![Action::RipTitle],
            vec![Action::VerifyFile],
            vec![Action::RipTitle],
            vec![Action::VerifyFile],
            vec![Action::RipTitle],
            vec![Action::VerifyFile],
            vec![Action::Eject],
            vec![notify(NotifyEvent::DiscDone)],
            vec![Action::Probe],
            vec![Action::ExtractSubtitleEvidence, Action::Decide],
            vec![Action::CheckSpace],
            vec![Action::Remux],
            vec![Action::VerifyFile],
            vec![Action::File],
            vec![notify(NotifyEvent::Filed)],
        ]
    );
}

// `reduce`: whole-disc copy mode (6.3, D18) and the extraction after approval.

#[test]
fn a_copy_plan_with_space_copies_the_disc() {
    assert_eq!(step(&State::Planned { mode: Mode::Copy }, &Event::SpaceOk), (State::Copying, vec![Action::CopyDisc]));
}

#[test]
fn a_copy_whose_scan_lists_titles_ejects_with_or_without_hash_errors() {
    for hash_errors in [false, true] {
        assert_eq!(
            step(&State::Copying, &Event::CopyDone { scan_listed_titles: true, hash_errors }),
            (State::RipVerified { after_copy: true }, vec![Action::Eject])
        );
    }
}

#[test]
fn a_copy_ejected_sends_disc_done_and_waits_to_be_held() {
    assert_eq!(
        step(&State::RipVerified { after_copy: true }, &Event::Ejected),
        (State::Ejected { after_copy: true }, vec![notify(NotifyEvent::DiscDone)])
    );
    assert!(rejects(&State::Copying, &Event::Ejected));
}

#[test]
fn a_copy_whose_eject_failed_sends_eject_failed_and_waits_to_be_held() {
    assert_eq!(
        step(&State::RipVerified { after_copy: true }, &Event::EjectFailed),
        (State::Ejected { after_copy: true }, vec![notify(NotifyEvent::EjectFailed)])
    );
}

#[test]
fn a_copy_job_ejected_is_held_for_review_with_its_reasons() {
    let reasons = vec![ReviewReason::TitleAmbiguous, ReviewReason::CopyHashErrors];
    assert_eq!(
        step(&State::Ejected { after_copy: true }, &Event::HoldForReview { reasons: reasons.clone() }),
        (State::NeedsReview { held: Held::AfterCopy }, vec![notify(NotifyEvent::NeedsReview { reasons })])
    );
}

#[test]
fn a_copy_job_ejected_does_not_take_the_file_phase_slot() {
    assert!(rejects(&State::Ejected { after_copy: true }, &Event::FilePhaseSlot));
}

#[test]
fn a_copy_whose_scan_failed_switches_to_direct_mode_with_a_note_and_checks_space() {
    for hash_errors in [false, true] {
        assert_eq!(
            step(&State::Copying, &Event::CopyDone { scan_listed_titles: false, hash_errors }),
            (
                State::Planned { mode: Mode::Direct },
                vec![Action::RecordNote { note: Note::DirectModeFallback }, Action::CheckSpace]
            )
        );
    }
}

#[test]
fn a_first_read_failure_of_the_copy_fails_the_job_with_media_read_error() {
    assert_eq!(
        step(&State::Copying, &Event::CopyFailed { failure: failure(FailureClass::MediaReadError), second: false }),
        (
            State::Failed { class: FailureClass::MediaReadError, from: boxed(State::Copying) },
            vec![notify(NotifyEvent::Failed { class: FailureClass::MediaReadError })]
        )
    );
}

#[test]
fn a_first_copy_failure_of_the_share_waits_for_the_share() {
    assert_eq!(
        step(&State::Copying, &Event::CopyFailed { failure: output(OutputCause::ShareUnreachable), second: false }),
        (
            State::AwaitingShare { return_to: boxed(State::Copying) },
            vec![notify(NotifyEvent::Waiting { reason: WaitReason::Share })]
        )
    );
}

#[test]
fn a_copy_failure_needing_the_network_starts_the_10_minute_timer_before_the_notification() {
    assert_eq!(
        step(&State::Copying, &Event::CopyFailed { failure: failure(FailureClass::NeedsNetwork), second: false }),
        (
            State::AwaitingNetwork { return_to: boxed(State::Copying) },
            vec![
                Action::StartTimer { minutes: NETWORK_RETRY_MIN },
                notify(NotifyEvent::Waiting { reason: WaitReason::Network })
            ]
        )
    );
}

#[test]
fn a_second_read_failure_of_the_copy_switches_to_direct_mode_with_a_note() {
    assert_eq!(
        step(&State::Copying, &Event::CopyFailed { failure: failure(FailureClass::MediaReadError), second: true }),
        (
            State::Planned { mode: Mode::Direct },
            vec![Action::RecordNote { note: Note::DirectModeFallback }, Action::CheckSpace]
        )
    );
}

#[test]
fn a_second_copy_failure_that_is_not_a_read_failure_is_an_ordinary_outcome() {
    assert_eq!(
        step(&State::Copying, &Event::CopyFailed { failure: failure(FailureClass::Unknown), second: true }),
        (
            State::Failed { class: FailureClass::Unknown, from: boxed(State::Copying) },
            vec![notify(NotifyEvent::Failed { class: FailureClass::Unknown })]
        )
    );
}

#[test]
fn approving_a_copy_job_checks_space_for_the_extraction() {
    assert_eq!(
        step(
            &State::NeedsReview { held: Held::AfterCopy },
            &Event::ReviewApproved { overrides: Overrides::default(), approved_reasons: vec![ReviewReason::TitleAmbiguous] }
        ),
        (State::Extracting, vec![Action::CheckSpace])
    );
}

#[test]
fn extracting_with_space_extracts_a_title() {
    assert_eq!(step(&State::Extracting, &Event::SpaceOk), (State::Extracting, vec![Action::ExtractTitle]));
}

#[test]
fn an_extracted_title_is_verified() {
    assert_eq!(step(&State::Extracting, &Event::TitleExtracted), (State::Extracting, vec![Action::VerifyFile]));
}

#[test]
fn an_extracted_title_verified_that_is_not_the_last_extracts_the_next() {
    assert_eq!(
        step(&State::Extracting, &Event::TitleVerified { last: false }),
        (State::Extracting, vec![Action::ExtractTitle])
    );
}

#[test]
fn an_extracted_title_failing_verification_is_extracted_again_once_then_the_next_follows() {
    for last in [false, true] {
        assert_eq!(
            step(&State::Extracting, &Event::TitleVerifyFailed { second: false, last }),
            (State::Extracting, vec![Action::ExtractTitle])
        );
    }
    assert_eq!(
        step(&State::Extracting, &Event::TitleVerifyFailed { second: true, last: false }),
        (State::Extracting, vec![Action::ExtractTitle])
    );
}

#[test]
fn the_last_extracted_title_verified_goes_on_to_inspecting_and_probes() {
    assert_eq!(
        step(&State::Extracting, &Event::TitleVerified { last: true }),
        (State::Inspecting, vec![Action::Probe])
    );
    assert_eq!(
        step(&State::Extracting, &Event::TitleVerifyFailed { second: true, last: true }),
        (State::Inspecting, vec![Action::Probe])
    );
}

#[test]
fn a_copy_job_walks_from_its_plan_through_review_and_extraction_to_filed() {
    let (end, actions) = walk(
        State::Scanned,
        &[
            Event::PlanDone { mode: Mode::Copy },
            Event::SpaceOk,
            Event::CopyDone { scan_listed_titles: true, hash_errors: true },
            Event::Ejected,
            Event::HoldForReview { reasons: vec![ReviewReason::TitleAmbiguous, ReviewReason::CopyHashErrors] },
            Event::ReviewApproved {
                overrides: Overrides::default(),
                approved_reasons: vec![ReviewReason::TitleAmbiguous, ReviewReason::CopyHashErrors],
            },
            Event::SpaceOk,
            Event::TitleExtracted,
            Event::TitleVerified { last: true },
            Event::ProbeDone,
            Event::DecisionDone { disposition: Disposition::AutoFile, reasons: Vec::new(), has_path: true },
            Event::SpaceOk,
            Event::RemuxDone,
            Event::FileVerified,
            Event::FileDone,
        ],
    );
    assert_eq!(end, State::Filed);
    assert_eq!(
        actions,
        [
            vec![Action::CheckSpace],
            vec![Action::CopyDisc],
            vec![Action::Eject],
            vec![notify(NotifyEvent::DiscDone)],
            vec![notify(NotifyEvent::NeedsReview {
                reasons: vec![ReviewReason::TitleAmbiguous, ReviewReason::CopyHashErrors]
            })],
            vec![Action::CheckSpace],
            vec![Action::ExtractTitle],
            vec![Action::VerifyFile],
            vec![Action::Probe],
            vec![Action::ExtractSubtitleEvidence, Action::Decide],
            vec![Action::CheckSpace],
            vec![Action::Remux],
            vec![Action::VerifyFile],
            vec![Action::File],
            vec![notify(NotifyEvent::Filed)],
        ]
    );
}

// `reduce`: the review queue (10.1, spec 11).

#[test]
fn approving_at_the_disposition_gate_decides_again_with_the_owners_overrides_and_records_the_reasons_approved() {
    let overrides = Overrides { season: Some(3), ..Overrides::default() };
    let approved_reasons = vec![ReviewReason::TvUnmapped];
    assert_eq!(
        step(&State::NeedsReview { held: Held::DispositionGate }, &Event::ReviewApproved { overrides, approved_reasons }),
        (State::Decided { approved: Some(vec![ReviewReason::TvUnmapped]) }, vec![Action::Decide])
    );
}

#[test]
fn a_decision_after_approval_checks_space_whatever_its_disposition() {
    let approved = State::Decided { approved: Some(vec![ReviewReason::TvUnmapped]) };
    for disposition in [Disposition::AutoFile, Disposition::NeedsReview] {
        assert_eq!(
            step(&approved, &Event::DecisionDone { disposition, reasons: vec![ReviewReason::TvUnmapped], has_path: true }),
            (approved.clone(), vec![Action::CheckSpace])
        );
    }
}

#[test]
fn a_redecided_auto_filed_job_that_now_needs_review_is_held() {
    // A job that auto-filed (Decided { approved: None }) decides again after a restart, a Return or a Retry; the
    // settings or the library facts may have changed meanwhile (6.4 step 12: it "stops here until approved").
    let unapproved = State::Decided { approved: None };
    assert_eq!(
        step(&unapproved, &Event::DecisionDone { disposition: Disposition::AutoFile, reasons: Vec::new(), has_path: true }),
        (unapproved.clone(), vec![Action::CheckSpace])
    );
    let reasons = vec![ReviewReason::PathCollision];
    assert_eq!(
        step(&unapproved, &Event::DecisionDone { disposition: Disposition::NeedsReview, reasons: reasons.clone(), has_path: true }),
        (State::NeedsReview { held: Held::DispositionGate }, vec![notify(NotifyEvent::NeedsReview { reasons })])
    );
}

#[test]
fn an_approved_job_walks_from_the_gate_through_the_remux_to_filed() {
    let (end, actions) = walk(
        State::NeedsReview { held: Held::DispositionGate },
        &[
            Event::ReviewApproved { overrides: Overrides::default(), approved_reasons: vec![ReviewReason::ReadErrors] },
            Event::DecisionDone { disposition: Disposition::NeedsReview, reasons: vec![ReviewReason::ReadErrors], has_path: true },
            Event::SpaceOk,
            Event::RemuxDone,
            Event::FileVerified,
            Event::FileDone,
        ],
    );
    assert_eq!(end, State::Filed);
    assert_eq!(
        actions,
        [
            vec![Action::Decide],
            vec![Action::CheckSpace],
            vec![Action::Remux],
            vec![Action::VerifyFile],
            vec![Action::File],
            vec![notify(NotifyEvent::Filed)],
        ]
    );
}

#[test]
fn an_approved_job_whose_decision_made_again_raises_path_collision_is_held_at_the_gate() {
    // Spec 7: the job never overwrites a file it did not create; 6.4 step 12 holds it again, with the same notification,
    // even when the owner approved the reasons the decision carries.
    let approved = State::Decided {
        approved: Some(vec![ReviewReason::TvUnmapped, ReviewReason::PathCollision, ReviewReason::ReadErrors]),
    };
    for reasons in [
        vec![ReviewReason::PathCollision],
        vec![ReviewReason::TvUnmapped, ReviewReason::PathCollision, ReviewReason::ReadErrors],
    ] {
        assert_eq!(
            step(&approved, &Event::DecisionDone { disposition: Disposition::NeedsReview, reasons: reasons.clone(), has_path: true }),
            (State::NeedsReview { held: Held::DispositionGate }, vec![notify(NotifyEvent::NeedsReview { reasons })])
        );
    }
}

#[test]
fn an_approved_job_whose_decision_made_again_has_no_final_path_is_held_at_the_gate() {
    // Open point 38: `decide` gives NeedsReview whenever the path is None; `has_path` false says so, with any reasons.
    let approved = State::Decided { approved: Some(vec![ReviewReason::TvUnmapped]) };
    for reasons in [Vec::new(), vec![ReviewReason::TvUnmapped]] {
        assert_eq!(
            step(
                &approved,
                &Event::DecisionDone { disposition: Disposition::NeedsReview, reasons: reasons.clone(), has_path: false }
            ),
            (State::NeedsReview { held: Held::DispositionGate }, vec![notify(NotifyEvent::NeedsReview { reasons })])
        );
    }
}

#[test]
fn an_approved_job_whose_decision_made_again_is_held_only_by_a_low_confidence_goes_on() {
    // 8.8: only a review decision raises a confidence, and spec 11's Approve continues with the reviewed decisions; a
    // NeedsReview disposition with no review reason (a confidence below auto_threshold) does not hold the job again.
    let approved = State::Decided { approved: Some(Vec::new()) };
    assert_eq!(
        step(&approved, &Event::DecisionDone { disposition: Disposition::NeedsReview, reasons: Vec::new(), has_path: true }),
        (approved.clone(), vec![Action::CheckSpace])
    );
}

#[test]
fn an_approved_job_goes_on_for_every_review_reason_the_owner_approved_but_path_collision() {
    // 8.8: the owner approved the reasons the job carried; only path_collision (and a missing path) still blocks the file.
    let approved = State::Decided { approved: Some(ReviewReason::ALL.to_vec()) };
    for reason in ReviewReason::ALL.into_iter().filter(|reason| *reason != ReviewReason::PathCollision) {
        assert_eq!(
            step(&approved, &Event::DecisionDone { disposition: Disposition::NeedsReview, reasons: vec![reason], has_path: true }),
            (approved.clone(), vec![Action::CheckSpace]),
            "{reason:?}"
        );
    }
    assert_eq!(
        step(&approved, &Event::DecisionDone { disposition: Disposition::AutoFile, reasons: Vec::new(), has_path: true }),
        (approved.clone(), vec![Action::CheckSpace])
    );
}

#[test]
fn a_re_finalised_job_whose_recorded_file_no_longer_matches_is_held_until_the_owner_resolves_it() {
    // Spec 7: a same-path decision after a re-finalise raises path_collision; Open point 39: the engine does not file it.
    // The filed decision carried read_errors, which filing approved; the gate shows both reasons, the owner approves
    // them with keep both, and the decision made again carries read_errors only.
    let reasons = vec![ReviewReason::PathCollision, ReviewReason::ReadErrors];
    let (end, actions) = walk(
        State::Filed,
        &[
            Event::ReFinalise { approved_reasons: vec![ReviewReason::ReadErrors] },
            Event::DecisionDone { disposition: Disposition::NeedsReview, reasons: reasons.clone(), has_path: true },
            Event::ReviewApproved {
                overrides: Overrides {
                    collision: vec![CollisionAnswer { path: "/srv/media/Movies/Film.mkv".to_string(), action: Collision::KeepBoth }],
                    ..Overrides::default()
                },
                approved_reasons: reasons.clone(),
            },
            Event::DecisionDone { disposition: Disposition::NeedsReview, reasons: vec![ReviewReason::ReadErrors], has_path: true },
        ],
    );
    assert_eq!(end, State::Decided { approved: Some(reasons.clone()) });
    assert_eq!(
        actions,
        [
            vec![Action::Decide],
            vec![notify(NotifyEvent::NeedsReview { reasons })],
            vec![Action::Decide],
            vec![Action::CheckSpace],
        ]
    );
}

#[test]
fn discarding_a_held_job_deletes_its_data() {
    for held in Held::ALL {
        assert_eq!(
            step(&State::NeedsReview { held }, &Event::ReviewDiscarded),
            (State::Discarded, vec![Action::DeleteJobData])
        );
    }
}

#[test]
fn re_finalising_a_filed_job_decides_again_approving_the_filed_decisions_reasons() {
    assert_eq!(
        step(&State::Filed, &Event::ReFinalise { approved_reasons: vec![ReviewReason::ReadErrors] }),
        (State::Decided { approved: Some(vec![ReviewReason::ReadErrors]) }, vec![Action::Decide])
    );
}

#[test]
fn an_approved_job_whose_decision_made_again_carries_a_reason_the_owner_did_not_approve_is_held() {
    // F12: held with identity_ambiguous only, the owner sets the identity to a Japanese film and approves. The decision
    // made again wants Japanese audio, which the disc lacks, so it raises original_language_missing: the owner never saw
    // that reason, and 8.6 says the review reasons hold the job regardless (6.4 step 12).
    let japanese = TmdbIdentity { kind: TmdbKind::Movie, id: 4935 };
    let raised = vec![ReviewReason::OriginalLanguageMissing];
    let (end, actions) = walk(
        State::NeedsReview { held: Held::DispositionGate },
        &[
            Event::ReviewApproved {
                overrides: Overrides { identity: Some(japanese), ..Overrides::default() },
                approved_reasons: vec![ReviewReason::IdentityAmbiguous],
            },
            Event::DecisionDone { disposition: Disposition::NeedsReview, reasons: raised.clone(), has_path: true },
        ],
    );
    assert_eq!(end, State::NeedsReview { held: Held::DispositionGate });
    assert_eq!(actions, [vec![Action::Decide], vec![notify(NotifyEvent::NeedsReview { reasons: raised })]]);
    // Approved again with the new reason shown, the job goes on.
    let approved = State::Decided { approved: Some(vec![ReviewReason::OriginalLanguageMissing]) };
    let again = Event::DecisionDone {
        disposition: Disposition::NeedsReview,
        reasons: vec![ReviewReason::OriginalLanguageMissing],
        has_path: true,
    };
    assert_eq!(step(&approved, &again), (approved.clone(), vec![Action::CheckSpace]));
    // A re-finalise approves the filed decision's reasons only: a new no_wanted_audio holds the job, never files it.
    let (end, actions) = walk(
        State::Filed,
        &[
            Event::ReFinalise { approved_reasons: Vec::new() },
            Event::DecisionDone {
                disposition: Disposition::NeedsReview,
                reasons: vec![ReviewReason::NoWantedAudio],
                has_path: true,
            },
        ],
    );
    assert_eq!(end, State::NeedsReview { held: Held::DispositionGate });
    assert_eq!(
        actions,
        [vec![Action::Decide], vec![notify(NotifyEvent::NeedsReview { reasons: vec![ReviewReason::NoWantedAudio] })]]
    );
}

#[test]
fn rip_again_arms_a_new_job_for_a_job_past_its_disc_phase_and_keeps_its_state() {
    let mut states = vec![
        State::Filed,
        State::NeedsReview { held: Held::AfterCopy },
        State::NeedsReview { held: Held::DispositionGate },
        State::Failed { class: FailureClass::Unknown, from: boxed(State::Remuxing) },
    ];
    states.extend(file_phase_states());
    // A waiting job returning to a file-phase state is past the disc phase too (job_lookup's reading).
    states.extend(waiting_states(State::Inspecting));
    for state in states {
        assert_eq!(step(&state, &Event::RipAgain), (state.clone(), vec![Action::ArmRipAgain]));
    }
}

#[test]
fn rip_again_is_rejected_in_the_disc_phase_and_for_discarded_or_disc_phase_waiting_or_failed_jobs() {
    let mut states = disc_phase_states();
    states.extend(waiting_states(State::Ripping));
    states.push(State::Failed { class: FailureClass::Unknown, from: boxed(State::Ripping) });
    states.push(State::Discarded);
    for state in states {
        assert!(rejects(&state, &Event::RipAgain), "{state:?}");
    }
}

#[test]
fn a_held_job_takes_no_disc_or_file_phase_event() {
    for held in Held::ALL {
        for event in [Event::SpaceOk, Event::FilePhaseSlot, Event::ProbeDone, Event::ShareLost, Event::MediaArrived] {
            assert!(rejects(&State::NeedsReview { held }, &event), "{held:?} {event:?}");
        }
    }
}

#[test]
fn re_finalise_and_discard_are_rejected_outside_their_states() {
    let re_finalise = Event::ReFinalise { approved_reasons: Vec::new() };
    assert!(rejects(&State::Decided { approved: Some(Vec::new()) }, &re_finalise));
    assert!(rejects(&State::NeedsReview { held: Held::DispositionGate }, &re_finalise));
    assert!(rejects(&State::Filed, &Event::ReviewDiscarded));
    assert!(rejects(
        &State::Inspecting,
        &Event::ReviewApproved { overrides: Overrides::default(), approved_reasons: Vec::new() }
    ));
    // A waiting job is neither held nor failed: Discard waits until it is.
    assert!(rejects(&State::AwaitingSpace { return_to: boxed(State::Copying) }, &Event::ReviewDiscarded));
    for state in disc_phase_states().into_iter().chain(file_phase_states()) {
        assert!(rejects(&state, &Event::ReviewDiscarded), "{state:?}");
    }
    assert!(rejects(&State::Discarded, &Event::ReviewDiscarded));
}

#[test]
fn discarding_a_failed_job_deletes_its_data() {
    // Spec 11's Discard applies to a failed job too (spec 7 never deletes its data automatically): every class that
    // fails a job, from a disc-phase and from a file-phase step.
    for class in [FailureClass::DriveIncapable, FailureClass::MediaReadError, FailureClass::Unknown] {
        for from in [State::Copying, State::Remuxing] {
            assert_eq!(
                step(&State::Failed { class, from: boxed(from.clone()) }, &Event::ReviewDiscarded),
                (State::Discarded, vec![Action::DeleteJobData]),
                "{class:?} from {from:?}"
            );
        }
    }
}

// `reduce`: the waiting states and their returns (10.1, section 7, 10.2's ToolMissing row).

/// Every disc-phase and file-phase state.
fn working_states() -> Vec<State> {
    let mut states = disc_phase_states();
    states.extend(file_phase_states());
    states
}

#[test]
fn a_lost_share_in_any_disc_or_file_phase_state_waits_for_the_share() {
    for state in working_states() {
        assert_eq!(
            step(&state, &Event::ShareLost),
            (
                State::AwaitingShare { return_to: boxed(state.clone()) },
                vec![notify(NotifyEvent::Waiting { reason: WaitReason::Share })]
            )
        );
    }
}

#[test]
fn short_space_in_any_disc_or_file_phase_state_waits_for_space() {
    for state in working_states() {
        assert_eq!(
            step(&state, &Event::SpaceShort),
            (
                State::AwaitingSpace { return_to: boxed(state.clone()) },
                vec![notify(NotifyEvent::Waiting { reason: WaitReason::Space })]
            )
        );
    }
}

#[test]
fn a_lost_share_or_short_space_is_rejected_for_a_held_filed_failed_discarded_or_waiting_job() {
    let mut states = resting_states();
    states.extend(waiting_states(State::Ripping));
    for state in states {
        assert!(rejects(&state, &Event::ShareLost), "{state:?}");
        assert!(rejects(&state, &Event::SpaceShort), "{state:?}");
    }
}

#[test]
fn the_share_back_returns_to_the_interrupted_state_with_its_resume_actions_then_resumed() {
    assert_eq!(
        step(&State::AwaitingShare { return_to: boxed(State::Ripping) }, &Event::ShareBack),
        (State::Ripping, vec![Action::EmptyTitleDirectory, Action::RipTitle, notify(NotifyEvent::Resumed)])
    );
    assert_eq!(
        step(&State::AwaitingShare { return_to: boxed(State::Remuxing) }, &Event::ShareBack),
        (State::Remuxing, vec![Action::DeletePartialFile, Action::Remux, notify(NotifyEvent::Resumed)])
    );
}

#[test]
fn space_freed_returns_to_the_interrupted_state_with_its_resume_actions_then_resumed() {
    assert_eq!(
        step(&State::AwaitingSpace { return_to: boxed(State::Decided { approved: Some(Vec::new()) }) }, &Event::SpaceFreed),
        (State::Decided { approved: Some(Vec::new()) }, vec![Action::Decide, notify(NotifyEvent::Resumed)])
    );
}

#[test]
fn a_direct_fallback_whose_titles_do_not_fit_waits_in_awaiting_space_and_checks_again_when_freed() {
    let (end, actions) = walk(
        State::Copying,
        &[
            Event::CopyDone { scan_listed_titles: false, hash_errors: false },
            Event::SpaceShort,
            Event::SpaceFreed,
            Event::SpaceOk,
        ],
    );
    assert_eq!(end, State::Ripping);
    assert_eq!(
        actions,
        [
            vec![Action::RecordNote { note: Note::DirectModeFallback }, Action::CheckSpace],
            vec![notify(NotifyEvent::Waiting { reason: WaitReason::Space })],
            vec![Action::CheckSpace, notify(NotifyEvent::Resumed)],
            vec![Action::RipTitle],
        ]
    );
}

#[test]
fn a_licence_or_version_message_while_scanned_ripping_or_copying_waits_for_makemkv() {
    for state in [State::Scanned, State::Ripping, State::Copying] {
        for event in [Event::LicenceMessage, Event::VersionMessage] {
            assert_eq!(
                step(&state, &event),
                (
                    State::AwaitingMakeMkv { return_to: boxed(state.clone()) },
                    vec![notify(NotifyEvent::Waiting { reason: WaitReason::Makemkv })]
                )
            );
        }
    }
}

#[test]
fn a_licence_or_version_message_is_rejected_outside_scanned_ripping_and_copying() {
    for state in [
        State::Fingerprinted,
        State::Planned { mode: Mode::Direct },
        State::RipVerified { after_copy: false },
        State::Inspecting,
    ] {
        assert!(rejects(&state, &Event::LicenceMessage), "{state:?}");
        assert!(rejects(&state, &Event::VersionMessage), "{state:?}");
    }
}

#[test]
fn a_changed_makemkv_returns_to_the_interrupted_state() {
    assert_eq!(
        step(&State::AwaitingMakeMkv { return_to: boxed(State::Copying) }, &Event::MakeMkvChanged),
        (State::Copying, vec![Action::EmptyCopyDirectory, Action::CopyDisc, notify(NotifyEvent::Resumed)])
    );
}

#[test]
fn a_disc_needing_the_network_waits_and_starts_the_10_minute_timer() {
    for state in [State::Scanned, State::Ripping, State::Copying] {
        assert_eq!(
            step(&state, &Event::NetworkNeeded),
            (
                State::AwaitingNetwork { return_to: boxed(state.clone()) },
                vec![
                    Action::StartTimer { minutes: NETWORK_RETRY_MIN },
                    notify(NotifyEvent::Waiting { reason: WaitReason::Network })
                ]
            )
        );
    }
}

#[test]
fn only_scanned_ripping_and_copying_wait_for_the_network() {
    for state in [
        State::Detected,
        State::Fingerprinted,
        State::RipVerified { after_copy: false },
        State::Inspecting,
        State::Remuxing,
    ] {
        assert!(rejects(&state, &Event::NetworkNeeded), "{state:?}");
    }
}

#[test]
fn the_network_retry_returns_to_the_interrupted_state() {
    assert_eq!(
        step(&State::AwaitingNetwork { return_to: boxed(State::Scanned) }, &Event::NetworkRetry),
        (State::Scanned, vec![Action::Plan, notify(NotifyEvent::Resumed)])
    );
}

#[test]
fn a_pending_lookup_while_ejected_or_inspecting_waits_and_starts_the_hourly_timer() {
    for state in [State::Ejected { after_copy: false }, State::Ejected { after_copy: true }, State::Inspecting] {
        assert_eq!(
            step(&state, &Event::LookupPending),
            (
                State::AwaitingLookup { return_to: boxed(state.clone()) },
                vec![
                    Action::StartTimer { minutes: LOOKUP_RETRY_MIN },
                    notify(NotifyEvent::Waiting { reason: WaitReason::Lookup })
                ]
            )
        );
    }
}

#[test]
fn the_disc_phase_never_waits_for_a_lookup() {
    for state in disc_phase_states() {
        assert!(rejects(&state, &Event::LookupPending), "{state:?}");
    }
    for approved in [None, Some(vec![ReviewReason::IdentityAmbiguous])] {
        assert!(rejects(&State::Decided { approved }, &Event::LookupPending));
    }
}

#[test]
fn a_lookup_still_pending_at_the_hourly_retry_starts_the_timer_again() {
    let waiting = State::AwaitingLookup { return_to: boxed(State::Inspecting) };
    assert_eq!(
        step(&waiting, &Event::LookupPending),
        (waiting.clone(), vec![Action::StartTimer { minutes: LOOKUP_RETRY_MIN }])
    );
}

#[test]
fn an_answered_lookup_refreshes_the_identity_and_returns_to_the_interrupted_state() {
    // F6 (8.1): the answers reach the plan before the file phase's next action, so the Decide that follows uses them.
    assert_eq!(
        step(&State::AwaitingLookup { return_to: boxed(State::Inspecting) }, &Event::LookupAnswered),
        (State::Inspecting, vec![Action::RefreshIdentity, Action::Probe, notify(NotifyEvent::Resumed)])
    );
    assert_eq!(
        step(&State::AwaitingLookup { return_to: boxed(State::Ejected { after_copy: false }) }, &Event::LookupAnswered),
        (State::Ejected { after_copy: false }, vec![Action::RefreshIdentity, notify(NotifyEvent::Resumed)])
    );
    // A lookup still pending at the hourly retry refreshes nothing.
    let waiting = State::AwaitingLookup { return_to: boxed(State::Inspecting) };
    assert_eq!(step(&waiting, &Event::LookupPending), (waiting.clone(), vec![Action::StartTimer { minutes: LOOKUP_RETRY_MIN }]));
}

#[test]
fn a_missing_makemkvcon_holds_the_disc_phase_and_an_extraction() {
    let mut states = disc_phase_states();
    states.push(State::Extracting);
    for state in states {
        assert_eq!(
            step(&state, &Event::ToolMissing { tool: Tool::Makemkvcon }),
            (
                State::AwaitingTool { return_to: boxed(state.clone()) },
                vec![notify(NotifyEvent::Waiting { reason: WaitReason::Tool })]
            )
        );
    }
}

#[test]
fn a_missing_makemkvcon_is_rejected_by_the_file_phase_states_that_do_not_run_it() {
    for state in file_phase_states() {
        if state == State::Extracting {
            continue;
        }
        assert!(rejects(&state, &Event::ToolMissing { tool: Tool::Makemkvcon }), "{state:?}");
    }
}

#[test]
fn a_missing_mkvmerge_holds_every_disc_and_file_phase_state() {
    for state in working_states() {
        assert_eq!(
            step(&state, &Event::ToolMissing { tool: Tool::Mkvmerge }),
            (
                State::AwaitingTool { return_to: boxed(state.clone()) },
                vec![notify(NotifyEvent::Waiting { reason: WaitReason::Tool })]
            )
        );
    }
}

#[test]
fn a_missing_ffprobe_or_ffmpeg_holds_the_file_phase() {
    for tool in [Tool::Ffprobe, Tool::Ffmpeg] {
        for state in file_phase_states() {
            assert_eq!(
                step(&state, &Event::ToolMissing { tool }),
                (
                    State::AwaitingTool { return_to: boxed(state.clone()) },
                    vec![notify(NotifyEvent::Waiting { reason: WaitReason::Tool })]
                )
            );
        }
    }
}

#[test]
fn a_missing_ffprobe_or_ffmpeg_lets_the_disc_phase_run_on() {
    for tool in [Tool::Ffprobe, Tool::Ffmpeg] {
        for state in disc_phase_states() {
            assert_eq!(step(&state, &Event::ToolMissing { tool }), (state.clone(), Vec::new()), "{tool:?} {state:?}");
        }
    }
}

#[test]
fn a_tool_missing_is_rejected_for_a_held_filed_failed_discarded_or_waiting_job_except_java() {
    let mut states = resting_states();
    states.extend(waiting_states(State::Ripping));
    for state in states {
        for tool in [Tool::Makemkvcon, Tool::Mkvmerge, Tool::Ffprobe, Tool::Ffmpeg] {
            assert!(rejects(&state, &Event::ToolMissing { tool }), "{tool:?} {state:?}");
        }
    }
}

#[test]
fn a_found_tool_returns_to_the_interrupted_state() {
    for tool in Tool::ALL {
        assert_eq!(
            step(&State::AwaitingTool { return_to: boxed(State::Detected) }, &Event::ToolFound { tool }),
            (State::Detected, vec![Action::Fingerprint, notify(NotifyEvent::Resumed)])
        );
    }
}

#[test]
fn a_waiting_state_returns_only_on_its_own_event() {
    let releases = [
        Event::ShareBack,
        Event::SpaceFreed,
        Event::MakeMkvChanged,
        Event::NetworkRetry,
        Event::LookupAnswered,
        Event::ToolFound { tool: Tool::Mkvmerge },
    ];
    for (index, waiting) in waiting_states(State::Ripping).into_iter().enumerate() {
        for (other, release) in releases.iter().enumerate() {
            assert_eq!(rejects(&waiting, release), index != other, "{waiting:?} {release:?}");
        }
    }
}

// `reduce`: step failures, Retry, the disc inserted again, and the disc leaving or arriving in the disc phase.

/// Every step-failure event with the state whose step it reports, for `failure`.
fn failing_steps(failure: Failure) -> Vec<(State, Event)> {
    vec![
        (State::Detected, Event::FingerprintFailed { failure }),
        (State::Fingerprinted, Event::ScanFailed { failure }),
        (State::Scanned, Event::PlanFailed { failure }),
        (State::Ripping, Event::TitleRipFailed { failure }),
        (State::Extracting, Event::ExtractFailed { failure }),
        (State::Inspecting, Event::ProbeFailed { failure }),
        (State::Inspecting, Event::DecisionFailed { failure }),
        (State::Decided { approved: None }, Event::DecisionFailed { failure }),
        (State::Decided { approved: Some(Vec::new()) }, Event::DecisionFailed { failure }),
        (State::Remuxing, Event::RemuxFailed { failure }),
        (State::Remuxing, Event::FileVerifyFailed { failure }),
        (State::Verified, Event::FileFailed { failure }),
    ]
}

#[test]
fn a_failed_step_with_a_failing_class_fails_the_job_and_sends_failed_with_the_class() {
    for class in [FailureClass::DriveIncapable, FailureClass::MediaReadError, FailureClass::Unknown] {
        for (state, event) in failing_steps(failure(class)) {
            assert_eq!(
                step(&state, &event),
                (
                    State::Failed { class, from: boxed(state.clone()) },
                    vec![notify(NotifyEvent::Failed { class })]
                ),
                "{event:?}"
            );
        }
    }
}

#[test]
fn a_failed_step_with_a_waiting_class_waits_with_its_reason_and_returns_to_the_step() {
    let cases = [
        (failure(FailureClass::Licence), WaitReason::Makemkv),
        (failure(FailureClass::NeedsMakeMkvUpdate), WaitReason::Makemkv),
        (output(OutputCause::OutOfSpace), WaitReason::Space),
        (output(OutputCause::ShareUnreachable), WaitReason::Share),
        (output(OutputCause::WriteError), WaitReason::Share),
        (failure(FailureClass::ToolMissing), WaitReason::Tool),
    ];
    for (cause, reason) in cases {
        for (state, event) in failing_steps(cause) {
            let (waiting, actions) = step(&state, &event);
            assert_eq!(waiting, failure_outcome(cause, &state), "{event:?}");
            assert_eq!(actions, [notify(NotifyEvent::Waiting { reason })], "{event:?}");
        }
    }
}

#[test]
fn a_failed_step_needing_the_network_starts_the_10_minute_timer_before_the_notification() {
    for (state, event) in failing_steps(failure(FailureClass::NeedsNetwork)) {
        assert_eq!(
            step(&state, &event),
            (
                State::AwaitingNetwork { return_to: boxed(state.clone()) },
                vec![
                    Action::StartTimer { minutes: NETWORK_RETRY_MIN },
                    notify(NotifyEvent::Waiting { reason: WaitReason::Network })
                ]
            ),
            "{event:?}"
        );
    }
}

#[test]
fn a_failed_step_with_a_lookup_unavailable_starts_the_hourly_timer_before_the_notification() {
    for (state, event) in failing_steps(failure(FailureClass::LookupUnavailable)) {
        assert_eq!(
            step(&state, &event),
            (
                State::AwaitingLookup { return_to: boxed(state.clone()) },
                vec![
                    Action::StartTimer { minutes: LOOKUP_RETRY_MIN },
                    notify(NotifyEvent::Waiting { reason: WaitReason::Lookup })
                ]
            ),
            "{event:?}"
        );
    }
}

#[test]
fn a_step_failure_is_rejected_from_a_state_that_did_not_run_the_step() {
    let read = failure(FailureClass::MediaReadError);
    assert!(rejects(&State::Ripping, &Event::ProbeFailed { failure: read }));
    assert!(rejects(&State::Detected, &Event::ScanFailed { failure: read }));
    assert!(rejects(&State::Copying, &Event::TitleRipFailed { failure: read }));
    assert!(rejects(&State::Verified, &Event::RemuxFailed { failure: read }));
    assert!(rejects(&State::Filed, &Event::FileFailed { failure: read }));
}

#[test]
fn a_share_that_drops_mid_rip_fails_the_title_then_the_title_is_ripped_again_when_the_share_is_back() {
    let dropped = classify_title_failure(false, false, false, true);
    let (end, actions) = walk(State::Ripping, &[Event::TitleRipFailed { failure: dropped }, Event::ShareBack]);
    assert_eq!(end, State::Ripping);
    assert_eq!(
        actions,
        [
            vec![notify(NotifyEvent::Waiting { reason: WaitReason::Share })],
            vec![Action::EmptyTitleDirectory, Action::RipTitle, notify(NotifyEvent::Resumed)],
        ]
    );
}

#[test]
fn retry_takes_a_failed_job_back_to_the_state_that_failed_with_its_resume_actions() {
    for class in FailureClass::ALL {
        if class == FailureClass::DriveIncapable {
            continue;
        }
        assert_eq!(
            step(&State::Failed { class, from: boxed(State::Ripping) }, &Event::Retry),
            (State::Ripping, vec![Action::EmptyTitleDirectory, Action::RipTitle]),
            "{class:?}"
        );
    }
    assert_eq!(
        step(&State::Failed { class: FailureClass::Unknown, from: boxed(State::Inspecting) }, &Event::Retry),
        (State::Inspecting, vec![Action::Probe])
    );
}

#[test]
fn retrying_a_failed_decision_after_approval_decides_again() {
    assert_eq!(
        step(
            &State::Failed { class: FailureClass::Unknown, from: boxed(State::Decided { approved: Some(Vec::new()) }) },
            &Event::Retry,
        ),
        (State::Decided { approved: Some(Vec::new()) }, vec![Action::Decide])
    );
}

#[test]
fn retry_is_rejected_for_a_drive_incapable_failure() {
    for from in [State::Scanned, State::Ripping, State::Copying] {
        assert!(rejects(&State::Failed { class: FailureClass::DriveIncapable, from: boxed(from) }, &Event::Retry));
    }
}

#[test]
fn retry_is_rejected_for_a_job_that_has_not_failed() {
    for state in [State::Ripping, State::NeedsReview { held: Held::DispositionGate }, State::Filed, State::Discarded] {
        assert!(rejects(&state, &Event::Retry), "{state:?}");
    }
}

#[test]
fn reinserting_the_disc_resumes_only_a_media_read_error() {
    assert_eq!(
        step(&State::Failed { class: FailureClass::MediaReadError, from: boxed(State::Ripping) }, &Event::MediaArrived),
        (State::Ripping, vec![Action::EmptyTitleDirectory, Action::RipTitle, notify(NotifyEvent::Resumed)])
    );
    for class in FailureClass::ALL {
        if class == FailureClass::MediaReadError {
            continue;
        }
        assert!(rejects(&State::Failed { class, from: boxed(State::Ripping) }, &Event::MediaArrived), "{class:?}");
    }
}

#[test]
fn reinserting_the_disc_after_a_copy_read_failure_restarts_the_copy_from_scratch() {
    let (end, actions) = walk(
        State::Copying,
        &[Event::CopyFailed { failure: failure(FailureClass::MediaReadError), second: false }, Event::MediaArrived],
    );
    assert_eq!(end, State::Copying);
    assert_eq!(
        actions,
        [
            vec![notify(NotifyEvent::Failed { class: FailureClass::MediaReadError })],
            vec![Action::EmptyCopyDirectory, Action::CopyDisc, notify(NotifyEvent::Resumed)],
        ]
    );
}

#[test]
fn removing_the_disc_in_the_disc_phase_leaves_the_state_and_waits_for_the_disc() {
    for state in disc_phase_states() {
        assert_eq!(step(&state, &Event::MediaRemoved), (state.clone(), vec![Action::WaitForDisc]));
    }
}

#[test]
fn the_disc_arriving_in_the_disc_phase_resumes_the_state() {
    for state in disc_phase_states() {
        assert_eq!(step(&state, &Event::MediaArrived), resume(&state, true), "{state:?}");
    }
    assert_eq!(
        step(&State::Ripping, &Event::MediaArrived),
        (State::Ripping, vec![Action::EmptyTitleDirectory, Action::RipTitle])
    );
}

#[test]
fn the_disc_leaving_or_arriving_is_rejected_after_the_disc_phase() {
    let mut states = file_phase_states();
    states.extend([State::Filed, State::NeedsReview { held: Held::AfterCopy }, State::Discarded]);
    states.extend(waiting_states(State::Ripping));
    for state in states {
        assert!(rejects(&state, &Event::MediaRemoved), "{state:?}");
        assert!(rejects(&state, &Event::MediaArrived), "{state:?}");
    }
}

// Spec 5's job ID (`job_id`) and spec 6.1 step 2's job lookup (`job_lookup`).

/// A made-up fingerprint in the form `fingerprint::content_hash` returns: 32 upper-case hexadecimal digits (spec 5).
const FP: &str = "0123456789ABCDEF0123456789ABCDEF";

/// An earlier job of FP numbered `n`.
fn earlier(n: u32, state: State, armed: bool) -> (String, State, bool) {
    (format!("{FP}-{n}"), state, armed)
}

#[test]
fn the_first_job_of_a_fingerprint_is_numbered_1() {
    assert_eq!(job_id(FP, &[]), format!("{FP}-1"));
}

#[test]
fn the_next_job_is_one_more_than_the_highest_earlier_number_in_any_order() {
    let ids = [format!("{FP}-1"), format!("{FP}-3"), format!("{FP}-2")];
    let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
    assert_eq!(job_id(FP, &ids), format!("{FP}-4"));
}

#[test]
fn the_highest_number_is_compared_as_a_number_not_as_text() {
    let ids = [format!("{FP}-9"), format!("{FP}-10")];
    let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
    assert_eq!(job_id(FP, &ids), format!("{FP}-11"));
}

#[test]
fn ids_of_other_fingerprints_are_ignored() {
    assert_eq!(job_id("ABC", &["DEF-7", "ABCD-9", "XABC-4"]), "ABC-1");
    assert_eq!(job_id("ABC", &["DEF-7", "ABC-2"]), "ABC-3");
}

#[test]
fn malformed_ids_are_ignored() {
    let malformed = ["ABC", "ABC-", "ABC-x", "ABC-3-2", "ABC3", "ABC-+4", "ABC- 4", "ABC-4 ", "ABC-4294967296"];
    assert_eq!(job_id("ABC", &malformed), "ABC-1");
    assert_eq!(job_id("ABC", &["ABC-3-2", "ABC-5"]), "ABC-6");
}

#[test]
fn a_job_number_may_be_as_high_as_u32_max() {
    assert_eq!(job_id("ABC", &["ABC-4294967295"]), "ABC-4294967296");
}

#[test]
fn without_an_earlier_job_a_new_job_starts() {
    assert_eq!(job_lookup(&[]), JobLookup::NewJob);
}

#[test]
fn a_job_that_has_not_finished_its_disc_phase_resumes() {
    for state in disc_phase_states() {
        for armed in [false, true] {
            assert_eq!(
                job_lookup(&[earlier(1, state.clone(), armed)]),
                JobLookup::Resume(format!("{FP}-1")),
                "{state:?} armed {armed}"
            );
        }
    }
}

#[test]
fn a_job_waiting_to_return_to_its_disc_phase_resumes() {
    for state in waiting_states(State::Copying) {
        assert_eq!(job_lookup(&[earlier(2, state.clone(), false)]), JobLookup::Resume(format!("{FP}-2")), "{state:?}");
    }
}

#[test]
fn a_job_failed_with_a_media_read_error_resumes() {
    for from in [State::Ripping, State::Copying] {
        let failed = State::Failed { class: FailureClass::MediaReadError, from: boxed(from) };
        assert_eq!(job_lookup(&[earlier(1, failed, false)]), JobLookup::Resume(format!("{FP}-1")));
    }
}

#[test]
fn a_job_failed_in_its_disc_phase_with_another_class_starts_a_new_one() {
    for class in FailureClass::ALL {
        if class == FailureClass::MediaReadError {
            continue;
        }
        let failed = State::Failed { class, from: boxed(State::Ripping) };
        assert_eq!(job_lookup(&[earlier(1, failed, false)]), JobLookup::NewJob, "{class:?}");
    }
}

#[test]
fn a_job_failed_in_its_file_phase_is_already_ripped() {
    let failed = State::Failed { class: FailureClass::Unknown, from: boxed(State::Remuxing) };
    assert_eq!(job_lookup(&[earlier(1, failed.clone(), false)]), JobLookup::AlreadyRipped(format!("{FP}-1")));
    assert_eq!(job_lookup(&[earlier(1, failed, true)]), JobLookup::NewJob);
}

#[test]
fn a_media_read_error_in_the_file_phase_is_already_ripped_and_rip_again_starts_a_new_job() {
    // Rip again is accepted for a job that failed in its file phase; once armed, the disc inserted again starts a new
    // rip instead of resuming the failed job (only a disc-phase MediaReadError resumes).
    for from in [State::Extracting, State::Inspecting, State::Remuxing, State::Verified] {
        let failed = State::Failed { class: FailureClass::MediaReadError, from: boxed(from.clone()) };
        assert_eq!(step(&failed, &Event::RipAgain), (failed.clone(), vec![Action::ArmRipAgain]), "{from:?}");
        assert_eq!(job_lookup(&[earlier(1, failed.clone(), true)]), JobLookup::NewJob, "{from:?}");
        assert_eq!(job_lookup(&[earlier(1, failed, false)]), JobLookup::AlreadyRipped(format!("{FP}-1")), "{from:?}");
    }
}

#[test]
fn a_job_past_its_disc_phase_is_already_ripped() {
    let mut states = file_phase_states();
    states.extend([
        State::NeedsReview { held: Held::AfterCopy },
        State::NeedsReview { held: Held::DispositionGate },
        State::Filed,
    ]);
    states.extend(waiting_states(State::Inspecting));
    for state in states {
        assert_eq!(
            job_lookup(&[earlier(3, state.clone(), false)]),
            JobLookup::AlreadyRipped(format!("{FP}-3")),
            "{state:?}"
        );
    }
}

#[test]
fn a_job_past_its_disc_phase_armed_for_rip_again_starts_a_new_one() {
    for state in [State::Filed, State::NeedsReview { held: Held::DispositionGate }, State::Inspecting] {
        assert_eq!(job_lookup(&[earlier(3, state.clone(), true)]), JobLookup::NewJob, "{state:?}");
    }
}

#[test]
fn a_discarded_job_starts_a_new_one() {
    assert_eq!(job_lookup(&[earlier(1, State::Discarded, false)]), JobLookup::NewJob);
    assert_eq!(job_id(FP, &[format!("{FP}-1").as_str()]), format!("{FP}-2"));
}

#[test]
fn the_job_with_the_highest_number_decides_wherever_it_is_listed() {
    let jobs = [
        earlier(10, State::Ripping, false),
        earlier(9, State::Filed, false),
        earlier(2, State::Discarded, false),
    ];
    assert_eq!(job_lookup(&jobs), JobLookup::Resume(format!("{FP}-10")));
    let jobs = [earlier(1, State::Ripping, false), earlier(2, State::Filed, false)];
    assert_eq!(job_lookup(&jobs), JobLookup::AlreadyRipped(format!("{FP}-2")));
    let jobs = [earlier(2, State::Discarded, false), earlier(1, State::Filed, false)];
    assert_eq!(job_lookup(&jobs), JobLookup::NewJob);
}

#[test]
fn a_job_lookup_skips_an_id_that_is_not_numbered() {
    let jobs = [(format!("{FP}-x"), State::Ripping, false), earlier(1, State::Filed, false)];
    assert_eq!(job_lookup(&jobs), JobLookup::AlreadyRipped(format!("{FP}-1")));
    assert_eq!(job_lookup(&[(FP.to_string(), State::Ripping, false)]), JobLookup::NewJob);
}
