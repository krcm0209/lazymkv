//! Restarts over the stand-in tools (design part 5.10, L1, L1a; Ruling S12).

use std::fs;
use std::path::{Path, PathBuf};

use lazymkv_core::job::{
    job_lookup, Action, Event, Failure, FailureClass, Held, JobLookup, NotifyEvent, OutputCause, State, WaitReason,
};
use lazymkv_core::record::{JobRecord, Transition};
use lazymkv_core::review::ReviewReason;
use lazymkv_engine::testkit::{ci_factor, Harness};
use lazymkv_platform::Reach;
use serde_json::json;

/// The disc of the base scenario, as Task 22's Harness::insert names it.
const DISC: &str = "movie-direct";
const FINAL: &str = "Synthetic Movie (2001) {tmdb-900001}/Synthetic Movie (2001) {tmdb-900001} - 1080p.mkv";

fn settings() -> serde_json::Value {
    json!({"keep_tuning_evidence": false, "drive_settle_s": 0, "drive_ready_timeout_s": 1})
}

fn harness(scenarios: &[&str], settings: serde_json::Value) -> Harness {
    Harness::new(Path::new(env!("CARGO_BIN_EXE_fake-tool")), scenarios, settings)
}

fn job_ids(h: &Harness) -> Vec<String> {
    let mut ids: Vec<String> = fs::read_dir(h.data().join("jobs")).map_or(vec![], |dir| {
        dir.map(|e| e.unwrap().file_name().into_string().unwrap()).collect()
    });
    ids.sort();
    ids
}

fn only_job(h: &Harness) -> String {
    let ids = job_ids(h);
    assert_eq!(ids.len(), 1, "{ids:?}");
    ids[0].clone()
}

fn transitions(dir: &Path) -> Vec<Transition> {
    fs::read_to_string(dir.join("transitions.jsonl")).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
}

fn state_json(dir: &Path) -> JobRecord {
    serde_json::from_slice(&fs::read(dir.join("state.json")).unwrap()).unwrap()
}

fn final_path(h: &Harness) -> PathBuf { h.library().join(FINAL) }

fn root(path: &Path) -> String {
    path.to_str().unwrap().to_string()
}

/// Waits until the job's state.json is in Failed or Filed, the two ends File can reach.
fn wait_for_file_end(h: &Harness, id: &str) -> JobRecord {
    h.wait("File's end", |h| matches!(h.state_of(id).as_str(), "Failed" | "Filed"));
    state_json(&h.job_dir(id))
}

#[test]
fn a_restart_in_each_state_of_the_path_still_reaches_filed() {
    for state in ["Detected", "Fingerprinted", "Scanned", "Planned", "Ripping", "RipVerified", "Ejected", "Inspecting",
                  "Decided", "Remuxing", "Verified"] {
        let h = harness(&["movie-direct"], settings());
        h.insert("tok0", DISC);
        h.run_until(state);
        h.restart();
        h.run_until("Filed");
        let dir = h.job_dir(&only_job(&h));
        let lines = transitions(&dir);
        assert!(lines.iter().any(|t| matches!(t.event, Some(Event::EngineRestart { .. }))), "{state}");
        assert_eq!(lines.last().unwrap().new, State::Filed, "{state}");
        assert_eq!(fs::read(final_path(&h)).unwrap().len(), 16, "{state}");
        assert!(fs::read_dir(h.library().join(".lazymkv-partial")).unwrap().next().is_none(), "{state}");
    }
}

#[test]
fn a_restart_after_a_lost_last_transition_line_resumes_from_state_json_and_appends_after_it() {
    let h = harness(&["movie-direct"], settings());
    h.insert("tok0", DISC);
    h.run_until("Ripping");
    let dir = h.job_dir(&only_job(&h));
    let text = fs::read_to_string(dir.join("transitions.jsonl")).unwrap();
    let kept: Vec<&str> = text.lines().collect::<Vec<_>>()[..text.lines().count() - 1].to_vec();
    let kept_text: String = kept.iter().map(|l| format!("{l}\n")).collect();
    fs::write(dir.join("transitions.jsonl"), &kept_text).unwrap();
    assert_eq!(state_json(&dir).state, State::Ripping);

    h.restart();
    h.run_until("Filed");
    let after = fs::read_to_string(dir.join("transitions.jsonl")).unwrap();
    assert!(after.starts_with(&kept_text));
    let lines = transitions(&dir);
    let resumed = &lines[kept.len()];
    assert_eq!(resumed.event, Some(Event::EngineRestart { disc_present: true }));
    assert_eq!(resumed.old, Some(State::Ripping));
    assert_eq!(resumed.new, State::Ripping);
    assert_eq!(resumed.actions, vec![Action::EmptyTitleDirectory, Action::RipTitle]);
    assert_eq!(lines.last().unwrap().new, State::Filed);
}

/// RP-8: the next append first restores a torn last line's newline, so every line still parses.
#[test]
fn a_torn_last_line_gets_its_newline_before_the_next_append() {
    let h = harness(&["movie-direct"], settings());
    h.insert("tok0", DISC);
    h.run_until("Ripping");
    let dir = h.job_dir(&only_job(&h));
    let text = fs::read_to_string(dir.join("transitions.jsonl")).unwrap();
    fs::write(dir.join("transitions.jsonl"), text.strip_suffix('\n').unwrap()).unwrap();

    h.restart();
    h.run_until("Filed");
    assert!(fs::read_to_string(dir.join("transitions.jsonl")).unwrap().starts_with(&text));
    // `transitions` parses every line, so a joined line fails here.
    assert_eq!(transitions(&dir).last().unwrap().new, State::Filed);
}

#[test]
fn a_held_job_stays_held_after_a_restart() {
    let h = harness(&["movie-direct"], settings());
    fs::create_dir_all(final_path(&h).parent().unwrap()).unwrap();
    fs::write(final_path(&h), b"owner's").unwrap();
    h.insert("tok0", DISC);
    h.run_until("NeedsReview");
    let dir = h.job_dir(&only_job(&h));
    let before = fs::read(dir.join("transitions.jsonl")).unwrap();
    h.restart();
    assert_eq!(fs::read(dir.join("transitions.jsonl")).unwrap(), before);
    assert_eq!(state_json(&dir).state, State::NeedsReview { held: Held::DispositionGate });
    assert_eq!(fs::read(final_path(&h)).unwrap(), b"owner's");
}

#[test]
fn a_disc_left_in_after_a_copy_mode_failure_starts_nothing_at_a_restart() {
    let h = harness(&["movie-direct", "movie-direct-copy-plan"], settings());
    h.insert("tok0", DISC);
    h.run_until("Failed");
    let id = only_job(&h);
    let before = fs::read(h.job_dir(&id).join("transitions.jsonl")).unwrap();
    let ejects = h.ejector().calls().len();
    h.restart();
    h.settle();
    assert_eq!(job_ids(&h), vec![id.clone()]);
    assert_eq!(fs::read(h.job_dir(&id).join("transitions.jsonl")).unwrap(), before);
    assert_eq!(h.ejector().calls().len(), ejects);
    let drives: serde_json::Value = serde_json::from_slice(&fs::read(h.data().join("drives.json")).unwrap()).unwrap();
    assert_eq!(drives["drives"]["tok0"]["handled"], json!(true));
}

#[test]
fn a_disc_left_in_after_a_failed_eject_starts_nothing_at_a_restart() {
    let h = harness(&["movie-direct"], settings());
    h.ejector().fail_next("stuck");
    h.insert("tok0", DISC);
    h.run_until("Filed");
    let id = only_job(&h);
    assert!(transitions(&h.job_dir(&id)).iter().any(|t| t.event == Some(Event::EjectFailed)));
    let before = fs::read(h.job_dir(&id).join("transitions.jsonl")).unwrap();
    let ejects = h.ejector().calls().len();
    h.restart();
    h.settle();
    assert_eq!(job_ids(&h), vec![id.clone()]);
    assert_eq!(fs::read(h.job_dir(&id).join("transitions.jsonl")).unwrap(), before);
    assert_eq!(h.ejector().calls().len(), ejects);
    let drives: serde_json::Value = serde_json::from_slice(&fs::read(h.data().join("drives.json")).unwrap()).unwrap();
    assert_eq!(drives["drives"]["tok0"]["handled"], json!(true));
}

#[test]
fn a_restart_after_a_late_answer_decides_with_the_refreshed_identity() {
    let h = harness(&["movie-direct", "movie-direct-lookup-late"], settings());
    h.scan_waits_for_lookups(false);
    h.insert("tok0", DISC);
    h.run_until("Inspecting");
    let dir = h.job_dir(&only_job(&h));
    assert!(fs::read_to_string(dir.join("lookup-status.txt")).unwrap().contains("dischash 404"));
    assert!(state_json(&dir).plan.unwrap().identity.lookup_pending, "the refresh before Decide has not run yet");
    h.restart();
    h.run_until("Filed");
    let record = state_json(&dir);
    assert!(!record.plan.unwrap().identity.lookup_pending);
    assert_eq!(Path::new(&record.titles[0].filed.as_ref().unwrap().path), final_path(&h).as_path());
}

#[test]
fn a_rebuilt_remux_promise_counts_against_another_jobs_rip_check() {
    // One volume for staging and the library. A's rip check needs 4301 + 4301 (S54) and passes; A's Remux step,
    // resumed at the restart, replaces the rebuilt estimate with write_bytes_needed(16) = 17 (`refine_remux_promise`,
    // engine.md:585); B's identical check needs more than 8610 with either figure, 8619 with 17.
    let h = harness(&["movie-direct", "movie-direct-remux-hang"], settings());
    let (staging, movies) = (h.staging().to_str().unwrap().to_string(), h.library().to_str().unwrap().to_string());
    h.shares().same_volume(&staging, &movies);
    h.shares().set_free(&staging, 8610);
    h.shares().set_free(&movies, 8610);
    h.insert("tok0", DISC);
    h.run_until("Remuxing");
    let a = only_job(&h);
    h.restart();
    h.scan_waits_for_lookups(false);
    h.insert("tok0", "disc-b");
    h.wait("B waits for space", |h| h.job_ids().iter().any(|id| *id != a && h.state_of(id) == "AwaitingSpace"));
    assert_eq!(h.state_of(&a), "Remuxing");
}

/// engine.md:590 (V7): a restart into Remuxing runs DeletePartialFile and Remux, and each asks `reach` of the title's
/// library root before it touches the share, so with the share not answering nothing there is deleted or written and
/// Remux fails naming the share and the way out.
#[test]
fn a_restart_into_remuxing_with_the_library_share_unreachable_fails_remux_without_writing() {
    let h = harness(&["movie-direct"], settings());
    h.insert("tok0", DISC);
    h.run_until("Remuxing");
    let id = only_job(&h);
    let library = root(h.library());
    let partial = h.library().join(".lazymkv-partial").join(format!("{id}-t00-00800.mkv.partial"));
    fs::create_dir_all(partial.parent().unwrap()).unwrap();
    fs::write(&partial, b"sentinel").unwrap();
    h.shares().set_reach(&library, Reach::Unreachable("cable out".into()));
    h.restart();
    h.wait("Failed", |h| h.state_of(&id) == "Failed");
    let record = state_json(&h.job_dir(&id));
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Remuxing) });
    let why = format!(
        "t00-00800: the library share holding {library} does not answer (cable out); the remux was not started, and a \
         retry once the share answers runs it"
    );
    assert_eq!(record.message.as_deref(), Some(why.as_str()));
    assert_eq!(fs::read(&partial).unwrap(), b"sentinel");
    let log = fs::read_to_string(h.job_dir(&id).join("job.log")).unwrap();
    let kept = format!(
        "t00-00800: the library share holding {library} does not answer (cable out); the partial file {} was not deleted",
        partial.display()
    );
    assert!(log.contains(&kept), "{log}");
}

#[test]
fn a_disc_already_in_a_drive_is_an_arrival_at_start() {
    let h = harness(&["movie-direct"], json!({"keep_tuning_evidence": false, "drive_settle_s": 0,
                                                   "drive_ready_timeout_s": 1, "file_phase_concurrency": 0}));
    h.insert("tok0", DISC);
    h.restart(); // the settings problem keeps the arrival waiting; no job folder exists
    h.settle();
    assert!(job_ids(&h).is_empty());
    let path = h.data().join("settings.json");
    let mut stored: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    stored["file_phase_concurrency"] = json!(1);
    fs::write(&path, serde_json::to_vec(&stored).unwrap()).unwrap();
    h.restart();
    h.run_until("Filed");
    assert_eq!(job_ids(&h).len(), 1);
}

#[test]
fn a_restart_in_verified_with_the_file_already_filed_feeds_file_done() {
    let h = harness(&["movie-direct"], settings());
    h.insert("tok0", DISC);
    h.run_until("Verified");
    let id = only_job(&h);
    let partial = fs::read_dir(h.library().join(".lazymkv-partial")).unwrap().next().unwrap().unwrap().path();
    fs::create_dir_all(final_path(&h).parent().unwrap()).unwrap();
    fs::rename(&partial, final_path(&h)).unwrap();
    h.restart();
    h.run_until("Filed");
    let lines = transitions(&h.job_dir(&id));
    let n = lines.len();
    assert_eq!(lines[n - 2].event, Some(Event::EngineRestart { disc_present: false }));
    assert_eq!(lines[n - 1].event, Some(Event::FileDone));
    let record = state_json(&h.job_dir(&id));
    let filed = record.titles[0].filed.as_ref().unwrap();
    assert_eq!(Path::new(&filed.path), final_path(&h).as_path());
    assert_eq!(filed.size, 16);
    assert!(record.filed_unix.is_some());
}

#[test]
fn a_restart_in_verified_with_another_size_at_the_final_path_fails_as_unknown() {
    let h = harness(&["movie-direct"], settings());
    h.insert("tok0", DISC);
    h.run_until("Verified");
    let id = only_job(&h);
    let partial = fs::read_dir(h.library().join(".lazymkv-partial")).unwrap().next().unwrap().unwrap().path();
    fs::remove_file(partial).unwrap();
    fs::create_dir_all(final_path(&h).parent().unwrap()).unwrap();
    fs::write(final_path(&h), b"abc").unwrap();
    h.restart();
    h.run_until("Failed");
    let last = transitions(&h.job_dir(&id)).pop().unwrap();
    assert!(matches!(last.event, Some(Event::FileFailed { failure }) if failure.class == FailureClass::Unknown));
    assert_eq!(fs::read(final_path(&h)).unwrap(), b"abc");
}

/// Off Windows only (S13): there `rename_no_replace` is a hard link then a remove, and on Windows one `MoveFileExW`,
/// so the half state cannot arise there and a hard link at the final path is a collision (Ruling T3-4-b).
/// Design part 6.5 with `rename_no_replace`'s two steps (hard link, then remove): a stop between them leaves both names
/// on one file, and File after the restart removes the partial name and records the filed file, with no collision.
#[cfg(not(windows))]
#[test]
fn a_restart_after_a_rename_stopped_between_link_and_remove_finishes_it() {
    let h = harness(&["movie-direct"], settings());
    h.insert("tok0", DISC);
    h.run_until("Verified");
    let id = only_job(&h);
    let partial = fs::read_dir(h.library().join(".lazymkv-partial")).unwrap().next().unwrap().unwrap().path();
    fs::create_dir_all(final_path(&h).parent().unwrap()).unwrap();
    fs::hard_link(&partial, final_path(&h)).unwrap();
    h.restart();
    h.run_until("Filed");
    assert!(!partial.exists());
    assert_eq!(fs::read(final_path(&h)).unwrap().len(), 16);
    let record = state_json(&h.job_dir(&id));
    assert!(record.collisions.is_empty(), "{:?}", record.collisions);
    assert_eq!(record.titles[0].filed.as_ref().unwrap().size, 16);
}

/// Review round 1, V5 (spec 7, 10.1, 10.2; RR1-8): a restart in Ripping while the staging share is gone fails the
/// emptying of the title's folder as Output { ShareUnreachable }, so the job waits in AwaitingShare to return to
/// Ripping and a reinserted disc resumes it instead of starting a new job.
#[test]
fn a_restart_in_ripping_with_the_staging_share_gone_waits_in_awaiting_share() {
    let h = harness(&["movie-direct"], settings());
    h.insert("tok0", DISC);
    h.run_until("Ripping");
    let id = only_job(&h);
    fs::write(h.staging().join(&id), b"").unwrap();
    h.shares().set_reach(&root(h.staging()), Reach::Unreachable("gone".into()));
    h.restart();
    // Not `run_until`: it stops before the transition's actions, the waiting notification among them.
    h.wait("AwaitingShare", |h| h.state_of(&id) == "AwaitingShare");
    h.settle();
    let lines = transitions(&h.job_dir(&id));
    let at = lines.iter().position(|t| matches!(t.event, Some(Event::EngineRestart { .. }))).expect("a restart line");
    assert_eq!(lines[at].actions, vec![Action::EmptyTitleDirectory, Action::RipTitle]);
    let unreachable = Failure { class: FailureClass::Output, output: Some(OutputCause::ShareUnreachable) };
    assert_eq!(lines[at + 1].event, Some(Event::TitleRipFailed { failure: unreachable }));
    assert_eq!(lines[at + 1].new, State::AwaitingShare { return_to: Box::new(State::Ripping) });
    let sent: Vec<NotifyEvent> = h.notifications().into_iter().map(|n| n.event).collect();
    assert!(sent.contains(&NotifyEvent::Waiting { reason: WaitReason::Share }), "{sent:?}");
    assert!(!sent.iter().any(|e| matches!(e, NotifyEvent::Failed { .. })), "{sent:?}");
    let state = state_json(&h.job_dir(&id)).state;
    assert_eq!(job_lookup(&[(id.clone(), state, false)]), JobLookup::Resume(id));
}

/// Review round 1, V7 (design part 6.1; RR1-9): File asks `reach` of the library root before it touches any path, so a
/// library share that does not answer at a restart in Verified fails File naming the share, the title and the way out,
/// renames nothing and records no collision. The retry that files it is `engine_ipc.rs`'s
/// `file_with_the_library_share_unreachable_fails_naming_the_share_and_a_retry_files_it`.
#[test]
fn file_asks_the_library_share_before_touching_the_partial() {
    let h = harness(&["movie-direct"], settings());
    h.insert("tok0", DISC);
    h.run_until("Verified");
    let id = only_job(&h);
    let partial = fs::read_dir(h.library().join(".lazymkv-partial")).unwrap().next().unwrap().unwrap().path();
    h.shares().set_reach(&root(h.library()), Reach::Unreachable("cable out".into()));
    h.restart();
    let record = wait_for_file_end(&h, &id);
    assert_eq!(record.state, State::Failed { class: FailureClass::Unknown, from: Box::new(State::Verified) });
    assert!(partial.is_file());
    assert!(!final_path(&h).exists());
    let message = record.message.unwrap_or_default();
    for part in ["cable out", "t00-00800", "a retry once the share answers"] {
        assert!(message.contains(part), "{part}: {message}");
    }
    assert!(!message.contains("is gone"), "{message}");
    assert!(record.collisions.is_empty(), "{:?}", record.collisions);
}

/// Restores a folder's mode when the test ends, so the harness can remove its scratch folder.
#[cfg(unix)]
struct RestoreMode(PathBuf);

#[cfg(unix)]
impl Drop for RestoreMode {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o755));
    }
}

/// Review round 1, V7: a partial file whose metadata cannot be read is not "gone": File fails naming it and the error.
/// Unix only (mode 0o000 on the partial folder); skipped as root, which the mode does not stop.
#[cfg(unix)]
#[test]
fn a_partial_that_cannot_be_read_fails_file_naming_the_error_not_gone() {
    use std::os::unix::fs::PermissionsExt;
    let h = harness(&["movie-direct"], settings());
    h.insert("tok0", DISC);
    h.run_until("Verified");
    let id = only_job(&h);
    let folder = h.library().join(".lazymkv-partial");
    let partial = fs::read_dir(&folder).unwrap().next().unwrap().unwrap().path();
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o000)).unwrap();
    let _restore = RestoreMode(folder.clone());
    if fs::metadata(&partial).is_ok() {
        return; // root reads through mode 0o000
    }
    h.restart();
    let record = wait_for_file_end(&h, &id);
    assert!(matches!(record.state, State::Failed { .. }), "{:?}", record.state);
    let message = record.message.unwrap_or_default();
    assert!(message.contains(&partial.display().to_string()), "{message}");
    assert!(message.contains("was not checked") && !message.contains("is gone"), "{message}");
}

/// Review round 1, V7: a final path whose folder cannot be read does not "hold no file": with the partial file gone,
/// File fails naming the final path and records no collision. Unix only, as above.
#[cfg(unix)]
#[test]
fn an_unreadable_final_folder_fails_file_naming_the_final_path_not_holds_no_file() {
    use std::os::unix::fs::PermissionsExt;
    let h = harness(&["movie-direct"], settings());
    h.insert("tok0", DISC);
    h.run_until("Verified");
    let id = only_job(&h);
    let partial = fs::read_dir(h.library().join(".lazymkv-partial")).unwrap().next().unwrap().unwrap().path();
    fs::remove_file(partial).unwrap();
    let film = final_path(&h).parent().unwrap().to_path_buf();
    fs::create_dir_all(&film).unwrap();
    fs::set_permissions(&film, fs::Permissions::from_mode(0o000)).unwrap();
    let _restore = RestoreMode(film.clone());
    if fs::read_dir(&film).is_ok() {
        return; // root reads through mode 0o000
    }
    h.restart();
    let record = wait_for_file_end(&h, &id);
    assert!(matches!(record.state, State::Failed { .. }), "{:?}", record.state);
    let message = record.message.unwrap_or_default();
    assert!(message.contains(&final_path(&h).display().to_string()), "{message}");
    assert!(message.contains("was not checked") && !message.contains("holds no file"), "{message}");
    assert!(record.collisions.is_empty(), "{:?}", record.collisions);
}

/// Review round 1, V12 (spec 6.2 step 7, design part 7.3; RR1-13): a title's failed verifications are counted in
/// state.json, so a restart between the first and the second failure rips the title only once more, not twice.
#[test]
fn a_restart_after_a_first_verify_failure_rips_the_title_only_once_more() {
    let h = harness(&["movie-direct", "movie-direct-verify-twice"], settings());
    h.insert("tok0", DISC);
    h.run_until_event("the first TitleVerifyFailed", |event| {
        matches!(event, Event::TitleVerifyFailed { second: false, .. })
    });
    let dir = h.job_dir(&only_job(&h));
    let title = state_json(&dir).titles[0].clone();
    assert_eq!((title.attempts, title.verify_failures), (1, 1));
    h.restart();
    h.run_until("Ejected");
    let lines = transitions(&dir);
    assert!(lines.iter().any(|t| matches!(t.event, Some(Event::EngineRestart { .. }))));
    let failed: Vec<Event> =
        lines.into_iter().filter_map(|t| t.event).filter(|e| matches!(e, Event::TitleVerifyFailed { .. })).collect();
    let expected = vec![
        Event::TitleVerifyFailed { second: false, last: true },
        Event::TitleVerifyFailed { second: true, last: true },
    ];
    assert_eq!(failed, expected);
    let title = state_json(&dir).titles[0].clone();
    assert_eq!((title.attempts, title.verify_failures), (2, 2));
    assert_eq!(title.rip_reviews, vec![ReviewReason::VerifyFailed]);
    assert!(dir.join("t00-00800/attempt-1").is_dir());
    assert!(!dir.join("t00-00800/attempt-2").exists());
}

/// Sets or clears the read-only attribute of every file under `root` (on Unix, every write bit).
#[allow(clippy::permissions_set_readonly_false)]
fn set_read_only(root: &Path, read_only: bool) -> std::io::Result<()> {
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if path.is_dir() {
            set_read_only(&path, read_only)?;
        } else {
            let mut permissions = fs::metadata(&path)?.permissions();
            permissions.set_readonly(read_only);
            fs::set_permissions(&path, permissions)?;
        }
    }
    Ok(())
}

/// Makes a volume's files writable again when the test ends, so the harness can remove its scratch folder.
struct Writable(PathBuf);

impl Drop for Writable {
    fn drop(&mut self) {
        let _ = set_read_only(&self.0, false);
    }
}

/// Review round 1, V10 end to end (S29): a stop during the recording leaves the job in Detected with its first line
/// only, after the recording wrote its nav copies. The restart records the disc again into the same folder, and a
/// disc whose files are read-only no longer makes that second snapshot fail.
#[test]
fn a_restart_in_detected_after_a_finished_recording_fingerprints_again() {
    let h = harness(&["movie-direct"], settings());
    let volume = h.volume_root(DISC);
    let _writable = Writable(volume.clone());
    set_read_only(&volume, true).unwrap();
    h.insert("tok0", DISC);
    h.run_until("Fingerprinted");
    let dir = h.job_dir(&only_job(&h));
    assert!(dir.join("nav/BDMV/index.bdmv").is_file());
    let mut record = state_json(&dir);
    record.state = State::Detected;
    fs::write(dir.join("state.json"), serde_json::to_vec(&record).unwrap()).unwrap();
    let text = fs::read_to_string(dir.join("transitions.jsonl")).unwrap();
    fs::write(dir.join("transitions.jsonl"), format!("{}\n", text.lines().next().unwrap())).unwrap();
    h.restart();
    h.run_until("Filed");
    let lines = transitions(&dir);
    let at = lines.iter().position(|t| t.event == Some(Event::EngineRestart { disc_present: true })).expect("a restart");
    assert_eq!(lines[at].old, Some(State::Detected));
    assert_eq!(lines[at + 1].event, Some(Event::FingerprintDone));
    let log = fs::read_to_string(dir.join("job.log")).unwrap_or_default();
    assert!(!log.contains("could not be written"), "{log}");
}

#[test]
fn a_decide_after_restart_reads_its_inputs_back_with_devs_readers() {
    let h = harness(&["movie-direct"], settings());
    h.insert("tok0", DISC);
    h.run_until("Decided");
    let dir = h.job_dir(&only_job(&h));
    let key = fs::read_dir(&dir).unwrap().map(|e| e.unwrap().path()).find(|p| p.join("decide.json").is_file()).unwrap();
    let first = fs::read(key.join("decide.json")).unwrap();
    h.restart();
    h.run_until("Filed");
    assert!(fs::read_to_string(dir.join("job.log")).unwrap().contains(
        "decide inputs read back from the job folder with lazymkv-dev's readers (design part 5.10)"));
    assert_eq!(fs::read(key.join("decide.json")).unwrap(), first);
}

/// The PR #6 review's V14 (R-B1, RR1-14): a Decide after a restart reads the job's remembered.json back with dev's
/// readers and applies the same track overrides, even with the data folder's remembered decision gone: Decide never
/// reads the data folder's file. The remembered decision keeps movie-direct's one audio stream first and as the
/// default, which sets the audio part to 1.00.
#[test]
fn a_decide_after_restart_applies_the_jobs_remembered_track_overrides() {
    let h = harness(&["movie-direct"], settings());
    let fp = h.fingerprint(DISC);
    let data_copy = h.data().join("remembered").join(format!("{fp}.json"));
    fs::create_dir_all(data_copy.parent().unwrap()).unwrap();
    fs::write(&data_copy, format!(concat!(
        r#"{{"format":"lazymkv remembered v1","fingerprint":"{fp}","tmdb":{{"kind":"movie","id":900001}},"titles":[{{"#,
        r#""source_file":"00800.mpls","dvd_title":null,"segment_map":"1","seconds":7150,"role":{{"role":"feature"}},"#,
        r#""audio":[{{"signature":{{"kind":"audio","language":"eng","codec":"dts-hd-ma","channels":6,"position":0}},"#,
        r#""action":"keep","order":0,"default":true}}],"subtitles":[]}}]}}"#), fp = fp)).unwrap();
    h.insert("tok0", DISC);
    h.run_until("Decided");
    let dir = h.job_dir(&only_job(&h));
    let key = dir.join("t00-00800");
    let first = fs::read(key.join("decide.json")).unwrap();
    let doc: serde_json::Value = serde_json::from_slice(&first).unwrap();
    assert_eq!(doc["decision"]["decision"]["audio"]["confidence"], json!(1.0));
    fs::remove_file(&data_copy).unwrap();
    h.restart();
    h.run_until("Filed");
    assert!(fs::read_to_string(dir.join("job.log")).unwrap().contains(
        "decide inputs read back from the job folder with lazymkv-dev's readers (design part 5.10)"));
    assert_eq!(fs::read(key.join("decide.json")).unwrap(), first);
}

#[test]
fn awaiting_lookup_recreates_its_timer() {
    use lazymkv_engine::testkit::StepScript;
    use lazymkv_engine::StepResult;
    let steps = vec![
        ("RunScan", StepScript::Result(StepResult { event: Some(Event::ScanDone), message: None })),
        ("Plan", StepScript::Result(StepResult { event: Some(Event::PlanDone { mode: lazymkv_core::plan::Mode::Direct }), message: None })),
        ("CheckSpace", StepScript::Result(StepResult { event: Some(Event::SpaceOk), message: None })),
        ("RipTitle", StepScript::Result(StepResult { event: Some(Event::TitleRipped), message: None })),
        ("VerifyFile", StepScript::Result(StepResult { event: Some(Event::TitleVerified { last: true }), message: None })),
        ("Eject", StepScript::Result(StepResult { event: Some(Event::Ejected), message: None })),
        ("Probe", StepScript::Result(StepResult { event: Some(Event::ProbeDone), message: None })),
        ("ExtractSubtitleEvidence", StepScript::Result(StepResult { event: None, message: None })),
        ("Decide", StepScript::Result(StepResult { event: Some(Event::LookupPending), message: None })),
    ];
    let h = Harness::new_scripted(steps, settings());
    h.insert("tok0", DISC);
    h.run_until("AwaitingLookup");
    let dir = h.job_dir(&only_job(&h));
    // PF-4: the scripted Eject never calls the ejector, so this stands for the removal a real eject causes (R22.6).
    h.remove("tok0");
    h.restart();
    let restart = transitions(&dir).pop().unwrap();
    assert_eq!(restart.event, Some(Event::EngineRestart { disc_present: false }));
    assert_eq!(restart.actions, vec![Action::StartTimer { minutes: 60 }]);
    let before = transitions(&dir).len();
    h.fire_timers();
    let after = std::time::Instant::now();
    // 10 s, 40 s under CI (Ruling R29.c5).
    let wait = std::time::Duration::from_secs(10) * ci_factor();
    while transitions(&dir).len() == before && after.elapsed() < wait {
        std::thread::yield_now();
    }
    let fired = transitions(&dir).pop().unwrap();
    assert!(matches!(fired.old, Some(State::AwaitingLookup { .. })));
    assert!(matches!(fired.event, Some(Event::LookupPending | Event::LookupAnswered)));
}

// ---- Unpersisted transitions (review round 1, V1, RR1-16; Ruling W23) ----

/// The ID of the first job of the base scenario's disc.
fn first_job(h: &Harness) -> String {
    format!("{}-1", h.fingerprint(DISC).to_ascii_lowercase())
}

/// RR1-16 (engine.md:593; Ruling W23): with no Java and a profile MakeMKV rejects, RunScan posts ToolMissing { Java }
/// and the fallback's `record.profile`, then reports ScanDone. When ToolMissing's transition and the failure fed in its
/// place both cannot be persisted, the report stops there: ScanDone is not fed, so no Scanned is persisted with the
/// job profile while scan.txt came from the fallback.
#[test]
fn a_lost_tool_missing_java_stops_the_report_before_scan_done() {
    use lazymkv_core::job::Tool;
    let h = harness(&["movie-direct", "movie-direct-fallback"], json!({"keep_tuning_evidence": false,
        "drive_settle_s": 0, "drive_ready_timeout_s": 1, "tool_paths": {"java": null}}));
    h.refuse_persists(|_, transition| {
        transition.event == Some(Event::ToolMissing { tool: Tool::Java }) || matches!(transition.new, State::Failed { .. })
    });
    h.insert("tok0", DISC);
    let id = first_job(&h);
    let kept = format!("{id}: keeps its persisted state Fingerprinted until a restart resumes it");
    h.wait("the lost ToolMissing's failure kept", |h| h.engine_log().contains(&kept));
    h.settle();
    let lines = h.transitions(&id);
    assert!(lines.iter().all(|line| line.event != Some(Event::ScanDone)), "ScanDone was fed: {lines:?}");
    let record = h.record(&id);
    assert_eq!(record.state, State::Fingerprinted);
    assert_eq!(record.profile, "job.mmcp.xml");
    assert!(h.job_dir(&id).join("fallback.mmcp.xml").is_file(), "the fallback scan ran");
}

/// V1 (engine.md:592; Ruling W23): CheckSpace's SpaceOk in Planned, a state with no failure row, whose transition
/// cannot be persisted is kept and fed again at a one-minute timer, again after each refusal, until it persists; the
/// job then rips and files as usual.
#[test]
fn a_planned_job_whose_next_transition_failed_to_persist_moves_on_at_the_retry_timer() {
    let h = harness(&["movie-direct"], settings());
    h.refuse_persists(|_, transition| transition.event == Some(Event::SpaceOk));
    h.insert("tok0", DISC);
    let id = first_job(&h);
    let kept = format!("{id}: keeps its persisted state Planned; SpaceOk is fed again in 1 minute");
    h.wait("SpaceOk kept", |h| h.engine_log().contains(&kept));
    assert_eq!(h.state_of(&id), "Planned");
    assert_eq!(h.timers().pending(), vec![1]);
    h.fire_timers();
    h.wait("SpaceOk kept again", |h| h.engine_log().matches(&kept).count() == 2);
    assert_eq!(h.state_of(&id), "Planned");
    assert_eq!(h.timers().pending(), vec![1]);
    h.allow_persists();
    h.fire_timers();
    h.wait("SpaceOk persisted", |h| h.transitions(&id).iter().any(|line| line.event == Some(Event::SpaceOk)));
    let fed = h.transitions(&id).into_iter().find(|line| line.event == Some(Event::SpaceOk)).unwrap();
    let planned = State::Planned { mode: lazymkv_core::plan::Mode::Direct };
    assert_eq!((fed.old, fed.new), (Some(planned), State::Ripping));
    h.run_until("Filed");
}

/// V1 (engine.md:592; Ruling W23): Eject's Ejected in RipVerified, a state with no failure row, whose transition cannot
/// be persisted is kept and fed again at the one-minute timer; the disc is not ejected a second time.
#[test]
fn a_rip_verified_job_moves_on_at_the_retry_timer() {
    let h = harness(&["movie-direct"], settings());
    h.refuse_persists(|_, transition| transition.event == Some(Event::Ejected));
    h.insert("tok0", DISC);
    let id = first_job(&h);
    let kept = format!("{id}: keeps its persisted state RipVerified; Ejected is fed again in 1 minute");
    h.wait("Ejected kept", |h| h.engine_log().contains(&kept));
    assert_eq!(h.state_of(&id), "RipVerified");
    assert_eq!(h.timers().pending(), vec![1]);
    assert_eq!(h.ejector().calls(), vec!["tok0".to_string()]);
    h.allow_persists();
    h.fire_timers();
    h.run_until("Filed");
    let fed = h.transitions(&id).into_iter().find(|line| line.event == Some(Event::Ejected)).unwrap();
    assert_eq!((fed.old, fed.new), (Some(State::RipVerified { after_copy: false }), State::Ejected { after_copy: false }));
    assert_eq!(h.ejector().calls(), vec!["tok0".to_string()]);
}

/// V1 (engine.md:592; Ruling W23): an Ejected job's FilePhaseSlot whose transition cannot be persisted goes back into
/// the slot's queue (review round 1) and is granted again at the one-minute timer, with no other job's transition,
/// lookup round, `settings_set` or restart needed.
#[test]
fn an_ejected_jobs_slot_grant_is_persisted_at_the_retry_timer() {
    let h = harness(&["movie-direct"], settings());
    h.refuse_persists(|_, transition| transition.event == Some(Event::FilePhaseSlot));
    h.insert("tok0", DISC);
    let id = first_job(&h);
    let kept = format!("{id}: keeps its persisted state Ejected; FilePhaseSlot is fed again in 1 minute");
    h.wait("FilePhaseSlot kept", |h| h.engine_log().contains(&kept));
    h.settle();
    assert_eq!(h.state_of(&id), "Ejected");
    assert_eq!(h.timers().pending(), vec![1]);
    h.allow_persists();
    h.fire_timers();
    h.run_until("Filed");
    let granted = h.transitions(&id).into_iter().find(|line| line.event == Some(Event::FilePhaseSlot)).unwrap();
    assert_eq!((granted.old, granted.new), (Some(State::Ejected { after_copy: false }), State::Inspecting));
}
