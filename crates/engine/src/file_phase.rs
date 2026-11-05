//! The file phase's own parts (design parts 5.7, 6.4, 8.3, 8.4): the file-phase slot and its queue, the partial
//! file's path, the engine's `DecideInputs` for one title (K5), the library facts read at the namer's path, the
//! identity refreshed from the saved answers, the waiting message of a pending lookup, and the refetch of a job's
//! stale TMDb copies. The file phase's steps are `RealSteps`' (steps.rs).

use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use lazymkv_clients::{
    reason_text, refetch_record, refetch_search, stale_copies, status_record, HttpTransport, Lookup, LookupBook,
    LookupState, PendingReason, SavedFile, StaleCopy,
};
use lazymkv_core::job::State;
use lazymkv_core::lookup::search_queries;
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
use lazymkv_platform::write_atomic;
use serde_json::Value;

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
/// folder, given only when the file exists. The `command` relay gives the child the same files (design part 9.5).
/// Def: T24. Use: T24, T3-22.
pub(crate) fn replay_options(job_dir: &Path) -> PlanOptions {
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
/// the edition name.
///
/// Changed (Ruling W22, R-B1; engine.md:591): every read error but NotFound is Err(`the library folder <folder> could
/// not be read: <error>`), `<folder>` being the film's folder `<root>/<components[0]>` (or `path.path` without
/// components), for the final path's metadata, the folder's listing, an entry of it and an entry's metadata. NotFound
/// reads as absent, as before, so inputs that read without error give the same facts and decide's output is unchanged.
/// Def: T24, its errors T3-18. Use: T24.
pub(crate) fn library_facts(path: &NamedPath, style: EditionStyle) -> Result<LibraryFacts, String> {
    let folder = match path.components.first() {
        Some(film) => Path::new(&path.root).join(film),
        None => PathBuf::from(&path.path),
    };
    let unread = |error: io::Error| format!("the library folder {} could not be read: {error}", folder.display());
    let path_collision = match fs::metadata(&path.path) {
        Ok(_) => true,
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(error) => return Err(unread(error)),
    };
    let mut facts = LibraryFacts { path_collision, ..LibraryFacts::default() };
    let Some(film) = path.components.first() else {
        return Ok(facts);
    };
    let entries = match fs::read_dir(&folder) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(facts),
        Err(error) => return Err(unread(error)),
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(unread)?;
        let is_dir = match fs::metadata(entry.path()) {
            Ok(meta) => meta.is_dir(),
            Err(error) if error.kind() == io::ErrorKind::NotFound => false,
            Err(error) => return Err(unread(error)),
        };
        if is_dir {
            continue;
        }
        if let Ok(name) = entry.file_name().into_string() {
            names.push(name);
        }
    }
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
    Ok(facts)
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

/// A lookup's word in the waiting message and job.log, the first word of its status line except the record's
/// (Rulings R24.6, T4-R1): `dischash`, `discid`, `search`, `search-tagged`, `search-entry`, `tmdb record`,
/// `season-<n>`. Def: T4-2. Use: T4-3, T4-4.
pub(crate) fn lookup_word(lookup: Lookup) -> String {
    let word = match lookup {
        Lookup::Dischash => "dischash",
        Lookup::Discid => "discid",
        Lookup::Search => "search",
        Lookup::TaggedSearch => "search-tagged",
        Lookup::EntrySearch => "search-entry",
        Lookup::Record => "tmdb record",
        Lookup::Season(n) => return format!("season-{n}"),
    };
    word.to_string()
}

/// The waiting message of a pending lookup (design part 8.3): each pending lookup in `Lookup` order as
/// `<word>: <reason>`, joined by "; ", the word being `lookup_word`'s and the reason `lazymkv_clients::reason_text`'s
/// (Ruling R24.6).
/// Def: T24. Use: T24, T26.
pub(crate) fn waiting_message(book: &LookupBook) -> String {
    let pending = book.states.iter().filter_map(|(lookup, state)| match state {
        LookupState::Pending { reason } => Some(format!("{}: {}", lookup_word(*lookup), reason_text(reason))),
        _ => None,
    });
    pending.collect::<Vec<String>>().join("; ")
}

/// The two copies a record fetch writes (design part 8.4).
const RECORD_COPIES: [&str; 2] = ["tmdb.json", "tmdb-record.json"];
/// The search's copy (design part 8.4).
const SEARCH_COPY: &str = "tmdb-search.json";

/// Whether the TMDb copy `file` keeps `lookup`'s answer (Ruling T4-R8): `tmdb.json` and `tmdb-record.json` keep the
/// record's and each season's, `tmdb-search.json` the three searches'. Def: T4-9. Use: T4-9.
pub(crate) fn copy_holds(file: &str, lookup: Lookup) -> bool {
    match lookup {
        Lookup::Record | Lookup::Season(_) => RECORD_COPIES.contains(&file),
        Lookup::Search | Lookup::TaggedSearch | Lookup::EntrySearch => file == SEARCH_COPY,
        Lookup::Dischash | Lookup::Discid => false,
    }
}

/// Why a stale TMDb copy was not fetched again, as a sentence (design part 8.4; Ruling T4-R9): `<file>: the TMDb copy
/// is more than six months old (fetched_at <date>) and could not be fetched again: <reason>` for a copy with a date;
/// `has no fetched_at date` in place of the age for a copy present without one; `was deleted` for a copy no longer in
/// `job_dir`. The reason is `reason_text`'s. Def: T4-9. Use: T4-9.
pub(crate) fn stale_sentence(job_dir: &Path, copy: &StaleCopy, reason: &PendingReason) -> String {
    let file = copy.file;
    let why = match &copy.fetched_at {
        Some(date) => format!("is more than six months old (fetched_at {date})"),
        None if job_dir.join(file).exists() => "has no fetched_at date".to_string(),
        None => "was deleted".to_string(),
    };
    format!("{file}: the TMDb copy {why} and could not be fetched again: {}", reason_text(reason))
}

/// Writes `files` into `job_dir` with `write_atomic`, in order, stopping at the first that fails; Err is that copy's
/// reason, `<path>: the TMDb copy could not be written: <error>`.
fn write_copies(job_dir: &Path, files: &[SavedFile]) -> Result<(), PendingReason> {
    for file in files {
        let path = job_dir.join(&file.name);
        if let Err(error) = write_atomic(&path, &file.bytes) {
            let sentence = format!("{}: the TMDb copy could not be written: {error}", path.display());
            return Err(PendingReason::Unparsed(sentence));
        }
    }
    Ok(())
}

/// A JSON file's value after one byte order mark, or None when it cannot be read or is not JSON.
fn read_json(path: &Path) -> Option<Value> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(text.strip_prefix('\u{feff}').unwrap_or(&text)).ok()
}

/// Every state of the lookups `file` holds that is `Pending { Stale(_) }` becomes `Answered { status: 200 }`.
fn answer_stale(book: &mut LookupBook, file: &str) {
    for (lookup, state) in book.states.iter_mut() {
        if copy_holds(file, *lookup) && matches!(state, LookupState::Pending { reason: PendingReason::Stale(_) }) {
            *state = LookupState::Answered { status: 200 };
        }
    }
}

/// The job's stale TMDb copies fetched again (design parts 8.4, 5.10, 9.5; Rulings T4-R8, T4-R9), before Plan, Decide
/// and the relay's child read them. The copies are `stale_copies` over `lookup-status.txt` (empty when absent) in a job
/// folder on `today`; none asks nothing and gives Ok(vec![]). A listed `tmdb.json` or `tmdb-record.json` is fetched
/// once for both with `refetch_record`, by the plan's TMDb identity, else the status file's record (`status_record`),
/// and the season of `chain_inputs`' disc name when that reads; no identity fails both with `no TMDb ID names the
/// record`. A listed `tmdb-search.json` is fetched with `refetch_search` over the file's `queries`, else
/// `search_queries` over the book. Each fetched file is written with `write_atomic` (a record fetch writes both of its
/// files, and one that cannot be written fails the copies of its fetch). The book is locked only to read the queries
/// and to install an answer, never across a request: the record and `record_file`, or the page into `answers.entry`
/// when the status holds `search-entry 200`, else `answers.search` (and `answers.tagged` too when the status holds
/// `search-tagged 200`), each `Pending { Stale(_) }` state of that file's
/// lookups becoming `Answered { status: 200 }`. A copy that failed sets its file's lookups present in the book to
/// `Pending { Stale(<first failed file of its fetch>) }`, so the next round asks them again. Ok holds the listed copies
/// fetched again, in `TMDB_COPIES` order; Err joins a `stale_sentence` per copy not fetched again with "; ".
/// Def: T4-9. Use: T4-9.
pub(crate) fn refetch_stale(
    job_dir: &Path,
    plan: Option<&Plan>,
    lookups: &Mutex<LookupBook>,
    http: &dyn HttpTransport,
    token: Option<&str>,
    today: &str,
) -> Result<Vec<String>, String> {
    let status = fs::read_to_string(job_dir.join("lookup-status.txt")).unwrap_or_default();
    let copies = stale_copies(&status, &|file| fs::read(job_dir.join(file)).ok(), true, today);
    if copies.is_empty() {
        return Ok(Vec::new());
    }
    let lock = || lookups.lock().unwrap_or_else(PoisonError::into_inner);
    let name = chain_inputs(job_dir).ok().and_then(|inputs| inputs.name);
    let listed = |file: &str| copies.iter().any(|copy| copy.file == file);
    let mut failed: BTreeMap<&str, PendingReason> = BTreeMap::new();
    if RECORD_COPIES.iter().any(|file| listed(file)) {
        let identity = plan.and_then(|plan| plan.identity.tmdb).or_else(|| status_record(&status));
        let season = name.as_ref().and_then(|name| name.season);
        let fetched = match identity {
            None => Err(PendingReason::Unparsed("no TMDb ID names the record".to_string())),
            Some(id) => refetch_record(http, token, id, season, today),
        };
        let fetched = fetched.and_then(|(record, files)| write_copies(job_dir, &files).map(|()| (record, files)));
        match fetched {
            Ok((record, files)) => {
                let saved = files.iter().find(|file| file.name == "tmdb-record.json");
                let record_file = saved.and_then(|file| serde_json::from_slice(&file.bytes).ok());
                let mut book = lock();
                book.answers.record = Some(record);
                book.record_file = record_file;
                answer_stale(&mut book, "tmdb-record.json");
            }
            Err(reason) => {
                for file in RECORD_COPIES.into_iter().filter(|file| listed(file)) {
                    failed.insert(file, reason.clone());
                }
            }
        }
    }
    if listed(SEARCH_COPY) {
        let saved = read_json(&job_dir.join(SEARCH_COPY));
        let saved = saved.as_ref().and_then(|saved| saved["queries"].as_array()).map(Vec::as_slice).unwrap_or_default();
        let mut queries: Vec<String> = saved.iter().filter_map(|query| query.as_str().map(str::to_string)).collect();
        if queries.is_empty() {
            queries = search_queries(&lock().lookup_inputs(name.as_ref()));
        }
        let fetched = refetch_search(http, token, &queries, today)
            .and_then(|(page, file)| write_copies(job_dir, std::slice::from_ref(&file)).map(|()| page));
        match fetched {
            Ok(page) => {
                let answered = |key: &str| {
                    status.lines().any(|line| {
                        let words: Vec<&str> = line.split_whitespace().collect();
                        words.first() == Some(&key) && words.last() == Some(&"200")
                    })
                };
                let mut book = lock();
                if answered("search-entry") {
                    book.answers.entry = Some(page);
                } else {
                    // The page merges both queries, and merge_pages of a page with itself is the page (J8), as
                    // rebuild_book reads a `search-tagged 200` line.
                    if answered("search-tagged") {
                        book.answers.tagged = Some(page.clone());
                    }
                    book.answers.search = Some(page);
                }
                answer_stale(&mut book, SEARCH_COPY);
            }
            Err(reason) => {
                failed.insert(SEARCH_COPY, reason);
            }
        }
    }
    if failed.is_empty() {
        return Ok(copies.iter().map(|copy| copy.file.to_string()).collect());
    }
    {
        let mut book = lock();
        let mut marked: BTreeMap<Lookup, &str> = BTreeMap::new();
        for copy in copies.iter().filter(|copy| failed.contains_key(copy.file)) {
            for lookup in book.states.keys().filter(|lookup| copy_holds(copy.file, **lookup)) {
                marked.entry(*lookup).or_insert(copy.file);
            }
        }
        for (lookup, file) in marked {
            book.states.insert(lookup, LookupState::Pending { reason: PendingReason::Stale(file.to_string()) });
        }
    }
    let sentences: Vec<String> = copies
        .iter()
        .filter_map(|copy| failed.get(copy.file).map(|reason| stale_sentence(job_dir, copy, reason)))
        .collect();
    Err(sentences.join("; "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lazymkv_clients::{PendingReason, RecordedTransport};
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
            library_facts(&path, EditionStyle::Jellyfin).unwrap(),
            LibraryFacts {
                folder_has_edition_named_file: true,
                earlier_edition_names: [("extended cut".to_string(), "Extended Cut".to_string())].into(),
                path_collision: false,
            }
        );
        assert_eq!(library_facts(&path, EditionStyle::Combined).unwrap(), LibraryFacts::default());
    }

    #[test]
    fn an_earlier_copy_without_an_edition_is_the_theatrical_cut_and_a_collision() {
        let s = Scratch::new("theatrical-facts");
        let path = film_path(&s.0);
        let folder = path.components[0].clone();
        s.write(&format!("{folder}/{}", path.components[1]), "x");
        s.write(&format!("{folder}/{folder} - 2160p Theatrical Cut {{edition-Theatrical Cut}}.mkv"), "x");
        let facts = library_facts(&path, EditionStyle::Combined).unwrap();
        assert!(facts.path_collision && facts.folder_has_edition_named_file);
        assert_eq!(facts.earlier_edition_names, [("theatrical".to_string(), String::new())].into());
    }

    #[test]
    fn a_missing_film_folder_gives_no_facts() {
        let s = Scratch::new("no-film-folder");
        assert_eq!(library_facts(&film_path(&s.0), EditionStyle::Jellyfin).unwrap(), LibraryFacts::default());
    }

    /// Ruling W22: a film folder that cannot be listed is an error naming it, not a folder holding nothing. A regular
    /// file stands where the folder goes, so the read fails on every system with an error other than NotFound.
    #[test]
    fn a_film_folder_that_cannot_be_listed_is_an_error_naming_it() {
        let s = Scratch::new("unlistable-film");
        let path = film_path(&s.0);
        let folder = s.0.join(&path.components[0]);
        std::fs::write(&folder, b"").unwrap();
        let error = library_facts(&path, EditionStyle::Jellyfin).unwrap_err();
        assert!(error.starts_with(&format!("the library folder {} could not be read: ", folder.display())), "{error}");
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

    #[test]
    fn the_waiting_message_names_a_tv_record_season() {
        let mut book = LookupBook::default();
        book.states.insert(Lookup::Season(2), LookupState::Pending { reason: PendingReason::Timeout });
        assert_eq!(lookup_word(Lookup::Season(2)), "season-2");
        assert_eq!(waiting_message(&book), "season-2: no answer within 10 s");
    }

    /// A slim copy of movie 900001 fetched on `date`.
    fn movie_copy(date: &str) -> String {
        format!(r#"{{"id": 900001, "title": "Synthetic Movie", "original_title": "Synthetic Movie", "release_date": "2001-02-03", "original_language": "en", "kind": "movie", "runtime": 119, "fetched_at": "{date}"}}"#)
    }

    /// A job folder whose status says the search and movie 900001 answered 200, with a slim tmdb-search.json fetched
    /// 2026-10-01 and each `(file, fetched_at)` of `copies` as a slim copy of movie 900001.
    fn stale_folder(copies: &[(&str, &str)]) -> Scratch {
        let s = Scratch::new("stale-folder");
        s.write("lookup-status.txt", "dischash 404\nsearch 200\ntmdb movie/900001 200\n");
        let page = r#"{"results": [{"media_type": "movie", "id": 900001, "title": "Synthetic Movie", "original_title": "Synthetic Movie", "release_date": "2001-02-03"}], "queries": ["Synthetic Movie"], "fetched_at": "2026-10-01"}"#;
        s.write("tmdb-search.json", page);
        for (file, date) in copies {
            s.write(file, movie_copy(date));
        }
        s
    }

    fn lock(book: &Mutex<LookupBook>) -> std::sync::MutexGuard<'_, LookupBook> {
        book.lock().unwrap_or_else(PoisonError::into_inner)
    }

    #[test]
    fn refetch_stale_of_fresh_copies_asks_nothing() {
        let s = stale_folder(&[("tmdb.json", "2026-10-01"), ("tmdb-record.json", "2026-10-01")]);
        let t = RecordedTransport::new();
        assert_eq!(refetch_stale(&s.0, None, &Mutex::default(), &t, None, "2026-10-23"), Ok(vec![]));
        assert!(t.requests().is_empty());
    }

    #[test]
    fn refetch_stale_names_each_copy_it_could_not_fetch() {
        let s = stale_folder(&[("tmdb.json", "2026-01-01")]); // tmdb-record.json deleted
        let (t, book) = (RecordedTransport::new(), Mutex::new(LookupBook::default()));
        lock(&book).states.insert(Lookup::Record, LookupState::Answered { status: 200 });
        let error = refetch_stale(&s.0, None, &book, &t, None, "2026-10-23").unwrap_err();
        assert_eq!(error, [
            "tmdb.json: the TMDb copy is more than six months old (fetched_at 2026-01-01) and could not be fetched again: no tmdb_token is set",
            "tmdb-record.json: the TMDb copy was deleted and could not be fetched again: no tmdb_token is set",
        ].join("; "));
        assert_eq!(lock(&book).states[&Lookup::Record], LookupState::Pending { reason: PendingReason::Stale("tmdb.json".into()) });
        assert!(t.requests().is_empty());
    }

    /// A /search/multi first page holding the one movie `id`.
    fn one_result_page(id: i64, title: &str) -> String {
        format!(r#"{{"page": 1, "results": [{{"media_type": "movie", "id": {id}, "title": "{title}", "original_title": "{title}", "release_date": "2010-08-20"}}], "total_pages": 1, "total_results": 1}}"#)
    }

    #[test]
    fn a_refetched_search_page_fills_the_tagged_answer_when_its_line_says_200() {
        let s = Scratch::new("stale-tagged");
        s.write("lookup-status.txt", "dischash 404\nsearch 200\nsearch-tagged 200\n");
        let page = r#"{"results": [{"media_type": "movie", "id": 900101, "title": "Piranha", "original_title": "Piranha", "release_date": "1978-08-03"}], "queries": ["PIRANHA", "PIRANHA 3D"], "fetched_at": "2026-01-01"}"#;
        s.write("tmdb-search.json", page);
        let t = RecordedTransport::new();
        t.answer(&lazymkv_clients::search_url("PIRANHA"), 200, one_result_page(900101, "Piranha").as_bytes());
        t.answer(&lazymkv_clients::search_url("PIRANHA 3D"), 200, one_result_page(900103, "Piranha 3D").as_bytes());
        let book = Mutex::new(LookupBook::default());
        for lookup in [Lookup::Search, Lookup::TaggedSearch] {
            let stale = LookupState::Pending { reason: PendingReason::Stale("tmdb-search.json".into()) };
            lock(&book).states.insert(lookup, stale);
        }
        let fetched = refetch_stale(&s.0, None, &book, &t, Some("token-sentinel-5d2e"), "2026-10-23");
        assert_eq!(fetched, Ok(vec!["tmdb-search.json".to_string()]));
        let book = lock(&book);
        assert_eq!(book.states[&Lookup::Search], LookupState::Answered { status: 200 });
        assert_eq!(book.states[&Lookup::TaggedSearch], LookupState::Answered { status: 200 });
        assert!(book.answers.search.is_some());
        assert_eq!(book.answers.tagged, book.answers.search);
        assert_eq!(book.answers.entry, None);
        let saved: Value = serde_json::from_slice(&std::fs::read(s.0.join("tmdb-search.json")).unwrap()).unwrap();
        assert_eq!(saved["queries"], serde_json::json!(["PIRANHA", "PIRANHA 3D"]));
    }

    #[test]
    fn a_search_answered_http_429_waits_with_http_429() {
        let mut book = LookupBook::default();
        book.states.insert(Lookup::Search, LookupState::Pending { reason: PendingReason::Status(429) });
        assert_eq!(waiting_message(&book), "search: HTTP 429");
    }
}
