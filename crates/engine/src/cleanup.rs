//! The cleanup task (design part 5.9, S40, P4, R-B4, L11; Rulings T4-R11, FR-1): one pass over every job the owner
//! holds, run on a thread of its own. It deletes the TMDb copies past TMDb's six-month limit in every job's folder,
//! whatever the job's state, and the staging folder of each filed job past its retention whose dataset copy is not
//! pending (C2). A discarded job's staging folder and its own partial files in its library roots' `.lazymkv-partial`
//! folders go at any pass, whatever its age (phase 3 design, part 6.8). It never touches `tuning_root`, never touches
//! anything under `jobs/` but the three TMDb copies, never touches a library file, and never follows a link out of a
//! staging folder or a partial folder.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, TryLockError};

use lazymkv_clients::LookupBook;
use lazymkv_core::job::State;
use lazymkv_core::lookup::stale::{add_months, fetched_at, is_stale, TMDB_COPIES};
use lazymkv_core::record::{JobRecord, TuningCopy};
use lazymkv_core::settings::configured_root;
use lazymkv_platform::{Reach, ShareConnector};

use crate::file_phase::PARTIAL_FOLDER;

/// The seconds of one day of `retention_days`.
const DAY_SECONDS: u64 = 86_400;

/// One job as the owner thread hands it to a pass (design part 5.9; Ruling T4-R11): its record, its own settings'
/// staging root and retention_days (ENG-19), and its lookup book, which a round holds for its whole length (R23.1).
/// Def: T4-11. Use: T4-11.
pub(crate) struct CleanupJob {
    /// The job's id, the name of its folder under `jobs/` and under the staging root.
    pub(crate) id: String,
    /// The job's record as the owner holds it.
    pub(crate) record: JobRecord,
    /// The staging root of the job's own settings.
    pub(crate) staging_root: Option<String>,
    /// The library roots of the job's own settings, its `movies_root` and `tv_root` that are set, whose
    /// `.lazymkv-partial` folders a discard empties of the job's partial files (phase 3 design, part 6.8). Def: P3-T10.
    /// Use: P3-T15, P3-T16.
    pub(crate) library_roots: Vec<String>,
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

/// Whether the staging folder of `job` is due for deletion at `now_unix` (design part 5.9, S40; C2): the job is Filed
/// with a `filed_unix`, `now_unix` is later than that plus its `retention_days`, and its dataset copy is not pending
/// (C2; T5-R14: the note is not read). Def: T4-11, T5-19. Use: T4-11, T5-19.
pub(crate) fn retention_due(job: &CleanupJob, now_unix: u64) -> bool {
    let record = &job.record;
    let Some(filed) = record.filed_unix else {
        return false;
    };
    record.state == State::Filed
        && now_unix > filed.saturating_add(u64::from(job.retention_days) * DAY_SECONDS)
        && record.tuning_copy != Some(TuningCopy::Pending)
}

/// Whether `job`'s staging data and partial files are due for deletion (phase 3 design, part 6.8): its record is
/// Discarded, at any pass and whatever its age or its dataset copy. Def: P3-T10. Use: P3-T15, P3-T16.
pub(crate) fn discard_due(job: &CleanupJob) -> bool {
    job.record.state == State::Discarded
}

/// The roots a pass may use for `job` (phase 3 design, part 6.8; fix round 1): its staging root and its library roots,
/// each through `configured_root`, so an absent, blank or whitespace root and `/` are none and nothing is ever asked or
/// deleted under them. Def: P3-T10. Use: P3-T15, P3-T16.
pub(crate) fn pass_roots(job: &CleanupJob) -> (Option<&str>, Vec<&str>) {
    let staging = configured_root(job.staging_root.as_deref());
    (staging, job.library_roots.iter().filter_map(|root| configured_root(Some(root))).collect())
}

/// One cleanup pass over `jobs`, in the order given (design part 5.9, S40, R-B4; Rulings T4-R11, FR-1). Before each
/// job the pass reads `stop`, the engine's stop flag, and ends when it is raised, so a stop waits on at most one share
/// call. For each job: a name not shaped like a job ID is left alone (`job_id_shaped`); a job whose lookup book
/// another holder has locked is skipped this pass, and the pass holds the book only while it deletes the job's TMDb
/// copies, never across the share's reach or the staging deletion (PR #15 review round 1, R6); each TMDb copy in
/// `<data>/jobs/<id>/` whose `fetched_at` is a real date past the six-month limit on `today` is deleted (a copy without
/// one is left to the refetch, Ruling T4-R7); and when `retention_due` or `discard_due` holds, the job's staging folder
/// is deleted once its share answers, without following any link inside it (a root that is absent or blank, `/`
/// included, is none: `pass_roots`). When `discard_due` holds, the job's partial files in each of its library
/// roots are deleted once that root's share answers (`delete_partial_files`; phase 3 design, part 6.8). The share of
/// each root is asked once per pass and its answer kept for the pass's other jobs. Every deletion and failure gives
/// `log` one engine.log line, and each root not reached gives one line at the pass's end with the count of jobs it
/// skipped, naming it a staging or a library root (P3-T10). It returns the ids of the jobs whose TMDb copies it
/// deleted, in the pass's order, each once, for the refresh's mirror (design part 7.5). Def: T4-11; the ids: T5-20;
/// the discard: P3-T10. Use: T4-11, T5-20, P3-T15, P3-T16.
pub(crate) fn run_pass(
    jobs: Vec<CleanupJob>,
    data: &Path,
    shares: &dyn ShareConnector,
    today: &str,
    now_unix: u64,
    stop: &AtomicBool,
    log: &dyn Fn(String),
) -> Vec<String> {
    let mut roots = Vec::new();
    let mut deleted = Vec::new();
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
        // The book is held only while the copies are deleted, never across the share's reach or the staging deletion,
        // since the owner thread takes it blocking (PR #15 review round 1, R6).
        {
            let _book = match job.lookups.try_lock() {
                Ok(guard) => guard,
                Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
                Err(TryLockError::WouldBlock) => continue,
            };
            if delete_stale_copies(&data.join("jobs").join(id), id, today, log) {
                deleted.push(id.to_string());
            }
        }
        let discarded = discard_due(&job);
        if !discarded && !retention_due(&job, now_unix) {
            continue;
        }
        let (staging_root, library_roots) = pass_roots(&job);
        if let Some(root) = staging_root {
            if reached(&mut roots, root, STAGING_ROOT, shares) {
                delete_staging_folder(root, id, log);
            }
        }
        if !discarded {
            continue;
        }
        for root in library_roots {
            if reached(&mut roots, root, LIBRARY_ROOT, shares) {
                delete_partial_files(root, id, log);
            }
        }
    }
    for RootReach { root, kind, failure, skipped } in roots {
        if let Some(why) = failure {
            log(format!("cleanup: the {kind} {root} was not reached: {why}; {skipped} job(s) skipped"));
        }
    }
    deleted
}

/// The name a pass's line gives a staging root it did not reach.
const STAGING_ROOT: &str = "staging root";
/// The name a pass's line gives a library root it did not reach (phase 3 design, part 6.8).
const LIBRARY_ROOT: &str = "library root";

/// One root as a pass found it (Ruling FR-1): what kind of root its line names (`STAGING_ROOT` or `LIBRARY_ROOT`, the
/// kind of its first job's ask), the failure text when its share did not answer, and how many jobs it skipped for that.
struct RootReach {
    root: String,
    kind: &'static str,
    failure: Option<String>,
    skipped: usize,
}

/// Whether the share of `root` answered this pass: asked on the root's first job only, its answer kept in `roots`,
/// and a job under a root that did not answer counted as skipped.
fn reached(roots: &mut Vec<RootReach>, root: &str, kind: &'static str, shares: &dyn ShareConnector) -> bool {
    let index = match roots.iter().position(|known| known.root == root) {
        Some(index) => index,
        None => {
            let failure = match shares.reach(root) {
                Reach::Unreachable(why) => Some(why),
                Reach::Reachable => None,
            };
            roots.push(RootReach { root: root.to_string(), kind, failure, skipped: 0 });
            roots.len() - 1
        }
    };
    let known = &mut roots[index];
    if known.failure.is_some() {
        known.skipped += 1;
    }
    known.failure.is_none()
}

/// Deletes each TMDb copy in `folder` whose `fetched_at` is a real date past the six-month limit on `today`; true when
/// it removed at least one.
fn delete_stale_copies(folder: &Path, id: &str, today: &str, log: &dyn Fn(String)) -> bool {
    let mut removed = false;
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
            Ok(()) => {
                removed = true;
                log(format!("cleanup: {id}: deleted {file} (fetched_at {date})"));
            }
            Err(error) => log(format!("cleanup: {id}: {file} could not be deleted: {error}")),
        }
    }
    removed
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

/// Deletes the partial files of job `id` in `<root>/.lazymkv-partial/`, the share holding `root` having answered this
/// pass (phase 3 design, part 6.8): each entry directly inside that folder whose name starts with `<id>-` and ends with
/// `.mkv.partial` and which is a regular file, by its own type, never by a link's target. A partial folder that is
/// itself a link, or no folder, is left alone; a link, a folder or a deeper entry in it is never deleted or followed;
/// another job's file is left, since its name starts with its own id and a dash. Each deletion and failure gives `log`
/// one line. The caller has checked that `id` is shaped like a job ID and that `root` is set. Def: P3-T10. Use:
/// P3-T15, P3-T16.
fn delete_partial_files(root: &str, id: &str, log: &dyn Fn(String)) {
    let folder = Path::new(root).join(PARTIAL_FOLDER);
    match fs::symlink_metadata(&folder) {
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => return log(format!("cleanup: {id}: {} was left alone: it is not a folder", folder.display())),
        Err(error) if error.kind() == ErrorKind::NotFound => return,
        Err(error) => return log(format!("cleanup: {id}: {} could not be read: {error}", folder.display())),
    }
    let entries = match fs::read_dir(&folder) {
        Ok(entries) => entries,
        Err(error) => return log(format!("cleanup: {id}: {} could not be read: {error}", folder.display())),
    };
    let prefix = format!("{id}-");
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                log(format!("cleanup: {id}: {} could not be read: {error}", folder.display()));
                continue;
            }
        };
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        if !name.starts_with(&prefix) || !name.ends_with(PARTIAL_SUFFIX) {
            continue;
        }
        // `DirEntry::file_type` does not follow a link, so a link to a file or a folder is never a regular file here.
        match entry.file_type() {
            Ok(kind) if kind.is_file() => {}
            Ok(_) => continue,
            Err(error) => {
                log(format!("cleanup: {id}: the partial file {name} could not be read: {error}"));
                continue;
            }
        }
        match fs::remove_file(folder.join(name)) {
            Ok(()) => log(format!("cleanup: {id}: deleted the partial file {name}")),
            Err(error) => log(format!("cleanup: {id}: the partial file {name} could not be deleted: {error}")),
        }
    }
}

/// The end of a partial file's name (`file_phase::partial_path`).
const PARTIAL_SUFFIX: &str = ".mkv.partial";

#[cfg(test)]
mod tests {
    use super::*;
    use lazymkv_core::job::{FailureClass, Held};
    use lazymkv_core::profile::JOB_PROFILE_FILE;
    use lazymkv_core::record::TuningCopy;
    use lazymkv_core::record::JOB_FORMAT;
    use lazymkv_core::review::Note;
    use lazymkv_core::review::Noted;
    use lazymkv_core::testsupport::{Scratch, ScratchExt};
    use lazymkv_platform::fakes::FakeShares;
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::time::Duration;

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
            tuning_copy: None,
        };
        CleanupJob {
            id, record, staging_root: None, library_roots: vec![], retention_days: 7,
            lookups: Arc::new(Mutex::new(LookupBook::default())),
        }
    }

    const UNANSWERED: &str = "the share did not answer";

    fn library(folders: &Scratch) -> String {
        folders.0.join("library").display().to_string()
    }

    /// A Discarded job (phase 3 design, part 6.8) updated `NOW`, so no age holds it, whose library root is the
    /// scratch `library` folder.
    fn discarded(n: u64) -> CleanupJob {
        let mut job = job(n, State::Discarded, None);
        job.record.updated_unix = NOW;
        job
    }

    /// `pass` with each job's library roots set to the scratch `library` folder (phase 3 design, part 6.8).
    fn pass_with_library(folders: &Scratch, jobs: Vec<CleanupJob>, shares: &FakeShares) -> Vec<String> {
        let root = library(folders);
        let jobs = jobs.into_iter().map(|job| CleanupJob { library_roots: vec![root.clone()], ..job }).collect();
        pass(folders, jobs, shares, TODAY, NOW)
    }

    /// Phase 3 design, part 6.8: a discard deletes at any pass, whatever the job's age, and nothing but its staging
    /// folder and its own partial files; the job folder, the library's files and the tuning root stay.
    #[test]
    fn a_discarded_job_has_its_staging_folder_and_partial_files_deleted_at_any_age() {
        let folders = Scratch::new("cleanup-discarded");
        let id = id(1);
        let state = folders.write_path(&format!("data/jobs/{id}/state.json"), "x");
        folders.write(&format!("staging/{id}/rip/t00-00800/title_t00.mkv"), "b");
        folders.write(&format!("staging/{id}/tuning/t00-00800/audio-proxy.mka"), "mka");
        let partial = format!("{id}-t00-00800.mkv.partial");
        folders.write(&format!("library/.lazymkv-partial/{partial}"), "p");
        let film = folders.write_path("library/Film (2001)/Film (2001) - 1080p.mkv", "film");
        let tuning = folders.write_path(&format!("tuning/dataset/{id}/a.txt"), "t");
        let mut pending = discarded(1);
        pending.record.tuning_copy = Some(TuningCopy::Dropped);
        assert!(discard_due(&pending));
        assert!(!retention_due(&pending, NOW));
        let lines = pass_with_library(&folders, vec![pending], &FakeShares::new());
        assert!(!folders.0.join("staging").join(&id).exists());
        assert!(folders.0.join("staging").is_dir());
        assert!(!folders.0.join("library/.lazymkv-partial").join(&partial).exists());
        assert!(folders.0.join("library/.lazymkv-partial").is_dir(), "the partial folder itself stays");
        assert_eq!(fs::read(&state).unwrap(), b"x");
        assert_eq!(fs::read(&film).unwrap(), b"film");
        assert_eq!(fs::read(&tuning).unwrap(), b"t");
        assert_eq!(
            lines,
            [
                format!("cleanup: {id}: deleted its staging folder"),
                format!("cleanup: {id}: deleted the partial file {partial}"),
            ]
        );
        for state in [held(), failed(), State::Filed, State::Ripping] {
            assert!(!discard_due(&job(2, state, Some(NOW - 30 * DAY))));
        }
    }

    /// Phase 3 design, part 6.8: nothing is deleted under a root whose share does not answer, and each such root
    /// gets its line at the pass's end.
    #[test]
    fn a_discarded_job_under_an_unreached_root_keeps_its_data_and_is_logged() {
        let folders = Scratch::new("cleanup-discarded-unreached");
        let id = id(1);
        let staged = folders.write_path(&format!("staging/{id}/rip/t00/b.mkv"), "b");
        let partial = folders.write_path(&format!("library/.lazymkv-partial/{id}-t00.mkv.partial"), "p");
        let (staging_root, library_root) = (staging(&folders), library(&folders));
        let shares = FakeShares::new();
        shares.set_reach(&staging_root, Reach::Unreachable(UNANSWERED.into()));
        shares.set_reach(&library_root, Reach::Unreachable("cable out".into()));
        let lines = pass_with_library(&folders, vec![discarded(1)], &shares);
        assert_eq!(fs::read(&staged).unwrap(), b"b");
        assert_eq!(fs::read(&partial).unwrap(), b"p");
        assert_eq!(
            lines,
            [
                format!("cleanup: the staging root {staging_root} was not reached: {UNANSWERED}; 1 job(s) skipped"),
                format!("cleanup: the library root {library_root} was not reached: cable out; 1 job(s) skipped"),
            ]
        );
        // The library answers: its partial file goes, and the staging data waits for its own share.
        shares.set_reach(&library_root, Reach::Reachable);
        let lines = pass_with_library(&folders, vec![discarded(1)], &shares);
        assert_eq!(fs::read(&staged).unwrap(), b"b");
        assert!(!partial.exists());
        assert_eq!(
            lines,
            [
                format!("cleanup: {id}: deleted the partial file {id}-t00.mkv.partial"),
                format!("cleanup: the staging root {staging_root} was not reached: {UNANSWERED}; 1 job(s) skipped"),
            ]
        );
    }

    /// The roots a pass may use (phase 3 design, part 6.8; fix round 1): an absent, blank or whitespace root and `/`
    /// are none, for a staging root and for a library root. This checks the guard at a level that deletes nothing, since
    /// a literal `/` cannot be placed inside the scratch folder.
    #[test]
    fn a_blank_or_slash_root_is_no_root_a_pass_may_use() {
        let mut job = discarded(1);
        for root in ["", "  ", "/", " / ", "//"] {
            job.staging_root = Some(root.to_string());
            job.library_roots = vec![root.to_string(), "/srv/media/Movies".to_string()];
            assert_eq!(pass_roots(&job), (None, vec!["/srv/media/Movies"]), "{root:?}");
        }
        job.staging_root = None;
        job.library_roots = vec![];
        assert_eq!(pass_roots(&job), (None, vec![]));
        job.staging_root = Some("/srv/staging".to_string());
        assert_eq!(pass_roots(&job), (Some("/srv/staging"), vec![]));
    }

    /// Fix round 1: a pass never asks or deletes under a blank, whitespace or `/` root, for a discarded job's staging
    /// and library roots or a filed job's staging root past its retention. Each such root is set unreachable in the
    /// fake, so a missing guard would show as a "was not reached" line and a reach, never as a deletion: a pass whose
    /// share does not answer deletes nothing, so the test cannot act on the real `/` or the working folder even then.
    #[test]
    fn a_pass_never_uses_a_blank_or_slash_root() {
        let folders = Scratch::new("cleanup-blank-roots");
        let shares = FakeShares::new();
        let roots = ["", "  ", "/"];
        for root in roots {
            shares.set_reach(root, Reach::Unreachable("the guard is missing".into()));
        }
        let mut jobs = Vec::new();
        for (n, root) in (1..).zip(roots) {
            let mut staging = discarded(n);
            staging.staging_root = Some(root.to_string());
            let mut filed = job(n + 10, State::Filed, Some(NOW - 30 * DAY));
            filed.staging_root = Some(root.to_string());
            let mut library = discarded(n + 20);
            library.library_roots = vec![root.to_string()];
            jobs.extend([staging, filed, library]);
        }
        let lines = RefCell::new(Vec::new());
        let stop = AtomicBool::new(false);
        run_pass(jobs, &folders.0.join("data"), &shares, TODAY, NOW, &stop, &|line| lines.borrow_mut().push(line));
        for root in roots {
            assert_eq!(shares.reaches(root), 0, "{root:?} was asked");
        }
        assert_eq!(lines.into_inner(), Vec::<String>::new());
    }

    /// Phase 3 design, part 6.8: only the regular files `<job id>-*.mkv.partial` directly inside `.lazymkv-partial`
    /// go. Another job's file (one whose number begins with this job's included), a name without the suffix, a folder,
    /// a file outside the partial folder and a link are left, and no link is followed.
    #[test]
    fn a_partial_file_of_another_job_is_left_alone() {
        let folders = Scratch::new("cleanup-partial-others");
        let other = id(2);
        let id = id(1);
        let own = folders.write_path(&format!("library/.lazymkv-partial/{id}-t00.mkv.partial"), "p");
        let kept = [
            folders.write_path(&format!("library/.lazymkv-partial/{other}-t00.mkv.partial"), "other"),
            folders.write_path(&format!("library/.lazymkv-partial/{:032x}-12-t00.mkv.partial", 1), "twelve"),
            folders.write_path(&format!("library/.lazymkv-partial/{id}-t00.mkv"), "no suffix"),
            folders.write_path(&format!("library/.lazymkv-partial/{id}-t01.mkv.partial/inner.mkv.partial"), "inside"),
            folders.write_path(&format!("library/{id}-t00.mkv.partial"), "outside"),
            folders.write_path(&format!("library/.lazymkv-partial/sub/{id}-t00.mkv.partial"), "deeper"),
        ];
        let film = folders.write_path("elsewhere/film.mkv", "film");
        let elsewhere = folders.0.join("elsewhere");
        // A second library root whose partial folder is a link to a folder holding this job's name.
        let linked = folders.write_path(&format!("target/{id}-t00.mkv.partial"), "linked");
        let second = folders.0.join("second");
        fs::create_dir_all(&second).unwrap();
        #[cfg(unix)]
        {
            let folder = folders.0.join("library/.lazymkv-partial");
            std::os::unix::fs::symlink(&film, folder.join(format!("{id}-link.mkv.partial"))).unwrap();
            std::os::unix::fs::symlink(&elsewhere, folder.join(format!("{id}-dirlink.mkv.partial"))).unwrap();
            std::os::unix::fs::symlink(folders.0.join("target"), second.join(".lazymkv-partial")).unwrap();
        }
        #[cfg(windows)]
        {
            let junction = |link: &Path, target: &Path| {
                let made = std::process::Command::new("cmd").arg("/c").arg("mklink").arg("/J").arg(link).arg(target)
                    .output().unwrap();
                assert!(made.status.success(), "mklink /J failed");
            };
            let folder = folders.0.join("library/.lazymkv-partial");
            junction(&folder.join(format!("{id}-dirlink.mkv.partial")), &elsewhere);
            junction(&second.join(".lazymkv-partial"), &folders.0.join("target"));
        }
        let mut job = discarded(1);
        job.library_roots = vec![library(&folders), second.display().to_string()];
        let partials = folders.0.join("library/.lazymkv-partial");
        let lines = pass(&folders, vec![job], &FakeShares::new(), TODAY, NOW);
        assert!(!own.exists());
        for path in &kept {
            assert!(path.is_file(), "{}", path.display());
        }
        assert_eq!(fs::read(&film).unwrap(), b"film");
        assert_eq!(fs::read(&linked).unwrap(), b"linked");
        assert!(fs::symlink_metadata(partials.join(format!("{id}-dirlink.mkv.partial"))).is_ok());
        #[cfg(unix)]
        assert!(fs::symlink_metadata(partials.join(format!("{id}-link.mkv.partial"))).is_ok());
        let linked_folder = second.join(".lazymkv-partial");
        assert_eq!(
            lines,
            [
                format!("cleanup: {id}: deleted the partial file {id}-t00.mkv.partial"),
                format!("cleanup: {id}: {} was left alone: it is not a folder", linked_folder.display()),
            ]
        );
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

    /// C2 (owner item 22's interim ended; R5-code-51, R5-code-52): only a pending dataset copy holds the staging data, and
    /// the note, which stays after the copy lands, holds nothing (T5-R14).
    #[test]
    fn retention_is_due_only_for_a_filed_job_older_than_its_days_without_a_pending_copy() {
        let mut filed = job(1, State::Filed, Some(NOW - 7 * DAY - 1));
        assert!(retention_due(&filed, NOW));
        filed.record.filed_unix = Some(NOW - 7 * DAY);
        assert!(!retention_due(&filed, NOW), "exactly its retention is not older");
        filed.record.filed_unix = Some(NOW - 7 * DAY - 1);
        filed.record.tuning_copy = Some(TuningCopy::Pending);
        assert!(!retention_due(&filed, NOW), "a pending copy holds the staging data");
        filed.record.tuning_copy = Some(TuningCopy::Done);
        filed.record.notes.push(Noted { note: Note::TuningEvidencePending, stream: None, detail: String::new() });
        assert!(retention_due(&filed, NOW), "the note stays after the copy lands and holds nothing");
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

    /// C2, R1-evidence-35: pull request 4's real job (filed with the setting on, before step 10a existed) has no staged
    /// tuning folder and nothing pending, so it goes once its 7 days have passed.
    #[test]
    fn a_filed_job_with_no_staged_tuning_folder_is_cleaned_once_its_days_pass() {
        let folders = Scratch::new("cleanup-no-tuning");
        let id = id(1);
        folders.write(&format!("staging/{id}/rip/t00-00800/title_t00.mkv"), "b");
        let lines = pass(&folders, vec![job(1, State::Filed, Some(NOW - 7 * DAY - 1))], &FakeShares::new(), TODAY, NOW);
        assert!(!folders.0.join("staging").join(&id).exists());
        assert_eq!(lines, [format!("cleanup: {id}: deleted its staging folder")]);
    }

    #[test]
    fn a_pending_copy_holds_the_staging_data() {
        let folders = Scratch::new("cleanup-pending");
        let id = id(1);
        let proxy = folders.write_path(&format!("staging/{id}/tuning/t00-00800/audio-proxy.mka"), "mka");
        folders.write(&format!("staging/{id}/rip/t00-00800/title_t00.mkv"), "b");
        let mut pending = job(1, State::Filed, Some(NOW - 30 * DAY));
        pending.record.tuning_copy = Some(TuningCopy::Pending);
        let lines = pass(&folders, vec![pending], &FakeShares::new(), TODAY, NOW);
        assert_eq!(fs::read(&proxy).unwrap(), b"mka");
        assert_eq!(lines, Vec::<String>::new());
    }

    /// R1-evidence-37: a job run with the setting off has a staged folder but no copy to make.
    #[test]
    fn a_setting_off_job_has_nothing_pending() {
        let folders = Scratch::new("cleanup-setting-off");
        let id = id(1);
        folders.write(&format!("staging/{id}/tuning/t00-00800/streamhash.txt"), "md5");
        folders.write(&format!("staging/{id}/tuning/t00-00800/subs-ocr/summary.json"), "{}");
        let lines = pass(&folders, vec![job(1, State::Filed, Some(NOW - 7 * DAY - 1))], &FakeShares::new(), TODAY, NOW);
        assert!(!folders.0.join("staging").join(&id).exists());
        assert_eq!(lines, [format!("cleanup: {id}: deleted its staging folder")]);
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
        kept.record.tuning_copy = Some(TuningCopy::Pending);
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
    fn a_pass_holds_no_book_while_it_reaches_the_staging_share() {
        let folders = Scratch::new("cleanup-book-free");
        let id = id(1);
        folders.write(&format!("staging/{id}/rip/t00/b.mkv"), "b");
        let shares = FakeShares::new();
        let gate = shares.stall(&staging(&folders));
        let job = job(1, State::Filed, Some(NOW - 7 * DAY - 1));
        let book = Arc::clone(&job.lookups);
        std::thread::scope(|scope| {
            let passing = scope.spawn(|| pass(&folders, vec![job], &shares, TODAY, NOW));
            let waited = gate.wait_for_caller(Duration::from_secs(10));
            // The owner takes the book blocking; the pass must not hold it while the share is asked.
            let free = book.try_lock().is_ok();
            gate.release();
            let lines = passing.join().unwrap();
            assert!(waited, "the pass never reached the staging share");
            assert!(free, "the pass held the job's book across the share's reach");
            assert_eq!(lines, [format!("cleanup: {id}: deleted its staging folder")]);
        });
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

    #[test]
    fn a_pass_returns_the_ids_of_the_jobs_whose_tmdb_copies_it_deleted() {
        let folders = Scratch::new("cleanup-deleted-ids");
        let (first, second, third) = (id(1), id(2), id(3));
        folders.write(&format!("data/jobs/{first}/tmdb.json"), copy("2026-04-22"));
        folders.write(&format!("data/jobs/{first}/tmdb-search.json"), copy("2026-04-01"));
        folders.write(&format!("data/jobs/{second}/tmdb.json"), copy("2026-10-01"));
        folders.write(&format!("data/jobs/{third}/tmdb-record.json"), copy("2026-01-01"));
        let jobs = vec![job(1, held(), None), job(2, held(), None), job(3, failed(), None)];
        let stop = AtomicBool::new(false);
        let deleted = run_pass(jobs, &folders.0.join("data"), &FakeShares::new(), TODAY, NOW, &stop, &|_| {});
        assert_eq!(deleted, [first, third], "each job once, in the pass's order");
    }
}
