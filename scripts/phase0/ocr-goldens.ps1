# The decoder goldens of design part 10.4 (pull request 5's Task 11; notes calls 7 and 24): runs the built test
# program of crates/ocr/tests/goldens.rs in the owner's desktop session on the mini PC, the only session that reaches
# the share, so it lives under scripts\phase0\, the only folder run-interactive.ps1 runs, and its output is the log
# C:\lazymkv\phase0\logs\<log name>.txt. It never builds: build the program first over SSH with
# scripts/box.sh cargo test -p lazymkv-ocr --locked --release --test goldens --no-run, whose Executable line names it,
# and run no scripts/box.sh until this job ends, since each sync replaces C:\lazymkv\src.
# The program reads the six trial tracks from C:\lazymkv\phase0\trial-tracks.txt (outside the repository, one
# <slug>/<label>/s<n> line per row of design part 10.9's table, T5-R45) and prints row numbers and counts only.
# Usage: scripts/phase0/job.sh ocr-goldens.ps1 <log name> -Exe C:\lazymkv\target\release\deps\goldens-<hash>.exe
# The last line is "RESULT ok" or "RESULT FAILED: <reason>"; after a failure the script throws.
param(
    [Parameter(Mandatory)][string]$Exe
)
. C:\lazymkv\src\scripts\phase0\env.ps1
$Tracks = 'C:\lazymkv\phase0\trial-tracks.txt'
try {
    if ($Exe -notlike 'C:\lazymkv\target\release\deps\goldens-*.exe') { throw '-Exe must be the goldens test program, C:\lazymkv\target\release\deps\goldens-<hash>.exe' }
    if (-not (Test-Path -LiteralPath $Exe)) { throw "missing $Exe; build it over SSH first, as the comment at the top says" }
    if (-not (Test-Path -LiteralPath $Tracks)) { throw "missing $Tracks, one <slug>/<label>/s<n> line per row of design part 10.9's table" }
    # The goldens read these; without any of the first three each test prints "skipped:" and passes. Staging is the
    # last place an evidence file is looked for (R6-phase0-20).
    $env:LAZYMKV_EVIDENCE = $Recordings
    $env:LAZYMKV_OCR_TEXT = $TuningData
    $env:LAZYMKV_OCR_TRACKS = $Tracks
    $env:LAZYMKV_OCR_STAGING = $Staging
    # cmd joins the program's standard error to its standard output, so each skipped: line reaches the log, and
    # Windows PowerShell 5.1 never turns a native standard-error line into an error record.
    cmd /c "$Exe --test-threads=1 --nocapture 2>&1"
    if ($LASTEXITCODE -ne 0) { throw "the goldens exited $LASTEXITCODE" }
    'RESULT ok'
} catch {
    "RESULT FAILED: $($_.Exception.Message)"
    throw
}
