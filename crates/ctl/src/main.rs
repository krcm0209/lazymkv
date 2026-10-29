//! lazymkvctl's binary (design parts 4.1, 9.5, 9.6). It hands its arguments, context and streams to `lazymkv_ctl::run`.
//! On Windows the engine commands reach the engine through the named pipe's client (design part 3.3); on Linux and
//! macOS there is no transport, and they exit 4 with NO_TRANSPORT (S38).
fn main() {
    // As lazymkv-dev's main reads them, so a non-Unicode argument cannot panic and the exit codes stay identical (P23).
    let argv: Vec<String> = std::env::args_os().skip(1).map(|a| a.to_string_lossy().into_owned()).collect();
    let ctx = lazymkv_dev::evidence::Ctx::from_process();
    #[cfg(windows)]
    let pipe = lazymkv_platform::win::PipeConnector::new(lazymkv_platform::PIPE_NAME);
    #[cfg(windows)]
    let connector: Option<&dyn lazymkv_platform::IpcConnector> = Some(&pipe);
    #[cfg(not(windows))]
    let connector: Option<&dyn lazymkv_platform::IpcConnector> = None;
    let (stdin, stdout, stderr) = (std::io::stdin(), std::io::stdout(), std::io::stderr());
    let code = lazymkv_ctl::run(&argv, lazymkv_ctl::Env { ctx: &ctx, connector, stdin: &mut stdin.lock(),
        out: &mut stdout.lock(), err: &mut stderr.lock() });
    std::process::exit(code);
}
