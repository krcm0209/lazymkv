//! lazymkv-tools: the makemkvcon, mkvmerge and ffprobe drivers, their capture-style recorded runs under ProcessGuard, the tool lookup of design part 3.6, and the disc snapshot's file I/O moved from capture (design part 4.1, R-B2).

mod lookup;
mod record;

pub use lookup::{exe_name, find_tool, Found, ToolLocation};
pub use record::{
    run_recorded, tee_lines, with_suffix, CancelToken, Clock, LineAction, Limits, RealClock, RunResult, ToolCtx,
};

#[cfg(test)]
pub(crate) mod testutil {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A fresh, empty folder under the system temporary folder, named by `tag`, the process id and a count.
    pub fn temp_dir(tag: &str) -> PathBuf {
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        let n = COUNT.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("lazymkv-tools-{tag}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
