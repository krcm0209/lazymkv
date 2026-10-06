# Experiment 14 (decision D15; design: docs/phase0/experiments/EXP-14-design.md, section 6): reads a ripped title's
# bitmap subtitle tracks with Tesseract so that scripts/phase0/subtitle_text.py can tell SDH, regular, forced and
# commentary subtitle tracks apart by their text. Input: the subtitle-only evidence.mks that probe-title.ps1 left in
# $Staging\<Slug>\<Label>\. It reads the subtitle streams given with -Streams, or else every subtitle stream whose
# language tag is in -Languages (default eng and und; a stream without a tag counts as und; "all" reads every one).
# Of those, only bitmap streams (PGS, VobSub, DVB) are read: a text stream, such as the DVD closed captions that
# MakeMKV converts to SubRip and spec 8.7 sets aside, is printed as "skipped" and listed in summary.json.
# For each: ffmpeg draws the stream (-Render canvas: on a black canvas that ticks at -Fps; -Render changes: only when
# the subtitle changes), keeps one frame per change (mpdecimate), inverts it (dark text on white, which
# Tesseract reads best) and writes PNG frames named by their time to a work folder on C:; tesseract then reads all of
# a stream's frames in one call. The work folder of a stream is deleted once its frames are in the tuning dataset, so
# at most one stream's frames are on C: at any time.
#
# In $Recordings\<Slug>\<Label>\<Folder>\ (small text files, pulled into the corpus; no subtitle text):
#   streams.json         ffprobe's view of evidence.mks
#   ffmpeg-<n>.txt       ffmpeg's messages for stream s:<n> (.stderr holds the showinfo line of every kept frame)
#   sub-<n>.frames.json  one record per kept frame: frame number, file, time in seconds (t, from the start of the
#                        file; pts, the stream's own time), characters, lines and the marker counts of its text
#   summary.json         render mode, languages, the skipped text streams, and per stream: frames, frames with text,
#                        text lines, marker counts (bracketed lines, music notes, lines beginning with #, speaker
#                        labels, upper-case lines), the time offset and run times. Written last.
# In $TuningData\<Slug>\<Label>\<Folder>\ (the tuning dataset, decision D16; the full text stays there):
#   s<n>\f-<pts>.png     the frames, so that OCR can be re-run with other settings without the rip
#   s<n>\list.txt        the frame list given to tesseract
#   s<n>\tesseract.txt   tesseract's raw output (pages separated by form feeds) with .stderr/.timing.jsonl/.exit.json
#   sub-<n>.ocr.jsonl    one line per frame: frame number, file, t, pts, the text (lines joined with " | ", kept for
#                        older readers) and lines (the same lines as an array, never joined)
#   examples.json        per stream and marker, up to <Examples> example lines, each cut at 60 characters
#   evidence.mks         a copy of the subtitle-only file: the bitmaps with their palettes (colour, glyph shapes,
#                        another OCR engine can be tried later without the disc)
# The log shows the first <Show> frames with text of every stream so that the OCR quality can be judged; the log
# stays on the mini PC.
# An existing <Folder> (in either place) is refused unless -Force is given; a second reading of a title uses another
# -Folder (for example subs-ocr-changes) instead.
# -Languages and -Streams take lists (eng,und or 0,1): job.sh passes its arguments inside -Command, where the comma
# builds an array, so both are declared as arrays.
# The last line is "RESULT ok" or "RESULT FAILED: <reason>"; after a failure the script throws.
param(
    [Parameter(Mandatory)][string]$Slug,
    [Parameter(Mandatory)][string]$Label,
    [int[]]$Streams = @(),
    [string[]]$Languages = @('eng', 'und'),
    [int]$Fps = 4,
    [string]$Canvas = '',
    [string]$Lang = 'eng',
    [int]$Psm = 6,
    [string]$Render = 'canvas',
    [string]$Folder = 'subs-ocr',
    [int]$Show = 30,
    [int]$Examples = 12,
    [switch]$Force
)
. C:\lazymkv\src\scripts\phase0\env.ps1

$FF = [string][char]12
$NOTE = [string][char]0x266A + [char]0x266B

function Count-Markers([string[]]$Lines) {
    $m = [ordered]@{ bracket = 0; note = 0; hash = 0; speaker = 0; upper = 0 }
    $ex = [ordered]@{ bracket = @(); note = @(); hash = @(); speaker = @(); upper = @() }
    foreach ($line in $Lines) {
        $hits = @()
        if ($line -match '[\[\(].*[\]\)]') { $hits += 'bracket' }
        if ($line.IndexOfAny($NOTE.ToCharArray()) -ge 0) { $hits += 'note' }
        if ($line -match '^\s*#|#\s*$') { $hits += 'hash' }
        if ($line -cmatch '^\s*[A-Z][A-Z0-9 .''\-]{0,24}:(\s|$)') { $hits += 'speaker' }
        if ($line -cnotmatch '[a-z]' -and $line -cmatch '[A-Z].*[A-Z].*[A-Z]') { $hits += 'upper' }
        foreach ($h in $hits) {
            $m[$h]++
            if ($ex[$h].Count -lt $Examples) { $ex[$h] += @($(if ($line.Length -gt 60) { $line.Substring(0, 60) } else { $line })) }
        }
    }
    @{ counts = $m; examples = $ex }
}

# The language tag of a subtitle stream in ffprobe's view, lower case; a stream without one counts as und.
function Get-SubLanguage($Stream) {
    $tag = [string]$Stream.tags.language
    if ($tag) { $tag.ToLowerInvariant() } else { 'und' }
}

try {
    Assert-Slug $Slug
    Assert-Slug $Label
    Assert-Slug $Folder
    if ($Fps -lt 1 -or $Fps -gt 25) { throw "-Fps must be between 1 and 25" }
    if ($Canvas -and $Canvas -notmatch '^\d{2,4}x\d{2,4}$') { throw "-Canvas must look like 1920x1080" }
    if ($Render -cnotin @('canvas', 'changes')) { throw '-Render must be canvas or changes' }
    # Through job.sh -Languages eng,und arrives as an array; through powershell -File as one string with commas.
    $Languages = @($Languages | ForEach-Object { $_ -split ',' } | ForEach-Object { $_.Trim() } | Where-Object { $_ -ne '' })
    if ($Languages.Count -eq 0) { throw '-Languages is empty: give language tags such as eng,und, or all' }
    foreach ($l in $Languages) {
        if ($l -cnotmatch '^([a-z]{2,3}|all)$') { throw "-Languages takes lower-case language tags such as eng,und, or all; '$l' is neither" }
    }
    if ($Languages -ccontains 'all' -and $Languages.Count -gt 1) { throw '-Languages all reads every subtitle stream and takes no other tag' }
    if (-not $FFMPEG -or -not $FFPROBE) { throw 'ffmpeg/ffprobe not found' }
    if (-not $TESSERACT) { throw 'tesseract.exe not found (winget install UB-Mannheim.TesseractOCR)' }
    if (-not (Test-Path -LiteralPath $CAP)) { throw "missing $CAP; build it with: scripts/box.sh cargo build --release -p lazymkv-capture" }
    if (-not $TuningData) { throw "`$TuningData is not set (C:\lazymkv\phase0\config.ps1)" }

    $evidence = "$Staging\$Slug\$Label\evidence.mks"
    if (-not (Test-Path -LiteralPath $evidence)) { throw "no $evidence; run probe-title.ps1 first" }
    $rec = "$Recordings\$Slug\$Label"
    if (-not (Test-Path -LiteralPath $rec)) { throw "no recording $rec; run probe-title.ps1 first" }
    $out = "$rec\$Folder"
    $tune = "$TuningData\$Slug\$Label\$Folder"
    $work = "$P0\work\subs-ocr\$Slug-$Label"
    foreach ($dir in @($out, $tune)) {
        if ((Test-Path -LiteralPath $dir) -and -not $Force) { throw "$dir already exists and is kept as evidence; use -Force to replace it" }
    }
    foreach ($dir in @($out, $tune, $work)) {
        if (Test-Path -LiteralPath $dir) { Remove-Item -LiteralPath $dir -Recurse -Force -ErrorAction Stop }
        New-Item -ItemType Directory -Force -Path $dir -ErrorAction Stop | Out-Null
    }
    "ocr: $evidence tesseract=$TESSERACT lang=$Lang psm=$Psm fps=$Fps render=$Render"

    & $CAP record --out "$out\streams.json" -- $FFPROBE -v error -show_streams -show_format -of json $evidence | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "ffprobe exited $LASTEXITCODE; see streams.json.stderr" }
    $probe = Get-Content -LiteralPath "$out\streams.json" -Raw -ErrorAction Stop | ConvertFrom-Json
    $subs = @($probe.streams | Where-Object { $_.codec_type -eq 'subtitle' })
    if ($subs.Count -eq 0) { throw "$evidence has no subtitle stream" }
    $duration = [double]$probe.format.duration
    if ($duration -le 0) { throw "ffprobe reports no duration for $evidence" }
    # ffmpeg counts the frame times from the file's start; adding the file's start_time gives the stream's own time
    # (the packets' pts_time in subs\sub-<n>.packets.json).
    $ptsOffset = if ($probe.format.start_time) { [math]::Round([double]$probe.format.start_time, 3) } else { 0.0 }
    if ($Streams.Count -gt 0) {
        $wanted = $Streams
        foreach ($n in $wanted) { if ($n -lt 0 -or $n -ge $subs.Count) { throw "no subtitle stream s:$n (the file has $($subs.Count))" } }
    } else {
        $wanted = @(0..($subs.Count - 1) | Where-Object { $Languages -ccontains 'all' -or $Languages -ccontains (Get-SubLanguage $subs[$_]) })
        if ($wanted.Count -eq 0) { throw "no subtitle stream with a language in $($Languages -join ',')" }
    }
    # Spec 8.7 sets the DVD closed-caption stream aside (MakeMKV converts it to text), and sub2video draws bitmap
    # subtitles only: a chosen stream of any other codec is skipped and recorded, never rendered.
    $bitmapCodecs = @('hdmv_pgs_subtitle', 'dvd_subtitle', 'dvb_subtitle')
    $read = @()
    $skipped = @()
    foreach ($n in $wanted) {
        $codec = [string]$subs[$n].codec_name
        if ($bitmapCodecs -ccontains $codec) {
            $read += $n
        } else {
            "skipped s:${n} (${codec}): not a bitmap subtitle track"
            $skipped += [ordered]@{ n = $n; language = $subs[$n].tags.language; codec = $codec; reason = 'not a bitmap subtitle track' }
        }
    }
    if ($read.Count -eq 0) {
        $names = @($skipped | ForEach-Object { 's:' + $_.n + ' ' + $_.codec }) -join ', '
        throw "no bitmap subtitle stream to read: the chosen streams are text streams ($names)"
    }
    $wanted = $read

    $summary = [ordered]@{
        slug = $Slug; label = $Label; folder = $Folder; evidence = $evidence; duration_s = [math]::Round($duration, 3)
        render = $Render; languages = @($Languages); fps = $Fps; lang = $Lang; psm = $Psm; tesseract = $TESSERACT
        run = (Get-Date).ToString('s'); skipped = @($skipped); streams = @()
    }
    $exampleStreams = @()
    foreach ($n in $wanted) {
        $s = $subs[$n]
        $size = if ($Canvas) { $Canvas } elseif ([int]$s.width -gt 0 -and [int]$s.height -gt 0) { "$($s.width)x$($s.height)" } else { '1920x1080' }
        $sdir = "$work\s$n"
        New-Item -ItemType Directory -Force -Path $sdir -ErrorAction Stop | Out-Null
        if ($Render -eq 'canvas') {
            # sub2video: ffmpeg turns the bitmap subtitle stream into video frames of -canvas_size, overlaid on a black
            # canvas that ticks at -Fps; mpdecimate keeps only the frames that differ from the one before.
            $graph = "color=c=black:s=${size}:r=${Fps}:d=${duration}[bg];[bg][0:s:${n}]overlay=format=auto[ov];[ov]mpdecimate,negate,format=gray,showinfo[v]"
        } else {
            # Timed in plan Task 12 Step 25: no canvas and no overlay, so that ffmpeg emits a frame only when the
            # subtitle changes.
            $graph = "[0:s:${n}]negate,format=gray,mpdecimate,showinfo[v]"
        }
        # canvas: vfr drops nothing (mpdecimate already thinned the ticking canvas) and the file name carries the
        # frame's tick. changes: two frames can share a timestamp (a subtitle replaced in the same instant), which
        # vfr would drop and -frame_pts would overwrite; passthrough with plain numbering keeps every frame
        # showinfo logged, and the time comes from showinfo by index.
        $muxArgs = if ($Render -eq 'canvas') { @('-fps_mode', 'vfr', '-frame_pts', '1') } else { @('-fps_mode', 'passthrough') }
        $t0 = Get-Date
        & $CAP record --out "$out\ffmpeg-$n.txt" -- $FFMPEG -hide_banner -nostats -v info -y -canvas_size $size -i $evidence `
            -filter_complex $graph -map '[v]' @muxArgs -f image2 "$sdir\f-%08d.png" | Out-Null
        $ffmpegS = [math]::Round(((Get-Date) - $t0).TotalSeconds, 1)
        if ($LASTEXITCODE -ne 0) { throw "ffmpeg exited $LASTEXITCODE on stream s:${n}; see ffmpeg-$n.txt.stderr" }
        $frames = @(Get-ChildItem -LiteralPath $sdir -Filter 'f-*.png' -ErrorAction Stop | Sort-Object Name)
        if ($frames.Count -eq 0) { throw "ffmpeg wrote no frame for stream s:${n}; see ffmpeg-$n.txt.stderr" }
        # showinfo prints one line per kept frame, in output order: its pts_time is the frame's time.
        $info = @(Select-String -LiteralPath "$out\ffmpeg-$n.txt.stderr" -Pattern 'Parsed_showinfo.*\bn:\s*(\d+).*\bpts_time:\s*([0-9.]+)' -ErrorAction Stop)
        $times = @{}
        foreach ($line in $info) { $times[[int]$line.Matches[0].Groups[1].Value] = [double]$line.Matches[0].Groups[2].Value }
        if ($times.Count -ne $frames.Count) {
            # Only the canvas ticks at -Fps, so only there can a frame's time be read from its file name.
            if ($Render -ne 'canvas') { throw "ffmpeg gave $($times.Count) frame times (showinfo) for $($frames.Count) frames of stream s:${n}; with -Render changes no time can come from the file names" }
            "warning: s:${n} showinfo lines=$($times.Count) frames=$($frames.Count); times come from the file names"
        }

        $list = "$sdir\list.txt"
        Set-Content -LiteralPath $list -Value ($frames | ForEach-Object { $_.FullName }) -Encoding Ascii -ErrorAction Stop
        $t1 = Get-Date
        & $CAP record --out "$sdir\tesseract.txt" -- $TESSERACT $list stdout -l $Lang --psm $Psm | Out-Null
        $tessS = [math]::Round(((Get-Date) - $t1).TotalSeconds, 1)
        if ($LASTEXITCODE -ne 0) { throw "tesseract exited $LASTEXITCODE on stream s:${n}; see $sdir\tesseract.txt.stderr" }
        $raw = [IO.File]::ReadAllText("$sdir\tesseract.txt", [Text.Encoding]::UTF8)
        $pages = @($raw -split $FF)
        if ($pages.Count -eq $frames.Count + 1 -and $pages[-1].Trim() -eq '') { $pages = $pages[0..($frames.Count - 1)] }
        if ($pages.Count -ne $frames.Count) { throw "tesseract returned $($pages.Count) pages for $($frames.Count) frames of stream s:${n}" }

        $records = @()
        $jsonl = @()
        $allLines = @()
        $shown = 0
        for ($k = 0; $k -lt $frames.Count; $k++) {
            $lines = @($pages[$k] -split "`r?`n" | ForEach-Object { $_.Trim() } | Where-Object { $_ -ne '' })
            $text = $lines -join ' | '
            $t = if ($times.ContainsKey($k)) { $times[$k] } else { [math]::Round([int]($frames[$k].BaseName.Substring(2)) / $Fps, 3) }
            $pts = [math]::Round($t + $ptsOffset, 3)
            $mk = Count-Markers $lines
            $records += [ordered]@{ frame = $k; file = $frames[$k].Name; t = $t; pts = $pts; chars = ($text -replace ' \| ', '').Length; lines = $lines.Count; markers = $mk.counts }
            $jsonl += (ConvertTo-Json -Compress ([ordered]@{ frame = $k; file = $frames[$k].Name; t = $t; pts = $pts; text = $text; lines = [string[]]$lines }))
            $allLines += $lines
            if ($lines.Count -gt 0 -and $shown -lt $Show) { "  s:${n} t=$t | $text"; $shown++ }
        }
        $withText = @($records | Where-Object { $_.chars -gt 0 }).Count
        $all = Count-Markers $allLines
        "stream s:${n} lang=$($s.tags.language) codec=$($s.codec_name) canvas=$size frames=$($frames.Count) text_frames=$withText lines=$($allLines.Count) " +
            "bracket=$($all.counts.bracket) note=$($all.counts.note) hash=$($all.counts.hash) speaker=$($all.counts.speaker) upper=$($all.counts.upper) ffmpeg_s=$ffmpegS tesseract_s=$tessS pts_offset=$ptsOffset"
        [IO.File]::WriteAllLines("$work\sub-$n.ocr.jsonl", [string[]]$jsonl, (New-Object Text.UTF8Encoding $false))
        Set-Content -LiteralPath "$out\sub-$n.frames.json" -Value (ConvertTo-Json -Depth 4 @($records)) -Encoding UTF8 -ErrorAction Stop
        # The recording keeps the counts only; example lines are subtitle text and go to the tuning dataset (D16).
        $summary.streams += [ordered]@{
            n = $n; language = $s.tags.language; codec = $s.codec_name; canvas = $size
            frames = $frames.Count; text_frames = $withText; lines = $allLines.Count
            markers = $all.counts; pts_offset = $ptsOffset; ffmpeg_s = $ffmpegS; tesseract_s = $tessS
        }
        $exampleStreams += [ordered]@{ n = $n; language = $s.tags.language; examples = $all.examples }
        # The frames, tesseract's output and the full text go to the tuning dataset; then the stream's frames leave C:.
        Copy-Item -LiteralPath $sdir -Destination "$tune\s$n" -Recurse -ErrorAction Stop
        Copy-Item -LiteralPath "$work\sub-$n.ocr.jsonl" -Destination "$tune\sub-$n.ocr.jsonl" -ErrorAction Stop
        $copied = @(Get-ChildItem -LiteralPath "$tune\s$n" -Filter 'f-*.png' -ErrorAction Stop).Count
        if ($copied -ne $frames.Count) { throw "the copy of stream s:${n} in $tune\s$n holds $copied frames, not $($frames.Count)" }
        Remove-Item -LiteralPath $sdir -Recurse -Force -ErrorAction Stop
    }
    Set-Content -LiteralPath "$tune\examples.json" -Value (ConvertTo-Json -Depth 6 ([ordered]@{ slug = $Slug; label = $Label; folder = $Folder; streams = @($exampleStreams) })) -Encoding UTF8 -ErrorAction Stop
    # The bitmaps with their palettes, for later measures (colour, glyph shapes, another OCR engine) without the disc.
    Copy-Item -LiteralPath $evidence -Destination "$tune\evidence.mks" -ErrorAction Stop
    $evBytes = (Get-Item -LiteralPath $evidence -ErrorAction Stop).Length
    $copyBytes = (Get-Item -LiteralPath "$tune\evidence.mks" -ErrorAction Stop).Length
    if ($copyBytes -ne $evBytes) { throw "the copy of evidence.mks in $tune holds $copyBytes bytes, not $evBytes" }
    "evidence: copied $copyBytes bytes"
    Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction Stop
    Set-Content -LiteralPath "$out\summary.json" -Value (ConvertTo-Json -Depth 6 $summary) -Encoding UTF8 -ErrorAction Stop
    "written: $out and $tune"
    'RESULT ok'
} catch {
    "RESULT FAILED: $($_.Exception.Message)"
    throw
}
