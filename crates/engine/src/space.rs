//! Design part 5.8's space checks: the promises of unfinished steps, by volume, the Direct rip's check and the remux's
//! check. A check adds, on each volume it reads, this job's later steps there and what other jobs' unfinished steps
//! were promised. Every share read happens on the step or check thread that needs it, before the promises are locked,
//! so a share that stalls never holds the lock the owner thread takes (engine.md:584); a promise rebuilt on the owner
//! thread reads no share and no staged file (engine.md:585): a Remuxing job's Remux promise is the plan's estimate
//! until its Remux step replaces it with the staged sizes (`refine_remux_promise`, Ruling T3-17-a). It also holds the
//! engine's folder rule (design part 6.2, Ruling W15): `make_root` and `root_ready`.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::{Mutex, PoisonError};

use lazymkv_core::job::{Event, State};
use lazymkv_core::plan::{staging_bytes_needed, write_bytes_needed, Kind, Plan};
use lazymkv_core::record::{JobRecord, Transition};
use lazymkv_core::settings::Settings;
use lazymkv_platform::{Reach, ShareConnector};
use lazymkv_tools::CancelToken;

use crate::restart::held_promise;
use crate::steps::mkv_files;

/// Design part 5.8's two steps that promise space. Def: T23. Use: T23, T24, T25.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum PromisedStep {
    /// A Direct rip's staged titles, on staging.
    Rip,
    /// A remux's final files, on the library.
    Remux,
}

/// Space promised to unfinished steps, by (job, step), each on one volume (design part 5.8). Def: T23. Use: T23, T24,
/// T25.
#[derive(Default)]
pub(crate) struct Promises {
    /// (job, step) to (volume, bytes).
    held: BTreeMap<(String, PromisedStep), (String, u64)>,
}

impl Promises {
    /// Promises `bytes` on `volume` to `job`'s `step`, replacing any earlier promise of that step (design part 5.8).
    /// Def: T23. Use: T23, T24, T25.
    pub(crate) fn promise(&mut self, job: &str, step: PromisedStep, volume: &str, bytes: u64) {
        self.held.insert((job.to_string(), step), (volume.to_string(), bytes));
    }

    /// Drops the promise of `job`'s `step`, once the step has finished or will not run (design part 5.8). Def: T23.
    /// Use: T23, T24, T25.
    pub(crate) fn release(&mut self, job: &str, step: PromisedStep) {
        self.held.remove(&(job.to_string(), step));
    }

    /// The bytes promised on `volume` to every job but `except_job` (design part 5.8). Def: T23. Use: T23, T24, T25.
    pub(crate) fn promised(&self, volume: &str, except_job: &str) -> u64 {
        self.held
            .iter()
            .filter(|((job, _), (on, _))| job != except_job && on == volume)
            .map(|(_, (_, bytes))| *bytes)
            .fold(0, u64::saturating_add)
    }
}

/// `shares.volume(root)`, else the root itself (S11, design part 6.1). Def: T23. Use: T23, T24, T25.
pub(crate) fn volume_of(shares: &dyn ShareConnector, root: &str) -> String {
    shares.volume(root).unwrap_or_else(|| root.to_string())
}

/// ShareLost's sentence for a root that does not answer: `<root> is unreachable: <why>` (design part 6.1, "the error
/// says which"; review round 1, V15).
fn unreachable(root: &str, why: &str) -> String {
    format!("{root} is unreachable: {why}")
}

/// ShareLost's sentence for a root whose free space cannot be read: `the free space of <root> could not be read:
/// <error>` (design part 6.1; review round 1, V15).
fn free_space_unread(root: &str, error: &io::Error) -> String {
    format!("the free space of {root} could not be read: {error}")
}

/// Design part 6.2, Ruling W15: a missing `staging_root` or `tuning_root` is created with `fs::create_dir` when its
/// parent is a folder, never deeper; a library root is never passed. Ok when `root` is a folder or was made
/// (`AlreadyExists` counts as made). Err: `<root> does not exist and could not be created: <parent> is not a folder`;
/// `<root> does not exist and could not be created: <error>`; `<root> is not a folder` for a root with no parent.
/// It calls `root_ready` before `fs::create_dir`. Def: T3-17. Use: T3-17.
pub(crate) fn make_root(root: &str) -> Result<(), String> {
    root_ready(root)?;
    if Path::new(root).is_dir() {
        return Ok(());
    }
    match fs::create_dir(root) {
        Err(error) if error.kind() != io::ErrorKind::AlreadyExists => {
            Err(format!("{root} does not exist and could not be created: {error}"))
        }
        _ => Ok(()),
    }
}

/// Whether `make_root` could succeed, creating nothing: Ok when `root` is a folder or its parent is; Err `make_root`'s
/// text otherwise (engine.md:589, Ruling W21: AwaitingShare's re-check asks it of `staging_root`, so a missing parent
/// keeps the job waiting instead of cycling). Def: T3-17. Use: T3-17.
pub(crate) fn root_ready(root: &str) -> Result<(), String> {
    let path = Path::new(root);
    if path.is_dir() {
        return Ok(());
    }
    match path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        Some(parent) if parent.is_dir() => Ok(()),
        Some(parent) => {
            Err(format!("{root} does not exist and could not be created: {} is not a folder", parent.display()))
        }
        None => Err(format!("{root} is not a folder")),
    }
}

/// Design part 5.8's releases of `transition`, from it alone, so the owner locks the promises only when there is
/// something to release (engine.md:584): Rip when it leaves Ripping, Remux when it leaves Remuxing, both when it enters
/// a waiting state or Failed; each at most once, Rip first. Def: T3-17. Use: T3-17.
pub(crate) fn released_steps(transition: &Transition) -> Vec<PromisedStep> {
    let stops = transition.new.is_waiting() || matches!(transition.new, State::Failed { .. });
    let leaves = |state: State| transition.old.as_ref() == Some(&state) && transition.new != state;
    let mut steps = Vec::new();
    if stops || leaves(State::Ripping) {
        steps.push(PromisedStep::Rip);
    }
    if stops || leaves(State::Remuxing) {
        steps.push(PromisedStep::Remux);
    }
    steps
}

/// The Direct rip's check (design part 5.8, S54): an unreachable staging or library root gives ShareLost; staging must
/// hold `rip_bytes`, plus `remux_bytes` when the library shares its volume, plus what other jobs were promised there; a
/// library on its own volume must hold `remux_bytes` plus what other jobs were promised there. A `free_bytes` error
/// gives ShareLost and a short volume SpaceShort. Otherwise the job is promised Rip on staging and Remux on the
/// library (on staging when no library is set), and the check gives SpaceOk. ShareLost comes with its sentence, which
/// names the root and why (`<root> is unreachable: <why>`, staging first, or `the free space of <root> could not be
/// read: <error>`; design part 6.1, review round 1's V15); every other event comes with none. It reads `reach`,
/// `volume` and `free_bytes` (staging's, and the library's when on its own volume) before it locks `promises`, and
/// holds the lock only to add the promised amounts, compare and promise, so a share that stalls never holds the lock
/// the owner thread takes (engine.md:584). Def: T23, its sentence PR #6 review (V15), its lock T3-17. Use: T23, T26,
/// T3-17.
pub(crate) fn rip_check(
    rip_bytes: u64,
    remux_bytes: u64,
    staging: &str,
    library: Option<&str>,
    shares: &dyn ShareConnector,
    promises: &Mutex<Promises>,
    job: &str,
) -> (Event, Option<String>) {
    for root in std::iter::once(staging).chain(library) {
        if let Reach::Unreachable(why) = shares.reach(root) {
            return (Event::ShareLost, Some(unreachable(root, &why)));
        }
    }
    let staging_volume = volume_of(shares, staging);
    let library_volume = library.map(|root| volume_of(shares, root));
    let shared = library_volume.as_deref().is_none_or(|volume| volume == staging_volume);
    let staging_free = shares.free_bytes(staging);
    let library_free = match (library, library_volume.as_deref(), shared) {
        (Some(root), Some(volume), false) => Some((root, volume, shares.free_bytes(root))),
        _ => None,
    };
    let mut promises = promises.lock().unwrap_or_else(PoisonError::into_inner);
    let staging_needs = rip_bytes
        .saturating_add(if shared && library.is_some() { remux_bytes } else { 0 })
        .saturating_add(promises.promised(&staging_volume, job));
    let mut checks = vec![(staging, staging_free, staging_needs)];
    if let Some((root, volume, free)) = library_free {
        checks.push((root, free, remux_bytes.saturating_add(promises.promised(volume, job))));
    }
    for (root, free, needs) in checks {
        match free {
            Err(error) => return (Event::ShareLost, Some(free_space_unread(root, &error))),
            Ok(free) if free < needs => return (Event::SpaceShort, None),
            Ok(_) => {}
        }
    }
    promises.promise(job, PromisedStep::Rip, &staging_volume, rip_bytes);
    let remux_volume = library_volume.unwrap_or(staging_volume);
    promises.promise(job, PromisedStep::Remux, &remux_volume, remux_bytes);
    (Event::SpaceOk, None)
}

/// The Rip promise on staging: `staging_bytes_needed(plan, disc_size)` (plan/mod.rs; design part 5.8). Def: T23.
/// Use: T23, T25.
pub(crate) fn rip_promise(plan: &Plan, disc_size: u64) -> u64 {
    staging_bytes_needed(plan, disc_size)
}

/// The rip check's Remux promise on the library: `write_bytes_needed` of the planned titles' sizes (S54, design part
/// 5.8). Def: T23. Use: T23, T25.
pub(crate) fn planned_remux_promise(plan: &Plan) -> u64 {
    write_bytes_needed(plan.titles.iter().map(|title| title.size.unwrap_or(0)).fold(0, u64::saturating_add))
}

/// The remux check's Remux promise: `write_bytes_needed(staged_bytes)`, the staged files' sizes (S54, design part
/// 5.8). Def: T23. Use: T24, T25.
pub(crate) fn remux_promise(staged_bytes: u64) -> u64 {
    write_bytes_needed(staged_bytes)
}

/// The staged `.mkv` files' sizes summed: the files of each planned title's staging folder
/// `<staging>/<id>/rip/<title key>` (design part 5.6 step 6, S54), listed with `mkv_files`. A folder that cannot be
/// listed, a missing one included, or a file whose size cannot be read is an Err naming it (review round 1, V3;
/// RR1-6(a)), never 0 bytes. It reads staging, so a caller computes it on its own thread before it locks the
/// `Promises`, never on the owner thread (engine.md:585). Def: PR #6 review (V3). Use: `remux_space`, `space_freed`,
/// `remux` (through `refine_remux_promise`, T3-17).
pub(crate) fn staged_bytes(staging: &str, id: &str, plan: &Plan) -> Result<u64, String> {
    let rip = Path::new(staging).join(id).join("rip");
    let mut total: u64 = 0;
    for planned in &plan.titles {
        for file in mkv_files(&rip.join(&planned.key))? {
            let size = fs::metadata(&file).map_err(|error| format!("{}: {error}", file.display()))?.len();
            total = total.saturating_add(size);
        }
    }
    Ok(total)
}

/// The remux check of Decided over `staged_bytes`' result (design part 5.8, S54; review round 1, V3 and RR1-6(b)):
/// with the staged sizes, `remux_check` of `remux_promise(bytes)`. Without them, staging that `reach` finds
/// unreachable gives ShareLost naming it, the staging error after it, and no promise is touched; staging that answers
/// gives `remux_check` of the rip check's estimate, `planned_remux_promise(plan)`, never 0 bytes (design part 5.8: a
/// passed check means the job can finish), with `the staged files could not be read: <error>` as the sentence when the
/// check gives none: CheckSpace carries it with SpaceOk and writes it only to job.log with SpaceShort, whose message
/// stays the shortage's (`space_result`; the final review of review round 1, N1). ShareLost is never fed for a staging
/// error on a share that answers, which AwaitingShare's re-check, reading no staged file (S81, Ruling W21), could not
/// clear. Its share reads, as `remux_check`'s, come before the `promises` lock (engine.md:584). Def: PR #6 review
/// (V3), its lock T3-17. Use: `remux_space`, `space_freed`.
pub(crate) fn staged_remux_check(
    staged: Result<u64, String>,
    plan: &Plan,
    staging: &str,
    library: &str,
    shares: &dyn ShareConnector,
    promises: &Mutex<Promises>,
    job: &str,
) -> (Event, Option<String>) {
    let error = match staged {
        Ok(bytes) => return remux_check(remux_promise(bytes), library, shares, promises, job),
        Err(error) => error,
    };
    if let Reach::Unreachable(why) = shares.reach(staging) {
        let sentence = format!("{}; the staged files could not be read: {error}", unreachable(staging, &why));
        return (Event::ShareLost, Some(sentence));
    }
    let (found, sentence) = remux_check(planned_remux_promise(plan), library, shares, promises, job);
    (found, sentence.or_else(|| Some(format!("the staged files could not be read: {error}"))))
}

/// The promises a job holds across a restart, a return from a wait or a Retry, which no check made in this engine run
/// (design parts 5.8, 5.10, Rulings S75, R25.c2): `held_promise`'s step through Task 23's functions (`rebuild_promise`:
/// Rip in Ripping, and in Remuxing the plan's estimate, which the Remux step replaces with the staged sizes,
/// `refine_remux_promise`, Ruling T3-17-a), and from Ripping to Decided (Ripping, RipVerified, Ejected, Inspecting,
/// Decided) the Remux promise `rip_check` made, `planned_remux_promise(plan)` on the volume of the plan kind's library
/// root, or of staging when none is set, as `rip_check` places it; the remux check replaces it in Decided. Every other
/// state holds none, as `release_promises` leaves it. Nothing is promised without a plan or a `staging_root`. It reads
/// no share and no staged file, only the job folder's meta.json and `volume`, which answers from the root's text, so
/// the owner thread may call it (engine.md:585). Def: T25, its Remux promise T3-17. Use: T25.
pub(crate) fn rebuild_promises(
    promises: &mut Promises,
    shares: &dyn ShareConnector,
    record: &JobRecord,
    settings: &Settings,
    job_dir: &Path,
) {
    if let Some(step) = held_promise(&record.state) {
        rebuild_promise(promises, shares, step, record, settings, job_dir);
    }
    let checked = matches!(
        record.state,
        State::Ripping | State::RipVerified { .. } | State::Ejected { .. } | State::Inspecting | State::Decided { .. }
    );
    let (true, Some(plan), Some(staging)) = (checked, &record.plan, settings.staging_root.as_deref()) else {
        return;
    };
    let volume = volume_of(shares, library_root(plan, settings).unwrap_or(staging));
    promises.promise(&record.id, PromisedStep::Remux, &volume, planned_remux_promise(plan));
}

/// Ruling S75: `record`'s promise of `step` rebuilt from the job folder, through Task 23's functions. Rip is
/// `rip_promise(plan, meta.json's total_bytes)` on `volume_of` the job's `staging_root`; Remux is the rip check's
/// estimate, `planned_remux_promise(plan)`, on `volume_of` the plan kind's library root (`movies_root` for a movie),
/// read from no staged file, so the owner thread never lists staging (engine.md:585); the Remux step replaces it with
/// `remux_promise` of the staged sizes (`refine_remux_promise`). Nothing is promised without a plan or those roots.
/// Ruling T3-17-a amends S75 and V3(c): the Remuxing job's rebuilt Remux promise is the plan's estimate.
fn rebuild_promise(
    promises: &mut Promises,
    shares: &dyn ShareConnector,
    step: PromisedStep,
    record: &JobRecord,
    settings: &Settings,
    job_dir: &Path,
) {
    let (Some(plan), Some(staging)) = (&record.plan, settings.staging_root.as_deref()) else {
        return;
    };
    match step {
        PromisedStep::Rip => {
            let meta = fs::read(job_dir.join("meta.json")).ok();
            let meta = meta.and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
            let disc_size = meta.and_then(|meta| meta["total_bytes"].as_u64()).unwrap_or(0);
            promises.promise(&record.id, step, &volume_of(shares, staging), rip_promise(plan, disc_size));
        }
        PromisedStep::Remux => {
            let Some(library) = library_root(plan, settings) else {
                return;
            };
            promises.promise(&record.id, step, &volume_of(shares, library), planned_remux_promise(plan));
        }
    }
}

/// The library root of the plan's kind: `movies_root` or `tv_root`.
fn library_root<'a>(plan: &Plan, settings: &'a Settings) -> Option<&'a str> {
    match plan.kind {
        Kind::Movie => settings.movies_root.as_deref(),
        Kind::Tv => settings.tv_root.as_deref(),
    }
}

/// The remux's check (design part 5.8, S54): the staged files' sizes; SpaceOk replaces this job's Remux promise. An
/// unreachable library root, or a `free_bytes` error, gives ShareLost, with its sentence naming the root and why
/// (design part 6.1; review round 1, V15); a library that does not hold `remux_bytes` plus what other jobs were
/// promised on its volume gives SpaceShort. Otherwise `job`'s Remux promise becomes `remux_bytes` on the library's
/// volume and the check gives SpaceOk. It reads `reach`, `volume` and `free_bytes` before it locks `promises`, and
/// holds the lock only to add the promised amounts, compare and promise (engine.md:584). Def: T24, its sentence PR #6
/// review (V15), its lock T3-17. Use: T24, T3-17.
pub(crate) fn remux_check(
    remux_bytes: u64,
    library: &str,
    shares: &dyn ShareConnector,
    promises: &Mutex<Promises>,
    job: &str,
) -> (Event, Option<String>) {
    if let Reach::Unreachable(why) = shares.reach(library) {
        return (Event::ShareLost, Some(unreachable(library, &why)));
    }
    let volume = volume_of(shares, library);
    let free = shares.free_bytes(library);
    let mut promises = promises.lock().unwrap_or_else(PoisonError::into_inner);
    let needs = remux_bytes.saturating_add(promises.promised(&volume, job));
    match free {
        Err(error) => (Event::ShareLost, Some(free_space_unread(library, &error))),
        Ok(free) if free < needs => (Event::SpaceShort, None),
        Ok(_) => {
            promises.promise(job, PromisedStep::Remux, &volume, remux_bytes);
            (Event::SpaceOk, None)
        }
    }
}

/// engine.md:585 (Ruling S75): the Remux step's own promise once its staged sizes are read with no lock held:
/// `remux_promise(bytes)` on `volume_of(library)`, written under the lock unless `cancel` is raised (the owner cancels a
/// step before it releases that job's promises, so a release is never undone). Err leaves the estimate in place.
/// Ruling T3-17-a: it replaces the plan's estimate `rebuild_promise` gives a Remuxing job. Def: T3-17. Use: T3-17.
pub(crate) fn refine_remux_promise(
    staged: Result<u64, String>,
    library: &str,
    shares: &dyn ShareConnector,
    promises: &Mutex<Promises>,
    job: &str,
    cancel: &CancelToken,
) {
    let Ok(bytes) = staged else {
        return;
    };
    let volume = volume_of(shares, library);
    let mut promises = promises.lock().unwrap_or_else(PoisonError::into_inner);
    if !cancel.is_cancelled() {
        promises.promise(job, PromisedStep::Remux, &volume, remux_promise(bytes));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    use lazymkv_core::disc::disc;
    use lazymkv_core::job::FailureClass;
    use lazymkv_core::lookup::LookupInputs;
    use lazymkv_core::plan::movie::LibraryFacts;
    use lazymkv_core::plan::Mode;
    use lazymkv_core::plan::{plan as make_plan, PlanInputs};
    use lazymkv_core::record::Transition;
    use lazymkv_core::record::{TitleRecord, JOB_FORMAT};
    use lazymkv_core::robot::parse;
    use lazymkv_core::testsupport::{Scratch, ScratchExt};
    use lazymkv_platform::fakes::FakeShares;
    use lazymkv_tools::CancelToken;

    /// movie-direct's plan: one title, `t00-00800`, whose estimate `planned_remux_promise` is more than 0.
    fn movie_plan() -> Plan {
        let scan = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/scenarios/movie-direct/scan.txt");
        let typed = disc(&parse(&fs::read_to_string(scan).unwrap())).unwrap();
        let settings =
            Settings { staging_root: Some("/s".into()), movies_root: Some("/m".into()), ..Settings::default() };
        make_plan(&PlanInputs {
            disc: &typed, fingerprint: Some("AAA"), disc_id: None, lookups: LookupInputs::default(), remembered: None,
            overrides: None, sessions: &[], library: &LibraryFacts::default(), settings: &settings,
        })
        .unwrap()
    }

    #[test]
    fn a_rip_check_on_a_volume_shared_with_the_library_counts_its_own_remux() {
        let shares = FakeShares::new();
        shares.same_volume("/s", "/m");
        shares.set_free("/s", 8601);
        shares.set_free("/m", 8601);
        let promises = Mutex::new(Promises::default());
        assert_eq!(rip_check(4301, 4301, "/s", Some("/m"), &shares, &promises, "a-1"), (Event::SpaceShort, None));
        shares.set_free("/s", 8602);
        shares.set_free("/m", 8602);
        assert_eq!(rip_check(4301, 4301, "/s", Some("/m"), &shares, &promises, "a-1"), (Event::SpaceOk, None));
        assert_eq!(promises.lock().unwrap().promised(&volume_of(&shares, "/s"), "other"), 8602);
    }

    #[test]
    fn another_jobs_promised_remux_counts_against_this_rip() {
        let shares = FakeShares::new();
        shares.same_volume("/s", "/m");
        let promises = Mutex::new(Promises::default());
        promises.lock().unwrap().promise("a-1", PromisedStep::Remux, &volume_of(&shares, "/m"), 4301);
        shares.set_free("/s", 12902);
        shares.set_free("/m", 12902);
        assert_eq!(rip_check(4301, 4301, "/s", Some("/m"), &shares, &promises, "b-1"), (Event::SpaceShort, None));
        shares.set_free("/s", 12903);
        shares.set_free("/m", 12903);
        assert_eq!(rip_check(4301, 4301, "/s", Some("/m"), &shares, &promises, "b-1"), (Event::SpaceOk, None));
    }

    #[test]
    fn the_remux_check_replaces_the_rips_remux_promise_with_the_staged_size() {
        let shares = FakeShares::new();
        let promises = Mutex::new(Promises::default());
        let library = volume_of(&shares, "/m");
        promises.lock().unwrap().promise("a-1", PromisedStep::Remux, &library, 4301);
        shares.set_free("/m", 17);
        assert_eq!(remux_check(17, "/m", &shares, &promises, "a-1"), (Event::SpaceOk, None));
        assert_eq!(promises.lock().unwrap().promised(&library, "b-1"), 17);
        shares.set_free("/m", 16);
        assert_eq!(remux_check(17, "/m", &shares, &promises, "a-1"), (Event::SpaceShort, None));
    }

    #[test]
    fn a_promise_is_released_when_its_step_finishes() {
        let mut promises = Promises::default();
        promises.promise("a-1", PromisedStep::Rip, "v", 100);
        promises.promise("a-1", PromisedStep::Remux, "v", 50);
        assert_eq!(promises.promised("v", "b-1"), 150);
        assert_eq!(promises.promised("v", "a-1"), 0);
        promises.release("a-1", PromisedStep::Rip);
        assert_eq!(promises.promised("v", "b-1"), 50);
    }

    /// Review round 1, V15 (design part 6.1): ShareLost names the root that does not answer and why, staging first, and
    /// makes no promise.
    #[test]
    fn an_unreachable_root_is_named_with_its_reason() {
        let shares = FakeShares::new();
        shares.set_reach("/m", Reach::Unreachable("cable out".to_string()));
        let promises = Mutex::new(Promises::default());
        let lost = rip_check(4301, 4301, "/s", Some("/m"), &shares, &promises, "a-1");
        assert_eq!(lost, (Event::ShareLost, Some("/m is unreachable: cable out".to_string())));
        shares.set_reach("/s", Reach::Unreachable("gone".to_string()));
        let lost = rip_check(4301, 4301, "/s", Some("/m"), &shares, &promises, "a-1");
        assert_eq!(lost, (Event::ShareLost, Some("/s is unreachable: gone".to_string())));
        assert_eq!(promises.lock().unwrap().promised(&volume_of(&shares, "/s"), "b-1"), 0);
        assert_eq!(promises.lock().unwrap().promised(&volume_of(&shares, "/m"), "b-1"), 0);
    }

    /// Review round 1, V15: the remux check names an unreachable library and leaves the job's earlier Remux promise as
    /// it was.
    #[test]
    fn the_remux_check_names_an_unreachable_library() {
        let shares = FakeShares::new();
        let promises = Mutex::new(Promises::default());
        let library = volume_of(&shares, "/m");
        promises.lock().unwrap().promise("a-1", PromisedStep::Remux, &library, 4301);
        shares.set_reach("/m", Reach::Unreachable("cable out".to_string()));
        let lost = remux_check(17, "/m", &shares, &promises, "a-1");
        assert_eq!(lost, (Event::ShareLost, Some("/m is unreachable: cable out".to_string())));
        assert_eq!(promises.lock().unwrap().promised(&library, "b-1"), 4301);
    }

    /// Review round 1, V15: a free space that cannot be read is ShareLost naming the root and the error, in both
    /// checks. `/m` answers while its free space cannot be read (`FakeShares::set_free_error`, Ruling W21).
    #[test]
    fn a_free_space_error_is_named_with_its_root() {
        let named = Some("the free space of /m could not be read: access is denied".to_string());
        let shares = FakeShares::new();
        shares.set_free_error("/m", "access is denied");
        let promises = Mutex::new(Promises::default());
        let rip = rip_check(4301, 4301, "/s", Some("/m"), &shares, &promises, "a-1");
        assert_eq!(rip, (Event::ShareLost, named.clone()));
        assert_eq!(remux_check(17, "/m", &shares, &promises, "a-1"), (Event::ShareLost, named));
        assert_eq!(promises.lock().unwrap().promised("/m", "b-1"), 0);
    }

    /// Review round 1, V3: the staged sizes are every planned title's `.mkv` files summed, and only those.
    #[test]
    fn staged_bytes_sums_every_planned_titles_staged_file() {
        let s = Scratch::new("staged-bytes");
        let mut plan = movie_plan();
        let mut second = plan.titles[0].clone();
        second.key = "t01-00801".to_string();
        plan.titles.push(second);
        s.write("a-1/rip/t00-00800/title_t00.mkv", [0u8; 16]);
        s.write("a-1/rip/t00-00800/notes.txt", [0u8; 100]);
        s.write("a-1/rip/t01-00801/title_t01.MKV", [0u8; 5]);
        s.write("a-1/rip/t02-00802/title_t02.mkv", [0u8; 1000]);
        assert_eq!(staged_bytes(s.0.to_str().unwrap(), "a-1", &plan), Ok(21));
        let missing = staged_bytes(s.0.to_str().unwrap(), "b-1", &plan).unwrap_err();
        let folder = s.0.join("b-1").join("rip").join("t00-00800");
        assert!(missing.starts_with(&format!("{}: ", folder.display())), "{missing}");
    }

    /// Review round 1, V3 (RR1-6(b)): staged sizes that cannot be read give ShareLost only when staging does not
    /// answer, leaving the job's promise as it was; otherwise the check promises the plan's estimate, not 0 bytes, and
    /// says why.
    #[test]
    fn an_unreadable_title_folder_gives_share_lost_only_when_staging_is_unreachable() {
        let plan = movie_plan();
        let estimate = planned_remux_promise(&plan);
        let shares = FakeShares::new();
        let library = volume_of(&shares, "/m");
        let promises = Mutex::new(Promises::default());
        promises.lock().unwrap().promise("a-1", PromisedStep::Remux, &library, 4301);
        let unread = || Err::<u64, String>("/s/a-1/rip/t00-00800: not found".to_string());
        shares.set_reach("/s", Reach::Unreachable("gone".to_string()));
        let (found, why) = staged_remux_check(unread(), &plan, "/s", "/m", &shares, &promises, "a-1");
        assert_eq!(found, Event::ShareLost);
        let named = "/s is unreachable: gone; the staged files could not be read: /s/a-1/rip/t00-00800: not found";
        assert_eq!(why.as_deref(), Some(named));
        assert_eq!(promises.lock().unwrap().promised(&library, "b-1"), 4301);
        shares.set_reach("/s", Reach::Reachable);
        let (found, why) = staged_remux_check(unread(), &plan, "/s", "/m", &shares, &promises, "a-1");
        assert_eq!(found, Event::SpaceOk);
        assert_eq!(why.as_deref(), Some("the staged files could not be read: /s/a-1/rip/t00-00800: not found"));
        assert!(estimate > 0);
        assert_eq!(promises.lock().unwrap().promised(&library, "b-1"), estimate);
        let (found, why) = staged_remux_check(Ok(16), &plan, "/s", "/m", &shares, &promises, "a-1");
        assert_eq!((found, why), (Event::SpaceOk, None));
        assert_eq!(promises.lock().unwrap().promised(&library, "b-1"), remux_promise(16));
    }

    /// Shares that fail the test when a read runs while the promises are locked (engine.md:584).
    struct LockProbe(Arc<Mutex<Promises>>);

    impl ShareConnector for LockProbe {
        fn reach(&self, root: &str) -> Reach {
            assert!(self.0.try_lock().is_ok(), "reach({root}) ran under the promises lock");
            Reach::Reachable
        }
        fn free_bytes(&self, root: &str) -> io::Result<u64> {
            assert!(self.0.try_lock().is_ok(), "free_bytes({root}) ran under the promises lock");
            Ok(u64::MAX)
        }
        fn volume(&self, root: &str) -> Option<String> {
            Some(root.to_string())
        }
    }

    #[test]
    fn the_checks_read_the_shares_before_they_take_the_promises_lock() {
        let promises = Arc::new(Mutex::new(Promises::default()));
        let probe = LockProbe(Arc::clone(&promises));
        assert_eq!(rip_check(1, 1, "/s", Some("/m"), &probe, &promises, "a-1"), (Event::SpaceOk, None));
        assert_eq!(remux_check(1, "/m", &probe, &promises, "a-1"), (Event::SpaceOk, None));
        let unread = Err("/s/a-1/rip/t00-00800: not found".to_string());
        let (found, _) = staged_remux_check(unread, &movie_plan(), "/s", "/m", &probe, &promises, "a-1");
        assert_eq!(found, Event::SpaceOk);
    }

    #[test]
    fn the_steps_a_transition_releases_are_computed_from_it_alone() {
        let released =
            |old: Option<State>, new: State| released_steps(&Transition { time_unix: 1, event: None, old, new, actions: vec![] });
        let planned = || State::Planned { mode: Mode::Direct };
        let failed = State::Failed { class: FailureClass::Unknown, from: Box::new(State::Ripping) };
        let both = vec![PromisedStep::Rip, PromisedStep::Remux];
        assert_eq!(released(Some(State::Ripping), State::RipVerified { after_copy: false }), vec![PromisedStep::Rip]);
        assert_eq!(released(Some(State::Remuxing), State::Verified), vec![PromisedStep::Remux]);
        assert_eq!(released(Some(planned()), State::AwaitingSpace { return_to: Box::new(planned()) }), both);
        assert_eq!(released(Some(State::Ripping), failed), both);
        assert_eq!(released(None, State::Fingerprinted), Vec::<PromisedStep>::new());
        assert_eq!(released(Some(planned()), planned()), Vec::<PromisedStep>::new());
        assert_eq!(released(Some(State::Decided { approved: None }), State::Remuxing), Vec::<PromisedStep>::new());
    }

    #[test]
    fn a_missing_root_under_a_folder_is_made_and_nothing_deeper() {
        let s = Scratch::new("make-root");
        let made = s.0.join("engine");
        assert_eq!(make_root(made.to_str().unwrap()), Ok(()));
        assert!(made.is_dir());
        assert_eq!(make_root(made.to_str().unwrap()), Ok(()));
        let parent = s.0.join("absent");
        let deeper = parent.join("engine");
        let refused = make_root(deeper.to_str().unwrap()).unwrap_err();
        let expected =
            format!("{} does not exist and could not be created: {} is not a folder", deeper.display(), parent.display());
        assert_eq!(refused, expected);
        assert!(!parent.exists());
    }

    /// engine.md:585: a Remuxing job's rebuilt Remux promise is the plan's estimate on the library's volume whatever
    /// staging holds, so the owner thread reads no staged file; its Remux step replaces it (`refine_remux_promise`).
    /// Replaces `an_unreadable_staging_folder_rebuilds_the_remux_promise_from_the_plan`, with its record and settings.
    #[test]
    fn a_rebuilt_remux_promise_is_the_plans_estimate_and_reads_no_staged_file() {
        let plan = movie_plan();
        let s = Scratch::new("rebuild-estimate");
        let staging = s.0.join("staging");
        let settings = Settings {
            staging_root: Some(staging.to_str().unwrap().to_string()),
            movies_root: Some("/m".to_string()),
            ..Settings::default()
        };
        let record = JobRecord {
            format: JOB_FORMAT.to_string(), id: "a-1".to_string(), fingerprint: "AAA".to_string(), disc_id: None,
            drive: "tok0".to_string(), created_unix: 1, updated_unix: 1, state: State::Remuxing, message: None,
            rip_again: false, profile: "job.mmcp.xml".to_string(), plan: Some(plan.clone()),
            titles: Vec::<TitleRecord>::new(), filed_unix: None, collisions: vec![], notes: vec![],
        };
        let shares = FakeShares::new();
        for staged in [false, true] {
            if staged {
                s.write("staging/a-1/rip/t00-00800/title_t00.mkv", [0u8; 16]);
            }
            let mut promises = Promises::default();
            rebuild_promises(&mut promises, &shares, &record, &settings, &s.0);
            assert_eq!(promises.promised(&volume_of(&shares, "/m"), "b-1"), planned_remux_promise(&plan), "{staged}");
        }
    }

    #[test]
    fn the_remux_step_replaces_the_estimate_with_the_staged_sizes() {
        let shares = FakeShares::new();
        let library = volume_of(&shares, "/m");
        let promises = Mutex::new(Promises::default());
        promises.lock().unwrap().promise("a-1", PromisedStep::Remux, &library, 4301);
        let live = CancelToken::default();
        refine_remux_promise(Err("/s/a-1/rip/t00-00800: not found".to_string()), "/m", &shares, &promises, "a-1", &live);
        assert_eq!(promises.lock().unwrap().promised(&library, "b-1"), 4301);
        refine_remux_promise(Ok(16), "/m", &shares, &promises, "a-1", &live);
        assert_eq!(promises.lock().unwrap().promised(&library, "b-1"), remux_promise(16));
        promises.lock().unwrap().release("a-1", PromisedStep::Remux);
        let cancelled = CancelToken::default();
        cancelled.cancel();
        refine_remux_promise(Ok(16), "/m", &shares, &promises, "a-1", &cancelled);
        assert_eq!(promises.lock().unwrap().promised(&library, "b-1"), 0);
    }
}
