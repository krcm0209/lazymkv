//! The ffprobe driver (design parts 5.3, 5.7): the streams read of step 9, below normal priority under
//! `limits.header`, step 10's packet listing (below normal priority, no limit) and the argument list of step 10a's
//! proxy check.

use std::io;
use std::path::Path;

use lazymkv_core::ipc::CANCELLED;
use lazymkv_core::recording::files::FfprobeStreams;

use crate::mkvmerge::{code_text, run_header, run_paused};
use crate::record::{LineAction, ToolCtx};

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

/// `["-v","error","-select_streams","s:<stream>","-show_entries","packet=pts_time,duration_time,size","-of","json",
/// <evidence>]`: the packets of subtitle stream `stream` of the evidence file (design part 5.7 step 10). Def: T5-13.
/// Use: T5-14, T5-24.
pub fn packets_args(stream: i64, evidence: &Path) -> Vec<String> {
    ["-v", "error", "-select_streams"]
        .into_iter()
        .map(String::from)
        .chain([format!("s:{stream}")])
        .chain(["-show_entries", "packet=pts_time,duration_time,size", "-of", "json"].into_iter().map(String::from))
        .chain([evidence.display().to_string()])
        .collect()
}

/// The packet listing of `args`, below normal priority, with no time limit (design part 5.3's 120 s covers only the
/// header calls `mkvmerge -J` and `ffprobe -show_streams`), recorded into `out`, which is the listing
/// `subs\sub-<n>.packets.json` names, read later by `core::verify::read_packets_file`. Exit code 0 gives `Ok`; a
/// cancel gives `CANCELLED` and any other exit code `ffprobe exited with code <c>`, as phase 0's `probe-title.ps1`
/// refuses any exit but 0 (design part 5.7 step 10). Def: T5-13. Use: T5-14, T5-24.
pub fn packets(ctx: &ToolCtx, program: &Path, args: &[String], out: &Path) -> io::Result<Result<(), String>> {
    let run = run_paused(ctx, program, args, out, None, &mut |_| LineAction::Continue)?;
    Ok(if run.cancelled {
        Err(CANCELLED.to_string())
    } else if run.exit_code == Some(0) {
        Ok(())
    } else {
        Err(format!("ffprobe exited with code {}", code_text(run.exit_code)))
    })
}

/// Phase 0's proxy check (`scripts/phase0/audio-levels.ps1` line 381) for the audio proxy `proxy`: the stream entries
/// of the file, written as JSON with `-o` to `proxy-check.json`. Both file names are relative: the call runs in the
/// title's work folder through `run_ffmpeg` (design part 5.7 step 10a). Def: T5-13. Use: T5-17.
pub fn proxy_check_args(proxy: &str) -> Vec<String> {
    [
        "-v", "error", "-show_entries",
        "stream=index,codec_type,codec_name,sample_rate,channels,sample_fmt,bits_per_raw_sample:stream_tags",
        "-of", "json", "-o", "proxy-check.json",
    ]
    .into_iter()
    .map(String::from)
    .chain([proxy.to_string()])
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_args_are_v_error_show_streams_show_format_json() {
        assert_eq!(probe_args(std::path::Path::new("raw.mkv")),
            ["-v", "error", "-show_streams", "-show_format", "-of", "json", "raw.mkv"]);
    }

    #[test]
    fn packets_args_select_one_subtitle_stream() {
        assert_eq!(
            packets_args(1, std::path::Path::new("t00.mks")),
            ["-v", "error", "-select_streams", "s:1", "-show_entries", "packet=pts_time,duration_time,size", "-of", "json", "t00.mks"]
        );
    }

    #[test]
    fn proxy_check_args_are_phase_0s_and_write_proxy_check_json() {
        assert_eq!(
            proxy_check_args("audio-proxy.mka.partial"),
            [
                "-v", "error", "-show_entries",
                "stream=index,codec_type,codec_name,sample_rate,channels,sample_fmt,bits_per_raw_sample:stream_tags",
                "-of", "json", "-o", "proxy-check.json", "audio-proxy.mka.partial",
            ]
        );
    }
}
