//! lazymkvctl's binary (design parts 4.1, 9.5, 9.6). It hands its arguments, context and streams to `lazymkv_ctl::run`.
fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let ctx = lazymkv_dev::evidence::Ctx::from_process();
    let (stdin, stdout, stderr) = (std::io::stdin(), std::io::stdout(), std::io::stderr());
    let code = lazymkv_ctl::run(&argv, lazymkv_ctl::Env { ctx: &ctx, connector: None, stdin: &mut stdin.lock(),
        out: &mut stdout.lock(), err: &mut stderr.lock() });
    std::process::exit(code);
}
