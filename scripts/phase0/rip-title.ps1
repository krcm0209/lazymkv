# Rips one title of the disc recorded under -Slug to $Staging\<Slug>\<Label>\ with a job profile, and judges the rip.
# The evidence goes to $Recordings\<Slug>\<Label>\ (C:\lazymkv\phase0\recordings\<Slug>\<Label>\):
#   rip.txt with .timing.jsonl, .stderr and .exit.json   MakeMKV's output, recorded by lazymkv-capture
#   free-ram.csv       free RAM every 5 s while MakeMKV runs (mini PC clock, MB)
#   rip-summary.json   timing, size, throughput, memory and the verdict
# One label holds one rip. If the label's staging or evidence folder is not empty the run is refused, unless
# -Force is given: -Force deletes both, but only after Assert-DiscIs has accepted the disc.
# Success (spec 9.4): a success summary (MSG:5005 or 5036) for one title; none of 5003, 5004 or 5037; exactly
# one non-empty .mkv in the staging folder; and, where both name one, the same source file for the title in
# this rip (MSG:3307) as in the recording's scan.txt (TINFO attribute 16).
# MakeMKV's exit code is recorded, never judged (spec 9.2). MakeMKV text never builds a path.
# The last line is "RESULT ok" or "RESULT FAILED: <reason>"; after a failure the script throws.
param(
    [Parameter(Mandatory)][string]$Slug,
    [Parameter(Mandatory)][int]$Title,
    [Parameter(Mandatory)][string]$Label,
    [string]$Profile = '',
    [int]$CacheMb = 1024,
    [switch]$Force
)
. C:\lazymkv\src\scripts\phase0\env.ps1

$failure = $null
$note = ''
$summary = $null
$memJob = $null
$ripRan = $false
try {
    Assert-Slug $Slug
    Assert-Slug $Label
    if (-not $Profile) { $Profile = $AllTracksProfile }
    if (-not (Test-Path -LiteralPath $Profile)) { throw "profile not found: $Profile" }
    if (-not (Test-Path -LiteralPath $CAP)) { throw "missing $CAP; build it with: scripts/box.sh cargo build --release -p lazymkv-capture" }
    $rec = "$Recordings\$Slug\$Label"
    $dest = "$Staging\$Slug\$Label"

    # The title must be in the recording's scan (made with Get-MkFlags' defaults); its source file is compared below.
    $scanFile = "$Recordings\$Slug\scan.txt"
    if (-not (Test-Path -LiteralPath $scanFile)) { throw "no $scanFile; record the disc with a scan first (record-disc.ps1)" }
    $scanText = [IO.File]::ReadAllText($scanFile)
    if (-not [regex]::IsMatch($scanText, '(?m)^TINFO:' + $Title + ',')) { throw "scan.txt lists no title $Title" }
    $found = [regex]::Match($scanText, '(?m)^TINFO:' + $Title + ',16,\d+,"(.*)"\r?$')
    $scanSource = $null
    if ($found.Success) { $scanSource = $found.Groups[1].Value }

    # Only one MakeMKV may use the drive (spec 10.4); the GUI holds it too.
    $running = @(Get-Process -Name makemkvcon64, makemkvcon, makemkv -ErrorAction SilentlyContinue)
    if ($running.Count -gt 0) { throw "MakeMKV is still running ($($running[0].Name).exe, process id $($running[0].Id)); it must end before a rip starts" }

    # Evidence and staged files are never replaced without -Force.
    foreach ($dir in $dest, $rec) {
        if (-not $Force -and (Test-Path -LiteralPath $dir) -and @(Get-ChildItem -LiteralPath $dir -Force -ErrorAction Stop).Count -gt 0) {
            throw "$dir is not empty: label $Label was ripped before and is kept; choose another -Label, or use -Force only where the plan says so"
        }
    }
    Assert-DiscIs $Slug
    # The staging folder first: if the share cannot be reached, nothing is created on C:.
    foreach ($dir in $dest, $rec) {
        if ($Force -and (Test-Path -LiteralPath $dir)) { Remove-Item -LiteralPath $dir -Recurse -Force -ErrorAction Stop }
        Assert-EmptyDir $dir
    }

    $summary = [ordered]@{
        ok = $false; result = $null; slug = $Slug; label = $Label; title = $Title; profile = $Profile; cache_mb = $CacheMb
        started = $null; elapsed_s = $null; saving_started_s = $null; bytes = $null; mb_per_s = $null; write_mb_per_s = $null
        free_ram_before_gb = $null; min_free_ram_gb = $null; min_free_ram_at_s = $null; ram_samples = 0
        files = @(); dest = $dest; exit_code = $null; success_message = $null; fail_codes = @()
        profile_errors = $null; read_errors = $null; write_errors = $null
        scan_source_file = $scanSource; rip_source_file = $null
    }

    # Free RAM every 5 s while MakeMKV runs, sampled by a background job (mini PC clock, MB).
    $memFile = "$rec\free-ram.csv"
    Set-Content -LiteralPath $memFile -Value 'time,free_mb' -Encoding UTF8 -ErrorAction Stop
    $summary['free_ram_before_gb'] = Get-FreeRamGB
    $memJob = Start-Job -ArgumentList $memFile -ScriptBlock {
        param($file)
        while ($true) {
            try {
                $mb = [int]((Get-CimInstance Win32_OperatingSystem -ErrorAction Stop).FreePhysicalMemory / 1024)
                [IO.File]::AppendAllText($file, (Get-Date -Format 'yyyy-MM-dd HH:mm:ss') + ',' + $mb + "`r`n")
            } catch { }
            Start-Sleep -Seconds 5
        }
    }

    $index = Get-DiscIndex
    $flags = Get-MkFlags -Profile $Profile -CacheMb $CacheMb
    $start = Get-Date
    $summary['started'] = $start.ToString('yyyy-MM-dd HH:mm:ss.fff')
    "rip: $Slug title $Title (scan source file: $scanSource) to $dest with --cache=$CacheMb, started $($summary['started'])"
    & $CAP record --out "$rec\rip.txt" -- $MK @flags mkv "disc:$index" $Title $dest | Out-Null
    $summary['exit_code'] = $LASTEXITCODE
    $elapsed = ((Get-Date) - $start).TotalSeconds
    $summary['elapsed_s'] = [int]$elapsed
    $ripRan = $true
} catch {
    $failure = $_.Exception.Message
}

if ($memJob) {
    Stop-Job -Job $memJob -ErrorAction SilentlyContinue
    Remove-Job -Job $memJob -Force -ErrorAction SilentlyContinue
}
if ($null -ne $summary) {
    try {
        $samples = @(Get-Content -LiteralPath $memFile -ErrorAction SilentlyContinue | ForEach-Object {
            if ($_ -match '^(\d{4}-\d\d-\d\d \d\d:\d\d:\d\d),(\d+)$') {
                [pscustomobject]@{ at = [datetime]::ParseExact($Matches[1], 'yyyy-MM-dd HH:mm:ss', $null); mb = [int]$Matches[2] }
            }
        })
        $summary['ram_samples'] = $samples.Count
        if ($samples.Count -gt 0) {
            $low = $samples | Sort-Object -Property mb | Select-Object -First 1
            $summary['min_free_ram_gb'] = [math]::Round($low.mb / 1024, 2)
            if ($summary['started']) { $summary['min_free_ram_at_s'] = [int]($low.at - $start).TotalSeconds }
        }

        if ($ripRan) {
            $lines = @()
            if (Test-Path -LiteralPath "$rec\rip.txt") { $lines = [IO.File]::ReadAllText("$rec\rip.txt").Split("`n") }
            $timing = @()
            if (Test-Path -LiteralPath "$rec\rip.txt.timing.jsonl") { $timing = [IO.File]::ReadAllText("$rec\rip.txt.timing.jsonl").Split("`n") }
            # rip.txt split at newlines lines up one for one with the timing lines.
            $codes = @()
            $successLine = $null
            $savedCount = $null
            for ($i = 0; $i -lt $lines.Count; $i++) {
                $line = $lines[$i]
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
                if ($code -eq 5014 -and $null -eq $summary['saving_started_s'] -and $i -lt $timing.Count -and $timing[$i] -match '"t_ms":(\d+)') {
                    $summary['saving_started_s'] = [math]::Round([double]$Matches[1] / 1000, 1)
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
            $saveStart = $summary['saving_started_s']
            if ($null -ne $saveStart -and $bytes -gt 0 -and $elapsed -gt $saveStart) {
                $summary['write_mb_per_s'] = [math]::Round($bytes / 1MB / ($elapsed - $saveStart), 1)
            }

            $problems = @()
            if (-not (Test-Path -LiteralPath "$rec\rip.txt.exit.json")) { $problems += 'lazymkv-capture wrote no rip.txt.exit.json' }
            if (-not $successLine) { $problems += 'no success summary (MSG:5005 or 5036)' } elseif ($null -ne $savedCount -and $savedCount -ne 1) { $problems += "the success summary reports $savedCount titles" }
            if ($failCodes.Count -gt 0) { $problems += "failure messages $($failCodes -join ',')" }
            if ($mkvs.Count -ne 1) { $problems += "$($mkvs.Count) .mkv files in $dest" } elseif ($mkvs[0].Length -eq 0) { $problems += "$($mkvs[0].Name) is empty" }
            $ripSource = $summary['rip_source_file']
            if ($scanSource -and $ripSource -and $ripSource -ne $scanSource) { $problems += "title $Title is $ripSource in this rip but $scanSource in scan.txt" }
            if ($problems.Count -gt 0) { $failure = $problems -join '; ' }
            if ($summary['profile_errors'] -gt 0) { $note = " (MSG:1009 seen $($summary['profile_errors']) times: MakeMKV rejected the profile; see rip.txt)" }
        }
    } catch {
        if (-not $failure) { $failure = "judging the rip failed: $($_.Exception.Message)" }
    }
    $summary['ok'] = -not $failure
    $summary['result'] = 'ok'
    if ($failure) { $summary['result'] = "FAILED: $failure" }
    $json = $summary | ConvertTo-Json
    try { Set-Content -LiteralPath "$rec\rip-summary.json" -Value $json -Encoding UTF8 -ErrorAction Stop } catch { if (-not $failure) { $failure = "cannot write rip-summary.json: $($_.Exception.Message)" } }
    $json
}
if ($failure) {
    "RESULT FAILED: $failure"
    throw "rip-title.ps1: $failure"
}
"RESULT ok$note"
