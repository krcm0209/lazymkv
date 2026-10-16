# Experiment 2 (spec section 17, row 2): while a rip call scans the disc, does MakeMKV add each title
# under the index that was asked for, and before saving begins (MSG:5014)?
# For each tested title this script starts "mkv disc:<index> <title> <dir>" with the scan's flags, reads
# MakeMKV's output until the MSG:5014 line, stops the call there and keeps the output in
# $Recordings\<Slug>\exp02\title-<title>.txt, which lazymkv-dev exp02 reads on the Mac.
#   -MaxTitles n  test at most n of the scan's titles, evenly spaced, the first and the last always included
#   -Redo 3,7     test exactly these titles; an earlier title-N.txt is moved to exp02\superseded\<time>\
#   -Plan         check the recording, the disc and MakeMKV, print the titles to test, start nothing
#                 (it needs no share, so it also runs over plain ssh)
# A run without -Redo refuses to start when exp02 already holds title files: they are evidence.
# The calls write into $Staging\<Slug>\exp02-tmp (never C:), emptied before each call and removed at
# the end. A call that writes more than 512 MB, or runs longer than the time limit, without MSG:5014
# is stopped, and the run ends there. From the first MakeMKV call on, each run writes
# exp02\run-<time>.json. The last line is "RESULT ok: ..." or "RESULT FAILED: <reason>"; after a
# failure the script throws, so wait.sh exits 1.
[CmdletBinding()]
param(
    [string]$Slug = '',    # required: Assert-Slug rejects an empty value (a Mandatory prompt would hang a job)
    [int]$MaxTitles = 12,
    [int[]]$Redo = @(),
    [switch]$Plan
)
. C:\lazymkv\src\scripts\phase0\env.ps1

$process = $null
$runFile = $null
$tmpUsed = $false
$results = New-Object System.Collections.Generic.List[object]
$stamp = (Get-Date).ToString('yyyyMMdd-HHmmss')

# Removes the temporary rip folder, retrying while a just-stopped call releases its file.
function Clear-Tmp {
    for ($i = 0; $i -lt 6 -and (Test-Path -LiteralPath $tmp); $i++) {
        Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue
        if (Test-Path -LiteralPath $tmp) { Start-Sleep -Seconds 5 }
    }
}

# Writes this run's summary. It is rewritten after every title, so an interrupted run still leaves one.
function Save-Run([string]$Result) {
    $doc = [ordered]@{
        slug         = $Slug
        started      = $startedAt
        fingerprint  = $fingerprint
        command      = "makemkvcon64 $flags mkv disc:$index <title> $tmp"
        time_limit_s = $limitSeconds
        byte_limit   = $limitBytes
        tested       = @($titles)
        results      = $results.ToArray()
        result       = $Result
    }
    Set-Content -LiteralPath $runFile -Value (ConvertTo-Json $doc -Depth 5) -Encoding UTF8 -ErrorAction Stop
}

try {
    Assert-Slug $Slug
    if ($MaxTitles -lt 2) { throw "-MaxTitles must be 2 or more (it was $MaxTitles)" }
    $rec = "$Recordings\$Slug"
    $out = "$rec\exp02"
    $tmp = "$Staging\$Slug\exp02-tmp"
    foreach ($name in 'meta.json', 'scan.txt') {
        if (-not (Test-Path -LiteralPath "$rec\$name")) { throw "no $name in $rec (record the disc first: Task 8 or Task 9)" }
    }

    # The scan's titles, in the scan's numbering, and its angle (3308) and duplicate (3309) messages.
    $scanLines = @(Get-Content -LiteralPath "$rec\scan.txt" -ErrorAction Stop)
    $all = @($scanLines | ForEach-Object { if ($_ -match '^TINFO:(\d+),') { [int]$Matches[1] } } | Sort-Object -Unique)
    if ($all.Count -eq 0) { throw "$rec\scan.txt has no TINFO lines" }
    $n3308 = @($scanLines | Where-Object { $_.StartsWith('MSG:3308,') }).Count
    $n3309 = @($scanLines | Where-Object { $_.StartsWith('MSG:3309,') }).Count
    "scan: $($all.Count) titles, numbers $($all[0]) to $($all[-1]); MSG:3308 lines=$n3308; MSG:3309 lines=$n3309"

    # Evenly spaced, always including the first and the last title.
    if ($all.Count -le $MaxTitles) {
        $planned = $all
    } else {
        $planned = @(0..($MaxTitles - 1) | ForEach-Object { $all[[int][math]::Round($_ * ($all.Count - 1) / ($MaxTitles - 1))] } | Sort-Object -Unique)
    }
    if ($Redo.Count -gt 0) {
        $unknown = @($Redo | Where-Object { $all -notcontains $_ })
        if ($unknown.Count -gt 0) { throw "-Redo names titles that $rec\scan.txt does not list: $($unknown -join ',')" }
        $titles = @($Redo | Sort-Object -Unique)
    } else {
        $titles = $planned
    }
    "testing titles: $($titles -join ', ') ($($titles.Count) of $($all.Count))"

    # Earlier results are evidence: a run without -Redo refuses to mix with them.
    $have = @{}
    foreach ($found in @(Get-ChildItem -LiteralPath $out -File -ErrorAction SilentlyContinue | Where-Object { $_.Name -match '^title-\d+\.txt$' })) {
        $have[[int]($found.Name -replace '^title-(\d+)\.txt$', '$1')] = [bool](Select-String -LiteralPath $found.FullName -Pattern '^MSG:501[45],' -Quiet)
    }
    if ($have.Count -eq 0) {
        'earlier results: none'
    } else {
        $todo = @(@($planned | Where-Object { -not $have.ContainsKey([int]$_) }) + @($have.Keys | Where-Object { -not $have[$_] }) | Sort-Object -Unique)
        $todoText = if ($todo.Count -gt 0) { $todo -join ',' } else { 'none' }
        "earlier results: $($have.Count) title file(s) in $out; titles still to test: $todoText"
        if ($Redo.Count -eq 0 -and -not $Plan) {
            throw "$out already holds results of an earlier run, which are kept as evidence; test the titles still to test ($todoText) with -Redo (Task 10 Step 5)"
        }
    }

    # Nothing else may be using MakeMKV or the drive.
    $running = @(Get-Process -Name makemkv, makemkvcon, makemkvcon64 -ErrorAction SilentlyContinue)
    if ($running.Count -gt 0) {
        $list = ($running | ForEach-Object { $_.ProcessName + ' pid ' + $_.Id }) -join '; '
        throw "MakeMKV is already running ($list): ask the owner to close the MakeMKV window; a leftover makemkvcon64 is ended with: taskkill /f /pid <pid>"
    }

    # The disc in the drive must be the recorded one, or the scan's titles say nothing about it.
    Assert-DiscIs $Slug
    $meta = Get-Content -LiteralPath "$rec\meta.json" -Raw -ErrorAction Stop | ConvertFrom-Json
    $fingerprint = $meta.fingerprint
    "disc: ${Drive}: holds the disc of recording $Slug (fingerprint $fingerprint)"
    if (-not (Test-Path -LiteralPath $MK)) { throw "missing $MK" }
    if (-not (Test-Path -LiteralPath $AllTracksProfile)) { throw "missing job profile $AllTracksProfile" }
    "calls write into $tmp (emptied before each call, removed at the end)"

    # Limits: no call may write much, or run much longer than the recorded scan, without MSG:5014.
    $limitBytes = 512MB
    $limitSeconds = 1800
    $scanSeconds = 0
    try {
        $scanExit = Get-Content -LiteralPath "$rec\scan.txt.exit.json" -Raw -ErrorAction Stop | ConvertFrom-Json
        $scanSeconds = [int]([double]$scanExit.elapsed_ms / 1000)
    } catch { }
    if (3 * $scanSeconds -gt $limitSeconds) { $limitSeconds = 3 * $scanSeconds }
    $estimate = if ($scanSeconds -gt 0) { "about $([math]::Ceiling($titles.Count * ($scanSeconds + 20) / 60)) minutes" } else { 'unknown (scan.txt.exit.json gives no scan time)' }
    "limits per call: $limitSeconds s, or $limitBytes bytes written, without MSG:5014; estimate: $estimate"
    if ($Plan) {
        'RESULT ok: plan only; no MakeMKV call was made'
        return
    }

    $tmpUsed = $true
    Clear-Tmp
    Assert-EmptyDir $tmp
    New-Item -ItemType Directory -Force -Path $out -ErrorAction Stop | Out-Null
    $index = Get-DiscIndex
    $flags = (Get-MkFlags) -join ' '
    $startedAt = (Get-Date).ToString('s')
    $runFile = "$out\run-$stamp.json"
    Save-Run 'running'
    "run summary: $runFile"

    foreach ($title in $titles) {
        $file = "$out\title-$title.txt"
        # -Redo: an earlier result is evidence too; it is moved aside, never replaced.
        $keep = "$out\superseded\$stamp"
        foreach ($old in @($file, "$file.stderr")) {
            if (Test-Path -LiteralPath $old) {
                New-Item -ItemType Directory -Force -Path $keep -ErrorAction Stop | Out-Null
                Move-Item -LiteralPath $old -Destination $keep -ErrorAction Stop
                "kept the earlier $(Split-Path -Leaf $old) in exp02\superseded\$stamp"
            }
        }
        Clear-Tmp
        Assert-EmptyDir $tmp

        $psi = New-Object System.Diagnostics.ProcessStartInfo
        $psi.FileName = $MK
        $psi.Arguments = "$flags mkv disc:$index $title `"$tmp`""
        $psi.UseShellExecute = $false
        $psi.CreateNoWindow = $true
        $psi.RedirectStandardOutput = $true
        $psi.RedirectStandardError = $true
        $psi.StandardOutputEncoding = [System.Text.Encoding]::UTF8
        $psi.StandardErrorEncoding = [System.Text.Encoding]::UTF8
        $started = Get-Date
        $process = [System.Diagnostics.Process]::Start($psi)
        $errRead = $process.StandardError.ReadToEndAsync()

        # Read line by line, but never wait more than 2 s for a line, so that the limits are checked.
        $lines = New-Object System.Collections.Generic.List[string]
        $saw5014 = $false
        $why = ''
        $lastCheck = $started
        $read = $process.StandardOutput.ReadLineAsync()
        while ($true) {
            if ($read.Wait(2000)) {
                $line = $read.Result
                if ($null -eq $line) { break }
                $lines.Add($line)
                if ($line.StartsWith('MSG:5014,') -or $line.StartsWith('MSG:5015,')) { $saw5014 = $true; break }
                $read = $process.StandardOutput.ReadLineAsync()
            }
            $now = Get-Date
            if (($now - $lastCheck).TotalSeconds -ge 5) {
                $lastCheck = $now
                $written = [long]((Get-ChildItem -LiteralPath $tmp -Recurse -File -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum)
                if ($written -gt $limitBytes) { $why = "wrote $written bytes without MSG:5014"; break }
                if (($now - $started).TotalSeconds -gt $limitSeconds) { $why = "no MSG:5014 within $limitSeconds s"; break }
            }
        }
        $stopped = -not $process.HasExited
        if ($stopped) { try { $process.Kill() } catch { } }
        if (-not $process.WaitForExit(60000)) {
            throw "makemkvcon64 (pid $($process.Id)) did not exit within 60 s of being stopped; end it with: taskkill /f /pid $($process.Id)"
        }
        $exitCode = $process.ExitCode
        $seconds = [int]((Get-Date) - $started).TotalSeconds
        if ($stopped) { Start-Sleep -Seconds 10 }   # let the drive settle after a killed call
        $errText = ''
        try { if ($errRead.Wait(10000)) { $errText = $errRead.Result } } catch { }
        $written = [long]((Get-ChildItem -LiteralPath $tmp -Recurse -File -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum)
        $codes = @($lines | ForEach-Object { if ($_ -match '^MSG:(\d+),') { [int]$Matches[1] } } | Sort-Object -Unique)

        if ($lines.Count -gt 0) {
            Set-Content -LiteralPath $file -Value $lines.ToArray() -Encoding UTF8 -ErrorAction Stop
        } else {
            Set-Content -LiteralPath $file -Value $null -Encoding UTF8 -ErrorAction Stop
        }
        if ($errText) { Set-Content -LiteralPath "$file.stderr" -Value $errText -Encoding UTF8 -ErrorAction Stop }

        if ($why) { $ended = "stopped early: $why" } elseif ($saw5014) { $ended = 'stopped at MSG:5014' } else { $ended = 'ended by itself without MSG:5014' }
        $entry = [ordered]@{
            title         = $title
            saw_5014      = $saw5014
            ended         = $ended
            lines         = $lines.Count
            written_bytes = $written
            exit_code     = $exitCode
            seconds       = $seconds
            msg_codes     = $codes
        }
        $results.Add($entry)
        $process = $null
        Save-Run 'running'
        $summaryLine = "title ${title}: lines=$($lines.Count) stopped-at-5014=$saw5014 written-bytes=$written exit=$exitCode seconds=$seconds codes=$($codes -join ',')"
        if ($why) { $summaryLine += " STOPPED-EARLY: $why" }
        $summaryLine
        if ($why) {
            $rest = @($titles | Where-Object { $_ -gt $title })
            $restText = if ($rest.Count -gt 0) { $rest -join ',' } else { 'none' }
            throw "title $title was stopped early ($why); titles not tested in this run: $restText (Task 10 Step 5)"
        }
    }

    Clear-Tmp
    if (Test-Path -LiteralPath $tmp) { "WARNING: $tmp could not be removed (it holds what a stopped call wrote); delete it later" }
    $failed = @($results | Where-Object { -not $_.saw_5014 } | ForEach-Object { $_.title })
    if ($failed.Count -gt 0) {
        throw "no MSG:5014 for title(s) $($failed -join ','); read Task 10 Step 5 before testing them again with -Redo $($failed -join ',')"
    }
    Save-Run 'ok'
    "RESULT ok: $($titles.Count) title(s) tested; every call reached MSG:5014 and was stopped there"
} catch {
    $err = $_
    $reason = $err.Exception.Message
    # Never leave a rip running: a call that is still going is stopped here.
    if ($process -and -not $process.HasExited) {
        try { $process.Kill() } catch { }
        [void]$process.WaitForExit(60000)
    }
    if ($tmpUsed) { Clear-Tmp }
    if ($runFile) {
        try { Save-Run "FAILED: $reason" } catch { $reason += " ($runFile could not be written)" }
    }
    "RESULT FAILED: $reason"
    throw $err
}
