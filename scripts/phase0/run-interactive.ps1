# Starts a phase 0 script in the owner's logged-on desktop session, elevated, and returns at once.
# Saved share credentials and the optical drive are available there. Poll the log for PHASE0-DONE.
# ArgLine must not contain double quotes; use single quotes around values with spaces.
param(
    [Parameter(Mandatory)][string]$Script,
    [string]$ArgLine = '',
    [Parameter(Mandatory)][string]$Log
)
$full = "C:\lazymkv\src\scripts\phase0\$Script"
if (-not (Test-Path $full)) { throw "no such script: $full" }
New-Item -ItemType Directory -Force (Split-Path $Log) | Out-Null
Remove-Item $Log -ErrorAction SilentlyContinue
# The log is written as UTF-8: a plain *> redirect writes UTF-16 in PowerShell 5.1, which findstr (wait.sh) cannot search.
$inner = "& '$full' $ArgLine *>&1 | Out-File -FilePath '$Log' -Encoding utf8; Add-Content -Path '$Log' -Value ('PHASE0-DONE exit=' + `$LASTEXITCODE)"
$action = New-ScheduledTaskAction -Execute 'powershell.exe' `
    -Argument "-NoProfile -WindowStyle Hidden -ExecutionPolicy Bypass -Command `"$inner`""
$principal = New-ScheduledTaskPrincipal -UserId (whoami) -LogonType Interactive -RunLevel Highest
Register-ScheduledTask -TaskName 'lazymkv-phase0' -Action $action -Principal $principal -Force | Out-Null
Start-ScheduledTask -TaskName 'lazymkv-phase0'
"started $Script; log: $Log"
