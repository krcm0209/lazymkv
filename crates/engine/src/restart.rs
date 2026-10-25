//! Restarts (design part 5.10, L1, L1a): which job takes the disc-phase token, the EngineRestart each job is fed, the
//! discs that are arrivals at start, the file-phase slot, the space promises and each job's lookup book, rebuilt from
//! the persisted states and the job folders.

use std::fs;
use std::path::Path;

use lazymkv_clients::{Answers, Lookup, LookupBook, LookupState, PendingReason};
use lazymkv_core::job::State;
use lazymkv_core::lookup::discdb::{parse_dischash, parse_discid};
use lazymkv_core::lookup::tmdb::{parse_movie, parse_search};
use lazymkv_core::record::{DrivesFile, JobRecord};

use crate::arrival::chain_inputs;
use crate::space::PromisedStep;

/// The lookups' status file in a job folder (S21).
const STATUS_FILE: &str = "lookup-status.txt";

/// The reason of a lookup whose line said `000`: its request was out, or not answered, when the engine stopped (S73).
const NO_ANSWER: &str = "no answer before the restart";

/// The job.log line a Decide writes when it read its inputs back (design part 5.10). Def: T25. Use: T25.
pub(crate) const READ_BACK_LINE: &str =
    "decide inputs read back from the job folder with lazymkv-dev's readers (design part 5.10)";

/// What the engine does at start (design part 5.10, L1, L1a): which job takes the disc-phase token, the
/// EngineRestart each job is fed, the discs that are arrivals and the slot. Def: T25. Use: T25, T26.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct RestartPlan {
    /// The oldest disc-phase job with its disc in a drive (part 5.4 way (c)).
    pub token: Option<String>,
    /// (job id, disc_present), oldest job first.
    pub feeds: Vec<(String, bool)>,
    /// Drive ids whose disc is an arrival (L1), in drive-id order.
    pub arrivals: Vec<String>,
    /// Drive ids whose disc only has its presence noted (the token job's; L1a).
    pub noted: Vec<String>,
    /// Jobs that hold the file-phase slot (design part 5.7).
    pub slot_holders: Vec<String>,
    /// Ejected { after_copy: false } jobs, oldest first.
    pub slot_queue: Vec<String>,
}

/// The restart of design part 5.10 over the loaded jobs, the discs in the drives and drives.json. The jobs go oldest
/// first, by (`created_unix`, `id`), and Filed, Discarded, NeedsReview and Failed jobs are not fed. The token goes to
/// the first disc-phase job whose fingerprint a present disc carries, which is fed `disc_present` true; every other
/// disc-phase job is fed false and waits for its disc. Every other fed job is fed true exactly when a present disc
/// carries its fingerprint. In drive-id order, the token job's drive (the first holding its fingerprint) and a drive
/// whose drives.json record has the same fingerprint with `handled` true are noted (S71, L1a); every other drive is an
/// arrival (L1). The slot follows Ruling S72: a job in Inspecting, Decided, Remuxing or Verified, or waiting to return
/// to one of them, holds it; an `Ejected { after_copy: false }` job queues for it.
///
/// `present`: each drive with a disc, and its fingerprint (None when the walk failed). Def: T25. Use: T25.
pub(crate) fn plan_restart(
    jobs: &[JobRecord],
    present: &[(String, Option<String>)],
    drives: &DrivesFile,
) -> RestartPlan {
    let mut fed: Vec<&JobRecord> = jobs.iter().filter(|job| is_fed(&job.state)).collect();
    fed.sort_by(|a, b| (a.created_unix, &a.id).cmp(&(b.created_unix, &b.id)));
    let mut present: Vec<&(String, Option<String>)> = present.iter().collect();
    present.sort_by(|a, b| a.0.cmp(&b.0));
    let drive_of = |fingerprint: &str| {
        present.iter().find(|(_, found)| found.as_deref() == Some(fingerprint)).map(|(drive, _)| drive.clone())
    };
    let token = fed.iter().find(|job| job.state.is_disc_phase() && drive_of(&job.fingerprint).is_some());
    let token_drive = token.and_then(|job| drive_of(&job.fingerprint));
    let token = token.map(|job| job.id.clone());
    let feeds = fed
        .iter()
        .map(|job| {
            let disc_present = if job.state.is_disc_phase() {
                token.as_deref() == Some(job.id.as_str())
            } else {
                drive_of(&job.fingerprint).is_some()
            };
            (job.id.clone(), disc_present)
        })
        .collect();
    let (mut arrivals, mut noted) = (Vec::new(), Vec::new());
    for (drive, fingerprint) in present {
        let handled = drives
            .drives
            .get(drive)
            .is_some_and(|record| record.handled && record.fingerprint == *fingerprint);
        if token_drive.as_deref() == Some(drive.as_str()) || handled {
            noted.push(drive.clone());
        } else {
            arrivals.push(drive.clone());
        }
    }
    let slot_holders = fed.iter().filter(|job| holds_slot(&job.state)).map(|job| job.id.clone()).collect();
    let ejected = State::Ejected { after_copy: false };
    let slot_queue = fed.iter().filter(|job| job.state == ejected).map(|job| job.id.clone()).collect();
    RestartPlan { token, feeds, arrivals, noted, slot_holders, slot_queue }
}

/// Whether a job in `state` is fed EngineRestart: every state but Filed, Discarded, held and failed (design part 5.10).
fn is_fed(state: &State) -> bool {
    !matches!(state, State::Filed | State::Discarded | State::NeedsReview { .. } | State::Failed { .. })
}

/// Ruling S72: Inspecting, Decided, Remuxing and Verified hold the file-phase slot, and so does a waiting state that
/// returns to one of them (design part 5.7: the slot is held through any wait entered after the grant).
fn holds_slot(state: &State) -> bool {
    let granted = |state: &State| {
        matches!(state, State::Inspecting | State::Decided { .. } | State::Remuxing | State::Verified)
    };
    match state {
        State::AwaitingShare { return_to }
        | State::AwaitingSpace { return_to }
        | State::AwaitingMakeMkv { return_to }
        | State::AwaitingNetwork { return_to }
        | State::AwaitingLookup { return_to }
        | State::AwaitingTool { return_to } => granted(return_to),
        other => granted(other),
    }
}

/// The promise a job in `state` holds across a restart: Ripping holds Rip, Remuxing holds Remux, every other
/// state none (design parts 5.8, 5.10). Def: T25. Use: T25.
pub(crate) fn held_promise(state: &State) -> Option<PromisedStep> {
    match state {
        State::Ripping => Some(PromisedStep::Rip),
        State::Remuxing => Some(PromisedStep::Remux),
        _ => None,
    }
}

/// Ruling S73: each line of lookup-status.txt is mapped to a lookup by its first word (`dischash`, `discid`, `search`,
/// and `tmdb` for the record) and to a state by its last word. `200` is `Answered { status: 200 }` with the saved
/// file's parsed answer, or `Pending { Unparsed }` with the parser's sentence when the file does not parse; `404` is
/// `Answered`; `000` is `Pending { Unreachable("no answer before the restart") }`; `unparsed` is `Pending { Unparsed }`
/// with the sentence of reading the file again; any other number is `Pending { Status(n) }`. A line of another shape
/// is skipped. A folder with no status line gives the empty book, as before the first round, so RunScan starts one.
/// Otherwise each lookup of the disc's chain with no line is `NotAsked`, as a round leaves it: TheDiscDB by
/// fingerprint always, by disc ID when meta.json has one, and the search when the disc's name is not generic
/// (`chain_inputs`).
///
/// A job's LookupBook read back from its folder's lookup-status.txt and saved answers (design part 8.3). Def: T25.
/// Use: T25, T26.
pub(crate) fn rebuild_book(job_dir: &Path) -> LookupBook {
    let mut book = LookupBook::default();
    let Ok(text) = fs::read_to_string(job_dir.join(STATUS_FILE)) else {
        return book;
    };
    for line in text.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        let (Some(first), Some(last)) = (words.first(), words.last()) else {
            continue;
        };
        let lookup = match *first {
            "dischash" => Lookup::Dischash,
            "discid" => Lookup::Discid,
            "search" => Lookup::Search,
            "tmdb" => Lookup::Record,
            _ => continue,
        };
        let state = match *last {
            "200" => match read_answer(job_dir, lookup, &mut book.answers) {
                Ok(()) => LookupState::Answered { status: 200 },
                Err(sentence) => LookupState::Pending { reason: PendingReason::Unparsed(sentence) },
            },
            "404" => LookupState::Answered { status: 404 },
            "000" => LookupState::Pending { reason: PendingReason::Unreachable(NO_ANSWER.to_string()) },
            "unparsed" => {
                let again = read_answer(job_dir, lookup, &mut Answers::default());
                let sentence = again.err().unwrap_or_else(|| {
                    let path = answer_path(job_dir, lookup);
                    format!("{}: the answer did not parse before the restart", path.display())
                });
                LookupState::Pending { reason: PendingReason::Unparsed(sentence) }
            }
            other => match other.parse::<u16>() {
                Ok(status) => LookupState::Pending { reason: PendingReason::Status(status) },
                Err(_) => continue,
            },
        };
        book.states.insert(lookup, state);
    }
    if book.states.is_empty() {
        return book;
    }
    let mut chain = vec![Lookup::Dischash];
    if let Ok(inputs) = chain_inputs(job_dir) {
        if inputs.disc_id.is_some() {
            chain.push(Lookup::Discid);
        }
        if inputs.name.is_some_and(|name| !name.generic) {
            chain.push(Lookup::Search);
        }
    }
    for lookup in chain {
        book.states.entry(lookup).or_insert(LookupState::NotAsked);
    }
    book
}

/// The saved answer of `lookup` in a job folder: `thediscdb-hash.json`, `thediscdb-discid.json`, `tmdb-search.json` or
/// `tmdb-record.json` (design part 7.1).
fn answer_path(job_dir: &Path, lookup: Lookup) -> std::path::PathBuf {
    let name = match lookup {
        Lookup::Dischash => "thediscdb-hash.json",
        Lookup::Discid => "thediscdb-discid.json",
        Lookup::Search => "tmdb-search.json",
        Lookup::Record => "tmdb-record.json",
    };
    job_dir.join(name)
}

/// Reads `lookup`'s saved answer with core's parser into `answers` (`parse_movie` for the record: pull request 2 is
/// movie only, S19). Err is the sentence of a file that cannot be read, is not UTF-8 or does not parse.
fn read_answer(job_dir: &Path, lookup: Lookup, answers: &mut Answers) -> Result<(), String> {
    let path = answer_path(job_dir, lookup);
    let shown = path.display().to_string();
    let bytes = fs::read(&path).map_err(|error| format!("{shown}: {error}"))?;
    let text = String::from_utf8(bytes).map_err(|_| format!("{shown}: the body is not UTF-8"))?;
    match lookup {
        Lookup::Dischash => answers.by_fingerprint = parse_dischash(&text, &shown)?,
        Lookup::Discid => answers.by_disc_id = parse_discid(&text, &shown)?,
        Lookup::Search => answers.search = Some(parse_search(&text, &shown)?),
        Lookup::Record => answers.record = Some(parse_movie(&text, &shown)?),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lazymkv_core::job::{FailureClass, Held, State};
    use lazymkv_core::plan::Mode;
    use lazymkv_core::profile::JOB_PROFILE_FILE;
    use lazymkv_core::record::{DriveRecord, DrivesFile, JobRecord, DRIVES_FORMAT, JOB_FORMAT};
    use lazymkv_core::testsupport::{search_json, Scratch, ScratchExt};
    use std::collections::BTreeMap;

    fn record(id: &str, fingerprint: &str, created: u64, state: State) -> JobRecord {
        JobRecord {
            format: JOB_FORMAT.to_string(), id: id.to_string(), fingerprint: fingerprint.to_string(), disc_id: None,
            drive: "tok0".to_string(), created_unix: created, updated_unix: created, state, message: None,
            rip_again: false, profile: JOB_PROFILE_FILE.to_string(), plan: None, titles: vec![], filed_unix: None,
            collisions: vec![], notes: vec![],
        }
    }

    fn drives(entries: &[(&str, Option<&str>, bool)]) -> DrivesFile {
        DrivesFile {
            format: DRIVES_FORMAT.to_string(),
            drives: entries
                .iter()
                .map(|(id, fp, handled)| (id.to_string(), DriveRecord { fingerprint: fp.map(str::to_string), handled: *handled }))
                .collect::<BTreeMap<_, _>>(),
            drive_incapable: vec![],
        }
    }

    fn present(entries: &[(&str, Option<&str>)]) -> Vec<(String, Option<String>)> {
        entries.iter().map(|(d, fp)| (d.to_string(), fp.map(str::to_string))).collect()
    }

    fn back(state: State) -> Box<State> { Box::new(state) }

    #[test]
    fn the_oldest_disc_phase_job_with_its_disc_present_takes_the_token() {
        let jobs = [record("bbb-1", "BBB", 20, State::Scanned), record("aaa-1", "AAA", 10, State::Ripping)];
        let plan = plan_restart(&jobs, &present(&[("tok0", Some("BBB")), ("tok1", Some("AAA"))]), &drives(&[]));
        assert_eq!(plan.token.as_deref(), Some("aaa-1"));
        assert_eq!(plan.feeds, vec![("aaa-1".to_string(), true), ("bbb-1".to_string(), false)]);
        assert_eq!(plan.noted, vec!["tok1".to_string()]);
        assert_eq!(plan.arrivals, vec!["tok0".to_string()]);
    }

    #[test]
    fn other_disc_phase_jobs_wait_for_their_disc() {
        let jobs = [record("aaa-1", "AAA", 10, State::Ripping), record("ccc-1", "CCC", 30, State::Planned { mode: Mode::Direct })];
        let plan = plan_restart(&jobs, &present(&[("tok0", Some("AAA"))]), &drives(&[]));
        assert_eq!(plan.feeds, vec![("aaa-1".to_string(), true), ("ccc-1".to_string(), false)]);
    }

    #[test]
    fn a_disc_phase_job_without_its_disc_leaves_the_token_to_the_next_oldest() {
        let jobs = [record("aaa-1", "AAA", 10, State::Ripping), record("bbb-1", "BBB", 20, State::Fingerprinted)];
        let plan = plan_restart(&jobs, &present(&[("tok0", Some("BBB"))]), &drives(&[]));
        assert_eq!(plan.token.as_deref(), Some("bbb-1"));
        assert_eq!(plan.feeds, vec![("aaa-1".to_string(), false), ("bbb-1".to_string(), true)]);
        assert!(plan.arrivals.is_empty());
    }

    #[test]
    fn filed_discarded_held_and_failed_jobs_are_not_fed() {
        let jobs = [
            record("aaa-1", "AAA", 10, State::Filed),
            record("bbb-1", "BBB", 20, State::Discarded),
            record("ccc-1", "CCC", 30, State::NeedsReview { held: Held::DispositionGate }),
            record("ddd-1", "DDD", 40, State::Failed { class: FailureClass::Unknown, from: back(State::Ripping) }),
        ];
        let plan = plan_restart(&jobs, &present(&[]), &drives(&[]));
        assert!(plan.feeds.is_empty());
        assert_eq!(plan.token, None);
    }

    #[test]
    fn a_waiting_jobs_disc_present_says_whether_a_drive_holds_its_fingerprint() {
        let jobs = [
            record("aaa-1", "AAA", 10, State::AwaitingSpace { return_to: back(State::Planned { mode: Mode::Direct }) }),
            record("bbb-1", "BBB", 20, State::AwaitingLookup { return_to: back(State::Ejected { after_copy: false }) }),
        ];
        let plan = plan_restart(&jobs, &present(&[("tok0", Some("AAA"))]), &drives(&[("tok0", Some("AAA"), true)]));
        assert_eq!(plan.feeds, vec![("aaa-1".to_string(), true), ("bbb-1".to_string(), false)]);
        assert_eq!(plan.token, None);
        assert_eq!(plan.noted, vec!["tok0".to_string()]);
    }

    #[test]
    fn a_disc_already_in_a_drive_is_an_arrival_at_start() {
        let plan = plan_restart(&[], &present(&[("tok1", Some("CCC")), ("tok0", None)]), &drives(&[]));
        assert_eq!(plan.arrivals, vec!["tok0".to_string(), "tok1".to_string()]);
    }

    #[test]
    fn a_handled_drive_record_is_not_an_arrival_again() {
        let plan = plan_restart(&[], &present(&[("tok0", Some("CCC"))]), &drives(&[("tok0", Some("CCC"), true)]));
        assert!(plan.arrivals.is_empty());
        assert_eq!(plan.noted, vec!["tok0".to_string()]);
    }

    #[test]
    fn a_queued_disc_or_another_disc_in_a_recorded_drive_is_an_arrival_again() {
        let queued = plan_restart(&[], &present(&[("tok0", Some("CCC"))]), &drives(&[("tok0", Some("CCC"), false)]));
        assert_eq!(queued.arrivals, vec!["tok0".to_string()]);
        let other = plan_restart(&[], &present(&[("tok0", Some("DDD"))]), &drives(&[("tok0", Some("CCC"), true)]));
        assert_eq!(other.arrivals, vec!["tok0".to_string()]);
    }

    #[test]
    fn the_file_phase_slot_is_rebuilt_from_persisted_states() {
        let jobs = [
            record("aaa-1", "AAA", 10, State::Remuxing),
            record("bbb-1", "BBB", 20, State::Ejected { after_copy: false }),
            record("ccc-1", "CCC", 5, State::Ejected { after_copy: false }),
            record("ddd-1", "DDD", 40, State::AwaitingLookup { return_to: back(State::Ejected { after_copy: false }) }),
            record("eee-1", "EEE", 50, State::AwaitingSpace { return_to: back(State::Decided { approved: None }) }),
        ];
        let plan = plan_restart(&jobs, &present(&[]), &drives(&[]));
        assert_eq!(plan.slot_holders, vec!["aaa-1".to_string(), "eee-1".to_string()]);
        assert_eq!(plan.slot_queue, vec!["ccc-1".to_string(), "bbb-1".to_string()]);
    }

    #[test]
    fn space_promises_are_rebuilt_from_persisted_states() {
        assert_eq!(held_promise(&State::Ripping), Some(PromisedStep::Rip));
        assert_eq!(held_promise(&State::Remuxing), Some(PromisedStep::Remux));
        for state in [
            State::Planned { mode: Mode::Direct }, State::RipVerified { after_copy: false }, State::Ejected { after_copy: false },
            State::Verified, State::AwaitingSpace { return_to: back(State::Planned { mode: Mode::Direct }) },
        ] {
            assert_eq!(held_promise(&state), None, "{}", state.as_str());
        }
    }

    #[test]
    fn a_lookup_book_is_rebuilt_from_lookup_status_and_the_saved_files() {
        let s = Scratch::new("book");
        s.write("lookup-status.txt", "dischash 404\ndiscid 000\nsearch 200\ntmdb movie/900001 503\n");
        s.write("thediscdb-hash.json", "");
        s.write("tmdb-search.json", search_json(&[("movie", 900001, "Synthetic Movie", "Synthetic Movie", "2001-02-03")]));
        let book = rebuild_book(&s.0);
        assert!(matches!(book.states[&Lookup::Dischash], LookupState::Answered { status: 404 }));
        assert!(matches!(book.states[&Lookup::Discid], LookupState::Pending { reason: PendingReason::Unreachable(_) }));
        assert!(matches!(book.states[&Lookup::Search], LookupState::Answered { status: 200 }));
        assert!(matches!(book.states[&Lookup::Record], LookupState::Pending { reason: PendingReason::Status(503) }));
        assert_eq!(book.answers.search.as_ref().unwrap().results.len(), 1);
        assert!(book.answers.by_fingerprint.is_none());
    }

    #[test]
    fn a_200_whose_saved_file_does_not_parse_is_pending_and_no_line_is_not_asked() {
        let s = Scratch::new("book-unparsed");
        s.write("lookup-status.txt", "search 200\n");
        s.write("tmdb-search.json", "{not json");
        let book = rebuild_book(&s.0);
        assert!(matches!(book.states[&Lookup::Search], LookupState::Pending { reason: PendingReason::Unparsed(_) }));
        assert!(book.states.get(&Lookup::Dischash).map_or(true, |state| matches!(state, LookupState::NotAsked)));
    }
}
