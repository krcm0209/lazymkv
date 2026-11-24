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

use crate::owner::panic_text;

/// The start of the `unknown` reason of a preview whose staging share did not answer (ruling T32-R7). Def: P3-T10.
/// Use: P3-T15, P3-T16.
pub(crate) const STAGING_UNANSWERED: &str = "the staging share did not answer: ";

/// Counts `<staging_root>/<id>/` on a thread of its own and hands the answer to `reply` (phase 3 design, part 6.8;
/// ruling T32-R7): `{"bytes": n, "files": n}`, the sum of the sizes of the regular files under the folder and their
/// count, no link followed; zeros for a job with no staging root set (`configured_root`) or with no folder there;
/// `{"unknown": "<reason>"}` when the share does not answer (`STAGING_UNANSWERED` and its reason) or a folder under it
/// cannot be read (its path and the error), which a client shows as a size it does not know. A panic is answered as
/// `unknown` with its text. The owner thread keeps the handle and does not wait for it at a stop. Def: P3-T10.
/// Use: P3-T15, P3-T16.
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
/// followed nor counted; (0, 0) when `folder` is not there. Err is the path that could not be read and the error.
fn count(folder: &Path) -> Result<(u64, u64), String> {
    match fs::symlink_metadata(folder) {
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => return Ok((0, 0)),
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok((0, 0)),
        Err(error) => return Err(format!("{}: {error}", folder.display())),
    }
    let unread = |path: &Path, error: std::io::Error| format!("{}: {error}", path.display());
    let (mut bytes, mut files) = (0u64, 0u64);
    let mut folders: Vec<PathBuf> = vec![folder.to_path_buf()];
    while let Some(next) = folders.pop() {
        for entry in fs::read_dir(&next).map_err(|error| unread(&next, error))? {
            let entry = entry.map_err(|error| unread(&next, error))?;
            let path = entry.path();
            let kind = entry.file_type().map_err(|error| unread(&path, error))?;
            if kind.is_dir() {
                folders.push(path);
            } else if kind.is_file() {
                bytes += entry.metadata().map_err(|error| unread(&path, error))?.len();
                files += 1;
            }
        }
    }
    Ok((bytes, files))
}
