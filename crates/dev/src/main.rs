//! The lazymkv-dev binary: the process's arguments, environment and standard streams handed to `dispatch`.

use std::io::Write;

fn main() {
    let argv: Vec<String> = std::env::args_os().skip(1).map(|arg| arg.to_string_lossy().into_owned()).collect();
    let ctx = lazymkv_dev::evidence::Ctx::from_process();
    let mut out = std::io::stdout().lock();
    let mut err = std::io::stderr().lock();
    let code = lazymkv_dev::dispatch(&ctx, &argv, &mut out, &mut err);
    let _ = out.flush();
    let _ = err.flush();
    std::process::exit(code);
}
