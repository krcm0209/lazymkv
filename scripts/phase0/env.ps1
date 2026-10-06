# Dot-source on the mini PC:  . C:\lazymkv\src\scripts\phase0\env.ps1
$MK  = 'C:\Program Files (x86)\MakeMKV\makemkvcon64.exe'
$P0  = 'C:\lazymkv\phase0'
$CAP = 'C:\lazymkv\target\release\lazymkv-capture.exe'
$AllTracksProfile = "$P0\profiles\all-tracks.mmcp.xml"
$EngProfile       = "$P0\profiles\eng-only.mmcp.xml"

# Owner-specific values: $Staging, $Drive, $TestLibrary, $TuningData.
if (-not (Test-Path "$P0\config.ps1")) { throw "missing $P0\config.ps1 (the owner's settings)" }
. "$P0\config.ps1"
if (-not $Staging -or -not $Drive) { throw "$P0\config.ps1 must set `$Staging and `$Drive" }
if (-not $TuningData) { throw "$P0\config.ps1 must set `$TuningData (the folder on the share that keeps the tuning dataset)" }

function Resolve-Tool([string]$Name, [string[]]$Fallbacks) {
    $found = Get-Command $Name -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($found) { return $found.Source }
    foreach ($path in $Fallbacks) { if (Test-Path $path) { return $path } }
    return $null
}

$links = @("$env:LOCALAPPDATA\Microsoft\WinGet\Links", 'C:\Program Files\WinGet\Links')
$MKVMERGE  = Resolve-Tool 'mkvmerge.exe'  @('C:\Program Files\MKVToolNix\mkvmerge.exe')
$FFPROBE   = Resolve-Tool 'ffprobe.exe'   ($links | ForEach-Object { "$_\ffprobe.exe" })
$FFMPEG    = Resolve-Tool 'ffmpeg.exe'    ($links | ForEach-Object { "$_\ffmpeg.exe" })
$MEDIAINFO = Resolve-Tool 'mediainfo.exe' ($links | ForEach-Object { "$_\mediainfo.exe" })
$TESSERACT = Resolve-Tool 'tesseract.exe' @('C:\Program Files\Tesseract-OCR\tesseract.exe')

function Get-MkFlags([string]$Profile = $AllTracksProfile, [int]$CacheMb = 1024) {
    # --debug (decision D16): MakeMKV adds MSG:1003/1004 lines and writes $MkDebugLog; Save-MkDebugLog keeps that file.
    @('-r', '--messages=-stdout', '--progress=-same', '--debug', '--minlength=120', "--cache=$CacheMb", "--profile=$Profile")
}

# With --debug, MakeMKV 2.0.0 writes this file anew on every run (a path given to --debug is ignored).
$MkDebugLog = "$env:USERPROFILE\MakeMKV_log.txt"

# Keeps the debug log of the MakeMKV run that started at $Since beside its recording. Never fails the caller:
# it returns one line for the job log. The registration key is removed if the log should ever contain it.
function Save-MkDebugLog([datetime]$Since, [string]$To) {
    try {
        $file = Get-Item -LiteralPath $MkDebugLog -ErrorAction Stop
        if ($file.LastWriteTime -lt $Since) { return "makemkv debug log: none written by this run ($MkDebugLog is older)" }
        $text = [IO.File]::ReadAllText($file.FullName)
        $key = [string](Get-ItemProperty 'HKCU:\Software\MakeMKV' -ErrorAction SilentlyContinue).app_Key
        if ($key -and $text.Contains($key)) { $text = $text.Replace($key, '[app_Key removed]') }
        [IO.File]::WriteAllText($To, $text, (New-Object Text.UTF8Encoding($false)))
        "makemkv debug log: $(Split-Path $To -Leaf) $($file.Length) bytes"
    } catch { "makemkv debug log: NOT SAVED: $($_.Exception.Message)" }
}

# Keeps the warnings and errors Windows logged since $Since about storage, USB and the network (for a failed
# read or a lost share). Never fails the caller: it returns one line for the job log.
function Save-SystemEvents([datetime]$Since, [string]$To) {
    try {
        $found = @()
        foreach ($log in 'System', 'Microsoft-Windows-SmbClient/Connectivity') {
            $found += @(Get-WinEvent -FilterHashtable @{ LogName = $log; StartTime = $Since; Level = 1, 2, 3 } -ErrorAction SilentlyContinue)
        }
        $wanted = 'cdrom|disk|stor|usb|ntfs|udfs|volmgr|smb|mrxsmb|pnp|scsi|nvme|Kernel-Power|Tcpip|NDIS|NetBT|Netw|e1d|e1r'
        $kept = @($found | Where-Object { $_.ProviderName -match $wanted } | Sort-Object TimeCreated | ForEach-Object {
            [ordered]@{
                time = $_.TimeCreated.ToString('yyyy-MM-dd HH:mm:ss'); log = $_.LogName; provider = $_.ProviderName
                id = $_.Id; level = $_.LevelDisplayName; message = ([string]$_.Message -split "`r?`n")[0]
            }
        })
        Set-Content -LiteralPath $To -Value (ConvertTo-Json @($kept)) -Encoding UTF8 -ErrorAction Stop
        "system events: $(Split-Path $To -Leaf) $($kept.Count) kept of $($found.Count) warnings and errors since $($Since.ToString('HH:mm:ss'))"
    } catch { "system events: NOT SAVED: $($_.Exception.Message)" }
}

# MakeMKV's index for the drive letter in $Drive, read from the seventh DRV field.
function Get-DiscIndex {
    $pattern = '^DRV:(\d+),\d+,\d+,\d+,".*",".*","' + [regex]::Escape("${Drive}:") + '"\s*$'
    $line = & $MK -r --cache=1 info disc:9999 | Where-Object { $_ -match $pattern } | Select-Object -First 1
    if (-not $line) { throw "MakeMKV lists no drive at ${Drive}:" }
    [int]([regex]::Match($line, '^DRV:(\d+),').Groups[1].Value)
}

function Get-FreeRamGB {
    [math]::Round((Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory / 1MB, 2)
}

$Recordings = "$P0\recordings"

function Assert-Slug([string]$Value) {
    if ($Value -cnotmatch '^[a-z0-9][a-z0-9-]*$') { throw "bad name '$Value': use lowercase letters, digits and hyphens only" }
}

function Assert-EmptyDir([string]$Path) {
    if (-not $Path) { throw 'Assert-EmptyDir: no path given' }
    New-Item -ItemType Directory -Force -Path $Path -ErrorAction Stop | Out-Null
    $items = @(Get-ChildItem -LiteralPath $Path -Force -ErrorAction Stop)
    if ($items.Count -gt 0) { throw "$Path is not empty ($($items.Count) entries)" }
}

# Reads the disc in $Drive with lazymkv-capture only (never MakeMKV); returns its meta.json object and prints nothing.
function Get-InsertedDisc {
    if (-not (Test-Path -LiteralPath $CAP)) { throw "missing $CAP; build it with: scripts/box.sh cargo build --release -p lazymkv-capture" }
    if (@(Get-ChildItem -LiteralPath "${Drive}:\" -ErrorAction SilentlyContinue).Count -eq 0) { throw "no disc is mounted in ${Drive}:" }
    $dir = "$P0\tmp\disc-check"
    if (Test-Path -LiteralPath $dir) { Remove-Item -LiteralPath $dir -Recurse -Force -ErrorAction Stop }
    New-Item -ItemType Directory -Force -Path $dir -ErrorAction Stop | Out-Null
    & $CAP disc --root "${Drive}:\" --label check --out $dir | Out-Null
    $code = $LASTEXITCODE
    if ($code -ne 0 -or -not (Test-Path -LiteralPath "$dir\meta.json")) { throw "lazymkv-capture disc failed on ${Drive}: (exit $code)" }
    $meta = Get-Content -LiteralPath "$dir\meta.json" -Raw -Encoding UTF8 -ErrorAction Stop | ConvertFrom-Json
    if (-not $meta.fingerprint) { throw "the disc in ${Drive}: has no fingerprint" }
    $meta
}

function Assert-DiscIs([string]$Slug) {
    Assert-Slug $Slug
    $file = "$Recordings\$Slug\meta.json"
    if (-not (Test-Path -LiteralPath $file)) { throw "no $file" }
    $want = [string](Get-Content -LiteralPath $file -Raw -Encoding UTF8 -ErrorAction Stop | ConvertFrom-Json).fingerprint
    $have = [string](Get-InsertedDisc).fingerprint
    if (-not $want -or $have -ne $want) { throw "the disc in ${Drive}: (fingerprint $have) is not the disc of recording $Slug (recorded fingerprint $(if ($want) { $want } else { 'none' }))" }
}
