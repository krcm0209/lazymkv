//! Restarts (design part 5.10, L1, L1a): which job takes the disc-phase token, the EngineRestart each job is fed, the
//! discs that are arrivals at start, the file-phase slot, the space promises and each job's lookup book, rebuilt from
//! the persisted states and the job folders.

use std::fs;
use std::path::Path;

use lazymkv_clients::{seasons_to_fetch, stale_copies, Answers, Lookup, LookupBook, LookupState, PendingReason};
use lazymkv_core::job::State;
use lazymkv_core::lookup::discdb::{parse_dischash, parse_discid};
use lazymkv_core::lookup::entry_search_title;
use lazymkv_core::lookup::name::fold_name;
use lazymkv_core::lookup::tmdb::{parse_movie, parse_search, parse_tv, TmdbKind};
use lazymkv_core::record::{DrivesFile, JobRecord};

use crate::arrival::chain_inputs;
use crate::file_phase::copy_holds;
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
/// carries its fingerprint. In drive-id order, the token job's drive (the first holding its fingerprint) is noted
/// (S71), and so is a drive whose drives.json record has the same fingerprint with `handled` true (L1a), unless its disc
/// is that of a disc-phase job fed false: that job waits for its disc, so the disc is an arrival whatever the record
/// says, and is routed to it as MediaArrived once the token is free (Ruling R25.c1, design part 5.10). Every other
/// drive is an arrival (L1). The slot follows Ruling S72: a job in Inspecting, Decided, Remuxing or Verified, or waiting to return
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
    let waiting: Vec<&str> = fed
        .iter()
        .filter(|job| job.state.is_disc_phase() && token.as_deref() != Some(job.id.as_str()))
        .map(|job| job.fingerprint.as_str())
        .collect();
    let (mut arrivals, mut noted) = (Vec::new(), Vec::new());
    for (drive, fingerprint) in present {
        let waited_for = fingerprint.as_deref().is_some_and(|found| waiting.contains(&found));
        let handled = drives
            .drives
            .get(drive)
            .is_some_and(|record| record.handled && record.fingerprint == *fingerprint);
        if token_drive.as_deref() == Some(drive.as_str()) || (handled && !waited_for) {
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
/// returns to one of them (design part 5.7: the slot is held through any wait entered after the grant). The restart
/// sequence also seeds the slot by it, so a job failed at the restart holds no place (review round 1, V1, RR1-5).
/// Def: T25. Use: T25, owner.rs.
pub(crate) fn holds_slot(state: &State) -> bool {
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

/// Whether a job in `state` may still run a disc-phase step, so the drive its disc is noted in becomes its record's
/// drive (review round 1, V2): a disc-phase state, or a waiting state that returns to one. The drive listing that picks
/// `makemkvcon`'s `disc:` index, the Eject and the ejecting mark read the record's drive; a file-phase job keeps the
/// drive it ripped in, which its notifications and summary name. Def: PR #6 review (V2). Use: owner.rs.
pub(crate) fn reads_drive(state: &State) -> bool {
    match state {
        State::AwaitingShare { return_to }
        | State::AwaitingSpace { return_to }
        | State::AwaitingMakeMkv { return_to }
        | State::AwaitingNetwork { return_to }
        | State::AwaitingLookup { return_to }
        | State::AwaitingTool { return_to } => return_to.is_disc_phase(),
        other => other.is_disc_phase(),
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
/// `search-tagged`, `search-entry`, and `tmdb` for the record) and to a state by its last word. `200` is
/// `Answered { status: 200 }` with the saved file's parsed answer, or `Pending { Unparsed }` with the parser's sentence
/// when the file does not parse; `404` is `Answered`; `000` is
/// `Pending { Unreachable("no answer before the restart") }`; `unparsed` is `Pending { Unparsed }` with the sentence of
/// reading the file again; any other number is `Pending { Status(n) }`. A line of another shape is skipped. A folder
/// with no status line gives the empty book, as before the first round, so RunScan starts one. Otherwise each lookup of
/// the disc's chain with no line is `NotAsked`, as a round leaves it: TheDiscDB by fingerprint always, by disc ID when
/// meta.json has one, the search when the disc's name is not generic (`chain_inputs`), and the tagged search when that
/// name's tagged and cleaned forms also fold to different texts. A `search-tagged 200` puts tmdb-search.json's page in
/// `answers.tagged` as `search 200` puts it in `answers.search`; a file holding the merged page then sits in both,
/// which merges to itself (J8). Ruling T4-R2: when the status text holds `search-entry 200`, the file is the
/// entry-title search's page (S47) and goes to `answers.entry`, and the `200` of the `search` and `search-tagged` lines
/// sets their states without a page. The entry-title search with no line is `NotAsked` when `entry_search_title` over
/// the book names a title.
///
/// Design part 8.2 item 5 (Ruling T4-R4): a `season-<n>` line is `Lookup::Season(n)`, whose `200` reads no file (its
/// episodes are in tmdb-record.json). A `tmdb` line whose second word starts `tv/` reads tmdb-record.json with
/// `parse_tv`, else with `parse_movie`, and its `200` puts the file's JSON in `record_file`. An answered TV record's
/// seasons of `seasons_to_fetch` with no line are `NotAsked`, so a restart before a season's answer asks it again.
///
/// Ruling T4-R8: the stale TMDb copies of the folder on `today` (`stale_copies` in a job folder) are read once; a
/// `200` line whose lookup's copy is listed (`tmdb.json` or `tmdb-record.json` for the record and its seasons,
/// `tmdb-search.json` for the three searches) is `Pending { Stale(<the first listed copy of it>) }` and its answer is
/// not read, so the next round or refetch asks it again. A `season-<n>` line's `unparsed` sentence is
/// `<job>/tmdb-record.json: season <n>'s answer did not parse before the restart`.
///
/// A job's LookupBook read back from its folder's lookup-status.txt and saved answers (design part 8.3). Def: T25
/// (`today`: T4-9). Use: T25, T26, T4-9.
pub(crate) fn rebuild_book(job_dir: &Path, today: &str) -> LookupBook {
    let mut book = LookupBook::default();
    let Ok(text) = fs::read_to_string(job_dir.join(STATUS_FILE)) else {
        return book;
    };
    let stale = stale_copies(&text, &|file| fs::read(job_dir.join(file)).ok(), true, today);
    let stale_copy = |lookup: Lookup| stale.iter().find(|copy| copy_holds(copy.file, lookup)).map(|copy| copy.file);
    // The saved tmdb-search.json holds the entry-title search's page once that search answered (S47).
    let entry_page = text.lines().any(|line| {
        let words: Vec<&str> = line.split_whitespace().collect();
        words.first() == Some(&"search-entry") && words.last() == Some(&"200")
    });
    for line in text.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        let (Some(first), Some(last)) = (words.first(), words.last()) else {
            continue;
        };
        let lookup = match *first {
            "dischash" => Lookup::Dischash,
            "discid" => Lookup::Discid,
            "search" => Lookup::Search,
            "search-tagged" => Lookup::TaggedSearch,
            "search-entry" => Lookup::EntrySearch,
            "tmdb" => Lookup::Record,
            other => match other.strip_prefix("season-").map(str::parse::<u32>) {
                Some(Ok(n)) => Lookup::Season(n),
                _ => continue,
            },
        };
        let tv = lookup == Lookup::Record && words.get(1).is_some_and(|word| word.starts_with("tv/"));
        if let Some(file) = stale_copy(lookup).filter(|_| *last == "200") {
            book.states.insert(lookup, LookupState::Pending { reason: PendingReason::Stale(file.to_string()) });
            continue;
        }
        let state = match *last {
            "200" if entry_page && matches!(lookup, Lookup::Search | Lookup::TaggedSearch) => {
                LookupState::Answered { status: 200 }
            }
            "200" => match read_answer(job_dir, lookup, tv, &mut book.answers) {
                Ok(read) => {
                    if lookup == Lookup::Record {
                        // The record and record_file come from the one read of tmdb-record.json.
                        book.record_file = read.as_deref().and_then(json_text);
                    }
                    LookupState::Answered { status: 200 }
                }
                Err(sentence) => LookupState::Pending { reason: PendingReason::Unparsed(sentence) },
            },
            "404" => LookupState::Answered { status: 404 },
            "000" => LookupState::Pending { reason: PendingReason::Unreachable(NO_ANSWER.to_string()) },
            "unparsed" => {
                let again = read_answer(job_dir, lookup, tv, &mut Answers::default());
                let sentence = again.err().unwrap_or_else(|| {
                    let path = answer_path(job_dir, lookup);
                    match lookup {
                        Lookup::Season(n) => {
                            format!("{}: season {n}'s answer did not parse before the restart", path.display())
                        }
                        _ => format!("{}: the answer did not parse before the restart", path.display()),
                    }
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
    let inputs = chain_inputs(job_dir).ok();
    if let Some(inputs) = inputs.as_ref() {
        if inputs.disc_id.is_some() {
            chain.push(Lookup::Discid);
        }
        if let Some(name) = inputs.name.as_ref().filter(|name| !name.generic) {
            chain.push(Lookup::Search);
            if fold_name(&name.tagged) != fold_name(&name.cleaned) {
                chain.push(Lookup::TaggedSearch);
            }
        }
    }
    for lookup in chain {
        book.states.entry(lookup).or_insert(LookupState::NotAsked);
    }
    if !book.states.contains_key(&Lookup::EntrySearch) && entry_search_title(&book.lookup_inputs(None)).is_some() {
        book.states.insert(Lookup::EntrySearch, LookupState::NotAsked);
    }
    let answered = matches!(book.states.get(&Lookup::Record), Some(LookupState::Answered { .. }));
    let tv_record = book.answers.record.as_ref().filter(|record| answered && record.identity.kind == TmdbKind::Tv);
    if let Some(record) = tv_record {
        let name = inputs.as_ref().and_then(|inputs| inputs.name.as_ref());
        for n in seasons_to_fetch(record, name) {
            book.states.entry(Lookup::Season(n)).or_insert(LookupState::NotAsked);
        }
    }
    book
}

/// A saved answer's text as JSON (after one byte order mark), or None when it is not JSON.
fn json_text(text: &str) -> Option<serde_json::Value> {
    serde_json::from_str(text.strip_prefix('\u{feff}').unwrap_or(text)).ok()
}

/// The saved answer of `lookup` in a job folder: `thediscdb-hash.json`, `thediscdb-discid.json`, `tmdb-search.json`
/// (for the name searches and the entry-title search) or `tmdb-record.json` (for the record and its seasons; design
/// part 7.1).
fn answer_path(job_dir: &Path, lookup: Lookup) -> std::path::PathBuf {
    let name = match lookup {
        Lookup::Dischash => "thediscdb-hash.json",
        Lookup::Discid => "thediscdb-discid.json",
        Lookup::Search | Lookup::TaggedSearch | Lookup::EntrySearch => "tmdb-search.json",
        Lookup::Record | Lookup::Season(_) => "tmdb-record.json",
    };
    job_dir.join(name)
}

/// Reads `lookup`'s saved answer with core's parser into `answers` (the record with `parse_tv` when `tv`, else with
/// `parse_movie`; a season reads nothing, as its episodes are in the record's file). Ok holds the text read (None for a
/// season). Err is the sentence of a file that cannot be read, is not UTF-8 or does not parse.
fn read_answer(job_dir: &Path, lookup: Lookup, tv: bool, answers: &mut Answers) -> Result<Option<String>, String> {
    if let Lookup::Season(_) = lookup {
        return Ok(None);
    }
    let path = answer_path(job_dir, lookup);
    let shown = path.display().to_string();
    let bytes = fs::read(&path).map_err(|error| format!("{shown}: {error}"))?;
    let text = String::from_utf8(bytes).map_err(|_| format!("{shown}: the body is not UTF-8"))?;
    match lookup {
        Lookup::Dischash => answers.by_fingerprint = parse_dischash(&text, &shown)?,
        Lookup::Discid => answers.by_disc_id = parse_discid(&text, &shown)?,
        Lookup::Search => answers.search = Some(parse_search(&text, &shown)?),
        Lookup::TaggedSearch => answers.tagged = Some(parse_search(&text, &shown)?),
        Lookup::EntrySearch => answers.entry = Some(parse_search(&text, &shown)?),
        Lookup::Record if tv => answers.record = Some(parse_tv(&text, &shown)?),
        Lookup::Record => answers.record = Some(parse_movie(&text, &shown)?),
        Lookup::Season(_) => {}
    }
    Ok(Some(text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lazymkv_core::job::{FailureClass, Held, State};
    use lazymkv_core::plan::Mode;
    use lazymkv_core::profile::JOB_PROFILE_FILE;
    use lazymkv_core::record::{DriveRecord, DrivesFile, JobRecord, DRIVES_FORMAT, JOB_FORMAT};
    use lazymkv_core::lookup::tmdb::{TmdbIdentity, TmdbKind};
    use lazymkv_core::testsupport::{search_json, Scratch, ScratchExt};
    use crate::file_phase::waiting_message;
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

    /// Review round 1 (V2): a disc-phase state, or a wait returning to one, takes the drive its disc is noted in; a
    /// file-phase state, or a wait returning to one, keeps the drive it ripped in.
    #[test]
    fn only_a_job_that_may_still_run_a_disc_phase_step_reads_the_drive() {
        assert!(reads_drive(&State::Ripping));
        assert!(reads_drive(&State::AwaitingMakeMkv { return_to: back(State::Fingerprinted) }));
        assert!(reads_drive(&State::AwaitingShare { return_to: back(State::Planned { mode: Mode::Direct }) }));
        assert!(!reads_drive(&State::Ejected { after_copy: false }));
        assert!(!reads_drive(&State::AwaitingLookup { return_to: back(State::Ejected { after_copy: false }) }));
        assert!(!reads_drive(&State::AwaitingSpace { return_to: back(State::Decided { approved: None }) }));
        assert!(!reads_drive(&State::Failed { class: FailureClass::Unknown, from: back(State::Ripping) }));
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

    /// R25.c1: the younger disc-phase job is fed false, so its disc in another drive is routed as an arrival although
    /// that drive's record says handled; otherwise nothing would ever feed it MediaArrived.
    #[test]
    fn a_disc_phase_job_fed_false_has_its_present_disc_routed_as_an_arrival() {
        let jobs = [record("aaa-1", "AAA", 10, State::Ripping), record("bbb-1", "BBB", 20, State::Ripping)];
        let discs = present(&[("tok0", Some("BBB")), ("tok1", Some("AAA"))]);
        let plan = plan_restart(&jobs, &discs, &drives(&[("tok0", Some("BBB"), true), ("tok1", Some("AAA"), false)]));
        assert_eq!(plan.token.as_deref(), Some("aaa-1"));
        assert_eq!(plan.feeds, vec![("aaa-1".to_string(), true), ("bbb-1".to_string(), false)]);
        assert_eq!(plan.arrivals, vec!["tok0".to_string()]);
        assert_eq!(plan.noted, vec!["tok1".to_string()]);
    }

    /// R25.c2: between its rip check and its remux a job holds the Remux promise the rip check made, on the library's
    /// volume; Ripping also holds Rip on staging; Filed, Failed and waiting jobs hold none.
    #[test]
    fn the_rip_checks_remux_promise_is_rebuilt_until_the_remux_check() {
        use crate::space::{planned_remux_promise, rebuild_promises, rip_promise, volume_of, Promises};
        use lazymkv_core::disc::disc;
        use lazymkv_core::lookup::LookupInputs;
        use lazymkv_core::plan::movie::LibraryFacts;
        use lazymkv_core::plan::{plan as make_plan, PlanInputs};
        use lazymkv_core::robot::parse;
        use lazymkv_core::settings::Settings;
        use lazymkv_platform::fakes::FakeShares;

        let scan = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/scenarios/movie-direct/scan.txt");
        let typed = disc(&parse(&fs::read_to_string(scan).unwrap())).unwrap();
        let settings = Settings { staging_root: Some("/s".into()), movies_root: Some("/m".into()), ..Settings::default() };
        let planned = make_plan(&PlanInputs {
            disc: &typed, fingerprint: Some("AAA"), disc_id: None, lookups: LookupInputs::default(), remembered: None,
            overrides: None, sessions: &[], library: &LibraryFacts::default(), settings: &settings,
        })
        .unwrap();
        let remux = planned_remux_promise(&planned);
        assert!(remux > 0);
        let shares = FakeShares::new();
        let (staging, library) = (volume_of(&shares, "/s"), volume_of(&shares, "/m"));
        let job_dir = Scratch::new("promises");
        let failed = State::Failed { class: FailureClass::Unknown, from: back(State::Inspecting) };
        let waiting = State::AwaitingSpace { return_to: back(State::Decided { approved: None }) };
        for (state, on_staging, on_library) in [
            (State::Ripping, rip_promise(&planned, 0), remux),
            (State::RipVerified { after_copy: false }, 0, remux),
            (State::Ejected { after_copy: false }, 0, remux),
            (State::Inspecting, 0, remux),
            (State::Decided { approved: None }, 0, remux),
            (State::Verified, 0, 0),
            (State::Filed, 0, 0),
            (failed, 0, 0),
            (waiting, 0, 0),
        ] {
            let mut job = record("aaa-1", "AAA", 10, state);
            job.plan = Some(planned.clone());
            let mut promises = Promises::default();
            rebuild_promises(&mut promises, &shares, &job, &settings, &job_dir.0);
            let name = job.state.as_str();
            assert_eq!(promises.promised(&staging, "bbb-1"), on_staging, "{name} on staging");
            assert_eq!(promises.promised(&library, "bbb-1"), on_library, "{name} on the library");
        }
    }

    #[test]
    fn a_lookup_book_is_rebuilt_from_lookup_status_and_the_saved_files() {
        let s = Scratch::new("book");
        s.write("lookup-status.txt", "dischash 404\ndiscid 000\nsearch 200\ntmdb movie/900001 503\n");
        s.write("thediscdb-hash.json", "");
        s.write("tmdb-search.json", search_json(&[("movie", 900001, "Synthetic Movie", "Synthetic Movie", "2001-02-03")]));
        let book = rebuild_book(&s.0, "2026-10-23");
        assert!(matches!(book.states[&Lookup::Dischash], LookupState::Answered { status: 404 }));
        assert!(matches!(book.states[&Lookup::Discid], LookupState::Pending { reason: PendingReason::Unreachable(_) }));
        assert!(matches!(book.states[&Lookup::Search], LookupState::Answered { status: 200 }));
        assert!(matches!(book.states[&Lookup::Record], LookupState::Pending { reason: PendingReason::Status(503) }));
        assert_eq!(book.answers.search.as_ref().unwrap().results.len(), 1);
        assert!(book.answers.by_fingerprint.is_none());
    }

    #[test]
    fn a_200_whose_saved_file_does_not_parse_is_stale_and_fetched_again_and_no_line_is_not_asked() {
        let s = Scratch::new("book-unparsed");
        s.write("lookup-status.txt", "search 200\n");
        s.write("tmdb-search.json", "{not json");
        let book = rebuild_book(&s.0, "2026-10-23");
        assert!(matches!(book.states[&Lookup::Search], LookupState::Pending { reason: PendingReason::Stale(_) }));
        assert!(book.states.get(&Lookup::Dischash).map_or(true, |state| matches!(state, LookupState::NotAsked)));
    }

    #[test]
    fn the_rebuilt_book_reads_the_tagged_search_line() {
        let s = Scratch::new("book-tagged");
        let page = search_json(&[("movie", 900101, "Piranha", "Piranha", "1978-08-03"), ("movie", 900103, "Piranha 3D", "Piranha 3D", "2010-08-20")]);
        s.write("tmdb-search.json", &page);
        s.write("lookup-status.txt", "dischash 404\nsearch 200\nsearch-tagged 200\n");
        let book = rebuild_book(&s.0, "2026-10-23");
        assert!(matches!(book.states[&Lookup::TaggedSearch], LookupState::Answered { status: 200 }));
        assert_eq!(book.answers.search.as_ref().map(|p| p.results.len()), Some(2));
        assert_eq!(book.answers.tagged, book.answers.search);
        s.write("lookup-status.txt", "dischash 404\nsearch 200\nsearch-tagged 503\n");
        let book = rebuild_book(&s.0, "2026-10-23");
        assert!(book.answers.tagged.is_none());
        assert_eq!(waiting_message(&book), "search-tagged: HTTP 503");
    }

    #[test]
    fn a_restart_routes_the_saved_page_by_the_search_entry_line() {
        let s = Scratch::new("book-entry");
        s.write("thediscdb-hash.json", r#"{"media": {"title": "Knowing", "year": 2009, "type": "Movie", "external_ids": {}}, "titles": [{"index": 0}]}"#);
        s.write("tmdb-search.json", &search_json(&[("movie", 13811, "Knowing", "Knowing", "2009-03-19")]));
        s.write("lookup-status.txt", "dischash 200\nsearch 200\nsearch-entry 200\n");
        let book = rebuild_book(&s.0, "2026-10-23");
        assert!(matches!(book.states[&Lookup::EntrySearch], LookupState::Answered { status: 200 }));
        assert!(matches!(book.states[&Lookup::Search], LookupState::Answered { status: 200 }));
        assert_eq!((book.answers.entry.as_ref().map(|p| p.results.len()), book.answers.search.is_none()), (Some(1), true));
        s.write("lookup-status.txt", "dischash 200\nsearch 200\n");
        let book = rebuild_book(&s.0, "2026-10-23");
        assert!(book.answers.entry.is_none() && book.answers.search.is_some());
        assert!(matches!(book.states[&Lookup::EntrySearch], LookupState::NotAsked));
        assert!(book.pending().1);
    }

    #[test]
    fn a_failed_entry_search_leaves_the_name_page_for_the_restart() {
        let s = Scratch::new("book-entry-failed");
        s.write("thediscdb-hash.json", r#"{"media": {"title": "Knowing", "year": 2009, "type": "Movie", "external_ids": {}}, "titles": [{"index": 0}]}"#);
        s.write("tmdb-search.json", &search_json(&[("movie", 13811, "Knowing", "Knowing", "2009-03-19")]));
        s.write("lookup-status.txt", "dischash 200\nsearch 200\nsearch-entry 404\n");
        let book = rebuild_book(&s.0, "2026-10-23");
        assert!(matches!(book.states[&Lookup::Search], LookupState::Answered { status: 200 }));
        assert_eq!(book.answers.search.as_ref().map(|p| p.results.len()), Some(1));
        assert!(matches!(book.states[&Lookup::EntrySearch], LookupState::Answered { status: 404 }));
        assert!(book.answers.entry.is_none());
        assert_eq!(book.pending(), (false, false));
    }

    /// A slim TV record whose season 1 is merged; no meta.json, so the name is None and every listed season, season 0
    /// included, is wanted.
    const SHOW_RECORD: &str = r#"{"id":900002,"name":"Example Show","original_name":"Example Show","first_air_date":"2005-03-24","original_language":"en","kind":"tv","seasons":[{"season_number":0,"episode_count":1},{"season_number":1,"episode_count":2,"episodes":[{"episode_number":1,"runtime":44},{"episode_number":2,"runtime":43}]},{"season_number":2,"episode_count":2}],"fetched_at":"2026-10-23"}"#;

    /// The slim tmdb.json beside SHOW_RECORD, fresh on 2026-10-23, so the record is not stale (Ruling T4-R8).
    const SHOW_TMDB: &str = r#"{"id":900002,"name":"Example Show","original_name":"Example Show","first_air_date":"2005-03-24","original_language":"en","fetched_at":"2026-10-23"}"#;

    #[test]
    fn a_restart_reads_a_tv_record_with_its_merged_seasons() {
        let job = Scratch::new("tv-record");
        job.write("tmdb.json", SHOW_TMDB);
        job.write("lookup-status.txt", "dischash 404\nseason-1 200\nseason-2 000\ntmdb tv/900002 200\n");
        job.write("tmdb-record.json", SHOW_RECORD);
        let book = rebuild_book(&job.0, "2026-10-23");
        let record = book.answers.record.as_ref().unwrap();
        assert_eq!(record.identity, TmdbIdentity { kind: TmdbKind::Tv, id: 900002 });
        assert_eq!(record.episode_runtimes(None), vec![44, 43]);
        assert_eq!(book.states.get(&Lookup::Season(1)), Some(&LookupState::Answered { status: 200 }));
        assert!(matches!(book.states.get(&Lookup::Season(2)), Some(LookupState::Pending { .. })));
        assert_eq!(book.record_file.as_ref().unwrap()["seasons"][1]["episodes"][1]["runtime"], serde_json::json!(43));
        assert_eq!(book.pending(), (false, true));
    }

    #[test]
    fn a_restart_after_a_tv_record_marks_its_unasked_seasons_not_asked() {
        let job = Scratch::new("tv-record-unasked");
        job.write("tmdb.json", SHOW_TMDB);
        job.write("lookup-status.txt", "dischash 404\ntmdb tv/900002 200\n");
        job.write("tmdb-record.json", SHOW_RECORD);
        let book = rebuild_book(&job.0, "2026-10-23");
        assert_eq!(book.states.get(&Lookup::Season(0)), Some(&LookupState::NotAsked));
        assert_eq!(book.states.get(&Lookup::Season(1)), Some(&LookupState::NotAsked));
        assert_eq!(book.states.get(&Lookup::Season(2)), Some(&LookupState::NotAsked));
        assert!(book.pending().1);
    }

    /// A job folder with a meta.json (no disc ID, volume label SYNTHETIC_MOVIE), a status saying the search and movie
    /// 900001 answered 200, a slim tmdb-search.json and tmdb.json fetched 2026-10-01, and a slim tmdb-record.json
    /// fetched 2026-01-01.
    fn stale_job_folder() -> Scratch {
        let s = Scratch::new("stale-job-folder");
        s.write("meta.json", r#"{"fingerprint": "ABCDEF0123456789ABCDEF0123456789", "disc_id": null, "volume_label": "SYNTHETIC_MOVIE"}"#);
        s.write("lookup-status.txt", "dischash 404\nsearch 200\ntmdb movie/900001 200\n");
        let page = r#"{"results": [{"media_type": "movie", "id": 900001, "title": "Synthetic Movie", "original_title": "Synthetic Movie", "release_date": "2001-02-03"}], "queries": ["Synthetic Movie"], "fetched_at": "2026-10-01"}"#;
        s.write("tmdb-search.json", page);
        let copy = |date: &str| format!(r#"{{"id": 900001, "title": "Synthetic Movie", "original_title": "Synthetic Movie", "release_date": "2001-02-03", "original_language": "en", "kind": "movie", "runtime": 119, "fetched_at": "{date}"}}"#);
        s.write("tmdb.json", copy("2026-10-01"));
        s.write("tmdb-record.json", copy("2026-01-01"));
        s
    }

    #[test]
    fn a_stale_copy_rebuilds_as_pending_stale_and_a_fresh_one_as_answered() {
        // tmdb-record.json fetched_at 2026-01-01, tmdb-search.json fetched_at 2026-10-01, both slim and parseable.
        let book = rebuild_book(&stale_job_folder().0, "2026-10-23");
        assert_eq!(book.states[&Lookup::Record], LookupState::Pending { reason: PendingReason::Stale("tmdb-record.json".into()) });
        assert_eq!(book.answers.record, None);
        assert_eq!(book.states[&Lookup::Search], LookupState::Answered { status: 200 });
    }
}
