//! The engine's persisted records: state.json, transitions.jsonl and drives.json (design parts 7.3, 7.4; RP23).
//! Plain serde types with no I/O (RP19).

use std::collections::BTreeMap;

use serde::Serializer;

use crate::decision::FileDecision;
use crate::job::{Action, Event, State};
use crate::plan::Plan;
use crate::review::{Noted, ReviewReason};
use crate::verify::ReadError;

/// state.json's format (design part 7.4). Def: T6. Use: T21.
pub const JOB_FORMAT: &str = "lazymkv job v1";
/// drives.json's format (design part 7.4). Def: T6. Use: T21, T22.
pub const DRIVES_FORMAT: &str = "lazymkv drives v1";
/// A remembered decision's format (design part 7.4). Def: T6. Use: T21, T23.
pub const REMEMBERED_FORMAT: &str = "lazymkv remembered v1";

/// Ok when `document["format"]` is `expected` or a minor version of it (`<name> v<major>[.<minor>]`, same name and
/// major); Err naming both strings otherwise (design part 7.4: refused, reported, never overwritten). Def: T6. Use: T21.
pub fn check_format(document: &serde_json::Value, expected: &str) -> Result<(), String> {
    let Some(found) = document.get("format").and_then(serde_json::Value::as_str) else {
        return Err(format!("format is missing; expected \"{expected}\""));
    };
    let refused = || Err(format!("format \"{found}\" is not \"{expected}\" or a minor version of it"));
    let Some((name, major)) = expected.rsplit_once(" v") else {
        return if found == expected { Ok(()) } else { refused() };
    };
    let Some(version) = found.strip_prefix(name).and_then(|rest| rest.strip_prefix(" v")) else {
        return refused();
    };
    let same_major = version == major;
    let minor_of_it = version
        .strip_prefix(major)
        .and_then(|rest| rest.strip_prefix('.'))
        .is_some_and(|minor| !minor.is_empty() && minor.bytes().all(|b| b.is_ascii_digit()));
    if same_major || minor_of_it {
        Ok(())
    } else {
        refused()
    }
}

/// state.json (design part 7.3; S1). No deny_unknown_fields, so a minor version's added key reads.
/// `titles` serialise in key order and `notes` by note (RP23). Field order is the file's key order.
/// Def: T6. Use: T7, T21, T22, T25.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct JobRecord {
    /// `lazymkv job v1` (design part 7.4).
    pub format: String,
    /// The job's ID (K3: lower case).
    pub id: String,
    /// The disc's fingerprint.
    pub fingerprint: String,
    /// The disc's Disc ID, once known.
    pub disc_id: Option<String>,
    /// The drive's ID the job runs on.
    pub drive: String,
    /// When the job was created, in Unix seconds.
    pub created_unix: u64,
    /// When the record was last written, in Unix seconds.
    pub updated_unix: u64,
    /// The job's state.
    pub state: State,
    /// The message of the state, when it has one.
    pub message: Option<String>,
    /// Whether Rip again was armed.
    pub rip_again: bool,
    /// The profile file the job rips with.
    pub profile: String,
    /// The plan, once planned.
    pub plan: Option<Plan>,
    /// The planned titles' progress, serialised in key order.
    #[serde(serialize_with = "sorted_titles")]
    pub titles: Vec<TitleRecord>,
    /// When the job was filed, in Unix seconds.
    pub filed_unix: Option<u64>,
    /// The collisions found at filing.
    pub collisions: Vec<String>,
    /// The job's notes, serialised sorted by note.
    #[serde(serialize_with = "sorted_notes")]
    pub notes: Vec<Noted>,
}

/// `titles` sorted by key (RP23), so that every writer's output is in the sorted form.
fn sorted_titles<S: Serializer>(titles: &[TitleRecord], serializer: S) -> Result<S::Ok, S::Error> {
    let mut sorted: Vec<&TitleRecord> = titles.iter().collect();
    sorted.sort_by(|a, b| a.key.cmp(&b.key));
    serializer.collect_seq(sorted)
}

/// `notes` sorted by `(note, stream, detail)`, `Note`'s own order first (RP23), so that every writer's output is in the
/// sorted form.
fn sorted_notes<S: Serializer>(notes: &[Noted], serializer: S) -> Result<S::Ok, S::Error> {
    let mut sorted: Vec<&Noted> = notes.iter().collect();
    sorted.sort_by(|a, b| (a.note, a.stream, &a.detail).cmp(&(b.note, b.stream, &b.detail)));
    serializer.collect_seq(sorted)
}

/// One planned title's progress in state.json (design part 7.3). Def: T6. Use: T23, T24.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TitleRecord {
    /// The plan's title key.
    pub key: String,
    /// The scan's title number.
    pub scan_title: u32,
    /// The ripped file's index, once ripped.
    pub rip_index: Option<u32>,
    /// How many times the title was ripped.
    pub attempts: u32,
    /// Whether the ripped file passed verification.
    pub verified: bool,
    /// The rip log's read errors.
    pub read_errors: Vec<ReadError>,
    /// The review reasons the rip's verification added.
    pub rip_reviews: Vec<ReviewReason>,
    /// The title's file-phase decision, once decided.
    pub decision: Option<FileDecision>,
    /// The filed file, once filed.
    pub filed: Option<FiledFile>,
}

/// A filed file's exact path, size and modification time (design part 7.3). Def: T6. Use: T24.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FiledFile {
    /// The file's exact path.
    pub path: String,
    /// The file's size in bytes.
    pub size: u64,
    /// The file's modification time, in Unix seconds.
    pub mtime_unix: u64,
}

/// One transitions.jsonl line (design part 7.3). Def: T6. Use: T21.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Transition {
    /// When the transition was taken, in Unix seconds.
    pub time_unix: u64,
    /// The event that caused it; None for a job's first line.
    pub event: Option<Event>,
    /// The state before; None for a job's first line.
    pub old: Option<State>,
    /// The state after.
    pub new: State,
    /// The actions the transition asked for.
    pub actions: Vec<Action>,
}

/// drives.json (design part 7.4, L1a; S3). No deny_unknown_fields, so a minor version's added key reads.
/// Def: T6. Use: T21, T22, T25.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DrivesFile {
    /// `lazymkv drives v1` (design part 7.4).
    pub format: String,
    /// Each drive's record, keyed by the drive's ID.
    pub drives: BTreeMap<String, DriveRecord>,
    /// The DriveIncapable failures seen so far.
    pub drive_incapable: Vec<DriveIncapableEntry>,
}

/// One drive's last disc and whether its arrival was handled (design part 7.4, L1a). Def: T6. Use: T22.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DriveRecord {
    /// The fingerprint of the drive's last disc.
    pub fingerprint: Option<String>,
    /// Whether that disc's arrival was handled.
    pub handled: bool,
}

/// One DriveIncapable failure (design part 7.4, spec 10.2). Def: T6. Use: phase 5.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DriveIncapableEntry {
    /// The drive's name without its serial (`drive_name_without_serial`).
    pub drive_name: String,
    /// The disc's format ("bluray", "dvd"), when known.
    pub format: Option<String>,
    /// The job that failed.
    pub job_id: String,
    /// When it failed, in Unix seconds.
    pub time_unix: u64,
    /// The failure's message.
    pub message: String,
}

/// A DRV line's field 4 cut at its last ASCII space (the serial dropped), the rest byte for byte; the whole field when
/// it has no space; "" when the line has no field 4 (spec 3, design part 7.4). Def: T6. Use: phase 5.
pub fn drive_name_without_serial(drv: &[String]) -> String {
    let Some(field) = drv.get(4) else {
        return String::new();
    };
    match field.rfind(' ') {
        Some(space) => field[..space].to_string(),
        None => field.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::{Action, Event, FailureClass, State};
    use crate::review::{Note, Noted};
    use serde_json::json;
    use std::collections::BTreeMap;

    fn job() -> JobRecord {
        JobRecord {
            format: JOB_FORMAT.to_string(),
            id: "abc-1".to_string(),
            fingerprint: "ABC".to_string(),
            disc_id: None,
            drive: "tok0".to_string(),
            created_unix: 1,
            updated_unix: 2,
            state: State::Detected,
            message: None,
            rip_again: false,
            profile: "job.mmcp.xml".to_string(),
            plan: None,
            titles: vec![],
            filed_unix: None,
            collisions: vec![],
            notes: vec![],
        }
    }

    fn title(key: &str) -> TitleRecord {
        TitleRecord {
            key: key.to_string(),
            scan_title: 0,
            rip_index: None,
            attempts: 1,
            verified: true,
            read_errors: vec![],
            rip_reviews: vec![],
            decision: None,
            filed: None,
        }
    }

    fn noted(note: Note) -> Noted {
        Noted { note, stream: None, detail: "d".to_string() }
    }

    #[test]
    fn a_job_record_round_trips_with_its_format_first() {
        let mut record = job();
        record.titles = vec![title("t00-00800")];
        record.titles[0].filed = Some(FiledFile { path: "/lib/a.mkv".to_string(), size: 16, mtime_unix: 8 });
        record.filed_unix = Some(9);
        let text = serde_json::to_string(&record).unwrap();
        assert!(text.starts_with(r#"{"format":"lazymkv job v1","id":"abc-1","#), "{text}");
        assert!(text.contains(r#""filed":{"path":"/lib/a.mkv","size":16,"mtime_unix":8}"#), "{text}");
        assert!(text.contains(r#""filed_unix":9"#), "{text}");
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(check_format(&value, JOB_FORMAT), Ok(()));
        assert_eq!(serde_json::from_value::<JobRecord>(value).unwrap(), record);
    }

    #[test]
    fn a_record_with_no_format_is_refused() {
        let refused = r#"format is missing; expected "lazymkv job v1""#.to_string();
        assert_eq!(check_format(&json!({"id": "abc-1"}), JOB_FORMAT), Err(refused.clone()));
        assert_eq!(check_format(&json!({"format": 1}), JOB_FORMAT), Err(refused));
    }

    #[test]
    fn a_record_of_another_major_version_is_refused() {
        assert_eq!(
            check_format(&json!({"format": "lazymkv job v2"}), JOB_FORMAT),
            Err(r#"format "lazymkv job v2" is not "lazymkv job v1" or a minor version of it"#.to_string())
        );
        for found in ["lazymkv job v10", "lazymkv job v1.", "lazymkv job v1x", "lazymkv job v1.1.1", "lazymkv drives v1"] {
            assert!(check_format(&json!({ "format": found }), JOB_FORMAT).is_err(), "{found}");
        }
    }

    #[test]
    fn a_minor_version_is_accepted() {
        assert_eq!(check_format(&json!({"format": "lazymkv job v1.1"}), JOB_FORMAT), Ok(()));
        assert_eq!(check_format(&json!({"format": "lazymkv drives v1"}), DRIVES_FORMAT), Ok(()));
        assert_eq!(check_format(&json!({"format": "lazymkv remembered v1.12"}), REMEMBERED_FORMAT), Ok(()));
    }

    #[test]
    fn a_minor_versions_added_key_is_ignored_on_read() {
        let mut value = serde_json::to_value(job()).unwrap();
        value["added_in_v1_1"] = json!(true);
        assert_eq!(serde_json::from_value::<JobRecord>(value).unwrap(), job());
    }

    #[test]
    fn a_failed_state_keeps_its_class_and_from() {
        let mut record = job();
        record.state = State::Failed { class: FailureClass::Unknown, from: Box::new(State::Scanned) };
        record.message = Some("needs whole-disc copy mode".to_string());
        let text = serde_json::to_string(&record).unwrap();
        assert!(text.contains(r#""state":{"state":"Failed","class":"Unknown","from":{"state":"Scanned"}}"#), "{text}");
        assert_eq!(serde_json::from_str::<JobRecord>(&text).unwrap(), record);
    }

    #[test]
    fn the_first_transition_has_no_event_and_no_old_state() {
        let first = Transition { time_unix: 7, event: None, old: None, new: State::Detected, actions: vec![Action::Fingerprint] };
        assert_eq!(
            serde_json::to_string(&first).unwrap(),
            r#"{"time_unix":7,"event":null,"old":null,"new":{"state":"Detected"},"actions":[{"action":"Fingerprint"}]}"#
        );
        let second = Transition {
            time_unix: 8,
            event: Some(Event::FingerprintDone),
            old: Some(State::Detected),
            new: State::Fingerprinted,
            actions: vec![Action::RunScan],
        };
        let text = serde_json::to_string(&second).unwrap();
        assert_eq!(
            text,
            r#"{"time_unix":8,"event":{"event":"FingerprintDone"},"old":{"state":"Detected"},"new":{"state":"Fingerprinted"},"actions":[{"action":"RunScan"}]}"#
        );
        assert_eq!(serde_json::from_str::<Transition>(&text).unwrap(), second);
    }

    #[test]
    fn drives_are_keyed_by_drive_id() {
        let file = DrivesFile {
            format: DRIVES_FORMAT.to_string(),
            drives: BTreeMap::from([("D:".to_string(), DriveRecord { fingerprint: None, handled: false })]),
            drive_incapable: vec![],
        };
        let text = serde_json::to_string(&file).unwrap();
        assert_eq!(
            text,
            r#"{"format":"lazymkv drives v1","drives":{"D:":{"fingerprint":null,"handled":false}},"drive_incapable":[]}"#
        );
        assert_eq!(serde_json::from_str::<DrivesFile>(&text).unwrap(), file);
    }

    #[test]
    fn a_drive_incapable_entry_keeps_its_five_keys_in_order() {
        let entry = DriveIncapableEntry {
            drive_name: "BD-RE SYNTHETIC DRIVE 1.00".to_string(),
            format: Some("bluray".to_string()),
            job_id: "abc-1".to_string(),
            time_unix: 7,
            message: "m".to_string(),
        };
        assert_eq!(
            serde_json::to_string(&entry).unwrap(),
            r#"{"drive_name":"BD-RE SYNTHETIC DRIVE 1.00","format":"bluray","job_id":"abc-1","time_unix":7,"message":"m"}"#
        );
    }

    #[test]
    fn a_drive_name_drops_the_serial() {
        let drv = |name: &str| ["0", "2", "999", "12", name, "L", "D:"].map(String::from).to_vec();
        assert_eq!(drive_name_without_serial(&drv("BD-RE PIONEER BD-RW  BDR-XS07U 1.01 SERIAL")), "BD-RE PIONEER BD-RW  BDR-XS07U 1.01");
        assert_eq!(drive_name_without_serial(&drv("DRIVE")), "DRIVE");
        assert_eq!(drive_name_without_serial(&["0".to_string()]), "");
    }

    #[test]
    fn notes_serialise_sorted_by_note() {
        let mut record = job();
        record.notes = vec![noted(Note::LabelRuleUnavailable), noted(Note::TuningEvidencePending)];
        let text = serde_json::to_string(&record).unwrap();
        let tuning = text.find(r#""note":"tuning_evidence_pending""#).unwrap();
        let label = text.find(r#""note":"label_rule_unavailable""#).unwrap();
        assert!(tuning < label, "{text}");
    }

    #[test]
    fn titles_serialise_in_key_order() {
        let mut record = job();
        record.titles = vec![title("t01-00055"), title("t00-00800")];
        let text = serde_json::to_string(&record).unwrap();
        assert!(text.find(r#""key":"t00-00800""#).unwrap() < text.find(r#""key":"t01-00055""#).unwrap(), "{text}");
    }
}
