//! lazymkv-engine's binary (design parts 3.1 to 3.8, 4.1, 4.2; S28): `[--data DIR]` through
//! `lazymkv_engine::main_with`, which runs the engine on Windows (`native::run_with`) and elsewhere exits 1 with S28's
//! sentence.
fn main() {
    // As lazymkv-dev's and lazymkvctl's mains read them, so an argument that is not Unicode cannot panic (exit 101).
    let args: Vec<String> = std::env::args_os().skip(1).map(|arg| arg.to_string_lossy().into_owned()).collect();
    std::process::exit(lazymkv_engine::main_with(&args, &mut std::io::stderr()));
}
