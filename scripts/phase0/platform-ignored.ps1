# Runs one session's lazymkv-platform tests in the owner's desktop session on the mini PC (design part 12.4): the
# DriveWatcher, real-share and Credential Manager tests that SSH cannot run. It is started from the Mac through
# job.sh, so it lives under scripts\phase0\, the only folder run-interactive.ps1 runs, and its output is the log
# C:\lazymkv\phase0\logs\<log name>.txt. Build the test binary first over SSH
# (scripts/box.sh cargo test -p lazymkv-platform --locked --no-run), so this run only starts it, and run no
# scripts/box.sh until the job ends: each sync replaces C:\lazymkv\src.
# Usage: scripts/phase0/job.sh platform-ignored.ps1 <log name> -Filter <module path> [-Drive <id>] [-ShareRoot <UNC folder>]
# job.sh passes the arguments unquoted, so no value may hold a space or a quote.
param(
    [Parameter(Mandatory)][string]$Filter,
    [string]$Drive = '',
    [string]$ShareRoot = ''
)
$ErrorActionPreference = 'Stop'
# As scripts/box.sh sets them for a command over SSH.
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
$env:CARGO_TARGET_DIR = 'C:\lazymkv\target'
# The ignored tests read these. An empty value removes the variable, and a test without its input prints
# "skipped: <reason>" and passes (design part 12.1's convention); the notes record that as not run.
$env:LAZYMKV_TEST_DRIVE = $Drive
$env:LAZYMKV_TEST_SHARE_ROOT = $ShareRoot
# The vault tests write the owner's Credential Manager only in their own job (Owner decision 8).
$env:LAZYMKV_TEST_VAULT = if ($Filter -like 'win::creds::*') { '1' } else { '' }
"platform-ignored: filter $Filter, drive '$Drive', share root '$ShareRoot'"
Set-Location 'C:\lazymkv\src'
# cmd joins the test binary's standard error to its standard output, so each "CUE:" line reaches the log when it is
# written, and Windows PowerShell 5.1 never turns a native standard-error line into an error record.
# --include-ignored also runs the module's tests that are not ignored; --test-threads=1 gives one cue at a time.
cmd /c "cargo test -p lazymkv-platform --locked -- --include-ignored --test-threads=1 --nocapture $Filter 2>&1"
if ($LASTEXITCODE -ne 0) { throw "cargo test exited $LASTEXITCODE" }
