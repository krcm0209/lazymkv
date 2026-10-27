use lazymkv_core::testsupport::{Scratch, ScratchExt};
// Every direct run of a freshly copied stand-in goes through `retry_busy` (Rulings R29.c3, R29.c4).
use lazymkv_engine::testkit::retry_busy;
use std::fs;
use std::io::{self, BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Duration;

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
        retry_busy(|| Command::new(&tool).args(args).env("LAZYMKV_FAKE_DIR", &dir).output()).unwrap()
    };

    let rip = run(&["-r", "mkv", "disc:0", "0", out_dir.to_str().unwrap()]);
    assert_eq!(rip.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&rip.stdout), recorded);
    assert_eq!(fs::read(out_dir.join("title_t00.mkv")).unwrap(), b"not really an mkv");

    assert_eq!(run(&["-r", "backup", "disc:0", "x"]).status.code(), Some(7));
    assert_eq!(run(&["-r", "info", "disc:0"]).status.code(), Some(2));
}

#[test]
fn a_spawn_refused_as_busy_is_retried_and_the_error_kept_after_twenty_tries() {
    let busy = || io::Error::from(io::ErrorKind::ExecutableFileBusy);
    let mut calls = 0;
    let answered = retry_busy(|| {
        calls += 1;
        if calls < 3 { Err(busy()) } else { Ok(calls) }
    });
    assert_eq!(answered.unwrap(), 3);
    let mut calls = 0;
    let refused: io::Result<()> = retry_busy(|| {
        calls += 1;
        Err(busy())
    });
    assert_eq!((refused.unwrap_err().kind(), calls), (io::ErrorKind::ExecutableFileBusy, 20));
    let mut calls = 0;
    let missing: io::Result<()> = retry_busy(|| {
        calls += 1;
        Err(io::Error::from(io::ErrorKind::NotFound))
    });
    assert_eq!((missing.unwrap_err().kind(), calls), (io::ErrorKind::NotFound, 1));
}

const SAVED: &str = "MSG:5036,0,1,\"Copy complete. 1 titles saved.\"\n";

fn stand_in(dir: &Path, stem: &str) -> PathBuf {
    let tool = dir.join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
    fs::copy(env!("CARGO_BIN_EXE_fake-tool"), &tool).unwrap();
    tool
}

fn run_in(tool: &Path, fake_dir: Option<&Path>, args: &[&str]) -> Output {
    let mut command = Command::new(tool);
    command.args(args);
    match fake_dir {
        Some(dir) => command.env("LAZYMKV_FAKE_DIR", dir),
        None => command.env_remove("LAZYMKV_FAKE_DIR"),
    };
    retry_busy(|| command.output()).unwrap()
}

const REMUX: &str = r#"{"responses":[{"args_contain":["--gui-mode","-o"],"stdout":"remux.txt",
    "creates":[{"arg_after":"-o","copy_from":"placeholder.mkv"}]}]}"#;

#[test]
fn the_file_after_dash_o_is_created() {
    let dir = Scratch::new("fake-o");
    let tool = stand_in(&dir.0, "mkvmerge");
    dir.write("mkvmerge.json", REMUX);
    dir.write("remux.txt", "#GUI#progress 45%\n#GUI#progress 100%\n");
    dir.write("placeholder.mkv", b"0123456789abcdef");
    let partial = dir.0.join("lib/.lazymkv-partial/abc-1-t00-00800.mkv.partial");
    fs::create_dir_all(partial.parent().unwrap()).unwrap();

    let remux = run_in(&tool, None, &["--gui-mode", "-o", partial.to_str().unwrap(), "--language", "1:eng", "raw.mkv"]);

    assert_eq!(remux.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&remux.stdout), "#GUI#progress 45%\n#GUI#progress 100%\n");
    assert_eq!(fs::read(&partial).unwrap(), b"0123456789abcdef");
}

#[test]
fn the_file_after_dash_o_needs_its_folder_to_exist() {
    let dir = Scratch::new("fake-o-missing");
    let tool = stand_in(&dir.0, "mkvmerge");
    dir.write("mkvmerge.json", REMUX);
    dir.write("remux.txt", "#GUI#progress 100%\n");
    dir.write("placeholder.mkv", b"0123456789abcdef");
    let partial = dir.0.join("lib/.lazymkv-partial/abc-1-t00-00800.mkv.partial");

    let remux = run_in(&tool, None, &["--gui-mode", "-o", partial.to_str().unwrap(), "raw.mkv"]);

    assert_eq!(remux.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&remux.stderr).contains("does not exist"));
    assert!(!partial.parent().unwrap().exists());
}

#[test]
fn a_directory_counted_from_the_end_survives_an_added_flag() {
    let dir = Scratch::new("fake-end");
    let tool = stand_in(&dir.0, "makemkvcon64");
    dir.write(
        "makemkvcon64.json",
        r#"{"responses":[{"args_contain":["mkv","disc:0"],"stdout":"rip.txt",
            "creates":[{"dir_arg_from_end":1,"name":"title_t00.mkv","copy_from":"sample.mkv"}]}]}"#,
    );
    dir.write("rip.txt", SAVED);
    dir.write("sample.mkv", b"sample");
    let (plain, debug) = (dir.0.join("plain"), dir.0.join("debug"));
    let rip = |extra: &[&str], out: &Path| {
        let mut args = vec!["-r", "--messages=-stdout", "--progress=-same"];
        args.extend_from_slice(extra);
        args.extend(["mkv", "disc:0", "0", out.to_str().unwrap()]);
        run_in(&tool, None, &args)
    };

    assert_eq!(rip(&[], &plain).status.code(), Some(0));
    assert_eq!(rip(&["--debug"], &debug).status.code(), Some(0));
    assert_eq!(fs::read(plain.join("title_t00.mkv")).unwrap(), b"sample");
    assert_eq!(fs::read(debug.join("title_t00.mkv")).unwrap(), b"sample");
}

#[test]
fn stderr_is_replayed_from_its_file() {
    let dir = Scratch::new("fake-stderr");
    let tool = stand_in(&dir.0, "mkvmerge");
    dir.write(
        "mkvmerge.json",
        r#"{"responses":[{"args_contain":["-J"],"stdout":"ident.json","stderr":"warn.txt",
            "stderr_timing":"warn.txt.timing.jsonl","exit_code":1}]}"#,
    );
    dir.write("ident.json", "{\"tracks\":[]}\n");
    dir.write("warn.txt", "Warning: x\nWarning: y\n");
    dir.write("warn.txt.timing.jsonl", "{\"n\":0,\"t_ms\":5}\n{\"n\":1,\"t_ms\":9}\n");

    let ident = run_in(&tool, None, &["-J", "a.mkv"]);

    assert_eq!(ident.status.code(), Some(1));
    assert_eq!(ident.stdout, b"{\"tracks\":[]}\n");
    assert_eq!(ident.stderr, b"Warning: x\nWarning: y\n");
}

#[test]
fn the_same_arguments_get_the_next_response_by_a_counter_in_the_scenario_folder() {
    let dir = Scratch::new("fake-count");
    let tool = stand_in(&dir.0, "mkvmerge");
    dir.write("mkvmerge.json", r#"{"responses":[{"args_contain":["-J"],"exit_code":1},{"args_contain":["-J"],"exit_code":0}]}"#);

    let codes: Vec<_> = (0..3).map(|_| run_in(&tool, None, &["-J", "title_t00.mkv"]).status.code()).collect();

    assert_eq!(codes, [Some(1), Some(0), Some(0)]);
    let calls: serde_json::Value = serde_json::from_slice(&fs::read(dir.0.join("mkvmerge.calls.json")).unwrap()).unwrap();
    assert_eq!(calls, serde_json::json!({"0": 3}));
}

#[test]
fn a_hanging_run_sleeps_until_killed() {
    let dir = Scratch::new("fake-hang");
    let tool = stand_in(&dir.0, "makemkvcon64");
    dir.write("makemkvcon64.json", r#"{"responses":[{"args_contain":["mkv"],"stdout":"rip.txt","hang_after_line":1}]}"#);
    dir.write("rip.txt", format!("MSG:5015,0,0,\"Saving 1 titles\"\n{SAVED}"));
    let mut child = retry_busy(|| {
        Command::new(&tool)
            .args(["-r", "mkv", "disc:0", "0", "out"])
            .env_remove("LAZYMKV_FAKE_DIR")
            .stdout(Stdio::piped())
            .spawn()
    })
    .unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    // The blocking read waits for the first line however slowly a freshly copied executable starts.
    let mut first = String::new();
    stdout.read_line(&mut first).unwrap();
    assert_eq!(first, "MSG:5015,0,0,\"Saving 1 titles\"\n");
    std::thread::sleep(Duration::from_millis(150));
    assert!(child.try_wait().unwrap().is_none(), "the stand-in exited before it was killed");
    child.kill().unwrap();
    child.wait().unwrap();

    let mut rest = String::new();
    stdout.read_to_string(&mut rest).unwrap();
    assert_eq!(rest, "");
}

#[test]
fn output_stops_at_the_given_line_and_exits_with_the_given_code() {
    let dir = Scratch::new("fake-stop");
    let tool = stand_in(&dir.0, "makemkvcon64");
    dir.write(
        "makemkvcon64.json",
        r#"{"responses":[{"args_contain":["mkv"],"stdout":"rip.txt","stop_after_line":2,"exit_code":4,
            "creates":[{"dir_arg_from_end":1,"name":"title_t00.mkv","copy_from":"sample.mkv"}]}]}"#,
    );
    dir.write("rip.txt", format!("MSG:5015,0,0,\"Saving 1 titles\"\nPRGV:10,32768,65536\n{SAVED}"));
    dir.write("sample.mkv", b"sample");
    let out = dir.0.join("out");

    let rip = run_in(&tool, None, &["-r", "mkv", "disc:0", "0", out.to_str().unwrap()]);

    assert_eq!(rip.status.code(), Some(4));
    assert_eq!(String::from_utf8_lossy(&rip.stdout), "MSG:5015,0,0,\"Saving 1 titles\"\nPRGV:10,32768,65536\n");
    assert!(!out.join("title_t00.mkv").exists());
}

#[test]
fn without_lazymkv_fake_dir_the_scenario_beside_the_executable_is_used() {
    let (dir, other) = (Scratch::new("fake-beside"), Scratch::new("fake-env"));
    let tool = stand_in(&dir.0, "makemkvcon64");
    dir.write("makemkvcon64.json", r#"{"responses":[{"args_contain":["info"],"exit_code":3}]}"#);
    other.write("makemkvcon64.json", r#"{"responses":[{"args_contain":["info"],"exit_code":5}]}"#);

    assert_eq!(run_in(&tool, None, &["-r", "--cache=1", "info", "disc:9999"]).status.code(), Some(3));
    assert_eq!(run_in(&tool, Some(&other.0), &["-r", "--cache=1", "info", "disc:9999"]).status.code(), Some(5));
}

#[test]
fn the_ffmpeg_stem_selects_ffmpeg_json() {
    let dir = Scratch::new("fake-ffmpeg");
    let tool = stand_in(&dir.0, "ffmpeg");
    dir.write("ffmpeg.json", r#"{"responses":[{"args_contain":["-f","streamhash"],"stdout":"hash.txt"}]}"#);
    dir.write("makemkvcon64.json", r#"{"responses":[{"args_contain":["-f"],"exit_code":9}]}"#);
    dir.write("hash.txt", "0,a,MD5=00\n");

    let hashed = run_in(&tool, None, &["-i", "raw.mkv", "-map", "0:1", "-f", "streamhash", "-hash", "md5", "-"]);

    assert_eq!(hashed.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&hashed.stdout), "0,a,MD5=00\n");
}
