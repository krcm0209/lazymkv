//! The ffmpeg driver (design parts 5.3, 5.7 step 10a; R5-code-27): the argument lists of step 10a's two passes and the one runner every FFmpeg-suite call of step 10a goes through, below normal priority under `ProcessGuard`, recorded capture-style (design part 7.2), in the title's work folder when given one (T5-R37, T5-R39).

use std::io;
use std::path::Path;

use lazymkv_core::ipc::CANCELLED;
use lazymkv_platform::Spawn;

use crate::mkvmerge::code_text;
use crate::record::{run_recorded_in, LineAction, RunResult, ToolCtx};

/// `["-hide_banner","-layouts"]`: ffmpeg's table of channel layouts, which phase 0's `audio-levels.ps1` read (line 122)
/// before it named a layout in a filter (design part 5.7 step 10a). Def: T5-13. Use: T5-17, T5-24.
pub fn layouts_args() -> Vec<String> {
    ["-hide_banner", "-layouts"].into_iter().map(String::from).collect()
}

/// `["-i",<raw>,"-map","0:a","-c","copy","-f","streamhash","-hash","md5",<out>]`: the setting-off pass of design part
/// 5.7 step 10a word for word (R6-phase0-54): every audio stream copied, hashed with MD5 and written to `out`.
/// Def: T5-13. Use: T5-17, T5-24.
pub fn streamhash_args(raw: &Path, out: &str) -> Vec<String> {
    ["-i"]
        .into_iter()
        .map(String::from)
        .chain([raw.display().to_string()])
        .chain(["-map", "0:a", "-c", "copy", "-f", "streamhash", "-hash", "md5"].into_iter().map(String::from))
        .chain([out.to_string()])
        .collect()
}

/// Runs `program`, any program of the FFmpeg suite, with `args` below normal priority (R1-evidence-5: the tool
/// lookup's program under `ProcessGuard`, design part 5.3), with no time limit, no extra environment variable and
/// `cwd` as its working folder when given, recorded into `out` as design part 7.2 records a tool (`<out>`, `.stderr`,
/// `.timing.jsonl`, `.exit.json`). A kill, a cancel or an error of the recorder pauses `limits.after_kill` before the
/// return (S56). Exit code 0 gives the run, whose `stdout` the caller reads (`-layouts` needs it); a cancel gives
/// `CANCELLED` and any other end `<program stem> exited with code <c>` (`none` for a run without an exit code).
/// Task 17 runs step 10a's ffmpeg passes, `-layouts` and its ffprobe proxy check through it. Def: T5-13. Use: T5-17.
pub fn run_ffmpeg(
    ctx: &ToolCtx,
    program: &Path,
    args: &[String],
    out: &Path,
    cwd: Option<&Path>,
) -> io::Result<Result<RunResult, String>> {
    let spawn = Spawn {
        program: program.to_path_buf(),
        args: args.to_vec(),
        env: Vec::new(),
        below_normal: true,
        cwd: cwd.map(Path::to_path_buf),
    };
    let ran = run_recorded_in(ctx, &spawn, out, None, &mut |_| LineAction::Continue);
    if ran.as_ref().map_or(true, |run| run.timed_out || run.cancelled) {
        std::thread::sleep(ctx.limits.after_kill);
    }
    let run = ran?;
    Ok(if run.cancelled {
        Err(CANCELLED.to_string())
    } else if run.exit_code == Some(0) {
        Ok(run)
    } else {
        let stem = program.file_stem().map(|stem| stem.to_string_lossy().into_owned()).unwrap_or_default();
        Err(format!("{stem} exited with code {}", code_text(run.exit_code)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn layouts_args_are_hide_banner_and_layouts() {
        assert_eq!(layouts_args(), ["-hide_banner", "-layouts"]);
    }

    #[test]
    fn streamhash_args_copy_every_audio_stream() {
        assert_eq!(
            streamhash_args(Path::new("raw.mkv"), "streamhash.txt"),
            ["-i", "raw.mkv", "-map", "0:a", "-c", "copy", "-f", "streamhash", "-hash", "md5", "streamhash.txt"]
        );
    }
}
