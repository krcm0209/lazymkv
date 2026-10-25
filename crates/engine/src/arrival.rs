//! Arrivals (design part 5.4 steps 1 to 3): the wait for the volume, the fingerprint walk before any job exists, the
//! new job's recording, and the lookup chain's inputs read back from a job folder. No `makemkvcon` runs here, so no
//! arrival waits for another disc's phase (R-A4).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use lazymkv_clients::ChainInputs;
use lazymkv_core::disc::{bdmt_candidates, bdmt_name};
use lazymkv_core::job::{Event, Failure, FailureClass};
use lazymkv_core::lookup::name::disc_name;
use lazymkv_core::snapshot::{disc_meta, unit_key_file, DiscMeta};
use lazymkv_platform::Host;
use lazymkv_tools::{list_files, snapshot};

use crate::owner::{Msg, StepResult};

/// How often the wait for a volume looks for its root (design part 5.4).
const READY_POLL: Duration = Duration::from_millis(100);

/// A fingerprinted arrival (design part 5.4 steps 1 and 2): the drive, the volume root, the drive's arrival number at
/// the time (a removal or a later arrival makes it stale), meta.json's values and the volume label. Def: T22. Use: T22,
/// T25.
#[derive(Clone, Debug)]
pub(crate) struct Arrival {
    /// The drive's ID.
    pub drive: String,
    /// The volume root the disc is read under.
    pub root: PathBuf,
    /// The drive's arrival number when the disc arrived.
    pub number: u64,
    /// meta.json's values, with a fingerprint.
    pub meta: DiscMeta,
    /// The volume label, empty when the host reads none.
    pub label: String,
}

/// Waits `settle`, waits up to `ready` for `root` to exist (polled every 100 ms), then `read_arrival`; sends the
/// outcome as `Msg::Arrival`, a failure as (drive, sentence) (design part 5.4 step 1). A root still missing after
/// `ready` fails with `the volume did not appear within drive_ready_timeout_s (<n> s)`. Def: T22. Use: T22, T25.
pub(crate) fn spawn_arrival(
    drive: String,
    root: PathBuf,
    number: u64,
    settle: Duration,
    ready: Duration,
    host: Arc<dyn Host>,
    done: Sender<Msg>,
) {
    thread::spawn(move || {
        thread::sleep(settle);
        let deadline = Instant::now() + ready;
        while !root.exists() {
            if Instant::now() >= deadline {
                let sentence =
                    format!("the volume did not appear within drive_ready_timeout_s ({} s)", ready.as_secs());
                let _ = done.send(Msg::Arrival(Err((drive, sentence))));
                return;
            }
            thread::sleep(READY_POLL);
        }
        let outcome = read_arrival(&drive, &root, number, host.as_ref()).map_err(|sentence| (drive.clone(), sentence));
        let _ = done.send(Msg::Arrival(outcome));
    });
}

/// Lists the volume, reads the unit key, `disc_meta` (design part 5.4 step 2). A failed walk gives `the disc's
/// fingerprint could not be computed: <io error>`; a fingerprint of None gives `the disc's fingerprint could not be
/// computed: the volume holds no BDMV/STREAM or VIDEO_TS files`. The label is the host's, else empty.
/// Def: T22. Use: T22, T25.
pub(crate) fn read_arrival(drive: &str, root: &Path, number: u64, host: &dyn Host) -> Result<Arrival, String> {
    let unreadable = |reason: String| format!("the disc's fingerprint could not be computed: {reason}");
    let files = list_files(root).map_err(|error| unreadable(error.to_string()))?;
    let unit_key = unit_key_file(&files).and_then(|file| fs::read(root.join(&file.path)).ok());
    let label = host.volume_label(root).unwrap_or_default();
    let meta = disc_meta(&files, &label, unit_key.as_deref(), host.now_unix());
    if meta.fingerprint.is_none() {
        return Err(unreadable("the volume holds no BDMV/STREAM or VIDEO_TS files".to_string()));
    }
    Ok(Arrival { drive: drive.to_string(), root: root.to_path_buf(), number, meta, label })
}

/// Writes the new job's recording with `tools::snapshot` (design part 5.4 steps 2 and 3: the `Fingerprint` that
/// `new_job` returns "is already done") and sends its `Msg::StepDone`: FingerprintDone, or
/// FingerprintFailed { Unknown } with the message `the disc's recording could not be written: <error>` when the
/// snapshot fails or its fingerprint differs from the arrival's. Def: T22. Use: T22.
pub(crate) fn spawn_recording(
    job: String,
    generation: u64,
    arrival: Arrival,
    job_dir: PathBuf,
    captured_unix: u64,
    done: Sender<Msg>,
) {
    thread::spawn(move || {
        let written = snapshot(&arrival.root, &arrival.label, &job_dir, captured_unix)
            .map_err(|error| error.to_string())
            .and_then(|meta| match (&meta.fingerprint, &arrival.meta.fingerprint) {
                (now, then) if now == then => Ok(()),
                (now, then) => Err(format!(
                    "its fingerprint {} differs from the arrival's {}",
                    now.as_deref().unwrap_or("none"),
                    then.as_deref().unwrap_or("none")
                )),
            });
        let result = match written {
            Ok(()) => StepResult { event: Some(Event::FingerprintDone), message: None },
            Err(error) => StepResult {
                event: Some(Event::FingerprintFailed {
                    failure: Failure { class: FailureClass::Unknown, output: None },
                }),
                message: Some(format!("the disc's recording could not be written: {error}")),
            },
        };
        let _ = done.send(Msg::StepDone { job, generation, result });
    });
}

/// The lookup chain's inputs from a job folder: meta.json's `fingerprint`, `disc_id` and `volume_label`, and the
/// `<di:name>` of the first of `bdmt_candidates` under `nav/BDMV/META/DL/` (design part 5.4 step 1, spec 8.1). The name
/// is `disc_name(bdmt, Some(volume_label))`. An unreadable meta.json, or one without a `fingerprint`, is an Err naming
/// meta.json. Def: T22. Use: T23, T24, T25.
#[cfg_attr(not(test), expect(dead_code, reason = "Task 23's RunScan and Plan read the chain's inputs"))]
pub(crate) fn chain_inputs(job_dir: &Path) -> Result<ChainInputs, String> {
    let path = job_dir.join("meta.json");
    let shown = path.display();
    let bytes = fs::read(&path).map_err(|error| format!("{shown}: {error}"))?;
    let meta: serde_json::Value = serde_json::from_slice(&bytes).map_err(|error| format!("{shown}: {error}"))?;
    let fingerprint =
        meta["fingerprint"].as_str().ok_or_else(|| format!("{shown}: no fingerprint"))?.to_string();
    let disc_id = meta["disc_id"].as_str().map(str::to_string);
    let dl = job_dir.join("nav").join("BDMV").join("META").join("DL");
    let names: Vec<String> = fs::read_dir(&dl)
        .map(|entries| {
            let names = entries.filter_map(|entry| entry.ok()).map(|entry| entry.file_name());
            names.map(|name| name.to_string_lossy().into_owned()).collect()
        })
        .unwrap_or_default();
    let bdmt = bdmt_candidates(&names)
        .first()
        .and_then(|first| fs::read(dl.join(first)).ok())
        .and_then(|bytes| bdmt_name(&String::from_utf8_lossy(&bytes)));
    let name = disc_name(bdmt.as_deref(), meta["volume_label"].as_str());
    Ok(ChainInputs { fingerprint, disc_id, name })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lazymkv_core::lookup::name::NameSource;
    use lazymkv_core::testsupport::{Scratch, ScratchExt};

    #[test]
    fn a_bdmt_name_wins_over_the_volume_label_for_the_search() {
        let job = Scratch::new("bdmt");
        job.write("meta.json", r#"{"volume_label":"SYNTHETIC_MOVIE","format":"bluray","fingerprint":"ABC","disc_id":"DEF","file_count":5,"total_bytes":1,"captured_unix":7}"#);
        job.write("nav/BDMV/META/DL/bdmt_fra.xml", "<di:name>Film Synthétique</di:name>");
        job.write("nav/BDMV/META/DL/bdmt_eng.xml", "<di:name>Synthetic Movie</di:name>");
        let inputs = chain_inputs(&job.0).unwrap();
        assert_eq!((inputs.fingerprint.as_str(), inputs.disc_id.as_deref()), ("ABC", Some("DEF")));
        let name = inputs.name.unwrap();
        assert_eq!((name.raw.as_str(), name.source), ("Synthetic Movie", NameSource::Bdmt));
    }

    #[test]
    fn without_a_bdmt_file_the_volume_label_names_the_disc() {
        let job = Scratch::new("label");
        job.write("meta.json", r#"{"volume_label":"SYNTHETIC_MOVIE","format":"bluray","fingerprint":"ABC","disc_id":null,"file_count":5,"total_bytes":1,"captured_unix":7}"#);
        let inputs = chain_inputs(&job.0).unwrap();
        assert_eq!(inputs.disc_id, None);
        assert_eq!(inputs.name.unwrap().source, NameSource::VolumeLabel);
    }
}
