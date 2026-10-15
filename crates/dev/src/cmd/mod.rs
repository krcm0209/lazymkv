//! One file per command (port spec 3.4). Each defines
//! `pub fn run(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32`; the task that writes
//! it adds the file's `pub mod` line here and its row to `crate::COMMANDS`.
