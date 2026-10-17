//! `lazymkv-dev plan <slug|recording dir|scan file> [--evidence DIR] [--settings FILE] [--tmdb-search FILE]
//! [--tmdb-record FILE] [--remembered FILE]` (ruling RP14 of the phase 1 plan; port spec 3.4): the disc phase of the
//! design spec's 6.1 step 5 for one recording or one pinned TheDiscDB scan log, printed as one JSON document,
//! `PlanDoc`: the slug or path, the inputs found, the settings in force (spec 14) with their problems, and the `Plan`
//! of `lazymkv_core::plan::plan` (the identity of 8.1, the kind of 8.2, the titles of 8.3 to 8.5, the mode of 6.2 and
//! 6.3, the identity and title confidences and the review reasons of 8.8). A recording is read from its scan.txt,
//! meta.json, nav/BDMV/META/DL/bdmt_*.xml, thediscdb-hash.json, thediscdb-discid.json, tmdb.json and
//! lookup-status.txt (spec 15 "Recordings"); a file that is absent means that lookup was not made, an answer file
//! whose lookup-status.txt line is not 200 is not parsed (the recorder keeps the error body of a 404 or a failed
//! lookup), and only a file that exists and does not parse is refused. A bare scan file (a pinned log of corpus/thediscdb) takes its
//! fingerprint from its HSH lines (spec 5) and its TheDiscDB answer from corpus/thediscdb-api under the repository
//! root, else from the discNN.json beside it, and has no TMDb answer. Exit 0 when the plan is printed, with or without
//! review reasons or settings problems (a plan that needs review is still a plan); 2 for an argument error, a scan that
//! cannot be read, typed or planned, a lookup, settings, TMDb or remembered file that exists and cannot be read or does
//! not parse, or a remembered decision for another disc; 3 when a slug is given and the evidence clone is absent;
//! never 1 (ruling RP24). Defined by Task 15;
//! `lazymkv-dev decide`'s `plan` section (Task 16) calls `plan_doc`.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use lazymkv_core::disc::{bdmt_candidates, bdmt_name, disc, Disc};
use lazymkv_core::fingerprint::{content_hash, hashed_files};
use lazymkv_core::lookup::discdb::{parse_dischash, parse_discid, DiscDbRecord};
use lazymkv_core::lookup::name::disc_name;
use lazymkv_core::lookup::tmdb::{parse_movie, parse_recorded, parse_search, parse_tv, TmdbRecord, TmdbSearch};
use lazymkv_core::lookup::LookupInputs;
use lazymkv_core::plan::movie::LibraryFacts;
use lazymkv_core::plan::{plan, Plan, PlanInputs};
use lazymkv_core::recording::files::Meta;
use lazymkv_core::recording::{pathlib_str_beside, read_json, read_text, text_mode, Encoding, Errors, PathKey, Tree};
use lazymkv_core::remembered::Remembered;
use lazymkv_core::robot::parse;
use lazymkv_core::settings::{Settings, SettingsProblem};

use crate::cmd::hit_rate::{lookup as lookup_answer, statuses, FAILED, HIT, MISS, NOT_MADE};
use crate::evidence::{pathlib_split, read_file_text, repo_root, resolve_recording, Ctx, Recording, Refused};
use crate::pyjson::{dumps, Style};
use crate::{usage_error, Args};

/// The recording's files the command reads (design spec section 15, "Recordings").
const SCAN_FILE: &str = "scan.txt";
const META_FILE: &str = "meta.json";
const STATUS_FILE: &str = "lookup-status.txt";
const DISCHASH_FILE: &str = "thediscdb-hash.json";
const DISCID_FILE: &str = "thediscdb-discid.json";
const TMDB_FILE: &str = "tmdb.json";
/// Where a recording's nav tree keeps the bdmt files (spec 6.1 step 2, 8.1).
const BDMT_DIR: &str = "nav/BDMV/META/DL";
/// TheDiscDB's live answers below the repository root, `dischash-<fingerprint>.json` (Open point 26), and how the
/// document names them whatever the root is.
const API_DIR: &str = "corpus/thediscdb-api";

/// One lookup file the command looked for (spec 15 "Recordings": thediscdb-hash.json, thediscdb-discid.json,
/// tmdb.json; for a pinned log its TheDiscDB answer): its displayed path when the answer file exists (None: no answer
/// file is on disk, because the lookup was not made or failed before the recorder wrote one; its line in
/// lookup-status.txt says which) and the answer word "hit", "miss", "failed" or "n/a". For a
/// recording's TheDiscDB answers the word is `hit_rate::lookup`'s; for its tmdb.json it is `tmdb_answer`'s (the same
/// words over a TMDb record); for a pinned log "hit" when a record was read, else "n/a". RP14. Def: T15. Use: T15,
/// T16.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LookupFile {
    pub path: Option<String>,
    pub answer: String,
}

/// The inputs `plan_doc` found, as the document prints them (RP14). `lookup_status` is `hit_rate::statuses` (which
/// strips the byte-order mark) converted to a BTreeMap, empty for a pinned log. Field order is the output's key order.
/// Def: T15. Use: T15, T16.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PlanInputsSummary {
    /// The scan read: `<recording>/scan.txt` as the tree displays it, or the scan file as given.
    pub scan: String,
    /// meta.json's fingerprint (spec 5), or for a pinned log `fingerprint::content_hash` of its HSH lines.
    pub fingerprint: Option<String>,
    /// meta.json's disc ID (spec 5); None for a pinned log.
    pub disc_id: Option<String>,
    /// The bdmt file read for the disc's name (the first of `disc::bdmt_candidates`), as displayed; None without one.
    pub bdmt: Option<String>,
    /// Whether the recording has a meta.json; false for a pinned log.
    pub meta: bool,
    pub discdb_hash: LookupFile,
    pub discdb_discid: LookupFile,
    pub tmdb: LookupFile,
    /// --tmdb-search FILE and --tmdb-record FILE as given.
    pub tmdb_search: Option<String>,
    pub tmdb_record: Option<String>,
    pub lookup_status: BTreeMap<String, String>,
    /// --remembered FILE as given.
    pub remembered: Option<String>,
}

/// `lazymkv-dev plan`'s document (RP14): the slug (a recording's folder name) or the scan file as given, the inputs
/// found, the settings in force (spec 14's defaults, or --settings FILE) with their problems (`Settings::validate`;
/// printed, and the plan is made with them), and the Plan of spec 6.1 step 5. --settings FILE is read with
/// `Settings::from_json`, which returns the settings with validate's problems, so a file whose only problems are
/// validate's still plans; only from_json's Err refuses the file. Field order is the output's key order. Def: T15.
/// Use: T15, T16, T19.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PlanDoc {
    pub slug: String,
    pub inputs: PlanInputsSummary,
    pub settings: Settings,
    pub settings_problems: Vec<SettingsProblem>,
    pub plan: Plan,
}

/// The argument, borrowed: a resolved recording (a slug or a recording folder), or a bare scan file (a pinned log).
/// RP14. Def: T15. Use: T15, T16.
pub enum PlanSource<'a> {
    Recording(&'a Recording),
    ScanFile(&'a Path),
}

/// The optional files, each taken against `Ctx::cwd` when relative. RP14. Def: T15. Use: T15, T16.
#[derive(Clone, Debug, Default)]
pub struct PlanOptions {
    /// A settings file of spec 14's keys (absent keys take their defaults).
    pub settings: Option<PathBuf>,
    /// A TMDb `/search/multi` answer for the disc's name (8.1 steps 4 and 5; TMDb's documented shape).
    pub tmdb_search: Option<PathBuf>,
    /// A TMDb `/movie/{id}` or `/tv/{id}` answer (8.1); it replaces the recording's tmdb.json.
    pub tmdb_record: Option<PathBuf>,
    /// A remembered decision for this disc's fingerprint (spec 5, 8.1 step 1).
    pub remembered: Option<PathBuf>,
}

/// What a recording or a pinned log gives before the option files: the document's inputs and the parsed answers.
struct Found {
    slug: String,
    scan: String,
    disc: Disc,
    fingerprint: Option<String>,
    disc_id: Option<String>,
    bdmt: Option<String>,
    /// The `<di:name>` of the bdmt file read.
    bdmt_name: Option<String>,
    meta: bool,
    discdb_hash: LookupFile,
    discdb_discid: LookupFile,
    tmdb: LookupFile,
    by_fingerprint: Option<DiscDbRecord>,
    /// The disc ID's answer: every record of the array, in order.
    by_disc_id: Vec<DiscDbRecord>,
    /// tmdb.json's record.
    recorded: Option<TmdbRecord>,
    lookup_status: BTreeMap<String, String>,
    discdb_pending: bool,
    tmdb_pending: bool,
}

/// The document for one recording or pinned log (RP14). A recording (slug or directory) is read from scan.txt
/// (typed by `disc::disc`), meta.json (fingerprint and disc ID), the first of `disc::bdmt_candidates` under
/// nav/BDMV/META/DL through `disc::bdmt_name`, thediscdb-hash.json (`parse_dischash`), thediscdb-discid.json
/// (`parse_discid`; its first record with titles is `discdb_by_disc_id`), tmdb.json (`parse_recorded`) and
/// lookup-status.txt (`hit_rate::statuses`); an absent lookup file means that lookup was not made, a lookup that
/// failed is pending (8.1), an answer file whose status line is not 200 is not parsed (the recorder keeps the error
/// body of a 404 or a failed lookup), and only a file that exists and does not parse is Refused { code: 2 }. A bare
/// scan file
/// `<dir>/discNN.txt` is a pinned log: its fingerprint is `fingerprint::content_hash(&fingerprint::hashed_files(log))`
/// of its HSH lines; `discdb_by_fingerprint` is `corpus/thediscdb-api/dischash-<fingerprint>.json` under
/// `repo_root(ctx.cwd)` when present (it carries media), else `<dir>/discNN.json` with the same stem (no media: kind
/// and TMDb ID then unknown); `discdb_by_disc_id` is None (TheDiscDB's global_disc_id is not the disc ID, spec 5); no
/// TMDb answer. Then --settings (defaults otherwise), --tmdb-search, --tmdb-record (which replaces tmdb.json's record)
/// and --remembered (refused when it remembers another fingerprint); the disc name with
/// `lookup::name::disc_name(bdmt, disc.volume_label)`; PlanInputs with no sessions, no overrides and default
/// LibraryFacts; `plan::plan` (its Err is Refused { code: 2 }, prefixed by the scan's path). Def: T15. Use: T15, T16.
pub fn plan_doc(ctx: &Ctx, source: &PlanSource<'_>, options: &PlanOptions) -> Result<PlanDoc, Refused> {
    let found = match source {
        PlanSource::Recording(rec) => recording_inputs(rec)?,
        PlanSource::ScanFile(path) => scan_file_inputs(ctx, path)?,
    };
    let (settings, settings_problems) = match &options.settings {
        Some(path) => read_settings(ctx, path)?,
        None => {
            let settings = Settings::default();
            let problems = settings.validate();
            (settings, problems)
        }
    };
    let search: Option<(String, TmdbSearch)> = match &options.tmdb_search {
        Some(path) => {
            let (shown, text) = option_text(ctx, path)?;
            let search = parse_search(&text, &shown).map_err(refused)?;
            Some((shown, search))
        }
        None => None,
    };
    let given_record: Option<(String, TmdbRecord)> = match &options.tmdb_record {
        Some(path) => {
            let (shown, text) = option_text(ctx, path)?;
            let record = parse_record(&text, &shown).map_err(refused)?;
            Some((shown, record))
        }
        None => None,
    };
    let remembered: Option<(String, Remembered)> = match &options.remembered {
        Some(path) => Some(read_remembered(ctx, path, found.fingerprint.as_deref())?),
        None => None,
    };
    let name = disc_name(found.bdmt_name.as_deref(), found.disc.volume_label.as_deref());
    let record = given_record.as_ref().map(|(_, record)| record).or(found.recorded.as_ref());
    let library = LibraryFacts::default();
    let inputs = PlanInputs {
        disc: &found.disc,
        fingerprint: found.fingerprint.as_deref(),
        disc_id: found.disc_id.as_deref(),
        lookups: LookupInputs {
            name: name.as_ref(),
            discdb_by_disc_id: found.by_disc_id.iter().find(|answer| !answer.titles.is_empty()),
            discdb_by_fingerprint: found.by_fingerprint.as_ref(),
            discdb_pending: found.discdb_pending,
            search: search.as_ref().map(|(_, search)| search),
            record,
            tmdb_pending: found.tmdb_pending,
            remembered: None,
            reviewed: None,
        },
        remembered: remembered.as_ref().map(|(_, remembered)| remembered),
        overrides: None,
        sessions: &[],
        library: &library,
        settings: &settings,
    };
    let planned = plan(&inputs).map_err(|sentence| refused(format!("{}: {}", found.scan, sentence)))?;
    Ok(PlanDoc {
        slug: found.slug,
        inputs: PlanInputsSummary {
            scan: found.scan,
            fingerprint: found.fingerprint,
            disc_id: found.disc_id,
            bdmt: found.bdmt,
            meta: found.meta,
            discdb_hash: found.discdb_hash,
            discdb_discid: found.discdb_discid,
            tmdb: found.tmdb,
            tmdb_search: search.map(|(shown, _)| shown),
            tmdb_record: given_record.map(|(shown, _)| shown),
            lookup_status: found.lookup_status,
            remembered: remembered.map(|(shown, _)| shown),
        },
        settings,
        settings_problems,
        plan: planned,
    })
}

/// A recording's inputs (spec 15 "Recordings"), as `plan_doc`'s doc says.
fn recording_inputs(rec: &Recording) -> Result<Found, Refused> {
    let tree: &dyn Tree = &rec.tree;
    let dir = &rec.dir;
    let scan_key = dir.join(SCAN_FILE);
    let scan = tree.display(&scan_key);
    let text = read_text(tree, &scan_key, Encoding::Utf8, Errors::Replace).map_err(refused)?;
    let typed = typed_disc(&text, &scan)?;
    let status_key = dir.join(STATUS_FILE);
    let lookup_status: BTreeMap<String, String> = statuses(tree, dir)
        .map_err(|error| refused(format!("{}: {}", tree.display(&status_key), error)))?
        .iter()
        .map(|(lookup, status)| (lookup.to_string(), status.clone()))
        .collect();
    // An answer file is parsed only when its lookup answered 200 or has no status line: the recorder keeps the body of
    // every status (curl -o), so a 404 or a failed lookup leaves TMDb's or the server's error text on disk (8.1).
    let answered = |lookup: &str| matches!(lookup_status.get(lookup).map(String::as_str), None | Some("200"));
    let meta_key = dir.join(META_FILE);
    let meta: Option<Meta> = if tree.is_file(&meta_key) {
        let parsed = read_json(tree, &meta_key).map_err(|error| refused(format!("{}: {}", tree.display(&meta_key), error)))?;
        Some(parsed)
    } else {
        None
    };
    let fingerprint = meta.as_ref().and_then(|meta| present(meta.fingerprint.as_deref()));
    let disc_id = meta.as_ref().and_then(|meta| present(meta.disc_id.as_deref()));
    let (bdmt, bdmt_text_name) = read_bdmt(tree, dir)?;
    let hash_key = dir.join(DISCHASH_FILE);
    let hash = file_text(tree, &hash_key)?;
    let by_fingerprint = match &hash {
        Some((shown, text)) if answered("dischash") => parse_dischash(text, shown).map_err(refused)?,
        _ => None,
    };
    let discid_key = dir.join(DISCID_FILE);
    let discid = file_text(tree, &discid_key)?;
    let by_disc_id = match &discid {
        Some((shown, text)) if answered("discid") => parse_discid(text, shown).map_err(refused)?,
        _ => Vec::new(),
    };
    let tmdb_key = dir.join(TMDB_FILE);
    let tmdb_file = file_text(tree, &tmdb_key)?;
    let recorded = match &tmdb_file {
        Some((shown, text)) if answered("tmdb") => Some(parse_recorded(text, shown).map_err(refused)?),
        _ => None,
    };
    let status = |lookup: &str| lookup_status.get(lookup).map(String::as_str);
    let discdb_hash = LookupFile {
        path: hash.map(|(shown, _)| shown),
        answer: lookup_answer(tree, &hash_key, status("dischash")).to_string(),
    };
    let discdb_discid = LookupFile {
        path: discid.map(|(shown, _)| shown),
        answer: lookup_answer(tree, &discid_key, status("discid")).to_string(),
    };
    let tmdb = LookupFile {
        path: tmdb_file.map(|(shown, _)| shown),
        answer: tmdb_answer(status("tmdb"), recorded.is_some()).to_string(),
    };
    // 8.1: an unreachable service leaves its lookup pending; planning proceeds on what is in hand.
    let discdb_pending = discdb_hash.answer == FAILED || discdb_discid.answer == FAILED;
    let tmdb_pending = tmdb.answer == FAILED;
    Ok(Found {
        slug: rec.slug.clone(),
        scan,
        disc: typed,
        fingerprint,
        disc_id,
        bdmt,
        bdmt_name: bdmt_text_name,
        meta: meta.is_some(),
        discdb_hash,
        discdb_discid,
        tmdb,
        by_fingerprint,
        by_disc_id,
        recorded,
        lookup_status,
        discdb_pending,
        tmdb_pending,
    })
}

/// A pinned log's inputs (Open point 26), as `plan_doc`'s doc says.
fn scan_file_inputs(ctx: &Ctx, path: &Path) -> Result<Found, Refused> {
    let shown = shown_path(path);
    let full = ctx.abs(&path.to_string_lossy());
    let text = read_file_text(&full, &shown, Encoding::Utf8, Errors::Replace).map_err(refused)?;
    let typed = typed_disc(&text, &shown)?;
    let fingerprint = content_hash(&hashed_files(&text));
    let (answer_path, record) = pinned_answer(ctx, &full, &shown, fingerprint.as_deref())?;
    let answer = if record.is_some() { HIT } else { NOT_MADE };
    Ok(Found {
        slug: shown.clone(),
        scan: shown,
        disc: typed,
        fingerprint,
        disc_id: None,
        bdmt: None,
        bdmt_name: None,
        meta: false,
        discdb_hash: LookupFile { path: answer_path, answer: answer.to_string() },
        discdb_discid: not_made(),
        tmdb: not_made(),
        by_fingerprint: record,
        by_disc_id: Vec::new(),
        recorded: None,
        lookup_status: BTreeMap::new(),
        discdb_pending: false,
        tmdb_pending: false,
    })
}

/// A pinned log's TheDiscDB answer and its shown path: the API answer of its fingerprint under the repository root
/// (`corpus/thediscdb-api/dischash-<fingerprint>.json`), else the `<stem>.json` beside the scan file, else none.
fn pinned_answer(
    ctx: &Ctx,
    full: &Path,
    shown: &str,
    fingerprint: Option<&str>,
) -> Result<(Option<String>, Option<DiscDbRecord>), Refused> {
    if let (Some(fingerprint), Some(root)) = (fingerprint, repo_root(&ctx.cwd)) {
        let name = format!("dischash-{fingerprint}.json");
        let api = root.join(API_DIR).join(&name);
        if api.is_file() {
            let api_shown = format!("{API_DIR}/{name}");
            let text = read_file_text(&api, &api_shown, Encoding::Utf8Sig, Errors::Strict)
                .map_err(|error| refused(format!("{api_shown}: {error}")))?;
            let record = parse_dischash(&text, &api_shown).map_err(refused)?;
            return Ok((Some(api_shown), record));
        }
    }
    let stem = full.file_stem().map(|stem| stem.to_string_lossy().into_owned()).unwrap_or_default();
    let name = format!("{stem}.json");
    let beside = full.with_file_name(&name);
    if !beside.is_file() {
        return Ok((None, None));
    }
    let beside_shown = pathlib_str_beside(shown, &name);
    let text = read_file_text(&beside, &beside_shown, Encoding::Utf8Sig, Errors::Strict)
        .map_err(|error| refused(format!("{beside_shown}: {error}")))?;
    let record = parse_dischash(&text, &beside_shown).map_err(refused)?;
    Ok((Some(beside_shown), record))
}

/// The bdmt file of a recording's nav tree (the first of `bdmt_candidates` among the files of nav/BDMV/META/DL) as
/// displayed, and the `<di:name>` it holds; (None, None) without one. Spec 6.1 step 2, 8.1.
fn read_bdmt(tree: &dyn Tree, dir: &PathKey) -> Result<(Option<String>, Option<String>), Refused> {
    let folder = dir.join(BDMT_DIR);
    if !tree.is_dir(&folder) {
        return Ok((None, None));
    }
    let names: Vec<String> =
        tree.list(&folder).into_iter().filter(|key| tree.is_file(key)).map(|key| key.name().to_string()).collect();
    let Some(first) = bdmt_candidates(&names).into_iter().next() else {
        return Ok((None, None));
    };
    let key = folder.join(&first);
    let shown = tree.display(&key);
    let text = read_text(tree, &key, Encoding::Utf8, Errors::Replace).map_err(|error| refused(format!("{shown}: {error}")))?;
    Ok((Some(shown), bdmt_name(&text)))
}

/// A JSON file of the recording as (shown path, text), read as utf-8-sig, strict; None when it is not a file there.
fn file_text(tree: &dyn Tree, key: &PathKey) -> Result<Option<(String, String)>, Refused> {
    if !tree.is_file(key) {
        return Ok(None);
    }
    let shown = tree.display(key);
    let text =
        read_text(tree, key, Encoding::Utf8Sig, Errors::Strict).map_err(|error| refused(format!("{shown}: {error}")))?;
    Ok(Some((shown, text)))
}

/// An option file as (shown path, text): taken against ctx.cwd, read as utf-8-sig, strict. A missing file is refused
/// with Python's OSError text, which names the path (`[Errno 2] No such file or directory: '<path>'`); a file that is
/// there but cannot be read or decoded with `<shown>: <error>` (choice 9: the decode error alone names no file).
fn option_text(ctx: &Ctx, path: &Path) -> Result<(String, String), Refused> {
    let shown = shown_path(path);
    let full = ctx.abs(&path.to_string_lossy());
    let text = read_file_text(&full, &shown, Encoding::Utf8Sig, Errors::Strict)
        .map_err(|error| refused(if full.is_file() { format!("{shown}: {error}") } else { error }))?;
    Ok((shown, text))
}

/// --settings FILE (spec 14, ruling RP22): read through `Settings::from_json`, which returns the settings with
/// validate's problems; only its Err refuses the file (`<file>: <sentence>`, for a bad key `<file>: <key>: <message>`),
/// and the plan is made with the problems printed.
fn read_settings(ctx: &Ctx, path: &Path) -> Result<(Settings, Vec<SettingsProblem>), Refused> {
    let (shown, text) = option_text(ctx, path)?;
    Settings::from_json(&text).map_err(|error| refused(format!("{shown}: {error}")))
}

/// --remembered FILE (spec 5): the remembered decision, refused when it remembers another disc than `fingerprint`
/// (ASCII case ignored); a disc without a fingerprint takes it as given.
fn read_remembered(ctx: &Ctx, path: &Path, fingerprint: Option<&str>) -> Result<(String, Remembered), Refused> {
    let (shown, text) = option_text(ctx, path)?;
    let remembered: Remembered = serde_json::from_str(&text).map_err(|error| refused(format!("{shown}: {error}")))?;
    if let Some(fingerprint) = fingerprint {
        if !remembered.fingerprint.eq_ignore_ascii_case(fingerprint) {
            return Err(refused(format!(
                "{shown}: it remembers the disc {}, not this disc ({fingerprint})",
                remembered.fingerprint
            )));
        }
    }
    Ok((shown, remembered))
}

/// A TMDb record answer (8.1): `/movie/{id}` when the object has a non-null `title`, else `/tv/{id}`. Err is the
/// parser's sentence, prefixed by `shown`.
fn parse_record(text: &str, shown: &str) -> Result<TmdbRecord, String> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|error| format!("{shown}: {error}"))?;
    if value.get("title").map_or(false, |title| !title.is_null()) {
        parse_movie(text, shown)
    } else {
        parse_tv(text, shown)
    }
}

/// tmdb.json's answer word from its lookup-status.txt line: none "n/a" (not made), 404 "miss", 200 "hit" when the
/// record was read, any other case "failed" (a 200 whose file is absent or unread, a network error, a rate limit).
fn tmdb_answer(status: Option<&str>, read: bool) -> &'static str {
    match status {
        None => NOT_MADE,
        Some("404") => MISS,
        Some("200") if read => HIT,
        Some(_) => FAILED,
    }
}

/// The typed disc of a scan's text (spec 9.4), read through text mode; `disc::disc`'s refusal is prefixed by `shown`.
fn typed_disc(text: &str, shown: &str) -> Result<Disc, Refused> {
    disc(&parse(&text_mode(text))).map_err(|sentence| refused(format!("{shown}: {sentence}")))
}

/// A meta.json text field trimmed; None when absent or blank.
fn present(text: Option<&str>) -> Option<String> {
    text.map(str::trim).filter(|text| !text.is_empty()).map(str::to_string)
}

/// A lookup that was not made.
fn not_made() -> LookupFile {
    LookupFile { path: None, answer: NOT_MADE.to_string() }
}

/// A path as pathlib prints it (Windows separators read as '/').
fn shown_path(path: &Path) -> String {
    pathlib_split(&path.to_string_lossy()).0
}

/// A refusal with exit code 2 (port spec 3.4: unreadable).
fn refused(line: String) -> Refused {
    Refused { code: 2, line }
}

/// The command's name as typed.
const NAME: &str = "plan";
/// The usage line under the command's name: the COMMANDS synopsis after "plan ", with the --evidence flag every slug
/// command takes (port spec 3.4).
const USAGE: &str = "plan <slug|recording dir|scan file> [--evidence DIR] [--settings FILE] [--tmdb-search FILE] \
                     [--tmdb-record FILE] [--remembered FILE]";
/// The flags the command declares.
const FLAGS: [&str; 5] = ["--evidence", "--settings", "--tmdb-search", "--tmdb-record", "--remembered"];
/// What -h prints after the usage line.
const HELP: &str = "
The disc phase's plan (design spec 6.1 step 5) as one JSON document: the identity (8.1), the kind (8.2), the titles
(8.3 to 8.5), the mode (6.2, 6.3), the identity and title confidences and the review reasons (8.8), with the inputs
found and the settings in force (14).

arguments:
  source                a recording (a slug under the evidence clone, or a recording folder's path), or a scan
                        file (a pinned log, such as corpus/thediscdb/movie/<film>/<release>/disc01.txt)
  --evidence DIR        the evidence clone (default: LAZYMKV_EVIDENCE, else corpus/owner)
  --settings FILE       a settings file of spec 14's keys (default: spec 14's defaults)
  --tmdb-search FILE    a TMDb /search/multi answer for the disc's name
  --tmdb-record FILE    a TMDb /movie/{id} or /tv/{id} answer (replaces the recording's tmdb.json)
  --remembered FILE     a remembered decision for the disc's fingerprint (spec 5)

exit status: 0 when the plan is printed, with or without review reasons or settings problems; 2 for an argument
error, a scan that cannot be read, typed or planned, a lookup, settings, TMDb or remembered file that cannot be read
or does not parse, or a remembered decision for another disc; 3 when a slug is given and the evidence clone is
absent; never 1.
";

/// The words of one run: the source argument, --evidence and the optional files.
struct Parsed {
    source: String,
    evidence: Option<String>,
    options: PlanOptions,
}

/// The arguments as argparse reads them; Err is the message `usage_error` prints.
fn parse_args(args: &[String]) -> Result<Parsed, String> {
    let mut parsed = Args::new(args, &FLAGS);
    let evidence = parsed.flag("--evidence")?;
    let settings = parsed.flag("--settings")?.map(PathBuf::from);
    let tmdb_search = parsed.flag("--tmdb-search")?.map(PathBuf::from);
    let tmdb_record = parsed.flag("--tmdb-record")?.map(PathBuf::from);
    let remembered = parsed.flag("--remembered")?.map(PathBuf::from);
    let words = parsed.positionals()?;
    match words.as_slice() {
        [] => Err("the following arguments are required: source".to_string()),
        [source] => Ok(Parsed {
            source: source.clone(),
            evidence,
            options: PlanOptions { settings, tmdb_search, tmdb_record, remembered },
        }),
        [_, rest @ ..] => Err(format!("unrecognized arguments: {}", rest.join(" "))),
    }
}

/// `lazymkv-dev plan` (RP14, port spec 3.4): `ctx.abs(arg).is_file()` is tested first (a scan file), and only
/// otherwise `resolve_recording` (which refuses a path-shaped argument that is not a directory, so
/// `plan corpus/owner/x/scan.txt` for a missing file exits 2); its refusal is printed bare with its code (3 without
/// the clone), plan_doc's under the command's name; the document goes to stdout through `pyjson::dumps` with indent 1.
/// Exit 0 when the plan is printed, with or without review reasons or settings problems; 2 for an argument error or
/// any refusal of plan_doc (an unreadable scan, or a lookup, settings, TMDb or remembered file that does not parse, and
/// also a scan that cannot be typed or planned and a remembered file for another fingerprint); 3 when a slug is given
/// and the evidence clone is absent; never 1 (RP24). Def: T15. Use: T15.
pub fn run(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        let _ = writeln!(out, "usage: lazymkv-dev {}", USAGE);
        let _ = write!(out, "{}", HELP);
        return 0;
    }
    let parsed = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(message) => return usage_error(err, NAME, USAGE, &message),
    };
    let result = if ctx.abs(&parsed.source).is_file() {
        plan_doc(ctx, &PlanSource::ScanFile(Path::new(&parsed.source)), &parsed.options)
    } else {
        let rec = match resolve_recording(ctx, parsed.evidence.as_deref(), &parsed.source) {
            Ok(rec) => rec,
            Err(refused) => {
                let _ = writeln!(err, "{}", refused.line);
                return refused.code;
            }
        };
        plan_doc(ctx, &PlanSource::Recording(&rec), &parsed.options)
    };
    let doc = match result {
        Ok(doc) => doc,
        Err(refused) => {
            let _ = writeln!(err, "lazymkv-dev {}: error: {}", NAME, refused.line);
            return refused.code;
        }
    };
    let value = match serde_json::to_value(&doc) {
        Ok(value) => value,
        Err(error) => {
            let _ = writeln!(err, "lazymkv-dev {}: error: {}", NAME, error);
            return 2;
        }
    };
    let _ = writeln!(out, "{}", dumps(&value, Style::INDENT1_ASCII));
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    use lazymkv_core::lang::wanted_languages;
    use lazymkv_core::lookup::name::NameSource;
    use lazymkv_core::lookup::tmdb::{TmdbIdentity, TmdbKind};
    use lazymkv_core::lookup::{IdentityStep, Kind};
    use lazymkv_core::plan::movie::TitleRule;
    use lazymkv_core::plan::{DiscDbMatch, KindSource, Mode, PlannedRole, PlannedTitle};
    use lazymkv_core::remembered::{RememberedTitle, TitleRole};
    use lazymkv_core::review::{Note, ReviewReason};
    use lazymkv_core::testsupport::{scan_text_with_messages, search_json, tmdb_movie_json, tmdb_tv_json};

    use crate::evidence::resolve_recording;
    use crate::testutil::{Scratch, ScratchExt};

    /// The made-up film's disc: a Blu-ray named "Made Up Film" (CINFO 2) with the volume label MADE_UP_FILM (CINFO
    /// 32), scanned by MakeMKV 2.0.0 without Java (no MSG:3344).
    const FILM_DISC: &[(u32, &str)] = &[(1, "Blu-ray disc"), (2, "Made Up Film"), (32, "MADE_UP_FILM")];
    /// Its one title, 0: 16 chapters, 1:40:00 (6000 s), 30,000,000,000 bytes, playlist 00001.mpls, segment map "1".
    const FILM_TITLE: &[(u32, &str)] =
        &[(2, "Made Up Film"), (8, "16"), (9, "1:40:00"), (11, "30000000000"), (16, "00001.mpls"), (25, "1"), (26, "1")];
    /// A video stream and an English Dolby Digital 5.1 stream, as MakeMKV reports them (attributes 1, 3, 5, 6, 7, 14).
    const VIDEO: &[(u32, &str)] = &[(1, "Video"), (5, "V_MPEG4/ISO/AVC"), (6, "Mpeg4"), (7, "Mpeg4 AVC High@L4.1")];
    const ENG_AC3: &[(u32, &str)] = &[(1, "Audio"), (3, "eng"), (5, "A_AC3"), (6, "DD"), (7, "Dolby Digital"), (14, "6")];
    /// The same audio stream with an attribute 22 that is not a number (never seen; `disc::disc` refuses it).
    const BAD_FLAGS: &[(u32, &str)] =
        &[(1, "Audio"), (3, "eng"), (5, "A_AC3"), (6, "DD"), (7, "Dolby Digital"), (14, "6"), (22, "x")];
    const FILM_STREAMS: &[(u32, &[(u32, &str)])] = &[(0, VIDEO), (1, ENG_AC3)];
    const BAD_STREAMS: &[(u32, &[(u32, &str)])] = &[(0, VIDEO), (1, BAD_FLAGS)];

    /// The made-up film's fingerprint and disc ID, as meta.json states them.
    const FINGERPRINT: &str = "0123456789ABCDEF0123456789ABCDEF";
    const DISC_ID: &str = "00112233445566778899AABBCCDDEEFF00112233";
    /// meta.json as crates/capture writes it.
    const META: &str = r#"{"volume_label": "MADE_UP_FILM", "format": "bluray", "fingerprint": "0123456789ABCDEF0123456789ABCDEF",
 "disc_id": "00112233445566778899AABBCCDDEEFF00112233", "file_count": 12, "total_bytes": 31000000000,
 "captured_unix": 1790000000}"#;
    /// TheDiscDB's `dischash` answer in the API's shape (spec 8.1, last bullet): a Movie with TMDb ID 6537 and one
    /// MainMovie title that matches scan title 0 by playlist, segment map, size and duration (8.3).
    const DISCHASH: &str = r#"{"content_hash": "0123456789ABCDEF0123456789ABCDEF", "format": "Blu-ray",
 "media": {"title": "Made Up Film", "full_title": "Made Up Film", "year": 2007, "type": "Movie",
           "external_ids": {"tmdb": "6537", "imdb": "tt0006537"}},
 "disc": {"index": 1, "name": "Blu-ray", "slug": "blu-ray"},
 "titles": [{"index": 0, "source_file": "00001.mpls", "segment_map": "1", "duration": "1:40:00", "size": 30000000000,
             "item": {"title": "Made Up Film", "type": "MainMovie"}}]}"#;
    /// The slim tmdb.json, with the keys of the recorded files: a Spanish film.
    const TMDB_SLIM: &str = r#"{"id": 6537, "imdb_id": "tt0006537", "title": "Made Up Film", "original_title": "Pelicula Inventada",
 "release_date": "2007-05-20", "original_language": "es", "external_ids": {"imdb_id": "tt0006537"},
 "fetched_at": "2026-10-05"}"#;
    /// lookup-status.txt as the recorder writes it: a byte-order mark, CRLF line ends, "<lookup> [<what>] <status>".
    const STATUS: &str = "\u{feff}dischash 200\r\ndiscid 404\r\ntmdb movie/6537 200\r\n";
    /// The pinned John Wick UHD log and its answers (corpus/thediscdb, corpus/thediscdb-api).
    const JOHN_WICK_UHD: &str = "thediscdb/movie/John Wick (2014)/4k-ultra-hd--blu-ray";
    const JOHN_WICK_UHD_API: &str = "76E63EBB5B66F73B0C37BFAB59B8ABA5";

    /// A bdmt file shaped like the recorded one (a byte-order mark, the disclib wrapper, one `<di:name>`).
    fn bdmt_xml(name: &str) -> String {
        format!(
            "\u{feff}<?xml version=\"1.0\" encoding=\"utf-8\"?>\r\n<disclib xmlns=\"urn:BDA:bdmv;disclib\">\r\n  \
             <di:discinfo xmlns:di=\"urn:BDA:bdmv;discinfo\">\r\n    <di:title>\r\n      <di:name>{name}</di:name>\r\n    \
             </di:title>\r\n  </di:discinfo>\r\n</disclib>"
        )
    }

    /// The made-up film's scan with these streams in title 0.
    fn film_scan(streams: &[(u32, &[(u32, &str)])]) -> String {
        scan_text_with_messages("2.0.0", false, FILM_DISC, &[(0, FILM_TITLE, streams)])
    }

    /// The made-up film's recording `name` under `tmp` with every file a recording holds.
    fn film_recording(tmp: &Scratch, name: &str) {
        tmp.write(&format!("{name}/scan.txt"), film_scan(FILM_STREAMS));
        tmp.write(&format!("{name}/meta.json"), META);
        tmp.write(&format!("{name}/{BDMT_DIR}/bdmt_eng.xml"), bdmt_xml("Made Up Film"));
        tmp.write(&format!("{name}/thediscdb-hash.json"), DISCHASH);
        tmp.write(&format!("{name}/thediscdb-discid.json"), "");
        tmp.write(&format!("{name}/tmdb.json"), TMDB_SLIM);
        tmp.write(&format!("{name}/lookup-status.txt"), STATUS);
    }

    /// The made-up film's recording `name` with its scan.txt only.
    fn scan_only(tmp: &Scratch, name: &str) {
        tmp.write(&format!("{name}/scan.txt"), film_scan(FILM_STREAMS));
    }

    /// plan_doc of the recording folder `name` under `tmp`, named as a path from `tmp`.
    fn doc_of(tmp: &Scratch, name: &str, options: &PlanOptions) -> Result<PlanDoc, Refused> {
        let ctx = Ctx::for_test(&tmp.0);
        let rec = resolve_recording(&ctx, None, name).expect("the recording folder is there");
        plan_doc(&ctx, &PlanSource::Recording(&rec), options)
    }

    /// The document of the recording `rec` under `tmp`, which must plan.
    fn planned(tmp: &Scratch, options: &PlanOptions) -> PlanDoc {
        doc_of(tmp, "rec", options).unwrap_or_else(|refused| panic!("{}", refused.line))
    }

    /// The refusal of the recording `rec` under `tmp`, which must not plan.
    fn refusal(tmp: &Scratch, options: &PlanOptions) -> Refused {
        match doc_of(tmp, "rec", options) {
            Ok(doc) => panic!("{} was planned", doc.slug),
            Err(refused) => refused,
        }
    }

    /// plan_doc of the scan file `path`, named from `tmp`.
    fn scan_file_doc(tmp: &Scratch, path: &str) -> PlanDoc {
        let ctx = Ctx::for_test(&tmp.0);
        plan_doc(&ctx, &PlanSource::ScanFile(Path::new(path)), &PlanOptions::default())
            .unwrap_or_else(|refused| panic!("{}", refused.line))
    }

    /// The made-up film's one planned title (spec 5's key `t00-00001`, the scan's attributes 9, 11 and 16).
    fn film_title(remembered: bool) -> PlannedTitle {
        PlannedTitle {
            key: "t00-00001".to_string(),
            title: 0,
            role: PlannedRole::Feature { edition: None },
            seconds: Some(6000),
            size: Some(30_000_000_000),
            source_file: Some("00001.mpls".to_string()),
            suggested: false,
            remembered,
        }
    }

    /// The made-up film's TMDb identity.
    fn film_identity() -> TmdbIdentity {
        TmdbIdentity { kind: TmdbKind::Movie, id: 6537 }
    }

    /// A lookup file that is there, with its answer word.
    fn found(path: &str, answer: &str) -> LookupFile {
        LookupFile { path: Some(path.to_string()), answer: answer.to_string() }
    }

    /// lookup-status.txt's lines as the document's map.
    fn statuses_of(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs.iter().map(|(name, status)| (name.to_string(), status.to_string())).collect()
    }

    /// The notes of a plan, without their details.
    fn notes_of(plan: &Plan) -> Vec<Note> {
        plan.notes.iter().map(|noted| noted.note).collect()
    }

    /// A file of corpus/ by its path below it.
    fn corpus(relative: &str) -> Vec<u8> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus").join(relative);
        fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
    }

    #[test]
    fn a_recording_with_every_lookup_file_is_planned_from_thediscdb_and_its_tmdb_record() {
        let tmp = Scratch::new("plan-full");
        film_recording(&tmp, "rec");
        let doc = planned(&tmp, &PlanOptions::default());
        assert_eq!(doc.slug, "rec");
        assert_eq!(
            doc.inputs,
            PlanInputsSummary {
                scan: "rec/scan.txt".to_string(),
                fingerprint: Some(FINGERPRINT.to_string()),
                disc_id: Some(DISC_ID.to_string()),
                bdmt: Some("rec/nav/BDMV/META/DL/bdmt_eng.xml".to_string()),
                meta: true,
                discdb_hash: found("rec/thediscdb-hash.json", "hit"),
                discdb_discid: found("rec/thediscdb-discid.json", "miss"),
                tmdb: found("rec/tmdb.json", "hit"),
                tmdb_search: None,
                tmdb_record: None,
                lookup_status: statuses_of(&[("dischash", "200"), ("discid", "404"), ("tmdb", "200")]),
                remembered: None,
            }
        );
        assert_eq!(doc.settings, Settings::default());
        assert_eq!(doc.settings_problems, Settings::default().validate());
        let plan = &doc.plan;
        // 8.1 step 3: TheDiscDB knows the fingerprint (the disc ID answered 404); title, year and original language
        // come from tmdb.json, the record of the same TMDb ID.
        assert_eq!(
            (plan.identity.step, plan.identity.confidence, plan.identity_confidence),
            (IdentityStep::DiscdbOne, 0.95, 0.95)
        );
        assert_eq!(plan.identity.tmdb, Some(film_identity()));
        assert_eq!((plan.identity.title.as_deref(), plan.identity.year), (Some("Made Up Film"), Some(2007)));
        assert_eq!(plan.identity.original_language.as_deref(), Some("es"));
        assert_eq!((plan.identity.review, plan.identity.lookup_pending), (None, false));
        // 8.2: TheDiscDB states the kind; 8.3 rule 1: its MainMovie reconciles with scan title 0.
        assert_eq!((plan.kind, plan.kind_source), (Kind::Movie, KindSource::Discdb));
        assert_eq!(plan.titles, vec![film_title(false)]);
        assert_eq!(plan.evidence.title_rule, Some(TitleRule::Database));
        assert_eq!(plan.evidence.discdb_matches, vec![DiscDbMatch { discdb_index: 0, title: 0 }]);
        assert_eq!((plan.title_confidence, plan.mode), (0.98, Mode::Direct));
        assert_eq!(plan.reviews, Vec::<ReviewReason>::new());
        assert_eq!(notes_of(plan), Vec::<Note>::new());
        // R4: English and the original language, Spanish.
        assert_eq!(plan.wanted, wanted_languages(Some("es")));
        assert_eq!(plan.wanted.original, vec!["spa".to_string()]);
        assert_eq!(plan.session, None);
        let name = plan.evidence.name.as_ref().expect("the bdmt file names the disc");
        assert_eq!((name.raw.as_str(), name.source), ("Made Up Film", NameSource::Bdmt));
    }

    #[test]
    fn an_absent_lookup_file_is_not_an_error_and_its_lookup_was_not_made() {
        let tmp = Scratch::new("plan-bare");
        scan_only(&tmp, "rec");
        let doc = planned(&tmp, &PlanOptions::default());
        assert_eq!(
            doc.inputs,
            PlanInputsSummary {
                scan: "rec/scan.txt".to_string(),
                fingerprint: None,
                disc_id: None,
                bdmt: None,
                meta: false,
                discdb_hash: not_made(),
                discdb_discid: not_made(),
                tmdb: not_made(),
                tmdb_search: None,
                tmdb_record: None,
                lookup_status: BTreeMap::new(),
                remembered: None,
            }
        );
        let plan = &doc.plan;
        // 8.1 step 6: no answer and no search, so nothing is known (0, identity_unknown).
        assert_eq!((plan.identity.step, plan.identity_confidence), (IdentityStep::Nothing, 0.0));
        assert_eq!((plan.identity.tmdb, plan.identity.review), (None, Some(ReviewReason::IdentityUnknown)));
        // 8.2's default kind and 8.3 rule 3.5's one rival at 0.85, which still rips directly (0.85 >= 0.85).
        assert_eq!((plan.kind, plan.kind_source), (Kind::Movie, KindSource::Default));
        assert_eq!(plan.titles, vec![film_title(false)]);
        assert_eq!(plan.evidence.title_rule, Some(TitleRule::Structure));
        assert_eq!((plan.title_confidence, plan.mode), (0.85, Mode::Direct));
        assert_eq!(plan.reviews, vec![ReviewReason::IdentityUnknown]);
        // A Blu-ray that reached rule 2 without MSG:3344: the note records why the label rule did not apply (10.2).
        assert_eq!(notes_of(plan), vec![Note::LabelRuleUnavailable]);
        assert_eq!(plan.wanted, wanted_languages(None));
        let name = plan.evidence.name.as_ref().expect("the volume label names the disc");
        assert_eq!((name.raw.as_str(), name.source), ("MADE_UP_FILM", NameSource::VolumeLabel));
    }

    #[test]
    fn a_meta_json_that_does_not_parse_exits_2() {
        let tmp = Scratch::new("plan-bad-meta");
        film_recording(&tmp, "rec");
        tmp.write("rec/meta.json", "{\"fingerprint\": 7}");
        let refused = refusal(&tmp, &PlanOptions::default());
        assert_eq!(refused.code, 2);
        assert!(refused.line.starts_with("rec/meta.json: invalid type: integer `7`, expected a string"), "{}", refused.line);
    }

    #[test]
    fn a_dischash_answer_that_does_not_parse_exits_2() {
        let tmp = Scratch::new("plan-bad-hash");
        film_recording(&tmp, "rec");
        tmp.write("rec/thediscdb-hash.json", "{\"titles\": [");
        let refused = refusal(&tmp, &PlanOptions::default());
        assert_eq!(refused.code, 2);
        assert!(refused.line.starts_with("rec/thediscdb-hash.json: "), "{}", refused.line);
    }

    #[test]
    fn a_discid_answer_that_is_not_an_array_exits_2() {
        let tmp = Scratch::new("plan-bad-discid");
        film_recording(&tmp, "rec");
        tmp.write("rec/thediscdb-discid.json", DISCHASH);
        // The disc ID lookup answered 200, so its file is parsed (a 404's body is not).
        tmp.write("rec/lookup-status.txt", "dischash 200\ndiscid 200\ntmdb movie/6537 200\n");
        let refused = refusal(&tmp, &PlanOptions::default());
        assert_eq!(refused.code, 2);
        assert!(refused.line.starts_with("rec/thediscdb-discid.json: "), "{}", refused.line);
        assert!(refused.line.contains("expected a sequence"), "{}", refused.line);
    }

    #[test]
    fn a_tmdb_json_without_a_title_or_a_name_exits_2() {
        let tmp = Scratch::new("plan-bad-tmdb");
        film_recording(&tmp, "rec");
        tmp.write("rec/tmdb.json", r#"{"id": 6537, "fetched_at": "2026-10-05"}"#);
        assert_eq!(
            refusal(&tmp, &PlanOptions::default()),
            Refused {
                code: 2,
                line: "rec/tmdb.json: the record has neither a title (a movie) nor a name (a TV show)".to_string()
            }
        );
    }

    #[test]
    fn a_failed_lookup_is_planned_as_pending() {
        let tmp = Scratch::new("plan-pending");
        // TheDiscDB answered 503 and TMDb 500: neither answer is in (8.1 "the lookup is recorded as pending").
        scan_only(&tmp, "rec");
        tmp.write("rec/lookup-status.txt", "dischash 503\ntmdb movie/6537 500\n");
        let doc = planned(&tmp, &PlanOptions::default());
        assert_eq!(doc.inputs.discdb_hash, LookupFile { path: None, answer: "failed".to_string() });
        assert_eq!(doc.inputs.discdb_discid, not_made());
        assert_eq!(doc.inputs.tmdb, LookupFile { path: None, answer: "failed".to_string() });
        assert!(doc.plan.identity.lookup_pending);
        assert_eq!(
            doc.plan.identity.evidence[..2],
            ["the TheDiscDB lookup is pending".to_string(), "the TMDb lookup is pending".to_string()]
        );
        // Planning proceeds on structure alone: the step is the one the answers in hand reach.
        assert_eq!(doc.plan.identity.step, IdentityStep::Nothing);
        assert_eq!((doc.plan.title_confidence, doc.plan.mode), (0.85, Mode::Direct));
        // A failed disc ID lookup leaves TheDiscDB pending too.
        scan_only(&tmp, "rec2");
        tmp.write("rec2/lookup-status.txt", "dischash 404\ndiscid 429\n");
        let doc = doc_of(&tmp, "rec2", &PlanOptions::default()).unwrap();
        assert_eq!(doc.inputs.discdb_hash, LookupFile { path: None, answer: "miss".to_string() });
        assert_eq!(doc.inputs.discdb_discid, LookupFile { path: None, answer: "failed".to_string() });
        assert!(doc.plan.identity.lookup_pending);
        // A recording whose lookups all answered is not pending.
        film_recording(&tmp, "rec3");
        assert!(!doc_of(&tmp, "rec3", &PlanOptions::default()).unwrap().plan.identity.lookup_pending);
    }

    #[test]
    fn the_tmdb_answer_word_follows_the_lookup_status() {
        // No status line: the lookup was not made, whatever is on disk.
        assert_eq!(tmdb_answer(None, false), "n/a");
        assert_eq!(tmdb_answer(None, true), "n/a");
        // 404: TMDb has no such record.
        assert_eq!(tmdb_answer(Some("404"), false), "miss");
        // 200 with the record read; 200 without it, or any other status (a network error, a rate limit).
        assert_eq!(tmdb_answer(Some("200"), true), "hit");
        assert_eq!(tmdb_answer(Some("200"), false), "failed");
        assert_eq!(tmdb_answer(Some("500"), false), "failed");
        assert_eq!(tmdb_answer(Some("429"), true), "failed");
    }

    #[test]
    fn an_error_body_of_a_failed_or_missed_lookup_is_not_parsed() {
        let tmp = Scratch::new("plan-error-bodies");
        film_recording(&tmp, "rec");
        // The recorder keeps the body of every status (curl -o): a rate-limited disc ID lookup leaves the server's
        // text, and a TMDb 404 leaves TMDb's error object. Neither is parsed (8.1: a failed lookup is pending, a 404
        // is a miss), and planning goes on with TheDiscDB's fingerprint answer.
        tmp.write("rec/thediscdb-discid.json", "rate limited");
        tmp.write(
            "rec/tmdb.json",
            r#"{"success": false, "status_code": 34, "status_message": "The resource you requested could not be found."}"#,
        );
        tmp.write("rec/lookup-status.txt", "dischash 200\ndiscid 429\ntmdb movie/6537 404\n");
        let doc = planned(&tmp, &PlanOptions::default());
        assert_eq!(doc.inputs.discdb_discid, found("rec/thediscdb-discid.json", "failed"));
        assert_eq!(doc.inputs.tmdb, found("rec/tmdb.json", "miss"));
        assert!(doc.plan.identity.lookup_pending);
        assert_eq!((doc.plan.identity.step, doc.plan.identity.original_language.clone()), (IdentityStep::DiscdbOne, None));
    }

    #[test]
    fn a_disc_id_answer_alone_identifies_the_disc_with_its_first_record_that_has_titles() {
        let tmp = Scratch::new("plan-discid");
        scan_only(&tmp, "rec");
        tmp.write("rec/thediscdb-discid.json", format!("[{{\"titles\": []}}, {DISCHASH}]"));
        tmp.write("rec/lookup-status.txt", "discid 200\n");
        let doc = planned(&tmp, &PlanOptions::default());
        assert_eq!(doc.inputs.discdb_hash, not_made());
        assert_eq!(doc.inputs.discdb_discid, found("rec/thediscdb-discid.json", "hit"));
        let plan = &doc.plan;
        // 8.1 step 3 by disc ID: TheDiscDB's media gives the title and year; no TMDb record, so no original language.
        assert_eq!((plan.identity.step, plan.identity_confidence), (IdentityStep::DiscdbOne, 0.95));
        assert_eq!(plan.identity.tmdb, Some(film_identity()));
        assert_eq!((plan.identity.title.as_deref(), plan.identity.year), (Some("Made Up Film"), Some(2007)));
        assert_eq!(plan.identity.original_language, None);
        assert_eq!(plan.titles, vec![film_title(false)]);
        assert_eq!((plan.evidence.title_rule, plan.title_confidence), (Some(TitleRule::Database), 0.98));
        assert!(
            plan.evidence
                .sentences
                .contains(&"TheDiscDB's record by disc ID is matched against the scan: 1 titles match".to_string()),
            "{:?}",
            plan.evidence.sentences
        );
    }

    #[test]
    fn both_thediscdb_answers_identify_the_disc_at_0_99() {
        let tmp = Scratch::new("plan-both");
        film_recording(&tmp, "rec");
        tmp.write("rec/thediscdb-discid.json", format!("[{DISCHASH}]"));
        tmp.write("rec/lookup-status.txt", "dischash 200\ndiscid 200\ntmdb movie/6537 200\n");
        let doc = planned(&tmp, &PlanOptions::default());
        assert_eq!(doc.inputs.discdb_discid, found("rec/thediscdb-discid.json", "hit"));
        // 8.1 step 2: both records, the same TMDb ID.
        assert_eq!(
            (doc.plan.identity.step, doc.plan.identity.confidence, doc.plan.identity_confidence),
            (IdentityStep::DiscdbBoth, 0.99, 0.99)
        );
        assert_eq!(doc.plan.identity.tmdb, Some(film_identity()));
        assert_eq!(doc.plan.reviews, Vec::<ReviewReason>::new());
    }

    #[test]
    fn the_english_bdmt_file_names_the_disc_before_the_volume_label() {
        let tmp = Scratch::new("plan-bdmt");
        // Spec 8.1: the bdmt disc name, English first.
        film_recording(&tmp, "rec");
        tmp.write(&format!("rec/{BDMT_DIR}/bdmt_fra.xml"), bdmt_xml("Film Invente"));
        let doc = planned(&tmp, &PlanOptions::default());
        assert_eq!(doc.inputs.bdmt.as_deref(), Some("rec/nav/BDMV/META/DL/bdmt_eng.xml"));
        let name = doc.plan.evidence.name.expect("named");
        assert_eq!((name.raw.as_str(), name.source), ("Made Up Film", NameSource::Bdmt));
        // Without an English file, the first of the others in name order.
        film_recording(&tmp, "rec2");
        fs::remove_file(tmp.0.join("rec2").join(BDMT_DIR).join("bdmt_eng.xml")).unwrap();
        tmp.write(&format!("rec2/{BDMT_DIR}/bdmt_fra.xml"), bdmt_xml("Film Invente"));
        tmp.write(&format!("rec2/{BDMT_DIR}/bdmt_deu.xml"), bdmt_xml("Erfundener Film"));
        let doc = doc_of(&tmp, "rec2", &PlanOptions::default()).unwrap();
        assert_eq!(doc.inputs.bdmt.as_deref(), Some("rec2/nav/BDMV/META/DL/bdmt_deu.xml"));
        let name = doc.plan.evidence.name.expect("named");
        assert_eq!((name.raw.as_str(), name.source), ("Erfundener Film", NameSource::Bdmt));
        // A bdmt file without a name element is read, and the volume label names the disc.
        film_recording(&tmp, "rec3");
        tmp.write(&format!("rec3/{BDMT_DIR}/bdmt_eng.xml"), "<disclib></disclib>");
        let doc = doc_of(&tmp, "rec3", &PlanOptions::default()).unwrap();
        assert_eq!(doc.inputs.bdmt.as_deref(), Some("rec3/nav/BDMV/META/DL/bdmt_eng.xml"));
        let name = doc.plan.evidence.name.expect("named");
        assert_eq!((name.raw.as_str(), name.source), ("MADE_UP_FILM", NameSource::VolumeLabel));
        // A META/DL folder without a bdmt file: none read.
        scan_only(&tmp, "rec4");
        tmp.write(&format!("rec4/{BDMT_DIR}/notes.txt"), "not a bdmt file");
        let doc = doc_of(&tmp, "rec4", &PlanOptions::default()).unwrap();
        assert_eq!(doc.inputs.bdmt, None);
        assert_eq!(doc.plan.evidence.name.map(|name| name.source), Some(NameSource::VolumeLabel));
    }

    #[test]
    fn a_bare_scan_file_takes_its_thediscdb_answer_from_the_pinned_json_beside_it() {
        let tmp = Scratch::new("plan-pinned");
        // No repository root above the scratch folder: the API answers are out of reach.
        assert_eq!(repo_root(&tmp.0), None, "the scratch folder must not be inside a Cargo workspace");
        for name in ["disc01.txt", "disc01.json"] {
            tmp.write(name, corpus(&format!("{JOHN_WICK_UHD}/{name}")));
        }
        let doc = scan_file_doc(&tmp, "disc01.txt");
        assert_eq!(doc.slug, "disc01.txt");
        assert_eq!(
            doc.inputs,
            PlanInputsSummary {
                scan: "disc01.txt".to_string(),
                fingerprint: Some(JOHN_WICK_UHD_API.to_string()),
                disc_id: None,
                bdmt: None,
                meta: false,
                discdb_hash: found("disc01.json", "hit"),
                discdb_discid: not_made(),
                tmdb: not_made(),
                tmdb_search: None,
                tmdb_record: None,
                lookup_status: BTreeMap::new(),
                remembered: None,
            }
        );
        let plan = &doc.plan;
        // Open point 26: the pinned form has no media, so no TMDb ID and no kind: the identity stops at step 6, the
        // kind is 8.2's default, and rule 1 still plans the MainMovie item.
        assert_eq!((plan.identity.step, plan.identity_confidence), (IdentityStep::Nothing, 0.0));
        assert_eq!((plan.kind, plan.kind_source), (Kind::Movie, KindSource::Default));
        assert_eq!(plan.evidence.title_rule, Some(TitleRule::Database));
        assert_eq!(
            plan.titles,
            vec![PlannedTitle {
                key: "t00-00001".to_string(),
                title: 0,
                role: PlannedRole::Feature { edition: None },
                seconds: Some(6073),
                size: Some(67_757_930_496),
                source_file: Some("00001.mpls".to_string()),
                suggested: false,
                remembered: false,
            }]
        );
        assert_eq!((plan.title_confidence, plan.mode), (0.98, Mode::Direct));
        assert_eq!(plan.reviews, vec![ReviewReason::IdentityUnknown]);
        assert_eq!(notes_of(plan), Vec::<Note>::new());
    }

    #[test]
    fn a_bare_scan_file_prefers_the_api_answer_of_its_fingerprint_under_the_repository_root() {
        let tmp = Scratch::new("plan-api");
        tmp.write("Cargo.toml", "[workspace]\nmembers = []\n");
        let api = format!("thediscdb-api/dischash-{JOHN_WICK_UHD_API}.json");
        tmp.write(&format!("corpus/{api}"), corpus(&api));
        for name in ["disc01.txt", "disc01.json"] {
            tmp.write(&format!("logs/{name}"), corpus(&format!("{JOHN_WICK_UHD}/{name}")));
        }
        let doc = scan_file_doc(&tmp, "logs/disc01.txt");
        assert_eq!(doc.inputs.scan, "logs/disc01.txt");
        assert_eq!(doc.inputs.fingerprint.as_deref(), Some(JOHN_WICK_UHD_API));
        assert_eq!(
            doc.inputs.discdb_hash,
            found(&format!("corpus/thediscdb-api/dischash-{JOHN_WICK_UHD_API}.json"), "hit")
        );
        // The API answer carries media: 8.1 step 3 with the movie 245891, and TheDiscDB states the kind.
        let plan = &doc.plan;
        assert_eq!((plan.identity.step, plan.identity_confidence), (IdentityStep::DiscdbOne, 0.95));
        assert_eq!(plan.identity.tmdb, Some(TmdbIdentity { kind: TmdbKind::Movie, id: 245891 }));
        assert_eq!((plan.kind, plan.kind_source), (Kind::Movie, KindSource::Discdb));
        assert_eq!(plan.reviews, Vec::<ReviewReason>::new());
    }

    #[test]
    fn a_bare_scan_file_without_hsh_lines_reads_only_the_json_beside_it() {
        let tmp = Scratch::new("plan-film-file");
        tmp.write("film.txt", film_scan(FILM_STREAMS));
        // No HSH line: no fingerprint, and without film.json no answer.
        let doc = scan_file_doc(&tmp, "film.txt");
        assert_eq!((doc.inputs.fingerprint.clone(), doc.inputs.discdb_hash.clone()), (None, not_made()));
        assert_eq!((doc.plan.identity.step, doc.plan.title_confidence), (IdentityStep::Nothing, 0.85));
        // The answer beside it has the scan file's stem.
        tmp.write("film.json", DISCHASH);
        let doc = scan_file_doc(&tmp, "film.txt");
        assert_eq!(doc.inputs.discdb_hash, found("film.json", "hit"));
        assert_eq!((doc.plan.identity.step, doc.plan.identity.tmdb), (IdentityStep::DiscdbOne, Some(film_identity())));
        assert_eq!((doc.plan.kind_source, doc.plan.title_confidence), (KindSource::Discdb, 0.98));
        assert_eq!(doc.plan.titles, vec![film_title(false)]);
    }

    #[test]
    fn a_settings_file_replaces_the_defaults_and_its_problems_are_printed_with_the_plan() {
        let tmp = Scratch::new("plan-settings");
        film_recording(&tmp, "rec");
        // Spec 14's defaults: auto_threshold 0.85, so the 0.98 title confidence rips directly; keep_tuning_evidence on
        // without a tuning_root is the one problem the defaults raise.
        let doc = planned(&tmp, &PlanOptions::default());
        assert_eq!(doc.settings.auto_threshold, 0.85);
        assert_eq!(doc.settings_problems.iter().map(|problem| problem.key.as_str()).collect::<Vec<_>>(), vec!["tuning_root"]);
        assert_eq!((doc.plan.title_confidence, doc.plan.mode), (0.98, Mode::Direct));
        // A file naming two keys: every other key keeps its default, no problem is left, and 0.98 is below the bar.
        tmp.write("valid.json", r#"{"auto_threshold": 0.99, "tuning_root": "D:\\lazymkv\\tuning"}"#);
        let doc = planned(&tmp, &PlanOptions { settings: Some(PathBuf::from("valid.json")), ..PlanOptions::default() });
        assert_eq!(
            doc.settings,
            Settings {
                auto_threshold: 0.99,
                tuning_root: Some("D:\\lazymkv\\tuning".to_string()),
                ..Settings::default()
            }
        );
        assert_eq!(doc.settings_problems, Vec::<SettingsProblem>::new());
        assert_eq!((doc.plan.title_confidence, doc.plan.mode), (0.98, Mode::Copy));
        // A file whose only problem is validate's: printed, and the plan is made with those settings.
        tmp.write("unset.json", r#"{"auto_threshold": 0.99}"#);
        let doc = planned(&tmp, &PlanOptions { settings: Some(PathBuf::from("unset.json")), ..PlanOptions::default() });
        assert_eq!(doc.settings.auto_threshold, 0.99);
        assert_eq!(doc.settings_problems, Settings::default().validate());
        assert_eq!(doc.plan.mode, Mode::Copy);
    }

    #[test]
    fn a_settings_file_with_an_unknown_key_or_a_wrong_type_exits_2() {
        let tmp = Scratch::new("plan-bad-settings");
        film_recording(&tmp, "rec");
        let with = |file: &str| PlanOptions { settings: Some(PathBuf::from(file)), ..PlanOptions::default() };
        tmp.write("unknown.json", r#"{"bogus_key": 1}"#);
        let refused = refusal(&tmp, &with("unknown.json"));
        assert_eq!(refused.code, 2);
        assert!(refused.line.starts_with("unknown.json: bogus_key: unknown field `bogus_key`"), "{}", refused.line);
        tmp.write("wrong.json", r#"{"auto_threshold": "high"}"#);
        let refused = refusal(&tmp, &with("wrong.json"));
        assert_eq!(refused.code, 2);
        assert!(
            refused.line.starts_with("wrong.json: auto_threshold: invalid type: string \"high\", expected f64"),
            "{}",
            refused.line
        );
        tmp.write("list.json", "[1]");
        assert_eq!(
            refusal(&tmp, &with("list.json")),
            Refused { code: 2, line: "list.json: a settings file is one JSON object of spec 14's keys".to_string() }
        );
        // serde alone would read [] as a sequence into the defaults; from_json refuses it.
        tmp.write("empty.json", "[]");
        assert_eq!(
            refusal(&tmp, &with("empty.json")),
            Refused { code: 2, line: "empty.json: a settings file is one JSON object of spec 14's keys".to_string() }
        );
    }

    #[test]
    fn tmdb_search_and_record_files_identify_a_disc_thediscdb_does_not_know() {
        let tmp = Scratch::new("plan-search");
        scan_only(&tmp, "rec");
        tmp.write("search.json", search_json(&[("movie", 6537, "Made Up Film", "Pelicula Inventada", "2007-05-20")]));
        tmp.write("movie.json", tmdb_movie_json(6537, "Made Up Film", "Pelicula Inventada", "2007-05-20", "es", Some(100)));
        let options = PlanOptions {
            tmdb_search: Some(PathBuf::from("search.json")),
            tmdb_record: Some(PathBuf::from("movie.json")),
            ..PlanOptions::default()
        };
        let doc = planned(&tmp, &options);
        assert_eq!(
            (doc.inputs.tmdb_search.as_deref(), doc.inputs.tmdb_record.as_deref()),
            (Some("search.json"), Some("movie.json"))
        );
        let plan = &doc.plan;
        // 8.1 step 4: one plausible result ("MADE UP FILM" folds to its title) whose 100 minutes are within 3% of
        // the 6000 s feature; tmdb_only_autofile is on by default (D9).
        assert!(doc.settings.tmdb_only_autofile);
        assert_eq!((plan.identity.step, plan.identity_confidence), (IdentityStep::TmdbOne, 0.90));
        assert_eq!(plan.identity.tmdb, Some(film_identity()));
        assert_eq!(plan.identity.runtime_check, Some(true));
        assert_eq!(plan.identity.original_language.as_deref(), Some("es"));
        assert_eq!(plan.reviews, Vec::<ReviewReason>::new());
        assert_eq!((plan.title_confidence, plan.mode), (0.85, Mode::Direct));
        // tmdb_only_autofile off: capped at 0.84 and held for review (8.1, D9).
        tmp.write("off.json", r#"{"tmdb_only_autofile": false}"#);
        let doc = planned(&tmp, &PlanOptions { settings: Some(PathBuf::from("off.json")), ..options.clone() });
        assert!(!doc.settings.tmdb_only_autofile);
        assert_eq!((doc.plan.identity.step, doc.plan.identity_confidence), (IdentityStep::TmdbOne, 0.84));
        assert_eq!(doc.plan.reviews, vec![ReviewReason::IdentityAmbiguous]);
    }

    #[test]
    fn a_tmdb_record_file_is_read_as_a_movie_when_it_has_a_title_else_as_a_show() {
        let movie = tmdb_movie_json(6537, "Made Up Film", "Pelicula Inventada", "2007-05-20", "es", Some(100));
        let record = parse_record(&movie, "movie.json").unwrap();
        assert_eq!((record.identity, record.runtime), (film_identity(), Some(100)));
        let episodes: &[(u32, &str, Option<u32>)] = &[(1, "One", Some(22)), (2, "Two", Some(23))];
        let show = tmdb_tv_json(99, "Show", "2010-01-01", "en", &[(1, episodes)]);
        let record = parse_record(&show, "tv.json").unwrap();
        assert_eq!(record.identity, TmdbIdentity { kind: TmdbKind::Tv, id: 99 });
        assert_eq!(record.episode_runtimes(Some(1)), vec![22, 23]);
        // A null title is no title: the text is read as a show, which needs a name.
        assert_eq!(
            parse_record(r#"{"id": 5, "title": null}"#, "x.json"),
            Err("x.json: the TV record 5 has no name".to_string())
        );
        assert!(parse_record("not json", "x.json").unwrap_err().starts_with("x.json: "));
    }

    #[test]
    fn a_tmdb_record_file_replaces_tmdb_json_and_an_answer_without_a_status_line_is_read() {
        let tmp = Scratch::new("plan-override");
        // Choice 7: --tmdb-record replaces tmdb.json's record (the given record's French wins over the slim file's
        // Spanish), while tmdb.json is still read and keeps its answer word.
        film_recording(&tmp, "rec");
        tmp.write("movie.json", tmdb_movie_json(6537, "Made Up Film", "Pelicula Inventada", "2007-05-20", "fr", Some(100)));
        let doc = planned(&tmp, &PlanOptions { tmdb_record: Some(PathBuf::from("movie.json")), ..PlanOptions::default() });
        assert_eq!(doc.inputs.tmdb, found("rec/tmdb.json", "hit"));
        assert_eq!(doc.inputs.tmdb_record.as_deref(), Some("movie.json"));
        assert_eq!(doc.plan.identity.original_language.as_deref(), Some("fr"));
        // Without lookup-status.txt no lookup has a status line: every answer file there is parsed and used, and each
        // answer word is "n/a" (`hit_rate::lookup` and `tmdb_answer` with no status).
        film_recording(&tmp, "rec2");
        fs::remove_file(tmp.0.join("rec2").join("lookup-status.txt")).unwrap();
        let doc = doc_of(&tmp, "rec2", &PlanOptions::default()).unwrap();
        assert_eq!(doc.inputs.discdb_hash, found("rec2/thediscdb-hash.json", "n/a"));
        assert_eq!(doc.inputs.tmdb, found("rec2/tmdb.json", "n/a"));
        assert_eq!(doc.inputs.lookup_status, BTreeMap::new());
        assert_eq!(doc.plan.identity.step, IdentityStep::DiscdbOne);
        assert_eq!(doc.plan.identity.original_language.as_deref(), Some("es"));
        assert!(!doc.plan.identity.lookup_pending);
    }

    /// The made-up film's remembered decision (spec 5): its TMDb identity and title 0 as the feature.
    fn remembered_film(fingerprint: &str) -> Remembered {
        Remembered {
            fingerprint: fingerprint.to_string(),
            tmdb: film_identity(),
            titles: vec![RememberedTitle {
                source_file: Some("00001.mpls".to_string()),
                dvd_title: None,
                segment_map: "1".to_string(),
                seconds: 6000,
                role: TitleRole::Feature,
            }],
            audio: Vec::new(),
            subtitles: Vec::new(),
        }
    }

    #[test]
    fn a_remembered_file_sets_the_identity_and_the_titles_to_1_00() {
        let tmp = Scratch::new("plan-remembered");
        film_recording(&tmp, "rec");
        tmp.write("remembered.json", serde_json::to_string(&remembered_film(FINGERPRINT)).unwrap());
        let options = PlanOptions { remembered: Some(PathBuf::from("remembered.json")), ..PlanOptions::default() };
        let doc = planned(&tmp, &options);
        assert_eq!(doc.inputs.remembered.as_deref(), Some("remembered.json"));
        let plan = &doc.plan;
        // Spec 5, 8.1 step 1 and 8.8: a remembered decision covers the identity and the titles, each at 1.00.
        assert_eq!((plan.identity.step, plan.identity_confidence), (IdentityStep::Remembered, 1.0));
        assert_eq!(plan.identity.tmdb, Some(film_identity()));
        assert_eq!(plan.title_confidence, 1.0);
        assert_eq!(plan.titles, vec![film_title(true)]);
        assert_eq!((plan.mode, plan.reviews.clone()), (Mode::Direct, Vec::<ReviewReason>::new()));
    }

    #[test]
    fn a_remembered_file_for_another_fingerprint_exits_2() {
        let tmp = Scratch::new("plan-remembered-other");
        film_recording(&tmp, "rec");
        let options = PlanOptions { remembered: Some(PathBuf::from("remembered.json")), ..PlanOptions::default() };
        tmp.write("remembered.json", serde_json::to_string(&remembered_film("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")).unwrap());
        assert_eq!(
            refusal(&tmp, &options),
            Refused {
                code: 2,
                line: "remembered.json: it remembers the disc FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF, not this disc \
                       (0123456789ABCDEF0123456789ABCDEF)"
                    .to_string()
            }
        );
        // The fingerprint compares ignoring ASCII case.
        let lower = FINGERPRINT.to_ascii_lowercase();
        tmp.write("remembered.json", serde_json::to_string(&remembered_film(&lower)).unwrap());
        assert_eq!(planned(&tmp, &options).plan.identity.step, IdentityStep::Remembered);
        // A disc without a fingerprint (no meta.json) takes any remembered file it is given.
        scan_only(&tmp, "rec2");
        tmp.write("remembered.json", serde_json::to_string(&remembered_film("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")).unwrap());
        let doc = doc_of(&tmp, "rec2", &options).unwrap();
        assert_eq!((doc.plan.identity.step, doc.plan.title_confidence), (IdentityStep::Remembered, 1.0));
    }

    #[test]
    fn an_option_file_that_cannot_be_read_or_parsed_exits_2() {
        let tmp = Scratch::new("plan-bad-options");
        film_recording(&tmp, "rec");
        let missing = Refused { code: 2, line: "[Errno 2] No such file or directory: 'missing.json'".to_string() };
        let file = || Some(PathBuf::from("missing.json"));
        for options in [
            PlanOptions { settings: file(), ..PlanOptions::default() },
            PlanOptions { tmdb_search: file(), ..PlanOptions::default() },
            PlanOptions { tmdb_record: file(), ..PlanOptions::default() },
            PlanOptions { remembered: file(), ..PlanOptions::default() },
        ] {
            assert_eq!(refusal(&tmp, &options), missing, "{options:?}");
        }
        tmp.write("search.json", r#"{"results": 5}"#);
        let refused = refusal(&tmp, &PlanOptions { tmdb_search: Some(PathBuf::from("search.json")), ..PlanOptions::default() });
        assert_eq!(refused.code, 2);
        assert!(refused.line.starts_with("search.json: "), "{}", refused.line);
        tmp.write("record.json", r#"{"title": "Made Up Film"}"#);
        let refused = refusal(&tmp, &PlanOptions { tmdb_record: Some(PathBuf::from("record.json")), ..PlanOptions::default() });
        assert_eq!(refused.code, 2);
        assert!(refused.line.starts_with("record.json: ") && refused.line.contains("missing field `id`"), "{}", refused.line);
        tmp.write("remembered.json", "{}");
        let refused = refusal(&tmp, &PlanOptions { remembered: Some(PathBuf::from("remembered.json")), ..PlanOptions::default() });
        assert_eq!(refused.code, 2);
        assert!(refused.line.starts_with("remembered.json: missing field `fingerprint`"), "{}", refused.line);
        // A file that is there but not UTF-8: the decode error under the file's name (choice 9).
        tmp.write("latin.json", b"{\"title\": \"\xe9\"}");
        assert_eq!(
            refusal(&tmp, &PlanOptions { tmdb_record: Some(PathBuf::from("latin.json")), ..PlanOptions::default() }),
            Refused {
                code: 2,
                line: "latin.json: 'utf-8' codec can't decode byte 0xe9 in position 11: invalid continuation byte".to_string()
            }
        );
    }

    #[test]
    fn a_scan_that_cannot_be_read_or_typed_or_has_no_title_exits_2() {
        let tmp = Scratch::new("plan-bad-scan");
        // A recording folder without scan.txt.
        tmp.write("rec/meta.json", META);
        assert_eq!(
            refusal(&tmp, &PlanOptions::default()),
            Refused { code: 2, line: "[Errno 2] No such file or directory: 'rec/scan.txt'".to_string() }
        );
        // Attribute 22 that is not a number: disc::disc refuses the scan, naming the title and the stream.
        tmp.write("bad/scan.txt", film_scan(BAD_STREAMS));
        let refused = doc_of(&tmp, "bad", &PlanOptions::default()).unwrap_err();
        assert_eq!(refused.code, 2);
        assert!(refused.line.starts_with("bad/scan.txt: title 0 stream 1: attribute 22 (flags): "), "{}", refused.line);
        // A scan with no title: plan::plan's one refusal.
        tmp.write("empty/scan.txt", scan_text_with_messages("2.0.0", false, FILM_DISC, &[]));
        assert_eq!(
            doc_of(&tmp, "empty", &PlanOptions::default()).unwrap_err(),
            Refused { code: 2, line: "empty/scan.txt: the scan lists no title, so there is nothing to plan".to_string() }
        );
    }

    /// The keys of a JSON object, in order.
    fn keys(value: &serde_json::Value) -> Vec<String> {
        value.as_object().expect("an object").keys().cloned().collect()
    }

    #[test]
    fn a_file_argument_is_a_scan_and_a_directory_argument_is_a_recording() {
        let tmp = Scratch::new("plan-forms");
        film_recording(&tmp, "rec");
        let ctx = Ctx::for_test(&tmp.0);
        // A file is a bare scan file, even beside a recording's meta.json (RP14; run tests is_file first).
        let (code, out, err) = crate::testutil::call(run, &ctx, &["rec/scan.txt"]);
        assert_eq!((code, err.as_str()), (0, ""));
        let doc: PlanDoc = serde_json::from_str(&out).expect("one JSON document");
        assert_eq!(
            (doc.slug.as_str(), doc.inputs.scan.as_str(), doc.inputs.meta, doc.inputs.fingerprint.as_deref()),
            ("rec/scan.txt", "rec/scan.txt", false, None)
        );
        assert_eq!(doc.inputs.discdb_hash, not_made());
        // A directory is a recording.
        let (code, out, err) = crate::testutil::call(run, &ctx, &["rec"]);
        assert_eq!((code, err.as_str()), (0, ""));
        let doc: PlanDoc = serde_json::from_str(&out).expect("one JSON document");
        assert_eq!(
            (doc.slug.as_str(), doc.inputs.scan.as_str(), doc.inputs.meta, doc.inputs.fingerprint.as_deref()),
            ("rec", "rec/scan.txt", true, Some(FINGERPRINT))
        );
        assert_eq!(doc.inputs.discdb_hash, found("rec/thediscdb-hash.json", "hit"));
        // A path-shaped argument that names nothing: resolve_recording's refusal, printed bare, exit 2.
        let (code, out, err) = crate::testutil::call(run, &ctx, &["no/such/dir"]);
        assert_eq!((code, out.as_str(), err.as_str()), (2, "", "no/such/dir: no such folder\n"));
    }

    #[test]
    fn a_slug_is_read_under_the_evidence_clone_or_exits_3_without_it() {
        let tmp = Scratch::new("plan-slug");
        film_recording(&tmp, "clone/made-up-film");
        let ctx = Ctx::for_test(&tmp.0);
        let (code, out, err) = crate::testutil::call(run, &ctx, &["--evidence", "clone", "made-up-film"]);
        assert_eq!((code, err.as_str()), (0, ""));
        let doc: PlanDoc = serde_json::from_str(&out).expect("one JSON document");
        // Port spec 3.4: paths are printed under corpus/owner whatever --evidence says.
        assert_eq!(doc.slug, "made-up-film");
        assert_eq!(doc.inputs.scan, "corpus/owner/made-up-film/scan.txt");
        assert_eq!(doc.inputs.discdb_hash, found("corpus/owner/made-up-film/thediscdb-hash.json", "hit"));
        // LAZYMKV_EVIDENCE names the clone when --evidence does not.
        let mut env_ctx = Ctx::for_test(&tmp.0);
        env_ctx.env.insert("LAZYMKV_EVIDENCE".to_string(), "clone".to_string());
        let (code, out, _) = crate::testutil::call(run, &env_ctx, &["made-up-film"]);
        assert_eq!(code, 0);
        assert_eq!(serde_json::from_str::<PlanDoc>(&out).expect("one JSON document"), doc);
        // No clone: exit 3 with the line naming the evidence repository and where to clone it.
        let (code, out, err) = crate::testutil::call(run, &ctx, &["--evidence", "missing", "made-up-film"]);
        assert_eq!((code, out.as_str()), (3, ""));
        assert_eq!(
            err,
            "missing: no such folder; the evidence repository lazymkv-evidence is not cloned there \
             (git clone https://github.com/krcm0209/lazymkv-evidence missing)\n"
        );
        let (code, _, err) = crate::testutil::call(run, &ctx, &["made-up-film"]);
        assert_eq!(code, 3);
        assert_eq!(
            err,
            "corpus/owner: no such folder; the evidence repository lazymkv-evidence is not cloned there \
             (git clone https://github.com/krcm0209/lazymkv-evidence corpus/owner)\n"
        );
        // A slug the clone lacks: exit 2.
        let (code, _, err) = crate::testutil::call(run, &ctx, &["--evidence", "clone", "no-such-disc"]);
        assert_eq!(
            (code, err.as_str()),
            (2, "corpus/owner/no-such-disc: no such recording (neither a recording folder nor a slug under corpus/owner)\n")
        );
    }

    #[test]
    fn the_document_is_printed_as_json_with_indent_1_and_the_key_order_of_plan_doc() {
        let tmp = Scratch::new("plan-print");
        film_recording(&tmp, "rec");
        let ctx = Ctx::for_test(&tmp.0);
        let rec = resolve_recording(&ctx, None, "rec").unwrap();
        let doc = plan_doc(&ctx, &PlanSource::Recording(&rec), &PlanOptions::default()).unwrap();
        let (code, out, err) = crate::testutil::call(run, &ctx, &["rec"]);
        assert_eq!((code, err.as_str()), (0, ""));
        // pyjson::dumps with indent 1, as decide prints its document (port spec 3.4).
        assert_eq!(out, format!("{}\n", dumps(&serde_json::to_value(&doc).unwrap(), Style::INDENT1_ASCII)));
        assert!(out.contains("\n  \"identity_confidence\": 0.95,\n"), "{out}");
        assert!(out.contains("\n  \"title_confidence\": 0.98,\n"), "{out}");
        assert_eq!(serde_json::from_str::<PlanDoc>(&out).expect("the document reads back"), doc);
        let value: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(keys(&value), ["slug", "inputs", "settings", "settings_problems", "plan"]);
        assert_eq!(
            keys(&value["inputs"]),
            [
                "scan",
                "fingerprint",
                "disc_id",
                "bdmt",
                "meta",
                "discdb_hash",
                "discdb_discid",
                "tmdb",
                "tmdb_search",
                "tmdb_record",
                "lookup_status",
                "remembered"
            ]
        );
        assert_eq!(keys(&value["inputs"]["discdb_hash"]), ["path", "answer"]);
        assert_eq!(keys(&value["inputs"]["lookup_status"]), ["dischash", "discid", "tmdb"]);
        assert_eq!(
            keys(&value["plan"]),
            [
                "identity",
                "kind",
                "kind_source",
                "titles",
                "mode",
                "identity_confidence",
                "title_confidence",
                "reviews",
                "notes",
                "wanted",
                "session",
                "evidence"
            ]
        );
        let settings = keys(&value["settings"]);
        assert_eq!(settings, keys(&serde_json::to_value(Settings::default()).unwrap()));
        assert_eq!((settings.first().map(String::as_str), settings.last().map(String::as_str)), (Some("staging_root"), Some("tool_paths")));
        assert_eq!(keys(&value["settings_problems"][0]), ["key", "problem"]);
        // Absent values are written as null, never left out.
        assert_eq!(value["inputs"]["tmdb_search"], serde_json::Value::Null);
        assert_eq!(value["plan"]["session"], serde_json::Value::Null);
    }

    #[test]
    fn argument_errors_are_reported_as_argparse_reports_them() {
        let tmp = Scratch::new("plan-args");
        film_recording(&tmp, "rec");
        let ctx = Ctx::for_test(&tmp.0);
        let usage = format!("usage: lazymkv-dev {USAGE}\n");
        for (args, message) in [
            (&[][..], "the following arguments are required: source"),
            (&["rec", "other"][..], "unrecognized arguments: other"),
            (&["rec", "--bogus"][..], "unrecognized arguments: --bogus"),
            (&["rec", "--settings"][..], "argument --settings: expected one argument"),
        ] {
            let (code, out, err) = crate::testutil::call(run, &ctx, args);
            assert_eq!((code, out.as_str()), (2, ""), "{args:?}");
            assert_eq!(err, format!("{usage}lazymkv-dev plan: error: {message}\n"), "{args:?}");
        }
        // A refusal of plan_doc is printed under the command's name with its code.
        let (code, out, err) = crate::testutil::call(run, &ctx, &["rec", "--tmdb-search", "missing.json"]);
        assert_eq!(
            (code, out.as_str(), err.as_str()),
            (2, "", "lazymkv-dev plan: error: [Errno 2] No such file or directory: 'missing.json'\n")
        );
    }

    #[test]
    fn help_prints_the_usage_and_exits_0() {
        let tmp = Scratch::new("plan-help");
        let ctx = Ctx::for_test(&tmp.0);
        for flag in ["--help", "-h"] {
            let (code, out, err) = crate::testutil::call(run, &ctx, &[flag]);
            assert_eq!((code, err.as_str()), (0, ""));
            assert_eq!(out, format!("usage: lazymkv-dev {USAGE}\n{HELP}"));
        }
        assert!(HELP.contains("exit status: 0 when the plan is printed, with or without review reasons"), "{HELP}");
        assert!(HELP.contains("3 when a slug is given and the evidence clone is\nabsent; never 1."), "{HELP}");
    }

    #[test]
    fn the_command_is_registered_after_decide_with_its_synopsis() {
        let names: Vec<&str> = crate::COMMANDS.iter().map(|(name, _, _)| *name).collect();
        let decide = names.iter().position(|name| *name == "decide").expect("decide is registered");
        assert_eq!(names.get(decide + 1), Some(&"plan"), "{names:?}");
        let (_, _, synopsis) = crate::COMMANDS[decide + 1];
        assert_eq!(
            synopsis,
            "<slug|recording dir|scan file> [--evidence DIR] [--settings FILE] [--tmdb-search FILE] [--tmdb-record FILE] \
             [--remembered FILE]"
        );
        assert_eq!(USAGE.strip_prefix("plan "), Some(synopsis));
        // dispatch reaches the command by its name.
        let tmp = Scratch::new("plan-dispatch");
        film_recording(&tmp, "rec");
        let ctx = Ctx::for_test(&tmp.0);
        let argv = vec!["plan".to_string(), "rec".to_string()];
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(crate::dispatch(&ctx, &argv, &mut out, &mut err), 0);
        assert!(err.is_empty());
        assert!(String::from_utf8(out).unwrap().starts_with("{\n \"slug\": \"rec\",\n"));
    }
}
