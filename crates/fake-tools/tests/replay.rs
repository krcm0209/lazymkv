use std::fs;
use std::process::Command;

#[test]
fn replays_output_creates_files_and_returns_the_exit_code() {
    let dir = std::env::temp_dir().join(format!("lazymkv-fake-{}", std::process::id()));
    let out_dir = dir.join("out");
    fs::create_dir_all(&dir).unwrap();
    let tool = dir.join(if cfg!(windows) { "makemkvcon64.exe" } else { "makemkvcon64" });
    fs::copy(env!("CARGO_BIN_EXE_fake-tool"), &tool).unwrap();

    let recorded = "TCOUNT:1\nMSG:5036,0,1,\"Copy complete. 1 titles saved.\"\n";
    fs::write(dir.join("rip.txt"), recorded).unwrap();
    fs::write(dir.join("sample.mkv"), b"not really an mkv").unwrap();
    fs::write(
        dir.join("makemkvcon64.json"),
        r#"{"responses":[
            {"args_contain":["mkv","disc:0"],"stdout":"rip.txt",
             "creates":[{"dir_arg":4,"name":"title_t00.mkv","copy_from":"sample.mkv"}]},
            {"args_contain":["backup"],"exit_code":7}
        ]}"#,
    )
    .unwrap();

    let run = |args: &[&str]| {
        Command::new(&tool)
            .args(args)
            .env("LAZYMKV_FAKE_DIR", &dir)
            .output()
            .unwrap()
    };

    let rip = run(&["-r", "mkv", "disc:0", "0", out_dir.to_str().unwrap()]);
    assert_eq!(rip.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&rip.stdout), recorded);
    assert_eq!(fs::read(out_dir.join("title_t00.mkv")).unwrap(), b"not really an mkv");

    assert_eq!(run(&["-r", "backup", "disc:0", "x"]).status.code(), Some(7));
    assert_eq!(run(&["-r", "info", "disc:0"]).status.code(), Some(2));
}
