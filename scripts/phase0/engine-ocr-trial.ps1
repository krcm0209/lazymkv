# The OCR (optical character recognition) trial's desktop-session steps (design part 10.9; pull request 5, Tasks 10
# and 12; the notes' B2 and calls 8 and 19; Rulings T5-R24, T5-R25, T5-R44, T5-R45, T5-R62). One run does one step for
# one row of design 10.9's table, named by its row number only: the row's slug, label and stream are line <row> of
# C:\lazymkv\phase0\trial-tracks.txt (git-ignored, outside the repository: six lines <slug>/<label>/s<n> in row order,
# Task 11), and -Slug, -Label and -Stream, when given, must equal them. The log names rows, never a slug or a label;
# a system error's message may name a path, so an evidence line quotes only the row and the step. Subtitle text stays
# on the share and in this script's work folder on C:, which every run deletes, whatever happened.
#   inventory  step 1: lazymkv-dev ocr-trial inventory (counts of frames, Tesseract records, arm F's reading,
#              evidence.mks and events); a missing input stops the trial and goes to the owner (B2)
#   read       step 3, arm R: evidence.mks copied to C: before timing, lazymkv-dev ocr-trial read (the sibling
#              lazymkv-ocr.exe through the native guard, below normal, RTEN_NUM_THREADS=3, the engine's cap), the copy
#              deleted; the records, run.json and crops copied to $TuningData\<slug>\<label>\subs-ocr-engine\ and its
#              numbers-only summary.json written
#   rows       step 3, arm R': phase 0's frames copied to C:, lazymkv-dev ocr-trial rows with RTEN_NUM_THREADS=3, the
#              records copied to subs-ocr-engine\rows-s<n>.jsonl
#   compare    step 4, once per title under its first row (rows 4 and 5 run as -Row 4): the records of arms F and R'
#              given their times by f-records, then parity of F against R (the verdict's half) and of F against R'
#              (reported only)
#   f3         F3 (report only, T5-R25): retune --subtitles over arm R's text, through a tree on C: deleted afterwards
# Build first over SSH: scripts/box.sh "cargo build --release --locked -p lazymkv-ocr -p lazymkv-dev --features
# lazymkv-dev/ocr-trial"; this script builds nothing. Stop the engine (lazymkvctl stop) before read and rows.
# Usage: scripts/phase0/job.sh engine-ocr-trial.ps1 <log name> -Step <step> -Row <k> [-Slug <slug> -Label <label> -Stream <n>]
#        scripts/phase0/job.sh engine-ocr-trial.ps1 <log name> -Step f3 [-Evidence <dir>]
# The capture files of each run (numbers on standard output; standard error may name a path) stay in
# C:\lazymkv\phase0\tmp\engine-ocr\logs\<step>-<row>\. The last line is "RESULT ok" or "RESULT FAILED: <reason>";
# after a failure the script throws.
param(
    [Parameter(Mandatory)][ValidateSet('inventory', 'read', 'rows', 'compare', 'f3')][string]$Step,
    [int]$Row = 0,
    [string]$Slug = '',
    [string]$Label = '',
    [int]$Stream = -1,
    [string]$Evidence = ''
)
. C:\lazymkv\src\scripts\phase0\env.ps1

$DEV = 'C:\lazymkv\target\release\lazymkv-dev.exe'
$WORKER = 'C:\lazymkv\target\release\lazymkv-ocr.exe'
$Tracks = "$P0\trial-tracks.txt"
$Spec = 'C:\lazymkv\src\docs\superpowers\specs\2026-10-04-lazymkv-design.md'
$ClearMax = 64
$MaxBytes = 2GB

# The six rows of trial-tracks.txt; a line of another form stops the run, naming its number only.
function Get-TrialRows {
    if (-not (Test-Path -LiteralPath $Tracks)) { throw "missing $Tracks (Task 11 writes it)" }
    $lines = @(Get-Content -LiteralPath $Tracks -Encoding UTF8 -ErrorAction Stop | ForEach-Object { $_.Trim() } | Where-Object { $_ -ne '' })
    if ($lines.Count -ne 6) { throw "$Tracks holds $($lines.Count) rows, not 6" }
    $rows = @()
    for ($k = 0; $k -lt 6; $k++) {
        $m = [regex]::Match($lines[$k], '^([a-z0-9][a-z0-9-]*)/([a-z0-9][a-z0-9-]*)/s(\d+)$')
        if (-not $m.Success) { throw "line $($k + 1) of $Tracks is not of the form name/name/s<n>" }
        $rows += [pscustomobject]@{ row = $k + 1; slug = $m.Groups[1].Value; label = $m.Groups[2].Value; stream = [int]$m.Groups[3].Value }
    }
    $rows
}

# Design 10.9 step 3: nothing else runs while an arm is timed.
function Assert-Quiet {
    $busy = @(Get-Process -Name 'lazymkv-engine', 'makemkvcon64', 'makemkvcon', 'cargo', 'rustc', 'ocrs-trial' -ErrorAction SilentlyContinue)
    if ($busy.Count -gt 0) {
        throw "these run, so nothing is timed: $(($busy | ForEach-Object { $_.ProcessName } | Sort-Object -Unique) -join ', '); stop the engine with lazymkvctl stop and wait for the rest"
    }
}

# lazymkv-dev ocr-trial through lazymkv-capture: its standard output reaches the log and $Logs\<Name>.txt, its
# standard error only $Logs\<Name>.txt.stderr. Sets $TrialExit.
function Invoke-Trial([string]$Name, [string[]]$Arguments) {
    & $CAP record --out "$Logs\$Name.txt" -- $DEV ocr-trial @Arguments
    $script:TrialExit = $LASTEXITCODE
}

# Copies one file and checks its size.
function Copy-Checked([string]$From, [string]$To, [string]$What) {
    Copy-Item -LiteralPath $From -Destination $To -ErrorAction Stop
    if ((Get-Item -LiteralPath $To).Length -ne (Get-Item -LiteralPath $From).Length) { throw "the copy of $What differs in size from its source" }
}

$work = $null
try {
    foreach ($exe in @($CAP, $DEV, $WORKER)) { if (-not (Test-Path -LiteralPath $exe)) { throw "missing $exe; build it first (see the header)" } }
    $all = Get-TrialRows
    $Logs = "$P0\tmp\engine-ocr\logs\$Step-$Row"
    if (Test-Path -LiteralPath $Logs) { Remove-Item -LiteralPath $Logs -Recurse -Force -ErrorAction Stop }
    New-Item -ItemType Directory -Force -Path $Logs -ErrorAction Stop | Out-Null
    $work = "$P0\tmp\engine-ocr\work-$Step-$Row"
    if (Test-Path -LiteralPath $work) { Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction Stop }
    New-Item -ItemType Directory -Force -Path $work -ErrorAction Stop | Out-Null
    if (Test-Path Env:RTEN_NUM_THREADS) { Remove-Item Env:RTEN_NUM_THREADS }

    if ($Step -eq 'f3') {
        # T5-R25: <tmp>\<slug>\<label>\subs-ocr\sub-<n>.ocr.jsonl holds copies of arm R's records.
        foreach ($t in $all) {
            $f3Dir = "$work\$($t.slug)\$($t.label)\subs-ocr"
            New-Item -ItemType Directory -Force -Path $f3Dir -ErrorAction Stop | Out-Null
            $engineOut = "$TuningData\$($t.slug)\$($t.label)\subs-ocr-engine"
            Copy-Checked "$engineOut\sub-$($t.stream).ocr.jsonl" "$f3Dir\sub-$($t.stream).ocr.jsonl" "row $($t.row)'s arm R records"
        }
        $source = if ($Evidence) { $Evidence } else { $Recordings }
        "f3: retune --subtitles over arm R's text of rows 1 to 6"
        & $CAP record --out "$Logs\retune.txt" -- $DEV retune --subtitles --evidence $source --text $work --spec $Spec
        "retune exit $LASTEXITCODE"
    } else {
        if ($Row -lt 1 -or $Row -gt 6) { throw "-Row must be 1 to 6 for the step $Step" }
        $track = $all[$Row - 1]
        if ($Slug -and $Slug -cne $track.slug) { throw "row ${Row}: -Slug differs from line $Row of trial-tracks.txt" }
        if ($Label -and $Label -cne $track.label) { throw "row ${Row}: -Label differs from line $Row of trial-tracks.txt" }
        if ($Stream -ge 0 -and $Stream -ne $track.stream) { throw "row ${Row}: -Stream differs from line $Row of trial-tracks.txt" }
        $n = $track.stream
        $rec = "$Recordings\$($track.slug)\$($track.label)"
        $text = "$TuningData\$($track.slug)\$($track.label)\subs-ocr"
        $engineOut = "$TuningData\$($track.slug)\$($track.label)\subs-ocr-engine"
        $inventoryArgs = @('inventory', '--rec', $rec, '--text', $text, '--stream', [string]$n, '--staging', $Staging, '--where', "$work\where.txt")
        "row ${Row}: step $Step, stream s$n"

        if ($Step -eq 'inventory') {
            Invoke-Trial 'inventory' $inventoryArgs
            if ($TrialExit -ne 0) { throw "row ${Row}: an input is missing (B2); the owner decides whether the trial stands without this row" }
        }

        if ($Step -eq 'read') {
            Assert-Quiet
            if ((Test-Path -LiteralPath "$engineOut\sub-$n.ocr.jsonl") -or (Test-Path -LiteralPath "$engineOut\s$n")) {
                throw "row ${Row}: subs-ocr-engine already holds stream s$n; the trial reads each track once (design 10.9)"
            }
            Invoke-Trial 'inventory' $inventoryArgs
            if ($TrialExit -ne 0) { throw "row ${Row}: an input is missing (B2)" }
            $source = Get-Content -LiteralPath "$work\where.txt" -Encoding UTF8 -ErrorAction Stop | Select-Object -First 1
            $copy = "$work\evidence.mks"
            Copy-Checked $source $copy 'evidence.mks'
            "row ${Row}: evidence.mks copied to C: before timing, $((Get-Item -LiteralPath $copy).Length) bytes"
            Invoke-Trial 'read' @('read', $copy, '--stream', [string]$n, '--clear-max', [string]$ClearMax, '--packets', "$rec\subs\sub-$n.packets.json", '--out', "$work\r")
            Remove-Item -LiteralPath $copy -Force -ErrorAction Stop
            "row ${Row}: the copy of evidence.mks deleted"
            if ($TrialExit -ne 0) { throw "row ${Row}: arm R did not read the stream (its end line says why); a kill fails the trial (T5-R44)" }
            New-Item -ItemType Directory -Force -Path $engineOut -ErrorAction Stop | Out-Null
            foreach ($name in @("sub-$n.ocr.jsonl", "sub-$n.run.json")) { Copy-Checked "$work\r\$name" "$engineOut\$name" $name }
            Copy-Item -LiteralPath "$work\r\s$n" -Destination "$engineOut\s$n" -Recurse -ErrorAction Stop
            $crops = @(Get-ChildItem -LiteralPath "$work\r\s$n" -Filter 'f-*.png' -File -ErrorAction Stop).Count
            $copied = @(Get-ChildItem -LiteralPath "$engineOut\s$n" -Filter 'f-*.png' -File -ErrorAction Stop).Count
            if ($copied -ne $crops) { throw "row ${Row}: $copied of $crops crops were copied" }
            Invoke-Trial 'summary' @('summary', '--dir', $engineOut, '--rec', $rec)
            if ($TrialExit -ne 0) { throw "row ${Row}: summary.json was not written" }
            "row ${Row}: arm R copied to subs-ocr-engine: sub-$n.ocr.jsonl, sub-$n.run.json, $crops crops, summary.json"
        }

        if ($Step -eq 'rows') {
            Assert-Quiet
            $frames = @(Get-ChildItem -LiteralPath "$text\s$n" -Filter 'f-*.png' -File -ErrorAction SilentlyContinue | Sort-Object Name)
            if ($frames.Count -eq 0) {
                "row ${Row}: R' not run (no frame set, B2)"
            } else {
                if (Test-Path -LiteralPath "$engineOut\rows-s$n.jsonl") { throw "row ${Row}: subs-ocr-engine already holds rows-s$n.jsonl; the trial reads each track once (design 10.9)" }
                $bytes = [long]($frames | Measure-Object -Property Length -Sum).Sum
                if ($bytes -gt $MaxBytes) { throw "row ${Row}: the frames hold $bytes bytes, more than the 2 GB the copy may put on C:" }
                New-Item -ItemType Directory -Force -Path "$work\frames" -ErrorAction Stop | Out-Null
                foreach ($f in $frames) { Copy-Item -LiteralPath $f.FullName -Destination "$work\frames" -ErrorAction Stop }
                $copied = @(Get-ChildItem -LiteralPath "$work\frames" -Filter 'f-*.png' -File -ErrorAction Stop).Count
                if ($copied -ne $frames.Count) { throw "row ${Row}: $copied of $($frames.Count) frames were copied to C:" }
                "row ${Row}: $($frames.Count) frames copied to C:, $bytes bytes; RTEN_NUM_THREADS=3 for arm R'"
                $env:RTEN_NUM_THREADS = '3'
                try { Invoke-Trial 'rows' @('rows', "$work\frames", '--out', "$work\rows-s$n.jsonl") }
                finally { Remove-Item Env:RTEN_NUM_THREADS -ErrorAction SilentlyContinue }
                if ($TrialExit -ne 0) { throw "row ${Row}: arm R' stopped (exit $TrialExit)" }
                New-Item -ItemType Directory -Force -Path $engineOut -ErrorAction Stop | Out-Null
                Copy-Checked "$work\rows-s$n.jsonl" "$engineOut\rows-s$n.jsonl" "rows-s$n.jsonl"
                "row ${Row}: arm R' copied to subs-ocr-engine: rows-s$n.jsonl"
            }
        }

        if ($Step -eq 'compare') {
            $title = @($all | Where-Object { $_.slug -ceq $track.slug -and $_.label -ceq $track.label })
            $first = ($title | Measure-Object -Property row -Minimum).Minimum
            if ($first -ne $Row) { throw "row $Row is compared with row $first, its title's first row" }
            $shown = ($title | ForEach-Object { $_.row }) -join ' and '
            foreach ($sub in @('f', 'rprime')) { New-Item -ItemType Directory -Force -Path "$work\$sub" -ErrorAction Stop | Out-Null }
            $everyRprime = $true
            foreach ($t in $title) {
                $m = $t.stream
                Invoke-Trial "f-records-$m" @('f-records', '--rec', $rec, '--stream', [string]$m, '--in', "$text\ocrs-s$m.jsonl", '--out', "$work\f\sub-$m.ocr.jsonl")
                if ($TrialExit -ne 0) { throw "row $($t.row): arm F's records could not be given their times (B2)" }
                if (Test-Path -LiteralPath "$engineOut\rows-s$m.jsonl") {
                    Invoke-Trial "rprime-records-$m" @('f-records', '--rec', $rec, '--stream', [string]$m, '--in', "$engineOut\rows-s$m.jsonl", '--out', "$work\rprime\sub-$m.ocr.jsonl")
                    if ($TrialExit -ne 0) { throw "row $($t.row): the records of arm R' could not be given their times" }
                } else {
                    $everyRprime = $false
                    "row $($t.row): R' not run"
                }
            }
            "rows ${shown}: parity of F against R"
            Invoke-Trial 'parity-r' @('parity', '--rec', $rec, '--f', "$work\f", '--r', $engineOut)
            if ($TrialExit -eq 2) { throw "rows ${shown}: parity could not read an arm (parity-r.txt.stderr in the logs folder)" }
            if ($everyRprime) {
                "rows ${shown}: parity of F against R' (reported only)"
                Invoke-Trial 'parity-rprime' @('parity', '--rec', $rec, '--f', "$work\f", '--r', "$work\rprime")
                if ($TrialExit -eq 2) { throw "rows ${shown}: parity could not read arm R' (parity-rprime.txt.stderr in the logs folder)" }
            }
        }
    }
    'RESULT ok'
} catch {
    "RESULT FAILED: $($_.Exception.Message)"
    throw
} finally {
    # No subtitle text stays on C:: the evidence copy, the frames, the arms' records and the worker's capture go.
    if (Test-Path Env:RTEN_NUM_THREADS) { Remove-Item Env:RTEN_NUM_THREADS }
    if ($work -and (Test-Path -LiteralPath $work)) { Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue }
}
