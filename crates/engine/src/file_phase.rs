//! The file phase's own parts (design parts 5.7, 6.4, 8.3, 8.4): the file-phase slot and its queue, the partial
//! file's path, the engine's `DecideInputs` for one title (K5), the library facts read at the namer's path, the
//! identity refreshed from the saved answers, and the waiting message of a pending lookup. The file phase's steps are
//! `RealSteps`' (steps.rs).

use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use lazymkv_clients::{Lookup, LookupBook, LookupState, PendingReason, TIMEOUT_S};
use lazymkv_core::job::State;
use lazymkv_core::naming::{edition_label, NamedPath};
use lazymkv_core::plan::movie::{LibraryFacts, THEATRICAL_CUT};
use lazymkv_core::plan::{refresh_identity, Plan};
use lazymkv_core::recording::files::{FfprobeStreams, MkvmergeIdent, Selection};
use lazymkv_core::recording::{read_text, text_mode, Encoding, Errors, Tree};
use lazymkv_core::reconcile::{reconcile, RipLog, RIP_LOG_NAMES};
use lazymkv_core::robot::parse;
use lazymkv_core::settings::{EditionStyle, Settings};
use lazymkv_dev::cmd::decide::{
    no_text_entry, read_decide_inputs, rec_folder, replay_answers, replay_remembered, DecideInputs, FileFacts,
    PlanEntry, RipLogText,
};
use lazymkv_dev::cmd::plan::{plan_doc, PlanOptions, PlanSource};
use lazymkv_dev::evidence::Ctx;

use crate::arrival::chain_inputs;
use crate::owner::JobContext;

/// The folder under a library root that holds the partial files (design part 5.7 step 13), where the self-test probes
/// a library root and a forced shutdown deletes a cancelled job's partial files (design part 3.8). Def: T24. Use: T24,
/// T26.
pub(crate) const PARTIAL_FOLDER: &str = ".lazymkv-partial";

/// `naming::resolution_label`'s values, the `<R>` of a file name the namer builds (Ruling R24.5).
const RESOLUTIONS: [&str; 5] = ["2160p", "1080p", "720p", "576p", "480p"];

/// The cut key of the theatrical or main cut in `LibraryFacts.earlier_edition_names` (spec 8.4).
const THEATRICAL_KEY: &str = "theatrical";

/// The file-phase slot, first in first out (design part 5.7): `capacity` places, the jobs holding one, and the jobs in
/// `Ejected { after_copy: false }` waiting for one, oldest first. Def: T24. Use: T24, T25, T26.
pub(crate) struct SlotQueue {
    /// `settings.file_phase_concurrency`.
    capacity: usize,
    /// The jobs holding a place, in the order they were granted it.
    holders: Vec<String>,
    /// The jobs waiting for a place, oldest first.
    queue: VecDeque<String>,
}

impl SlotQueue {
    /// No holder and no job waiting; `capacity` is `settings.file_phase_concurrency` (design part 5.7). Def: T24.
    /// Use: T24, T25.
    pub(crate) fn new(capacity: usize) -> SlotQueue {
        SlotQueue { capacity, holders: Vec::new(), queue: VecDeque::new() }
    }

    /// The slot as a restart finds it (design parts 5.7, 5.10, Ruling S72): `holders` hold their places whatever the
    /// capacity, as a wait entered after the grant keeps it, and `queue` waits in its order, oldest first. Def: T25.
    /// Use: T25.
    pub(crate) fn seed(&mut self, holders: &[String], queue: &[String]) {
        for holder in holders {
            self.grant(holder);
        }
        for queued in queue {
            self.join(queued);
        }
    }

    /// Sets the number of places to `capacity`, after a `settings_set` of `file_phase_concurrency` (design part 9.4);
    /// a holder keeps its place whatever the capacity. Def: T26. Use: T26.
    pub(crate) fn set_capacity(&mut self, capacity: usize) {
        self.capacity = capacity;
    }

    /// Appends `job` to the queue; a job already queued or holding a place keeps its place (design part 5.7).
    /// Def: T24. Use: T24, T25, T26.
    pub(crate) fn join(&mut self, job: &str) {
        if !self.holders.iter().any(|holder| holder == job) && !self.queue.iter().any(|queued| queued == job) {
            self.queue.push_back(job.to_string());
        }
    }

    /// Takes `job` out of the queue, and out of the place it holds if it holds one (design parts 5.7, 8.3: a job whose
    /// lookup is still pending after its round leaves the queue for AwaitingLookup). Def: T24. Use: T24, T26.
    pub(crate) fn leave(&mut self, job: &str) {
        self.queue.retain(|queued| queued != job);
        self.holders.retain(|holder| holder != job);
    }

    /// The oldest waiting job, only while a place is free (design part 5.7). Def: T24. Use: T24.
    pub(crate) fn next(&self) -> Option<String> {
        if self.holders.len() < self.capacity {
            self.queue.front().cloned()
        } else {
            None
        }
    }

    /// Moves `job` from the queue into a place (design part 5.7). Def: T24. Use: T24, T26.
    pub(crate) fn grant(&mut self, job: &str) {
        self.queue.retain(|queued| queued != job);
        if !self.holders.iter().any(|holder| holder == job) {
            self.holders.push(job.to_string());
        }
    }

    /// After a transition of `job` into `state`: a job that reached Filed, NeedsReview, Failed or Discarded leaves the
    /// slot, and the result is true when it held a place (or waited for one) and so freed it. A job keeps its place
    /// through any other state, a wait entered after the grant included (design part 5.7). Def: T24. Use: T24, T26.
    pub(crate) fn after(&mut self, job: &str, state: &State) -> bool {
        let done = matches!(state, State::Filed | State::NeedsReview { .. } | State::Failed { .. } | State::Discarded);
        if !done {
            return false;
        }
        let before = self.holders.len() + self.queue.len();
        self.leave(job);
        self.holders.len() + self.queue.len() != before
    }

    /// Whether any job holds a place, which keeps KeepAwake held (design parts 4.2, 5.7). Def: T24. Use: T24.
    pub(crate) fn held(&self) -> bool {
        !self.holders.is_empty()
    }
}

/// `<library root>/.lazymkv-partial/<job id>-<title key>.mkv.partial` (design part 5.7 step 13). Def: T24. Use: T24,
/// T25, T26.
pub(crate) fn partial_path(library_root: &str, job_id: &str, key: &str) -> PathBuf {
    Path::new(library_root).join(PARTIAL_FOLDER).join(format!("{job_id}-{key}.mkv.partial"))
}

/// The four replay files of a job folder as `lazymkv-dev plan` and `decide` take them (design part 7.1):
/// `settings.json`, `tmdb-search.json`, `tmdb-record.json` and `remembered.json`, each its absolute path in the job
/// folder, given only when the file exists.
fn replay_options(job_dir: &Path) -> PlanOptions {
    let given = |name: &str| Some(job_dir.join(name)).filter(|path| path.is_file());
    PlanOptions {
        settings: given("settings.json"),
        tmdb_search: given("tmdb-search.json"),
        tmdb_record: given("tmdb-record.json"),
        remembered: given("remembered.json"),
    }
}

/// The engine's DecideInputs for one title (K5, R-B1, design part 5.7 step 11.3), built from what it measured and read
/// as `read_decide_inputs` reads the same files: the rip label `<job dir>/<key>` through `rec_folder`, the planned
/// scan title with `Selection::All` (the one value both the inputs and `reconcile` take), scan.txt, the title's
/// mkvmerge.json and rip log, `reconcile`, no audio section, the subtitles section of a run with no OCR text, the plan
/// section of `plan_doc` over the job folder with its `.plan` replaced by `plan`, the plan the job decides with (RP-4),
/// the answers of `replay_answers` with the same replay files, the remembered decision of `replay_remembered` over the
/// job's own remembered.json (the copy the plan applied, never the data folder's file, which can change after Plan:
/// R-B1, the PR #6 review's V14), the probe the engine ran and no other file fact, and `library`. Err is the sentence
/// of a file that could not be read, or reconcile's. Def: T24. Use: T24, T25.
pub(crate) fn engine_decide_inputs(
    ctx: &Ctx,
    job: &JobContext,
    plan: &Plan,
    key: &str,
    probes: &FfprobeStreams,
    library: LibraryFacts,
) -> Result<DecideInputs, String> {
    let planned = plan.titles.iter().find(|title| title.key == key);
    let title = planned.ok_or_else(|| format!("the plan has no title {key}"))?.title;
    let (rec, label) = rec_folder(ctx, &job.job_dir.join(key).display().to_string()).map_err(|refused| refused.line)?;
    let tree: &dyn Tree = &rec.tree;
    let label_dir = rec.dir.join(&label);
    let scan = parse(&text_mode(&read_text(tree, &rec.dir.join("scan.txt"), Encoding::Utf8, Errors::Replace)?));
    let ident_path = label_dir.join("mkvmerge.json");
    let ident: MkvmergeIdent = serde_json::from_str(&read_text(tree, &ident_path, Encoding::Utf8Sig, Errors::Strict)?)
        .map_err(|error| format!("{}: {error}", tree.display(&ident_path)))?;
    let rip_log = match RIP_LOG_NAMES.iter().map(|name| label_dir.join(name)).find(|path| tree.exists(path)) {
        Some(path) => {
            let text = text_mode(&read_text(tree, &path, Encoding::Utf8, Errors::Replace)?);
            Some(RipLogText { shown: tree.display(&path), text })
        }
        None => None,
    };
    let log = rip_log.as_ref().map(|log| RipLog { shown: log.shown.clone(), text: log.text.as_str() });
    let report = reconcile(&scan, title, &ident, Selection::All, log)?;
    let options = replay_options(&job.job_dir);
    let plan_entry = match plan_doc(ctx, &PlanSource::Recording(&rec), &options) {
        Ok(mut found) => {
            found.plan = plan.clone();
            PlanEntry::Results(found)
        }
        Err(refused) => PlanEntry::Skipped { skipped: refused.line },
    };
    let answers = replay_answers(ctx, &rec, &options);
    let remembered = replay_remembered(ctx, &options);
    let file = FileFacts {
        level_md5: Ok(Vec::new()),
        probes: Ok(Some(probes.clone())),
        subs_index: Ok(None),
        packets: BTreeMap::new(),
    };
    Ok(DecideInputs {
        slug: job.record.id.clone(),
        label: key.to_string(),
        title: i64::from(title),
        selection: Selection::All,
        scan,
        ident,
        rip_log,
        reconcile: report,
        audio: Vec::new(),
        subtitles: no_text_entry(),
        plan: plan_entry,
        answers,
        remembered,
        file,
        library,
        overrides: None,
    })
}

/// The DecideInputs of a title whose probe the engine no longer holds, as after a restart (design part 5.10, PF-27):
/// `read_decide_inputs` over the rip label `<job dir>/<key>` with no OCR text, `Selection::All` and the replay files of
/// `engine_decide_inputs`, its plan section's `.plan` replaced by `plan` when the section has results (RP-4); the
/// readers' `replay_remembered` reads the same remembered.json. Err is the readers' refusal line. Def: T24. Use: T24,
/// T25.
pub(crate) fn read_back_inputs(ctx: &Ctx, job_dir: &Path, plan: &Plan, key: &str) -> Result<DecideInputs, String> {
    let (rec, label) = rec_folder(ctx, &job_dir.join(key).display().to_string()).map_err(|refused| refused.line)?;
    let options = replay_options(job_dir);
    let mut inputs =
        read_decide_inputs(ctx, &rec, &label, None, Some(Selection::All), &options).map_err(|refused| refused.line)?;
    if let PlanEntry::Results(found) = &mut inputs.plan {
        found.plan = plan.clone();
    }
    Ok(inputs)
}

/// What one file name in the film's folder says (Ruling R24.5).
enum Cut {
    /// `<F> - <R>.mkv`: a copy filed without an edition name.
    Plain,
    /// A copy filed under this edition name.
    Edition(String),
}

/// The cut a file the namer builds for the film folder `film` names under `style` (Ruling R24.5): `<F> - <R>.mkv` is
/// a plain copy, and `<F> - <R> <part>.mkv` an edition whose part is `E` (Jellyfin) or `E {edition-E}` (Combined), `E`
/// being neither empty nor ASCII digits only (a copy number) and equal to `edition_label(E)`. Plex files an edition in
/// its own folder, so a part names nothing there. None for any other name.
fn namer_cut(name: &str, film: &str, style: EditionStyle) -> Option<Cut> {
    let rest = name.strip_suffix(".mkv")?.strip_prefix(film)?.strip_prefix(" - ")?;
    let (resolution, part) = match rest.split_once(' ') {
        Some((resolution, part)) => (resolution, Some(part)),
        None => (rest, None),
    };
    if !RESOLUTIONS.contains(&resolution) {
        return None;
    }
    let Some(part) = part else {
        return Some(Cut::Plain);
    };
    let edition = match style {
        EditionStyle::Plex => return None,
        EditionStyle::Jellyfin => part,
        EditionStyle::Combined => {
            let (edition, tag) = part.split_once(" {edition-")?;
            if tag.strip_suffix('}')? != edition {
                return None;
            }
            edition
        }
    };
    let copy_number = edition.bytes().all(|byte| byte.is_ascii_digit());
    if edition.is_empty() || copy_number || edition_label(edition) != edition {
        return None;
    }
    Some(Cut::Edition(edition.to_string()))
}

/// LibraryFacts at the namer's path (design part 6.4): `path_collision` when `path.path` exists, and the edition facts
/// from one listing of the film's folder `<root>/<components[0]>` (Ruling R24.5); a missing folder gives no facts.
/// The names are read in byte order and the first wins a key: a plain copy gives `theatrical` the name "", and an
/// edition sets `folder_has_edition_named_file` and gives `theatrical` (for "Theatrical Cut") or its lower-cased name
/// the edition name. Def: T24. Use: T24.
pub(crate) fn library_facts(path: &NamedPath, style: EditionStyle) -> LibraryFacts {
    let mut facts = LibraryFacts { path_collision: Path::new(&path.path).exists(), ..LibraryFacts::default() };
    let Some(film) = path.components.first() else {
        return facts;
    };
    let Ok(entries) = fs::read_dir(Path::new(&path.root).join(film)) else {
        return facts;
    };
    let mut names: Vec<String> = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| !entry.path().is_dir())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    names.sort();
    for name in &names {
        match namer_cut(name, film, style) {
            Some(Cut::Plain) => {
                facts.earlier_edition_names.entry(THEATRICAL_KEY.to_string()).or_default();
            }
            Some(Cut::Edition(edition)) => {
                facts.folder_has_edition_named_file = true;
                let key = if edition == THEATRICAL_CUT { THEATRICAL_KEY.to_string() } else { edition.to_lowercase() };
                facts.earlier_edition_names.entry(key).or_insert(edition);
            }
            None => {}
        }
    }
    facts
}

/// The plan with its identity made again from the job's saved answers (design part 8.4, R24.2): `refresh_identity`
/// over the book's `lookup_inputs` with the disc name of `chain_inputs(job_dir)` and the job's settings, the book
/// locked only for the refresh. Err is chain_inputs' sentence. Def: T24. Use: T24.
pub(crate) fn refreshed_plan(
    plan: &Plan,
    job_dir: &Path,
    lookups: &Mutex<LookupBook>,
    settings: &Settings,
) -> Result<Plan, String> {
    let inputs = chain_inputs(job_dir)?;
    let book = lookups.lock().unwrap_or_else(PoisonError::into_inner);
    Ok(refresh_identity(plan, &book.lookup_inputs(inputs.name.as_ref()), settings))
}

/// A lookup's word in the waiting message and job.log (Ruling R24.6): `dischash`, `discid`, `search` or `tmdb record`.
/// Def: T24. Use: T24.
pub(crate) fn lookup_word(lookup: Lookup) -> &'static str {
    match lookup {
        Lookup::Dischash => "dischash",
        Lookup::Discid => "discid",
        Lookup::Search => "search",
        Lookup::Record => "tmdb record",
    }
}

/// Why a lookup is pending, in the words of Ruling R24.6.
fn reason_text(reason: &PendingReason) -> String {
    match reason {
        PendingReason::NoToken => "no tmdb_token is set".to_string(),
        PendingReason::TokenRefused => "TMDb refused the token (HTTP 401)".to_string(),
        PendingReason::Timeout => format!("no answer within {TIMEOUT_S} s"),
        PendingReason::Unreachable(text) => text.clone(),
        PendingReason::Status(status) => format!("HTTP {status}"),
        PendingReason::Unparsed(sentence) => sentence.clone(),
    }
}

/// The waiting message of a pending lookup (design part 8.3): each pending lookup in `Lookup` order as
/// `<word>: <reason>`, joined by "; ", the word being `dischash`, `discid`, `search` or `tmdb record` (Ruling R24.6).
/// Def: T24. Use: T24, T26.
pub(crate) fn waiting_message(book: &LookupBook) -> String {
    let pending = book.states.iter().filter_map(|(lookup, state)| match state {
        LookupState::Pending { reason } => Some(format!("{}: {}", lookup_word(*lookup), reason_text(reason))),
        _ => None,
    });
    pending.collect::<Vec<String>>().join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use lazymkv_core::naming::join;
    use lazymkv_core::testsupport::{Scratch, ScratchExt};

    #[test]
    fn the_slot_is_granted_to_the_oldest_ejected_job() {
        let mut slots = SlotQueue::new(1);
        slots.join("b-1");
        slots.join("a-1");
        slots.join("b-1");
        assert_eq!(slots.next(), Some("b-1".to_string()));
        slots.grant("b-1");
        assert_eq!(slots.next(), None);
        assert!(!slots.after("b-1", &State::Remuxing));
        assert!(slots.after("b-1", &State::Filed));
        assert_eq!(slots.next(), Some("a-1".to_string()));
    }

    #[test]
    fn the_slot_is_held_through_a_wait_entered_after_the_grant() {
        let mut slots = SlotQueue::new(1);
        slots.join("a-1");
        slots.grant("a-1");
        assert!(!slots.after("a-1", &State::AwaitingSpace { return_to: Box::new(State::Decided { approved: None }) }));
        assert!(slots.after("a-1", &State::NeedsReview { held: lazymkv_core::job::Held::DispositionGate }));
    }

    #[test]
    fn a_partial_file_is_named_by_job_and_title_key() {
        assert_eq!(partial_path("/lib/Movies", "abc-1", "t00-00800"), std::path::Path::new("/lib/Movies/.lazymkv-partial/abc-1-t00-00800.mkv.partial"));
    }

    fn film_path(root: &std::path::Path) -> NamedPath {
        let folder = "Synthetic Movie (2001) {tmdb-900001}".to_string();
        let components = vec![folder.clone(), format!("{folder} - 1080p.mkv")];
        let root = root.to_str().unwrap().to_string();
        NamedPath { path: join(&root, &components), root, components }
    }

    #[test]
    fn an_edition_named_file_in_the_film_folder_sets_folder_has_edition_named_file() {
        let s = Scratch::new("edition-facts");
        let path = film_path(&s.0);
        let folder = path.components[0].clone();
        s.write(&format!("{folder}/{folder} - 2160p Extended Cut.mkv"), "x");
        s.write(&format!("{folder}/{folder} - 1080p 2.mkv"), "x");
        s.write(&format!("{folder}/poster.jpg"), "x");
        assert_eq!(
            library_facts(&path, EditionStyle::Jellyfin),
            LibraryFacts {
                folder_has_edition_named_file: true,
                earlier_edition_names: [("extended cut".to_string(), "Extended Cut".to_string())].into(),
                path_collision: false,
            }
        );
        assert_eq!(library_facts(&path, EditionStyle::Combined), LibraryFacts::default());
    }

    #[test]
    fn an_earlier_copy_without_an_edition_is_the_theatrical_cut_and_a_collision() {
        let s = Scratch::new("theatrical-facts");
        let path = film_path(&s.0);
        let folder = path.components[0].clone();
        s.write(&format!("{folder}/{}", path.components[1]), "x");
        s.write(&format!("{folder}/{folder} - 2160p Theatrical Cut {{edition-Theatrical Cut}}.mkv"), "x");
        let facts = library_facts(&path, EditionStyle::Combined);
        assert!(facts.path_collision && facts.folder_has_edition_named_file);
        assert_eq!(facts.earlier_edition_names, [("theatrical".to_string(), String::new())].into());
    }

    #[test]
    fn a_missing_film_folder_gives_no_facts() {
        let s = Scratch::new("no-film-folder");
        assert_eq!(library_facts(&film_path(&s.0), EditionStyle::Jellyfin), LibraryFacts::default());
    }

    #[test]
    fn the_waiting_message_names_each_pending_lookup_and_its_reason() {
        let mut book = LookupBook::default();
        book.states.insert(Lookup::Dischash, LookupState::Pending { reason: PendingReason::Timeout });
        book.states.insert(Lookup::Discid, LookupState::Answered { status: 404 });
        book.states.insert(Lookup::Search, LookupState::Pending { reason: PendingReason::TokenRefused });
        book.states.insert(Lookup::Record, LookupState::Pending { reason: PendingReason::Status(503) });
        assert_eq!(
            waiting_message(&book),
            "dischash: no answer within 10 s; search: TMDb refused the token (HTTP 401); tmdb record: HTTP 503"
        );
        book.states.insert(Lookup::Dischash, LookupState::Pending { reason: PendingReason::Unparsed("x: bad".to_string()) });
        book.states.insert(Lookup::Search, LookupState::Pending { reason: PendingReason::NoToken });
        book.states.remove(&Lookup::Record);
        assert_eq!(waiting_message(&book), "dischash: x: bad; search: no tmdb_token is set");
    }
}
