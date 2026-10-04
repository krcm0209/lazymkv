use std::process::Command;

#[test]
fn record_exits_with_the_childs_exit_code() {
    let dir = std::env::temp_dir().join(format!("lazymkv-exit-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let run = |code: &str| {
        let child: Vec<String> = if cfg!(windows) {
            vec!["cmd".into(), "/c".into(), format!("exit /b {code}")]
        } else {
            vec!["sh".into(), "-c".into(), format!("exit {code}")]
        };
        Command::new(env!("CARGO_BIN_EXE_lazymkv-capture"))
            .arg("record")
            .arg("--out")
            .arg(dir.join(format!("out{code}.txt")))
            .arg("--")
            .args(&child)
            .output()
            .unwrap()
            .status
            .code()
    };
    assert_eq!(run("3"), Some(3));
    #[cfg(windows)]
    assert_eq!(run("-5"), Some(-5));
}
