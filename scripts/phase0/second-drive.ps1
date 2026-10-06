# Experiments 5 and 7 (Task 14): the disc recorded under -Slug, read in another drive (the second drive) at
# -Letter. Run it over plain ssh: it needs C: and the drive, never the share. It writes
# $Recordings\<Slug>\drive2\ (C:\lazymkv\phase0\recordings\<Slug>\drive2\):
#   drives.txt with .timing.jsonl, .stderr and .exit.json   MakeMKV's drive listing (info disc:9999)
#   drives.txt.makemkv-debug.txt   MakeMKV's debug log of that call (decision D16)
#   snapshot\meta.json, snapshot\files.json, snapshot\nav\  lazymkv-capture disc for <Letter>:\ (fingerprint, disc ID)
# Both are made in $P0\tmp\drive2-<Slug> first and moved into the recording only when the disc in <Letter>: has
# the recording's fingerprint, so a failed run leaves the recording unchanged. Assert-DiscIs is not used because
# it reads the drive in $Drive, not <Letter>:; the same fingerprint check is made here. A disc ID that cannot be
# read is a finding, not a failure. An existing drive2 folder is refused unless -Force is given.
# The last line is "RESULT ok" or "RESULT FAILED: <reason>"; after a failure the script throws.
param(
    [Parameter(Mandatory)][string]$Slug,
    [Parameter(Mandatory)][string]$Letter,
    [switch]$Force
)
. C:\lazymkv\src\scripts\phase0\env.ps1

try {
    Assert-Slug $Slug
    if ($Letter -notmatch '^[A-Za-z]$') { throw "bad drive letter: '$Letter' (one letter, without a colon)" }
    $Letter = $Letter.ToUpper()
    $type = $null
    try { $type = [string]([System.IO.DriveInfo]::new($Letter).DriveType) } catch { }
    if ($type -ne 'CDRom') { throw "${Letter}: is not an optical drive (drive type: $type)" }
    if (-not (Test-Path -LiteralPath $CAP)) { throw "missing $CAP; build it with: scripts/box.sh cargo build --release -p lazymkv-capture" }
    $metaFile = "$Recordings\$Slug\meta.json"
    if (-not (Test-Path -LiteralPath $metaFile)) { throw "no $metaFile; the disc must be recorded first" }
    $first = Get-Content -LiteralPath $metaFile -Raw -Encoding UTF8 -ErrorAction Stop | ConvertFrom-Json
    if (-not $first.fingerprint) { throw "$metaFile has no fingerprint" }
    $out = "$Recordings\$Slug\drive2"
    if ((Test-Path -LiteralPath $out) -and -not $Force) { throw "$out already exists and is kept as evidence; use -Force only where the plan says so" }
    $stage = "$P0\tmp\drive2-$Slug"
    if (Test-Path -LiteralPath $stage) { Remove-Item -LiteralPath $stage -Recurse -Force -ErrorAction Stop }
    Assert-EmptyDir $stage

    # The disc may still be spinning up: wait up to two minutes until its root can be listed.
    $ready = $false
    for ($i = 0; $i -lt 24 -and -not $ready; $i++) {
        if (@(Get-ChildItem -LiteralPath "${Letter}:\" -ErrorAction SilentlyContinue).Count -gt 0) { $ready = $true } else { Start-Sleep -Seconds 5 }
    }
    if (-not $ready) { throw "no readable disc in ${Letter}: after 120 s" }

    # MakeMKV's view of the drive: its index, its name (the model) and its state.
    $listStart = Get-Date
    & $CAP record --out "$stage\drives.txt" -- $MK -r --cache=1 --debug info disc:9999 | Out-Null
    Save-MkDebugLog -Since $listStart -To "$stage\drives.txt.makemkv-debug.txt"
    $raw = [string](Get-Content -LiteralPath "$stage\drives.txt" -Raw -ErrorAction SilentlyContinue)
    $drv = [regex]::Match($raw, '(?m)^DRV:(\d+),(\d+),\d+,\d+,"([^"]*)","([^"]*)","' + [regex]::Escape("${Letter}:") + '"\s*$')
    if (-not $drv.Success) { throw "MakeMKV lists no drive at ${Letter}: (see $stage\drives.txt)" }
    "drive: ${Letter}: is MakeMKV index $($drv.Groups[1].Value), state $($drv.Groups[2].Value), name '$($drv.Groups[3].Value)', disc label '$($drv.Groups[4].Value)'"
    if ($drv.Groups[2].Value -ne '2') { throw "MakeMKV does not list a disc in ${Letter}: (DRV state $($drv.Groups[2].Value); 2 means a disc is inserted)" }

    # The snapshot: the fingerprint and the disc ID as this drive presents the disc.
    $label = $null
    try { $label = ([System.IO.DriveInfo]::new($Letter)).VolumeLabel } catch { }
    if (-not $label) { $label = '(none)' }
    & $CAP disc --root "${Letter}:\" --label $label --out "$stage\snapshot" | Out-Null
    $capExit = $LASTEXITCODE
    if ($capExit -ne 0 -or -not (Test-Path -LiteralPath "$stage\snapshot\meta.json")) { throw "lazymkv-capture disc failed on ${Letter}: (exit $capExit)" }
    $meta = Get-Content -LiteralPath "$stage\snapshot\meta.json" -Raw -Encoding UTF8 -ErrorAction Stop | ConvertFrom-Json
    "snapshot: label=$label fingerprint=$($meta.fingerprint) disc_id=$($meta.disc_id) files=$($meta.file_count)"
    "first drive (meta.json): fingerprint=$($first.fingerprint) disc_id=$($first.disc_id)"
    if (-not $meta.fingerprint -or $meta.fingerprint -ne $first.fingerprint) {
        throw "the disc in ${Letter}: is not the disc of recording $Slug (fingerprint $($meta.fingerprint), recorded $($first.fingerprint))"
    }
    $sameId = 'not compared'
    if ($meta.disc_id -and $first.disc_id) { $sameId = [string]($meta.disc_id -eq $first.disc_id) }
    "disc_id: second drive $(if ($meta.disc_id) { 'read' } else { 'NOT read' }); same as the first drive: $sameId"

    if (Test-Path -LiteralPath $out) { Remove-Item -LiteralPath $out -Recurse -Force -ErrorAction Stop }
    Move-Item -LiteralPath $stage -Destination $out -ErrorAction Stop
    "saved: $out"
    'RESULT ok'
} catch {
    "RESULT FAILED: $($_.Exception.Message)"
    throw
}
