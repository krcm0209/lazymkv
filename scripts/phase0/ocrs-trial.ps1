# The ocrs trial (decisions D17 and D19; docs/phase0/ocr-engine-research.md, section "Trial"): reads the frames that
# ocr-subs.ps1 kept for one subtitle stream with the pure-Rust OCR engine ocrs (experiments/ocrs-trial), so that
# lazymkv-dev ocr-compare can set its text beside Tesseract's on the same frames, and so that its speed on the
# J4125 (no AVX2) is measured.
# Input: $TuningData\<Slug>\<Label>\<Folder>\s<Stream>\f-*.png (ocr-subs.ps1). The frames are first copied to
# $P0\tmp\ocrs-trial\<Slug>-<Label>-s<Stream>[-crop<pad>]\ on C: so that the timing measures OCR, not the share;
# the copy is refused above 2 GB and deleted at the end, also after a failure. -Limit N copies and reads only the first
# N frames (name order, so frame k is ocr-subs.ps1's frame k).
# -Crop <pad> (decision D22: the app reads only the subtitle's glyph box) passes --crop <pad>: ocrs then reads only the
# box of the dark pixels of each frame, grown by <pad> pixels, and skips a frame without one. A cropped reading is kept
# beside a full-frame one: its files are named ocrs-s<n>-crop<pad>.* and its ocrs-summary.json entry carries crop=<pad>
# (crop=0, the existing names, is the full frame).
# The exe and the models are fixed paths, like $CAP in env.ps1: C:\lazymkv\target\release\ocrs-trial.exe (built with
# scripts/box.sh cargo build --release --manifest-path experiments/ocrs-trial/Cargo.toml) and, copied there by hand,
# $P0\models\text-detection.onnx and text-recognition.onnx (the files ocrs 0.13.1 documents; CC-BY-SA-4.0, never in
# the repository; see experiments/ocrs-trial/README.md).
#
# In $TuningData\<Slug>\<Label>\<Folder>\ (the tuning dataset, decision D16; the text stays there):
#   ocrs-s<n>.jsonl      one line per frame (frame, file, lines, times; with -Crop the box) and a last summary line
#                        (ocrs-trial's output; experiments/ocrs-trial/README.md); -Crop: ocrs-s<n>-crop<pad>.jsonl
# In $Recordings\<Slug>\<Label>\<Folder>\ (small files, pulled into the corpus; no subtitle text):
#   ocrs-s<n>.txt        ocrs-trial's standard output (progress and timing only) with .stderr/.timing.jsonl/.exit.json
#                        (-Crop: ocrs-s<n>-crop<pad>.txt and so on)
#   ocrs-summary.json    per stream and crop read: frames, frames with text, lines, the marker counts of ocr-subs.ps1
#                        (bracketed lines, music notes, lines beginning or ending with #, speaker labels, upper-case
#                        lines), the times of ocrs-trial's summary line (OCR per frame and per frame read, decode, crop,
#                        detection, recognition), the crop boxes, model load time, the ocrs version and the models'
#                        SHA-1. Written last; a stream and crop read again replaces its own entry and keeps the others.
# The log shows the first <Show> frames with text so that the OCR quality can be judged; the log stays on the mini PC.
# An earlier reading of the same stream with the same -Crop (any of its files, or its entry in ocrs-summary.json) is
# refused unless -Force is given.
# The last line is "RESULT ok" or "RESULT FAILED: <reason>"; after a failure the script throws.
param(
    [Parameter(Mandatory)][string]$Slug,
    [Parameter(Mandatory)][string]$Label,
    [int]$Stream = 0,
    [string]$Folder = 'subs-ocr',
    [int]$Limit = 0,
    [int]$Show = 20,
    [int]$Crop = 0,
    [switch]$Force
)
. C:\lazymkv\src\scripts\phase0\env.ps1

$OCRS = 'C:\lazymkv\target\release\ocrs-trial.exe'
$OcrsModels = "$P0\models"
$DetModel = "$OcrsModels\text-detection.onnx"
$RecModel = "$OcrsModels\text-recognition.onnx"
$MaxBytes = 2GB
$NOTE = [string][char]0x266A + [char]0x266B

# The counts of ocr-subs.ps1's Count-Markers, with the same patterns (keep the two in step); no example lines.
function Count-Markers([string[]]$Lines) {
    $m = [ordered]@{ bracket = 0; note = 0; hash = 0; speaker = 0; upper = 0 }
    foreach ($line in $Lines) {
        if ($line -match '[\[\(].*[\]\)]') { $m['bracket']++ }
        if ($line.IndexOfAny($NOTE.ToCharArray()) -ge 0) { $m['note']++ }
        if ($line -match '^\s*#|#\s*$') { $m['hash']++ }
        if ($line -cmatch '^\s*[A-Z][A-Z0-9 .''\-]{0,24}:(\s|$)') { $m['speaker']++ }
        if ($line -cnotmatch '[a-z]' -and $line -cmatch '[A-Z].*[A-Z].*[A-Z]') { $m['upper']++ }
    }
    $m
}

function Get-Sha1([string]$Path) {
    (Get-FileHash -LiteralPath $Path -Algorithm SHA1 -ErrorAction Stop).Hash.ToLowerInvariant()
}

$work = $null
try {
    Assert-Slug $Slug
    Assert-Slug $Label
    Assert-Slug $Folder
    if ($Stream -lt 0) { throw '-Stream must be 0 or more' }
    if ($Limit -lt 0) { throw '-Limit must be 0 (every frame) or more' }
    if ($Show -lt 0) { throw '-Show must be 0 or more' }
    if ($Crop -lt 0 -or $Crop -gt 1000) { throw '-Crop must be 0 (the full frame) or a padding of 1 to 1000 pixels' }
    if (-not (Test-Path -LiteralPath $CAP)) { throw "missing $CAP; build it with: scripts/box.sh cargo build --release -p lazymkv-capture" }
    if (-not (Test-Path -LiteralPath $OCRS)) { throw "missing $OCRS; build it with: scripts/box.sh cargo build --release --manifest-path experiments/ocrs-trial/Cargo.toml" }
    foreach ($model in @($DetModel, $RecModel)) {
        if (-not (Test-Path -LiteralPath $model)) { throw "missing $model; copy text-detection.onnx and text-recognition.onnx to $OcrsModels (experiments/ocrs-trial/README.md)" }
    }
    if (-not $TuningData) { throw "`$TuningData is not set (C:\lazymkv\phase0\config.ps1)" }

    $tune = "$TuningData\$Slug\$Label\$Folder"
    $src = "$tune\s$Stream"
    $rec = "$Recordings\$Slug\$Label\$Folder"
    if (-not (Test-Path -LiteralPath $src)) { throw "no frames in $src; run ocr-subs.ps1 for this title (and stream s:$Stream) first" }
    if (-not (Test-Path -LiteralPath $rec)) { throw "no recording $rec; run ocr-subs.ps1 first" }
    # The name of this reading: s<n> for the full frame, s<n>-crop<pad> with -Crop.
    $tag = if ($Crop -gt 0) { "s$Stream-crop$Crop" } else { "s$Stream" }
    $jsonlOut = "$tune\ocrs-$tag.jsonl"
    $runOut = "$rec\ocrs-$tag.txt"
    $summaryFile = "$rec\ocrs-summary.json"
    $others = @()
    $had = $false
    if (Test-Path -LiteralPath $summaryFile) {
        $old = Get-Content -LiteralPath $summaryFile -Raw -Encoding UTF8 -ErrorAction Stop | ConvertFrom-Json
        # An entry without crop (written before -Crop existed) is a full-frame reading: [int]$null is 0.
        $others = @($old.streams | Where-Object { -not ([int]$_.n -eq $Stream -and [int]$_.crop -eq $Crop) })
        $had = @($old.streams | Where-Object { [int]$_.n -eq $Stream -and [int]$_.crop -eq $Crop }).Count -gt 0
    }
    $earlier = @(@($jsonlOut, $runOut, "$runOut.stderr", "$runOut.timing.jsonl", "$runOut.exit.json") | Where-Object { Test-Path -LiteralPath $_ })
    if (($earlier.Count -gt 0 -or $had) -and -not $Force) {
        $what = @($earlier)
        if ($had) { $what += "the s:$Stream crop=$Crop entry of $summaryFile" }
        throw "stream s:$Stream was read with ocrs (crop $Crop) before ($($what -join ', ')); that is kept as evidence; use -Force to replace it"
    }
    # -Force: the earlier reading goes first, so that a failed run cannot leave its files beside a new summary entry.
    foreach ($file in $earlier) { Remove-Item -LiteralPath $file -Force -ErrorAction Stop }
    if ($had -and $others.Count -gt 0) {
        Set-Content -LiteralPath $summaryFile -Value (ConvertTo-Json -Depth 6 ([ordered]@{ slug = $Slug; label = $Label; folder = $Folder; streams = @($others) })) -Encoding UTF8 -ErrorAction Stop
    } elseif ($had) {
        Remove-Item -LiteralPath $summaryFile -Force -ErrorAction Stop
    }

    $frames = @(Get-ChildItem -LiteralPath $src -Filter 'f-*.png' -File -ErrorAction Stop | Sort-Object Name)
    if ($frames.Count -eq 0) { throw "$src holds no f-*.png frame" }
    $available = $frames.Count
    if ($Limit -gt 0 -and $frames.Count -gt $Limit) { $frames = @($frames[0..($Limit - 1)]) }
    $bytes = [long]($frames | Measure-Object -Property Length -Sum).Sum
    if ($bytes -gt $MaxBytes) { throw "the $($frames.Count) frames of $src hold $bytes bytes, more than the 2 GB the copy may put on C:; nothing was copied (use -Limit)" }

    $work = "$P0\tmp\ocrs-trial\$Slug-$Label-$tag"
    if (Test-Path -LiteralPath $work) { Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction Stop }
    New-Item -ItemType Directory -Force -Path $work -ErrorAction Stop | Out-Null
    foreach ($f in $frames) { Copy-Item -LiteralPath $f.FullName -Destination $work -ErrorAction Stop }
    $copied = @(Get-ChildItem -LiteralPath $work -Filter 'f-*.png' -File -ErrorAction Stop).Count
    if ($copied -ne $frames.Count) { throw "the copy in $work holds $copied frames, not $($frames.Count)" }
    "ocrs: $src -> $work ($($frames.Count) of $available frames, $bytes bytes) crop=$Crop exe=$OCRS"

    $jsonlWork = "$work\ocrs-$tag.jsonl"
    $cropArgs = @()
    if ($Crop -gt 0) { $cropArgs = @('--crop', [string]$Crop) }
    $t0 = Get-Date
    & $CAP record --out $runOut -- $OCRS --detection $DetModel --recognition $RecModel --frames $work --out $jsonlWork @cropArgs | Out-Null
    $code = $LASTEXITCODE
    $wallS = [math]::Round(((Get-Date) - $t0).TotalSeconds, 1)
    if ($code -ne 0) { throw "ocrs-trial exited $code; see $runOut.stderr" }

    $rows = @(Get-Content -LiteralPath $jsonlWork -Encoding UTF8 -ErrorAction Stop | Where-Object { $_.Trim() -ne '' } | ForEach-Object { $_ | ConvertFrom-Json })
    $records = @($rows | Where-Object { $null -ne $_.PSObject.Properties['frame'] })
    $sum = @($rows | Where-Object { $null -ne $_.PSObject.Properties['summary'] }) | Select-Object -First 1
    if (-not $sum) { throw "$jsonlWork has no summary line: ocrs-trial stopped early" }
    $sum = $sum.summary
    if ($records.Count -ne $frames.Count -or [int]$sum.frames -ne $frames.Count) {
        throw "ocrs-trial returned $($records.Count) frame records (summary: $($sum.frames)) for $($frames.Count) frames"
    }
    if ([int]$sum.crop_pad -ne $Crop) { throw "ocrs-trial's summary says crop_pad $($sum.crop_pad), not $Crop" }
    $detSha1 = Get-Sha1 $DetModel
    $recSha1 = Get-Sha1 $RecModel
    if ($sum.models.detection -ne $detSha1 -or $sum.models.recognition -ne $recSha1) {
        throw "the model SHA-1 in the output ($($sum.models.detection), $($sum.models.recognition)) differ from Get-FileHash ($detSha1, $recSha1)"
    }

    $allLines = @()
    $withText = 0
    $shown = 0
    foreach ($r in $records) {
        $lines = @($r.lines | ForEach-Object { ([string]$_).Trim() } | Where-Object { $_ -ne '' })
        if ($lines.Count -eq 0) { continue }
        $withText++
        $allLines += $lines
        if ($shown -lt $Show) { "  ${tag} $($r.file) | $($lines -join ' | ')"; $shown++ }
    }
    $counts = Count-Markers $allLines
    "stream s:${Stream} crop=$Crop frames=$($records.Count) text_frames=$withText lines=$($allLines.Count) " +
        "bracket=$($counts.bracket) note=$($counts.note) hash=$($counts.hash) speaker=$($counts.speaker) upper=$($counts.upper) " +
        "ms_per_frame=$($sum.ms_per_frame) read_frames=$($sum.read_frames) ms_per_read_frame=$($sum.ms_per_read_frame) " +
        "detect_ms=$($sum.detect_ms_per_read_frame) rec_ms=$($sum.rec_ms_per_read_frame) decode_ms=$($sum.decode_ms_per_frame) " +
        "crop_ms=$($sum.crop_ms_per_frame) crop_mean=$($sum.crop_mean_w)x$($sum.crop_mean_h) load_ms=$($sum.load_ms) wall_s=$wallS"

    # The text goes to the tuning dataset; the recording keeps the counts only (D16).
    Copy-Item -LiteralPath $jsonlWork -Destination $jsonlOut -ErrorAction Stop
    $have = (Get-Item -LiteralPath $jsonlWork -ErrorAction Stop).Length
    $there = (Get-Item -LiteralPath $jsonlOut -ErrorAction Stop).Length
    if ($there -ne $have) { throw "the copy $jsonlOut holds $there bytes, not $have" }
    $entry = [ordered]@{
        n = $Stream; crop = $Crop; file = (Split-Path -Leaf $jsonlOut); run = (Get-Date).ToString('s')
        frames = $records.Count; frames_available = $available; limit = $Limit
        frame_bytes = $bytes; text_frames = $withText; lines = $allLines.Count; markers = $counts
        ms_per_frame = $sum.ms_per_frame; ms_total = $sum.ms_total; read_frames = $sum.read_frames
        ms_per_read_frame = $sum.ms_per_read_frame; detect_ms_per_read_frame = $sum.detect_ms_per_read_frame
        rec_ms_per_read_frame = $sum.rec_ms_per_read_frame; decode_ms_per_frame = $sum.decode_ms_per_frame
        crop_ms_per_frame = $sum.crop_ms_per_frame; crop_boxes = $sum.crop_boxes
        crop_mean_w = $sum.crop_mean_w; crop_mean_h = $sum.crop_mean_h; load_ms = $sum.load_ms; wall_s = $wallS
        ocrs = $sum.ocrs; exe = $OCRS
        models = [ordered]@{
            detection = [ordered]@{ file = (Split-Path -Leaf $DetModel); sha1 = $detSha1; bytes = (Get-Item -LiteralPath $DetModel -ErrorAction Stop).Length }
            recognition = [ordered]@{ file = (Split-Path -Leaf $RecModel); sha1 = $recSha1; bytes = (Get-Item -LiteralPath $RecModel -ErrorAction Stop).Length }
        }
    }
    $summary = [ordered]@{
        slug = $Slug; label = $Label; folder = $Folder
        streams = @(@($others) + @($entry) | Sort-Object { [int]$_.n }, { [int]$_.crop })
    }
    Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction Stop
    Set-Content -LiteralPath $summaryFile -Value (ConvertTo-Json -Depth 6 $summary) -Encoding UTF8 -ErrorAction Stop
    "written: $jsonlOut and $summaryFile"
    'RESULT ok'
} catch {
    "RESULT FAILED: $($_.Exception.Message)"
    throw
} finally {
    # The copy of the frames never stays on C:, whatever happened; on success it is already gone.
    if ($work -and (Test-Path -LiteralPath $work)) { Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue }
}
