# Experiment 3 (Task 14): extracts one title from the whole-disc copy that backup-disc.ps1 made and judged good
# (spec 6.3 step 8), and identifies the file. The disc is not used.
# The copy is read at the copy_root recorded in $Recordings\<Slug>\backup\backup-summary.json, which
# backup-disc.ps1 found by listing the copy. -Title is a title number of the copy's scan,
# $Recordings\<Slug>\backup\scan-file.txt. The file goes to $Staging\<Slug>\extract\; the evidence goes to
# $Recordings\<Slug>\extract\ (C:\lazymkv\phase0\recordings\<Slug>\extract\):
#   extract.txt with .timing.jsonl, .stderr and .exit.json     MakeMKV's run: mkv file:<copy root> <Title> <dir>
#   extract.txt.makemkv-debug.txt   MakeMKV's debug log of that run; system-events.json   Windows events during it (D16)
#   mkvmerge.json with .timing.jsonl, .stderr and .exit.json   mkvmerge -J of the extracted file
#   extract-summary.json   timing, size, messages and the verdict
# If either folder is not empty the run is refused, unless -Force is given, which deletes both first.
# Success (spec 9.4): a success summary (MSG:5005 or 5036) for one title; none of 5003, 5004 or 5037; exactly one
# non-empty .mkv in the staging folder; where both name one, the same source file for the title in this run
# (MSG:3307) as in scan-file.txt (TINFO attribute 16); and mkvmerge -J exits 0 or 1 and lists at least one track.
# MakeMKV's exit code is recorded, never judged (spec 9.2). MakeMKV text never builds a path.
# The extraction keeps MakeMKV's debug log and the system events but not the rip scripts' free-RAM/CPU sampler (system.csv); this is a documented D16 exception.
# The last line is "RESULT ok" or "RESULT FAILED: <reason>"; after a failure the script throws.
param(
    [Parameter(Mandatory)][string]$Slug,
    [Parameter(Mandatory)][int]$Title,
    [switch]$Force
)
. C:\lazymkv\src\scripts\phase0\env.ps1

$failure = $null
$note = ''
$summary = $null
$rec = $null
try {
    Assert-Slug $Slug
    if (-not (Test-Path -LiteralPath $CAP)) { throw "missing $CAP; build it with: scripts/box.sh cargo build --release -p lazymkv-capture" }
    if (-not (Test-Path -LiteralPath $AllTracksProfile)) { throw "missing job profile $AllTracksProfile" }
    if (-not $MKVMERGE) { throw 'mkvmerge.exe not found' }
    $backup = "$Recordings\$Slug\backup"
    $rec = "$Recordings\$Slug\extract"
    $dest = "$Staging\$Slug\extract"

    # Only a copy that backup-disc.ps1 judged good is used, at the root it found by listing the copy.
    $copyFile = "$backup\backup-summary.json"
    if (-not (Test-Path -LiteralPath $copyFile)) { throw "no $copyFile; make the copy with backup-disc.ps1 first" }
    $copy = Get-Content -LiteralPath $copyFile -Raw -Encoding UTF8 -ErrorAction Stop | ConvertFrom-Json
    if ($copy.ok -ne $true -or -not $copy.copy_root) { throw "the copy of $Slug did not succeed ($($copy.result)); only a copy whose backup-summary.json says ok is used" }
    $root = [string]$copy.copy_root

    # The title must be in the copy's scan, made with the same flags; its source file is compared below.
    $scanFile = "$backup\scan-file.txt"
    if (-not (Test-Path -LiteralPath $scanFile)) { throw "no $scanFile" }
    $scanText = [IO.File]::ReadAllText($scanFile)
    if (-not [regex]::IsMatch($scanText, '(?m)^TINFO:' + $Title + ',')) { throw "scan-file.txt lists no title $Title" }
    $found = [regex]::Match($scanText, '(?m)^TINFO:' + $Title + ',16,\d+,"(.*)"\r?$')
    $scanSource = $null
    if ($found.Success) { $scanSource = $found.Groups[1].Value }

    # Only one MakeMKV at a time (spec 10.4).
    $running = @(Get-Process -Name makemkvcon64, makemkvcon, makemkv -ErrorAction SilentlyContinue)
    if ($running.Count -gt 0) { throw "MakeMKV is still running ($($running[0].Name).exe, process id $($running[0].Id)); it must end before the extraction starts" }

    # An extraction and its evidence are never replaced without -Force.
    foreach ($dir in $dest, $rec) {
        if (-not $Force -and (Test-Path -LiteralPath $dir) -and @(Get-ChildItem -LiteralPath $dir -Force -ErrorAction Stop).Count -gt 0) {
            throw "$dir is not empty: an extraction was made before and is kept; use -Force only where the plan says so"
        }
    }
    # The staging folder first: if the share cannot be reached, nothing is created on C:.
    foreach ($dir in $dest, $rec) {
        if ($Force -and (Test-Path -LiteralPath $dir)) { Remove-Item -LiteralPath $dir -Recurse -Force -ErrorAction Stop }
        Assert-EmptyDir $dir
    }
    if (-not (Test-Path -LiteralPath $root)) { throw "the copy is no longer at $root" }

    $summary = [ordered]@{
        ok = $false; result = $null; slug = $Slug; title = $Title; copy_root = $root; dest = $dest
        started = $null; elapsed_s = $null; exit_code = $null; files = @(); bytes = $null; mb_per_s = $null
        success_message = $null; fail_codes = @(); profile_errors = $null; read_errors = $null; write_errors = $null
        scan_source_file = $scanSource; rip_source_file = $null; mkvmerge_exit = $null; tracks = $null
    }
    $flags = Get-MkFlags
    $start = Get-Date
    $summary['started'] = $start.ToString('yyyy-MM-dd HH:mm:ss.fff')
    "extract: $Slug title $Title (source file in scan-file.txt: $scanSource) from $root to $dest, started $($summary['started'])"
    & $CAP record --out "$rec\extract.txt" -- $MK @flags mkv "file:$root" $Title $dest | Out-Null
    $summary['exit_code'] = $LASTEXITCODE
    $elapsed = ((Get-Date) - $start).TotalSeconds
    $summary['elapsed_s'] = [int]$elapsed
    Save-MkDebugLog -Since $start -To "$rec\extract.txt.makemkv-debug.txt"
    Save-SystemEvents -Since $start -To "$rec\system-events.json"

    $codes = @()
    $successLine = $null
    $savedCount = $null
    if (Test-Path -LiteralPath "$rec\extract.txt") {
        foreach ($line in [IO.File]::ReadAllText("$rec\extract.txt").Split("`n")) {
            if (-not ($line -match '^MSG:(\d+),')) { continue }
            $code = [int]$Matches[1]
            $codes += $code
            if ($code -eq 5005 -or $code -eq 5036) {
                $successLine = $line.Trim()
                $savedCount = $null
                if ($line -match ',"(\d+)"\s*$') { $savedCount = [int]$Matches[1] }
            }
            if ($code -eq 3307 -and $line -match ',"([^"]*)","(\d+)"\s*$' -and [int]$Matches[2] -eq $Title) {
                $summary['rip_source_file'] = $Matches[1]
            }
        }
    }
    $failCodes = @($codes | Where-Object { $_ -eq 5003 -or $_ -eq 5004 -or $_ -eq 5037 } | Sort-Object -Unique)
    $summary['fail_codes'] = $failCodes
    $summary['success_message'] = $successLine
    $summary['profile_errors'] = @($codes | Where-Object { $_ -eq 1009 }).Count
    $summary['read_errors'] = @($codes | Where-Object { $_ -eq 2003 }).Count
    $summary['write_errors'] = @($codes | Where-Object { $_ -eq 2019 }).Count

    # The output is found by listing the folder that was empty before the call.
    $files = @(Get-ChildItem -LiteralPath $dest -File -Force -ErrorAction SilentlyContinue)
    $mkvs = @($files | Where-Object { $_.Extension -eq '.mkv' })
    $bytes = [int64](($mkvs | Measure-Object -Property Length -Sum).Sum)
    $summary['files'] = @($files | ForEach-Object { $_.Name })
    $summary['bytes'] = $bytes
    if ($elapsed -gt 0 -and $bytes -gt 0) { $summary['mb_per_s'] = [math]::Round($bytes / 1MB / $elapsed, 1) }

    $problems = @()
    if (-not (Test-Path -LiteralPath "$rec\extract.txt.exit.json")) { $problems += 'lazymkv-capture wrote no extract.txt.exit.json' }
    if (-not $successLine) { $problems += 'no success summary (MSG:5005 or 5036)' } elseif ($null -ne $savedCount -and $savedCount -ne 1) { $problems += "the success summary reports $savedCount titles" }
    if ($failCodes.Count -gt 0) { $problems += "failure messages $($failCodes -join ',')" }
    if ($mkvs.Count -ne 1) { $problems += "$($mkvs.Count) .mkv files in $dest" } elseif ($mkvs[0].Length -eq 0) { $problems += "$($mkvs[0].Name) is empty" }
    $ripSource = $summary['rip_source_file']
    if ($scanSource -and $ripSource -and $ripSource -ne $scanSource) { $problems += "title $Title is $ripSource in this run but $scanSource in scan-file.txt" }
    if ($problems.Count -gt 0) { throw "the extraction failed: $($problems -join '; ')" }
    if ($summary['profile_errors'] -gt 0) { $note = " (MSG:1009 seen $($summary['profile_errors']) times: MakeMKV rejected the profile; see extract.txt)" }

    # Identification: a header read. mkvmerge exits 1 for warnings only.
    & $CAP record --out "$rec\mkvmerge.json" -- $MKVMERGE -J $mkvs[0].FullName | Out-Null
    $mkvmergeExit = $LASTEXITCODE
    $summary['mkvmerge_exit'] = $mkvmergeExit
    if ($mkvmergeExit -gt 1) { throw "mkvmerge -J exited $mkvmergeExit; see mkvmerge.json" }
    $ident = Get-Content -LiteralPath "$rec\mkvmerge.json" -Raw -Encoding UTF8 -ErrorAction Stop | ConvertFrom-Json
    $summary['tracks'] = @($ident.tracks | Where-Object { $_ }).Count
    if ($summary['tracks'] -eq 0) { throw 'mkvmerge -J lists no tracks; see mkvmerge.json' }
    "extract: $($mkvs[0].Name) ($bytes bytes), $($summary['tracks']) tracks"
} catch {
    $failure = $_.Exception.Message
}

if ($null -ne $summary) {
    $summary['ok'] = -not $failure
    $summary['result'] = 'ok'
    if ($failure) { $summary['result'] = "FAILED: $failure" }
    $json = $summary | ConvertTo-Json
    try { Set-Content -LiteralPath "$rec\extract-summary.json" -Value $json -Encoding UTF8 -ErrorAction Stop } catch { if (-not $failure) { $failure = "cannot write extract-summary.json: $($_.Exception.Message)" } }
    $json
}
if ($failure) {
    "RESULT FAILED: $failure"
    throw "extract-from-copy.ps1: $failure"
}
"RESULT ok$note"
