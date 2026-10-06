# Experiment 3 (Task 14): a decrypted whole-disc copy of the disc recorded under -Slug, written to the staging
# share, and MakeMKV's scan of that copy (spec 6.3 steps 6 and 8).
# The copy goes to $Staging\<Slug>\backup\. The evidence goes to $Recordings\<Slug>\backup\
# (C:\lazymkv\phase0\recordings\<Slug>\backup\):
#   backup.txt with .timing.jsonl, .stderr and .exit.json      MakeMKV's backup run, recorded by lazymkv-capture
#   copy-files.txt       one line per file of the copy: its size, a space, its path below the copy's root
#   scan-file.txt with .timing.jsonl, .stderr and .exit.json   MakeMKV's scan of the copy, with Get-MkFlags
#   backup-summary.json  timing, size, messages, the copy's layout and the verdict
# If the staging folder or the evidence folder is not empty the run is refused, unless -Force is given:
# -Force deletes both, but only after Assert-DiscIs has accepted the disc.
# The copy succeeded (spec 6.3) when MakeMKV reported MSG:5070, or 5079 (completed with hash failures), and no
# 5069, and the copy holds a BDMV or VIDEO_TS folder with data in it. The copy's root is found by listing the
# folder that was empty before the call: the folder itself, or the one folder below it that holds BDMV or VIDEO_TS.
# The scan succeeded when it finished, printed the MSG:1005 banner and lists at least one title.
# MakeMKV's exit code is recorded, never judged (spec 9.2). MakeMKV text never builds a path.
# The last line is "RESULT ok" or "RESULT FAILED: <reason>"; after a failure the script throws.
param(
    [Parameter(Mandatory)][string]$Slug,
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
    $rec = "$Recordings\$Slug\backup"
    $dest = "$Staging\$Slug\backup"

    # Only one MakeMKV may use the drive (spec 10.4); the GUI holds it too.
    $running = @(Get-Process -Name makemkvcon64, makemkvcon, makemkv -ErrorAction SilentlyContinue)
    if ($running.Count -gt 0) { throw "MakeMKV is still running ($($running[0].Name).exe, process id $($running[0].Id)); it must end before the copy starts" }

    # A copy and its evidence are never replaced without -Force.
    foreach ($dir in $dest, $rec) {
        if (-not $Force -and (Test-Path -LiteralPath $dir) -and @(Get-ChildItem -LiteralPath $dir -Force -ErrorAction Stop).Count -gt 0) {
            throw "$dir is not empty: a copy of $Slug was made before and is kept; use -Force only where the plan says so"
        }
    }
    # The disc must be the one recorded under this slug; checked before anything is deleted.
    Assert-DiscIs $Slug
    $index = Get-DiscIndex
    # The staging folder first: if the share cannot be reached, nothing is created on C:.
    foreach ($dir in $dest, $rec) {
        if ($Force -and (Test-Path -LiteralPath $dir)) { Remove-Item -LiteralPath $dir -Recurse -Force -ErrorAction Stop }
        Assert-EmptyDir $dir
    }

    $summary = [ordered]@{
        ok = $false; result = $null; slug = $Slug; dest = $dest; started = $null; elapsed_s = $null; exit_code = $null
        copy_message = $null; fail_codes = @(); read_errors = $null; write_errors = $null
        top_level = @(); copy_root = $null; files = $null; bytes = $null; mb_per_s = $null
        scan_ok = $false; scan_titles = $null
    }
    $start = Get-Date
    $summary['started'] = $start.ToString('yyyy-MM-dd HH:mm:ss.fff')
    "copy: $Slug from disc:$index to $dest, started $($summary['started'])"
    & $CAP record --out "$rec\backup.txt" -- $MK -r --messages=-stdout --progress=-same --cache=1024 backup --decrypt "disc:$index" $dest | Out-Null
    $summary['exit_code'] = $LASTEXITCODE
    $elapsed = ((Get-Date) - $start).TotalSeconds
    $summary['elapsed_s'] = [int]$elapsed

    # The copy's verdict from MakeMKV's messages (spec 6.3).
    $codes = @()
    $doneLine = $null
    if (Test-Path -LiteralPath "$rec\backup.txt") {
        foreach ($line in [IO.File]::ReadAllText("$rec\backup.txt").Split("`n")) {
            if (-not ($line -match '^MSG:(\d+),')) { continue }
            $code = [int]$Matches[1]
            $codes += $code
            if ($code -eq 5070 -or $code -eq 5079) { $doneLine = $line.Trim() }
        }
    }
    $summary['copy_message'] = $doneLine
    $summary['fail_codes'] = @($codes | Where-Object { $_ -eq 5069 } | Sort-Object -Unique)
    $summary['read_errors'] = @($codes | Where-Object { $_ -eq 2003 }).Count
    $summary['write_errors'] = @($codes | Where-Object { $_ -eq 2019 }).Count

    # The copy's root is found by listing the folder that was empty before the call.
    $top = @(Get-ChildItem -LiteralPath $dest -Force -ErrorAction SilentlyContinue)
    $summary['top_level'] = @($top | ForEach-Object { $_.Name })
    $root = $null
    if ((Test-Path -LiteralPath "$dest\BDMV") -or (Test-Path -LiteralPath "$dest\VIDEO_TS")) {
        $root = $dest
    } else {
        $inner = @($top | Where-Object { $_.PSIsContainer -and ((Test-Path -LiteralPath "$($_.FullName)\BDMV") -or (Test-Path -LiteralPath "$($_.FullName)\VIDEO_TS")) })
        if ($inner.Count -eq 1) { $root = $inner[0].FullName }
    }
    $summary['copy_root'] = $root
    "copy: finished after $($summary['elapsed_s']) s; message: $doneLine; top level of ${dest}: $($summary['top_level'] -join ', '); copy root: $root"

    if ($root) {
        # No -ErrorAction Stop on the listing: a hiccup after a long copy must not hide the verdict below.
        $base = $root.TrimEnd('\')
        $files = @(Get-ChildItem -LiteralPath $root -Recurse -File -Force -ErrorAction SilentlyContinue | Sort-Object -Property FullName)
        $lines = @($files | ForEach-Object { '{0} {1}' -f $_.Length, $_.FullName.Substring($base.Length).TrimStart('\').Replace('\', '/') })
        if ($lines.Count -gt 0) { Set-Content -LiteralPath "$rec\copy-files.txt" -Value $lines -Encoding UTF8 -ErrorAction Stop }
        $bytes = [int64](($files | Measure-Object -Property Length -Sum).Sum)
        $summary['files'] = $files.Count
        $summary['bytes'] = $bytes
        if ($elapsed -gt 0 -and $bytes -gt 0) { $summary['mb_per_s'] = [math]::Round($bytes / 1MB / $elapsed, 1) }
    }

    $problems = @()
    if (-not (Test-Path -LiteralPath "$rec\backup.txt.exit.json")) { $problems += 'lazymkv-capture wrote no backup.txt.exit.json' }
    if (-not $doneLine) { $problems += 'no completion message (MSG:5070 or 5079)' }
    if ($summary['fail_codes'].Count -gt 0) { $problems += 'MSG:5069 (backup failed)' }
    if (-not $root) { $problems += "no BDMV or VIDEO_TS folder in $dest, nor in exactly one folder below it" } elseif (-not ($summary['bytes'] -gt 0)) { $problems += "the copy in $root holds no data" }
    if ($problems.Count -gt 0) { throw "the copy failed: $($problems -join '; ')" }
    if ($codes -contains 5079) { $note = ' (MSG:5079: the copy completed with hash failures)' }

    # The copy's titles, with the same flags as the disc-time scan (spec 9.2).
    $flags = Get-MkFlags
    & $CAP record --out "$rec\scan-file.txt" -- $MK @flags info "file:$root" | Out-Null
    $exitJson = [string](Get-Content -LiteralPath "$rec\scan-file.txt.exit.json" -Raw -ErrorAction SilentlyContinue)
    $raw = [string](Get-Content -LiteralPath "$rec\scan-file.txt" -Raw -ErrorAction SilentlyContinue)
    $banner = [regex]::IsMatch($raw, '(?m)^MSG:1005,')
    $count = [regex]::Match($raw, '(?m)^TCOUNT:(\d+)')
    $titles = if ($count.Success) { $count.Groups[1].Value } else { 'NO-TCOUNT-LINE' }
    if ($count.Success) { $summary['scan_titles'] = [int]$titles }
    "scan: scan-file.txt exit.json=$(if ($exitJson) { $exitJson.Trim() } else { 'MISSING' }) version_banner=$banner titles=$titles"
    if (-not $exitJson) { throw 'the copy succeeded, but its scan did not finish: scan-file.txt.exit.json was not written' }
    if (-not $banner) { throw 'the copy succeeded, but MakeMKV did not run its scan: no MSG:1005 version banner in scan-file.txt' }
    if (-not ($count.Success -and [int]$titles -gt 0)) { throw "the copy succeeded, but its scan lists no titles (titles=$titles)" }
    $summary['scan_ok'] = $true
} catch {
    $failure = $_.Exception.Message
}

if ($null -ne $summary) {
    $summary['ok'] = -not $failure
    $summary['result'] = 'ok'
    if ($failure) { $summary['result'] = "FAILED: $failure" }
    $json = $summary | ConvertTo-Json
    try { Set-Content -LiteralPath "$rec\backup-summary.json" -Value $json -Encoding UTF8 -ErrorAction Stop } catch { if (-not $failure) { $failure = "cannot write backup-summary.json: $($_.Exception.Message)" } }
    $json
}
if ($failure) {
    "RESULT FAILED: $failure"
    throw "backup-disc.ps1: $failure"
}
"RESULT ok$note"
