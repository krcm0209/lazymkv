# The helper functions that pull request 6's two sitting checks on the mini PC share (the review of pull request #21
# (X13)): scripts\phase0\ffmpeg-check.ps1 (notes call 12 (a)) and scripts\phase0\worker-check.ps1 (notes call 11)
# each dot-source it once, at the top, right after their error preference is set:
#   . C:\lazymkv\src\scripts\phase0\check-common.ps1
# It only defines functions (Add-Failure, Stop-Part, Get-Reason, Invoke-Recorded and Get-TrialRows), so it prints
# nothing and writes nothing by itself. The functions read the calling script's variables when they are called:
# Add-Failure adds to the $failures list the calling script creates, and Invoke-Recorded and Get-TrialRows read $CAP and
# $P0, which env.ps1 sets. Windows PowerShell 5.1; ASCII only, since it reads a file with no byte order mark in the
# system code page.

function Add-Failure([string]$Reason) { [void]$failures.Add($Reason) }

# A reason the calling script wrote itself; it never names a film, a track, a slug, a label or a path.
function Stop-Part([string]$Reason) { throw "check: $Reason" }

# The reason to print for an error: the calling script's own text, or else the error's type and line only, since a
# system error's message may name a path.
function Get-Reason($Record) {
    $message = [string]$Record.Exception.Message
    if ($message.StartsWith('check: ')) { return $message.Substring(7) }
    $where = [string]$Record.InvocationInfo.ScriptName
    if ($where) { $where = ' of ' + [IO.Path]::GetFileName($where) }
    "an error of type $($Record.Exception.GetType().Name) at line $($Record.InvocationInfo.ScriptLineNumber)$where"
}

# A program's standard output captured as bytes into $Out by lazymkv-capture (its standard error goes to
# $Out.stderr); nothing reaches the log. Returns the program's exit code. A native standard-error line never becomes
# an error record here: the preference is Continue inside this function only.
function Invoke-Recorded([string]$Out, [string]$Program, [string[]]$Arguments) {
    $ErrorActionPreference = 'Continue'
    & $CAP record --out $Out -- $Program @Arguments 2>&1 | Out-Null
    $LASTEXITCODE
}

# The six rows of trial-tracks.txt, read as engine-ocr-trial.ps1 reads them (that script keeps its own copy, which
# words its failures differently); a line of another form stops the run, naming its number only.
function Get-TrialRows {
    $tracks = "$P0\trial-tracks.txt"
    if (-not (Test-Path -LiteralPath $tracks)) { Stop-Part 'trial-tracks.txt is missing' }
    $lines = @(Get-Content -LiteralPath $tracks -Encoding UTF8 -ErrorAction Stop | ForEach-Object { $_.Trim() } |
        Where-Object { $_ -ne '' })
    if ($lines.Count -ne 6) { Stop-Part "trial-tracks.txt holds $($lines.Count) rows, not 6" }
    $rows = @()
    for ($k = 0; $k -lt 6; $k++) {
        $m = [regex]::Match($lines[$k], '^([a-z0-9][a-z0-9-]*)/([a-z0-9][a-z0-9-]*)/s(\d+)$')
        if (-not $m.Success) { Stop-Part "line $($k + 1) of trial-tracks.txt is not of the form name/name/s<n>" }
        $rows += [pscustomobject]@{
            row = $k + 1; slug = $m.Groups[1].Value; label = $m.Groups[2].Value; stream = [int]$m.Groups[3].Value
        }
    }
    $rows
}
