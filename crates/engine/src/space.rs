//! Design part 5.8's space checks: the promises of unfinished steps, by volume, and the Direct rip's check. A check
//! adds, on each volume it reads, this job's later steps there and what other jobs' unfinished steps were promised.

use std::collections::BTreeMap;

use lazymkv_core::job::Event;
use lazymkv_core::plan::{staging_bytes_needed, write_bytes_needed, Plan};
use lazymkv_platform::{Reach, ShareConnector};

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

/// `shares.volume(root)`, else the root itself (S11, design part 6.1). Def: T23. Use: T23, T24.
pub(crate) fn volume_of(shares: &dyn ShareConnector, root: &str) -> String {
    shares.volume(root).unwrap_or_else(|| root.to_string())
}

/// The Direct rip's check (design part 5.8, S54): an unreachable staging or library root gives ShareLost; staging must
/// hold `rip_bytes`, plus `remux_bytes` when the library shares its volume, plus what other jobs were promised there; a
/// library on its own volume must hold `remux_bytes` plus what other jobs were promised there. A `free_bytes` error
/// gives ShareLost and a short volume SpaceShort. Otherwise the job is promised Rip on staging and Remux on the
/// library (on staging when no library is set), and the check gives SpaceOk. Def: T23. Use: T23.
pub(crate) fn rip_check(
    rip_bytes: u64,
    remux_bytes: u64,
    staging: &str,
    library: Option<&str>,
    shares: &dyn ShareConnector,
    promises: &mut Promises,
    job: &str,
) -> Event {
    let roots = std::iter::once(staging).chain(library);
    if roots.clone().any(|root| matches!(shares.reach(root), Reach::Unreachable(_))) {
        return Event::ShareLost;
    }
    let staging_volume = volume_of(shares, staging);
    let library_volume = library.map(|root| volume_of(shares, root));
    let shared = library_volume.as_deref().is_none_or(|volume| volume == staging_volume);
    let staging_needs = rip_bytes
        .saturating_add(if shared && library.is_some() { remux_bytes } else { 0 })
        .saturating_add(promises.promised(&staging_volume, job));
    let mut checks = vec![(staging, staging_needs)];
    if let (Some(root), Some(volume), false) = (library, library_volume.as_deref(), shared) {
        checks.push((root, remux_bytes.saturating_add(promises.promised(volume, job))));
    }
    for (root, needs) in checks {
        match shares.free_bytes(root) {
            Err(_) => return Event::ShareLost,
            Ok(free) if free < needs => return Event::SpaceShort,
            Ok(_) => {}
        }
    }
    promises.promise(job, PromisedStep::Rip, &staging_volume, rip_bytes);
    let remux_volume = library_volume.unwrap_or(staging_volume);
    promises.promise(job, PromisedStep::Remux, &remux_volume, remux_bytes);
    Event::SpaceOk
}

/// The Rip promise on staging: `staging_bytes_needed(plan, disc_size)` (plan/mod.rs; design part 5.8). Def: T23.
/// Use: T23, T25.
pub(crate) fn rip_promise(plan: &Plan, disc_size: u64) -> u64 {
    staging_bytes_needed(plan, disc_size)
}

/// The rip check's Remux promise on the library: `write_bytes_needed` of the planned titles' sizes (S54, design part
/// 5.8). Def: T23. Use: T23.
pub(crate) fn planned_remux_promise(plan: &Plan) -> u64 {
    write_bytes_needed(plan.titles.iter().map(|title| title.size.unwrap_or(0)).fold(0, u64::saturating_add))
}

/// The remux check's Remux promise: `write_bytes_needed(staged_bytes)`, the staged files' sizes (S54, design part
/// 5.8). Def: T23. Use: T24, T25.
#[expect(dead_code, reason = "Task 24's remux check promises the staged files' sizes")]
pub(crate) fn remux_promise(staged_bytes: u64) -> u64 {
    write_bytes_needed(staged_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lazymkv_platform::fakes::FakeShares;

    #[test]
    fn a_rip_check_on_a_volume_shared_with_the_library_counts_its_own_remux() {
        let shares = FakeShares::new();
        shares.same_volume("/s", "/m");
        shares.set_free("/s", 8601);
        shares.set_free("/m", 8601);
        let mut promises = Promises::default();
        assert_eq!(rip_check(4301, 4301, "/s", Some("/m"), &shares, &mut promises, "a-1"), Event::SpaceShort);
        shares.set_free("/s", 8602);
        shares.set_free("/m", 8602);
        assert_eq!(rip_check(4301, 4301, "/s", Some("/m"), &shares, &mut promises, "a-1"), Event::SpaceOk);
        assert_eq!(promises.promised(&volume_of(&shares, "/s"), "other"), 8602);
    }

    #[test]
    fn another_jobs_promised_remux_counts_against_this_rip() {
        let shares = FakeShares::new();
        shares.same_volume("/s", "/m");
        let mut promises = Promises::default();
        promises.promise("a-1", PromisedStep::Remux, &volume_of(&shares, "/m"), 4301);
        shares.set_free("/s", 12902);
        shares.set_free("/m", 12902);
        assert_eq!(rip_check(4301, 4301, "/s", Some("/m"), &shares, &mut promises, "b-1"), Event::SpaceShort);
        shares.set_free("/s", 12903);
        shares.set_free("/m", 12903);
        assert_eq!(rip_check(4301, 4301, "/s", Some("/m"), &shares, &mut promises, "b-1"), Event::SpaceOk);
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
}
