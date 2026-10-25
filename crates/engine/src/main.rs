//! lazymkv-engine's binary (design parts 3.5, 4.1, 4.2; S28): `--data DIR` through `lazymkv_engine::main_with`, which
//! in pull request 2 exits 1 on every OS with its OS's sentence.
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(lazymkv_engine::main_with(&args, &mut std::io::stderr()));
}
