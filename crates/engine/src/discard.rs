//! The `discard_preview` request's worker (phase 3 design, parts 6.8, 11; ruling T32-R7): the bytes and file count of a
//! job's staging folder, counted on a thread of its own because it reads the staging share, which the owner thread
//! never reads. It only reads. Def: P3-T10. Use: P3-T15, P3-T16.

use std::fs;
use std::io::ErrorKind;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use lazymkv_core::settings::configured_root;
use lazymkv_platform::{Reach, ShareConnector};
use serde_json::{json, Value};

use crate::cleanup::keeps_replaced;
use crate::file_phase::REPLACED_FOLDER;
use crate::owner::panic_text;

/// The start of the `unknown` reason of a preview whose staging share did not answer (ruling T32-R7). Def: P3-T10.
/// Use: P3-T15, P3-T16.
pub(crate) const STAGING_UNANSWERED: &str = "the staging share did not answer: ";

/// Counts `<staging_root>/<id>/` on a thread of its own and hands the answer to `reply` (phase 3 design, part 6.8;
/// ruling T32-R7): `{"bytes": n, "files": n}`, the sum of the sizes of the regular files under the folder and their
/// count, no link followed and a `replaced` folder the discard keeps left out (Ruling T32-E11); zeros for a job with no
/// staging root set (`configured_root`) or with no folder there; `{"unknown": "<reason>"}` when the share does not
/// answer (`STAGING_UNANSWERED` and its reason) or a folder under it cannot be read (its path and the error), which a
/// client shows as a size it does not know. A panic is answered as `unknown` with its text. The owner thread keeps the
/// handle and does not wait for it at a stop. Def: P3-T10. Use: P3-T15, P3-T16.
pub(crate) fn spawn_preview(
    id: String,
    staging_root: Option<String>,
    shares: Arc<dyn ShareConnector>,
    reply: impl FnOnce(Value) + Send + 'static,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let counted = panic::catch_unwind(AssertUnwindSafe(|| preview(&id, staging_root.as_deref(), shares.as_ref())));
        reply(counted.unwrap_or_else(|payload| json!({ "unknown": panic_text(&*payload) })));
    })
}

/// The preview's answer for job `id` under `staging_root`.
fn preview(id: &str, staging_root: Option<&str>, shares: &dyn ShareConnector) -> Value {
    let Some(root) = configured_root(staging_root) else {
        return json!({"bytes": 0, "files": 0});
    };
    if let Reach::Unreachable(why) = shares.reach(root) {
        return json!({ "unknown": format!("{STAGING_UNANSWERED}{why}") });
    }
    match count(&Path::new(root).join(id)) {
        Ok((bytes, files)) => json!({"bytes": bytes, "files": files}),
        Err(reason) => json!({ "unknown": reason }),
    }
}

/// The bytes and count of the regular files under `folder`, read by each entry's own type, so a link is neither
/// followed nor counted, and leaving out a `replaced` folder the discard keeps (`cleanup::keeps_replaced`; Ruling
/// T32-E11, rule 5; P3-T11 fix round 1); (0, 0) when `folder` is not there. Err is the path that could not be read and
/// the error.
fn count(folder: &Path) -> Result<(u64, u64), String> {
    match fs::symlink_metadata(folder) {
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => return Ok((0, 0)),
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok((0, 0)),
        Err(error) => return Err(format!("{}: {error}", folder.display())),
    }
    let unread = |path: &Path, error: std::io::Error| format!("{}: {error}", path.display());
    // Ruling T32-E11, rule 5: a `replaced` folder the discard keeps is not counted.
    let kept = keeps_replaced(folder).then(|| folder.join(REPLACED_FOLDER));
    let (mut bytes, mut files) = (0u64, 0u64);
    let mut folders: Vec<PathBuf> = vec![folder.to_path_buf()];
    while let Some(next) = folders.pop() {
        for entry in fs::read_dir(&next).map_err(|error| unread(&next, error))? {
            let entry = entry.map_err(|error| unread(&next, error))?;
            let path = entry.path();
            let kind = entry.file_type().map_err(|error| unread(&path, error))?;
            if kind.is_dir() && kept.as_ref() != Some(&path) {
                folders.push(path);
            } else if kind.is_file() {
                bytes += entry.metadata().map_err(|error| unread(&path, error))?.len();
                files += 1;
            }
        }
    }
    Ok((bytes, files))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lazymkv_core::testsupport::{Scratch, ScratchExt};
    use lazymkv_platform::fakes::FakeShares;

    /// Ruling T32-E11, rule 5: the preview does not count a non-empty `replaced` folder, which a discard keeps; the
    /// rest of the staging folder is counted as before.
    #[test]
    fn the_preview_does_not_count_the_replaced_folder_a_discard_keeps() {
        let s = Scratch::new("preview-replaced");
        s.write("staging/job-1/rip/t00/title_t00.mkv", "12345");
        s.write(&format!("staging/job-1/{REPLACED_FOLDER}/Film - 1080p.mkv"), "owner's film");
        let root = s.0.join("staging").display().to_string();
        assert_eq!(preview("job-1", Some(&root), &FakeShares::new()), json!({"bytes": 5, "files": 1}));
    }
}
