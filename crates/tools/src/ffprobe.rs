//! The ffprobe driver (design parts 5.3, 5.7): the streams read of step 9, below normal priority under
//! `limits.header`.

use std::io;
use std::path::Path;

use lazymkv_core::recording::files::FfprobeStreams;

use crate::mkvmerge::{code_text, run_header};
use crate::record::ToolCtx;

/// `["-v","error","-show_streams","-show_format","-of","json",<raw>]` (design part 5.7 step 9). Def: T14. Use: T14,
/// T24.
pub fn probe_args(raw: &Path) -> Vec<String> {
    ["-v", "error", "-show_streams", "-show_format", "-of", "json"]
        .into_iter()
        .map(String::from)
        .chain([raw.display().to_string()])
        .collect()
}

/// `ffprobe` under `limits.header`, below normal priority, recorded into `out`. Exit code 0 parses the standard
/// output; a cancel, the time limit or any other exit code gives the `Err` text (design parts 5.3, 5.7 step 9).
/// Def: T14. Use: T24.
pub fn probe(ctx: &ToolCtx, program: &Path, raw: &Path, out: &Path) -> io::Result<Result<FfprobeStreams, String>> {
    let label = "ffprobe";
    let run = match run_header(ctx, program, &probe_args(raw), out, label)? {
        Ok(run) => run,
        Err(message) => return Ok(Err(message)),
    };
    Ok(match run.exit_code {
        Some(0) => serde_json::from_slice(&run.stdout).map_err(|error| format!("{}: {error}", out.display())),
        code => Err(format!("{label} exited with code {}", code_text(code))),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_args_are_v_error_show_streams_show_format_json() {
        assert_eq!(probe_args(std::path::Path::new("raw.mkv")),
            ["-v", "error", "-show_streams", "-show_format", "-of", "json", "raw.mkv"]);
    }
}
