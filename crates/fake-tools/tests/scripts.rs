//! `scripts/engine-task.ps1`, the desktop task's script (P5; design parts 3.1, 3.5; Task T3-24), read and parsed here;
//! on Windows its functions are called alone from its syntax tree; the script itself never runs, since running it
//! registers a task, which needs the owner's yes (Owner decision 3). On Windows,
//! `scripts/phase0/platform-ignored.ps1`, the desktop-session run of the platform's ignored tests (design part 12.4;
//! Task T3-25), is read and parsed here too, never run.

use std::fs;
#[cfg(windows)]
use std::path::Path;
use std::path::PathBuf;
#[cfg(windows)]
use std::sync::atomic::{AtomicUsize, Ordering};

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

/// `stem` followed by a suffix no other call of this test binary gives, for a temporary file or folder.
#[cfg(windows)]
fn unique(stem: &str) -> String {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |time| time.as_nanos());
    format!("{stem}-{}-{nanos}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))
}

/// What `call` prints, trimmed, after the functions named `functions` and the top-level assignments to the variables
/// named `variables` of the script at `path` are dot-sourced alone from its syntax tree (review round 1 of pull request
/// 3, V13): nothing else in the script runs (Owner decision 3). The snippet goes to a temporary .ps1 that Windows
/// PowerShell 5.1 runs with `-File`; a call that fails panics with what PowerShell printed.
#[cfg(windows)]
fn define_and_call(path: &Path, functions: &[&str], variables: &[&str], call: &str) -> String {
    let quote = |items: &[&str]| {
        items.iter().map(|item| format!("'{}'", item.replace('\'', "''"))).collect::<Vec<_>>().join(", ")
    };
    let snippet = format!(
        "$ErrorActionPreference = 'Stop'\r\n\
         $tokens = $null; $errors = $null\r\n\
         $ast = [System.Management.Automation.Language.Parser]::ParseFile('{script}', [ref]$tokens, [ref]$errors)\r\n\
         if ($errors.Count -gt 0) {{ throw \"the script does not parse: $($errors -join '; ')\" }}\r\n\
         $functionNames = @({functions})\r\n\
         $variableNames = @({variables})\r\n\
         $texts = @()\r\n\
         foreach ($statement in $ast.EndBlock.Statements) {{\r\n\
             $left = $statement.Left\r\n\
             if ($statement -is [System.Management.Automation.Language.AssignmentStatementAst] -and \
                 $left -is [System.Management.Automation.Language.VariableExpressionAst] -and \
                 $variableNames -contains $left.VariablePath.UserPath) {{ $texts += $statement.Extent.Text }}\r\n\
         }}\r\n\
         $isFunction = {{ param($node) $node -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $functionNames -contains $node.Name }}\r\n\
         foreach ($definition in $ast.FindAll($isFunction, $true)) {{ $texts += $definition.Extent.Text }}\r\n\
         if ($texts.Count -ne $functionNames.Count + $variableNames.Count) {{\r\n\
             throw \"the script holds $($texts.Count) of the $($functionNames.Count + $variableNames.Count) definitions asked for\"\r\n\
         }}\r\n\
         . ([scriptblock]::Create($texts -join \"`r`n\"))\r\n\
         {call}\r\n",
        script = path.display().to_string().replace('\'', "''"),
        functions = quote(functions),
        variables = quote(variables),
    );
    let file = std::env::temp_dir().join(format!("{}.ps1", unique("lazymkv-scripts")));
    // A byte order mark, so Windows PowerShell 5.1 reads the snippet as UTF-8 rather than in the system code page.
    fs::write(&file, format!("\u{feff}{snippet}")).expect("the snippet is written");
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(&file)
        .output()
        .expect("powershell starts");
    let _ = fs::remove_file(&file);
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    assert!(
        output.status.success(),
        "powershell {}: {stdout}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    );
    stdout
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
        // Review round 1 of pull request 3 (G1): Owner decision 13's check meets the release folder and each program.
        "foreach ($path in @($trees) + @($release) + $programs)",
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
    let (path, text) = script();
    assert!(text.is_ascii(), "Windows PowerShell 5.1 reads a file without a byte order mark in the system code page");
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

/// Review round 1 of pull request 3 (V13): the task's argument line is not quoted, Task Scheduler may expand a percent
/// sign in it, and the engine reads a relative folder against the run folder, protects whatever folder it is given and
/// creates only the last component, so `Get-DataProblem` refuses every `-Data` but a full local folder path below a
/// drive's root, outside the run folder, whose parent exists. C: paths only, since the runner may have no D:.
#[cfg(windows)]
#[test]
fn the_task_scripts_data_check_refuses_what_the_task_line_would_split_or_misplace() {
    let (path, _) = script();
    let missing = format!(r"'C:\{}\data'", unique("no-such-parent"));
    let run = r"'C:\lazymkv\run'";
    let accepted = [
        (r"'C:\lazymkv-data'", run),
        (r"'C:\lazymkv-data\'", run),
        (r"'C:\lazymkv-running'", r"'C:\lazymkv-run'"),
    ];
    let refused = [
        r"'C:\lazymkv data'",
        r"('C:\a' + [char]34 + 'b')",
        r"'C:\a%TEMP%'",
        "'data'",
        r"'\data'",
        "'C:data'",
        r"'C:\'",
        r"'C:\x\..'",
        r"'C:\lazymkv\run'",
        r"'C:\LAZYMKV\RUN\data'",
        r"'C:\lazymkv\run\..\run\data'",
        missing.as_str(),
    ];
    let cases = accepted.iter().copied().chain(refused.iter().map(|data| (*data, run))).collect::<Vec<_>>();
    let call = cases
        .iter()
        .map(|(data, run)| {
            format!("$problem = Get-DataProblem {data} {run}; if ($problem) {{ \"refused: $problem\" }} else {{ 'accepted' }}")
        })
        .collect::<Vec<_>>()
        .join("\r\n");
    let output = define_and_call(&path, &["Get-DataProblem"], &[], &call);
    let lines = output.lines().map(str::trim).collect::<Vec<_>>();
    assert_eq!(lines.len(), cases.len(), "{output}");
    for ((data, run), line) in cases.iter().zip(&lines).take(accepted.len()) {
        assert_eq!(*line, "accepted", "-Data {data} with the run folder {run}");
    }
    for ((data, _), line) in cases.iter().zip(&lines).skip(accepted.len()) {
        assert!(line.starts_with("refused: -Data "), "-Data {data}: {line}");
    }
}

/// Review round 1 of pull request 3 (V15): when lazymkvctl status never answered, the text follows the task's state
/// and last result: a running engine sends the reader to engine.log, an exit with 1 or 2 to the printed command, since
/// a start-up refusal reaches only the task's console window, and any other result to the owner's logon.
#[cfg(windows)]
#[test]
fn the_task_scripts_no_answer_text_follows_the_tasks_state_and_result() {
    let (path, _) = script();
    let command = r"C:\lazymkv\run\lazymkv-engine.exe --data C:\x";
    let call = format!(
        "Get-NoAnswerText 'Running' 267009 'C:\\x' '{command}'\r\n\
         Get-NoAnswerText 'Ready' 1 'C:\\x' '{command}'\r\n\
         Get-NoAnswerText 'Ready' 267011 'C:\\x' '{command}'"
    );
    let output = define_and_call(&path, &["Get-NoAnswerText"], &[], &call);
    let lines = output.lines().map(str::trim).collect::<Vec<_>>();
    assert_eq!(lines.len(), 3, "{output}");
    for (line, needles) in lines.iter().zip([
        [r"C:\x\engine.log", r"C:\x"],
        [command, "console window"],
        ["0x00041303", "logged on"],
    ]) {
        for needle in needles {
            assert!(line.contains(needle), "{line} lacks {needle}");
        }
    }
}

/// Review round 1 of pull request 3 (V15): with no -Data, the script names the data folder the engine's
/// `win::program_data` reads from the registry (whole-branch review F2), never `$env:ProgramData`.
#[cfg(windows)]
#[test]
fn the_task_scripts_data_folder_is_the_engines() {
    let (path, text) = script();
    assert!(!text.contains("$env:ProgramData"), "the script reads ProgramData from the environment");
    let output = define_and_call(&path, &["Get-DataFolder"], &[], "Get-DataFolder ''\r\nGet-DataFolder 'C:\\x'");
    let lines = output.lines().map(str::trim).collect::<Vec<_>>();
    assert_eq!(lines.len(), 2, "{output}");
    let engines = lazymkv_platform::win::program_data().expect("the registry names ProgramData");
    assert!(lines[0].eq_ignore_ascii_case(&engines.to_string_lossy()), "{} is not {}", lines[0], engines.display());
    assert_eq!(lines[1], r"C:\x");
}

/// Review round 1 of pull request 3 (R4): the run folder's read-back compares with the access list as Windows writes it
/// back, where the built-in Administrator's SID becomes LA. The SIDs are built at run time from the current account's
/// domain, and PowerShell compares them, so none is written here or printed (RP21).
#[cfg(windows)]
#[test]
fn the_task_scripts_read_back_expects_aliases_as_windows_writes_them() {
    let (path, _) = script();
    let reader = folder_sddl("$d-1999");
    let administrator = folder_sddl("$d-500");
    let call = format!(
        "$d = [System.Security.Principal.WindowsIdentity]::GetCurrent().User.AccountDomainSid.Value\r\n\
         $written = Get-WrittenAces \"{reader}\"\r\n\
         if ($written -ceq \"{aces}\") {{ 'same' }} else {{ 'differs' }}\r\n\
         $written = Get-WrittenAces \"{administrator}\"\r\n\
         if ($written.EndsWith('(A;OICI;FR;;;LA)')) {{ 'LA' }} elseif ($written.EndsWith(\"(A;OICI;FR;;;$d-500)\")) {{ 'literal' }} else {{ 'other' }}",
        aces = &reader[5..],
    );
    let output = define_and_call(&path, &["Get-WrittenAces"], &[], &call);
    let lines = output.lines().map(str::trim).collect::<Vec<_>>();
    assert_eq!(lines, ["same", "LA"], "{output}");
}

/// Runs icacls with `args` and panics when it fails.
#[cfg(windows)]
fn icacls(args: &[&std::ffi::OsStr]) {
    let output = std::process::Command::new("icacls").args(args).output().expect("icacls starts");
    assert!(output.status.success(), "icacls {}: {}", output.status, String::from_utf8_lossy(&output.stdout).trim());
}

/// A scratch folder below the temp folder, its access list reset and the folder removed on drop.
#[cfg(windows)]
struct Scratch(PathBuf);

#[cfg(windows)]
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::process::Command::new("icacls").arg(&self.0).args(["/reset", "/T", "/Q"]).output();
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Review round 1 of pull request 3 (G1): an entry set on a program file alone never shows in its folder's list, so
/// `Get-Writers` reads the file's own list, which is why the script checks each program copied from the release folder.
#[cfg(windows)]
#[test]
fn the_writers_check_sees_an_entry_a_program_file_alone_carries() {
    let (path, _) = script();
    let scratch = Scratch(std::env::temp_dir().join(unique("lazymkv-writers")));
    fs::create_dir(&scratch.0).expect("the scratch folder is made");
    let folder = scratch.0.as_os_str();
    let grants = ["/inheritance:r", "/grant:r", "*S-1-5-18:(OI)(CI)F", "*S-1-5-32-544:(OI)(CI)F"];
    icacls(&[&[folder][..], &grants.map(std::ffi::OsStr::new)[..]].concat());
    let program = scratch.0.join("lazymkv-engine.exe");
    fs::write(&program, b"").expect("the program file is made");
    icacls(&[program.as_os_str(), "/grant".as_ref(), "*S-1-5-11:(W)".as_ref()]);
    let call = format!(
        "'folder: ' + ((Get-Writers '{}') -join ',')\r\n'program: ' + ((Get-Writers '{}') -join ',')",
        scratch.0.display(),
        program.display()
    );
    let output = define_and_call(&path, &["Get-Writers"], &["trusted", "writeMask"], &call);
    let lines = output.lines().map(str::trim).collect::<Vec<_>>();
    assert_eq!(lines.len(), 2, "{output}");
    let authenticated = "S-1-5-11";
    let folder = lines[0].strip_prefix("folder:").expect("the folder's line");
    let program = lines[1].strip_prefix("program:").expect("the program's line");
    assert!(!folder.trim().split(',').any(|sid| sid == authenticated), "the folder's writers hold {authenticated}");
    assert!(program.trim().split(',').any(|sid| sid == authenticated), "the program's writers lack {authenticated}");
}
