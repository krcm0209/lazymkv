# Experiment 3 (Task 14), diagnosis of a crashed scan of the whole-disc copy: makes a PARTIAL copy of the copy on C:
# (every file except BDMV\STREAM\*, plus the stream clips named with -Clips; with -NoJava also without BDMV\JAR and
# BDMV\BDJO) and scans it with MakeMKV, to tell a share/UNC problem from a content problem without copying 50 GB.
# The partial copy is temporary: at most 2 GB, under C:\lazymkv\phase0\tmp\partial-<Slug>-<Name>\, deleted at the
# end unless -Keep is given (plan, Global Constraints: only small files on C:, and only for the minutes they are used).
# Writes to $Recordings\<Slug>\backup\:
#   scan-partial-<Name>.txt with .timing.jsonl, .stderr and .exit.json   MakeMKV's output (Get-MkFlags)
#   scan-partial-<Name>.txt.makemkv-debug.txt   MakeMKV's debug log of the run (decision D16)
#   scan-partial-<Name>.txt.files.txt   one line per copied file: size, a space, the path below the copy's root
# -Clips: clip numbers of BDMV\STREAM, as "27-60" or "4,6,9" or both ("1-3,7"); the number is the file's name.
# Declared [string[]]: run-interactive.ps1 passes arguments inside -Command, where "4,6,9" is an array literal.
# -SkipPlaylists: playlist numbers ("00055" or "55,120") whose .mpls files are left out of BDMV\PLAYLIST and BDMV\BACKUP\PLAYLIST.
# An existing scan-partial-<Name>.txt is refused unless -Force is given. The copy on the share is never changed.
# The line before the clean-up is "RESULT ok" or "RESULT FAILED: <reason>"; after a failure the script throws.
# Then comes the clean-up line: "partial: deleted <folder>", or "partial: NOT deleted: <folder> (remove it by hand)".
param(
    [Parameter(Mandatory)][string]$Slug,
    [Parameter(Mandatory)][string]$Name,
    [Parameter(Mandatory)][string[]]$Clips,
    [string[]]$SkipPlaylists = @(),
    [switch]$NoJava,
    [switch]$Keep,
    [switch]$Force
)
. C:\lazymkv\src\scripts\phase0\env.ps1

$MaxBytes = 2GB
$partial = $null
try {
    Assert-Slug $Slug
    Assert-Slug $Name
    $clipSpec = ($Clips -join ',')
    if ($clipSpec -notmatch '^\d+(-\d+)?(,\d+(-\d+)?)*$') { throw "-Clips must look like 27-60 or 4,6,9" }
    if ($SkipPlaylists.Count -gt 0 -and ($SkipPlaylists -join ',') -notmatch '^\d+(,\d+)*$') { throw '-SkipPlaylists must look like 55 or 00055,120' }
    $skipMpls = @(($SkipPlaylists -join ',') -split ',' | Where-Object { $_ -match '^\d+$' } | ForEach-Object { [int]$_ })
    $wanted = @()
    foreach ($part in $clipSpec -split ',') {
        if ($part -match '^(\d+)-(\d+)$') { $wanted += [int]$Matches[1]..[int]$Matches[2] } else { $wanted += [int]$part }
    }
    if (-not (Test-Path -LiteralPath $CAP)) { throw "missing $CAP; build it with: scripts/box.sh cargo build --release -p lazymkv-capture" }
    $rec = "$Recordings\$Slug\backup"
    $summaryFile = "$rec\backup-summary.json"
    if (-not (Test-Path -LiteralPath $summaryFile)) { throw "no $summaryFile; make the copy with backup-disc.ps1 first" }
    $backup = Get-Content -LiteralPath $summaryFile -Raw -ErrorAction Stop | ConvertFrom-Json
    $root = [string]$backup.copy_root
    if (-not $root) { throw "backup-summary.json names no copy_root: the copy did not succeed" }
    if ([string]$backup.result -like 'FAILED: the copy failed*') { throw "the copy did not succeed ($($backup.result))" }
    if (-not (Test-Path -LiteralPath $root)) { throw "the copy root $root cannot be reached" }
    $out = "$rec\scan-partial-$Name.txt"
    if ((Test-Path -LiteralPath $out) -and -not $Force) { throw "$out already exists and is kept as evidence; choose another name or use -Force" }
    foreach ($suffix in '', '.timing.jsonl', '.stderr', '.exit.json', '.makemkv-debug.txt', '.files.txt') {
        if (Test-Path -LiteralPath "$out$suffix") { Remove-Item -LiteralPath "$out$suffix" -Force -ErrorAction Stop }
    }
    $running = @(Get-Process -Name makemkvcon64, makemkvcon, makemkv -ErrorAction SilentlyContinue)
    if ($running.Count -gt 0) { throw "MakeMKV is still running ($($running[0].Name).exe, process id $($running[0].Id))" }
    $dest = "$P0\tmp\partial-$Slug-$Name"
    if (Test-Path -LiteralPath $dest) {
        if (-not $Force) { throw "$dest already exists (an earlier partial copy that was kept); use -Force to replace it" }
        Remove-Item -LiteralPath $dest -Recurse -Force -ErrorAction Stop
    }

    # Choose the files first and check their size, so that nothing above 2 GB is ever started.
    $base = $root.TrimEnd('\')
    $all = @(Get-ChildItem -LiteralPath $root -Recurse -File -Force -ErrorAction Stop)
    $chosen = @()
    foreach ($f in $all) {
        $rel = $f.FullName.Substring($base.Length).TrimStart('\')
        $upper = $rel.ToUpper()
        if ($upper -like 'BDMV\STREAM\*') {
            $num = [IO.Path]::GetFileNameWithoutExtension($f.Name)
            if ($num -match '^\d+$' -and $wanted -contains [int]$num) { $chosen += $f }
            continue
        }
        if ($NoJava -and ($upper -like 'BDMV\JAR\*' -or $upper -like 'BDMV\BDJO\*' -or $upper -like 'BDMV\BACKUP\BDJO\*')) { continue }
        if ($skipMpls.Count -gt 0 -and $upper -like '*\PLAYLIST\*.MPLS' -and $skipMpls -contains [int][IO.Path]::GetFileNameWithoutExtension($f.Name)) { continue }
        $chosen += $f
    }
    $bytes = [long](($chosen | Measure-Object -Property Length -Sum).Sum)
    # Partial copies kept earlier with -Keep (under other names) count against the same 2 GB on C:.
    $kept = [long]((Get-ChildItem -LiteralPath "$P0\tmp" -Directory -Filter 'partial-*' -ErrorAction SilentlyContinue | Get-ChildItem -Recurse -File -Force -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum)
    if ($bytes + $kept -gt $MaxBytes) { throw "the partial copy would hold $bytes bytes, and $kept bytes of earlier kept partial copies are in $P0\tmp, more than the 2 GB allowed on C:; choose fewer clips or remove the kept copies" }
    $clipsFound = @($chosen | Where-Object { $_.FullName.Substring($base.Length).TrimStart('\').ToUpper() -like 'BDMV\STREAM\*' }).Count
    "partial: $($chosen.Count) files, $bytes bytes ($clipsFound stream clips of $($wanted.Count) asked; NoJava=$NoJava; skipped playlists=$($skipMpls -join ',')) from $root to $dest"

    $partial = $dest
    New-Item -ItemType Directory -Force -Path $dest -ErrorAction Stop | Out-Null
    $lines = @()
    foreach ($f in $chosen) {
        $rel = $f.FullName.Substring($base.Length).TrimStart('\')
        $target = Join-Path $dest $rel
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $target) -ErrorAction Stop | Out-Null
        Copy-Item -LiteralPath $f.FullName -Destination $target -ErrorAction Stop
        $lines += ('{0} {1}' -f $f.Length, $rel.Replace('\', '/'))
    }
    Set-Content -LiteralPath "$out.files.txt" -Value $lines -Encoding UTF8 -ErrorAction Stop

    $flags = Get-MkFlags
    $start = Get-Date
    & $CAP record --out $out -- $MK @flags info "file:$dest" | Out-Null
    $elapsed = [math]::Round(((Get-Date) - $start).TotalSeconds, 1)
    Save-MkDebugLog -Since $start -To "$out.makemkv-debug.txt"
    $exitJson = [string](Get-Content -LiteralPath "$out.exit.json" -Raw -ErrorAction SilentlyContinue)
    $raw = [string](Get-Content -LiteralPath $out -Raw -ErrorAction SilentlyContinue)
    $banner = [regex]::IsMatch($raw, '(?m)^MSG:1005,')
    $count = [regex]::Match($raw, '(?m)^TCOUNT:(\d+)')
    $titles = if ($count.Success) { $count.Groups[1].Value } else { 'NO-TCOUNT-LINE' }
    $msgs = @([regex]::Matches($raw, '(?m)^MSG:(\d+),') | ForEach-Object { $_.Groups[1].Value })
    $skipped = @([regex]::Matches($raw, '(?m)^MSG:3025,\d+,\d+,"Title #([^ ]+) ') | ForEach-Object { $_.Groups[1].Value })
    $added = @([regex]::Matches($raw, '(?m)^MSG:3307,\d+,\d+,"File ([^ ]+) was added as title #(\d+)"') | ForEach-Object { "$($_.Groups[1].Value)=#$($_.Groups[2].Value)" })
    "scan: scan-partial-$Name.txt exit.json=$(if ($exitJson) { $exitJson.Trim() } else { 'MISSING' }) seconds=$elapsed version_banner=$banner titles=$titles messages=$($msgs.Count) skipped=$($skipped -join ',') added=$($added -join ',') codes=$(($msgs | Sort-Object -Unique) -join ',')"
    if (-not $exitJson) { throw 'the scan did not finish: exit.json was not written' }
    if (-not $banner) { throw 'MakeMKV did not run: no MSG:1005 version banner' }
    if (-not ($count.Success -and [int]$titles -gt 0)) { throw "the scan lists no titles (titles=$titles)" }
    'RESULT ok'
} catch {
    "RESULT FAILED: $($_.Exception.Message)"
    throw
} finally {
    if ($partial -and -not $Keep -and (Test-Path -LiteralPath $partial)) {
        Remove-Item -LiteralPath $partial -Recurse -Force -ErrorAction SilentlyContinue
        if (Test-Path -LiteralPath $partial) { "partial: NOT deleted: $partial (remove it by hand)" } else { "partial: deleted $partial" }
    }
}
