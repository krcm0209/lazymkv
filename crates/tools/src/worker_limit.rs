//! The optical character recognition (OCR) worker's memory limit (design part 5.3; notes A1, A2 and call 10; T6-R2):
//! the frozen `ocr::run_worker` over a `ToolCtx` whose runner is the caller's wrapped in `MemoryLimited`, so on Windows
//! the worker's Job Object holds its whole tree to `WORKER_MEMORY_LIMIT`. No other tool gets a limit.

use std::io;
use std::path::Path;
use std::time::Duration;

use lazymkv_platform::MemoryLimited;

use crate::ocr::{run_worker, WorkerEnd};
use crate::record::ToolCtx;

/// The most memory the OCR worker and what it starts may commit together, in bytes: 2 gibibytes (GiB), a fixed value
/// and not a setting (design part 5.3; notes A2; T6-R2). Def: T6-1. Use: T6-1.
pub const WORKER_MEMORY_LIMIT: u64 = 2 * 1024 * 1024 * 1024;

/// `ocr::run_worker` under `WORKER_MEMORY_LIMIT` (design part 5.3; notes A1 and call 10; T6-R2): `ctx`'s runner
/// wrapped in `MemoryLimited`, `ctx`'s other five fields unchanged. Step 10 and `lazymkv-dev ocr-trial read` start the
/// worker through it, so a trial measures what ships. A stream whose worker meets the limit ends unread, as a
/// worker abort does (notes A1). Def: T6-1. Use: T6-1.
pub fn run_worker_limited(
    ctx: &ToolCtx,
    program: &Path,
    args: &[String],
    capture: &Path,
    cap: Duration,
) -> io::Result<WorkerEnd> {
    let guard = MemoryLimited::new(ctx.guard, WORKER_MEMORY_LIMIT);
    let limited = ToolCtx {
        guard: &guard,
        host: ctx.host,
        limits: ctx.limits,
        cancel: ctx.cancel.clone(),
        on_output: ctx.on_output,
        on_progress: ctx.on_progress,
    };
    run_worker(&limited, program, args, capture, cap)
}
