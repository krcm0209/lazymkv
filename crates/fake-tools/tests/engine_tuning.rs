//! The tuning dataset copy on stand-ins (design part 7.5, EVID-4; spec 7 "Tuning evidence"; call 10, C2; T5-R13,
//! T5-R14, T5-R46, T5-R55; Review Focus 1).

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use lazymkv_core::job::{Event, State, Tool};
use lazymkv_core::record::TuningCopy;
use lazymkv_core::review::Note;
use lazymkv_engine::testkit::Harness;
use lazymkv_platform::Reach;
use lazymkv_tools::exe_name;
use serde_json::{json, Value};

const KEY: &str = "t00-00800";
const COPIED: &str = "tuning evidence copied to tuning_root";
const DAY_MS: u64 = 86_400_000;

fn harness(variants: &[&str], settings: Value) -> Harness {
    let mut scenarios = vec!["movie-direct"];
    scenarios.extend_from_slice(variants);
    Harness::new(Path::new(env!("CARGO_BIN_EXE_fake-tool")), &scenarios, settings)
}

fn on() -> Value {
    json!({"keep_tuning_evidence": true})
}

/// Inserts movie-direct in tok0 and returns the job's id.
fn insert(h: &Harness) -> String {
    h.insert("tok0", "movie-direct");
    format!("{}-1", h.fingerprint("movie-direct").to_ascii_lowercase())
}

/// Inserts movie-direct and waits for a persisted transition into `state`.
fn run_to(h: &Harness, state: &str) -> String {
    let id = insert(h);
    h.wait(state, |h| h.transitions(&id).iter().any(|line| line.new.as_str() == state));
    id
}

/// Waits until the job's state.json holds `copy` as its `tuning_copy`.
fn wait_copy_state(h: &Harness, id: &str, copy: TuningCopy) {
    h.wait("the copy's state", |h| h.job_ids().iter().any(|known| known == id) && h.record(id).tuning_copy == Some(copy));
}

/// Waits for the first copy attempt's line in job.log, then settles.
fn wait_first_attempt(h: &Harness, id: &str) {
    h.wait("a copy attempt's line", |h| !tuning_lines(h, id).is_empty());
    h.settle();
}

fn root(h: &Harness) -> String {
    h.tuning().display().to_string()
}

/// job.log's lines about the dataset copy, in order.
fn tuning_lines(h: &Harness, id: &str) -> Vec<String> {
    let log = fs::read_to_string(h.job_dir(id).join("job.log")).unwrap_or_default();
    log.lines().filter(|line| line.starts_with("tuning ")).map(str::to_string).collect()
}

fn staged(h: &Harness, id: &str) -> PathBuf {
    h.staging().join(id).join("tuning").join(KEY)
}

fn dataset(h: &Harness, id: &str) -> PathBuf {
    h.tuning().join("dataset").join(id).join(KEY)
}

/// The name of the one level folder `audio-levels-20ms-*` in `dir`.
fn level(dir: &Path) -> String {
    let names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("audio-levels-20ms-"))
        .collect();
    assert_eq!(names.len(), 1, "{names:?}");
    names[0].clone()
}

/// Every file under `dir`, relative to it with '/' separators; empty when `dir` does not exist.
fn files_under(dir: &Path) -> BTreeSet<String> {
    fn walk(root: &Path, dir: &Path, found: &mut BTreeSet<String>) {
        let Ok(entries) = fs::read_dir(dir) else { return };
        for entry in entries {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, found);
            } else {
                found.insert(path.strip_prefix(root).unwrap().to_str().unwrap().replace('\\', "/"));
            }
        }
    }
    let mut found = BTreeSet::new();
    walk(dir, dir, &mut found);
    found
}

fn pending_notes(h: &Harness, id: &str) -> usize {
    h.record(id).notes.iter().filter(|noted| noted.note == Note::TuningEvidencePending).count()
}

/// The job's DecisionDone events, which name no scratch path, so two harnesses compare.
fn decisions(h: &Harness, id: &str) -> Vec<Event> {
    let found = h.transitions(id).into_iter().filter_map(|line| line.event);
    found.filter(|event| matches!(event, Event::DecisionDone { .. })).collect()
}

/// How many spawns ran the program `name` (the stand-in's file name).
fn runs_of(h: &Harness, name: &str) -> usize {
    h.spawns().iter().filter(|spawn| spawn.program.file_name() == Some(OsStr::new(name))).count()
}

fn ocr_worker() -> String {
    format!("lazymkv-ocr{}", std::env::consts::EXE_SUFFIX)
}

/// A job that went Filed with `tuning_root` unreachable, its copy pending, and the share answering again.
fn pending_then_reachable(h: &Harness) -> String {
    h.shares().set_reach(&root(h), Reach::Unreachable("cable out".into()));
    let id = run_to(h, "Filed");
    wait_first_attempt(h, &id);
    assert_eq!(h.record(&id).tuning_copy, Some(TuningCopy::Pending));
    h.shares().set_reach(&root(h), Reach::Reachable);
    id
}

/// T5-R46, R6-phase0-44: the dataset gets the staged folder whole and the evidence file from the staging evidence
/// folder, and the job folder's level folder gets its audio-proxy.json only when the copy lands (T5-R13).
#[test]
fn the_dataset_gets_evidence_mks_from_the_staging_evidence_folder() {
    let h = harness(&["movie-direct-two-subs"], on());
    let id = insert(&h);
    wait_copy_state(&h, &id, TuningCopy::Done);
    h.settle();
    let (staged, dataset) = (staged(&h, &id), dataset(&h, &id));
    let mut expected = files_under(&staged);
    expected.insert("subs-ocr/evidence.mks".to_string());
    assert_eq!(files_under(&dataset), expected);
    for file in files_under(&staged) {
        assert_eq!(fs::read(dataset.join(&file)).unwrap(), fs::read(staged.join(&file)).unwrap(), "{file}");
    }
    assert!(expected.contains("subs-ocr/sub-0.ocr.jsonl") && expected.contains("subs-ocr/sub-1.run.json"));
    let evidence = h.staging().join(&id).join("evidence").join(format!("{KEY}.mks"));
    assert_eq!(fs::read(dataset.join("subs-ocr").join("evidence.mks")).unwrap(), fs::read(&evidence).unwrap());
    assert!(!staged.join("subs-ocr").join("evidence.mks").exists());
    let level = level(&staged);
    let marker = h.job_dir(&id).join(KEY).join(&level).join("audio-proxy.json");
    assert_eq!(fs::read(&marker).unwrap(), fs::read(staged.join(&level).join("audio-proxy.json")).unwrap());
    assert_eq!(tuning_lines(&h, &id), [COPIED]);
    assert_eq!(pending_notes(&h, &id), 0, "a copy that lands at once adds no note (T5-R14)");
}

#[test]
fn an_unreachable_tuning_root_leaves_the_copy_pending_with_the_note_and_never_fails_the_decision() {
    let h = harness(&[], on());
    h.shares().set_reach(&root(&h), Reach::Unreachable("cable out".into()));
    let id = run_to(&h, "Filed");
    wait_first_attempt(&h, &id);
    let record = h.record(&id);
    assert_eq!(record.state, State::Filed);
    assert_eq!(record.tuning_copy, Some(TuningCopy::Pending));
    assert_eq!(pending_notes(&h, &id), 1);
    let found: Vec<Event> = h.transitions(&id).into_iter().filter_map(|line| line.event).collect();
    assert!(!found.iter().any(|event| matches!(event, Event::DecisionFailed { .. })), "{found:?}");
    assert_eq!(decisions(&h, &id).len(), 1);
    let lines = tuning_lines(&h, &id);
    assert_eq!(lines[0], format!("tuning copy pending: tuning_root {} did not answer: cable out", root(&h)));
    assert!(lines[1..].iter().all(|line| line.starts_with("tuning copy retried: still pending: ")), "{lines:?}");
    assert!(!h.tuning().join("dataset").exists());
    let level = level(&staged(&h, &id));
    assert!(!h.job_dir(&id).join(KEY).join(level).join("audio-proxy.json").exists());
}

/// R1-evidence-27: a retry at engine start only copies.
#[test]
fn a_pending_copy_is_retried_at_engine_start_and_only_copies() {
    let h = harness(&[], on());
    let id = pending_then_reachable(&h);
    let (decodes, reads) = (runs_of(&h, &exe_name(Tool::Ffmpeg)), runs_of(&h, &ocr_worker()));
    h.restart();
    assert_eq!(h.wait_copy(), id);
    h.settle();
    assert_eq!((runs_of(&h, &exe_name(Tool::Ffmpeg)), runs_of(&h, &ocr_worker())), (decodes, reads));
    assert_eq!(h.record(&id).tuning_copy, Some(TuningCopy::Done));
    assert_eq!(tuning_lines(&h, &id).last().map(String::as_str), Some(COPIED));
    assert!(dataset(&h, &id).join("audio-proxy.mka").is_file());
}

/// Design part 7.5: a pending copy is retried whenever the share answers before another step, any job's step.
#[test]
fn a_pending_copy_is_retried_when_tuning_root_answers_before_another_step() {
    let h = harness(&[], on());
    let id = pending_then_reachable(&h);
    h.insert("tok1", "second-disc");
    assert_eq!(h.wait_copy(), id);
    h.settle();
    assert_eq!(h.record(&id).tuning_copy, Some(TuningCopy::Done));
    assert_eq!(tuning_lines(&h, &id).last().map(String::as_str), Some(COPIED));
}

/// T5-R14: the note stays after the copy lands; only `tuning_copy` says done.
#[test]
fn a_landed_copy_marks_the_state_done_and_keeps_the_note() {
    let h = harness(&[], on());
    let id = pending_then_reachable(&h);
    assert_eq!(h.run_copies(), [id.clone()]);
    let record = h.record(&id);
    assert_eq!(record.tuning_copy, Some(TuningCopy::Done));
    assert_eq!(pending_notes(&h, &id), 1);
    assert_eq!(record.state, State::Filed);
    let lines = tuning_lines(&h, &id);
    assert!(lines[0].starts_with("tuning copy pending: "), "{lines:?}");
    assert_eq!(lines.last().map(String::as_str), Some(COPIED));
    let proxy = |dir: PathBuf| fs::read(dir.join("audio-proxy.mka")).unwrap();
    assert_eq!(proxy(dataset(&h, &id)), proxy(staged(&h, &id)));
}

/// R1-evidence-23, R3-worker-39.
#[test]
fn with_the_setting_off_nothing_goes_to_tuning_root() {
    let h = harness(&[], json!({}));
    let id = run_to(&h, "Filed");
    h.settle();
    assert!(fs::read_dir(h.tuning()).unwrap().next().is_none(), "the tuning root stays empty");
    assert_eq!(h.record(&id).tuning_copy, None);
    assert!(tuning_lines(&h, &id).is_empty());
    assert_eq!(pending_notes(&h, &id), 0);
}

/// Call 22, R5-code-56: a title whose proxy clipped at both gains has no proxy to copy, and its copy is done.
#[test]
fn a_title_that_kept_no_proxy_counts_its_copy_as_done() {
    let h = harness(&["movie-direct-clip", "movie-direct-clip-twice"], on());
    let id = insert(&h);
    wait_copy_state(&h, &id, TuningCopy::Done);
    h.settle();
    let dataset = dataset(&h, &id);
    assert!(!dataset.join("audio-proxy.mka").exists());
    assert!(dataset.join("audio-proxy.json").is_file());
    assert!(dataset.join(level(&staged(&h, &id))).join("audio-proxy.json").is_file());
    assert_eq!(tuning_lines(&h, &id), [COPIED]);
}

/// Review Focus 1: a `.partial` a lost connection left of another size is replaced at the next reach.
#[test]
fn a_partial_left_by_a_lost_share_is_replaced_and_the_copy_lands_at_the_next_reach() {
    let h = harness(&[], on());
    let id = pending_then_reachable(&h);
    let dataset = dataset(&h, &id);
    fs::create_dir_all(&dataset).unwrap();
    fs::write(dataset.join("audio-proxy.mka.partial"), b"torn").unwrap();
    assert_eq!(h.run_copies(), [id.clone()]);
    assert_eq!(fs::read(dataset.join("audio-proxy.mka")).unwrap(), fs::read(staged(&h, &id).join("audio-proxy.mka")).unwrap());
    assert!(files_under(h.tuning()).iter().all(|file| !file.ends_with(".partial")), "{:?}", files_under(h.tuning()));
    assert_eq!(h.record(&id).tuning_copy, Some(TuningCopy::Done));
    let lines = tuning_lines(&h, &id);
    assert_eq!(lines.iter().filter(|line| *line == COPIED).count(), 1, "{lines:?}");
    assert_eq!(lines.last().map(String::as_str), Some(COPIED));
    assert!(lines[..lines.len() - 1].iter().all(|line| line.starts_with("tuning copy ")), "{lines:?}");
}

/// Review Focus 1: a file whose size differs after the write is not renamed, the copy stays pending without the note
/// (it is neither spec 7 case), and the pending copy holds the staging data past retention (C2).
#[test]
fn a_copy_whose_size_differs_after_the_write_stays_pending_and_keeps_the_staging_data() {
    let h = harness(&[], on());
    h.copy_with(|from: &Path, to: &Path| -> io::Result<u64> {
        if to.file_name() == Some(OsStr::new("audio-proxy.mka.partial")) {
            let bytes = fs::read(from)?;
            fs::write(to, &bytes[..bytes.len() / 2])?;
            return Ok(bytes.len() as u64);
        }
        fs::copy(from, to)
    });
    let id = run_to(&h, "Filed");
    wait_first_attempt(&h, &id);
    assert_eq!(h.record(&id).tuning_copy, Some(TuningCopy::Pending));
    assert_eq!(pending_notes(&h, &id), 0);
    let (dataset, level) = (dataset(&h, &id), level(&staged(&h, &id)));
    assert!(!dataset.join("audio-proxy.mka").exists() && !dataset.join("audio-proxy.mka.partial").exists());
    assert!(!dataset.join(&level).join("audio-proxy.json").exists());
    assert!(!h.job_dir(&id).join(KEY).join(&level).join("audio-proxy.json").exists());
    let first = &tuning_lines(&h, &id)[0];
    assert!(first.starts_with("tuning copy pending: ") && first.contains(" bytes after the copy, not the "), "{first}");
    h.host().set_now_ms(1_792_800_000 * 1000 + 8 * DAY_MS);
    h.run_cleanup();
    assert!(staged(&h, &id).join("audio-proxy.mka").is_file(), "a pending copy holds the staging data");
}

/// Review Focus 1: a full tuning root is spec 7's "short of space": the note, a pending copy, the same decision.
#[test]
fn a_full_tuning_root_leaves_the_copy_pending_with_the_note_and_the_decision_unchanged() {
    let reference = harness(&[], on());
    let reference_id = run_to(&reference, "Filed");
    wait_copy_state(&reference, &reference_id, TuningCopy::Done);
    let h = harness(&[], on());
    h.shares().set_free(&root(&h), 10);
    let id = run_to(&h, "Filed");
    wait_first_attempt(&h, &id);
    assert_eq!(h.record(&id).tuning_copy, Some(TuningCopy::Pending));
    assert_eq!(pending_notes(&h, &id), 1);
    let first = &tuning_lines(&h, &id)[0];
    let head = format!("tuning copy pending: tuning_root {} has 10 bytes free, and the copy needs ", root(&h));
    assert!(first.starts_with(&head) && first.ends_with(" bytes"), "{first}");
    assert_eq!(decisions(&h, &id), decisions(&reference, &reference_id));
    assert!(!h.tuning().join("dataset").exists());
}

/// C2: a pending copy whose job has no staged tuning folder left owes nothing, as a job held under pull request 4's
/// build and approved under this one has none: the attempt ends the copy, and the cleanup deletes the staging data
/// once the job's retention has passed.
#[test]
fn a_pending_copy_with_no_staged_tuning_folder_owes_nothing_and_the_cleanup_deletes_the_staging_data() {
    let h = harness(&[], on());
    let id = pending_then_reachable(&h);
    fs::remove_dir_all(h.staging().join(&id).join("tuning")).unwrap();
    assert_eq!(h.run_copies(), [id.clone()]);
    assert_eq!(h.record(&id).tuning_copy, None);
    let last = tuning_lines(&h, &id).last().cloned();
    assert_eq!(last.as_deref(), Some("tuning copy: no staged tuning folder, so nothing is owed (C2)"));
    assert!(!h.tuning().join("dataset").exists());
    h.host().set_now_ms(1_792_800_000 * 1000 + 8 * DAY_MS);
    h.run_cleanup();
    assert!(!h.staging().join(&id).exists(), "nothing pending holds the staging data (C2)");
}

/// Call 10: on an idle engine a pending copy is retried at the cleanup pass once `tuning_root` answers again.
#[test]
fn a_pending_copy_is_retried_at_the_cleanup_pass_of_an_idle_engine() {
    let h = harness(&[], on());
    let id = pending_then_reachable(&h);
    h.run_cleanup();
    h.settle();
    assert_eq!(h.record(&id).tuning_copy, Some(TuningCopy::Done));
    assert_eq!(tuning_lines(&h, &id).last().map(String::as_str), Some(COPIED));
    assert!(staged(&h, &id).join("audio-proxy.mka").is_file(), "the job's retention has not passed, so its staging data stays");
}
