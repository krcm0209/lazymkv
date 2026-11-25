//! The `review` request's worker (phase 3 design, part 6.2): the job's review document, filled with what only the
//! engine can read, built on a thread of its own because those reads touch the staging, tuning and library shares,
//! which the owner thread never reads. Subtitle lines go into the document only: nothing here logs them or puts them in
//! an error. Def: P3-T9. Use: P3-T13, P3-T14, P3-T16.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::UNIX_EPOCH;

use lazymkv_core::disc::Disc;
use lazymkv_core::lookup::stale::civil_date;
use lazymkv_core::record::{JobRecord, TuningCopy};
use lazymkv_core::review_doc::{self, DocInputs, ReviewDoc};
use lazymkv_core::settings::configured_root;
use lazymkv_core::subtitle::text::READING;
use lazymkv_dev::cmd::sheet::{job_scan, title_reading};
use lazymkv_dev::evidence::Ctx;
use lazymkv_platform::{Reach, ShareConnector};

use crate::file_phase::staged_text;
use crate::owner::{panic_text, JobContext};
use crate::steps::mkv_files;
use crate::tuning::DATASET;

/// The start of the answer of a review whose worker panicked (phase 3 design, part 6.2). Def: P3-T9. Use: P3-T13,
/// P3-T14, P3-T16.
pub(crate) const REVIEW_PANICKED: &str = "the review could not be built: ";

/// Builds `job`'s review document on a thread of its own and hands it to `reply` (phase 3 design, part 6.2): the owner
/// thread starts it, keeps the handle (a stop does not wait for it: it only reads, and a share can stall a read) and
/// sends the answer to the asking client. The worker reads `scan.txt` and each title's files from the job folder, the staged file's path and the OCR
/// reading's lines from the staging share (else the reading of `<tuning_root>\dataset\`), and the colliding library
/// files, asking each share with `shares.reach` first: a share that does not answer gives `unread` evidence or no
/// entry, never an error. A panic is answered as `REVIEW_PANICKED` and its text. Def: P3-T9. Use: P3-T13, P3-T14,
/// P3-T16.
pub(crate) fn spawn_review(
    job: JobContext,
    all: bool,
    shares: Arc<dyn ShareConnector>,
    reply: impl FnOnce(Result<ReviewDoc, String>) + Send + 'static,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let built = panic::catch_unwind(AssertUnwindSafe(|| build_review(&job, all, shares.as_ref())));
        reply(built.unwrap_or_else(|payload| Err(format!("{REVIEW_PANICKED}{}", panic_text(&*payload)))));
    })
}

/// What the engine read for the document (`DocInputs`' maps).
#[derive(Default)]
struct Read {
    staged: BTreeMap<String, String>,
    lines: BTreeMap<(String, u32), Vec<String>>,
    unread: BTreeMap<(String, u32), String>,
    collisions: BTreeMap<String, String>,
    nothing_at: BTreeSet<String>,
}

/// The document of `job` (phase 3 design, part 6.2): `review_doc::build` over the job folder's scan and what `read`
/// found. A job folder whose `scan.txt` cannot be read gives `review_doc::nothing_to_review`'s refusal first (such a
/// job may have no scan), else the read error naming the file (`job_scan`).
fn build_review(job: &JobContext, all: bool, shares: &dyn ShareConnector) -> Result<ReviewDoc, String> {
    let scanned = job_scan(&job.job_dir, &job.record)?;
    let read = read_evidence(job, &scanned, shares);
    review_doc::build(&inputs(&job.record, &scanned, &read, all))
}

fn inputs<'a>(record: &'a JobRecord, scanned: &'a Disc, read: &'a Read, all: bool) -> DocInputs<'a> {
    DocInputs {
        record,
        disc: scanned,
        staged: &read.staged,
        lines: &read.lines,
        unread: &read.unread,
        collisions: &read.collisions,
        nothing_at: &read.nothing_at,
        all,
    }
}

/// Every subtitle stream the decision of a title lists, kept or dropped: the streams that can be subtitle items.
fn subtitle_streams(record: &JobRecord) -> Vec<(String, u32)> {
    let mut streams = Vec::new();
    for title in &record.titles {
        let Some(decision) = &title.decision else { continue };
        let kept = decision.subtitles.kept.iter().map(|track| track.stream);
        let dropped = decision.subtitles.dropped.iter().map(|(stream, _)| *stream);
        streams.extend(kept.chain(dropped).map(|stream| (title.key.clone(), stream)));
    }
    streams
}

/// What only the engine can read for the document: with the staging root answering, each title's staged file (the
/// one `.mkv` of `<staging_root>\<job id>\rip\<key>\`, as `Step::staged_file` finds it, shown as its path) and the
/// lines of each subtitle stream step 10 read (`lines_of`); with it not answering, no path and no line, and every
/// subtitle stream `the staging share did not answer: <why>`. Then the colliding files (`collision_files`).
fn read_evidence(job: &JobContext, scanned: &Disc, shares: &dyn ShareConnector) -> Read {
    let mut read = Read::default();
    let staging = configured_root(job.settings.staging_root.as_deref());
    let mut tuning = TuningShare::new(job, shares);
    match staging.map(|root| (root, shares.reach(root))) {
        Some((_, Reach::Unreachable(why))) => {
            for at in subtitle_streams(&job.record) {
                read.unread.insert(at, format!("the staging share did not answer: {why}"));
            }
        }
        reached => {
            let root = reached.map(|(root, _)| root);
            for title in &job.record.titles {
                if let Some(root) = root {
                    let dir = Path::new(root).join(&job.record.id).join("rip").join(&title.key);
                    if let Ok([file]) = mkv_files(&dir).as_deref() {
                        read.staged.insert(title.key.clone(), file.display().to_string());
                    }
                }
                let seconds = scanned.title(title.scan_title).ok().and_then(|t| t.seconds).map_or(0.0, f64::from);
                lines_of(job, &title.key, seconds, &mut tuning, &mut read);
            }
        }
    }
    (read.collisions, read.nothing_at) = collision_files(job, shares);
    read
}

/// The tuning root of a job's own settings, asked once (phase 3 design, part 6.2: a job's dataset copy holds the
/// reading once the staged copy is gone).
struct TuningShare<'a> {
    root: Option<&'a str>,
    shares: &'a dyn ShareConnector,
    reach: Option<Reach>,
}

impl<'a> TuningShare<'a> {
    fn new(job: &'a JobContext, shares: &'a dyn ShareConnector) -> TuningShare<'a> {
        TuningShare { root: configured_root(job.settings.tuning_root.as_deref()), shares, reach: None }
    }

    /// `<tuning_root>\dataset\<job id>\<key>\subs-ocr\` when the root answers and it is a folder; Err with why the
    /// root did not answer.
    fn reading(&mut self, job: &JobContext, key: &str) -> Result<Option<PathBuf>, String> {
        let Some(root) = self.root else { return Ok(None) };
        let shares = self.shares;
        match self.reach.get_or_insert_with(|| shares.reach(root)) {
            Reach::Unreachable(why) => Err(why.clone()),
            Reach::Reachable => {
                let dir = Path::new(root).join(DATASET).join(&job.record.id).join(key).join(READING);
                Ok(Some(dir).filter(|dir| dir.is_dir()))
            }
        }
    }
}

/// The lines of the title `key` (phase 3 design, part 6.2; ruling T32-R5): the reading of `staged_text`, else the
/// dataset copy's; `subtitle_lines` over the job folder's `<key>` with the title's length `seconds`, each stream's
/// lines under (key, stream), and each stream the job folder's `subs-ocr/summary.json` records as not read
/// (`read_unread`) with that reason as `unread`. A title step 10 never read (no reading in either place) gets nothing;
/// a tuning share that does not answer, for a job whose dataset copy landed, gives each of the title's subtitle streams
/// `the tuning share did not answer: <why>`; a reading or job folder file that cannot be read gives each of them
/// `the subtitle reading could not be read: <error>`, whose error names the file and never holds the reading's text
/// (`subtitle_lines`).
fn lines_of(job: &JobContext, key: &str, seconds: f64, tuning: &mut TuningShare<'_>, read: &mut Read) {
    let streams: Vec<(String, u32)> = subtitle_streams(&job.record).into_iter().filter(|(at, _)| at == key).collect();
    let all_unread = |read: &mut Read, why: String| {
        for at in &streams {
            read.unread.entry(at.clone()).or_insert_with(|| why.clone());
        }
    };
    let text = match staged_text(job, key) {
        Some(text) => text,
        None => match tuning.reading(job, key) {
            Ok(Some(text)) => text,
            Ok(None) => return,
            Err(why) => {
                if job.record.tuning_copy == Some(TuningCopy::Done) {
                    all_unread(read, format!("the tuning share did not answer: {why}"));
                }
                return;
            }
        },
    };
    // No engine profile is named eng-only.mmcp.xml, so `scan_streams`' selection is Selection::All, as the decision's.
    match title_reading(&Ctx::from_process(), &job.job_dir, key, &text, seconds) {
        Ok((lines, skipped)) => {
            for (stream, shown) in lines {
                read.lines.insert((key.to_string(), stream), shown);
            }
            for (stream, why) in skipped {
                read.unread.insert((key.to_string(), stream), why);
            }
        }
        Err(error) => all_unread(read, format!("the subtitle reading could not be read: {error}")),
    }
}

/// The file at each of `record.collisions` and the paths with nothing at them (phase 3 design, parts 6.2, 6.7): under
/// the library root of the job's own settings (`movies_root` or `tv_root`) that holds it, once that root answers, a
/// path that is a file (or a link to one) gives `<bytes> bytes, <YYYY-MM-DD>` of its size and modification day (UTC),
/// and a path that `fs::symlink_metadata` says is not found goes in the set (a folder the share would not open has
/// opened since, and the title files there). A path that holds a folder or a link to anything else, is under no
/// configured root or under a root that does not answer, or whose read fails any other way is in neither. Runs on the
/// review worker's thread, never the owner's.
fn collision_files(job: &JobContext, shares: &dyn ShareConnector) -> (BTreeMap<String, String>, BTreeSet<String>) {
    let roots: Vec<&str> =
        [&job.settings.movies_root, &job.settings.tv_root].into_iter().filter_map(|root| configured_root(root.as_deref())).collect();
    let mut answered: BTreeMap<&str, bool> = BTreeMap::new();
    let mut files = BTreeMap::new();
    let mut nothing_at = BTreeSet::new();
    for path in &job.record.collisions {
        let Some(root) = roots.iter().copied().find(|root| Path::new(path).starts_with(root)) else { continue };
        if !*answered.entry(root).or_insert_with(|| shares.reach(root) == Reach::Reachable) {
            continue;
        }
        // `symlink_metadata` first: a link is something at the path even when its target is gone.
        match fs::symlink_metadata(path) {
            Err(error) if error.kind() == ErrorKind::NotFound => {
                nothing_at.insert(path.clone());
                continue;
            }
            Err(_) => continue,
            Ok(_) => {}
        }
        let meta = match fs::metadata(path) {
            Ok(meta) if meta.is_file() => meta,
            _ => continue,
        };
        let secs = meta.modified().ok().and_then(|time| time.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
        let (year, month, day) = civil_date(secs);
        files.insert(path.clone(), format!("{} bytes, {year:04}-{month:02}-{day:02}", meta.len()));
    }
    (files, nothing_at)
}
