//! The cleanup task (design part 5.9, S40, P4, R-B4, L11; Rulings T4-R11, FR-1): one pass over every job the owner
//! holds, run on a thread of its own. It deletes the TMDb copies past TMDb's six-month limit in every job's folder,
//! whatever the job's state, and the staging folder of each filed job past its retention. It never touches
//! `tuning_root`, never touches anything under `jobs/` but the three TMDb copies, and never follows a link out of a
//! staging folder.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, TryLockError};

use lazymkv_clients::LookupBook;
use lazymkv_core::job::State;
use lazymkv_core::lookup::stale::{add_months, fetched_at, is_stale, TMDB_COPIES};
use lazymkv_core::record::JobRecord;
use lazymkv_core::review::Note;
use lazymkv_platform::{Reach, ShareConnector};

/// The seconds of one day of `retention_days`.
const DAY_SECONDS: u64 = 86_400;

/// One job as the owner thread hands it to a pass (design part 5.9; Ruling T4-R11): its record, its own settings'
/// staging root, keep_tuning_evidence and retention_days (ENG-19), and its lookup book, which a round holds for its
/// whole length (R23.1). Def: T4-11. Use: T4-11.
pub(crate) struct CleanupJob {
    /// The job's id, the name of its folder under `jobs/` and under the staging root.
    pub(crate) id: String,
    /// The job's record as the owner holds it.
    pub(crate) record: JobRecord,
    /// The staging root of the job's own settings.
    pub(crate) staging_root: Option<String>,
    /// The keep_tuning_evidence of the job's own settings (Owner item 22).
    pub(crate) keep_tuning_evidence: bool,
    /// The retention_days of the job's own settings.
    pub(crate) retention_days: u32,
    /// The job's lookup book; a pass skips the job while another holder has it locked.
    pub(crate) lookups: Arc<Mutex<LookupBook>>,
}

/// Whether `id` is shaped like a job ID (K3; design part 5.9): 32 lower-case hexadecimal digits, a dash, and a decimal
/// number of 1 or more without a leading zero. A pass leaves any other name alone. Def: T4-11. Use: T4-11.
pub(crate) fn job_id_shaped(id: &str) -> bool {
    let Some((fingerprint, number)) = id.split_once('-') else {
        return false;
    };
    fingerprint.len() == 32
        && fingerprint.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
        && !number.is_empty()
        && number.bytes().all(|byte| byte.is_ascii_digit())
        && !number.starts_with('0')
}

/// Whether the staging folder of `job` is due for deletion at `now_unix` (design part 5.9, S40; Owner item 22): the job
/// is Filed with a `filed_unix`, `now_unix` is later than that plus its `retention_days`, its own settings have
/// keep_tuning_evidence off, and its record holds no TuningEvidencePending note. Def: T4-11. Use: T4-11.
pub(crate) fn retention_due(job: &CleanupJob, now_unix: u64) -> bool {
    let record = &job.record;
    let Some(filed) = record.filed_unix else {
        return false;
    };
    record.state == State::Filed
        && now_unix > filed.saturating_add(u64::from(job.retention_days) * DAY_SECONDS)
        && !job.keep_tuning_evidence
        && !record.notes.iter().any(|noted| noted.note == Note::TuningEvidencePending)
}

/// One cleanup pass over `jobs`, in the order given (design part 5.9, S40, R-B4; Rulings T4-R11, FR-1). Before each
/// job the pass reads `stop`, the engine's stop flag, and ends when it is raised, so a stop waits on at most one share
/// call. For each job: a name not shaped like a job ID is left alone (`job_id_shaped`); a job whose lookup book
/// another holder has locked is skipped this pass, and the book stays locked by the pass to the job's end; each TMDb
/// copy in `<data>/jobs/<id>/` whose `fetched_at` is a real date past the six-month limit on `today` is deleted (a copy
/// without one is left to the refetch, Ruling T4-R7); and when `retention_due` holds, the job's staging folder is
/// deleted once its share answers, without following any link inside it. The share of each staging root is asked
/// once per pass and its answer kept for the pass's other jobs. Every deletion and failure gives `log` one engine.log
/// line, and each root not reached gives one line at the pass's end with the count of jobs it skipped.
/// Def: T4-11. Use: T4-11.
pub(crate) fn run_pass(
    jobs: Vec<CleanupJob>,
    data: &Path,
    shares: &dyn ShareConnector,
    today: &str,
    now_unix: u64,
    stop: &AtomicBool,
    log: &dyn Fn(String),
) {
    let mut roots = Vec::new();
    for job in jobs {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        let id = job.id.as_str();
        if !job_id_shaped(id) {
            log(format!("cleanup: {id}: left alone: the name is not shaped like a job ID"));
            continue;
        }
        // A round or a refetch holds the book: the job waits for the next pass. A poisoned book is taken as it stands.
        let _book = match job.lookups.try_lock() {
            Ok(guard) => guard,
            Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
            Err(TryLockError::WouldBlock) => continue,
        };
        delete_stale_copies(&data.join("jobs").join(id), id, today, log);
        if !retention_due(&job, now_unix) {
            continue;
        }
        if let Some(root) = &job.staging_root {
            if reached(&mut roots, root, shares) {
                delete_staging_folder(root, id, log);
            }
        }
    }
    for RootReach { root, failure, skipped } in roots {
        if let Some(why) = failure {
            log(format!("cleanup: the staging root {root} was not reached: {why}; {skipped} job(s) skipped"));
        }
    }
}

/// One staging root as a pass found it (Ruling FR-1): the failure text when its share did not answer, and how many
/// jobs it skipped for that.
struct RootReach {
    root: String,
    failure: Option<String>,
    skipped: usize,
}

/// Whether the share of `root` answered this pass: asked on the root's first job only, its answer kept in `roots`,
/// and a job under a root that did not answer counted as skipped.
fn reached(roots: &mut Vec<RootReach>, root: &str, shares: &dyn ShareConnector) -> bool {
    let index = match roots.iter().position(|known| known.root == root) {
        Some(index) => index,
        None => {
            let failure = match shares.reach(root) {
                Reach::Unreachable(why) => Some(why),
                Reach::Reachable => None,
            };
            roots.push(RootReach { root: root.to_string(), failure, skipped: 0 });
            roots.len() - 1
        }
    };
    let known = &mut roots[index];
    if known.failure.is_some() {
        known.skipped += 1;
    }
    known.failure.is_none()
}

/// Deletes each TMDb copy in `folder` whose `fetched_at` is a real date past the six-month limit on `today`.
fn delete_stale_copies(folder: &Path, id: &str, today: &str, log: &dyn Fn(String)) {
    for file in TMDB_COPIES {
        let path = folder.join(file);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(error) => {
                log(format!("cleanup: {id}: {file} could not be read: {error}"));
                continue;
            }
        };
        let Some(date) = fetched_at(&String::from_utf8_lossy(&bytes)) else {
            continue;
        };
        if add_months(&date, 0).is_none() || !is_stale(Some(&date), today) {
            continue;
        }
        match fs::remove_file(&path) {
            Ok(()) => log(format!("cleanup: {id}: deleted {file} (fetched_at {date})")),
            Err(error) => log(format!("cleanup: {id}: {file} could not be deleted: {error}")),
        }
    }
}

/// Deletes `<root>/<id>`, the share holding `root` having answered this pass. `fs::remove_dir_all` removes a symbolic
/// link or a junction inside the folder without following it.
fn delete_staging_folder(root: &str, id: &str, log: &dyn Fn(String)) {
    let folder = Path::new(root).join(id);
    if matches!(fs::symlink_metadata(&folder), Err(error) if error.kind() == ErrorKind::NotFound) {
        return;
    }
    match fs::remove_dir_all(&folder) {
        Ok(()) => log(format!("cleanup: {id}: deleted its staging folder")),
        Err(error) => log(format!("cleanup: {id}: the staging folder could not be deleted: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lazymkv_core::job::{FailureClass, Held};
    use lazymkv_core::profile::JOB_PROFILE_FILE;
    use lazymkv_core::record::JOB_FORMAT;
    use lazymkv_core::review::Noted;
    use lazymkv_core::testsupport::{Scratch, ScratchExt};
    use lazymkv_platform::fakes::FakeShares;
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    const NOW: u64 = 1_792_800_000;
    const TODAY: &str = "2026-10-23";
    const DAY: u64 = 86_400;

    fn id(n: u64) -> String {
        format!("{n:032x}-1")
    }

    /// A job built as `restart.rs`'s test helper `record` builds one. Its `staging_root` is the scratch `staging`
    /// folder, which `pass` fills in, since the job is built before the scratch is known.
    fn job(n: u64, state: State, filed_unix: Option<u64>) -> CleanupJob {
        let id = id(n);
        let record = JobRecord {
            format: JOB_FORMAT.to_string(), id: id.clone(), fingerprint: format!("{n:032X}"), disc_id: None,
            drive: "tok0".to_string(), created_unix: n, updated_unix: n, state, message: None, rip_again: false,
            profile: JOB_PROFILE_FILE.to_string(), plan: None, titles: vec![], filed_unix, collisions: vec![],
            notes: vec![],
        };
        CleanupJob {
            id, record, staging_root: None, keep_tuning_evidence: false, retention_days: 7,
            lookups: Arc::new(Mutex::new(LookupBook::default())),
        }
    }

    fn staging(folders: &Scratch) -> String {
        folders.0.join("staging").display().to_string()
    }

    /// `run_pass` over `<scratch>/data`, each job's staging root the scratch `staging` folder; the lines `log` got.
    fn pass(folders: &Scratch, jobs: Vec<CleanupJob>, shares: &FakeShares, today: &str, now: u64) -> Vec<String> {
        fs::create_dir_all(folders.0.join("staging")).unwrap();
        let root = staging(folders);
        let jobs = jobs.into_iter().map(|job| CleanupJob { staging_root: Some(root.clone()), ..job }).collect();
        let lines = RefCell::new(Vec::new());
        let stop = AtomicBool::new(false);
        run_pass(jobs, &folders.0.join("data"), shares, today, now, &stop, &|line| lines.borrow_mut().push(line));
        lines.into_inner()
    }

    fn copy(date: &str) -> String {
        format!(r#"{{"id": 1, "fetched_at": "{date}"}}"#)
    }

    /// Every file under `root`, by path, with its bytes.
    fn listing(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
        let mut files = BTreeMap::new();
        let mut folders = vec![root.to_path_buf()];
        while let Some(folder) = folders.pop() {
            for entry in fs::read_dir(&folder).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    folders.push(path);
                } else {
                    let bytes = fs::read(&path).unwrap();
                    files.insert(path, bytes);
                }
            }
        }
        files
    }

    fn failed() -> State {
        State::Failed { class: FailureClass::Unknown, from: Box::new(State::Remuxing) }
    }

    fn held() -> State {
        State::NeedsReview { held: Held::DispositionGate }
    }

    #[test]
    fn retention_is_due_only_for_a_filed_job_older_than_its_days_without_pending_evidence() {
        let mut filed = job(1, State::Filed, Some(NOW - 7 * DAY - 1));
        assert!(retention_due(&filed, NOW));
        filed.record.filed_unix = Some(NOW - 7 * DAY);
        assert!(!retention_due(&filed, NOW), "exactly its retention is not older");
        filed.record.filed_unix = Some(NOW - 7 * DAY - 1);
        filed.keep_tuning_evidence = true;
        assert!(!retention_due(&filed, NOW), "Owner item 22");
        filed.keep_tuning_evidence = false;
        filed.record.notes.push(Noted { note: Note::TuningEvidencePending, stream: None, detail: String::new() });
        assert!(!retention_due(&filed, NOW));
        for state in [held(), failed(), State::Ripping] {
            assert!(!retention_due(&job(2, state, Some(NOW - 30 * DAY)), NOW));
        }
        assert!(!retention_due(&job(3, State::Filed, None), NOW));
    }

    #[test]
    fn a_filed_job_past_retention_has_its_staging_folder_deleted() {
        let folders = Scratch::new("cleanup-retention");
        let id = id(1);
        let state = folders.write_path(&format!("data/jobs/{id}/state.json"), "x");
        let tmdb = folders.write_path(&format!("data/jobs/{id}/tmdb.json"), copy("2026-10-01"));
        folders.write(&format!("staging/{id}/tuning/a.txt"), "a");
        folders.write(&format!("staging/{id}/rip/t00/b.mkv"), "b");
        let lines = pass(&folders, vec![job(1, State::Filed, Some(NOW - 7 * DAY - 1))], &FakeShares::new(), TODAY, NOW);
        assert!(!folders.0.join("staging").join(&id).exists());
        assert!(folders.0.join("staging").is_dir());
        assert_eq!(fs::read(&state).unwrap(), b"x");
        assert_eq!(fs::read(&tmdb).unwrap(), copy("2026-10-01").as_bytes());
        assert_eq!(lines, [format!("cleanup: {id}: deleted its staging folder")]);
    }

    #[test]
    fn a_filed_job_at_exactly_its_retention_keeps_its_staging_folder() {
        let folders = Scratch::new("cleanup-exact");
        let id = id(1);
        folders.write(&format!("data/jobs/{id}/state.json"), "x");
        folders.write(&format!("data/jobs/{id}/tmdb.json"), copy("2026-10-01"));
        folders.write(&format!("staging/{id}/tuning/a.txt"), "a");
        folders.write(&format!("staging/{id}/rip/t00/b.mkv"), "b");
        let lines = pass(&folders, vec![job(1, State::Filed, Some(NOW - 7 * DAY))], &FakeShares::new(), TODAY, NOW);
        assert!(folders.0.join("staging").join(&id).join("rip/t00/b.mkv").is_file());
        assert!(folders.0.join("staging").join(&id).join("tuning/a.txt").is_file());
        assert_eq!(lines, Vec::<String>::new());
    }

    #[test]
    fn the_cleanup_never_touches_tuning_root() {
        let folders = Scratch::new("cleanup-tuning");
        let id = id(1);
        let kept = folders.write_path(&format!("tuning/recordings/{id}/tmdb.json"), copy("2019-01-01"));
        folders.write(&format!("staging/{id}/rip/t00/b.mkv"), "b");
        pass(&folders, vec![job(1, State::Filed, Some(NOW - 7 * DAY - 1))], &FakeShares::new(), TODAY, NOW);
        assert_eq!(fs::read(&kept).unwrap(), copy("2019-01-01").as_bytes());
    }

    #[test]
    fn job_id_shaped_takes_only_a_fingerprint_and_a_number() {
        assert!(job_id_shaped(&id(1)));
        assert!(job_id_shaped(&format!("{:032x}-12", 7)));
        for refused in [
            "..".to_string(),
            "abc-1".to_string(),
            // An upper-case hex digit is not a job ID (id(1) holds no hex letter, so it is upper-cased from 0xab).
            id(0xab).to_ascii_uppercase(),
            format!("{:032x}-0", 1),
            format!("{:032x}-01", 1),
            format!("{:032x}", 1),
            format!("{:031x}-1", 1),
        ] {
            assert!(!job_id_shaped(&refused), "{refused}");
        }
    }

    #[test]
    fn the_cleanup_deletes_tmdb_copies_older_than_six_months_in_every_job_whatever_its_state() {
        let folders = Scratch::new("cleanup-copies");
        let awaiting = State::AwaitingLookup { return_to: Box::new(State::Ejected { after_copy: false }) };
        let jobs =
            vec![job(1, held(), None), job(2, failed(), None), job(3, State::Filed, Some(NOW)), job(4, awaiting, None)];
        for job in &jobs {
            folders.write(&format!("data/jobs/{}/tmdb.json", job.id), copy("2026-04-22"));
            folders.write(&format!("data/jobs/{}/tmdb-record.json", job.id), copy("2026-04-23"));
            folders.write(&format!("data/jobs/{}/tmdb-search.json", job.id), copy("2026-10-01"));
        }
        let ids: Vec<String> = jobs.iter().map(|job| job.id.clone()).collect();
        let lines = pass(&folders, jobs, &FakeShares::new(), TODAY, NOW);
        for id in &ids {
            let dir = folders.0.join("data/jobs").join(id);
            assert!(!dir.join("tmdb.json").exists(), "{id}");
            assert_eq!(fs::read(dir.join("tmdb-record.json")).unwrap(), copy("2026-04-23").as_bytes());
            assert_eq!(fs::read(dir.join("tmdb-search.json")).unwrap(), copy("2026-10-01").as_bytes());
        }
        let expected: Vec<String> =
            ids.iter().map(|id| format!("cleanup: {id}: deleted tmdb.json (fetched_at 2026-04-22)")).collect();
        assert_eq!(lines, expected);
    }

    #[test]
    fn a_clock_years_ahead_deletes_only_tmdb_copies() {
        let folders = Scratch::new("cleanup-years");
        let mut kept = job(1, State::Filed, Some(NOW));
        kept.keep_tuning_evidence = true;
        let jobs = vec![kept, job(2, held(), None), job(3, failed(), None), job(4, State::Ripping, None)];
        for job in &jobs {
            let id = &job.id;
            for file in ["state.json", "job.log", "remembered.json", "t00-00800/decide.json"] {
                folders.write(&format!("data/jobs/{id}/{file}"), format!("{id} {file}"));
            }
            for file in TMDB_COPIES {
                folders.write(&format!("data/jobs/{id}/{file}"), copy("2026-10-01"));
            }
            folders.write(&format!("staging/{id}/rip/t00/b.mkv"), "b");
        }
        let ids: Vec<String> = jobs.iter().map(|job| job.id.clone()).collect();
        let is_copy = |path: &PathBuf| TMDB_COPIES.iter().any(|file| path.file_name().unwrap() == *file);
        let mut before = listing(&folders.0);
        before.retain(|path, _| !is_copy(path));
        let lines = pass(&folders, jobs, &FakeShares::new(), "2031-10-23", NOW + 5 * 365 * DAY);
        let after = listing(&folders.0);
        assert!(!after.keys().any(is_copy), "{:?}", after.keys());
        assert_eq!(after, before);
        for id in &ids {
            assert!(folders.0.join("staging").join(id).is_dir(), "{id}");
        }
        assert_eq!(lines.len(), 12, "{lines:?}");
    }

    #[test]
    fn the_cleanup_never_deletes_a_folder_not_shaped_like_a_job_id() {
        let folders = Scratch::new("cleanup-shape");
        let mut dots = job(1, State::Filed, Some(NOW - 30 * DAY));
        dots.id = "..".to_string();
        let mut upper = job(1, State::Filed, Some(NOW - 30 * DAY));
        // An upper-case hex digit is not a job ID (id(1) holds no hex letter, so it is upper-cased from 0xab).
        upper.id = id(0xab).to_ascii_uppercase();
        let outside = folders.write_path("data/tmdb.json", copy("2019-01-01"));
        let other = folders.write_path("staging/other/x", "x");
        let lines = pass(&folders, vec![dots, upper], &FakeShares::new(), TODAY, NOW);
        assert_eq!(fs::read(&outside).unwrap(), copy("2019-01-01").as_bytes());
        assert_eq!(fs::read(&other).unwrap(), b"x");
        assert!(folders.0.join("staging").is_dir());
        assert_eq!(
            lines,
            [
                "cleanup: ..: left alone: the name is not shaped like a job ID".to_string(),
                format!("cleanup: {}: left alone: the name is not shaped like a job ID", id(0xab).to_ascii_uppercase()),
            ]
        );
    }

    #[test]
    fn the_cleanup_never_follows_a_link_out_of_a_staging_folder() {
        let folders = Scratch::new("cleanup-link");
        let id = id(1);
        let film = folders.write_path("library/film.mkv", "film");
        let library = folders.0.join("library");
        let rip = folders.0.join("staging").join(&id).join("rip");
        fs::create_dir_all(&rip).unwrap();
        let link = rip.join("link");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&library, &link).unwrap();
        #[cfg(windows)]
        {
            let made = std::process::Command::new("cmd").arg("/c").arg("mklink").arg("/J").arg(&link).arg(&library)
                .output().unwrap();
            assert!(made.status.success(), "mklink /J failed");
        }
        pass(&folders, vec![job(1, State::Filed, Some(NOW - 7 * DAY - 1))], &FakeShares::new(), TODAY, NOW);
        assert!(!folders.0.join("staging").join(&id).exists());
        assert_eq!(fs::read(&film).unwrap(), b"film");
    }

    #[test]
    fn a_copy_without_a_readable_fetched_at_is_left_for_the_refetch() {
        let folders = Scratch::new("cleanup-unreadable");
        let id = id(1);
        let tmdb = folders.write_path(&format!("data/jobs/{id}/tmdb.json"), r#"{"id": 1}"#);
        let record = folders.write_path(
            &format!("data/jobs/{id}/tmdb-record.json"),
            r#"{"id": 1, "fetched_at": "2026-13-45"}"#,
        );
        let lines = pass(&folders, vec![job(1, held(), None)], &FakeShares::new(), TODAY, NOW);
        assert_eq!(fs::read(&tmdb).unwrap(), br#"{"id": 1}"#);
        assert_eq!(fs::read(&record).unwrap(), br#"{"id": 1, "fetched_at": "2026-13-45"}"#);
        assert_eq!(lines, Vec::<String>::new());
    }

    #[test]
    fn an_unreached_staging_root_deletes_nothing_and_says_so() {
        let folders = Scratch::new("cleanup-unreached");
        let id = id(1);
        folders.write(&format!("staging/{id}/rip/t00/b.mkv"), "b");
        let shares = FakeShares::new();
        shares.set_reach(&staging(&folders), Reach::Unreachable("the share did not answer".into()));
        let lines = pass(&folders, vec![job(1, State::Filed, Some(NOW - 7 * DAY - 1))], &shares, TODAY, NOW);
        assert!(folders.0.join("staging").join(&id).join("rip/t00/b.mkv").is_file());
        let root = staging(&folders);
        assert_eq!(
            lines,
            [format!("cleanup: the staging root {root} was not reached: the share did not answer; 1 job(s) skipped")]
        );
    }

    #[test]
    fn two_filed_jobs_under_one_unreached_staging_root_reach_it_once_and_log_one_line() {
        let folders = Scratch::new("cleanup-unreached-once");
        let (first, second) = (id(1), id(2));
        folders.write(&format!("staging/{first}/rip/t00/b.mkv"), "b");
        folders.write(&format!("staging/{second}/rip/t00/b.mkv"), "b");
        let root = staging(&folders);
        let shares = FakeShares::new();
        shares.set_reach(&root, Reach::Unreachable("the share did not answer".into()));
        let filed = Some(NOW - 7 * DAY - 1);
        let lines = pass(&folders, vec![job(1, State::Filed, filed), job(2, State::Filed, filed)], &shares, TODAY, NOW);
        assert_eq!(shares.reaches(&root), 1);
        assert!(folders.0.join("staging").join(&first).join("rip/t00/b.mkv").is_file());
        assert!(folders.0.join("staging").join(&second).join("rip/t00/b.mkv").is_file());
        assert_eq!(
            lines,
            [format!("cleanup: the staging root {root} was not reached: the share did not answer; 2 job(s) skipped")]
        );
    }

    #[test]
    fn a_pass_whose_stop_flag_is_raised_after_its_first_job_reaches_no_further_share() {
        let folders = Scratch::new("cleanup-stop");
        let (first, second) = (id(1), id(2));
        folders.write(&format!("staging/{first}/rip/t00/b.mkv"), "b");
        folders.write(&format!("other-staging/{second}/rip/t00/b.mkv"), "b");
        let root = staging(&folders);
        let other = folders.0.join("other-staging").display().to_string();
        let filed = Some(NOW - 7 * DAY - 1);
        let jobs = vec![
            CleanupJob { staging_root: Some(root.clone()), ..job(1, State::Filed, filed) },
            CleanupJob { staging_root: Some(other.clone()), ..job(2, State::Filed, filed) },
        ];
        let shares = FakeShares::new();
        let stop = AtomicBool::new(false);
        let lines = RefCell::new(Vec::new());
        // The engine stops while the first job's line is written.
        run_pass(jobs, &folders.0.join("data"), &shares, TODAY, NOW, &stop, &|line| {
            stop.store(true, Ordering::SeqCst);
            lines.borrow_mut().push(line);
        });
        assert_eq!((shares.reaches(&root), shares.reaches(&other)), (1, 0));
        assert!(!folders.0.join("staging").join(&first).exists());
        assert!(folders.0.join("other-staging").join(&second).join("rip/t00/b.mkv").is_file());
        assert_eq!(lines.into_inner(), [format!("cleanup: {first}: deleted its staging folder")]);
    }

    #[test]
    fn a_job_whose_book_a_round_holds_is_skipped_this_pass() {
        let folders = Scratch::new("cleanup-held-book");
        let id = id(1);
        let tmdb = folders.write_path(&format!("data/jobs/{id}/tmdb.json"), copy("2019-01-01"));
        folders.write(&format!("staging/{id}/rip/t00/b.mkv"), "b");
        let job = job(1, State::Filed, Some(NOW - 7 * DAY - 1));
        let book = Arc::clone(&job.lookups);
        let round = book.lock().unwrap();
        let lines = pass(&folders, vec![job], &FakeShares::new(), TODAY, NOW);
        drop(round);
        assert_eq!(fs::read(&tmdb).unwrap(), copy("2019-01-01").as_bytes());
        assert!(folders.0.join("staging").join(&id).join("rip/t00/b.mkv").is_file());
        assert_eq!(lines, Vec::<String>::new());
    }
}
