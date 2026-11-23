//! `scripts/engine-task.ps1`, the desktop task's script (P5; design parts 3.1, 3.5; Task T3-24), read and parsed here;
//! on Windows its functions are called alone from its syntax tree; the script itself never runs, since running it
//! registers a task, which needs the owner's yes (Owner decision 3). On Windows,
//! `scripts/phase0/platform-ignored.ps1`, the desktop-session run of the platform's ignored tests (design part 12.4;
//! Task T3-25), and `scripts/phase0/engine-ocr-trial.ps1`, the OCR trial's desktop-session steps (design part 10.9;
//! pull request 5's Task 10), are read and parsed here too, never run.
//! Pull request 6's sitting scripts, `scripts/phase0/ffmpeg-check.ps1` and `scripts/phase0/worker-check.ps1`, the
//! helper functions they share, `scripts/phase0/check-common.ps1` (the review of pull request #21 (X13)), and
//! `audio-levels.ps1`'s two roots (Task 5) are read on every system and parsed on Windows, never run.

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

/// `text` with the temp folder's path written `<temp>` and each account SID (`S-1-5-21-` and its digits and hyphens)
/// written `<sid>`, for a failure's print, since the temp folder's path holds the account name (RP21; the round's final
/// review (W14)). The path is matched without its trailing separator, so a text holding it with one keeps that
/// separator after `<temp>`, and ignoring ASCII case, as Windows compares paths.
#[cfg(windows)]
fn redacted(text: &str) -> String {
    let temp = std::env::temp_dir().display().to_string();
    let temp = temp.trim_end_matches(['\\', '/']).to_ascii_lowercase();
    let mut without_temp = String::with_capacity(text.len());
    let mut last = 0;
    if !temp.is_empty() {
        // ASCII lowercasing keeps every byte offset, so the matches' offsets index `text` too.
        for (at, _) in text.to_ascii_lowercase().match_indices(&temp) {
            without_temp.push_str(&text[last..at]);
            without_temp.push_str("<temp>");
            last = at + temp.len();
        }
    }
    without_temp.push_str(&text[last..]);
    let prefix = "S-1-5-21-";
    let mut result = String::with_capacity(without_temp.len());
    let mut rest = without_temp.as_str();
    while let Some(at) = rest.find(prefix) {
        result.push_str(&rest[..at]);
        result.push_str("<sid>");
        let tail = &rest[at + prefix.len()..];
        rest = &tail[tail.find(|c: char| !c.is_ascii_digit() && c != '-').unwrap_or(tail.len())..];
    }
    result.push_str(rest);
    result
}

/// What `call` prints, trimmed, after the functions named `functions` and the top-level assignments to the variables
/// named `variables` of the script at `path` are dot-sourced alone from its syntax tree (review round 1 of pull request
/// 3, V13): nothing else in the script runs (Owner decision 3). The snippet goes to a temporary .ps1 that Windows
/// PowerShell 5.1 runs with `-File`; a call that fails panics with what PowerShell printed, `redacted`.
#[cfg(windows)]
fn define_and_call(path: &Path, functions: &[&str], variables: &[&str], call: &str) -> String {
    let quote = |items: &[&str]| {
        items.iter().map(|item| format!("'{}'", item.replace('\'', "''"))).collect::<Vec<_>>().join(", ")
    };
    // Windows PowerShell's own modules first: GitHub's windows-latest sets PSModulePath for PowerShell 7, whose edition of
    // Microsoft.PowerShell.Security Windows PowerShell 5.1 then finds first and cannot load, so Get-Acl fails with
    // CouldNotAutoloadMatchingModule (the CI run of the round's head; a planted module reproduced it on the mini PC).
    let snippet = format!(
        "$ErrorActionPreference = 'Stop'\r\n\
         $env:PSModulePath = \"$PSHOME\\Modules;$env:PSModulePath\"\r\n\
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
        "{}",
        redacted(&format!("powershell {}: {stdout}\n{}", output.status, String::from_utf8_lossy(&output.stderr).trim()))
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
        // Pull request 7's notes, call 10: the run folder without -Zip, the tree's bin with it.
        "-WorkingDirectory $bin",
        "$bin = $run",
        r"$bin = Join-Path $tree 'bin'",
        "ENGINE-TASK-DONE status=",
        // Review round 1 of pull request 3 (G1): Owner decision 13's check meets the release folder and each program
        // (without -Zip, call 10).
        "$checked = @($trees) + @($release) + $programs",
        "foreach ($path in $checked)",
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

/// Call 18, T5-R35: the OCR worker is copied into the run folder with the other two programs, and a release tree
/// without it refuses the deploy.
#[test]
fn the_task_script_requires_lazymkv_ocr() {
    let (_, text) = script();
    assert!(text.contains("$required = 'lazymkv-engine.exe', 'lazymkvctl.exe', 'lazymkv-ocr.exe'"));
    assert!(!text.contains("$optional"), "the script still lists an optional program");
}

/// Pull request 7's notes, call 10 (T7-R4): `-Zip` and `-Sha256` are declared empty and refused one without the other,
/// and the zip is hashed only as its copy in the run folder: `Get-FileHash` appears only in `Get-ZipProblem`'s body,
/// whose one call is on `$copy`, the run folder's `release.zip`, and the line that names `-Zip`'s own file copies it
/// there or checks that it is one, never hashes or extracts it. Textual, on every system.
#[test]
fn the_task_script_takes_a_zip_only_with_its_hash() {
    let (_, text) = script();
    let code = code_lines(&text);
    let start = code.iter().position(|line| *line == "param(").expect("a param block");
    let end = start + code[start..].iter().position(|line| *line == ")").expect("the param block ends");
    for declared in ["[string]$Zip = ''", "[string]$Sha256 = ''"] {
        assert!(code[start..end].iter().any(|line| line.trim_end_matches(',') == declared), "param lacks {declared}");
    }
    let refusal = "-Zip and -Sha256 go together: the hash is the zip's line in the release workflow's SHA256SUMS";
    assert!(text.contains(refusal), "the script lacks the refusal: {refusal}");

    let lines: Vec<&str> = text.lines().collect();
    let definition = lines
        .iter()
        .position(|line| line.starts_with("function Get-ZipProblem("))
        .expect("the script defines Get-ZipProblem");
    let close = definition + lines[definition..].iter().position(|line| *line == "}").expect("its definition ends");
    let hashing: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| !line.trim_start().starts_with('#') && line.contains("Get-FileHash"))
        .map(|(at, _)| at)
        .collect();
    assert!(!hashing.is_empty(), "the script hashes nothing");
    for at in hashing {
        assert!(at > definition && at < close, "Get-FileHash outside Get-ZipProblem: {}", lines[at]);
    }
    let calls: Vec<&str> =
        code.iter().copied().filter(|line| line.contains("Get-ZipProblem") && !line.starts_with("function ")).collect();
    assert_eq!(calls, ["$problem = Get-ZipProblem $copy $Sha256"], "Get-ZipProblem is asked about the copy alone");
    assert!(code.contains(&r"$copy = Join-Path $run 'release.zip'"), "the copy is not the run folder's release.zip");
    assert!(text.contains("Copy-Item -LiteralPath $Zip -Destination $copy -Force"), "the zip is not copied to $copy");
    for line in code.iter().filter(|line| line.contains("$Zip")) {
        for reader in ["Get-FileHash", "Get-ZipProblem", "Expand-ReleaseTree", "Expand-Archive"] {
            assert!(!line.contains(reader), "-Zip's own file reaches {reader}: {line}");
        }
    }
}

#[cfg(windows)]
#[test]
fn the_task_script_parses_without_errors() {
    let (path, text) = script();
    assert!(text.is_ascii(), "Windows PowerShell 5.1 reads a file without a byte order mark in the system code page");
    let errors = parse_errors(&path);
    assert!(errors.is_empty(), "{errors}");
}

/// Notes calls 8 and 13 (pull request 6's Task 4): the release job's assembly script and the zip's smoke script, which
/// the mini PC also runs under Windows PowerShell 5.1, and `archive.ps1`, a file of functions that is dot-sourced,
/// parse there without errors; none runs here.
#[cfg(windows)]
#[test]
fn the_release_scripts_parse_without_errors() {
    for name in ["assemble.ps1", "smoke.ps1", "archive.ps1"] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(r"..\..\packaging\release").join(name);
        let text = fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert!(text.is_ascii(), "{name}: PowerShell 5.1 reads a file with no byte order mark in the system code page");
        let errors = parse_errors(&path);
        assert!(errors.is_empty(), "{name}: {errors}");
    }
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
        // Review round 1 of pull request 3 (R3): win::creds itself, as Owner decision 8's job passes it, sets the vault.
        "$env:LAZYMKV_TEST_VAULT = if ($Filter -match '^win::creds(::|$)') { '1' } else { '' }",
        "\"platform-ignored: filter $Filter, drive '$Drive', share root '$ShareRoot', vault '$env:LAZYMKV_TEST_VAULT'\"",
        r"Set-Location 'C:\lazymkv\src'",
        r#"cmd /c "cargo test -p lazymkv-platform --locked -- --include-ignored --test-threads=1 --nocapture $Filter 2>&1""#,
    ] {
        assert!(text.contains(line), "platform-ignored.ps1 lacks: {line}");
    }
    assert!(!text.contains("ScheduledTask"), "the script registers nothing; run-interactive.ps1 does");
}

/// Design part 10.9 (Rulings T5-R45, T5-R62): the OCR trial's desktop-session script reads the rows from
/// trial-tracks.txt, lets a slug or a label reach only a path or a check, sets RTEN_NUM_THREADS=3 for arm R' alone and
/// runs no build. It is read here, and parsed on Windows; it never runs in a test.
#[test]
fn the_engine_ocr_trial_script_parses_sets_rten_num_threads_for_rows_and_prints_numbers_only() {
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("scripts").join("phase0").join("engine-ocr-trial.ps1");
    let text = fs::read_to_string(&script).unwrap_or_else(|error| panic!("{}: {error}", script.display()));
    for needle in [
        "[Parameter(Mandatory)][ValidateSet('inventory', 'read', 'rows', 'compare', 'f3')][string]$Step",
        r#"$Tracks = "$P0\trial-tracks.txt""#,
        "$ClearMax = 64",
        "$env:RTEN_NUM_THREADS = '3'",
        "Remove-Item Env:RTEN_NUM_THREADS",
        "'--clear-max', [string]$ClearMax",
        "subs-ocr-engine",
        "Assert-Quiet",
        "'RESULT ok'",
    ] {
        assert!(text.contains(needle), "engine-ocr-trial.ps1 lacks: {needle}");
    }
    let code: Vec<&str> = text.lines().map(str::trim).filter(|line| !line.starts_with('#')).collect();
    for absent in ["cargo build", "cargo test", "cargo run", "write-host", "ocrs-trial.exe", "makemkvcon64.exe"] {
        assert!(!code.iter().any(|line| line.to_ascii_lowercase().contains(absent)), "engine-ocr-trial.ps1 runs or prints {absent}");
    }
    let allowed = [
        "[string]$Slug = ''", "[string]$Label = ''", "if ($Slug -and ", "if ($Label -and ", "$rows += [pscustomobject]@{",
        "$rec = ", "$text = ", "$engineOut = ", "$title = @(", "$f3Dir = ",
    ];
    for line in code.iter().filter(|line| {
        let lower = line.to_ascii_lowercase();
        lower.contains("slug") || lower.contains("label")
    }) {
        assert!(allowed.iter().any(|start| line.starts_with(start)), "a slug or a label reaches more than a path or a check: {line}");
    }
    #[cfg(windows)]
    {
        assert!(text.is_ascii(), "Windows PowerShell 5.1 reads a file without a byte order mark in the system code page");
        let errors = parse_errors(&script);
        assert!(errors.is_empty(), "{errors}");
    }
}

/// Review round 1 of pull request 3 (V13): the task's argument line is not quoted, Task Scheduler may expand a percent
/// sign in it, and the engine reads a relative folder against the run folder, protects whatever folder it is given and
/// creates only the last component, so `Get-DataProblem` refuses every `-Data` but a full local folder path below a
/// drive's root, outside the run folder, whose parent exists. It refuses too a folder that holds the run folder or a
/// build tree, or lies in a build tree, which scripts/box.sh or cargo clean recreates, or elsewhere under their parent,
/// which a non-elevated process may write (the round's final review (W12)); the trees are the script's own. C: paths
/// only, since the runner may have no D:.
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
        r"'C:\lazymkv'",
        r"'C:\lazymkv\'",
        r"'C:\lazymkv\other'",
        r"'C:\lazymkv\src\data'",
        r"'C:\LAZYMKV\TARGET\x'",
        r"('C:\lazymkv-data' + [char]10)",
    ];
    // What the last six refusals say: the folder the value holds or lies in, or the first check's text.
    let texts = [
        r"holds C:\lazymkv\run",
        r"holds C:\lazymkv\run",
        r"is in C:\lazymkv, the run folder's parent",
        r"is in C:\lazymkv\src,",
        r"is in C:\lazymkv\target,",
        "is not a full local folder path",
    ];
    let cases = accepted.iter().copied().chain(refused.iter().map(|data| (*data, run))).collect::<Vec<_>>();
    // A refusal's text is printed on one line, since a value with a newline puts one in it.
    let call = cases
        .iter()
        .map(|(data, run)| {
            format!(
                "$problem = Get-DataProblem {data} {run} $trees; \
                 if ($problem) {{ \"refused: $($problem -replace '\\r?\\n', '<newline>')\" }} else {{ 'accepted' }}"
            )
        })
        .collect::<Vec<_>>()
        .join("\r\n");
    let output = define_and_call(&path, &["Get-DataProblem"], &["run", "trees"], &call);
    let lines = output.lines().map(str::trim).collect::<Vec<_>>();
    assert_eq!(lines.len(), cases.len(), "{output}");
    for ((data, run), line) in cases.iter().zip(&lines).take(accepted.len()) {
        assert_eq!(*line, "accepted", "-Data {data} with the run folder {run}");
    }
    for ((data, _), line) in cases.iter().zip(&lines).skip(accepted.len()) {
        assert!(line.starts_with("refused: -Data "), "-Data {data}: {line}");
    }
    for ((data, line), text) in refused.iter().zip(&lines[accepted.len()..]).skip(refused.len() - texts.len()).zip(texts) {
        assert!(line.contains(text), "-Data {data}: {line} lacks {text}");
    }
}

/// Review round 1 of pull request 3 (V15): when lazymkvctl status never answered, the text follows the task's state
/// and last result: a running engine sends the reader to engine.log, an exit with 1 or 2 to the printed command, since
/// a start-up refusal reaches only the task's console window, Task Scheduler's own codes (0x0004xxxx, 0x8004xxxx,
/// 0x8007xxxx) to the owner's logon, and any other result, the engine's own exit code, to engine.log and the printed
/// command (the round's final review (W11)). The results are given in decimal, since PowerShell reads 0xC0000005 as a
/// negative Int32 and Task Scheduler's LastTaskResult is unsigned.
#[cfg(windows)]
#[test]
fn the_task_scripts_no_answer_text_follows_the_tasks_state_and_result() {
    let (path, _) = script();
    let command = r"C:\lazymkv\run\lazymkv-engine.exe --data C:\x";
    // 0x41301 running, 1, 0x41303 not yet run, 0, 0xC0000005, 0x80070002.
    let results = [
        ("Running", 267_009_u64),
        ("Ready", 1),
        ("Ready", 267_011),
        ("Ready", 0),
        ("Ready", 3_221_225_477),
        ("Ready", 2_147_942_402),
    ];
    let call = results
        .iter()
        .map(|(state, result)| format!("Get-NoAnswerText '{state}' {result} 'C:\\x' '{command}'"))
        .collect::<Vec<_>>()
        .join("\r\n");
    let output = define_and_call(&path, &["Get-NoAnswerText"], &[], &call);
    let lines = output.lines().map(str::trim).collect::<Vec<_>>();
    assert_eq!(lines.len(), results.len(), "{output}");
    let exited = |code: &'static str| vec![code, r"C:\x\engine.log", command, "console window"];
    for (line, needles) in lines.iter().zip([
        vec![r"C:\x\engine.log", r"C:\x"],
        vec![command, "console window"],
        vec!["0x00041303", "logged on"],
        exited("exited with 0x00000000"),
        exited("exited with 0xC0000005"),
        vec!["0x80070002", "logged on"],
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

/// Runs icacls with `args` and panics when it fails, with what icacls printed, `redacted`.
#[cfg(windows)]
fn icacls(args: &[&std::ffi::OsStr]) {
    let output = std::process::Command::new("icacls").args(args).output().expect("icacls starts");
    assert!(
        output.status.success(),
        "{}",
        redacted(&format!("icacls {}: {}", output.status, String::from_utf8_lossy(&output.stdout).trim()))
    );
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

/// `path` as a single-quoted PowerShell string's content.
#[cfg(windows)]
fn quoted(path: &Path) -> String {
    path.display().to_string().replace('\'', "''")
}

/// Pull request 7's notes, call 10: `Get-ZipProblem` refuses a file that is not there, a hash that is not 64 hex
/// digits (short, long, not hex, empty, or ending in a newline) and one that is not the file's, and passes the file's
/// own in either case. The zip is made inside the PowerShell call by `Compress-Archive` in a fresh temp folder that is
/// removed afterwards; a refusal's text is printed on one line.
#[cfg(windows)]
#[test]
fn the_zip_check_refuses_a_missing_file_a_malformed_hash_and_a_wrong_hash_and_passes_the_right_one() {
    let (path, _) = script();
    let folder = std::env::temp_dir().join(unique("lazymkv-zip-check"));
    let call = format!(
        "$ProgressPreference = 'SilentlyContinue'\r\n\
         $folder = '{folder}'\r\n\
         New-Item -ItemType Directory -Path $folder | Out-Null\r\n\
         try {{\r\n\
             $file = Join-Path $folder 'a.txt'\r\n\
             Set-Content -LiteralPath $file -Value 'a'\r\n\
             $zip = Join-Path $folder 'a.zip'\r\n\
             Compress-Archive -LiteralPath $file -DestinationPath $zip\r\n\
             $hash = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash\r\n\
             $cases = @(\r\n\
                 @('missing', (Join-Path $folder 'none.zip'), $hash),\r\n\
                 @('short', $zip, ($hash.Substring(1))),\r\n\
                 @('long', $zip, ($hash + '0')),\r\n\
                 @('not hex', $zip, ('g' * 64)),\r\n\
                 @('empty', $zip, ''),\r\n\
                 @('newline', $zip, ($hash + [char]10)),\r\n\
                 @('wrong', $zip, ('0' * 64)),\r\n\
                 @('upper', $zip, ($hash.ToUpperInvariant())),\r\n\
                 @('lower', $zip, ($hash.ToLowerInvariant()))\r\n\
             )\r\n\
             foreach ($case in $cases) {{\r\n\
                 $case[0] + ': ' + ((Get-ZipProblem $case[1] $case[2]) -replace '\\r?\\n', '<newline>')\r\n\
             }}\r\n\
         }} finally {{\r\n\
             Remove-Item -LiteralPath $folder -Recurse -Force\r\n\
         }}",
        folder = quoted(&folder),
    );
    let output = define_and_call(&path, &["Get-ZipProblem"], &[], &call);
    let lines = output.lines().map(str::trim).collect::<Vec<_>>();
    assert_eq!(lines.len(), 9, "{}", redacted(&output));
    let malformed = "is not 64 hexadecimal digits; the hash is the zip's line in the release workflow's SHA256SUMS";
    let expected = [
        ("missing: ", Some(r"\none.zip is not a file")),
        ("short: ", Some(malformed)),
        ("long: ", Some(malformed)),
        ("not hex: ", Some(malformed)),
        ("empty: ", Some(malformed)),
        ("newline: ", Some(malformed)),
        ("wrong: ", Some("'s SHA-256 is ")),
        ("upper:", None),
        ("lower:", None),
    ];
    for (line, (label, needle)) in lines.iter().zip(expected) {
        match needle {
            Some(needle) => {
                let refusal = line.strip_prefix(label).unwrap_or_else(|| panic!("{} is not {label}", redacted(line)));
                assert!(refusal.contains(needle), "{label}{} lacks {needle}", redacted(refusal));
            }
            None => assert_eq!(*line, label, "{}", redacted(line)),
        }
    }
    assert!(lines[6].contains(&format!("not -Sha256's {}", "0".repeat(64))), "{}", redacted(lines[6]));
    assert!(!folder.exists(), "the call left {}", redacted(&folder.display().to_string()));
}

/// Pull request 7's notes, call 10: `Expand-ReleaseTree` removes an earlier `$into`, extracts the zip there and returns
/// its one top-level folder named `lazymkv-*`; a zip with two top-level entries, with one folder of another name, or
/// with one file named `lazymkv-*`, throws. The zips are made inside the PowerShell call by `Compress-Archive` in a
/// fresh temp folder that is removed afterwards.
#[cfg(windows)]
#[test]
fn the_extraction_returns_the_one_lazymkv_folder_and_replaces_an_earlier_tree() {
    let (path, _) = script();
    let folder = std::env::temp_dir().join(unique("lazymkv-zip-tree"));
    let call = format!(
        "$ProgressPreference = 'SilentlyContinue'\r\n\
         $folder = '{folder}'\r\n\
         New-Item -ItemType Directory -Path $folder | Out-Null\r\n\
         try {{\r\n\
             $src = Join-Path $folder 'src'\r\n\
             $good = Join-Path $src 'lazymkv-1.2.3-windows-x64'\r\n\
             New-Item -ItemType Directory -Path (Join-Path $good 'bin') | Out-Null\r\n\
             Set-Content -LiteralPath (Join-Path $good 'bin\\a.txt') -Value 'a'\r\n\
             $other = Join-Path $src 'other-1.2.3'\r\n\
             New-Item -ItemType Directory -Path $other | Out-Null\r\n\
             Set-Content -LiteralPath (Join-Path $other 'a.txt') -Value 'a'\r\n\
             $loose = Join-Path $src 'lazymkv-loose.txt'\r\n\
             Set-Content -LiteralPath $loose -Value 'a'\r\n\
             Compress-Archive -LiteralPath $good -DestinationPath (Join-Path $folder 'good.zip')\r\n\
             Compress-Archive -LiteralPath $good, $loose -DestinationPath (Join-Path $folder 'two.zip')\r\n\
             Compress-Archive -LiteralPath $other -DestinationPath (Join-Path $folder 'other.zip')\r\n\
             Compress-Archive -LiteralPath $loose -DestinationPath (Join-Path $folder 'file.zip')\r\n\
             $into = Join-Path $folder 'into'\r\n\
             New-Item -ItemType Directory -Path $into | Out-Null\r\n\
             Set-Content -LiteralPath (Join-Path $into 'stray.txt') -Value 'a'\r\n\
             $tree = Expand-ReleaseTree (Join-Path $folder 'good.zip') $into\r\n\
             'tree: ' + (Split-Path $tree -Leaf)\r\n\
             'file: ' + (Test-Path -LiteralPath (Join-Path $tree 'bin\\a.txt') -PathType Leaf)\r\n\
             'stray: ' + (Test-Path -LiteralPath (Join-Path $into 'stray.txt'))\r\n\
             'entries: ' + @(Get-ChildItem -LiteralPath $into -Force).Count\r\n\
             foreach ($bad in 'two', 'other', 'file') {{\r\n\
                 try {{ Expand-ReleaseTree (Join-Path $folder ($bad + '.zip')) $into | Out-Null; $bad + ': returned' }}\r\n\
                 catch {{ $bad + ': threw ' + $_.Exception.Message }}\r\n\
             }}\r\n\
         }} finally {{\r\n\
             Remove-Item -LiteralPath $folder -Recurse -Force\r\n\
         }}",
        folder = quoted(&folder),
    );
    let output = define_and_call(&path, &["Expand-ReleaseTree", "Remove-Earlier", "Get-LinkBelow"], &[], &call);
    let lines = output.lines().map(str::trim).collect::<Vec<_>>();
    let refusal = "top-level entries, not one folder lazymkv-<version>";
    assert_eq!(
        lines,
        [
            "tree: lazymkv-1.2.3-windows-x64".to_owned(),
            "file: True".to_owned(),
            "stray: False".to_owned(),
            "entries: 1".to_owned(),
            format!("two: threw the zip holds 2 {refusal}"),
            format!("other: threw the zip holds 1 {refusal}"),
            format!("file: threw the zip holds 1 {refusal}"),
        ],
        "{}",
        redacted(&output)
    );
    assert!(!folder.exists(), "the call left {}", redacted(&folder.display().to_string()));
}

/// Pull request 7's notes, call 10: `Get-TreeProblem` passes a tree with the six files of `$treeFiles` and otherwise
/// names the first one missing, in their order; a folder in a file's place, or no tree at all, lacks it too. The tree
/// is made inside the PowerShell call in a fresh temp folder that is removed afterwards.
#[cfg(windows)]
#[test]
fn the_tree_check_names_the_first_file_a_tree_lacks() {
    let (path, _) = script();
    let folder = std::env::temp_dir().join(unique("lazymkv-tree-check"));
    let call = format!(
        "$tree = '{folder}'\r\n\
         foreach ($sub in 'bin', 'resources') {{ New-Item -ItemType Directory -Path (Join-Path $tree $sub) -Force | Out-Null }}\r\n\
         try {{\r\n\
             foreach ($file in $treeFiles) {{ Set-Content -LiteralPath (Join-Path $tree $file) -Value '' }}\r\n\
             'full: ' + (Get-TreeProblem $tree)\r\n\
             Remove-Item -LiteralPath (Join-Path $tree 'resources\\ffprobe.exe')\r\n\
             'no ffprobe: ' + (Get-TreeProblem $tree)\r\n\
             Remove-Item -LiteralPath (Join-Path $tree 'bin\\lazymkvctl.exe')\r\n\
             'no ctl: ' + (Get-TreeProblem $tree)\r\n\
             Remove-Item -LiteralPath (Join-Path $tree 'bin\\lazymkv-engine.exe')\r\n\
             New-Item -ItemType Directory -Path (Join-Path $tree 'bin\\lazymkv-engine.exe') | Out-Null\r\n\
             'engine folder: ' + (Get-TreeProblem $tree)\r\n\
             'no tree: ' + (Get-TreeProblem (Join-Path $tree 'none'))\r\n\
         }} finally {{\r\n\
             Remove-Item -LiteralPath $tree -Recurse -Force\r\n\
         }}",
        folder = quoted(&folder),
    );
    let output = define_and_call(&path, &["Get-TreeProblem"], &["treeFiles"], &call);
    let lines = output.lines().map(str::trim).collect::<Vec<_>>();
    assert_eq!(
        lines,
        [
            "full:",
            r"no ffprobe: the release tree lacks resources\ffprobe.exe",
            r"no ctl: the release tree lacks bin\lazymkvctl.exe",
            r"engine folder: the release tree lacks bin\lazymkv-engine.exe",
            r"no tree: the release tree lacks bin\lazymkv-engine.exe",
        ],
        "{}",
        redacted(&output)
    );
    assert!(!folder.exists(), "the call left {}", redacted(&folder.display().to_string()));
}

/// The review of Task 4 of pull request 7 (finding 1): `Get-PlantedProblem` passes a run folder holding the three
/// programs, a real `release` folder and a `release.zip` file, and refuses one whose `release` is a junction, one with
/// an unknown name, a `release` that is a file and a `release.zip` that is a folder; and `Remove-Earlier` refuses a
/// junction, or a folder holding one, and removes a real folder. The junctions' target keeps its file throughout. The
/// stand-ins and junctions (`cmd /c mklink /J`) are made inside the PowerShell call in a fresh temp folder; the
/// junctions are removed alone with `rmdir` before the folder is.
#[cfg(windows)]
#[test]
fn the_planted_check_and_the_removal_refuse_a_link_and_leave_its_target_whole() {
    let (path, _) = script();
    let folder = std::env::temp_dir().join(unique("lazymkv-planted"));
    let call = format!(
        "$root = '{folder}'\r\n\
         New-Item -ItemType Directory -Path $root | Out-Null\r\n\
         $junction = Join-Path $root 'junction\\release'\r\n\
         $inner = Join-Path $root 'nested\\release\\inner'\r\n\
         try {{\r\n\
             $target = Join-Path $root 'target'\r\n\
             New-Item -ItemType Directory -Path $target | Out-Null\r\n\
             $kept = Join-Path $target 'kept.txt'\r\n\
             Set-Content -LiteralPath $kept -Value 'a'\r\n\
             foreach ($name in 'good', 'junction', 'unknown', 'file', 'folder', 'nested') {{\r\n\
                 $run = Join-Path $root $name\r\n\
                 New-Item -ItemType Directory -Path $run | Out-Null\r\n\
                 foreach ($program in $required) {{ Set-Content -LiteralPath (Join-Path $run $program) -Value '' }}\r\n\
             }}\r\n\
             New-Item -ItemType Directory -Path (Join-Path $root 'good\\release') | Out-Null\r\n\
             Set-Content -LiteralPath (Join-Path $root 'good\\release\\a.txt') -Value 'a'\r\n\
             Set-Content -LiteralPath (Join-Path $root 'good\\release.zip') -Value ''\r\n\
             cmd /c mklink /J $junction $target | Out-Null\r\n\
             if ($LASTEXITCODE -ne 0) {{ throw \"mklink /J exited $LASTEXITCODE\" }}\r\n\
             Set-Content -LiteralPath (Join-Path $root 'unknown\\other.txt') -Value ''\r\n\
             Set-Content -LiteralPath (Join-Path $root 'file\\release') -Value ''\r\n\
             New-Item -ItemType Directory -Path (Join-Path $root 'folder\\release.zip') | Out-Null\r\n\
             New-Item -ItemType Directory -Path (Join-Path $root 'nested\\release') | Out-Null\r\n\
             cmd /c mklink /J $inner $target | Out-Null\r\n\
             if ($LASTEXITCODE -ne 0) {{ throw \"mklink /J exited $LASTEXITCODE\" }}\r\n\
             foreach ($name in 'good', 'junction', 'unknown', 'file', 'folder', 'nested') {{\r\n\
                 $name + ': ' + (Get-PlantedProblem (Join-Path $root $name) $required)\r\n\
             }}\r\n\
             foreach ($earlier in $junction, (Join-Path $root 'nested\\release')) {{\r\n\
                 try {{ Remove-Earlier $earlier; 'remove: returned' }} catch {{ 'remove: threw ' + $_.Exception.Message }}\r\n\
             }}\r\n\
             'kept: ' + (Test-Path -LiteralPath $kept -PathType Leaf)\r\n\
             'inner: ' + (Test-Path -LiteralPath $inner)\r\n\
             Remove-Earlier (Join-Path $root 'good\\release')\r\n\
             'good release: ' + (Test-Path -LiteralPath (Join-Path $root 'good\\release'))\r\n\
         }} finally {{\r\n\
             foreach ($link in $junction, $inner) {{ if (Test-Path -LiteralPath $link) {{ cmd /c rmdir $link }} }}\r\n\
             Remove-Item -LiteralPath $root -Recurse -Force\r\n\
         }}",
        folder = quoted(&folder),
    );
    let output = define_and_call(
        &path,
        &["Get-PlantedProblem", "Get-LinkBelow", "Remove-Earlier"],
        &["required"],
        &call,
    );
    let lines = output.lines().map(str::trim).collect::<Vec<_>>();
    assert_eq!(lines.len(), 11, "{}", redacted(&output));
    let expected = [
        ("good:", None),
        ("junction: ", Some(r"\junction holds the link release, which this script never makes")),
        ("unknown: ", Some(r"\unknown holds other.txt, which this script did not put there")),
        ("file: ", Some(r"\file\release is a file, not the folder")),
        ("folder: ", Some(r"\folder\release.zip is a folder, not the zip's copy")),
        // The planted check reads the run folder's own entries; a link deeper down is Remove-Earlier's to refuse.
        ("nested:", None),
        ("remove: threw ", Some(r"\junction\release is a link, which this script never makes")),
        ("remove: threw ", Some(r"\nested\release\inner is a link, which this script never makes")),
        ("kept: True", None),
        ("inner: True", None),
        ("good release: False", None),
    ];
    for (line, (label, needle)) in lines.iter().zip(expected) {
        match needle {
            Some(needle) => {
                let text = line.strip_prefix(label).unwrap_or_else(|| panic!("{} is not {label}", redacted(line)));
                assert!(text.contains(needle), "{label}{} lacks {needle}", redacted(text));
            }
            None => assert_eq!(*line, label, "{}", redacted(line)),
        }
    }
    assert!(!folder.exists(), "the call left {}", redacted(&folder.display().to_string()));
}

/// Pull request 5's Task 11 (design part 10.4; notes calls 7 and 24; T5-R45): the desktop-session run of the decoder
/// goldens sets the four variables crates/ocr/tests/goldens.rs reads and starts only the test program it is given,
/// never cargo, since each scripts/box.sh call replaces C:\lazymkv\src under a running job. Read on every system and
/// parsed on Windows; never run.
#[test]
fn the_ocr_goldens_script_parses_sets_its_four_variables_and_runs_no_cargo() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("scripts").join("phase0").join("ocr-goldens.ps1");
    let text = fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    assert!(text.is_ascii(), "Windows PowerShell 5.1 reads a file without a byte order mark in the system code page");
    for line in [
        "[Parameter(Mandatory)][string]$Exe",
        r". C:\lazymkv\src\scripts\phase0\env.ps1",
        r"$Tracks = 'C:\lazymkv\phase0\trial-tracks.txt'",
        r"if ($Exe -notlike 'C:\lazymkv\target\release\deps\goldens-*.exe')",
        "$env:LAZYMKV_EVIDENCE = $Recordings",
        "$env:LAZYMKV_OCR_TEXT = $TuningData",
        "$env:LAZYMKV_OCR_TRACKS = $Tracks",
        "$env:LAZYMKV_OCR_STAGING = $Staging",
        r#"cmd /c "$Exe --test-threads=1 --nocapture 2>&1""#,
        "'RESULT ok'",
    ] {
        assert!(text.contains(line), "ocr-goldens.ps1 lacks: {line}");
    }
    for line in text.lines().filter(|line| !line.trim_start().starts_with('#')) {
        assert!(!line.contains("cargo"), "ocr-goldens.ps1 runs cargo: {line}");
    }
    assert!(!text.contains("ScheduledTask"), "the script registers nothing; run-interactive.ps1 does");
    #[cfg(windows)]
    {
        let errors = parse_errors(&path);
        assert!(errors.is_empty(), "{errors}");
    }
}

/// A script of scripts/phase0 and its text.
fn phase0_script(name: &str) -> (PathBuf, String) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let path = root.join("scripts").join("phase0").join(name);
    let text = fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    (path, text)
}

/// The lines of a PowerShell script that are code: trimmed, neither empty nor a comment.
fn code_lines(text: &str) -> Vec<&str> {
    text.lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with('#')).collect()
}

/// Notes call 12 (a), Rulings T6-R12 and T6-R13 (pull request 6's Task 5): the FFmpeg check prints its eight kinds of
/// line in the brief's order, and no name. The film's folder name is held in `$film`, and the rule is textual: every
/// `$film` in the code is either its one assignment (`$film = $found[0].folder`), the `-Slug $film` argument of the
/// call of audio-levels.ps1, or comes right after a backslash, that is a path segment inside a path string such as
/// `"$Recordings\$film\$Label"`; and no line holding `$film` throws, fails a part or writes. So `"levels: $film"`,
/// `"$($film)"`, `$name = $film` and `Stop-Part "...\$film"` all fail here. The one call of audio-levels.ps1 passes
/// `-RecordingsRoot` and `-TuningRoot` under `-Work` and no `-Force`. Read on every system, parsed on Windows; never
/// run.
#[test]
fn the_ffmpeg_check_script_parses_and_prints_no_name() {
    let (path, text) = phase0_script("ffmpeg-check.ps1");
    assert!(text.is_ascii(), "Windows PowerShell 5.1 reads a file without a byte order mark in the system code page");
    let stems = [
        "'hashes: not checked (the installed build)'",
        "\"hashes: $(Get-Same $same)\"",
        "\"levels: $($names.Count) series files, $equal equal, $different different; streamhash $(Get-Same $hash); \"",
        "\"proxy: $tracks tracks, packet hashes $(Get-Same $same)\"",
        "\"listing: $audio audio streams, $audioKeys differing keys; other streams: $otherKeys differing keys$named\"",
        "\"packets row ${r}: $(Get-Same $same)\"",
        "\"corpus copy: $($files.Count) files, $bytes bytes\"",
        "@{ key = 'golden'; line = 'golden check'; exits = @(0) }",
        "@{ key = 'retune'; line = 'retune'; exits = @(0, 1, 3) }",
        "\"$($part.line): $word; exit $($exits[$before]) then $($exits[$after])\"",
        "'RESULT ok'",
        "\"RESULT FAILED: $($failures -join '; ')\"",
    ];
    let mut last = 0;
    for stem in stems {
        let found = text[last..].find(stem);
        last += found.unwrap_or_else(|| panic!("ffmpeg-check.ps1 lacks, after the lines before it in order: {stem}"));
    }
    for line in ["'hashes: not run'", "'levels: not run'", "'proxy: not run'", "'listing: not run'",
        "\"packets row ${r}: not run\"", "'corpus copy: not run'", "\"$($part.line): not run\""] {
        assert!(text.contains(line), "ffmpeg-check.ps1 lacks: {line}");
    }

    let code = code_lines(&text);
    let mut uses = 0;
    for line in code.iter().filter(|line| line.contains("$film")) {
        for writer in ["Stop-Part", "Add-Failure", "throw", "Write-", "Out-File", "Set-Content"] {
            assert!(!line.contains(writer), "a line holding the film's folder name calls {writer}: {line}");
        }
        for (at, _) in line.match_indices("$film") {
            uses += 1;
            let before = &line[..at];
            let ok = before.ends_with('\\')
                || before.ends_with("-Slug ")
                || (at == 0 && *line == "$film = $found[0].folder");
            assert!(ok, "the film's folder name reaches more than a path: {line}");
        }
    }
    assert!(uses >= 8, "ffmpeg-check.ps1 uses $film {uses} times; the rule above checked nothing");
    assert_eq!(code.iter().filter(|line| line.contains("$found[0].folder")).count(), 1, "the film's name is read once");

    let calls: Vec<usize> =
        code.iter().enumerate().filter(|(_, line)| line.contains("& $LevelsScript")).map(|(i, _)| i).collect();
    assert_eq!(calls.len(), 1, "ffmpeg-check.ps1 calls audio-levels.ps1 {} times, not once", calls.len());
    let mut call = String::new();
    for line in &code[calls[0]..] {
        call.push_str(line.trim_end_matches('`'));
        call.push(' ');
        if !line.ends_with('`') {
            break;
        }
    }
    for part in ["-Slug $film -Label $Label -Main $main -Cand $cands", r#"-RecordingsRoot "$Work\recordings""#,
        r#"-TuningRoot "$Work\dataset""#] {
        assert!(call.contains(part), "the call of audio-levels.ps1 lacks {part}: {call}");
    }
    assert!(!call.contains("-Force"), "the call of audio-levels.ps1 passes -Force: {call}");
    let named: Vec<&&str> = code.iter().filter(|line| line.contains("audio-levels.ps1")).collect();
    assert_eq!(named, [&r"$LevelsScript = 'C:\lazymkv\src\scripts\phase0\audio-levels.ps1'"], "one path to it");
    #[cfg(windows)]
    {
        let errors = parse_errors(&path);
        assert!(errors.is_empty(), "{errors}");
    }
    #[cfg(not(windows))]
    let _ = path;
}

/// Notes call 11, Ruling T6-R15 (pull request 6's Task 5): the worker check's output lines name the row only, and it
/// never writes into the trial's kept output. Output is textual here: a code line that starts with a quote and does not
/// continue the line before it (which ends with a comma, an opening bracket, a plus, a pipe or a backtick) is a bare
/// string, so printed, and must start `'RESULT ok'` or `"RESULT FAILED: `; a line that is one variable alone is printed
/// too and must be one of the four line variables, which are assigned only strings starting `"row ${Row}: `. The one
/// exception is named: the sampler job's `$peak`; the values of the helpers it shares with the FFmpeg check are
/// check-common.ps1's, tested below (the review of pull request #21 (X13)). The kept output: `subs-ocr-engine` is named
/// in `$engineOut` only, `$engineOut` reaches only `$kept`, `$kept` is only tested and read, and no line that copies,
/// moves or writes names any of the three. Read on every system, parsed on Windows; never run.
#[test]
fn the_worker_check_script_parses_names_rows_only_and_never_writes_the_trials_kept_output() {
    let (path, text) = phase0_script("worker-check.ps1");
    assert!(text.is_ascii(), "Windows PowerShell 5.1 reads a file without a byte order mark in the system code page");
    let code = code_lines(&text);
    let lines = ["$shaLine", "$runLine", "$peakLine", "$recordsLine"];
    let returns = ["$peak"];
    let mut printed = Vec::new();
    for (i, line) in code.iter().enumerate() {
        let continues = i > 0 && ['+', ',', '(', '|', '`'].iter().any(|end| code[i - 1].ends_with(*end));
        if (line.starts_with('"') || line.starts_with('\'')) && !continues {
            assert!(line.starts_with("'RESULT ok'") || line.starts_with("\"RESULT FAILED: "), "an output line: {line}");
            printed.push(*line);
        }
        let alone = line.len() > 1 && line.starts_with('$') && line[1..].chars().all(|c| c.is_ascii_alphanumeric());
        if alone && !returns.contains(line) {
            assert!(lines.contains(line), "an output line that is not one of the four: {line}");
            printed.push(*line);
        }
        for name in lines {
            if let Some(value) = line.strip_prefix(&format!("{name} = ")) {
                assert!(value.starts_with("\"row ${Row}: "), "{name} gets a line that does not name the row: {line}");
            }
        }
    }
    let mut expected: Vec<&str> = lines.to_vec();
    expected.extend(["'RESULT ok'", "\"RESULT FAILED: $($failures -join '; ')\""]);
    assert_eq!(printed, expected, "worker-check.ps1 prints the four row lines, then its RESULT line");
    for absent in ["Write-Host", "Write-Output"] {
        assert!(!code.iter().any(|line| line.contains(absent)), "worker-check.ps1 calls {absent}");
    }

    let kept = ["subs-ocr-engine", "$engineOut", "$kept"];
    let writers =
        ["Copy-Item", "Copy-Checked", "Set-Content", "Out-File", "WriteAll", "Move-Item", "New-Item", "Add-Content"];
    for line in &code {
        if writers.iter().any(|writer| line.contains(writer)) {
            assert!(!kept.iter().any(|name| line.contains(name)), "a line writes into the kept output: {line}");
        }
    }
    let users = |name: &str| code.iter().copied().filter(|line| line.contains(name)).collect::<Vec<&str>>();
    let named = r#"$engineOut = "$TuningData\$($track.slug)\$($track.label)\subs-ocr-engine""#;
    assert_eq!(users("subs-ocr-engine"), [named]);
    assert_eq!(users("$engineOut"), [
        named,
        r#"$kept = "$engineOut\sub-$n.ocr.jsonl""#,
    ]);
    assert_eq!(users("$kept"), [
        r#"$kept = "$engineOut\sub-$n.ocr.jsonl""#,
        "if (-not (Test-Path -LiteralPath $kept -PathType Leaf)) {",
        "$two = @(Get-RecordLines $kept)",
    ]);
    for needle in [r#"-replace ',"ms":\{[^{}]*\}\}$', '}'"#, "PeakPagedMemorySize64", "Start-Sleep -Milliseconds 500",
        "Assert-Quiet", "Get-FileHash -LiteralPath $worker -Algorithm SHA256"] {
        assert!(text.contains(needle), "worker-check.ps1 lacks: {needle}");
    }
    #[cfg(windows)]
    {
        let errors = parse_errors(&path);
        assert!(errors.is_empty(), "{errors}");
    }
    #[cfg(not(windows))]
    let _ = path;
}

/// The review of pull request #21 (X13): the five helpers the FFmpeg check and the worker check share are one copy, in
/// check-common.ps1. The rules are textual, as above: its unindented code lines only open or close the five
/// functions' definitions, each defined once; its only bare output lines are the functions' values, Get-Reason's text,
/// Invoke-Recorded's `$LASTEXITCODE` and Get-TrialRows' `$rows`; and it calls nothing that prints, copies, moves or
/// writes. Each of the two scripts dot-sources it exactly once, by its absolute path, on the code line right after
/// `$ErrorActionPreference = 'Stop'` and so before its first call of any of the five, still creates the `$failures`
/// list Add-Failure fills, and defines none of the five itself. Read on every system, parsed on Windows; never run.
#[test]
fn the_check_helpers_are_one_copy_both_checks_dot_source_and_print_and_write_nothing() {
    let (path, text) = phase0_script("check-common.ps1");
    assert!(text.is_ascii(), "Windows PowerShell 5.1 reads a file without a byte order mark in the system code page");
    let functions = ["Add-Failure", "Stop-Part", "Get-Reason", "Invoke-Recorded", "Get-TrialRows"];
    let mut defined = Vec::new();
    for line in text.lines().filter(|line| !line.is_empty() && !line.starts_with([' ', '\t', '#'])) {
        match line.strip_prefix("function ") {
            Some(rest) => defined.push(rest.split(['(', ' ']).next().unwrap_or(rest)),
            None => assert_eq!(line, "}", "check-common.ps1 runs more than its functions' definitions: {line}"),
        }
    }
    assert_eq!(defined, functions, "check-common.ps1 defines the five helpers, each once");
    let code = code_lines(&text);
    let mut printed = Vec::new();
    for (i, line) in code.iter().enumerate() {
        let continues = i > 0 && ['+', ',', '(', '|', '`'].iter().any(|end| code[i - 1].ends_with(*end));
        let quoted = (line.starts_with('"') || line.starts_with('\'')) && !continues;
        let alone = line.len() > 1 && line.starts_with('$') && line[1..].chars().all(|c| c.is_ascii_alphanumeric());
        if quoted || alone {
            printed.push(*line);
        }
    }
    assert_eq!(printed.len(), 3, "check-common.ps1's bare output lines are its three functions' values: {printed:?}");
    assert!(printed[0].starts_with("\"an error of type "), "Get-Reason's text is not the first: {}", printed[0]);
    assert_eq!(printed[1..], ["$LASTEXITCODE", "$rows"], "Invoke-Recorded's and Get-TrialRows' values");
    for command in
        ["Write-Host", "Write-Output", "Out-File", "Set-Content", "Copy-Item", "Move-Item", "New-Item", "Remove-Item"]
    {
        let lower = command.to_ascii_lowercase();
        let calls = code.iter().any(|line| line.to_ascii_lowercase().contains(&lower));
        assert!(!calls, "check-common.ps1 calls {command}");
    }

    let source = r". C:\lazymkv\src\scripts\phase0\check-common.ps1";
    for name in ["ffmpeg-check.ps1", "worker-check.ps1"] {
        let (_, script) = phase0_script(name);
        let code = code_lines(&script);
        let named: Vec<usize> =
            code.iter().enumerate().filter(|(_, line)| line.contains("check-common.ps1")).map(|(i, _)| i).collect();
        assert_eq!(named.len(), 1, "{name} names check-common.ps1 on {} code lines, not one", named.len());
        let at = named[0];
        assert_eq!(code[at], source, "{name} does not dot-source check-common.ps1 by its absolute path");
        let preference = code
            .iter()
            .position(|line| *line == "$ErrorActionPreference = 'Stop'")
            .unwrap_or_else(|| panic!("{name} sets no error preference"));
        assert_eq!(at, preference + 1, "{name} dot-sources check-common.ps1 elsewhere than right after its preference");
        assert!(
            code.contains(&"$failures = New-Object System.Collections.ArrayList"),
            "{name} does not create the list Add-Failure fills"
        );
        for function in functions {
            let definition = format!("function {}", function.to_ascii_lowercase());
            assert!(
                !code.iter().any(|line| line.to_ascii_lowercase().contains(&definition)),
                "{name} still defines {function}"
            );
            let first = code
                .iter()
                .position(|line| line.contains(function))
                .unwrap_or_else(|| panic!("{name} never calls {function}"));
            assert!(first > at, "{name} calls {function} before it dot-sources check-common.ps1");
        }
    }
    #[cfg(windows)]
    {
        let errors = parse_errors(&path);
        assert!(errors.is_empty(), "{errors}");
    }
    #[cfg(not(windows))]
    let _ = path;
}

/// Ruling T6-R12 (pull request 6's Task 5): audio-levels.ps1's two optional roots are declared empty in its param
/// block and, on the two lines right after the dot-source of env.ps1 (which sets $Recordings and $TuningData), each
/// replaces its variable only when given; nothing else assigns either variable or reads either root, so without them
/// the script behaves as before. Read on every system, parsed on Windows; never run.
#[test]
fn audio_levels_takes_its_two_roots_only_when_given() {
    let (path, text) = phase0_script("audio-levels.ps1");
    assert!(text.is_ascii(), "Windows PowerShell 5.1 reads a file without a byte order mark in the system code page");
    assert!(text.contains("T6-R12"), "the header names the ruling");
    let code = code_lines(&text);
    let start = code.iter().position(|line| *line == "param(").expect("a param block");
    let end = start + code[start..].iter().position(|line| *line == ")").expect("the param block ends");
    for declared in ["[string]$RecordingsRoot = ''", "[string]$TuningRoot = ''"] {
        assert!(code[start..end].iter().any(|line| line.trim_end_matches(',') == declared), "param lacks {declared}");
    }
    let source = code.iter().position(|line| *line == r". C:\lazymkv\src\scripts\phase0\env.ps1").expect("env.ps1");
    assert!(source > end, "env.ps1 is dot-sourced after the param block");
    let replaces =
        ["if ($RecordingsRoot) { $Recordings = $RecordingsRoot }", "if ($TuningRoot) { $TuningData = $TuningRoot }"];
    assert_eq!(code[source + 1..source + 3], replaces, "the two roots replace their variables right after env.ps1");
    for line in &code {
        if replaces.contains(line) {
            continue;
        }
        for name in ["$Recordings =", "$TuningData =", "$RecordingsRoot", "$TuningRoot"] {
            assert!(!line.contains(name) || line.starts_with("[string]$"), "audio-levels.ps1 also uses {name}: {line}");
        }
    }
    #[cfg(windows)]
    {
        let errors = parse_errors(&path);
        assert!(errors.is_empty(), "{errors}");
    }
    #[cfg(not(windows))]
    let _ = path;
}
