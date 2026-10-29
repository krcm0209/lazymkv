//! `scripts/engine-task.ps1`, the desktop task's script (P5; design parts 3.1, 3.5; Task T3-24), read and parsed here,
//! never run: running it registers a task, which needs the owner's yes (Owner decision 3). On Windows,
//! `scripts/phase0/platform-ignored.ps1`, the desktop-session run of the platform's ignored tests (design part 12.4;
//! Task T3-25), is read and parsed here too, never run.

use std::fs;
#[cfg(windows)]
use std::path::Path;
use std::path::PathBuf;

use lazymkv_platform::security::folder_sddl;

fn script() -> (PathBuf, String) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("scripts").join("engine-task.ps1");
    let text = fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    (path, text)
}

/// The errors Windows PowerShell 5.1's parser finds in the script at `path`, one per line; empty when it parses
/// (Ruling PF3-S25-26-6). PowerShell prints each error and exits with their count.
#[cfg(windows)]
fn parse_errors(path: &Path) -> String {
    let check = format!(
        "$t = $null; $e = $null; [void][System.Management.Automation.Language.Parser]::ParseFile('{}', [ref]$t, [ref]$e); \
         $e | ForEach-Object {{ $_.ToString() }}; exit $e.Count",
        path.display()
    );
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &check])
        .output()
        .expect("powershell starts");
    let errors = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if output.status.success() || !errors.is_empty() {
        return errors;
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    format!("powershell {} and printed no parse error: {}", output.status, stderr.trim())
}

#[test]
fn the_task_script_names_the_task_the_run_folder_and_no_trigger() {
    let (_, text) = script();
    for needle in [
        "$name = 'lazymkv-engine'",
        r"$run = 'C:\lazymkv\run'",
        r"$release = 'C:\lazymkv\target\release'",
        "-LogonType Interactive -RunLevel Highest",
        "-ExecutionTimeLimit ([TimeSpan]::Zero)",
        "-WorkingDirectory $run",
        "ENGINE-TASK-DONE status=",
    ] {
        assert!(text.contains(needle), "the script lacks {needle}");
    }
    for absent in ["New-ScheduledTaskTrigger", "-Trigger"] {
        assert!(!text.contains(absent), "the script holds {absent}");
    }
    // Ruling T3-24-c: the run folder's parent is checked with the build trees, since whoever can write it can rename
    // the run folder away and put its own in its place.
    assert!(text.contains("$trees = (Split-Path $run), "), "the script does not check the run folder's parent");
}

/// Design part 3.5: the run folder gets the data folder's protected list, one string with `protect_data_folder`'s.
#[test]
fn the_run_folders_list_is_the_data_folders() {
    let (_, text) = script();
    assert!(text.contains(&folder_sddl("$sid")), "the script's list is not folder_sddl's");
}

#[cfg(windows)]
#[test]
fn the_task_script_parses_without_errors() {
    let (path, _) = script();
    let errors = parse_errors(&path);
    assert!(errors.is_empty(), "{errors}");
}

/// Design part 12.4, Task T3-25: the script that runs one module's platform tests, its ignored ones included, in the
/// owner's desktop session through `job.sh`. A parse error there would show only after a wait and a disc cue (Ruling
/// T3-25-h).
#[cfg(windows)]
#[test]
fn the_platform_ignored_script_parses_and_runs_one_session_of_platform_tests() {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join(r"..\..\scripts\phase0\platform-ignored.ps1");
    let text = fs::read_to_string(&script).unwrap_or_else(|error| panic!("{}: {error}", script.display()));
    assert!(text.is_ascii(), "Windows PowerShell 5.1 reads a file without a byte order mark in the system code page");
    let errors = parse_errors(&script);
    assert!(errors.is_empty(), "{errors}");
    for line in [
        "[Parameter(Mandatory)][string]$Filter",
        r#"$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path""#,
        r"$env:CARGO_TARGET_DIR = 'C:\lazymkv\target'",
        "$env:LAZYMKV_TEST_DRIVE = $Drive",
        "$env:LAZYMKV_TEST_SHARE_ROOT = $ShareRoot",
        "$env:LAZYMKV_TEST_VAULT = if ($Filter -like 'win::creds::*') { '1' } else { '' }",
        r"Set-Location 'C:\lazymkv\src'",
        r#"cmd /c "cargo test -p lazymkv-platform --locked -- --include-ignored --test-threads=1 --nocapture $Filter 2>&1""#,
    ] {
        assert!(text.contains(line), "platform-ignored.ps1 lacks: {line}");
    }
    assert!(!text.contains("ScheduledTask"), "the script registers nothing; run-interactive.ps1 does");
}
