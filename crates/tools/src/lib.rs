//! lazymkv-tools: the makemkvcon, mkvmerge and ffprobe drivers, their capture-style recorded runs under ProcessGuard, the tool lookup of design part 3.6, and the disc snapshot's file I/O moved from capture (design part 4.1, R-B2).

/// The ffprobe driver (design parts 5.3, 5.11). Def: T14. Use: T24.
pub mod ffprobe;
mod lookup;
/// The makemkvcon drivers: the drive listing, the scan and the rip (design parts 5.3, 5.4, 5.6). Def: T13. Use: T23,
/// T26.
pub mod makemkv;
/// The mkvmerge drivers: the identification and the remux (design parts 5.3, 5.7). Def: T14. Use: T23, T24.
pub mod mkvmerge;
mod record;
mod snapshot;

pub use ffprobe::{probe, probe_args};
pub use lookup::{exe_name, find_tool, Found, ToolLocation};
pub use makemkv::{
    drive_index, drive_list_args, list_drives, rip, rip_args, save_debug_log, scan, scan_args, DriveListing,
    MakemkvFlags, RipOutcome, RipRun, ScanOutcome, StagingHealth,
};
pub use mkvmerge::{gui_progress, ident, ident_args, remux, remux_args};
pub use record::{
    run_recorded, tee_lines, with_suffix, CancelToken, Clock, LineAction, Limits, RealClock, RunResult, ToolCtx,
};
pub use snapshot::{list_files, snapshot};

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
