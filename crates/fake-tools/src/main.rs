mod scenario;

use scenario::{parse_timing, select, Scenario};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;
use std::{env, fs, thread};

fn run() -> Result<i32, String> {
    let exe = env::current_exe().map_err(|error| error.to_string())?;
    let tool = exe
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let dir = PathBuf::from(
        env::var("LAZYMKV_FAKE_DIR").map_err(|_| "LAZYMKV_FAKE_DIR is not set".to_string())?,
    );
    let speed: f64 = env::var("LAZYMKV_FAKE_SPEED")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0.0);

    let scenario_path = dir.join(format!("{tool}.json"));
    let scenario_text = fs::read_to_string(&scenario_path)
        .map_err(|error| format!("{}: {error}", scenario_path.display()))?;
    let scenario: Scenario = serde_json::from_str(&scenario_text)
        .map_err(|error| format!("{}: {error}", scenario_path.display()))?;

    let args: Vec<String> = env::args().skip(1).collect();
    let response =
        select(&scenario, &args).ok_or_else(|| format!("{tool}: no response matches {args:?}"))?;

    if let Some(stdout_file) = &response.stdout {
        let text = fs::read(dir.join(stdout_file)).map_err(|error| format!("{stdout_file}: {error}"))?;
        let timing = match &response.timing {
            Some(timing_file) => parse_timing(
                &fs::read_to_string(dir.join(timing_file))
                    .map_err(|error| format!("{timing_file}: {error}"))?,
            ),
            None => Vec::new(),
        };
        let mut out = std::io::stdout();
        let mut waited_ms = 0u64;
        for (index, line) in text.split_inclusive(|byte| *byte == b'\n').enumerate() {
            if speed > 0.0 {
                if let Some(t_ms) = timing.get(index) {
                    let due_ms = (*t_ms as f64 * speed) as u64;
                    if due_ms > waited_ms {
                        thread::sleep(Duration::from_millis(due_ms - waited_ms));
                        waited_ms = due_ms;
                    }
                }
            }
            out.write_all(line).map_err(|error| error.to_string())?;
            out.flush().map_err(|error| error.to_string())?;
        }
    }

    for create in &response.creates {
        let target_dir = args
            .get(create.dir_arg)
            .ok_or_else(|| format!("creates: there is no argument {}", create.dir_arg))?;
        fs::create_dir_all(target_dir).map_err(|error| format!("{target_dir}: {error}"))?;
        fs::copy(dir.join(&create.copy_from), Path::new(target_dir).join(&create.name))
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
