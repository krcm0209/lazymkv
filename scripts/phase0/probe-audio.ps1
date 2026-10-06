# Reads the audio streams of a title that rip-title.ps1 ripped successfully once more with ffprobe, this time
# probing deeper than probe-title.ps1 does (-probesize 100M -analyzeduration 100M). Experiment 6 compares the two
# reads, so that a missing Atmos or DTS:X profile is known to be ffprobe's answer and not a read that was too short.
# It reads the one .mkv in $Staging\<Slug>\<Label>\ and writes, in $Recordings\<Slug>\<Label>\
# (C:\lazymkv\phase0\recordings\<Slug>\<Label>\), ffprobe-audio-deep.json with .timing.jsonl, .stderr and
# .exit.json beside it. Those files are never replaced unless -Force is given. Nothing is written to the share.
# The last line is "RESULT ok" or "RESULT FAILED: <reason>"; after a failure the script throws.
param(
    [Parameter(Mandatory)][string]$Slug,
    [Parameter(Mandatory)][string]$Label,
    [switch]$Force
)
. C:\lazymkv\src\scripts\phase0\env.ps1

try {
    Assert-Slug $Slug
    Assert-Slug $Label
    if (-not (Test-Path -LiteralPath $CAP)) { throw "missing $CAP; build it with: scripts/box.sh cargo build --release -p lazymkv-capture" }
    if (-not $FFPROBE) { throw 'ffprobe.exe not found' }
    $rec = "$Recordings\$Slug\$Label"
    $dest = "$Staging\$Slug\$Label"
    $out = "$rec\ffprobe-audio-deep.json"
    $outFiles = @($out, "$out.timing.jsonl", "$out.stderr", "$out.exit.json")

    # Only a rip that rip-title.ps1 judged good is read.
    $ripFile = "$rec\rip-summary.json"
    if (-not (Test-Path -LiteralPath $ripFile)) { throw "no $ripFile; rip the title with rip-title.ps1 first" }
    $rip = Get-Content -LiteralPath $ripFile -Raw -Encoding UTF8 -ErrorAction Stop | ConvertFrom-Json
    if ($rip.ok -ne $true) { throw "the rip of label $Label did not succeed ($($rip.result)); only a rip whose rip-summary.json says ok is read" }

    # Evidence is never replaced without -Force.
    foreach ($file in $outFiles) {
        if ((Test-Path -LiteralPath $file) -and -not $Force) {
            throw "$file already exists and is kept as evidence; use -Force only where the plan says so"
        }
    }

    # The file is found by listing the folder: it must be the one .mkv that rip-title.ps1 recorded.
    $mkvs = @(Get-ChildItem -LiteralPath $dest -File -Force -ErrorAction Stop | Where-Object { $_.Extension -eq '.mkv' })
    if ($mkvs.Count -ne 1) { throw "expected one .mkv in $dest, found $($mkvs.Count)" }
    $mkv = $mkvs[0]
    if (@($rip.files) -notcontains $mkv.Name -or $mkv.Length -ne [int64]$rip.bytes) {
        throw "$($mkv.Name) ($($mkv.Length) bytes) is not the file rip-title.ps1 recorded ($(@($rip.files) -join ', '), $($rip.bytes) bytes)"
    }

    # Files exist here only under -Force: remove them, so that stale files cannot pass for this read's.
    foreach ($file in $outFiles) {
        if (Test-Path -LiteralPath $file) { Remove-Item -LiteralPath $file -Force -ErrorAction Stop }
    }
    "probe-audio: $($mkv.FullName) ($($mkv.Length) bytes)"
    & $CAP record --out $out -- $FFPROBE -v error -probesize '100M' -analyzeduration '100M' -select_streams a -show_streams -of json $mkv.FullName | Out-Null
    $code = $LASTEXITCODE
    if (-not (Test-Path -LiteralPath "$out.exit.json")) { throw "lazymkv-capture wrote no ffprobe-audio-deep.json.exit.json (exit $code)" }
    if ($code -ne 0) { throw "ffprobe exited $code; see ffprobe-audio-deep.json.stderr" }
    $data = Get-Content -LiteralPath $out -Raw -Encoding UTF8 -ErrorAction Stop | ConvertFrom-Json
    $streams = @($data.streams | Where-Object { $_ })
    if ($streams.Count -eq 0) { throw 'ffprobe lists no audio stream; see ffprobe-audio-deep.json' }
    foreach ($s in $streams) {
        "audio $($s.index): $($s.codec_name) profile=$($s.profile) channels=$($s.channels) layout=$($s.channel_layout)"
    }
    'RESULT ok'
} catch {
    "RESULT FAILED: $($_.Exception.Message)"
    throw
}
