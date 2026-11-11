# The run of the release zip's OCR (optical character recognition) worker over one row of the OCR trial on the mini
# PC (pull request 6's notes, calls 11 and 12 (c); Ruling T6-R15; design part 10.9). It runs in the owner's desktop
# session with the engine stopped (lazymkvctl stop), since it reads the share, so it lives under scripts\phase0\, the
# only folder run-interactive.ps1 runs, and its output is the log C:\lazymkv\phase0\logs\<log name>.txt.
# Usage: scripts/phase0/job.sh worker-check.ps1 <log name> -Row <1..6> -WorkerSha256 <hex> [-Bin <dir>] [-Work <dir>]
# job.sh passes the arguments unquoted, so no value may hold a space or a quote; every path has a default.
# -Bin (default C:\lazymkv\tmp\pr6\bin) holds a copy of the mini PC's lazymkv-dev.exe built with ocr-trial from the
# pull request's head and, beside it, the zip's lazymkv-ocr.exe, so lazymkv-dev ocr-trial read starts the zip's worker
# through run_worker_limited, under the memory limit. -WorkerSha256 is the worker's SHA-256 (Secure Hash Algorithm
# 256) as the controller read it from the zip; the run goes ahead only when the worker in -Bin has it.
# The row's slug, label and stream are line <row> of C:\lazymkv\phase0\trial-tracks.txt, as engine-ocr-trial.ps1
# reads them. The row's evidence.mks is found by lazymkv-dev ocr-trial inventory, copied into -Work before the run
# and deleted after it. A background job samples PeakPagedMemorySize64 of the process named lazymkv-ocr every 500 ms
# and keeps the largest value. The records are compared line by line with the trial's kept arm R output
# ($TuningData\<slug>\<label>\subs-ocr-engine\sub-<n>.ocr.jsonl) once each line's last member, "ms" (the run's own
# timings), is removed. The script prints, in this order, then "RESULT ok" or "RESULT FAILED: <reason>":
#   row <r>: worker sha256 equal | different
#   row <r>: events <n>, wall <s> s, end <the read line's end word>
#   row <r>: peak committed <MiB> MiB
#   row <r>: records equal | different (<n> lines here, <n> kept, <d> lines differ) | not compared (no kept output)
# A line whose part did not run says "not run". A worker SHA-256 that differs, a read that does not end Read, a peak
# of 1 GiB or more, row 4's wall time above 600 s (P26) and records that differ fail the run; missing kept output
# does not (call 11). The lines name the row only, never a slug, a label, a share or a path, and no subtitle text.
# It writes only below -Work (default C:\lazymkv\tmp\pr6\worker-<row>), which must not exist; the script creates it
# and always removes it. It never writes into the trial's kept output, the recordings or the tuning dataset.
# The last line is "RESULT ok" or "RESULT FAILED: <reason>"; after a failure the script throws (wait.sh exits 1).
param(
    [Parameter(Mandatory)][int]$Row,
    [Parameter(Mandatory)][string]$WorkerSha256,
    [string]$Bin = 'C:\lazymkv\tmp\pr6\bin',
    [string]$Work = ''
)
$ErrorActionPreference = 'Stop'

$ClearMax = 64
$PeakLimit = 1GB
$Row4LimitS = 600
$Utf8 = New-Object System.Text.UTF8Encoding($false)
$Invariant = [Globalization.CultureInfo]::InvariantCulture

$failures = New-Object System.Collections.ArrayList
function Add-Failure([string]$Reason) { [void]$failures.Add($Reason) }

# A reason this script wrote itself; it never names a slug, a label or a path.
function Stop-Part([string]$Reason) { throw "check: $Reason" }

# The reason to print for an error: this script's own text, or else the error's type and line only, since a system
# error's message may name a path.
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

# Copied from engine-ocr-trial.ps1 (each script runs alone): the six rows of trial-tracks.txt; a line of another form
# stops the run, naming its number only.
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

# Copied from engine-ocr-trial.ps1 (design 10.9 step 3: nothing else runs while an arm is timed), with lazymkv-ocr
# added: a worker left running would be sampled with this run's.
function Assert-Quiet {
    $names = 'lazymkv-engine', 'lazymkv-ocr', 'makemkvcon64', 'makemkvcon', 'cargo', 'rustc', 'ocrs-trial'
    $busy = @(Get-Process -Name $names -ErrorAction SilentlyContinue)
    if ($busy.Count -gt 0) {
        $running = ($busy | ForEach-Object { $_.ProcessName } | Sort-Object -Unique) -join ', '
        Stop-Part "these run, so nothing is timed: $running; stop the engine with lazymkvctl stop and wait for the rest"
    }
}

# Copied from engine-ocr-trial.ps1: copies one file and checks its size.
function Copy-Checked([string]$From, [string]$To, [string]$What) {
    Copy-Item -LiteralPath $From -Destination $To -ErrorAction Stop
    if ((Get-Item -LiteralPath $To).Length -ne (Get-Item -LiteralPath $From).Length) {
        Stop-Part "the copy of $What differs in size from its source"
    }
}

# A records file's lines read as UTF-8, each with its last member "ms" (the run's own timings) removed (T6-R15).
function Get-RecordLines([string]$Path) {
    foreach ($line in [IO.File]::ReadAllLines($Path, $Utf8)) { $line -replace ',"ms":\{[^{}]*\}\}$', '}' }
}

$shaLine = "row ${Row}: worker sha256 not run"
$runLine = "row ${Row}: events not run"
$peakLine = "row ${Row}: peak committed not run"
$recordsLine = "row ${Row}: records not run"
$made = $false
$sampler = $null
$copy = $null
$hadThreads = Test-Path Env:RTEN_NUM_THREADS
$savedThreads = $env:RTEN_NUM_THREADS
try {
    if ($Row -lt 1 -or $Row -gt 6) { Stop-Part '-Row must be 1 to 6' }
    if ($WorkerSha256 -notmatch '^[0-9a-fA-F]{64}$') { Stop-Part '-WorkerSha256 must be 64 hexadecimal digits' }
    if (-not $Work) { $Work = "C:\lazymkv\tmp\pr6\worker-$Row" }
    . C:\lazymkv\src\scripts\phase0\env.ps1
    Assert-Quiet
    if (Test-Path -LiteralPath $Work) { Stop-Part 'the -Work folder exists already; remove it or name another' }
    $dev = "$Bin\lazymkv-dev.exe"
    $worker = "$Bin\lazymkv-ocr.exe"
    foreach ($exe in @($CAP, $dev, $worker)) {
        if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) {
            Stop-Part "$([IO.Path]::GetFileName($exe)) is missing"
        }
    }
    New-Item -ItemType Directory -Path $Work -ErrorAction Stop | Out-Null
    $made = $true

    # The worker is the zip's.
    $same = (Get-FileHash -LiteralPath $worker -Algorithm SHA256).Hash -ieq $WorkerSha256
    $shaLine = "row ${Row}: worker sha256 $(if ($same) { 'equal' } else { 'different' })"
    if (-not $same) { Stop-Part 'the worker in -Bin is not the zip''s (its SHA-256 differs), so it was not run' }

    # The row's inputs, as engine-ocr-trial.ps1's read step finds them.
    $track = @(Get-TrialRows)[$Row - 1]
    $n = $track.stream
    $rec = "$Recordings\$($track.slug)\$($track.label)"
    $text = "$TuningData\$($track.slug)\$($track.label)\subs-ocr"
    $engineOut = "$TuningData\$($track.slug)\$($track.label)\subs-ocr-engine"
    $inventoryArgs = @('ocr-trial', 'inventory', '--rec', $rec, '--text', $text, '--stream', [string]$n,
        '--staging', $Staging, '--where', "$Work\where.txt")
    $code = Invoke-Recorded "$Work\inventory.txt" $dev $inventoryArgs
    if ($code -ne 0) { Stop-Part "the inventory of the row's inputs exited $code (B2)" }
    $source = [string](@([IO.File]::ReadAllLines("$Work\where.txt", $Utf8)) | Select-Object -First 1)
    if (-not $source) { Stop-Part 'the inventory named no evidence.mks' }
    $copy = "$Work\evidence.mks"
    Copy-Checked $source $copy 'evidence.mks'

    # The sampler: the largest PeakPagedMemorySize64 of any process named lazymkv-ocr, every 500 ms, until the stop
    # file appears. It writes its ready file first, so the read starts only once it samples.
    $ready = "$Work\sampler.ready"
    $stop = "$Work\sampler.stop"
    $sampler = Start-Job -ArgumentList $ready, $stop -ScriptBlock {
        param($Ready, $Stop)
        $peak = [long]0
        [IO.File]::WriteAllText($Ready, 'ready')
        while ($true) {
            foreach ($p in @(Get-Process -Name 'lazymkv-ocr' -ErrorAction SilentlyContinue)) {
                try { if ($p.PeakPagedMemorySize64 -gt $peak) { $peak = [long]$p.PeakPagedMemorySize64 } } catch { }
            }
            if (Test-Path -LiteralPath $Stop) { break }
            Start-Sleep -Milliseconds 500
        }
        $peak
    }
    $deadline = (Get-Date).AddSeconds(60)
    while (-not (Test-Path -LiteralPath $ready)) {
        if ($sampler.State -in 'Failed', 'Stopped', 'Completed') {
            Stop-Part 'the memory sampler ended before it started'
        }
        if ((Get-Date) -gt $deadline) { Stop-Part 'the memory sampler did not start within 60 s' }
        Start-Sleep -Milliseconds 200
    }

    # The read, as engine-ocr-trial.ps1's read step runs it, from -Bin and into -Work.
    if (Test-Path Env:RTEN_NUM_THREADS) { Remove-Item Env:RTEN_NUM_THREADS }
    $readArgs = @('ocr-trial', 'read', $copy, '--stream', [string]$n, '--clear-max', [string]$ClearMax,
        '--packets', "$rec\subs\sub-$n.packets.json", '--out', "$Work\r")
    $readExit = Invoke-Recorded "$Work\read.txt" $dev $readArgs
    [IO.File]::WriteAllText($stop, 'stop')
    Wait-Job -Job $sampler -Timeout 30 | Out-Null
    $peak = [long](@(Receive-Job -Job $sampler) | Select-Object -Last 1)
    Remove-Item -LiteralPath $copy -Force -ErrorAction Stop
    $copy = $null

    # events, wall and end from the read's own lines (numbers and the end word only).
    $events = '-'
    $wall = '-'
    $end = '-'
    if (Test-Path -LiteralPath "$Work\read.txt" -PathType Leaf) {
        foreach ($line in [IO.File]::ReadAllLines("$Work\read.txt", $Utf8)) {
            if ($line -cmatch '^events (\d+) cap ') { $events = $Matches[1] }
            elseif ($line -cmatch '^wall (\d+(\.\d+)?) s ') { $wall = $Matches[1] }
            elseif ($line -cmatch '^end (not started|[A-Za-z]+)') { $end = $Matches[1] }
        }
    }
    $runLine = "row ${Row}: events $events, wall $wall s, end $end"
    if ($readExit -ne 0 -or $end -cne 'Read') { Add-Failure "row ${Row}: the read did not end Read (exit $readExit)" }
    if ($Row -eq 4) {
        if ($wall -eq '-') { Add-Failure 'row 4: the read printed no wall time' }
        elseif ([double]::Parse($wall, $Invariant) -gt $Row4LimitS) {
            Add-Failure "row 4: wall $wall s is above 600 s (P26)"
        }
    }

    # The peak.
    if ($peak -gt 0) {
        $mib = ($peak / 1MB).ToString('0.0', $Invariant)
        $peakLine = "row ${Row}: peak committed $mib MiB"
        if ($peak -ge $PeakLimit) { Add-Failure "row ${Row}: the worker's peak committed memory is 1 GiB or more (A2)" }
    } else {
        Add-Failure "row ${Row}: no sample of lazymkv-ocr was taken"
    }

    # The records against the trial's kept arm R output, which is only read.
    $here = "$Work\r\sub-$n.ocr.jsonl"
    $kept = "$engineOut\sub-$n.ocr.jsonl"
    if (-not (Test-Path -LiteralPath $kept -PathType Leaf)) {
        $recordsLine = "row ${Row}: records not compared (no kept output)"
    } elseif (-not (Test-Path -LiteralPath $here -PathType Leaf)) {
        $recordsLine = "row ${Row}: records not compared (no records here)"
        Add-Failure "row ${Row}: the read wrote no records"
    } else {
        $one = @(Get-RecordLines $here)
        $two = @(Get-RecordLines $kept)
        $differ = 0
        $count = [math]::Max($one.Count, $two.Count)
        for ($i = 0; $i -lt $count; $i++) {
            if ($i -ge $one.Count -or $i -ge $two.Count -or $one[$i] -cne $two[$i]) { $differ++ }
        }
        $word = if ($differ -eq 0) { 'equal' } else { 'different' }
        $recordsLine = "row ${Row}: records $word ($($one.Count) lines here, $($two.Count) kept, $differ lines differ)"
        if ($differ -gt 0) { Add-Failure "row ${Row}: the records differ from the trial's" }
    }
} catch {
    Add-Failure "row ${Row}: $(Get-Reason $_)"
} finally {
    if ($sampler) {
        try { Stop-Job -Job $sampler -ErrorAction SilentlyContinue } catch { }
        try { Remove-Job -Job $sampler -Force -ErrorAction SilentlyContinue } catch { }
    }
    if ($hadThreads) { $env:RTEN_NUM_THREADS = $savedThreads }
    elseif (Test-Path Env:RTEN_NUM_THREADS) { Remove-Item Env:RTEN_NUM_THREADS }
    if ($copy -and (Test-Path -LiteralPath $copy)) {
        try { Remove-Item -LiteralPath $copy -Force -ErrorAction Stop } catch { }
    }
    if ($made) {
        try { Remove-Item -LiteralPath $Work -Recurse -Force -ErrorAction Stop } catch { }
        if (Test-Path -LiteralPath $Work) { Add-Failure "row ${Row}: the -Work folder could not be removed" }
    }
}
$shaLine
$runLine
$peakLine
$recordsLine
if ($failures.Count -eq 0) {
    'RESULT ok'
} else {
    "RESULT FAILED: $($failures -join '; ')"
    throw 'worker-check.ps1 failed (the RESULT line says why)'
}
