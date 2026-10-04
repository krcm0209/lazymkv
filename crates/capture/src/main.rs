mod disc;
mod eject;
mod record;

use lazymkv_core::profile::make_job_profile;
use std::path::Path;
use std::process::ExitCode;

const USAGE: &str = "usage:
  lazymkv-capture disc --root <path> --label <text> --out <dir>
  lazymkv-capture record --out <file> -- <command> [args...]
  lazymkv-capture profile --default <xml file> --name <text> --selection <text> --out <file>
  lazymkv-capture eject --drive <letter>";

/// The value that follows `name` in `args`.
fn flag<'a>(args: &'a [String], name: &str) -> Result<&'a str, String> {
    args.iter()
        .position(|arg| arg == name)
        .and_then(|index| args.get(index + 1))
        .map(String::as_str)
        .ok_or_else(|| format!("missing {name}\n{USAGE}"))
}

fn run(args: &[String]) -> Result<i32, String> {
    let io_error = |error: std::io::Error| error.to_string();
    match args.first().map(String::as_str) {
        Some("disc") => {
            let meta = disc::snapshot(
                Path::new(flag(args, "--root")?),
                flag(args, "--label")?,
                Path::new(flag(args, "--out")?),
            )
            .map_err(io_error)?;
            println!("{}", serde_json::to_string_pretty(&meta).map_err(|error| error.to_string())?);
            Ok(0)
        }
        Some("record") => {
            let split = args
                .iter()
                .position(|arg| arg == "--")
                .ok_or_else(|| format!("missing --\n{USAGE}"))?;
            record::run_recorded(&args[split + 1..], Path::new(flag(&args[..split], "--out")?))
                .map_err(io_error)
        }
        Some("profile") => {
            let default_xml = std::fs::read_to_string(flag(args, "--default")?).map_err(io_error)?;
            let xml = make_job_profile(&default_xml, flag(args, "--name")?, flag(args, "--selection")?)
                .map_err(|error| format!("{error:?}"))?;
            std::fs::write(flag(args, "--out")?, xml).map_err(io_error)?;
            Ok(0)
        }
        Some("eject") => {
            let letter = flag(args, "--drive")?
                .chars()
                .next()
                .ok_or_else(|| "empty drive letter".to_string())?;
            eject::eject(letter)?;
            Ok(0)
        }
        _ => Err(USAGE.to_string()),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => ExitCode::from(code.clamp(0, 255) as u8),
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
pub(crate) mod testutil {
    use std::path::PathBuf;

    /// A fresh, empty directory under the system temporary folder.
    pub fn temp_dir(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("lazymkv-{tag}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flag_returns_the_value_after_its_name() {
        let args: Vec<String> = ["disc", "--root", "D:\\", "--out", "x"].iter().map(|s| s.to_string()).collect();
        assert_eq!(flag(&args, "--root"), Ok("D:\\"));
        assert_eq!(flag(&args, "--out"), Ok("x"));
        assert!(flag(&args, "--label").unwrap_err().contains("missing --label"));
    }
}
