//! One file per command (port spec 3.4). Each defines
//! `pub fn run(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32`; the task that writes
//! it adds the file's `pub mod` line here and its row to `crate::COMMANDS`.

pub mod scan;
pub mod reconcile;
pub mod titles;
pub mod audio_report;
pub mod labels;
pub mod levels_args;
pub mod retune;
pub mod align;
pub mod subs;
pub mod levels_from_proxy;
pub mod exp02;
pub mod encoding;
pub mod hit_rate;
pub mod compare_scans;
pub mod exp11;
pub mod ocr_compare;
pub mod decide;
pub mod golden;
