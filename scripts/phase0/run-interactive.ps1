# Starts a phase 0 script in the owner's logged-on desktop session, elevated, and returns at once.
# Saved share credentials and the optical drive are available there. Poll the log for PHASE0-DONE
# (scripts/phase0/wait.sh does that). ArgLine must not contain quotes of either kind.
# The log's last line is "PHASE0-DONE status=ok|error exit=N". status says whether the script threw;
# exit is only the exit code of the last native command it ran, which is not a verdict.
param(
    [Parameter(Mandatory)][string]$Script,
    [string]$ArgLine = '',
    [Parameter(Mandatory)][string]$Log
)
$ErrorActionPreference = 'Stop'
$full = "C:\lazymkv\src\scripts\phase0\$Script"
if (-not (Test-Path $full)) { throw "no such script: $full" }
$task = Get-ScheduledTask -TaskName 'lazymkv-phase0' -ErrorAction SilentlyContinue
if ($task -and $task.State -in 'Running', 'Queued') {
    throw 'a phase 0 job is still running; wait for it to finish before starting another (a hung job is ended with: Stop-ScheduledTask -TaskName lazymkv-phase0)'
}
New-Item -ItemType Directory -Force (Split-Path $Log) | Out-Null
if (Test-Path $Log) { Remove-Item $Log }
# The log is written as UTF-8: a plain *> redirect writes UTF-16 in PowerShell 5.1, which findstr (wait.sh) cannot search.
# try/catch/finally: a script that throws still gets its error and the PHASE0-DONE line into the log.
$inner = "`$status = 'ok'; try { & '$full' $ArgLine *>&1 | Out-File -FilePath '$Log' -Encoding utf8 } " +
    "catch { `$status = 'error'; Add-Content -Path '$Log' -Value ('ERROR: ' + `$_) } " +
    "finally { Add-Content -Path '$Log' -Value ('PHASE0-DONE status=' + `$status + ' exit=' + `$LASTEXITCODE) }"
# -NonInteractive: a missing mandatory parameter fails the job (status=error) instead of waiting at a hidden prompt.
$action = New-ScheduledTaskAction -Execute 'powershell.exe' `
    -Argument "-NoProfile -NonInteractive -WindowStyle Hidden -ExecutionPolicy Bypass -Command `"$inner`""
$principal = New-ScheduledTaskPrincipal -UserId (whoami) -LogonType Interactive -RunLevel Highest
# -ErrorAction Stop is explicit so that a failed registration or start can never be followed by "started".
Register-ScheduledTask -TaskName 'lazymkv-phase0' -Action $action -Principal $principal -Force -ErrorAction Stop | Out-Null
Start-ScheduledTask -TaskName 'lazymkv-phase0' -ErrorAction Stop
"started $Script; log: $Log"
