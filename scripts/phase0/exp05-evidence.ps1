# Experiment 5 evidence that record-disc.ps1 does not keep (Task 9). Run it over plain ssh: it needs C: and
# the drive, never the share. It writes into $Recordings\<Slug>\ (C:\lazymkv\phase0\recordings\<Slug>\):
#   makemkv-data-<Listing>.txt  one line per file in MakeMKV's data folders: last write time (UTC), size, path.
#                               No file is opened. Of MakeMKV's registry settings only app_DataDir is read;
#                               app_Key (the licence key) is never read.
#   <DriveList>.txt             only with -DriveList, and made before the folder listing: MakeMKV's drive
#                               listing (info disc:9999, the call Get-DiscIndex makes before every scan),
#                               recorded by lazymkv-capture with .timing.jsonl, .stderr and .exit.json.
# With -DriveList on a recording that already has a meta.json, Assert-DiscIs checks the disc before MakeMKV is started.
# An existing evidence file is refused unless -Force is given.
# The last line is "RESULT ok" or "RESULT FAILED: <reason>"; after a failure the script throws.
param(
    [Parameter(Mandatory)][string]$Slug,
    [Parameter(Mandatory)][string]$Listing,
    [string]$DriveList = '',
    [switch]$Force
)
. C:\lazymkv\src\scripts\phase0\env.ps1

try {
    Assert-Slug $Slug
    Assert-Slug $Listing
    if ($DriveList) { Assert-Slug $DriveList }
    $out = "$Recordings\$Slug"
    $list = "$out\makemkv-data-$Listing.txt"
    $files = @($list)
    if ($DriveList) {
        $drives = "$out\$DriveList.txt"
        $files += @($drives, "$drives.stderr", "$drives.timing.jsonl", "$drives.exit.json")
    }

    # Refuse before anything is created, deleted or started.
    foreach ($file in $files) {
        if ((Test-Path -LiteralPath $file) -and -not $Force) {
            throw "$file already exists and is kept as evidence; choose another name"
        }
    }
    if ($DriveList -and -not (Test-Path -LiteralPath $CAP)) { throw "missing $CAP; build it with: scripts/box.sh cargo build --release -p lazymkv-capture" }
    New-Item -ItemType Directory -Force -Path $out -ErrorAction Stop | Out-Null
    foreach ($file in $files) {
        if (Test-Path -LiteralPath $file) { Remove-Item -LiteralPath $file -Force -ErrorAction Stop }
    }

    if ($DriveList) {
        # The disc may still be spinning up: wait up to two minutes until its root can be listed.
        $ready = $false
        for ($i = 0; $i -lt 24 -and -not $ready; $i++) {
            if (@(Get-ChildItem -LiteralPath "${Drive}:\" -ErrorAction SilentlyContinue).Count -gt 0) { $ready = $true } else { Start-Sleep -Seconds 5 }
        }
        if (-not $ready) { throw "no readable disc in ${Drive}: after 120 s; MakeMKV was not started" }
        # A recording that already has a meta.json belongs to one disc: the drive listing must be of that disc.
        if (Test-Path -LiteralPath "$out\meta.json") { Assert-DiscIs $Slug }
        & $CAP record --out $drives -- $MK -r --cache=1 info disc:9999 | Out-Null
        $exitJson = [string](Get-Content -LiteralPath "$drives.exit.json" -Raw -ErrorAction SilentlyContinue)
        $raw = [string](Get-Content -LiteralPath $drives -Raw -ErrorAction SilentlyContinue)
        $banner = [regex]::IsMatch($raw, '(?m)^MSG:1005,')
        $drv = [regex]::Match($raw, '(?m)^DRV:\d+,(\d+),\d+,\d+,"[^"]*","([^"]*)","' + [regex]::Escape("${Drive}:") + '"\s*$')
        $state = if ($drv.Success) { $drv.Groups[1].Value } else { 'NO-DRV-LINE' }
        $label = if ($drv.Success) { $drv.Groups[2].Value } else { '' }
        $codes = @([regex]::Matches($raw, '(?m)^MSG:(\d+),') | ForEach-Object { $_.Groups[1].Value } | Where-Object { $_ -ne '1005' })
        $others = if ($codes.Count -gt 0) { $codes -join ',' } else { 'none' }
        "drives: $DriveList.txt exit.json=$(if ($exitJson) { $exitJson.Trim() } else { 'MISSING' }) version_banner=$banner state=$state label=$label other_messages=$others"
        if (-not $exitJson) { throw "the drive listing did not finish: $drives.exit.json was not written" }
        if (-not $banner) { throw "MakeMKV did not run: no MSG:1005 version banner in $drives" }
        if ($state -ne '2') { throw "MakeMKV does not list a disc in ${Drive}: (DRV state $state; 2 means a disc is inserted)" }
    }

    # MakeMKV's data folders: the one set in app_DataDir, if any, and the usual places.
    $settings = 'HKCU:\Software\MakeMKV'
    $dataDir = $null
    if (Test-Path -LiteralPath $settings) {
        $dataDir = (Get-ItemProperty -LiteralPath $settings -Name app_DataDir -ErrorAction SilentlyContinue).app_DataDir
        $lines = @("app_DataDir: $(if ($dataDir) { $dataDir } else { '(not set)' })")
    } else {
        $lines = @("app_DataDir: (no $settings key in this session)")
    }
    $folders = @("$env:USERPROFILE\.MakeMKV", "$env:APPDATA\MakeMKV", "$env:LOCALAPPDATA\MakeMKV", "$env:ProgramData\MakeMKV")
    if ($dataDir) { $folders = @([Environment]::ExpandEnvironmentVariables([string]$dataDir)) + $folders }
    $found = @()
    $total = 0
    foreach ($folder in @($folders | Select-Object -Unique)) {
        if (-not (Test-Path -LiteralPath $folder)) { $lines += "folder: $folder (absent)"; continue }
        $items = @(Get-ChildItem -LiteralPath $folder -Recurse -Force -File -ErrorAction Stop | Sort-Object FullName)
        $lines += "folder: $folder files=$($items.Count)"
        foreach ($item in $items) {
            $lines += ('{0} UTC {1,12} {2}' -f $item.LastWriteTimeUtc.ToString('yyyy-MM-dd HH:mm:ss'), $item.Length, $item.FullName)
        }
        $found += $folder
        $total += $items.Count
    }
    Set-Content -LiteralPath $list -Value $lines -Encoding UTF8 -ErrorAction Stop
    "listing: makemkv-data-$Listing.txt files=$total folders=$(if ($found.Count -gt 0) { $found -join ';' } else { 'NONE-FOUND' })"
    'RESULT ok'
} catch {
    "RESULT FAILED: $($_.Exception.Message)"
    throw
}
