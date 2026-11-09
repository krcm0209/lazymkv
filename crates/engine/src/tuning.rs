//! The tuning dataset copy (design part 7.5, EVID-4; spec 7 "Tuning evidence"; call 10, C2; T5-R13, T5-R14, T5-R46,
//! T5-R55): each title's staged tuning folder and evidence file to `<tuning_root>\dataset\<job id>\<title key>\`,
//! outside the reducer; and the recordings refresh (design part 7.5; call 11; T5-R15, T5-R16): the job folder's
//! changed files to `<tuning_root>\recordings\<job id>\`, `state.json` first.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use lazymkv_core::lookup::stale::TMDB_COPIES;
use lazymkv_platform::{write_atomic, Reach, ShareConnector, TEMP_SUFFIX};

use crate::staged::{copy_checked, copy_checked_with, copy_into_job, evidence_file, tuning_dir, PARTIAL};

/// The dataset tree's folder under `tuning_root` (design part 7.5). Def: T5-19. Use: T5-19, T5-22.
pub(crate) const DATASET: &str = "dataset";

/// job.log's line when a copy landed (design part 7.1's "a copy to `tuning_root` retried"). Def: T5-19. Use: T5-19.
pub(crate) const COPIED: &str = "tuning evidence copied to tuning_root";

/// The prefix of a level folder's name (design part 7.5), whose `audio-proxy.json` is the folder's marker (T5-R13).
const LEVEL_PREFIX: &str = "audio-levels-20ms-";
/// A level folder's marker, and the title's proxy description (T5-R13).
const PROXY_JSON: &str = "audio-proxy.json";

/// job.log's line when a copy stays pending: `tuning copy pending: <why>` for the attempt after DecisionDone,
/// `tuning copy retried: still pending: <why>` for a retry (design part 7.1). Def: T5-19. Use: T5-19.
pub(crate) fn pending_line(retry: bool, why: &str) -> String {
    if retry {
        format!("tuning copy retried: still pending: {why}")
    } else {
        format!("tuning copy pending: {why}")
    }
}

/// The copy step a test puts in place of the standard file copy (Review Focus 1), as `staged::copy_checked_with` takes
/// it.
/// Def: T5-19. Use: T5-19.
pub(crate) type CopyFn = Arc<dyn Fn(&Path, &Path) -> io::Result<u64> + Send + Sync>;

/// One job's copy as the owner hands it to a copy thread (design part 7.5): its id, its folder under `jobs\`, its own
/// settings' `staging_root` and `tuning_root` (ENG-19), and its plan's title keys. Def: T5-19. Use: T5-19.
#[derive(Clone, Debug)]
pub(crate) struct TuningJob {
    pub(crate) id: String,
    pub(crate) job_dir: PathBuf,
    pub(crate) staging_root: String,
    pub(crate) tuning_root: String,
    pub(crate) keys: Vec<String>,
}

/// Whether `path` is a level folder's `audio-proxy.json` (T5-R13).
fn is_level_marker(path: &Path) -> bool {
    path.file_name().is_some_and(|name| name == PROXY_JSON)
        && path
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(LEVEL_PREFIX))
}

/// `error` with `path` before it, unless its text names the path already (the checked copy's size error does).
fn located(path: &Path, error: &io::Error) -> String {
    let (text, shown) = (error.to_string(), path.display().to_string());
    if text.contains(&shown) {
        text
    } else {
        format!("{shown}: {text}")
    }
}

/// Every file under `from`, each folder's entries in name order and a folder walked where its name sorts, paired with
/// the same relative path under `to`; level markers go to `markers` instead.
fn walk(
    from: &Path,
    to: &Path,
    pairs: &mut Vec<(PathBuf, PathBuf)>,
    markers: &mut Vec<(PathBuf, PathBuf)>,
) -> io::Result<()> {
    let named = |error: io::Error| io::Error::new(error.kind(), format!("{}: {error}", from.display()));
    let mut entries = Vec::new();
    for entry in fs::read_dir(from).map_err(named)? {
        let entry = entry.map_err(named)?;
        entries.push((entry.file_name(), entry.path()));
    }
    entries.sort();
    for (name, path) in entries {
        let target = to.join(&name);
        if path.is_dir() {
            walk(&path, &target, pairs, markers)?;
        } else if is_level_marker(&path) {
            markers.push((path, target));
        } else {
            pairs.push((path, target));
        }
    }
    Ok(())
}

/// Every (source, target) pair of the job's dataset copy, title by title in `keys` order (design part 7.5; T5-R13,
/// T5-R46). Def: T5-19. Use: T5-19.
pub(crate) fn dataset_files(job: &TuningJob) -> io::Result<Vec<(PathBuf, PathBuf)>> {
    let mut pairs = Vec::new();
    for key in &job.keys {
        let staged = tuning_dir(&job.staging_root, &job.id, key);
        let target = Path::new(&job.tuning_root).join(DATASET).join(&job.id).join(key);
        let mut markers = Vec::new();
        walk(&staged, &target, &mut pairs, &mut markers)?;
        let evidence = evidence_file(&job.staging_root, &job.id, key);
        if evidence.is_file() {
            pairs.push((evidence, target.join("subs-ocr").join("evidence.mks")));
        }
        pairs.extend(markers);
    }
    Ok(pairs)
}

/// How a copy attempt ended (call 10; T5-R14): `Nothing` is a job none of whose titles has a staged tuning folder
/// while the staging share answers, which owes nothing (C2); `Waiting` is spec 7's unreachable or short `tuning_root`,
/// which adds the note; `Failed` is any other file that did not land, which keeps the copy pending without the note;
/// `Stopped` is the engine's stop. Def: T5-19. Use: T5-19, T5-20.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CopyEnd {
    /// Every file landed, the level markers last.
    Landed,
    /// No title has a staged tuning folder: nothing is owed (C2).
    Nothing,
    /// `tuning_root` did not answer or is short of space (spec 7); the text says why.
    Waiting(String),
    /// A file did not land for another reason; the text says why.
    Failed(String),
    /// The engine's stop flag was raised.
    Stopped,
}

/// `tuning_root`'s answer after a file did not land: `Waiting` when it no longer answers, else `Failed(why)`.
fn after_error(job: &TuningJob, shares: &dyn ShareConnector, why: String) -> CopyEnd {
    match shares.reach(&job.tuning_root) {
        Reach::Unreachable(reason) => {
            CopyEnd::Waiting(format!("tuning_root {} did not answer: {reason}", job.tuning_root))
        }
        Reach::Reachable => CopyEnd::Failed(why),
    }
}

/// The dataset copy of one job (design part 7.5, EVID-4; T5-R13, T5-R55): the share's reach and free space first,
/// each file through `staged::copy_checked` (or `copy_checked_with(.., copy)` when a test gives `copy`), `stop` read
/// before each file, and after the last file each title's level-folder `audio-proxy.json` copied into the job folder's
/// level folder; a staged folder that cannot be read keeps the copy owed (C2). Def: T5-19. Use: T5-19.
pub(crate) fn copy_dataset(
    job: &TuningJob,
    shares: &dyn ShareConnector,
    stop: &AtomicBool,
    copy: Option<&dyn Fn(&Path, &Path) -> io::Result<u64>>,
) -> CopyEnd {
    if stop.load(Ordering::SeqCst) {
        return CopyEnd::Stopped;
    }
    // C2: checked here, on the copy thread, so the owner thread asks no share; a staging share that does not answer
    // proves nothing, and the read error below keeps the copy owed.
    let staging_answers = !matches!(shares.reach(&job.staging_root), Reach::Unreachable(_));
    if staging_answers {
        let mut staged = false;
        for key in &job.keys {
            let dir = tuning_dir(&job.staging_root, &job.id, key);
            match fs::metadata(&dir) {
                // A file in the folder's place is not a staged folder.
                Ok(metadata) => staged |= metadata.is_dir(),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                // Anything else proves nothing, so the copy stays owed and is retried (C2).
                Err(error) => return CopyEnd::Failed(format!("{}: {error}", dir.display())),
            }
        }
        if !staged {
            return CopyEnd::Nothing;
        }
    }
    let root = &job.tuning_root;
    if let Reach::Unreachable(why) = shares.reach(root) {
        return CopyEnd::Waiting(format!("tuning_root {root} did not answer: {why}"));
    }
    let pairs = match dataset_files(job) {
        Ok(pairs) => pairs,
        Err(error) => return CopyEnd::Failed(error.to_string()),
    };
    let mut need: u64 = 0;
    for (from, _) in &pairs {
        match fs::metadata(from) {
            Ok(metadata) => need += metadata.len(),
            Err(error) => return CopyEnd::Failed(format!("{}: {error}", from.display())),
        }
    }
    match shares.free_bytes(root) {
        Err(error) => return CopyEnd::Waiting(format!("tuning_root {root}: the free space could not be read: {error}")),
        Ok(free) if free < need => {
            let why = format!("tuning_root {root} has {free} bytes free, and the copy needs {need} bytes");
            return CopyEnd::Waiting(why);
        }
        Ok(_) => {}
    }
    for (from, to) in &pairs {
        if stop.load(Ordering::SeqCst) {
            return CopyEnd::Stopped;
        }
        let landed = to.parent().map_or(Ok(()), fs::create_dir_all).and_then(|()| match copy {
            Some(copy) => copy_checked_with(from, to, copy),
            None => copy_checked(from, to),
        });
        if let Err(error) = landed {
            return after_error(job, shares, located(to, &error));
        }
    }
    // T5-R13: the job folder's level marker last, once the dataset holds every file; the folder exists from step 10a.
    for (from, to) in pairs.iter().filter(|(from, _)| is_level_marker(from)) {
        let (Some(level), Some(key)) = (to.parent(), to.parent().and_then(Path::parent)) else { continue };
        let (Some(level), Some(key)) = (level.file_name(), key.file_name()) else { continue };
        let marker = job.job_dir.join(key).join(level).join(PROXY_JSON);
        if let Err(error) = copy_into_job(from, &marker) {
            return CopyEnd::Failed(located(&marker, &error));
        }
    }
    CopyEnd::Landed
}

/// The recordings tree under `tuning_root`, `<tuning_root>\recordings` (design part 7.5). Def: T5-20. Use: T5-20, T5-22.
pub(crate) const RECORDINGS: &str = "recordings";

/// The refresh's manifest in `<tuning_root>\recordings\<job id>\`, naming each file it wrote with the source's size and
/// modification time; those are its own earlier copies (T5-R16). Def: T5-20. Use: T5-20, T5-22.
pub(crate) const REFRESH_MANIFEST: &str = ".lazymkv-refresh.json";

/// The manifest's `format` (design part 7.4's versioned files). Def: T5-20. Use: T5-20.
pub(crate) const REFRESH_FORMAT: &str = "lazymkv refresh v1";

/// The owner's files, which the refresh never writes or deletes (design part 7.1; T5-R15). Def: T5-20. Use: T5-20.
pub(crate) const OWNER_FILES: [&str; 3] = ["audio-labels.json", "subtitle-labels.json", "notes.md"];

/// What one refresh did (design part 7.5): how many files it copied and the manifest paths it deleted. Def: T5-20.
/// Use: T5-20.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct RefreshEnd {
    /// How many files the refresh copied.
    pub(crate) copied: usize,
    /// The manifest paths the refresh deleted, the job folder no longer holding them.
    pub(crate) deleted: Vec<String>,
}

/// The refresh's manifest (T5-R16): its format and each file it wrote, sorted by path.
#[derive(serde::Serialize, serde::Deserialize)]
struct Manifest {
    format: String,
    files: Vec<ManifestEntry>,
}

/// One file the refresh wrote: its path relative to the job folder with `/` separators, and the source's size and
/// modification time (nanoseconds since 1970, 0 for an earlier time) as read just before its copy.
#[derive(serde::Serialize, serde::Deserialize)]
struct ManifestEntry {
    path: String,
    bytes: u64,
    modified_ns: u64,
}

/// `error` with `path` before it (as `located`), its kind kept.
fn at(path: &Path, error: &io::Error) -> io::Error {
    io::Error::new(error.kind(), located(path, error))
}

/// Whether the refresh leaves a file of this name alone: the owner's files, its own manifest, and the temporary files
/// of `write_atomic` and the checked copy.
fn refresh_skips(name: &str) -> bool {
    OWNER_FILES.contains(&name) || name == REFRESH_MANIFEST || name.ends_with(TEMP_SUFFIX) || name.ends_with(PARTIAL)
}

/// Every regular file under `dir` the refresh copies, relative to `root` with `/` separators; a link or a junction is
/// neither followed nor listed.
fn refresh_sources(root: &Path, dir: &Path, found: &mut Vec<String>) -> io::Result<()> {
    for entry in fs::read_dir(dir).map_err(|error| at(dir, &error))? {
        let path = entry.map_err(|error| at(dir, &error))?.path();
        let kind = fs::symlink_metadata(&path).map_err(|error| at(&path, &error))?.file_type();
        if kind.is_dir() {
            refresh_sources(root, &path, found)?;
        } else if kind.is_file() {
            let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
            if refresh_skips(&name) {
                continue;
            }
            let Ok(relative) = path.strip_prefix(root) else { continue };
            let parts: Vec<String> =
                relative.components().map(|part| part.as_os_str().to_string_lossy().into_owned()).collect();
            found.push(parts.join("/"));
        }
    }
    Ok(())
}

/// The manifest in `target`; a missing file, a parse error or another format is an empty one.
fn read_manifest(target: &Path) -> Vec<ManifestEntry> {
    let Ok(bytes) = fs::read(target.join(REFRESH_MANIFEST)) else { return Vec::new() };
    match serde_json::from_slice::<Manifest>(&bytes) {
        Ok(manifest) if manifest.format == REFRESH_FORMAT => manifest.files,
        _ => Vec::new(),
    }
}

/// Writes `files`, sorted by path, as `target`'s manifest through `write_atomic`.
fn write_manifest(target: &Path, mut files: Vec<ManifestEntry>) -> io::Result<()> {
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let manifest = Manifest { format: REFRESH_FORMAT.to_string(), files };
    let mut bytes = serde_json::to_vec_pretty(&manifest).map_err(io::Error::other)?;
    bytes.push(b'\n');
    write_atomic(&target.join(REFRESH_MANIFEST), &bytes)
}

/// `relative`, a manifest path with `/` separators, under `dir`.
fn under(dir: &Path, relative: &str) -> PathBuf {
    relative.split('/').fold(dir.to_path_buf(), |path, part| path.join(part))
}

/// Steps 3 to 5 of `refresh_recordings`: the target folder, the deletions and the copies, `files` and `end` holding
/// what landed when an error ends them.
fn refresh_into(
    job_dir: &Path,
    target: &Path,
    sources: &[String],
    files: &mut Vec<ManifestEntry>,
    end: &mut RefreshEnd,
) -> io::Result<()> {
    fs::create_dir_all(target).map_err(|error| at(target, &error))?;
    let mut index = 0;
    while index < files.len() {
        let path = files[index].path.clone();
        let gone = TMDB_COPIES.contains(&path.as_str())
            && matches!(fs::symlink_metadata(job_dir.join(&path)), Err(error) if error.kind() == io::ErrorKind::NotFound);
        if !gone {
            index += 1;
            continue;
        }
        let copy = target.join(&path);
        match fs::remove_file(&copy) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(at(&copy, &error)),
            _ => {}
        }
        files.remove(index);
        end.deleted.push(path);
    }
    for path in sources {
        let (from, to) = (under(job_dir, path), under(target, path));
        let metadata = fs::metadata(&from).map_err(|error| at(&from, &error))?;
        let modified = metadata.modified().map_err(|error| at(&from, &error))?;
        let modified_ns = modified
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| u64::try_from(since.as_nanos()).unwrap_or(u64::MAX));
        let entry = ManifestEntry { path: path.clone(), bytes: metadata.len(), modified_ns };
        let known = files.iter().position(|known| known.path == *path);
        let unchanged = known.is_some_and(|index| {
            files[index].bytes == entry.bytes && files[index].modified_ns == entry.modified_ns
        });
        if unchanged && to.exists() {
            continue;
        }
        to.parent().map_or(Ok(()), fs::create_dir_all).map_err(|error| at(&to, &error))?;
        copy_checked(&from, &to).map_err(|error| at(&to, &error))?;
        match known {
            Some(index) => files[index] = entry,
            None => files.push(entry),
        }
        end.copied += 1;
    }
    Ok(())
}

/// Refreshes `target`, `<tuning_root>\recordings\<job id>\`, from `job_dir` (design part 7.5; call 11; T5-R15, T5-R16):
/// copies each file whose size or modification time changed since its copy, `state.json` first, never writes or
/// deletes `OWNER_FILES`, and deletes only a TMDb copy the manifest lists that `job_dir` no longer holds. Def: T5-20.
/// Use: T5-20, T5-22.
pub(crate) fn refresh_recordings(job_dir: &Path, target: &Path) -> io::Result<RefreshEnd> {
    let mut sources = Vec::new();
    refresh_sources(job_dir, job_dir, &mut sources)?;
    sources.sort();
    if let Some(index) = sources.iter().position(|path| path == "state.json") {
        let state = sources.remove(index);
        sources.insert(0, state);
    }
    let mut files = read_manifest(target);
    let mut end = RefreshEnd { copied: 0, deleted: Vec::new() };
    if let Err(error) = refresh_into(job_dir, target, &sources, &mut files, &mut end) {
        let _ = write_manifest(target, files);
        return Err(error);
    }
    write_manifest(target, files)?;
    Ok(end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lazymkv_core::testsupport::{Scratch, ScratchExt};
    use lazymkv_platform::fakes::FakeShares;
    use lazymkv_platform::Reach;
    use std::cell::RefCell;
    use std::sync::atomic::Ordering;

    const ID: &str = "abc-1";
    const KEY: &str = "t00-00800";
    const LEVEL: &str = "audio-levels-20ms-1-2";
    /// A made-up record line holding only a sentinel (RP21).
    const RECORD: &str = "{\"text\": \"text-sentinel-3e8f\"}\n";
    const PROXY_JSON: &str = "{\"format\": \"lazymkv audio proxy v1\", \"proxy\": \"audio-proxy.mka\"}\n";

    /// A job over `<scratch>/{jobs/abc-1, staging, tuning}`: one title whose staged folder holds a proxy, a level
    /// folder with its marker, and one read stream; its evidence file; and its job folder's level folder, empty.
    fn staged_job(folders: &Scratch) -> TuningJob {
        let staged = format!("staging/{ID}/tuning/{KEY}");
        folders.write(&format!("{staged}/{LEVEL}/main-mix.txt"), "series\r\n");
        folders.write(&format!("{staged}/{LEVEL}/streamhash.txt"), "md5\n");
        folders.write(&format!("{staged}/{LEVEL}/audio-proxy.json"), PROXY_JSON);
        folders.write(&format!("{staged}/audio-proxy.mka"), "mka-bytes");
        folders.write(&format!("{staged}/audio-proxy.json"), PROXY_JSON);
        folders.write(&format!("{staged}/subs-ocr/sub-0.ocr.jsonl"), RECORD);
        folders.write(&format!("{staged}/subs-ocr/sub-0.run.json"), "{}\n");
        folders.write(&format!("{staged}/subs-ocr/s0/f-000001000.png"), "png");
        folders.write(&format!("{staged}/subs-ocr/summary.json"), "{}\n");
        folders.write(&format!("staging/{ID}/evidence/{KEY}.mks"), "lazymkv-test-mks");
        fs::create_dir_all(folders.0.join("jobs").join(ID).join(KEY).join(LEVEL)).unwrap();
        fs::create_dir_all(folders.0.join("tuning")).unwrap();
        TuningJob {
            id: ID.to_string(),
            job_dir: folders.0.join("jobs").join(ID),
            staging_root: folders.0.join("staging").display().to_string(),
            tuning_root: folders.0.join("tuning").display().to_string(),
            keys: vec![KEY.to_string()],
        }
    }

    fn staged(job: &TuningJob) -> PathBuf {
        tuning_dir(&job.staging_root, ID, KEY)
    }

    fn dataset(job: &TuningJob) -> PathBuf {
        Path::new(&job.tuning_root).join(DATASET).join(ID).join(KEY)
    }

    /// `path` relative to `root`, with '/' separators.
    fn rel(root: &Path, path: &Path) -> String {
        path.strip_prefix(root).unwrap().to_str().unwrap().replace('\\', "/")
    }

    /// Every file under `dir`, relative, sorted; empty when `dir` does not exist.
    fn files(dir: &Path) -> Vec<String> {
        fn walk(root: &Path, dir: &Path, found: &mut Vec<String>) {
            let Ok(entries) = fs::read_dir(dir) else { return };
            for entry in entries {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    walk(root, &path, found);
                } else {
                    found.push(rel(root, &path));
                }
            }
        }
        let mut found = Vec::new();
        walk(dir, dir, &mut found);
        found.sort();
        found
    }

    /// A copy step that writes the bytes and reports their count, as the standard file copy does.
    fn plain(from: &Path, to: &Path) -> io::Result<u64> {
        fs::write(to, fs::read(from)?)?;
        Ok(fs::metadata(to)?.len())
    }

    #[test]
    fn dataset_files_lists_the_staged_folder_in_name_order_then_evidence_mks_then_the_level_marker() {
        let folders = Scratch::new("tuning-files");
        let job = staged_job(&folders);
        let (staged, dataset) = (staged(&job), dataset(&job));
        let pairs = dataset_files(&job).unwrap();
        let targets: Vec<String> = pairs.iter().map(|(_, to)| rel(&dataset, to)).collect();
        assert_eq!(
            targets,
            [
                "audio-levels-20ms-1-2/main-mix.txt",
                "audio-levels-20ms-1-2/streamhash.txt",
                "audio-proxy.json",
                "audio-proxy.mka",
                "subs-ocr/s0/f-000001000.png",
                "subs-ocr/sub-0.ocr.jsonl",
                "subs-ocr/sub-0.run.json",
                "subs-ocr/summary.json",
                "subs-ocr/evidence.mks",
                "audio-levels-20ms-1-2/audio-proxy.json",
            ]
        );
        for (from, to) in &pairs[..8] {
            assert_eq!(rel(&staged, from), rel(&dataset, to));
        }
        assert_eq!(pairs[8].0, evidence_file(&job.staging_root, ID, KEY), "T5-R46");
        assert_eq!(pairs[9].0, staged.join(LEVEL).join("audio-proxy.json"));
        fs::remove_file(evidence_file(&job.staging_root, ID, KEY)).unwrap();
        let pairs = dataset_files(&job).unwrap();
        assert_eq!(pairs.len(), 9, "a title without step 10 has no evidence file");
        assert!(pairs.iter().all(|(_, to)| !to.ends_with("evidence.mks")));
    }

    #[test]
    fn the_dataset_copy_lands_each_file_through_partial_and_writes_audio_proxy_json_last() {
        let folders = Scratch::new("tuning-copy");
        let job = staged_job(&folders);
        let (staged, dataset) = (staged(&job), dataset(&job));
        let written = RefCell::new(Vec::new());
        let copy = |from: &Path, to: &Path| -> io::Result<u64> {
            written.borrow_mut().push(to.to_path_buf());
            plain(from, to)
        };
        let end = copy_dataset(&job, &FakeShares::new(), &AtomicBool::new(false), Some(&copy));
        assert_eq!(end, CopyEnd::Landed);
        let written = written.into_inner();
        // Ten dataset files through the seam; the job folder's marker goes through copy_into_job (write_atomic), after them.
        assert_eq!(written.len(), 10, "{written:?}");
        assert!(written.iter().all(|to| to.to_str().unwrap().ends_with(".partial")), "{written:?}");
        assert_eq!(written[9], dataset.join(LEVEL).join("audio-proxy.json.partial"));
        assert_eq!(
            files(&dataset),
            [
                "audio-levels-20ms-1-2/audio-proxy.json",
                "audio-levels-20ms-1-2/main-mix.txt",
                "audio-levels-20ms-1-2/streamhash.txt",
                "audio-proxy.json",
                "audio-proxy.mka",
                "subs-ocr/evidence.mks",
                "subs-ocr/s0/f-000001000.png",
                "subs-ocr/sub-0.ocr.jsonl",
                "subs-ocr/sub-0.run.json",
                "subs-ocr/summary.json",
            ]
        );
        for file in files(&staged) {
            assert_eq!(fs::read(dataset.join(&file)).unwrap(), fs::read(staged.join(&file)).unwrap(), "{file}");
        }
        assert_eq!(fs::read(dataset.join("subs-ocr").join("evidence.mks")).unwrap(), b"lazymkv-test-mks");
        assert!(!staged.join("subs-ocr").join("evidence.mks").exists(), "T5-R46: the staged folder holds no second copy");
        assert_eq!(files(&job.job_dir.join(KEY)), ["audio-levels-20ms-1-2/audio-proxy.json"]);
        assert_eq!(fs::read(job.job_dir.join(KEY).join(LEVEL).join("audio-proxy.json")).unwrap(), PROXY_JSON.as_bytes());
    }

    #[test]
    fn an_unreachable_tuning_root_copies_nothing_and_waits() {
        let folders = Scratch::new("tuning-unreachable");
        let job = staged_job(&folders);
        let shares = FakeShares::new();
        shares.set_reach(&job.tuning_root, Reach::Unreachable("cable out".into()));
        let end = copy_dataset(&job, &shares, &AtomicBool::new(false), None);
        assert_eq!(end, CopyEnd::Waiting(format!("tuning_root {} did not answer: cable out", job.tuning_root)));
        assert!(files(Path::new(&job.tuning_root)).is_empty());
        assert!(files(&job.job_dir).is_empty());
        assert_eq!(shares.free_reads(&job.tuning_root), 0);
    }

    #[test]
    fn a_tuning_root_short_of_space_copies_nothing_and_waits() {
        let folders = Scratch::new("tuning-short");
        let job = staged_job(&folders);
        let need: u64 = dataset_files(&job).unwrap().iter().map(|(from, _)| fs::metadata(from).unwrap().len()).sum();
        let shares = FakeShares::new();
        shares.set_free(&job.tuning_root, 10);
        let end = copy_dataset(&job, &shares, &AtomicBool::new(false), None);
        let why = format!("tuning_root {} has 10 bytes free, and the copy needs {need} bytes", job.tuning_root);
        assert_eq!(end, CopyEnd::Waiting(why));
        assert!(files(Path::new(&job.tuning_root)).is_empty());
    }

    /// Review Focus 1: the share goes away in the middle of the copy.
    #[test]
    fn a_share_lost_during_the_copy_waits_and_writes_no_marker() {
        let folders = Scratch::new("tuning-lost");
        let job = staged_job(&folders);
        let shares = FakeShares::new();
        let lost = "the network name is no longer available";
        let copy = |from: &Path, to: &Path| -> io::Result<u64> {
            if to.file_name().unwrap() == "sub-0.ocr.jsonl.partial" {
                shares.set_reach(&job.tuning_root, Reach::Unreachable(lost.into()));
                return Err(io::Error::other(lost));
            }
            plain(from, to)
        };
        let end = copy_dataset(&job, &shares, &AtomicBool::new(false), Some(&copy));
        assert_eq!(end, CopyEnd::Waiting(format!("tuning_root {} did not answer: {lost}", job.tuning_root)));
        assert_eq!(
            files(&dataset(&job)),
            [
                "audio-levels-20ms-1-2/main-mix.txt",
                "audio-levels-20ms-1-2/streamhash.txt",
                "audio-proxy.json",
                "audio-proxy.mka",
                "subs-ocr/s0/f-000001000.png",
            ]
        );
        assert!(files(&job.job_dir).is_empty(), "the job folder's marker waits for the last file");
    }

    /// Review Focus 1: a file whose size differs after the write is not renamed, and the copy stays pending.
    #[test]
    fn a_copy_whose_size_differs_fails_and_writes_no_marker() {
        let folders = Scratch::new("tuning-short-file");
        let job = staged_job(&folders);
        let copy = |from: &Path, to: &Path| -> io::Result<u64> {
            if to.file_name().unwrap() == "audio-proxy.mka.partial" {
                fs::write(to, &fs::read(from)?[..4])?;
                return Ok(9);
            }
            plain(from, to)
        };
        let end = copy_dataset(&job, &FakeShares::new(), &AtomicBool::new(false), Some(&copy));
        let (partial, from) = (dataset(&job).join("audio-proxy.mka.partial"), staged(&job).join("audio-proxy.mka"));
        let why = format!("{}: 4 bytes after the copy, not the 9 bytes of {}", partial.display(), from.display());
        assert_eq!(end, CopyEnd::Failed(why));
        assert_eq!(
            files(&dataset(&job)),
            ["audio-levels-20ms-1-2/main-mix.txt", "audio-levels-20ms-1-2/streamhash.txt", "audio-proxy.json"]
        );
        assert!(files(&job.job_dir).is_empty());
    }

    #[test]
    fn a_raised_stop_flag_ends_the_copy_before_the_next_file() {
        let folders = Scratch::new("tuning-stop");
        let job = staged_job(&folders);
        let stop = AtomicBool::new(false);
        let copy = |from: &Path, to: &Path| -> io::Result<u64> {
            stop.store(true, Ordering::SeqCst);
            plain(from, to)
        };
        assert_eq!(copy_dataset(&job, &FakeShares::new(), &stop, Some(&copy)), CopyEnd::Stopped);
        assert_eq!(files(&dataset(&job)), ["audio-levels-20ms-1-2/main-mix.txt"]);
    }

    #[test]
    fn a_title_without_a_staged_folder_fails_the_copy() {
        let folders = Scratch::new("tuning-missing");
        let mut job = staged_job(&folders);
        // A second title that has no staged folder while the first has one: the copy is owed and cannot be made.
        job.keys.push("t01-00801".to_string());
        let missing = tuning_dir(&job.staging_root, ID, "t01-00801");
        let CopyEnd::Failed(why) = copy_dataset(&job, &FakeShares::new(), &AtomicBool::new(false), None) else {
            panic!("a missing staged folder beside a staged one is a failed copy");
        };
        assert!(why.starts_with(&format!("{}: ", missing.display())), "{why}");
        assert!(files(Path::new(&job.tuning_root)).is_empty());
    }

    /// C2: a job none of whose titles has a staged tuning folder owes nothing, as pull request 4's jobs and a job held
    /// under its build; a staging share that does not answer proves nothing, so that copy stays owed.
    #[test]
    fn a_job_with_no_staged_tuning_folder_owes_nothing() {
        let folders = Scratch::new("tuning-nothing");
        let job = staged_job(&folders);
        fs::remove_dir_all(staged(&job)).unwrap();
        let shares = FakeShares::new();
        assert_eq!(copy_dataset(&job, &shares, &AtomicBool::new(false), None), CopyEnd::Nothing);
        assert!(files(Path::new(&job.tuning_root)).is_empty());
        assert_eq!(shares.free_reads(&job.tuning_root), 0, "nothing owed asks tuning_root nothing");
        shares.set_reach(&job.staging_root, Reach::Unreachable("cable out".into()));
        let end = copy_dataset(&job, &shares, &AtomicBool::new(false), None);
        assert!(matches!(end, CopyEnd::Failed(_)), "{end:?}");
    }

    /// C2: a staged folder whose metadata cannot be read is no proof that nothing is owed, so the copy stays owed.
    #[test]
    fn a_staged_folder_that_cannot_be_read_keeps_the_copy_owed() {
        let folders = Scratch::new("tuning-unreadable");
        let job = staged_job(&folders);
        let tuning = Path::new(&job.staging_root).join(ID).join("tuning");
        fs::remove_dir_all(&tuning).unwrap();
        fs::write(&tuning, "not a folder").unwrap();
        let end = copy_dataset(&job, &FakeShares::new(), &AtomicBool::new(false), None);
        // Unix reports a file used as a folder as NotADirectory, an error that is not NotFound; Windows reports the
        // path as not found (ERROR_PATH_NOT_FOUND, read as NotFound), so there the key is simply not staged.
        #[cfg(unix)]
        assert!(
            matches!(&end, CopyEnd::Failed(why) if why.starts_with(&format!("{}: ", staged(&job).display()))),
            "{end:?}"
        );
        #[cfg(windows)]
        assert_eq!(end, CopyEnd::Nothing);
        assert!(files(Path::new(&job.tuning_root)).is_empty());
    }

    /// The recordings refresh (design part 7.5; call 11; T5-R15, T5-R16).
    mod refresh {
        use super::super::*;
        use lazymkv_core::testsupport::{Scratch, ScratchExt};
        use lazymkv_dev::evidence::Ctx;
        use lazymkv_dev::stale::{refresh_copies, JOB_STATE_FILE};
        use serde_json::Value;
        use std::fs;
        use std::path::{Path, PathBuf};
        use std::time::{Duration, UNIX_EPOCH};

        const ID: &str = "0000000000000000000000000000000a-1";
        /// The modification time every source file starts with, in seconds since 1970.
        const T0: u64 = 1_792_800_000;

        /// Sets `path`'s modification time to `secs` seconds after 1970.
        fn touch(path: &Path, secs: u64) {
            let file = fs::File::options().write(true).open(path).unwrap();
            file.set_modified(UNIX_EPOCH + Duration::from_secs(secs)).unwrap();
        }

        /// A scratch holding the job folder `jobs/<ID>` with `files` (relative path, text), each modified at `T0`,
        /// with the job folder and the target `tuning/recordings/<ID>`.
        fn folders(name: &str, files: &[(&str, &str)]) -> (Scratch, PathBuf, PathBuf) {
            let scratch = Scratch::new(name);
            for (path, text) in files {
                touch(&scratch.write_path(&format!("jobs/{ID}/{path}"), *text), T0);
            }
            let job = scratch.0.join("jobs").join(ID);
            let target = scratch.0.join("tuning").join("recordings").join(ID);
            (scratch, job, target)
        }

        /// Every file under `dir`, relative with '/' separators, sorted.
        fn files(dir: &Path) -> Vec<String> {
            fn walk(root: &Path, dir: &Path, found: &mut Vec<String>) {
                for entry in fs::read_dir(dir).unwrap() {
                    let path = entry.unwrap().path();
                    if path.is_dir() {
                        walk(root, &path, found);
                    } else {
                        found.push(path.strip_prefix(root).unwrap().to_str().unwrap().replace('\\', "/"));
                    }
                }
            }
            let mut found = Vec::new();
            walk(dir, dir, &mut found);
            found.sort();
            found
        }

        /// The `path` of each entry of the target's manifest, in its order.
        fn listed(target: &Path) -> Vec<String> {
            let manifest: Value = serde_json::from_slice(&fs::read(target.join(REFRESH_MANIFEST)).unwrap()).unwrap();
            manifest["files"].as_array().unwrap().iter().map(|entry| entry["path"].as_str().unwrap().to_string()).collect()
        }

        fn copy(date: &str) -> String {
            format!(r#"{{"id": 1, "fetched_at": "{date}"}}"#)
        }

        fn end(copied: usize, deleted: &[&str]) -> RefreshEnd {
            RefreshEnd { copied, deleted: deleted.iter().map(|path| path.to_string()).collect() }
        }

        #[test]
        fn the_refresh_copies_only_changed_files() {
            let (_scratch, job, target) =
                folders("refresh-changed", &[("state.json", "a"), ("job.log", "c"), ("t00-00800/decide.json", "b")]);
            assert_eq!(refresh_recordings(&job, &target).unwrap(), end(3, &[]));
            assert_eq!(files(&target), [".lazymkv-refresh.json", "job.log", "state.json", "t00-00800/decide.json"]);
            assert_eq!(refresh_recordings(&job, &target).unwrap(), end(0, &[]));
            // A copy whose source did not change is not written again.
            fs::write(target.join("state.json"), "kept").unwrap();
            fs::write(job.join("t00-00800/decide.json"), "B").unwrap(); // the same size, a later time
            touch(&job.join("t00-00800/decide.json"), T0 + 60);
            fs::write(job.join("job.log"), "cc").unwrap(); // another size, the same time
            touch(&job.join("job.log"), T0);
            assert_eq!(refresh_recordings(&job, &target).unwrap(), end(2, &[]));
            assert_eq!(fs::read(target.join("state.json")).unwrap(), b"kept");
            assert_eq!(fs::read(target.join("t00-00800/decide.json")).unwrap(), b"B");
            assert_eq!(fs::read(target.join("job.log")).unwrap(), b"cc");
            // A copy gone from the target is written again.
            fs::remove_file(target.join("state.json")).unwrap();
            assert_eq!(refresh_recordings(&job, &target).unwrap(), end(1, &[]));
            assert_eq!(fs::read(target.join("state.json")).unwrap(), b"a");
        }

        #[test]
        fn the_refresh_copies_state_json_so_dev_reads_the_folder_as_a_job_folder() {
            let stale = copy("2026-01-01");
            let (scratch, job, target) =
                folders("refresh-job-shaped", &[("state.json", "{}"), ("tmdb.json", stale.as_str())]);
            refresh_recordings(&job, &target).unwrap();
            assert!(target.join(JOB_STATE_FILE).is_file());
            let ctx = Ctx::for_test(&scratch.0);
            let shown = format!("recordings/{ID}");
            assert!(matches!(refresh_copies(&ctx, &target, &shown), Ok(names) if names.is_empty()));
            assert_eq!(fs::read(target.join("tmdb.json")).unwrap(), stale.as_bytes());
            // Without state.json the same folder is a recording whose stale copy dev's gate would fetch, and with no
            // token it refuses (G3: the tuning tree stays a mirror only because state.json is there).
            fs::remove_file(target.join(JOB_STATE_FILE)).unwrap();
            assert!(refresh_copies(&ctx, &target, &shown).is_err());
        }

        #[test]
        fn the_refresh_never_writes_the_owners_label_files_or_notes_md() {
            let (scratch, job, target) = folders("refresh-owner", &[
                ("state.json", "a"), ("audio-labels.json", "engine"), ("subtitle-labels.json", "engine"),
                ("notes.md", "engine"), ("t00-00800/subtitle-labels.json", "engine"),
            ]);
            let audio = scratch.write_path(&format!("tuning/recordings/{ID}/audio-labels.json"), "owner");
            let notes = scratch.write_path(&format!("tuning/recordings/{ID}/notes.md"), "owner notes");
            assert_eq!(refresh_recordings(&job, &target).unwrap(), end(1, &[]));
            assert_eq!(fs::read(&audio).unwrap(), b"owner");
            assert_eq!(fs::read(&notes).unwrap(), b"owner notes");
            assert_eq!(files(&target), [".lazymkv-refresh.json", "audio-labels.json", "notes.md", "state.json"]);
            assert_eq!(listed(&target), ["state.json"]);
            assert_eq!(OWNER_FILES, ["audio-labels.json", "subtitle-labels.json", "notes.md"]);
        }

        #[test]
        fn the_refresh_deletes_only_copies_it_wrote() {
            let (old, fresh) = (copy("2026-04-01"), copy("2026-10-01"));
            let (scratch, job, target) = folders("refresh-delete", &[
                ("state.json", "a"), ("tmdb.json", old.as_str()), ("tmdb-record.json", fresh.as_str()),
                ("t00-00800/decide.json", "b"),
            ]);
            refresh_recordings(&job, &target).unwrap();
            // An orphan's copy that a relayed retune fetched into the recordings folder is never listed (T5-R60).
            let orphan = scratch.write_path(&format!("tuning/recordings/{ID}/tmdb-search.json"), copy("2026-10-02"));
            fs::remove_file(job.join("tmdb.json")).unwrap(); // the cleanup's deletion
            fs::remove_file(job.join("t00-00800/decide.json")).unwrap(); // not a TMDb copy: its copy stays
            assert_eq!(refresh_recordings(&job, &target).unwrap(), end(0, &["tmdb.json"]));
            assert!(!target.join("tmdb.json").exists());
            assert_eq!(fs::read(&orphan).unwrap(), copy("2026-10-02").as_bytes());
            assert_eq!(fs::read(target.join("tmdb-record.json")).unwrap(), fresh.as_bytes());
            assert_eq!(fs::read(target.join("t00-00800/decide.json")).unwrap(), b"b");
            assert_eq!(listed(&target), ["state.json", "t00-00800/decide.json", "tmdb-record.json"]);
            assert_eq!(refresh_recordings(&job, &target).unwrap(), end(0, &[]), "a deletion is mirrored once");
        }

        #[test]
        fn the_manifest_names_each_copy_with_its_size_and_modified_time() {
            let (_scratch, job, target) = folders("refresh-manifest", &[
                ("state.json", "abc"), ("state.json.lazymkv-tmp", "torn"), ("t00-00800/rip.txt", "rip"),
                ("t00-00800/rip.txt.partial", "torn"),
            ]);
            refresh_recordings(&job, &target).unwrap();
            let expected = [
                "{",
                r#"  "format": "lazymkv refresh v1","#,
                r#"  "files": ["#,
                "    {",
                r#"      "path": "state.json","#,
                r#"      "bytes": 3,"#,
                r#"      "modified_ns": 1792800000000000000"#,
                "    },",
                "    {",
                r#"      "path": "t00-00800/rip.txt","#,
                r#"      "bytes": 3,"#,
                r#"      "modified_ns": 1792800000000000000"#,
                "    }",
                "  ]",
                "}",
                "",
            ]
            .join("\n");
            assert_eq!(fs::read_to_string(target.join(REFRESH_MANIFEST)).unwrap(), expected);
            assert_eq!(files(&target), [".lazymkv-refresh.json", "state.json", "t00-00800/rip.txt"]);
            assert_eq!((REFRESH_MANIFEST, REFRESH_FORMAT, RECORDINGS), (".lazymkv-refresh.json", "lazymkv refresh v1", "recordings"));
        }

        #[test]
        fn an_unreadable_manifest_is_read_as_empty_and_deletes_nothing() {
            let (scratch, job, target) = folders("refresh-unreadable", &[("state.json", "a")]);
            let kept = scratch.write_path(&format!("tuning/recordings/{ID}/tmdb.json"), "{}");
            let other = r#"{"format": "lazymkv refresh v2", "files": [{"path": "tmdb.json", "bytes": 2, "modified_ns": 0}]}"#;
            for text in ["not json", other] {
                scratch.write(&format!("tuning/recordings/{ID}/{REFRESH_MANIFEST}"), text);
                assert_eq!(refresh_recordings(&job, &target).unwrap(), end(1, &[]), "{text}");
                assert_eq!(fs::read(&kept).unwrap(), b"{}");
                assert_eq!(listed(&target), ["state.json"]);
            }
        }

        #[test]
        fn a_failed_refresh_keeps_what_landed_and_the_next_one_copies_the_rest() {
            let (scratch, job, target) =
                folders("refresh-torn", &[("state.json", "a"), ("job.log", "c"), ("t00-00800/decide.json", "b")]);
            let blocker = scratch.write_path(&format!("tuning/recordings/{ID}/t00-00800"), "a file where a folder goes");
            assert!(refresh_recordings(&job, &target).is_err());
            assert_eq!(listed(&target), ["job.log", "state.json"]);
            fs::remove_file(&blocker).unwrap();
            assert_eq!(refresh_recordings(&job, &target).unwrap(), end(1, &[]));
            assert_eq!(fs::read(target.join("t00-00800/decide.json")).unwrap(), b"b");
        }
    }
}
