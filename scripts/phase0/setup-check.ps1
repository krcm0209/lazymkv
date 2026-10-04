# Verifies tools, profiles, the drive and the staging share from the owner's desktop session.
. C:\lazymkv\src\scripts\phase0\env.ps1

"makemkvcon : " + ((& $MK -r --cache=1 info disc:9999 | Select-Object -First 1))
"mkvmerge   : $MKVMERGE : " + $(if ($MKVMERGE) { (& $MKVMERGE --version) } else { 'MISSING' })
"ffprobe    : $FFPROBE : " + $(if ($FFPROBE) { (& $FFPROBE -version | Select-Object -First 1) } else { 'MISSING' })
"ffmpeg     : $FFMPEG"
"mediainfo  : $MEDIAINFO : " + $(if ($MEDIAINFO) { (& $MEDIAINFO --Version | Select-Object -Last 1) } else { 'MISSING' })
"java       : " + (Get-ItemProperty 'HKCU:\Software\MakeMKV' -ErrorAction SilentlyContinue).app_Java
"drive      : ${Drive}: is MakeMKV disc index $(Get-DiscIndex)"
"free RAM   : $(Get-FreeRamGB) GB"
"TMDB_TOKEN : " + $(if ($env:TMDB_TOKEN) { 'set' } else { 'NOT SET' })
"profiles   : all-tracks=$(Test-Path $AllTracksProfile) eng-only=$(Test-Path $EngProfile)"

# Every step must stop on failure and is then checked, so "OK" is only printed when all three worked.
$probe = "$Staging\lazymkv-write-test.txt"
$renamed = "$Staging\lazymkv-write-test-renamed.txt"
try {
    New-Item -ItemType Directory -Force $Staging -ErrorAction Stop | Out-Null
    Remove-Item $probe, $renamed -ErrorAction SilentlyContinue
    Set-Content $probe 'ok' -ErrorAction Stop
    Rename-Item $probe 'lazymkv-write-test-renamed.txt' -ErrorAction Stop
    if ((Test-Path $probe) -or -not (Test-Path $renamed)) { throw 'the rename did not take effect' }
    Remove-Item $renamed -ErrorAction Stop
    if (Test-Path $renamed) { throw 'the delete did not take effect' }
    "staging    : $Staging : write, rename and delete OK"
} catch {
    "staging    : $Staging : FAILED: $($_.Exception.Message)"
}
Add-Type -Namespace Win32 -Name Disk -MemberDefinition '[DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)] public static extern bool GetDiskFreeSpaceEx(string path, out ulong freeToCaller, out ulong total, out ulong totalFree);'
[uint64]$free = 0; [uint64]$total = 0; [uint64]$totalFree = 0
if ([Win32.Disk]::GetDiskFreeSpaceEx("$Staging\", [ref]$free, [ref]$total, [ref]$totalFree)) {
    "staging free: $([math]::Round($free / 1GB, 1)) GB"
} else {
    "staging free: FAILED (Win32 error $([Runtime.InteropServices.Marshal]::GetLastWin32Error()))"
}
