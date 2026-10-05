# Dot-source on the mini PC:  . C:\lazymkv\src\scripts\phase0\env.ps1
$MK  = 'C:\Program Files (x86)\MakeMKV\makemkvcon64.exe'
$P0  = 'C:\lazymkv\phase0'
$CAP = 'C:\lazymkv\target\release\lazymkv-capture.exe'
$AllTracksProfile = "$P0\profiles\all-tracks.mmcp.xml"
$EngProfile       = "$P0\profiles\eng-only.mmcp.xml"

# Owner-specific values: $Staging, $Drive, $TestLibrary.
if (-not (Test-Path "$P0\config.ps1")) { throw "missing $P0\config.ps1 (the owner's settings)" }
. "$P0\config.ps1"
if (-not $Staging -or -not $Drive) { throw "$P0\config.ps1 must set `$Staging and `$Drive" }

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

function Get-MkFlags([string]$Profile = $AllTracksProfile, [int]$CacheMb = 1024) {
    @('-r', '--messages=-stdout', '--progress=-same', '--minlength=120', "--cache=$CacheMb", "--profile=$Profile")
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
