//! The mkvmerge driver (design parts 5.3, 5.7, 5.11): the header read, the remux's arguments and the remux itself.
//! Both calls run below normal priority and record their run with `run_recorded`. A call that is killed pauses
//! `limits.after_kill` before it returns (S56).

use std::io;
use std::path::Path;
use std::time::Duration;

use lazymkv_core::disc::StreamKind;
use lazymkv_core::ipc::CANCELLED;
use lazymkv_core::recording::files::MkvmergeIdent;
use lazymkv_core::tracks::layout::Layout;
use lazymkv_core::verify::VerifyReport;

use crate::record::{run_recorded, LineAction, RunResult, ToolCtx};

/// `["-J", <file>]` (design part 5.6 step 7, 5.7 step 14). Def: T14. Use: T14, T23, T24.
pub fn ident_args(file: &Path) -> Vec<String> {
    vec!["-J".to_string(), file.display().to_string()]
}

/// `mkvmerge -J` under `limits.header`, below normal priority, recorded into `out`. Exit code 0 or 1 parses the
/// standard output (PIPE-14); a cancel, the time limit or any other exit code gives the `Err` text (design part 5.3).
/// Def: T14. Use: T23, T24.
pub fn ident(ctx: &ToolCtx, program: &Path, file: &Path, out: &Path) -> io::Result<Result<MkvmergeIdent, String>> {
    let label = "mkvmerge -J";
    let run = match run_header(ctx, program, &ident_args(file), out, label)? {
        Ok(run) => run,
        Err(message) => return Ok(Err(message)),
    };
    Ok(match run.exit_code {
        Some(0 | 1) => serde_json::from_slice(&run.stdout).map_err(|error| format!("{}: {error}", out.display())),
        code => Err(format!("{label} exited with code {}", code_text(code))),
    })
}

/// The arguments of the remux of one staged title (design part 5.7 step 13; spec 8.10): `--gui-mode -o <partial>`,
/// then for each layout track, in the layout's order, its language, name and four flags, then the audio and subtitle
/// track lists, `--no-chapters` when the layout keeps none, the track order and `raw`. `staged` maps each layout
/// stream to the raw file's track ID (S60); it gives `Err` when its tracks did not match the scan title or a layout
/// stream has no track. A name or language the layout lacks is written empty or `und`, so that the options clear what
/// MakeMKV wrote. No `--video-tracks` is given, so every raw video track is kept. Def: T14. Use: T24.
pub fn remux_args(partial: &Path, layout: &Layout, staged: &VerifyReport, raw: &Path) -> Result<Vec<String>, String> {
    if !staged.tracks_ok {
        return Err("the staged file's tracks did not match its scan title, so no track ID is known".to_string());
    }
    let yes_no = |flag: bool| if flag { "yes" } else { "no" };
    let mut args = vec!["--gui-mode".to_string(), "-o".to_string(), partial.display().to_string()];
    let (mut audio, mut subtitles, mut order) = (Vec::new(), Vec::new(), Vec::new());
    for track in &layout.tracks {
        let id = staged
            .pairs
            .iter()
            .find(|(stream, _)| *stream == track.stream)
            .and_then(|(_, id)| *id)
            .ok_or_else(|| format!("stream {} has no track in the staged file", track.stream))?;
        let options = [
            ("--language", track.language.as_deref().unwrap_or("und")),
            ("--track-name", track.name.as_deref().unwrap_or("")),
            ("--default-track-flag", yes_no(track.default)),
            ("--forced-display-flag", yes_no(track.forced)),
            ("--hearing-impaired-flag", yes_no(track.hearing_impaired)),
            ("--original-flag", yes_no(track.original)),
        ];
        for (flag, value) in options {
            args.push(flag.to_string());
            args.push(format!("{id}:{value}"));
        }
        match track.kind {
            StreamKind::Video => {}
            StreamKind::Audio => audio.push(id.to_string()),
            StreamKind::Subtitles => subtitles.push(id.to_string()),
        }
        order.push(format!("0:{id}"));
    }
    for (list_flag, none_flag, ids) in [
        ("--audio-tracks", "--no-audio", audio),
        ("--subtitle-tracks", "--no-subtitles", subtitles),
    ] {
        if ids.is_empty() {
            args.push(none_flag.to_string());
        } else {
            args.push(list_flag.to_string());
            args.push(ids.join(","));
        }
    }
    if !layout.chapters_kept {
        args.push("--no-chapters".to_string());
    }
    args.push("--track-order".to_string());
    args.push(order.join(","));
    args.push(raw.display().to_string());
    Ok(args)
}

/// Reads `#GUI#progress 45%` as 0.45: a whole percentage from 0 to 100, and nothing else (design part 5.11).
/// Def: T14. Use: T14, T24.
pub fn gui_progress(line: &str) -> Option<f64> {
    let digits = line.strip_prefix("#GUI#progress ")?.strip_suffix('%')?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let percent: u32 = digits.parse().ok()?;
    (percent <= 100).then(|| f64::from(percent) / 100.0)
}

/// The remux, below normal priority, with no time limit, recorded into `out`. Each `#GUI#progress` line goes to
/// `ctx.on_progress`. Exit code 0 or 1 gives `Ok`, because verification of the result decides (PIPE-14; design parts
/// 5.3, 5.11); a cancel or any other exit code gives the `Err` text. Def: T14. Use: T24.
pub fn remux(ctx: &ToolCtx, program: &Path, args: &[String], out: &Path) -> io::Result<Result<(), String>> {
    let run = run_paused(ctx, program, args, out, None, &mut |line| {
        if let Some(fraction) = gui_progress(line) {
            (ctx.on_progress)(fraction);
        }
        LineAction::Continue
    })?;
    Ok(if run.cancelled {
        Err(CANCELLED.to_string())
    } else if matches!(run.exit_code, Some(0 | 1)) {
        Ok(())
    } else {
        Err(format!("mkvmerge exited with code {}", code_text(run.exit_code)))
    })
}

/// `exit_code` as text, or `none` when the process ended without one.
pub(crate) fn code_text(exit_code: Option<i32>) -> String {
    exit_code.map_or_else(|| "none".to_string(), |code| code.to_string())
}

/// `run_recorded` below normal priority, then the pause of S56 after any kill. It pauses too when the recorder gives
/// `Err`, because the recorder kills the tool on its error paths without saying so.
fn run_paused(
    ctx: &ToolCtx,
    program: &Path,
    args: &[String],
    out: &Path,
    limit: Option<Duration>,
    on_line: &mut dyn FnMut(&str) -> LineAction,
) -> io::Result<RunResult> {
    let ran = run_recorded(ctx, program, args, out, limit, true, on_line);
    if ran.as_ref().map_or(true, |run| run.timed_out || run.cancelled) {
        std::thread::sleep(ctx.limits.after_kill);
    }
    ran
}

/// A header call of `label` under `limits.header` (design part 5.3): the finished run, or the `Err` text when the
/// owner cancelled it or it passed the limit and was killed. Def: T14. Use: T14.
pub(crate) fn run_header(
    ctx: &ToolCtx,
    program: &Path,
    args: &[String],
    out: &Path,
    label: &str,
) -> io::Result<Result<RunResult, String>> {
    let run = run_paused(ctx, program, args, out, Some(ctx.limits.header), &mut |_| LineAction::Continue)?;
    Ok(if run.cancelled {
        Err(CANCELLED.to_string())
    } else if run.timed_out {
        Err(format!("{label} did not finish within {} s and was killed", ctx.limits.header.as_secs()))
    } else {
        Ok(run)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lazymkv_core::disc::StreamKind;
    use lazymkv_core::tracks::layout::{Layout, OutputTrack};
    use lazymkv_core::verify::VerifyReport;
    use std::path::Path;

    fn track(stream: u32, kind: StreamKind, language: Option<&str>, name: Option<&str>, flags: [bool; 4]) -> OutputTrack {
        OutputTrack {
            stream, kind, language: language.map(str::to_string), name: name.map(str::to_string),
            default: flags[0], forced: flags[1], hearing_impaired: flags[2], original: flags[3],
        }
    }

    fn staged(pairs: Vec<(u32, Option<i64>)>, tracks_ok: bool) -> VerifyReport {
        VerifyReport {
            duration_ok: true, expected_seconds: 7150.0, file_seconds: 7150.0, chapters_ok: true, scan_chapters: 12,
            file_chapters: 12, tracks_ok, pairs, extra_tracks: vec![], unknown_removed: vec![], mismatches: vec![],
            unchecked: vec![], read_errors: vec![], ok: tracks_ok,
        }
    }

    fn layout(tracks: Vec<OutputTrack>) -> Layout {
        Layout { tracks, chapters_kept: true }
    }

    #[test]
    fn ident_args_are_dash_j_and_the_file() {
        assert_eq!(ident_args(Path::new("t.mkv")), ["-J", "t.mkv"]);
    }

    #[test]
    fn remux_args_start_with_gui_mode_and_the_partial_path() {
        let partial = Path::new("lib").join(".lazymkv-partial").join("j-1-t00-00800.mkv.partial");
        let args = remux_args(&partial, &layout(vec![track(0, StreamKind::Video, None, None, [true, false, false, false])]),
            &staged(vec![(0, Some(0))], true), Path::new("raw.mkv")).unwrap();
        assert_eq!(&args[..3], ["--gui-mode".to_string(), "-o".to_string(), partial.display().to_string()]);
        assert_eq!(args.last().unwrap(), "raw.mkv");
    }

    #[test]
    fn remux_args_write_every_flag_of_each_layout_track() {
        let tracks = vec![
            track(0, StreamKind::Video, None, None, [true, false, false, false]),
            track(1, StreamKind::Audio, Some("eng"), Some("DTS-HD MA 5.1"), [true, false, false, true]),
            track(3, StreamKind::Subtitles, Some("eng"), Some("English (Forced)"), [false, true, false, false]),
        ];
        let report = staged(vec![(0, Some(0)), (1, Some(1)), (2, Some(2)), (3, Some(4))], true);
        let args = remux_args(Path::new("p.partial"), &layout(tracks), &report, Path::new("raw.mkv")).unwrap();
        assert_eq!(args, [
            "--gui-mode", "-o", "p.partial",
            "--language", "0:und", "--track-name", "0:", "--default-track-flag", "0:yes", "--forced-display-flag", "0:no",
            "--hearing-impaired-flag", "0:no", "--original-flag", "0:no",
            "--language", "1:eng", "--track-name", "1:DTS-HD MA 5.1", "--default-track-flag", "1:yes",
            "--forced-display-flag", "1:no", "--hearing-impaired-flag", "1:no", "--original-flag", "1:yes",
            "--language", "4:eng", "--track-name", "4:English (Forced)", "--default-track-flag", "4:no",
            "--forced-display-flag", "4:yes", "--hearing-impaired-flag", "4:no", "--original-flag", "4:no",
            "--audio-tracks", "1", "--subtitle-tracks", "4", "--track-order", "0:0,0:1,0:4", "raw.mkv",
        ]);
    }

    #[test]
    fn a_title_without_subtitles_gets_no_subtitles() {
        let tracks = vec![
            track(0, StreamKind::Video, None, None, [true, false, false, false]),
            track(1, StreamKind::Audio, Some("eng"), Some("AC-3 5.1"), [true, false, false, true]),
        ];
        let args = remux_args(Path::new("p"), &layout(tracks), &staged(vec![(0, Some(0)), (1, Some(1))], true),
            Path::new("raw.mkv")).unwrap();
        assert!(args.windows(1).any(|w| w[0] == "--no-subtitles"));
        assert!(!args.iter().any(|arg| arg == "--subtitle-tracks"));
    }

    #[test]
    fn a_layout_stream_without_a_staged_track_is_refused() {
        let tracks = vec![track(2, StreamKind::Audio, Some("eng"), Some("AC-3 5.1"), [true, false, false, false])];
        assert_eq!(
            remux_args(Path::new("p"), &layout(tracks.clone()), &staged(vec![(2, None)], true), Path::new("raw.mkv")),
            Err("stream 2 has no track in the staged file".to_string())
        );
        assert_eq!(
            remux_args(Path::new("p"), &layout(tracks), &staged(vec![(2, Some(2))], false), Path::new("raw.mkv")),
            Err("the staged file's tracks did not match its scan title, so no track ID is known".to_string())
        );
    }

    #[test]
    fn gui_progress_reads_a_percentage() {
        assert_eq!(gui_progress("#GUI#progress 45%"), Some(0.45));
        assert_eq!(gui_progress("#GUI#progress 100%"), Some(1.0));
        assert_eq!(gui_progress("#GUI#progress 101%"), None);
        assert_eq!(gui_progress("#GUI#warning something"), None);
        assert_eq!(gui_progress("Progress: 45%"), None);
        assert_eq!(gui_progress("#GUI#progress +45%"), None);
    }
}
