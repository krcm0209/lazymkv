mod scenario;

use scenario::{parse_timing, schedule, select, Scenario, Stream};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;
use std::{env, fs, thread};

/// The lines of a file in the scenario folder, each with its newline, or none when the response names no file.
fn read_lines(dir: &Path, file: &Option<String>) -> Result<Vec<Vec<u8>>, String> {
    let Some(file) = file else { return Ok(Vec::new()) };
    let bytes = fs::read(dir.join(file)).map_err(|error| format!("{file}: {error}"))?;
    Ok(bytes.split_inclusive(|byte| *byte == b'\n').map(<[u8]>::to_vec).collect())
}

/// The arrival times of a timing file in the scenario folder, or none when the response names no file.
fn read_timing(dir: &Path, file: &Option<String>) -> Result<Vec<u64>, String> {
    let Some(file) = file else { return Ok(Vec::new()) };
    let text = fs::read_to_string(dir.join(file)).map_err(|error| format!("{file}: {error}"))?;
    Ok(parse_timing(&text))
}

fn run() -> Result<i32, String> {
    let exe = env::current_exe().map_err(|error| error.to_string())?;
    let tool = exe
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    // The scenario sits beside the executable unless LAZYMKV_FAKE_DIR names another folder.
    let dir = match env::var_os("LAZYMKV_FAKE_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => exe.parent().ok_or_else(|| format!("{}: has no folder", exe.display()))?.to_path_buf(),
    };
    let speed: f64 = env::var("LAZYMKV_FAKE_SPEED")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0.0);

    let scenario_path = dir.join(format!("{tool}.json"));
    let scenario_text = fs::read_to_string(&scenario_path)
        .map_err(|error| format!("{}: {error}", scenario_path.display()))?;
    let scenario: Scenario = serde_json::from_str(&scenario_text)
        .map_err(|error| format!("{}: {error}", scenario_path.display()))?;

    // The counter file says how many runs each group of matching responses has had.
    let calls_path = dir.join(format!("{tool}.calls.json"));
    let mut calls: BTreeMap<String, u64> = match fs::read(&calls_path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|error| format!("{}: {error}", calls_path.display()))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
        Err(error) => return Err(format!("{}: {error}", calls_path.display())),
    };

    let args: Vec<String> = env::args().skip(1).collect();
    let response = select(&scenario, &args, &mut calls)
        .ok_or_else(|| format!("{tool}: no response matches {args:?}"))?;
    let counted = serde_json::to_vec(&calls).map_err(|error| error.to_string())?;
    fs::write(&calls_path, counted).map_err(|error| format!("{}: {error}", calls_path.display()))?;

    if response.stop_after_line.is_some() && response.hang_after_line.is_some() {
        return Err("a response may not have both stop_after_line and hang_after_line".to_string());
    }

    let out_lines = read_lines(&dir, &response.stdout)?;
    let err_lines = read_lines(&dir, &response.stderr)?;
    let out_ms = read_timing(&dir, &response.timing)?;
    let err_ms = read_timing(&dir, &response.stderr_timing)?;

    let (mut out, mut err) = (std::io::stdout(), std::io::stderr());
    let mut waited_ms = 0u64;
    for (stream, index, t_ms) in schedule(&out_ms, out_lines.len(), &err_ms, err_lines.len()) {
        if stream == Stream::Out
            && (response.stop_after_line == Some(index) || response.hang_after_line == Some(index))
        {
            break;
        }
        if speed > 0.0 {
            let due_ms = (t_ms as f64 * speed) as u64;
            if due_ms > waited_ms {
                thread::sleep(Duration::from_millis(due_ms - waited_ms));
                waited_ms = due_ms;
            }
        }
        let (writer, line): (&mut dyn Write, &[u8]) = match stream {
            Stream::Out => (&mut out, out_lines[index].as_slice()),
            Stream::Err => (&mut err, err_lines[index].as_slice()),
        };
        writer.write_all(line).map_err(|error| error.to_string())?;
        writer.flush().map_err(|error| error.to_string())?;
    }
    // The walk ends at the cut line, or at the last line when the cut is at or above the line count.
    if response.stop_after_line.is_some() {
        return Ok(response.exit_code);
    }
    if response.hang_after_line.is_some() {
        let _ = out.flush();
        let _ = err.flush();
        loop {
            thread::sleep(Duration::from_secs(3600));
        }
    }

    for create in &response.creates {
        let target = create.target(&args)?;
        let parent = target.parent().unwrap_or(Path::new(""));
        if create.arg_after.is_some() {
            // Like mkvmerge -o: the output's folder is the caller's to make.
            if !parent.as_os_str().is_empty() && !parent.exists() {
                return Err(format!("creates: {} does not exist", parent.display()));
            }
        } else {
            fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
        }
        fs::copy(dir.join(&create.copy_from), &target)
            .map_err(|error| format!("{}: {error}", create.copy_from))?;
    }

    Ok(response.exit_code)
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
    }
}
