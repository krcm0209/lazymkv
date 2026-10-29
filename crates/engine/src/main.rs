//! lazymkv-engine's binary (design parts 3.1 to 3.8, 4.1, 4.2; S28; M7): `[--data DIR]`, each argument Unicode
//! (`lazymkv_engine::unicode_args`), through `lazymkv_engine::main_with`, which runs the engine on Windows
//! (`native::run_with`) and elsewhere exits 1 with S28's sentence.
use std::io::Write;

fn main() {
    // M7, amended at review round 1 of pull request 3 (R5): an argument that is not Unicode is refused with exit 1
    // before anything is read, created or locked, on every system. `writeln!` with `let _`, never `eprintln!`, which
    // panics (exit 101) when standard error cannot be written.
    let code = match lazymkv_engine::unicode_args(std::env::args_os().skip(1)) {
        Ok(args) => lazymkv_engine::main_with(&args, &mut std::io::stderr()),
        Err(text) => {
            let _ = writeln!(std::io::stderr(), "{text}");
            1
        }
    };
    std::process::exit(code);
}
