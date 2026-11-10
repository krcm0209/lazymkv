//! lazymkv-tools: the makemkvcon, mkvmerge, ffprobe and ffmpeg drivers, their capture-style recorded runs under ProcessGuard, the tool lookup of design part 3.6, and the disc snapshot's file I/O moved from capture (design part 4.1, R-B2).

/// The ffmpeg driver: step 10a's argument lists and its runner (design parts 5.3, 5.7 step 10a; R5-code-27). Def:
/// T5-13. Use: T5-17.
pub mod ffmpeg;
/// The ffprobe driver: the streams read of step 9, step 10's packet listing and step 10a's proxy check (design parts
/// 5.3, 5.7, 5.11). Def: T14, T5-13. Use: T24, T5-14, T5-17.
pub mod ffprobe;
mod lookup;
/// The makemkvcon drivers: the drive listing, the scan and the rip (design parts 5.3, 5.4, 5.6). Def: T13. Use: T23,
/// T26.
pub mod makemkv;
/// The mkvmerge drivers: the identification, the remux and step 10's subtitle extraction (design parts 5.3, 5.7,
/// 5.11). Def: T14, T5-13. Use: T23, T24, T5-14.
pub mod mkvmerge;
/// The OCR worker's driver: its arguments, its one environment variable, its time cap and how its run ended (design
/// parts 5.3, 10.7, 10.8; K2). Def: T5-9. Use: T5-10, T5-15.
pub mod ocr;
mod record;
mod snapshot;

pub use ffmpeg::{layouts_args, run_ffmpeg, streamhash_args};
pub use ffprobe::{packets, packets_args, probe, probe_args, proxy_check_args};
pub use lookup::{exe_name, find_tool, Found, ToolLocation};
pub use makemkv::{
    drive_index, drive_list_args, list_drives, rip, rip_args, save_debug_log, scan, scan_args, DriveListing,
    MakemkvFlags, RipOutcome, RipRun, ScanOutcome, StagingHealth,
};
pub use mkvmerge::{extract, extract_args, gui_progress, ident, ident_args, remux, remux_args};
pub use ocr::{end_reason, ocr_args, run_worker, WorkerEnd, RTEN_NUM_THREADS};
pub use record::{
    run_recorded, run_recorded_in, tee_lines, with_suffix, CancelToken, Clock, LineAction, Limits, RealClock,
    RunResult, ToolCtx,
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
