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
