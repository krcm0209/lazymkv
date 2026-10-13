# Inspects a title that rip-title.ps1 ripped successfully: identification, stream profiles, subtitle evidence and
# the time of a copy-everything remux on the share (spec 6.4 steps 9, 10 and 13).
# It reads the one .mkv in $Staging\<Slug>\<Label>\ and writes to $Recordings\<Slug>\<Label>\:
#   mkvmerge.json, ffprobe-streams.json, mediainfo.json     identification and stream profiles
#   subs\index.json, subs\sub-<n>.packets.json             subtitle evidence, when the title has subtitles
#   evidence-mkvmerge.txt, remux-mkvmerge.txt               mkvmerge's messages from the extraction and the remux
#   probe-summary.json                                      times, sizes and the verdict
# lazymkv-capture adds .timing.jsonl, .stderr and .exit.json beside each recorded output.
# On the share it writes evidence.mks (kept) and remux-test.tmp (deleted once timed; not a .mkv, so it can never
# be taken for MakeMKV's file). -SkipRemux leaves the remux out. Probe evidence that already exists is refused
# unless -Force is given, which deletes it first; the rip's own files are never touched.
# The last line is "RESULT ok" or "RESULT FAILED: <reason>"; after a failure the script throws.
param(
    [Parameter(Mandatory)][string]$Slug,
    [Parameter(Mandatory)][string]$Label,
    [switch]$SkipRemux,
    [switch]$Force
)
. C:\lazymkv\src\scripts\phase0\env.ps1

$failure = $null
$summary = $null
try {
    Assert-Slug $Slug
    Assert-Slug $Label
    if (-not (Test-Path -LiteralPath $CAP)) { throw "missing $CAP; build it with: scripts/box.sh cargo build --release -p lazymkv-capture" }
    if (-not $MKVMERGE) { throw 'mkvmerge.exe not found' }
    if (-not $FFPROBE) { throw 'ffprobe.exe not found' }
    $rec = "$Recordings\$Slug\$Label"
    $dest = "$Staging\$Slug\$Label"

    # Only a rip that rip-title.ps1 judged good is probed.
    $ripFile = "$rec\rip-summary.json"
    if (-not (Test-Path -LiteralPath $ripFile)) { throw "no $ripFile; rip the title with rip-title.ps1 first" }
    $rip = Get-Content -LiteralPath $ripFile -Raw -Encoding UTF8 -ErrorAction Stop | ConvertFrom-Json
    if ($rip.ok -ne $true) { throw "the rip of label $Label did not succeed ($($rip.result)); only a rip whose rip-summary.json says ok is probed" }

    # Probe evidence is never replaced without -Force; the rip's own files stay.
    # What rip-title.ps1 leaves here, and the audio measurements, are not probe evidence.
    $ripFiles = 'rip.txt', 'rip.txt.timing.jsonl', 'rip.txt.stderr', 'rip.txt.exit.json', 'rip.txt.makemkv-debug.txt',
        'free-ram.csv', 'system.csv', 'system-events.json', 'rip-summary.json'
    $old = @(Get-ChildItem -LiteralPath $rec -Force -ErrorAction Stop | Where-Object { $ripFiles -notcontains $_.Name -and $_.Name -notlike 'audio-levels-*' })
    if ($old.Count -gt 0 -and -not $Force) {
        throw "$rec already holds probe evidence ($(($old | ForEach-Object { $_.Name }) -join ', ')), which is kept; use -Force only where the plan says so"
    }
    foreach ($item in $old) { Remove-Item -LiteralPath $item.FullName -Recurse -Force -ErrorAction Stop }

    # The file is found by listing the folder: it must be the one .mkv that rip-title.ps1 recorded.
    $mkvs = @(Get-ChildItem -LiteralPath $dest -File -Force -ErrorAction Stop | Where-Object { $_.Extension -eq '.mkv' })
    if ($mkvs.Count -ne 1) { throw "expected one .mkv in $dest, found $($mkvs.Count)" }
    $file = $mkvs[0]
    if (@($rip.files) -notcontains $file.Name -or $file.Length -ne [int64]$rip.bytes) {
        throw "$($file.Name) ($($file.Length) bytes) is not the file rip-title.ps1 recorded ($(@($rip.files) -join ', '), $($rip.bytes) bytes)"
    }
    $mkv = $file.FullName
    $evidence = "$dest\evidence.mks"
    $remux = "$dest\remux-test.tmp"
    foreach ($scratch in $evidence, $remux) {
        if (Test-Path -LiteralPath $scratch) { Remove-Item -LiteralPath $scratch -Force -ErrorAction Stop }
    }

    $summary = [ordered]@{
        ok = $false; result = $null; file = $mkv; file_bytes = $file.Length; subtitle_tracks = $null
        evidence_s = $null; evidence_bytes = $null
        remux_skipped = [bool]$SkipRemux; remux_s = $null; remux_exit = $null; remux_bytes = $null; remux_valid = $null; remux_deleted = $null
    }
    "probe: $mkv ($($file.Length) bytes)"

    # Identification and stream profiles: header reads. mkvmerge exits 0, or 1 for warnings only; any other code is a
    # failure, a crash too: lazymkv-capture passes the child's code on as it is, so a crash is a negative NTSTATUS.
    & $CAP record --out "$rec\mkvmerge.json" -- $MKVMERGE -J $mkv | Out-Null
    if ($LASTEXITCODE -ne 0 -and $LASTEXITCODE -ne 1) { throw "mkvmerge -J exited $LASTEXITCODE; see mkvmerge.json" }
    $ident = Get-Content -LiteralPath "$rec\mkvmerge.json" -Raw -Encoding UTF8 -ErrorAction Stop | ConvertFrom-Json
    if (@($ident.tracks).Count -eq 0) { throw 'mkvmerge -J lists no tracks; see mkvmerge.json' }
    & $CAP record --out "$rec\ffprobe-streams.json" -- $FFPROBE -v error -show_streams -show_format -of json $mkv | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "ffprobe exited $LASTEXITCODE; see ffprobe-streams.json.stderr" }
    if ($MEDIAINFO) {
        & $CAP record --out "$rec\mediainfo.json" -- $MEDIAINFO --Output=JSON $mkv | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "MediaInfo exited $LASTEXITCODE; see mediainfo.json.stderr" }
    }

    # Subtitle evidence (spec 6.4 step 10): every subtitle track into one small file, then each stream's packets.
    $subs = @($ident.tracks | Where-Object { $_.type -eq 'subtitles' })
    $summary['subtitle_tracks'] = $subs.Count
    $summary['evidence_s'] = 0
    $summary['evidence_bytes'] = 0
    if ($subs.Count -gt 0) {
        New-Item -ItemType Directory -Force -Path "$rec\subs" -ErrorAction Stop | Out-Null
        $ids = ($subs | ForEach-Object { $_.id }) -join ','
        $t0 = Get-Date
        & $CAP record --out "$rec\evidence-mkvmerge.txt" -- $MKVMERGE -q -o $evidence -D -A -s $ids $mkv | Out-Null
        $code = $LASTEXITCODE
        $summary['evidence_s'] = [int]((Get-Date) - $t0).TotalSeconds
        if (($code -ne 0 -and $code -ne 1) -or -not (Test-Path -LiteralPath $evidence)) { throw "the subtitle extraction failed (mkvmerge exit $code); see evidence-mkvmerge.txt" }
        $summary['evidence_bytes'] = (Get-Item -LiteralPath $evidence -ErrorAction Stop).Length
        if ($summary['evidence_bytes'] -eq 0) { throw "$evidence is empty" }
        $index = @()
        for ($n = 0; $n -lt $subs.Count; $n++) {
            & $CAP record --out "$rec\subs\sub-$n.packets.json" -- $FFPROBE -v error -select_streams "s:$n" -show_entries 'packet=pts_time,duration_time,size' -of json $evidence | Out-Null
            if ($LASTEXITCODE -ne 0) { throw "ffprobe could not list the packets of subtitle stream s:$n (exit $LASTEXITCODE)" }
            $index += [ordered]@{
                n          = $n
                track_id   = $subs[$n].id
                language   = $subs[$n].properties.language
                codec      = $subs[$n].codec
                track_name = $subs[$n].properties.track_name
                forced     = $subs[$n].properties.forced_track
                default    = $subs[$n].properties.default_track
            }
        }
        Set-Content -LiteralPath "$rec\subs\index.json" -Value (ConvertTo-Json @($index)) -Encoding UTF8 -ErrorAction Stop
    }

    # A copy-everything remux from the share back to the share: the file phase's cost (spec 6.4 step 13, section 18).
    if (-not $SkipRemux) {
        $t0 = Get-Date
        & $CAP record --out "$rec\remux-mkvmerge.txt" -- $MKVMERGE -q -o $remux $mkv | Out-Null
        $code = $LASTEXITCODE
        $summary['remux_s'] = [int]((Get-Date) - $t0).TotalSeconds
        $summary['remux_exit'] = $code
        $summary['remux_bytes'] = 0
        if (Test-Path -LiteralPath $remux) { $summary['remux_bytes'] = (Get-Item -LiteralPath $remux).Length }
        $summary['remux_valid'] = (($code -eq 0 -or $code -eq 1) -and $summary['remux_bytes'] -gt 0.9 * $file.Length)
        Remove-Item -LiteralPath $remux -Force -ErrorAction SilentlyContinue
        $summary['remux_deleted'] = -not (Test-Path -LiteralPath $remux)
        if (-not $summary['remux_valid']) { throw "the remux failed (mkvmerge exit $code, $($summary['remux_bytes']) of $($file.Length) bytes); see remux-mkvmerge.txt" }
    }
} catch {
    $failure = $_.Exception.Message
}

if ($null -ne $summary) {
    $summary['ok'] = -not $failure
    $summary['result'] = 'ok'
    if ($failure) { $summary['result'] = "FAILED: $failure" }
    $json = $summary | ConvertTo-Json
    try { Set-Content -LiteralPath "$rec\probe-summary.json" -Value $json -Encoding UTF8 -ErrorAction Stop } catch { if (-not $failure) { $failure = "cannot write probe-summary.json: $($_.Exception.Message)" } }
    $json
}
if ($failure) {
    "RESULT FAILED: $failure"
    throw "probe-title.ps1: $failure"
}
'RESULT ok'
